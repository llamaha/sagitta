use axum::{
    extract::{State, Json, Path},
    http::StatusCode,
    response::IntoResponse,
    routing::{post, get, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

use crate::http_transport::AppState;
use crate::oauth_user_mapping::{OAuthUserTenantMapping, OAuthUserTenantMappingStore, MappingStoreError};
use crate::middleware::auth_middleware::AuthenticatedUser; // For admin checks later
use axum::Extension;

// --- Request Payloads ---
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateOAuthMappingRequest {
    pub oauth_user_sub: String,
    pub tenant_id: String,
}

// --- Response Payloads ---
// For GET one and POST, we can return the OAuthUserTenantMapping directly.
// For GET list, a wrapper struct.
#[derive(Debug, Serialize, Deserialize)]
pub struct ListOAuthMappingsResponse {
    pub mappings: Vec<OAuthUserTenantMapping>,
}

// --- Handlers ---

async fn create_oauth_mapping_handler(
    State(app_state): State<AppState>,
    // TODO: Add admin auth check using Extension<AuthenticatedUser> and checking scopes/roles
    Json(payload): Json<CreateOAuthMappingRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Attempting to create OAuth user-tenant mapping: {:?}", payload);

    if payload.oauth_user_sub.trim().is_empty() || payload.tenant_id.trim().is_empty() {
        warn!("Create mapping failed: oauth_user_sub or tenant_id is empty");
        return Err((
            StatusCode::BAD_REQUEST,
            "oauth_user_sub and tenant_id cannot be empty".to_string(),
        ));
    }

    let mapping = OAuthUserTenantMapping::new(payload.oauth_user_sub, payload.tenant_id);

    match app_state.oauth_user_mapping_store.add_mapping(mapping.clone()).await {
        Ok(_) => {
            info!("OAuth user-tenant mapping created successfully: sub={}", mapping.oauth_user_sub);
            Ok((StatusCode::CREATED, Json(mapping)))
        }
        Err(MappingStoreError::MappingAlreadyExists(sub)) => {
            warn!("Create mapping failed: Already exists for sub={}", sub);
            Err((StatusCode::CONFLICT, format!("Mapping already exists for user sub: {}", sub)))
        }
        Err(e) => {
            error!("Create mapping failed: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create mapping: {}", e)))
        }
    }
}

async fn get_oauth_mapping_handler(
    State(app_state): State<AppState>,
    // TODO: Add admin auth check
    Path(oauth_user_sub): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Attempting to get OAuth mapping for sub: {}", oauth_user_sub);

    match app_state.oauth_user_mapping_store.get_mapping_by_sub(&oauth_user_sub).await {
        Ok(Some(mapping)) => {
            Ok((StatusCode::OK, Json(mapping)))
        }
        Ok(None) => {
            Err((StatusCode::NOT_FOUND, format!("Mapping not found for user sub: {}", oauth_user_sub)))
        }
        Err(e) => {
            error!("Get mapping failed for sub={}: {}", oauth_user_sub, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get mapping: {}", e)))
        }
    }
}

async fn list_oauth_mappings_handler(
    State(app_state): State<AppState>,
    // TODO: Add admin auth check
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Attempting to list all OAuth user-tenant mappings");

    match app_state.oauth_user_mapping_store.list_mappings().await {
        Ok(mappings) => {
            Ok((StatusCode::OK, Json(ListOAuthMappingsResponse { mappings })))
        }
        Err(e) => {
            error!("List mappings failed: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to list mappings: {}", e)))
        }
    }
}

async fn delete_oauth_mapping_handler(
    State(app_state): State<AppState>,
    // TODO: Add admin auth check
    Path(oauth_user_sub): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> { // Returns raw StatusCode or error tuple
    info!("Attempting to delete OAuth mapping for sub: {}", oauth_user_sub);

    match app_state.oauth_user_mapping_store.remove_mapping_by_sub(&oauth_user_sub).await {
        Ok(true) => {
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(false) => {
            Err((StatusCode::NOT_FOUND, format!("Mapping not found for user sub: {}", oauth_user_sub)))
        }
        Err(e) => {
            error!("Delete mapping failed for sub={}: {}", oauth_user_sub, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to delete mapping: {}", e)))
        }
    }
}

// --- Router --- 
pub fn oauth_mapping_admin_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_oauth_mapping_handler).get(list_oauth_mappings_handler))
        .route("/:oauth_user_sub", get(get_oauth_mapping_handler).delete(delete_oauth_mapping_handler))
}

