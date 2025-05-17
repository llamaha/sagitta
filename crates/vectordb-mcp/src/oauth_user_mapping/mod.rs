use anyhow::{Result, anyhow};
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuthUserTenantMapping {
    pub oauth_user_sub: String, // The 'sub' claim from OAuth UserInfo
    pub tenant_id: String,      // The ID of the tenant this user is mapped to
    pub created_at: u64,
}

impl OAuthUserTenantMapping {
    pub fn new(oauth_user_sub: String, tenant_id: String) -> Self {
        Self {
            oauth_user_sub,
            tenant_id,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MappingStoreError {
    #[error("Mapping already exists for OAuth user sub: {0}")]
    MappingAlreadyExists(String),
    #[error("An internal error occurred: {0}")]
    InternalError(#[from] anyhow::Error),
}

#[async_trait]
pub trait OAuthUserTenantMappingStore: Send + Sync {
    async fn add_mapping(&self, mapping: OAuthUserTenantMapping) -> Result<(), MappingStoreError>;
    async fn get_mapping_by_sub(&self, oauth_user_sub: &str) -> Result<Option<OAuthUserTenantMapping>, MappingStoreError>;
    async fn remove_mapping_by_sub(&self, oauth_user_sub: &str) -> Result<bool, MappingStoreError>;
    async fn list_mappings(&self) -> Result<Vec<OAuthUserTenantMapping>, MappingStoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryOAuthUserTenantMappingStore {
    mappings: Arc<DashMap<String, OAuthUserTenantMapping>>, // Keyed by oauth_user_sub
}

impl InMemoryOAuthUserTenantMappingStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl OAuthUserTenantMappingStore for InMemoryOAuthUserTenantMappingStore {
    async fn add_mapping(&self, mapping: OAuthUserTenantMapping) -> Result<(), MappingStoreError> {
        if self.mappings.contains_key(&mapping.oauth_user_sub) {
            return Err(MappingStoreError::MappingAlreadyExists(mapping.oauth_user_sub.clone()));
        }
        self.mappings.insert(mapping.oauth_user_sub.clone(), mapping);
        Ok(())
    }

    async fn get_mapping_by_sub(&self, oauth_user_sub: &str) -> Result<Option<OAuthUserTenantMapping>, MappingStoreError> {
        Ok(self.mappings.get(oauth_user_sub).map(|entry| entry.value().clone()))
    }

    async fn remove_mapping_by_sub(&self, oauth_user_sub: &str) -> Result<bool, MappingStoreError> {
        Ok(self.mappings.remove(oauth_user_sub).is_some())
    }

    async fn list_mappings(&self) -> Result<Vec<OAuthUserTenantMapping>, MappingStoreError> {
        let mappings_list: Vec<OAuthUserTenantMapping> = self.mappings.iter().map(|entry| entry.value().clone()).collect();
        Ok(mappings_list)
    }
}

#[cfg(test)]
mod tests {
    use super::*; 

    #[tokio::test]
    async fn test_add_and_get_mapping() {
        let store = InMemoryOAuthUserTenantMappingStore::new();
        let mapping1 = OAuthUserTenantMapping::new("user_sub_123".to_string(), "tenant_abc".to_string());

        assert!(store.add_mapping(mapping1.clone()).await.is_ok());

        let retrieved = store.get_mapping_by_sub("user_sub_123").await.unwrap();
        assert_eq!(retrieved, Some(mapping1.clone()));

        let non_existent = store.get_mapping_by_sub("unknown_sub").await.unwrap();
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_add_duplicate_mapping_fails() {
        let store = InMemoryOAuthUserTenantMappingStore::new();
        let mapping1 = OAuthUserTenantMapping::new("user_sub_456".to_string(), "tenant_def".to_string());
        let mapping2 = OAuthUserTenantMapping::new("user_sub_456".to_string(), "tenant_ghi".to_string()); // Same sub

        assert!(store.add_mapping(mapping1.clone()).await.is_ok());
        let result = store.add_mapping(mapping2.clone()).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            MappingStoreError::MappingAlreadyExists(sub) => assert_eq!(sub, "user_sub_456"),
            _ => panic!("Expected MappingAlreadyExists error"),
        }
    }

    #[tokio::test]
    async fn test_remove_mapping() {
        let store = InMemoryOAuthUserTenantMappingStore::new();
        let mapping1 = OAuthUserTenantMapping::new("user_sub_789".to_string(), "tenant_jkl".to_string());
        store.add_mapping(mapping1.clone()).await.unwrap();

        let removed = store.remove_mapping_by_sub("user_sub_789").await.unwrap();
        assert!(removed);

        let retrieved = store.get_mapping_by_sub("user_sub_789").await.unwrap();
        assert!(retrieved.is_none());

        let removed_again = store.remove_mapping_by_sub("user_sub_789").await.unwrap();
        assert!(!removed_again);
    }

    #[tokio::test]
    async fn test_list_mappings() {
        let store = InMemoryOAuthUserTenantMappingStore::new();
        let mapping1 = OAuthUserTenantMapping::new("user_sub_aaa".to_string(), "tenant_xxx".to_string());
        let mapping2 = OAuthUserTenantMapping::new("user_sub_bbb".to_string(), "tenant_yyy".to_string());

        store.add_mapping(mapping1.clone()).await.unwrap();
        store.add_mapping(mapping2.clone()).await.unwrap();

        let list = store.list_mappings().await.unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&mapping1));
        assert!(list.contains(&mapping2));

        // Test empty list
        let empty_store = InMemoryOAuthUserTenantMappingStore::new();
        let empty_list = empty_store.list_mappings().await.unwrap();
        assert!(empty_list.is_empty());
    }
} 