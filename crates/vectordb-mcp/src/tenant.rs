use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub status: TenantStatus,
    // Add other relevant fields like organization_id, settings, etc.
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: HashMap<String, String>, // For custom key-value pairs
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub enum TenantStatus {
    #[default]
    Active,
    Suspended,
    Disabled,
}

#[derive(Error, Debug, PartialEq)]
pub enum TenantStoreError {
    #[error("Tenant with ID '{0}' not found")]
    NotFound(String),
    #[error("Tenant with ID '{0}' already exists")]
    AlreadyExists(String),
    #[error("Tenant name '{0}' is already in use")]
    NameAlreadyExists(String),
    #[error("Internal storage error: {0}")]
    StorageError(String),
    #[error("Invalid tenant ID format: {0}")]
    InvalidId(String),
    #[error("Invalid tenant name: {0}")]
    InvalidName(String),
}

impl Tenant {
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            status: TenantStatus::Active,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        }
    }
}

// Using async_trait for trait methods that need to be async
#[async_trait::async_trait]
pub trait TenantStore: Send + Sync {
    async fn create_tenant(&self, tenant: Tenant) -> Result<Tenant, TenantStoreError>;
    async fn get_tenant(&self, id: &str) -> Result<Option<Tenant>, TenantStoreError>;
    async fn update_tenant(&self, tenant: Tenant) -> Result<Tenant, TenantStoreError>;
    async fn delete_tenant(&self, id: &str) -> Result<(), TenantStoreError>; // Or return Option<Tenant>
    async fn list_tenants(&self) -> Result<Vec<Tenant>, TenantStoreError>;
    async fn find_tenant_by_name(&self, name: &str) -> Result<Option<Tenant>, TenantStoreError>;
}

#[derive(Clone, Debug)]
pub struct InMemoryTenantStore {
    tenants: Arc<RwLock<HashMap<String, Tenant>>>,
}

