use axum::{
    extract::{State, Json, Query, Path, Extension},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    middleware,
};
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};

use crate::http_transport::AppState;
use crate::api_key::{ApiKey, ApiKeyStore, ApiKeyInfo, InMemoryApiKeyStore, API_KEY_PREFIX};
use crate::middleware::auth_middleware::{API_KEY_HEADER, AuthenticatedUser};
use axum_limit::LimitPerMinute;
use crate::middleware::rate_limit_middleware::TenantKey;
use dashmap::DashMap;
use std::sync::Arc;
use crate::tenant::InMemoryTenantStore;
use axum_limit::LimitState;
use crate::auth::AuthClient;
use crate::oauth_user_mapping::InMemoryOAuthUserTenantMappingStore;

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateApiKeyRequest {
    pub tenant_id: String,
    #[serde(default)]
    pub user_id: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub expires_at: Option<u64>, // Unix timestamp
}

// The response will be the full ApiKey struct, which includes the generated key value.
// This is typically only shown on creation.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateApiKeyResponse {
    // We can directly use the ApiKey struct here if its fields are suitable for the response
    // or create a custom response struct if we want to shape it differently.
    // For now, let's assume ApiKey is fine.
    #[serde(flatten)]
    pub api_key: ApiKey,
}

pub async fn create_api_key_handler(
    State(app_state): State<AppState>,
    _limit: LimitPerMinute<60, TenantKey>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> impl IntoResponse {
    info!("Handling create API key request: {:?}", payload);

    match app_state.api_key_store.create_key(
        payload.tenant_id,
        payload.user_id,
        payload.description,
        payload.scopes,
        payload.expires_at,
    ).await {
        Ok(created_api_key) => {
            info!("API Key created successfully: id={}", created_api_key.id);
            (StatusCode::CREATED, Json(CreateApiKeyResponse { api_key: created_api_key })).into_response()
        }
        Err(e) => {
            error!("Failed to create API key: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response()
        }
    }
}

// Placeholder for list and delete handlers, to be implemented next.

#[derive(Debug, Deserialize)]
pub struct ListApiKeysQuery {
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

// Response will be a list of ApiKeyInfo objects
#[derive(Debug, Serialize, Deserialize)]
pub struct ListApiKeysResponse {
    pub api_keys: Vec<crate::api_key::ApiKeyInfo>,
}

pub async fn list_api_keys_handler(
    State(app_state): State<AppState>,
    _limit: LimitPerMinute<60, TenantKey>,
    Query(query_params): Query<ListApiKeysQuery>,
    Extension(auth_user): Extension<AuthenticatedUser>,
) -> impl IntoResponse {
    info!(
        "Handling list API keys request: {:?} for authenticated user tenant: {}, scopes: {:?}",
        query_params,
        auth_user.tenant_id,
        auth_user.scopes
    );

    let target_tenant_id_to_filter_by: String;

    let is_privileged_request = auth_user.scopes.contains(&"manage:tenants".to_string()) ||
                                auth_user.scopes.contains(&"list_all:api_keys".to_string()) ||
                                auth_user.scopes.contains(&"admin".to_string());

    if let Some(requested_tenant_id) = &query_params.tenant_id {
        if requested_tenant_id != &auth_user.tenant_id {
            if !is_privileged_request {
                warn!(
                    "Forbidden attempt to list API keys for tenant '{}' by user from tenant '{}' without required scopes.",
                    requested_tenant_id,
                    auth_user.tenant_id
                );
                return (StatusCode::FORBIDDEN, Json(serde_json::json!({ "error": "You do not have permission to list API keys for the specified tenant." }))).into_response();
            }
            target_tenant_id_to_filter_by = requested_tenant_id.clone();
        } else {
            target_tenant_id_to_filter_by = requested_tenant_id.clone();
        }
    } else {
        target_tenant_id_to_filter_by = auth_user.tenant_id.clone();
        info!("No tenant_id provided in query, defaulting to authenticated user's tenant: {}", target_tenant_id_to_filter_by);
    }

    info!("Listing API keys for tenant_id: {}", target_tenant_id_to_filter_by);

    let keys_info = app_state.api_key_store.list_keys_info(
        Some(&target_tenant_id_to_filter_by),
        query_params.user_id.as_deref()
    ).await;

    (StatusCode::OK, Json(ListApiKeysResponse { api_keys: keys_info })).into_response()
}

pub async fn delete_api_key_handler(
    State(app_state): State<AppState>,
    _limit: LimitPerMinute<60, TenantKey>,
    Path(key_id): Path<String>,
) -> impl IntoResponse {
    info!("Handling delete API key request for id: {}", key_id);
    match app_state.api_key_store.revoke_key(&key_id).await {
        Ok(true) => {
            info!("API Key id={} revoked successfully", key_id);
            StatusCode::NO_CONTENT 
        }
        Ok(false) => {
            info!("API Key id={} not found for revocation", key_id);
            StatusCode::NOT_FOUND 
        }
        Err(e) => {
            error!("Failed to revoke API key id={}: {}", key_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;
    use axum::body::{Body, Bytes, to_bytes};
    use axum::http::{Request, Method};
    use crate::auth::{AuthClient, AuthClientOperations};
    use crate::server::Server;
    use sagitta_search::config::AppConfig;
    use qdrant_client::Qdrant;
    use dashmap::DashMap;
    use std::sync::Arc;
    use crate::api_key::{API_KEY_PREFIX, InMemoryApiKeyStore};
    use crate::tenant::InMemoryTenantStore;
    use crate::oauth_user_mapping::InMemoryOAuthUserTenantMappingStore;
    use crate::middleware::rate_limit_middleware::TenantKey;
    use axum_limit::LimitState;

    async fn setup_test_app_state() -> AppState {
        let config = AppConfig::default();
        let qdrant_client = Qdrant::from_url(&config.qdrant_url).build().expect("Failed to build Qdrant client for test");
        let mcp_server = Server::new_for_test(Arc::new(tokio::sync::RwLock::new(config.clone())), Arc::new(qdrant_client));
        
        let concrete_auth_client = AuthClient::new(config.oauth.clone()).unwrap();
        let auth_client_trait_obj: Arc<dyn AuthClientOperations + Send + Sync> = Arc::new(concrete_auth_client);

        AppState {
            server: Arc::new(mcp_server),
            active_connections: Arc::new(DashMap::new()),
            auth_client: auth_client_trait_obj,
            api_key_store: Arc::new(InMemoryApiKeyStore::default()),
            tenant_store: Arc::new(InMemoryTenantStore::default()),
            oauth_user_mapping_store: Arc::new(InMemoryOAuthUserTenantMappingStore::default()),
            rate_limit_state: Arc::new(LimitState::<TenantKey>::default()),
        }
    }

    fn app_router_with_all_key_routes(app_state: AppState) -> axum::Router {
        axum::Router::new()
            .route("/api/v1/keys/",
                axum::routing::post(create_api_key_handler)
                .get(list_api_keys_handler)
            )
            .route("/api/v1/keys/:key_id", axum::routing::delete(delete_api_key_handler))
            .route_layer(middleware::from_fn_with_state(app_state.clone(), crate::middleware::auth_middleware::auth_layer))
            .with_state(app_state)
    }

    #[tokio::test]
    async fn test_create_api_key_handler_success() {
        let app_state = setup_test_app_state().await;
        let app = app_router_with_all_key_routes(app_state.clone());

        // Create an admin key for making the creation request
        let admin_creator_key = app_state.api_key_store.create_key(
            "admin_tenant_for_create_test".to_string(), 
            Some("admin_creator_user".to_string()), 
            Some("Admin key for API key creation tests".to_string()), 
            vec!["manage:api_keys".to_string()], // Scope that implies permission to create keys
            None
        ).await.unwrap();

        let request_payload = CreateApiKeyRequest {
            tenant_id: "test_tenant".to_string(),
            user_id: Some("test_user".to_string()),
            description: Some("My test API key".to_string()),
            scopes: vec!["read:data".to_string(), "write:data".to_string()],
            expires_at: None,
        };
        let response = app
            .oneshot(Request::builder()
                .method(Method::POST)
                .uri("/api/v1/keys/") // Ensure trailing slash for POST to collection
                .header("content-type", "application/json")
                .header(API_KEY_HEADER, &admin_creator_key.key) // Add auth header
                .body(Body::from(serde_json::to_string(&request_payload).unwrap()))
                .unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let created_key_response: CreateApiKeyResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(created_key_response.api_key.tenant_id, request_payload.tenant_id);
        assert_eq!(created_key_response.api_key.user_id, request_payload.user_id);
        assert_eq!(created_key_response.api_key.description, request_payload.description);
        assert_eq!(created_key_response.api_key.scopes, request_payload.scopes);
        assert!(created_key_response.api_key.key.starts_with(API_KEY_PREFIX));
        let stored_key = app_state.api_key_store.get_key_by_id(&created_key_response.api_key.id).await;
        assert!(stored_key.is_some());
        assert_eq!(stored_key.unwrap().key, created_key_response.api_key.key);
    }

    #[tokio::test]
    async fn test_list_api_keys_handler() {
        let app_state = setup_test_app_state().await;

        // Create a key that can list other keys (e.g., admin scope)
        let admin_list_key = app_state.api_key_store.create_key(
            "admin_tenant_for_listing_test".to_string(), 
            Some("admin_lister_user".to_string()), 
            Some("Admin key for API key listing tests".to_string()), 
            vec!["list_all:api_keys".to_string()], // This scope should be checked by list_api_keys_handler
            None
        ).await.unwrap();

        app_state.api_key_store.create_key("t1".to_string(), Some("u1".to_string()), Some("Key 1 (t1, u1)".to_string()), vec!["read".to_string()], None).await.unwrap();
        app_state.api_key_store.create_key("t1".to_string(), Some("u2".to_string()), Some("Key 2 (t1, u2)".to_string()), vec!["read".to_string()], None).await.unwrap();
        app_state.api_key_store.create_key("t2".to_string(), Some("u1".to_string()), Some("Key 3 (t2, u1)".to_string()), vec!["admin".to_string()], None).await.unwrap();
        
        let app = app_router_with_all_key_routes(app_state.clone());
        
        let response_all = app.clone().oneshot(Request::builder()
            .uri("/api/v1/keys/") // Adjusted URI to include trailing slash as per new router structure
            .header(API_KEY_HEADER, &admin_list_key.key)
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response_all.status(), StatusCode::OK);
        let body_all = to_bytes(response_all.into_body(), usize::MAX).await.unwrap();
        let list_response_all: ListApiKeysResponse = serde_json::from_slice(&body_all).unwrap();
        // When no tenant_id is specified, handler defaults to auth_user's tenant.
        // admin_list_key is in 'admin_tenant_for_listing_test', only itself is there.
        assert_eq!(list_response_all.api_keys.len(), 1); 
        
        let response_t1 = app.clone().oneshot(Request::builder()
            .uri("/api/v1/keys/?tenant_id=t1") 
            .header(API_KEY_HEADER, &admin_list_key.key)
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response_t1.status(), StatusCode::OK);
        let body_t1 = to_bytes(response_t1.into_body(), usize::MAX).await.unwrap();
        let list_response_t1: ListApiKeysResponse = serde_json::from_slice(&body_t1).unwrap();
        assert_eq!(list_response_t1.api_keys.len(), 2);
        assert!(list_response_t1.api_keys.iter().all(|k| k.tenant_id == "t1".to_string()));
        
        let response_u1 = app.clone().oneshot(Request::builder()
            .uri("/api/v1/keys/?user_id=u1") 
            .header(API_KEY_HEADER, &admin_list_key.key)
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response_u1.status(), StatusCode::OK);
        let body_u1 = to_bytes(response_u1.into_body(), usize::MAX).await.unwrap();
        let list_response_u1: ListApiKeysResponse = serde_json::from_slice(&body_u1).unwrap();
        // This will list for admin_list_key's tenant if no tenant_id specified and using list_all:api_keys to view its own keys, or if store filters by user_id across tenants
        // If list_all:api_keys and no tenant_id means ALL keys, then this might be 2. 
        // If it defaults to admin_list_key's tenant, and user_id=u1 is not in that tenant, it would be 0.
        // Current handler logic defaults to auth_user's tenant if query.tenant_id is None.
        // So, this will list keys for 'admin_tenant_for_listing_test' that also match user_id=u1. Expect 0 if u1 isn't under admin_tenant_for_listing_test.
        // Let's adjust admin_list_key to be for tenant 't2' which has a 'u1' user to make this more predictable
        // Or, we need to be more precise about what list_all:api_keys implies when no tenant_id is given.
        // For now, let's assume the handler's logic: if no tenant_id, use auth_user.tenant_id.
        // The admin_list_key is for 'admin_tenant_for_listing_test'. Key 3 is user 'u1' but for tenant 't2'.
        // So, filtering by user_id='u1' within 'admin_tenant_for_listing_test' should yield 0.
        assert_eq!(list_response_u1.api_keys.len(), 0); // Adjusted expectation based on current handler logic
        
        let response_t1_u1 = app.clone().oneshot(Request::builder()
            .uri("/api/v1/keys/?tenant_id=t1&user_id=u1") 
            .header(API_KEY_HEADER, &admin_list_key.key)
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response_t1_u1.status(), StatusCode::OK);
        let body_t1_u1 = to_bytes(response_t1_u1.into_body(), usize::MAX).await.unwrap();
        let list_response_t1_u1: ListApiKeysResponse = serde_json::from_slice(&body_t1_u1).unwrap();
        assert_eq!(list_response_t1_u1.api_keys.len(), 1);
        let key_info = &list_response_t1_u1.api_keys[0];
        assert_eq!(key_info.tenant_id, "t1".to_string());
        assert_eq!(key_info.user_id, Some("u1".to_string()));
        assert_eq!(key_info.description, Some("Key 1 (t1, u1)".to_string()));
        assert!(key_info.key_preview.starts_with(API_KEY_PREFIX));
    }
    
    #[tokio::test]
    async fn test_delete_api_key_handler() {
        let app_state = setup_test_app_state().await;
        let app = app_router_with_all_key_routes(app_state.clone());

        // Create an admin key for making the deletion request
        let admin_deleter_key = app_state.api_key_store.create_key(
            "admin_tenant_for_delete_test".to_string(), 
            Some("admin_deleter_user".to_string()), 
            Some("Admin key for API key deletion tests".to_string()), 
            vec!["manage:api_keys".to_string()], // Scope that implies permission to delete keys
            None
        ).await.unwrap();

        let created_key = app_state.api_key_store.create_key("default_tenant_for_delete".to_string(), None, Some("To be deleted".to_string()), vec![], None).await.unwrap();
        let key_id_to_delete = created_key.id.clone();
        let key_before_delete = app_state.api_key_store.get_key_by_id(&key_id_to_delete).await.unwrap();
        assert!(!key_before_delete.revoked);
        let response_delete = app.clone()
            .oneshot(Request::builder()
                .method(Method::DELETE)
                .uri(format!("/api/v1/keys/{}", key_id_to_delete))
                .header(API_KEY_HEADER, &admin_deleter_key.key) // Add auth header
                .body(Body::empty())
                .unwrap())
            .await
            .unwrap();
        assert_eq!(response_delete.status(), StatusCode::NO_CONTENT);
        let key_after_delete = app_state.api_key_store.get_key_by_id(&key_id_to_delete).await.unwrap();
        assert!(key_after_delete.revoked);
        assert!(!key_after_delete.is_valid());
        let response_delete_nonexistent = app
            .oneshot(Request::builder()
                .method(Method::DELETE)
                .uri("/api/v1/keys/nonexistent-key-id")
                .header(API_KEY_HEADER, &admin_deleter_key.key) // Add auth header for this too
                .body(Body::empty())
                .unwrap())
            .await
            .unwrap();
        assert_eq!(response_delete_nonexistent.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_create_api_key_handler_auth() {
        let app_state = setup_test_app_state().await;
        let app = crate::http_transport::app_router_for_tests(app_state.clone()); 
        let unauthorized_payload = CreateApiKeyRequest { tenant_id: "unauth_tenant".to_string(), user_id: None, description: Some("Unauthorized attempt".to_string()), scopes: vec![], expires_at: None };    
        let response_unauth = app.clone()
            .oneshot(Request::builder()
                .method(Method::POST)
                .uri("/api/v1/keys")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&unauthorized_payload).unwrap()))
                .unwrap())
            .await
            .unwrap();
        assert_eq!(response_unauth.status(), StatusCode::UNAUTHORIZED);
        let bootstrap_key = app_state.api_key_store.create_key("admin_tenant".to_string(), None, Some("Bootstrap key for testing".to_string()), vec!["manage:api_keys".to_string()], None).await.unwrap();
        let request_payload = CreateApiKeyRequest {
            tenant_id: "test_tenant".to_string(),
            user_id: Some("test_user".to_string()),
            description: Some("My test API key".to_string()),
            scopes: vec!["read:data".to_string(), "write:data".to_string()],
            expires_at: None,
        };
        let response_auth = app.clone()
            .oneshot(Request::builder()
                .method(Method::POST)
                .uri("/api/v1/keys")
                .header("content-type", "application/json")
                .header(API_KEY_HEADER, bootstrap_key.key.clone())
                .body(Body::from(serde_json::to_string(&request_payload).unwrap()))
                .unwrap())
            .await
            .unwrap();
        assert_eq!(response_auth.status(), StatusCode::CREATED);
        let body_auth = to_bytes(response_auth.into_body(), usize::MAX).await.unwrap();
        let created_key_response: CreateApiKeyResponse = serde_json::from_slice(&body_auth).unwrap();
        assert_eq!(created_key_response.api_key.tenant_id, request_payload.tenant_id);
        assert_eq!(created_key_response.api_key.user_id, request_payload.user_id);
        assert_eq!(created_key_response.api_key.description, request_payload.description);
        assert_eq!(created_key_response.api_key.scopes, request_payload.scopes);
        assert!(created_key_response.api_key.key.starts_with(API_KEY_PREFIX));
        let stored_key = app_state.api_key_store.get_key_by_id(&created_key_response.api_key.id).await;
        assert!(stored_key.is_some());
        assert_eq!(stored_key.unwrap().key, created_key_response.api_key.key);
    }

    #[tokio::test]
    async fn test_list_api_keys_handler_auth() {
        let app_state = setup_test_app_state().await;
        let bootstrap_key = app_state.api_key_store.create_key("admin_tenant".to_string(), None, Some("Bootstrap key".to_string()), vec!["list:api_keys".to_string()], None).await.unwrap();
        let app = crate::http_transport::app_router_for_tests(app_state.clone());
        let response_unauth = app.clone().oneshot(Request::builder().uri("/api/v1/keys").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response_unauth.status(), StatusCode::UNAUTHORIZED);
        let response_auth = app.clone()
            .oneshot(Request::builder()
                .uri("/api/v1/keys")
                .header(API_KEY_HEADER, bootstrap_key.key.clone())
                .body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(response_auth.status(), StatusCode::OK);
        let body_auth = to_bytes(response_auth.into_body(), usize::MAX).await.unwrap();
        let list_response_auth: ListApiKeysResponse = serde_json::from_slice(&body_auth).unwrap();
        assert_eq!(list_response_auth.api_keys.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_api_key_handler_auth() {
        let app_state = setup_test_app_state().await;
        let bootstrap_key = app_state.api_key_store.create_key("admin_tenant".to_string(), None, Some("Bootstrap key".to_string()), vec!["delete:api_keys".to_string()], None).await.unwrap();
        let app = crate::http_transport::app_router_for_tests(app_state.clone());
        let key_to_delete = app_state.api_key_store.create_key("tenant_to_delete_with_auth".to_string(), None, Some("To be deleted".to_string()), vec![], None).await.unwrap();
        let key_id_to_delete = key_to_delete.id.clone();
        let response_unauth = app.clone()
            .oneshot(Request::builder()
                .method(Method::DELETE)
                .uri(format!("/api/v1/keys/{}", key_id_to_delete))
                .body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(response_unauth.status(), StatusCode::UNAUTHORIZED);
        let response_auth = app.clone()
            .oneshot(Request::builder()
                .method(Method::DELETE)
                .uri(format!("/api/v1/keys/{}", key_id_to_delete))
                .header(API_KEY_HEADER, bootstrap_key.key.clone())
                .body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(response_auth.status(), StatusCode::NO_CONTENT);
        let key_after_delete = app_state.api_key_store.get_key_by_id(&key_id_to_delete).await.unwrap();
        assert!(key_after_delete.revoked);
        assert!(!key_after_delete.is_valid());
    }

    #[tokio::test]
    async fn test_create_api_key_rate_limiting() {
        let app_state = setup_test_app_state().await;
        let app = crate::http_transport::app_router_for_tests(app_state.clone()); 
        let request_payload = CreateApiKeyRequest {
            tenant_id: "rate_limit_tenant".to_string(),
            description: Some("Rate limit test key".to_string()),
            user_id: None,
            scopes: vec![],
            expires_at: None,
        };
        let payload_bytes = Bytes::from(serde_json::to_string(&request_payload).unwrap());
        let admin_key = app_state.api_key_store.create_key("admin_tenant_for_rate_limit".to_string(), Some("admin".to_string()), Some("admin_key_for_rate_limit_test".to_string()), vec!["manage:api_keys".to_string()], None).await.unwrap();
        let mut success_count = 0;
        let mut rate_limited_count = 0;
        for i in 0..70 { 
            let response = app.clone().oneshot(Request::builder()
                .method(Method::POST)
                .uri("/api/v1/keys")
                .header("content-type", "application/json")
                .header(API_KEY_HEADER, admin_key.key.clone()) 
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
                info!("Rate limit test: request {}, status: {}", i, response.status());
            }
        }
        info!("Rate limit test finished. Successes: {}, Rate Limited: {}", success_count, rate_limited_count);
        assert!(success_count > 0 && success_count <= 60, "Expected some successful requests up to the limit (around 60), got {}", success_count);
        assert!(rate_limited_count > 0, "Expected some requests to be rate limited, got {}", rate_limited_count);
    }
} 