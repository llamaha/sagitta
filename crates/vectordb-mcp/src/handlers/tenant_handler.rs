use axum::{
    extract::{State, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use crate::http_transport::AppState;
use crate::tenant::{Tenant, TenantStore, TenantStoreError, TenantStatus};
use std::collections::HashMap;
use axum_limit::LimitPerMinute;
use crate::middleware::rate_limit_middleware::TenantKey;

#[derive(Deserialize, Serialize, Debug)]
pub struct CreateTenantRequest {
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct UpdateTenantRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<TenantStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

// Using Tenant struct directly as response for simplicity for now
type TenantResponse = Tenant;

pub async fn handle_create_tenant(
    State(app_state): State<AppState>,
    _limit: LimitPerMinute<60, TenantKey>,
    Json(payload): Json<CreateTenantRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(tenant_name = %payload.name, "Attempting to create tenant");

    if payload.name.trim().is_empty() {
        warn!("Create tenant failed: Name is empty");
        return Err((
            StatusCode::BAD_REQUEST,
            "Tenant name cannot be empty".to_string(),
        ));
    }

    // Create a new Tenant object using its constructor
    let tenant_to_create = Tenant::new(payload.name.clone());
    
    match app_state.tenant_store.create_tenant(tenant_to_create).await { // Pass the Tenant object
        Ok(created_tenant) => {
            info!(tenant_id = %created_tenant.id, tenant_name = %created_tenant.name, "Tenant created successfully");
            Ok((StatusCode::CREATED, Json(created_tenant as TenantResponse)))
        }
        Err(TenantStoreError::NameAlreadyExists(name)) => {
            warn!(tenant_name = %name, "Create tenant failed: Name already exists");
            Err((
                StatusCode::CONFLICT,
                format!("Tenant name '{}' already exists", name),
            ))
        }
        Err(e) => {
            error!(error = %e, "Create tenant failed due to an internal error");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create tenant: {}", e),
            ))
        }
    }
}

pub async fn handle_list_tenants(
    State(app_state): State<AppState>,
    _limit: LimitPerMinute<60, TenantKey>,
    // TODO: Add auth check for admin/specific scope later
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Attempting to list tenants");
    match app_state.tenant_store.list_tenants().await {
        Ok(tenants) => {
            info!("Successfully listed {} tenants", tenants.len());
            Ok((StatusCode::OK, Json(tenants)))
        }
        Err(e) => {
            error!(error = %e, "List tenants failed due to an internal error");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list tenants: {}", e),
            ))
        }
    }
}

pub async fn handle_get_tenant(
    State(app_state): State<AppState>,
    _limit: LimitPerMinute<60, TenantKey>,
    Path(tenant_id): Path<String>,
    // TODO: Add auth check for admin or if user belongs to this tenant
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(%tenant_id, "Attempting to get tenant");
    match app_state.tenant_store.get_tenant(&tenant_id).await {
        Ok(Some(tenant)) => {
            info!(%tenant_id, tenant_name = %tenant.name, "Tenant retrieved successfully");
            Ok((StatusCode::OK, Json(tenant)))
        }
        Ok(None) => {
            warn!(%tenant_id, "Get tenant failed: Not found");
            Err((StatusCode::NOT_FOUND, format!("Tenant with ID '{}' not found", tenant_id)))
        }
        Err(e) => {
            error!(%tenant_id, error = %e, "Get tenant failed due to an internal error");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get tenant: {}", e),
            ))
        }
    }
}