impl InMemoryTenantStore {
    pub fn new() -> Self {
        Self {
            tenants: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryTenantStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TenantStore for InMemoryTenantStore {
    async fn create_tenant(&self, mut tenant: Tenant) -> Result<Tenant, TenantStoreError> {
        let mut tenants_guard = self.tenants.write().map_err(|e| TenantStoreError::StorageError(e.to_string()))?;
        if tenants_guard.contains_key(&tenant.id) {
            return Err(TenantStoreError::AlreadyExists(tenant.id.clone()));
        }
        // Check for name uniqueness
        if tenants_guard.values().any(|t| t.name == tenant.name) {
            return Err(TenantStoreError::NameAlreadyExists(tenant.name.clone()));
        }
        let now = Utc::now();
        tenant.created_at = now;
        tenant.updated_at = now;
        tenants_guard.insert(tenant.id.clone(), tenant.clone());
        Ok(tenant)
    }

    async fn get_tenant(&self, id: &str) -> Result<Option<Tenant>, TenantStoreError> {
        let tenants_guard = self.tenants.read().map_err(|e| TenantStoreError::StorageError(e.to_string()))?;
        Ok(tenants_guard.get(id).cloned())
    }
    
    async fn find_tenant_by_name(&self, name: &str) -> Result<Option<Tenant>, TenantStoreError> {
        let tenants_guard = self.tenants.read().map_err(|e| TenantStoreError::StorageError(e.to_string()))?;
        Ok(tenants_guard.values().find(|t| t.name == name).cloned())
    }

    async fn update_tenant(&self, mut tenant: Tenant) -> Result<Tenant, TenantStoreError> {
        let mut tenants_guard = self.tenants.write().map_err(|e| TenantStoreError::StorageError(e.to_string()))?;
        if !tenants_guard.contains_key(&tenant.id) {
            return Err(TenantStoreError::NotFound(tenant.id.clone()));
        }
        // Check if the new name conflicts with an existing tenant (excluding itself)
        if tenants_guard.values().any(|t| t.name == tenant.name && t.id != tenant.id) {
            return Err(TenantStoreError::NameAlreadyExists(tenant.name.clone()));
        }
        tenant.updated_at = Utc::now();
        tenants_guard.insert(tenant.id.clone(), tenant.clone());
        Ok(tenant)
    }

    async fn delete_tenant(&self, id: &str) -> Result<(), TenantStoreError> {
        let mut tenants_guard = self.tenants.write().map_err(|e| TenantStoreError::StorageError(e.to_string()))?;
        if tenants_guard.remove(id).is_none() {
            return Err(TenantStoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn list_tenants(&self) -> Result<Vec<Tenant>, TenantStoreError> {
        let tenants_guard = self.tenants.read().map_err(|e| TenantStoreError::StorageError(e.to_string()))?;
        Ok(tenants_guard.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_tenant() {
        let store = InMemoryTenantStore::new();
        let tenant_name = "Test Tenant".to_string();
        let new_tenant = Tenant::new(tenant_name.clone());
        let new_tenant_id = new_tenant.id.clone();

        let created_tenant = store.create_tenant(new_tenant).await.unwrap();
        assert_eq!(created_tenant.name, tenant_name);
        assert_eq!(created_tenant.id, new_tenant_id);
        assert_eq!(created_tenant.status, TenantStatus::Active);

        let fetched_tenant = store.get_tenant(&new_tenant_id).await.unwrap().unwrap();
        assert_eq!(fetched_tenant.id, new_tenant_id);
        assert_eq!(fetched_tenant.name, tenant_name);
    }

    #[tokio::test]
    async fn test_create_tenant_already_exists_id() {
        let store = InMemoryTenantStore::new();
        let tenant_name = "Test Tenant".to_string();
        let mut new_tenant = Tenant::new(tenant_name.clone());
        let new_tenant_id = new_tenant.id.clone();
        new_tenant.id = new_tenant_id.clone(); // Ensure ID is set

        store.create_tenant(new_tenant.clone()).await.unwrap();
        
        // Try to create again with the same ID
        let result = store.create_tenant(new_tenant).await;
        assert!(matches!(result, Err(TenantStoreError::AlreadyExists(_))));
    }

    #[tokio::test]
    async fn test_create_tenant_name_already_exists() {
        let store = InMemoryTenantStore::new();
        let tenant_name = "Unique Name".to_string();
        
        let tenant1 = Tenant::new(tenant_name.clone());
        store.create_tenant(tenant1).await.unwrap();

        let tenant2 = Tenant::new(tenant_name.clone()); // Different ID, same name
        let result = store.create_tenant(tenant2).await;
        assert!(matches!(result, Err(TenantStoreError::NameAlreadyExists(name)) if name == tenant_name));
    }

    #[tokio::test]
    async fn test_get_tenant_not_found() {
        let store = InMemoryTenantStore::new();
        let result = store.get_tenant("non_existent_id").await.unwrap();
        assert!(result.is_none());
    }
    
    #[tokio::test]
    async fn test_find_tenant_by_name() {
        let store = InMemoryTenantStore::new();
        let tenant_name = "Find Me Tenant".to_string();
        let new_tenant = Tenant::new(tenant_name.clone());
        store.create_tenant(new_tenant).await.unwrap();

        let found_tenant = store.find_tenant_by_name(&tenant_name).await.unwrap().unwrap();
        assert_eq!(found_tenant.name, tenant_name);

        let not_found_tenant = store.find_tenant_by_name("No Such Name").await.unwrap();
        assert!(not_found_tenant.is_none());
    }

    #[tokio::test]
    async fn test_update_tenant() {
        let store = InMemoryTenantStore::new();
        let tenant_name = "Original Name".to_string();
        let mut tenant = Tenant::new(tenant_name);
        let tenant_id = tenant.id.clone();

        store.create_tenant(tenant.clone()).await.unwrap();

        let updated_name = "Updated Name".to_string();
        tenant.name = updated_name.clone();
        tenant.status = TenantStatus::Suspended;
        let updated_tenant = store.update_tenant(tenant).await.unwrap();

        assert_eq!(updated_tenant.id, tenant_id);
        assert_eq!(updated_tenant.name, updated_name);
        assert_eq!(updated_tenant.status, TenantStatus::Suspended);
        assert!(updated_tenant.updated_at > updated_tenant.created_at);
    }
    
    #[tokio::test]
    async fn test_update_tenant_name_conflict() {
        let store = InMemoryTenantStore::new();
        let name1 = "NameOne".to_string();
        let name2 = "NameTwo".to_string();

        let tenant1 = Tenant::new(name1.clone());
        let tenant1_id = tenant1.id.clone();
        store.create_tenant(tenant1).await.unwrap();
        
        let tenant2 = Tenant::new(name2.clone());
        store.create_tenant(tenant2).await.unwrap();

        // Try to update tenant1 to have name2
        let mut tenant_to_update = store.get_tenant(&tenant1_id).await.unwrap().unwrap();
        tenant_to_update.name = name2.clone();
        let result = store.update_tenant(tenant_to_update).await;
        assert!(matches!(result, Err(TenantStoreError::NameAlreadyExists(name)) if name == name2));
    }

    #[tokio::test]
    async fn test_update_tenant_not_found() {
        let store = InMemoryTenantStore::new();
        let tenant = Tenant::new("NonExistent".to_string());
        let result = store.update_tenant(tenant).await;
        assert!(matches!(result, Err(TenantStoreError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_delete_tenant() {
        let store = InMemoryTenantStore::new();
        let tenant = Tenant::new("To Be Deleted".to_string());
        let tenant_id = tenant.id.clone();
        store.create_tenant(tenant).await.unwrap();

        let result = store.delete_tenant(&tenant_id).await;
        assert!(result.is_ok());

        let fetched = store.get_tenant(&tenant_id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_delete_tenant_not_found() {
        let store = InMemoryTenantStore::new();
        let result = store.delete_tenant("non_existent_id").await;
        assert!(matches!(result, Err(TenantStoreError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_list_tenants() {
        let store = InMemoryTenantStore::new();
        
        let tenant1 = Tenant::new("Tenant Alpha".to_string());
        let tenant1_id = tenant1.id.clone();
        store.create_tenant(tenant1).await.unwrap();

        let tenant2 = Tenant::new("Tenant Beta".to_string());
        let tenant2_id = tenant2.id.clone();
        store.create_tenant(tenant2).await.unwrap();

        let tenants = store.list_tenants().await.unwrap();
        assert_eq!(tenants.len(), 2);
        assert!(tenants.iter().any(|t| t.id == tenant1_id));
        assert!(tenants.iter().any(|t| t.id == tenant2_id));
    }
} 