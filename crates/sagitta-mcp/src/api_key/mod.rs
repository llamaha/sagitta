use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{Result, anyhow};
use uuid::Uuid;
use dashmap::DashMap;
use std::sync::Arc;
use tracing;
use async_trait;

pub const API_KEY_PREFIX: &str = "vdb_sk_"; // Made public
const API_KEY_LENGTH: usize = 32; // Length of the random part of the key

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiKey {
    pub id: String, 
    pub key: String, 
    pub tenant_id: String,
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
        tenant_id: String,
        user_id: Option<String>,
        description: Option<String>,
        scopes: Vec<String>,
        expires_at: Option<u64>,
    ) -> Self {
        Self {
            id,
            key,
            tenant_id,
            user_id,
            description,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
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
        if let Some(expiry) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now >= expiry {
                return false;
            }
        }
        true
    }

    // Method to update last_used_at, might be useful later
    pub fn update_last_used(&mut self) {
        self.last_used_at = Some(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs());
    }
}

// Basic random string generation. In a real system, use a crypto-secure RNG.
fn generate_api_key_string(len: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

// New struct for listing API keys, omitting the raw key value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiKeyInfo {
    pub id: String,
    pub tenant_id: String,
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
    // Add a prefix or a few characters of the key for identification, e.g., key_preview
    pub key_preview: String, 
}

impl From<&ApiKey> for ApiKeyInfo {
    fn from(api_key: &ApiKey) -> Self {
        Self {
            id: api_key.id.clone(),
            tenant_id: api_key.tenant_id.clone(),
            user_id: api_key.user_id.clone(),
            description: api_key.description.clone(),
            created_at: api_key.created_at,
            expires_at: api_key.expires_at,
            last_used_at: api_key.last_used_at,
            scopes: api_key.scopes.clone(),
            revoked: api_key.revoked,
            key_preview: format!("{}{}", API_KEY_PREFIX, &api_key.key[API_KEY_PREFIX.len()..std::cmp::min(api_key.key.len(), API_KEY_PREFIX.len() + 4)]), // Show prefix + first 4 chars of random part
        }
    }
}

#[async_trait::async_trait]
pub trait ApiKeyStore: Send + Sync {
    async fn create_key(
        &self,
        tenant_id: String,
        user_id: Option<String>,
        description: Option<String>,
        scopes: Vec<String>,
        expires_at: Option<u64>,
    ) -> Result<ApiKey>;