pub async fn handle_update_tenant(
    State(app_state): State<AppState>,
    _limit: LimitPerMinute<60, TenantKey>,
    Path(tenant_id): Path<String>,
    Json(payload): Json<UpdateTenantRequest>,
    // TODO: Add auth check for admin or if user belongs to this tenant (and has perms)
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(%tenant_id, "Attempting to update tenant with payload: {:?}", payload);

    let mut tenant = match app_state.tenant_store.get_tenant(&tenant_id).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            warn!(%tenant_id, "Update tenant failed: Not found");
            return Err((StatusCode::NOT_FOUND, format!("Tenant with ID '{}' not found", tenant_id)));
        }
        Err(e) => {
            error!(%tenant_id, error = %e, "Update tenant failed: Error fetching tenant");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch tenant: {}", e)));
        }
    };

    let mut updated = false;
    if let Some(name) = payload.name {
        if !name.trim().is_empty() {
            tenant.name = name;
            updated = true;
        } else {
            return Err((StatusCode::BAD_REQUEST, "Tenant name cannot be empty".to_string()));
        }
    }
    if let Some(status) = payload.status {
        tenant.status = status;
        updated = true;
    }
    if let Some(metadata) = payload.metadata {
        // For metadata, we might want to merge or replace. Let's replace for now.
        tenant.metadata = metadata;
        updated = true;
    }

    if !updated {
        // No actual changes provided, could return 304 Not Modified or just the current tenant
        info!(%tenant_id, "No update performed as payload had no changes or only empty name.");
        return Ok((StatusCode::OK, Json(tenant))); // Return existing tenant
    }

    match app_state.tenant_store.update_tenant(tenant).await {
        Ok(updated_tenant) => {
            info!(%tenant_id, tenant_name = %updated_tenant.name, "Tenant updated successfully");
            Ok((StatusCode::OK, Json(updated_tenant)))
        }
        Err(TenantStoreError::NotFound(_)) => {
             // Should not happen if get_tenant succeeded, but handle defensively
            warn!(%tenant_id, "Update tenant failed: Not found during update operation");
            Err((StatusCode::NOT_FOUND, format!("Tenant with ID '{}' not found during update", tenant_id)))
        }
        Err(TenantStoreError::NameAlreadyExists(name)) => {
            warn!(%tenant_id, new_name = %name, "Update tenant failed: Name already exists");
            Err((StatusCode::CONFLICT, format!("Tenant name '{}' already exists", name)))
        }
        Err(e) => {
            error!(%tenant_id, error = %e, "Update tenant failed due to an internal error");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to update tenant: {}", e),
            ))
        }
    }
}

pub async fn handle_delete_tenant(
    State(app_state): State<AppState>,
    _limit: LimitPerMinute<60, TenantKey>,
    Path(tenant_id): Path<String>,
    // TODO: Add auth check for admin or if user belongs to this tenant (and has perms)
    // TODO: Consider implications: what happens to associated resources (API keys, repositories)?
    // For now, it's a simple delete from the tenant store.
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(%tenant_id, "Attempting to delete tenant");

    match app_state.tenant_store.delete_tenant(&tenant_id).await {
        Ok(_) => { // TenantStore::delete_tenant returns Result<(), TenantStoreError>
            info!(%tenant_id, "Tenant deleted successfully");
            Ok(StatusCode::NO_CONTENT)
        }
        Err(TenantStoreError::NotFound(_)) => {
            warn!(%tenant_id, "Delete tenant failed: Not found");
            Err((StatusCode::NOT_FOUND, format!("Tenant with ID '{}' not found", tenant_id)))
        }
        Err(e) => {
            error!(%tenant_id, error = %e, "Delete tenant failed due to an internal error");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to delete tenant: {}", e),
            ))
        }
    }
}