// --- Tests --- (Basic handler tests; store logic is tested in its own module)
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, Method, header::CONTENT_TYPE};
    use tower::ServiceExt; // for `oneshot`
    use std::sync::Arc;
    use sagitta_search::config::AppConfig;
    use qdrant_client::Qdrant;
    use crate::auth::{AuthClient, AuthClientOperations}; // Ensure AuthClientOperations is imported
    use crate::api_key::InMemoryApiKeyStore;
    use crate::tenant::InMemoryTenantStore;
    use crate::oauth_user_mapping::InMemoryOAuthUserTenantMappingStore;
    use crate::middleware::rate_limit_middleware::TenantKey;
    use axum_limit::LimitState;
    use crate::server::Server;

    // setup_test_app now returns the store as well for direct manipulation in tests
    async fn setup_test_app() -> (Router, Arc<dyn OAuthUserTenantMappingStore + Send + Sync>) {
        let app_config = AppConfig::default();
        let qdrant_client_for_server = Qdrant::from_url(&app_config.qdrant_url).build().unwrap();
        let config_arc = Arc::new(tokio::sync::RwLock::new(app_config.clone())); // Clone app_config for RwLock
        let server_for_test = Server::new_for_test(config_arc.clone(), Arc::new(qdrant_client_for_server));

        let concrete_auth_client = AuthClient::new(app_config.oauth.clone()).unwrap(); // Use cloned oauth config
        let auth_client_trait_obj: Arc<dyn AuthClientOperations + Send + Sync> = Arc::new(concrete_auth_client);
        
        let oauth_mapping_store_arc = Arc::new(InMemoryOAuthUserTenantMappingStore::default());

        let app_state = AppState {
            server: Arc::new(server_for_test),
            active_connections: Arc::new(dashmap::DashMap::new()),
            auth_client: auth_client_trait_obj,
            api_key_store: Arc::new(InMemoryApiKeyStore::default()),
            tenant_store: Arc::new(InMemoryTenantStore::default()),
            oauth_user_mapping_store: oauth_mapping_store_arc.clone(), // Clone Arc for AppState
            rate_limit_state: Arc::new(LimitState::<TenantKey>::default()),
        };

        let router = Router::new().nest("/api/v1/admin/oauth-mappings", oauth_mapping_admin_routes()).with_state(app_state);
        (router, oauth_mapping_store_arc) // Return the store Arc
    }

    #[tokio::test]
    async fn test_create_and_get_mapping_handler() {
        let (app, _store) = setup_test_app().await;

        let create_payload = CreateOAuthMappingRequest {
            oauth_user_sub: "test_sub_001".to_string(),
            tenant_id: "tenant_001".to_string(),
        };

        let response = app.clone().oneshot(Request::builder()
            .method(Method::POST)
            .uri("/api/v1/admin/oauth-mappings")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&create_payload).unwrap()))
            .unwrap())
        .await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let created_mapping: OAuthUserTenantMapping = serde_json::from_slice(&body).unwrap();
        assert_eq!(created_mapping.oauth_user_sub, "test_sub_001");
        assert_eq!(created_mapping.tenant_id, "tenant_001");

        // Test GET
        let response_get = app.clone().oneshot(Request::builder()
            .method(Method::GET)
            .uri("/api/v1/admin/oauth-mappings/test_sub_001")
            .body(Body::empty())
            .unwrap())
        .await.unwrap();
        assert_eq!(response_get.status(), StatusCode::OK);
        let body_get = axum::body::to_bytes(response_get.into_body(), usize::MAX).await.unwrap();
        let fetched_mapping: OAuthUserTenantMapping = serde_json::from_slice(&body_get).unwrap();
        assert_eq!(fetched_mapping.oauth_user_sub, "test_sub_001");
    }

    #[tokio::test]
    async fn test_list_and_delete_mapping_handler() {
        let (app, store) = setup_test_app().await;
        
        store.add_mapping(OAuthUserTenantMapping::new("sub_list_1".to_string(), "tenant_list_a".to_string())).await.unwrap();
        store.add_mapping(OAuthUserTenantMapping::new("sub_list_2".to_string(), "tenant_list_b".to_string())).await.unwrap();

        // Test LIST
        let response_list = app.clone().oneshot(Request::builder()
            .method(Method::GET)
            .uri("/api/v1/admin/oauth-mappings")
            .body(Body::empty())
            .unwrap())
        .await.unwrap();
        assert_eq!(response_list.status(), StatusCode::OK);
        let body_list = axum::body::to_bytes(response_list.into_body(), usize::MAX).await.unwrap();
        let list_response: ListOAuthMappingsResponse = serde_json::from_slice(&body_list).unwrap();
        assert_eq!(list_response.mappings.len(), 2);

        // Test DELETE
        let response_delete = app.clone().oneshot(Request::builder()
            .method(Method::DELETE)
            .uri("/api/v1/admin/oauth-mappings/sub_list_1")
            .body(Body::empty())
            .unwrap())
        .await.unwrap();
        assert_eq!(response_delete.status(), StatusCode::NO_CONTENT);

        // Verify deleted
        let get_deleted = store.get_mapping_by_sub("sub_list_1").await.unwrap();
        assert!(get_deleted.is_none());

        // List again
        let response_list_after_delete = app.clone().oneshot(Request::builder()
            .method(Method::GET)
            .uri("/api/v1/admin/oauth-mappings")
            .body(Body::empty())
            .unwrap())
        .await.unwrap();
        assert_eq!(response_list_after_delete.status(), StatusCode::OK);
        let body_list_after_delete = axum::body::to_bytes(response_list_after_delete.into_body(), usize::MAX).await.unwrap();
        let list_response_after_delete: ListOAuthMappingsResponse = serde_json::from_slice(&body_list_after_delete).unwrap();
        assert_eq!(list_response_after_delete.mappings.len(), 1);
    }
} 