    async fn get_key_by_id(&self, key_id: &str) -> Option<ApiKey>;
    async fn get_key_by_value(&self, key_value: &str) -> Option<ApiKey>;
    async fn list_keys_info(&self, tenant_id_filter: Option<&str>, user_id_filter: Option<&str>) -> Vec<ApiKeyInfo>;
    async fn revoke_key(&self, key_id: &str) -> Result<bool>;
    async fn record_key_usage(&self, key_id: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct InMemoryApiKeyStore { // Renamed from ApiKeyStore
    keys: Arc<DashMap<String, ApiKey>>,
}

// Implementation for InMemoryApiKeyStore (used to be ApiKeyStore impl)
impl InMemoryApiKeyStore { // Renamed from ApiKeyStore
    pub fn new() -> Self {
        Self { keys: Arc::new(DashMap::new()) }
    }

    // Note: The method implementations (create_key, get_key_by_id, etc.) 
    // that were previously under `impl ApiKeyStore` will now be part of 
    // `impl ApiKeyStore for InMemoryApiKeyStore` or directly on `InMemoryApiKeyStore`
    // if they are helper/private methods not part of the trait.
    // The public methods matching the trait will be moved to the trait impl block.

    /// Insert a key with a specific value (for test/dev bootstrapping only)
    pub async fn insert_key_with_value(
        &self,
        key_value: String,
        tenant_id: String,
        user_id: Option<String>,
        description: Option<String>,
        scopes: Vec<String>,
        expires_at: Option<u64>,
    ) -> ApiKey {
        let key_id = Uuid::new_v4().to_string();
        let api_key = ApiKey::new_internal(
            key_id.clone(),
            key_value,
            tenant_id,
            user_id,
            description,
            scopes,
            expires_at,
        );
        self.keys.insert(key_id, api_key.clone());
        api_key
    }
}

impl Default for InMemoryApiKeyStore { // Renamed from ApiKeyStore
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ApiKeyStore for InMemoryApiKeyStore { // Implement the trait
    async fn create_key(
        &self,
        tenant_id: String,
        user_id: Option<String>,
        description: Option<String>,
        scopes: Vec<String>,
        expires_at: Option<u64>,
    ) -> Result<ApiKey> {
        let key_id = Uuid::new_v4().to_string();
        let random_part = generate_api_key_string(API_KEY_LENGTH);
        let full_key_string = format!("{}{}", API_KEY_PREFIX, random_part);

        let api_key = ApiKey::new_internal(
            key_id.clone(),
            full_key_string.clone(),
            tenant_id,
            user_id,
            description,
            scopes,
            expires_at,
        );

        self.keys.insert(key_id, api_key.clone());
        Ok(api_key)
    }

    async fn get_key_by_id(&self, key_id: &str) -> Option<ApiKey> {
        self.keys.get(key_id).map(|entry| entry.value().clone())
    }

    async fn get_key_by_value(&self, key_value_to_find: &str) -> Option<ApiKey> {
        let trimmed_key_to_find = key_value_to_find.trim();
        tracing::info!(
            "InMemoryApiKeyStore: get_key_by_value called for (trimmed) key: [{}]. Current store size: {}. Original input: [{}]", 
            trimmed_key_to_find, 
            self.keys.len(),
            key_value_to_find
        );
        for entry in self.keys.iter() {
            let stored_key_id = entry.key(); 
            let stored_api_key_struct = entry.value();
            let trimmed_stored_key = stored_api_key_struct.key.trim();
            tracing::debug!(
                "InMemoryApiKeyStore: Comparing (trimmed) [{}] with stored key id: [{}], (trimmed) value: [{}]", 
                trimmed_key_to_find,
                stored_key_id, 
                trimmed_stored_key
            );
            if trimmed_stored_key == trimmed_key_to_find {
                tracing::info!("InMemoryApiKeyStore: Found key with id: {}", stored_api_key_struct.id);
                return Some(stored_api_key_struct.clone());
            }
        }
        tracing::warn!("InMemoryApiKeyStore: Key not found for (trimmed) value: [{}]", trimmed_key_to_find);
        None
    }

    async fn list_keys_info(&self, tenant_id_filter: Option<&str>, user_id_filter: Option<&str>) -> Vec<ApiKeyInfo> {
        self.keys
            .iter()
            .filter_map(|entry| {
                let key = entry.value();
                let tenant_match = tenant_id_filter.map_or(true, |tid| key.tenant_id == tid);
                let user_match = user_id_filter.map_or(true, |uid| key.user_id.as_deref() == Some(uid));
                if tenant_match && user_match {
                    Some(ApiKeyInfo::from(key))
                } else {
                    None
                }
            })
            .collect()
    }

    async fn revoke_key(&self, key_id: &str) -> Result<bool> {
        if let Some(mut entry) = self.keys.get_mut(key_id) {
            entry.value_mut().revoked = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    async fn record_key_usage(&self, key_id: &str) -> Result<()> {
        if let Some(mut entry) = self.keys.get_mut(key_id) {
            entry.value_mut().update_last_used();
            Ok(())
        } else {
            Err(anyhow!("API key not found for recording usage"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    // Test for internal ApiKey struct constructor if needed, but it's simple.
    // The main ApiKey tests remain relevant for its properties.
    #[test]
    fn test_new_api_key_struct_properties() { // Renamed from test_new_api_key
        let key_id = Uuid::new_v4().to_string();
        let key_value = generate_api_key_string(API_KEY_LENGTH);
        let full_key = format!("{}{}", API_KEY_PREFIX, key_value);

        let key = ApiKey::new_internal(
            key_id.clone(),
            full_key.clone(),
            "tenant1".to_string(),
            Some("user1".to_string()),
            Some("Test key".to_string()),
            vec!["read".to_string(), "write".to_string()],
            None,
        );

        assert_eq!(key.id, key_id);
        assert_eq!(key.key, full_key);
        assert!(key.key.starts_with(API_KEY_PREFIX));
        assert_eq!(key.key.len(), API_KEY_PREFIX.len() + API_KEY_LENGTH);
        assert_eq!(key.tenant_id, "tenant1".to_string());
        assert_eq!(key.user_id, Some("user1".to_string()));
        assert_eq!(key.description, Some("Test key".to_string()));
        assert!(key.created_at > 0);
        assert_eq!(key.expires_at, None);
        assert_eq!(key.scopes, vec!["read".to_string(), "write".to_string()]);
        assert!(!key.revoked);
        assert!(key.is_valid());
    }

    #[test]
    fn test_api_key_expiry() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let key_id = Uuid::new_v4().to_string();
        let key_val = format!("{}{}", API_KEY_PREFIX, generate_api_key_string(API_KEY_LENGTH));
        let mut key = ApiKey::new_internal(key_id, key_val, "default_tenant".to_string(), None, None, vec![], Some(now + 1)); // Expires in 1 second
        assert!(key.is_valid());
        sleep(Duration::from_secs(2));
        assert!(!key.is_valid());

        key.expires_at = None;
        assert!(key.is_valid()); // No expiry means valid
    }

    #[test]
    fn test_api_key_revoked() {
        let key_id = Uuid::new_v4().to_string();
        let key_val = format!("{}{}", API_KEY_PREFIX, generate_api_key_string(API_KEY_LENGTH));
        let mut key = ApiKey::new_internal(key_id, key_val, "default_tenant".to_string(), None, None, vec![], None);
        assert!(key.is_valid());
        key.revoked = true;
        assert!(!key.is_valid());
    }

    #[tokio::test]
    async fn test_api_key_store_create_get() {
        let store = InMemoryApiKeyStore::new();
        let tenant_id = "t1".to_string();
        let user_id = Some("u1".to_string());
        let description = Some("desc".to_string());
        let scopes = vec!["s1".to_string()];

        let created_key = store.create_key(tenant_id.clone(), user_id.clone(), description.clone(), scopes.clone(), None).await.unwrap();
        
        assert!(created_key.key.starts_with(API_KEY_PREFIX));
        assert_eq!(created_key.tenant_id, tenant_id);
        assert_eq!(created_key.user_id, user_id);
        assert_eq!(created_key.description, description);
        assert_eq!(created_key.scopes, scopes);

        let retrieved_key_by_id = store.get_key_by_id(&created_key.id).await.unwrap();
        assert_eq!(created_key, retrieved_key_by_id);
        // Note: retrieved_key_by_id.key will be the same as created_key.key because we store the full key for now.

        let retrieved_key_by_value = store.get_key_by_value(&created_key.key).await.unwrap();
        assert_eq!(created_key, retrieved_key_by_value);
    }

    #[tokio::test]
    async fn test_api_key_store_list_keys_info() {
        let store = InMemoryApiKeyStore::new();
        let key1 = store.create_key("t1".to_string(), Some("u1".to_string()), Some("desc1".to_string()), vec!["s1".to_string()], None).await.unwrap();
        store.create_key("t1".to_string(), Some("u2".to_string()), None, vec![], None).await.unwrap();
        store.create_key("t2".to_string(), Some("u1".to_string()), None, vec![], None).await.unwrap();

        let all_keys_info = store.list_keys_info(None, None).await;
        assert_eq!(all_keys_info.len(), 3);

        let t1_keys_info = store.list_keys_info(Some("t1"), None).await;
        assert_eq!(t1_keys_info.len(), 2);
        assert!(t1_keys_info.iter().all(|k| k.tenant_id == "t1".to_string()));

        let u1_keys_info = store.list_keys_info(None, Some("u1")).await;
        assert_eq!(u1_keys_info.len(), 2);
        assert!(u1_keys_info.iter().all(|k| k.user_id == Some("u1".to_string())));

        let t1_u1_keys_info = store.list_keys_info(Some("t1"), Some("u1")).await;
        assert_eq!(t1_u1_keys_info.len(), 1);
        let key_info = &t1_u1_keys_info[0];
        assert_eq!(key_info.id, key1.id);
        assert_eq!(key_info.tenant_id, "t1".to_string());
        assert_eq!(key_info.user_id, Some("u1".to_string()));
        assert_eq!(key_info.description, Some("desc1".to_string()));
        assert_eq!(key_info.scopes, vec!["s1".to_string()]);
        assert!(key_info.key_preview.starts_with(API_KEY_PREFIX));
        assert_eq!(key_info.key_preview.len(), API_KEY_PREFIX.len() + 4);
    }

    #[tokio::test]
    async fn test_api_key_store_revoke() {
        let store = InMemoryApiKeyStore::new();
        let key = store.create_key("default_tenant".to_string(), None, None, vec![], None).await.unwrap();
        assert!(key.is_valid());

        let retrieved_before_revoke = store.get_key_by_id(&key.id).await.unwrap();
        assert!(retrieved_before_revoke.is_valid());

        let revoke_result = store.revoke_key(&key.id).await.unwrap();
        assert!(revoke_result);

        let retrieved_after_revoke = store.get_key_by_id(&key.id).await.unwrap();
        assert!(!retrieved_after_revoke.is_valid());
        assert!(retrieved_after_revoke.revoked);

        let revoke_nonexistent = store.revoke_key("nonexistent").await.unwrap();
        assert!(!revoke_nonexistent);
    }
    
    #[tokio::test]
    async fn test_record_key_usage() {
        let store = InMemoryApiKeyStore::new();
        let key = store.create_key("default_tenant".to_string(), None, None, vec![], None).await.unwrap();
        assert!(key.last_used_at.is_none());

        store.record_key_usage(&key.id).await.unwrap();
        let updated_key = store.get_key_by_id(&key.id).await.unwrap();
        assert!(updated_key.last_used_at.is_some());
        assert!(updated_key.last_used_at.unwrap() >= key.created_at);

        let result = store.record_key_usage("nonexistent_id").await;
        assert!(result.is_err());
    }
} 