// Placeholder for other handlers to be added later:
// pub async fn handle_delete_tenant(...) { ... } 

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_transport::AppState;
    use crate::api_key::{InMemoryApiKeyStore, ApiKeyStore as _ApiKeyStoreTrait}; // Renamed trait import
    use crate::tenant::InMemoryTenantStore;
    use crate::middleware::rate_limit_middleware::TenantKey;
    use crate::middleware::auth_middleware::API_KEY_HEADER;
    use crate::auth::AuthClient;
    use crate::server::Server;
    use axum::{
        body::{Body, Bytes, to_bytes},
        http::{Request, Method, StatusCode},
        Router,
    };
    use axum_limit::LimitState;
    use dashmap::DashMap;
    use serde_json::json;
    use std::sync::Arc;
    use tower::ServiceExt; // for `oneshot`
    use vectordb_core::config::AppConfig;
    use qdrant_client::Qdrant;
    use crate::oauth_user_mapping::InMemoryOAuthUserTenantMappingStore;

    async fn setup_test_app_state_for_tenants() -> AppState {
        let config = AppConfig::default();
        let qdrant_client = Qdrant::from_url(&config.qdrant_url)
            .build()
            .expect("Failed to build Qdrant client for test");
        let mcp_server = Server::new_for_test(
            Arc::new(tokio::sync::RwLock::new(config.clone())),
            Arc::new(qdrant_client),
        );
        
        let concrete_auth_client = AuthClient::new(config.oauth.clone()).unwrap();
        let auth_client_trait_obj: Arc<dyn crate::auth::AuthClientOperations + Send + Sync> = Arc::new(concrete_auth_client);

        AppState {
            server: Arc::new(mcp_server),
            active_connections: Arc::new(DashMap::new()),
            auth_client: auth_client_trait_obj, // Use trait object
            api_key_store: Arc::new(InMemoryApiKeyStore::default()),
            tenant_store: Arc::new(InMemoryTenantStore::default()),
            oauth_user_mapping_store: Arc::new(InMemoryOAuthUserTenantMappingStore::default()), // Added
            rate_limit_state: Arc::new(LimitState::<TenantKey>::default()),
        }
    }

    // Helper to get a pre-authenticated router for tests
    async fn get_test_router_and_admin_key() -> (Router, String) {
        let app_state = setup_test_app_state_for_tenants().await;
        let admin_key = app_state.api_key_store.create_key(
            "admin_tenant".to_string(), // Provide a String tenant_id
            Some("admin_user".to_string()), 
            Some("Admin Key for Tenant Tests".to_string()), 
            vec!["manage:tenants".to_string()], // Scope needed for tenant management
            None
        ).await.unwrap().key;
        (crate::http_transport::app_router_for_tests(app_state), admin_key)
    }

    #[tokio::test]
    async fn test_create_tenant_success() {
        let (app, admin_key) = get_test_router_and_admin_key().await;
        let request_payload = CreateTenantRequest { name: "New Corp".to_string() };

        let response = app
            .oneshot(Request::builder()
                .method(Method::POST)
                .uri("/api/v1/tenants")
                .header(API_KEY_HEADER, &admin_key)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request_payload).unwrap()))
                .unwrap())
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let tenant_resp: Tenant = serde_json::from_slice(&body).unwrap();
        assert_eq!(tenant_resp.name, "New Corp");
        assert_eq!(tenant_resp.status, TenantStatus::Active);
    }

    #[tokio::test]
    async fn test_create_tenant_name_conflict() {
        let (app, admin_key) = get_test_router_and_admin_key().await;
        let tenant_name = "Conflict Corp".to_string();
        let request_payload = CreateTenantRequest { name: tenant_name.clone() };

        // Create first tenant
        app.clone().oneshot(Request::builder()
            .method(Method::POST).uri("/api/v1/tenants").header(API_KEY_HEADER, &admin_key)
            .header("content-type", "application/json").body(Body::from(serde_json::to_string(&request_payload).unwrap())).unwrap())
            .await.unwrap();

        // Attempt to create second tenant with same name
        let response = app
            .oneshot(Request::builder()
                .method(Method::POST).uri("/api/v1/tenants").header(API_KEY_HEADER, &admin_key)
                .header("content-type", "application/json").body(Body::from(serde_json::to_string(&request_payload).unwrap())).unwrap())
            .await.unwrap();
        
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
    
    #[tokio::test]
    async fn test_create_tenant_empty_name() {
        let (app, admin_key) = get_test_router_and_admin_key().await;
        let request_payload = CreateTenantRequest { name: "".to_string() };
        let response = app
            .oneshot(Request::builder()
                .method(Method::POST).uri("/api/v1/tenants").header(API_KEY_HEADER, &admin_key)
                .header("content-type", "application/json").body(Body::from(serde_json::to_string(&request_payload).unwrap())).unwrap())
            .await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_list_tenants_success() {
        let (app, admin_key) = get_test_router_and_admin_key().await;
        // Create a tenant first
        app.clone().oneshot(Request::builder()
            .method(Method::POST).uri("/api/v1/tenants").header(API_KEY_HEADER, &admin_key)
            .header("content-type", "application/json").body(Body::from(serde_json::to_string(&CreateTenantRequest{name: "ListMe Corp".to_string()}).unwrap())).unwrap())
            .await.unwrap();

        let response = app
            .oneshot(Request::builder().method(Method::GET).uri("/api/v1/tenants").header(API_KEY_HEADER, &admin_key).body(Body::empty()).unwrap())
            .await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let tenants: Vec<Tenant> = serde_json::from_slice(&body).unwrap();
        assert!(!tenants.is_empty());
        assert!(tenants.iter().any(|t| t.name == "ListMe Corp"));
    }

    #[tokio::test]
    async fn test_get_tenant_success_and_not_found() {
        let (app, admin_key) = get_test_router_and_admin_key().await;
        let create_payload = CreateTenantRequest { name: "GetMe Corp".to_string() };
        let create_response = app.clone()
            .oneshot(Request::builder().method(Method::POST).uri("/api/v1/tenants").header(API_KEY_HEADER, &admin_key).header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&create_payload).unwrap())).unwrap()).await.unwrap();
        let created_tenant: Tenant = serde_json::from_slice(&to_bytes(create_response.into_body(), usize::MAX).await.unwrap()).unwrap();

        // Test Get Success
        let response_get = app.clone()
            .oneshot(Request::builder().method(Method::GET).uri(format!("/api/v1/tenants/{}", created_tenant.id)).header(API_KEY_HEADER, &admin_key).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response_get.status(), StatusCode::OK);
        let fetched_tenant: Tenant = serde_json::from_slice(&to_bytes(response_get.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert_eq!(fetched_tenant.id, created_tenant.id);

        // Test Get Not Found
        let response_not_found = app
            .oneshot(Request::builder().method(Method::GET).uri("/api/v1/tenants/non-existent-id").header(API_KEY_HEADER, &admin_key).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response_not_found.status(), StatusCode::NOT_FOUND);
    }
    
    #[tokio::test]
    async fn test_update_tenant_success() {
        let (app, admin_key) = get_test_router_and_admin_key().await;
        let create_payload = CreateTenantRequest { name: "UpdateOldName Corp".to_string() };
        let create_response = app.clone().oneshot(Request::builder().method(Method::POST).uri("/api/v1/tenants").header(API_KEY_HEADER, &admin_key).header("content-type", "application/json").body(Body::from(serde_json::to_string(&create_payload).unwrap())).unwrap()).await.unwrap();
        let created_tenant: Tenant = serde_json::from_slice(&to_bytes(create_response.into_body(), usize::MAX).await.unwrap()).unwrap();

        let update_payload = UpdateTenantRequest { name: Some("UpdateNewName Corp".to_string()), status: Some(TenantStatus::Suspended), metadata: None };
        let response_update = app.clone().oneshot(Request::builder().method(Method::PUT).uri(format!("/api/v1/tenants/{}", created_tenant.id)).header(API_KEY_HEADER, &admin_key).header("content-type", "application/json").body(Body::from(serde_json::to_string(&update_payload).unwrap())).unwrap()).await.unwrap();
        assert_eq!(response_update.status(), StatusCode::OK);
        let updated_tenant: Tenant = serde_json::from_slice(&to_bytes(response_update.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert_eq!(updated_tenant.name, "UpdateNewName Corp");
        assert_eq!(updated_tenant.status, TenantStatus::Suspended);
    }

    #[tokio::test]
    async fn test_delete_tenant_success() {
        let (app, admin_key) = get_test_router_and_admin_key().await;
        let create_payload = CreateTenantRequest { name: "DeleteMe Corp".to_string() };
        let create_response = app.clone().oneshot(Request::builder().method(Method::POST).uri("/api/v1/tenants").header(API_KEY_HEADER, &admin_key).header("content-type", "application/json").body(Body::from(serde_json::to_string(&create_payload).unwrap())).unwrap()).await.unwrap();
        let created_tenant: Tenant = serde_json::from_slice(&to_bytes(create_response.into_body(), usize::MAX).await.unwrap()).unwrap();

        let response_delete = app.clone().oneshot(Request::builder().method(Method::DELETE).uri(format!("/api/v1/tenants/{}", created_tenant.id)).header(API_KEY_HEADER, &admin_key).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response_delete.status(), StatusCode::NO_CONTENT);

        let response_get_after_delete = app.oneshot(Request::builder().method(Method::GET).uri(format!("/api/v1/tenants/{}", created_tenant.id)).header(API_KEY_HEADER, &admin_key).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response_get_after_delete.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_tenant_api_rate_limiting() {
        let (app, admin_key) = get_test_router_and_admin_key().await;
        let request_payload = CreateTenantRequest { name: "RateLimitedTenant".to_string() };
        let payload_bytes = Bytes::from(serde_json::to_string(&request_payload).unwrap());

        let mut success_count = 0;
        let mut rate_limited_count = 0;

        for i in 0..70 { // Default LimitPerMinute<60, TenantKey>
            let response = app.clone().oneshot(Request::builder()
                .method(Method::POST)
                .uri("/api/v1/tenants")
                .header(API_KEY_HEADER, &admin_key)
                .header("content-type", "application/json")
                .body(Body::from(payload_bytes.clone()))
                .unwrap())
            .await
            .unwrap();
            
            if response.status() == StatusCode::CREATED {
                success_count += 1;
            } else if response.status() == StatusCode::TOO_MANY_REQUESTS {
                rate_limited_count += 1;
            }
             if i % 10 == 0 {
                info!("Tenant rate limit test: request {}, status: {}", i, response.status());
            }
        }
        info!("Tenant rate limit test finished. Successes: {}, Rate Limited: {}", success_count, rate_limited_count);
        assert!(success_count > 0 && success_count <= 60, "Expected successful tenant creations up to the limit (around 60), got {}", success_count);
        assert!(rate_limited_count > 0, "Expected some tenant creations to be rate limited, got {}", rate_limited_count);
    }
} 