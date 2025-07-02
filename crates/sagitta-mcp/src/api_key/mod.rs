use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{Result, anyhow};
use uuid::Uuid;
use dashmap::DashMap;
use std::sync::Arc;
use tracing;
use async_trait::async_trait;

pub const API_KEY_PREFIX: &str = "vdb_sk_"; // Made public
const API_KEY_LENGTH: usize = 32; // Length of the random part of the key

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiKey {
    pub id: String, 
    pub key: String, 
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,  
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: u64, 
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<u64>,
    pub scopes: Vec<String>,
    pub revoked: bool,
}

impl ApiKey {
    // Constructor remains largely the same, but key generation will be internal to ApiKeyStore
    // to ensure the raw key is returned only upon creation.
    fn new_internal(
        id: String,
        key: String,
        user_id: Option<String>,
        description: Option<String>,
        scopes: Vec<String>,
        expires_at: Option<u64>,
    ) -> Self {
        Self {
            id,
            key,
            user_id,
            description,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs(),
            expires_at,
            last_used_at: None,
            scopes,
            revoked: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        if self.revoked {
            return false;
        }
        if let Some(exp) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs();
            return exp > now;
        }
        true
    }
}

// Response object for when API keys are created
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyCreated {
    #[serde(flatten)]
    pub api_key: ApiKey,
    pub raw_key: String, // The full key is returned only upon creation
}

// Return object when validating a key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub key_id: String,
    pub user_id: Option<String>,
    pub scopes: Vec<String>,
}

#[async_trait]
pub trait ApiKeyStore: Send + Sync {
    async fn create_key(
        &self,
        user_id: Option<String>,
        description: Option<String>,
        scopes: Vec<String>,
        expires_at: Option<u64>,
    ) -> Result<ApiKeyCreated>;
    
    async fn insert_key_with_value(
        &self,
        key_value: String,
        user_id: Option<String>,
        description: Option<String>,
        scopes: Vec<String>,
        expires_at: Option<u64>,
    ) -> Result<ApiKeyCreated>;
    
    async fn get_key(&self, id: &str) -> Option<ApiKey>;
    async fn get_key_by_value(&self, key_value: &str) -> Option<ApiKeyInfo>;
    async fn list_keys(&self) -> Vec<ApiKey>;
    async fn revoke_key(&self, id: &str) -> Result<()>;
    async fn update_last_used(&self, id: &str) -> Result<()>;
    async fn delete_key(&self, id: &str) -> Result<()>;
}

#[derive(Default)]
pub struct InMemoryApiKeyStore {
    keys: Arc<DashMap<String, ApiKey>>,
}

impl InMemoryApiKeyStore {
    pub fn new() -> Self {
        Self {
            keys: Arc::new(DashMap::new()),
        }
    }

    fn generate_key() -> String {
        use rand::{thread_rng, Rng};
        use rand::distributions::Alphanumeric;
        
        let random_part: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(API_KEY_LENGTH)
            .map(char::from)
            .collect();
        
        format!("{}{}", API_KEY_PREFIX, random_part)
    }

    fn hash_key(key: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

#[async_trait]
impl ApiKeyStore for InMemoryApiKeyStore {
    async fn create_key(
        &self,
        user_id: Option<String>,
        description: Option<String>,
        scopes: Vec<String>,
        expires_at: Option<u64>,
    ) -> Result<ApiKeyCreated> {
        let id = Uuid::new_v4().to_string();
        let raw_key = Self::generate_key();
        let hashed_key = Self::hash_key(&raw_key);
        
        let api_key = ApiKey::new_internal(
            id.clone(),
            hashed_key,
            user_id,
            description,
            scopes,
            expires_at,
        );
        
        self.keys.insert(id.clone(), api_key.clone());
        
        Ok(ApiKeyCreated {
            api_key,
            raw_key,
        })
    }

    async fn insert_key_with_value(
        &self,
        key_value: String,
        user_id: Option<String>,
        description: Option<String>,
        scopes: Vec<String>,
        expires_at: Option<u64>,
    ) -> Result<ApiKeyCreated> {
        let id = Uuid::new_v4().to_string();
        let hashed_key = Self::hash_key(&key_value);
        
        let api_key = ApiKey::new_internal(
            id.clone(),
            hashed_key,
            user_id,
            description,
            scopes,
            expires_at,
        );
        
        self.keys.insert(id.clone(), api_key.clone());
        
        Ok(ApiKeyCreated {
            api_key,
            raw_key: key_value,
        })
    }
    
    async fn get_key(&self, id: &str) -> Option<ApiKey> {
        self.keys.get(id).map(|k| k.clone())
    }
    
    async fn get_key_by_value(&self, key_value: &str) -> Option<ApiKeyInfo> {
        let hashed = Self::hash_key(key_value);
        
        for entry in self.keys.iter() {
            let api_key = entry.value();
            if api_key.key == hashed && api_key.is_valid() {
                // Update last_used timestamp
                let _ = self.update_last_used(&api_key.id).await;
                
                return Some(ApiKeyInfo {
                    key_id: api_key.id.clone(),
                    user_id: api_key.user_id.clone(),
                    scopes: api_key.scopes.clone(),
                });
            }
        }
        None
    }
    
    async fn list_keys(&self) -> Vec<ApiKey> {
        self.keys
            .iter()
            .filter(|entry| !entry.value().revoked)
            .map(|entry| entry.value().clone())
            .collect()
    }
    
    async fn revoke_key(&self, id: &str) -> Result<()> {
        self.keys
            .get_mut(id)
            .map(|mut k| k.revoked = true)
            .ok_or_else(|| anyhow!("Key not found"))
    }
    
    async fn update_last_used(&self, id: &str) -> Result<()> {
        self.keys
            .get_mut(id)
            .map(|mut k| {
                k.last_used_at = Some(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("Time went backwards")
                        .as_secs(),
                )
            })
            .ok_or_else(|| anyhow!("Key not found"))
    }
    
    async fn delete_key(&self, id: &str) -> Result<()> {
        self.keys
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| anyhow!("Key not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_key_lifecycle() {
        let store = InMemoryApiKeyStore::new();
        
        // Create a key
        let created = store
            .create_key(
                Some("user123".to_string()),
                Some("Test key".to_string()),
                vec!["read:repos".to_string()],
                None,
            )
            .await
            .unwrap();
        
        assert!(created.raw_key.starts_with(API_KEY_PREFIX));
        assert_eq!(created.api_key.user_id, Some("user123".to_string()));
        
        // Get by ID
        let key = store.get_key(&created.api_key.id).await.unwrap();
        assert_eq!(key.description, Some("Test key".to_string()));
        
        // Get by value
        let info = store
            .get_key_by_value(&created.raw_key)
            .await
            .unwrap();
        assert_eq!(info.key_id, created.api_key.id);
        assert_eq!(info.user_id, Some("user123".to_string()));
        
        // List keys
        let keys = store.list_keys().await;
        assert_eq!(keys.len(), 1);
        
        // Revoke key
        store.revoke_key(&created.api_key.id).await.unwrap();
        assert!(store.get_key_by_value(&created.raw_key).await.is_none());
        
        // Delete key
        store.delete_key(&created.api_key.id).await.unwrap();
        assert!(store.get_key(&created.api_key.id).await.is_none());
    }
}