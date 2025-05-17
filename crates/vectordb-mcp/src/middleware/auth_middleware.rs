// crates/vectordb-mcp/src/middleware/auth_middleware.rs

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use tracing::{info, warn};

use crate::http_transport::AppState;
// axum_extra imports removed earlier due to being unused, re-evaluate if Bearer token parsing needs them directly.

pub const API_KEY_HEADER: &str = "X-API-Key";

// New constant for unmapped OAuth users
const UNMAPPED_OAUTH_TENANT_ID: &str = "__unmapped_oauth_tenant__";

// This struct will be populated and added to request extensions if auth is successful.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AuthenticatedUser {
    pub user_id: Option<String>, // For OAuth, this is typically the 'sub' claim
    pub tenant_id: String,       // For API Keys, from ApiKey.tenant_id; For OAuth, from mapping or placeholder
    pub scopes: Vec<String>,
}

pub async fn auth_layer(
    State(app_state): State<AppState>,
    mut req: Request, 
    next: Next,
) -> Result<Response, StatusCode> {
    info!("Auth middleware: processing request to: {:?}", req.uri());

    let auth_header = req.headers().get(header::AUTHORIZATION).and_then(|value| value.to_str().ok());
    let api_key_header = req.headers().get(API_KEY_HEADER).and_then(|value| value.to_str().ok());

    let mut authenticated_as: Option<AuthenticatedUser> = None;

    if let Some(auth_val) = auth_header {
        if auth_val.starts_with("Bearer ") {
            let token = auth_val.trim_start_matches("Bearer ");
            info!("Auth middleware: Found Bearer token");
            match app_state.auth_client.validate_token(token).await {
                Ok(true) => {
                    info!("Auth middleware: Bearer token validated successfully (via introspection).");
                    match app_state.auth_client.get_user_info(token).await {
                        Ok(user_info) => {
                            info!("Auth middleware: User info fetched: sub={}", user_info.sub);
                            // Check for OAuth user to tenant mapping
                            match app_state.oauth_user_mapping_store.get_mapping_by_sub(&user_info.sub).await {
                                Ok(Some(mapping)) => {
                                    info!("Auth middleware: OAuth user {} mapped to tenant {}", user_info.sub, mapping.tenant_id);
                                    authenticated_as = Some(AuthenticatedUser {
                                        user_id: Some(user_info.sub.clone()),
                                        tenant_id: mapping.tenant_id, // Use mapped tenant_id
                                        scopes: vec![], // Scopes might come from token introspection or user_info or mapping
                                    });
                                }
                                Ok(None) => {
                                    info!("Auth middleware: OAuth user {} not mapped to any tenant. Using unmapped placeholder.", user_info.sub);
                                    authenticated_as = Some(AuthenticatedUser {
                                        user_id: Some(user_info.sub.clone()),
                                        tenant_id: UNMAPPED_OAUTH_TENANT_ID.to_string(), // Use unmapped placeholder
                                        scopes: vec![],
                                    });
                                }
                                Err(e) => {
                                    warn!("Auth middleware: Error fetching OAuth user mapping for sub {}: {}. Using unmapped placeholder.", user_info.sub, e);
                                    authenticated_as = Some(AuthenticatedUser { // Fallback on error
                                        user_id: Some(user_info.sub.clone()),
                                        tenant_id: UNMAPPED_OAUTH_TENANT_ID.to_string(),
                                        scopes: vec![],
                                    });
                                }
                            }
                            req.extensions_mut().insert(user_info); // Still insert original UserInfo for other potential uses
                        }
                        Err(e) => {
                            warn!("Auth middleware: Failed to fetch user info after token validation: {}. Auth context will use unmapped tenant.", e);
                            authenticated_as = Some(AuthenticatedUser { 
                                user_id: None, 
                                tenant_id: UNMAPPED_OAUTH_TENANT_ID.to_string(), // Use unmapped placeholder
                                scopes: vec![], 
                            });
                        }
                    }
                }
                Ok(false) => {
                    info!("Auth middleware: Bearer token invalid (inactive or does not exist).");
                    return Err(StatusCode::UNAUTHORIZED);
                }
                Err(e) => {
                    warn!("Auth middleware: Error validating Bearer token: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        } else {
            info!("Auth middleware: Authorization header found, but not Bearer type.");
        }
    } else if let Some(key_value) = api_key_header {
        info!("Auth middleware: Found API Key in {} header. Value (truncated): {:.8}...", API_KEY_HEADER, &key_value[..std::cmp::min(8, key_value.len())]);
        match app_state.api_key_store.get_key_by_value(key_value).await {
            Some(ref api_key) => {
                info!("Auth middleware: API Key found in store: id={}, tenant_id={}, user_id={:?}, scopes={:?}, valid={}", api_key.id, api_key.tenant_id, api_key.user_id, api_key.scopes, api_key.is_valid());
                if api_key.is_valid() {
                    info!("Auth middleware: API Key validated successfully: id={}", api_key.id);
                    if let Err(e) = app_state.api_key_store.record_key_usage(&api_key.id).await {
                        warn!("Auth middleware: Failed to record API key usage: {}", e);
                    }
                    authenticated_as = Some(AuthenticatedUser {
                        user_id: api_key.user_id.clone(),
                        tenant_id: api_key.tenant_id.clone(), // This is String
                        scopes: api_key.scopes.clone(),
                    });
                } else {
                    info!("Auth middleware: API Key invalid (revoked or expired): id={}", api_key.id);
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }
            None => {
                info!("Auth middleware: API Key NOT found in store. Value (truncated): {:.8}...", &key_value[..std::cmp::min(8, key_value.len())]);
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    } else {
        info!("Auth middleware: No Authorization or API Key header found.");
    }

    if let Some(auth_user) = authenticated_as {
        info!("Auth middleware: Inserting AuthenticatedUser ({:?}) into request extensions.", auth_user);
        req.extensions_mut().insert(auth_user);
        Ok(next.run(req).await)
    } else {
        info!("Auth middleware: No successful authentication method. Denying access as auth is implied by middleware presence.");
        Err(StatusCode::UNAUTHORIZED) 
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_key::{ApiKeyStore, InMemoryApiKeyStore, API_KEY_PREFIX};
    use crate::auth::{AuthClientOperations, UserInfo, TokenResponse};
    use vectordb_core::config::{AppConfig, OAuthConfig};
    use axum::body::Body;
    use axum::http::{HeaderValue, Request as AxumRequest, StatusCode};
    use axum::middleware::{self};
    use axum::response::{IntoResponse, Json};
    use axum::routing::get;
    use axum::Router;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tower::ServiceExt;
    use crate::tenant::{InMemoryTenantStore, TenantStore};
    use crate::middleware::rate_limit_middleware::TenantKey;
    use axum_limit::LimitState;
    use crate::server::Server;
    use qdrant_client::Qdrant;
    use crate::oauth_user_mapping::{OAuthUserTenantMapping, OAuthUserTenantMappingStore, InMemoryOAuthUserTenantMappingStore, MappingStoreError};
    use anyhow;
    use async_trait::async_trait;

    // Helper handler to extract AuthenticatedUser from extensions
    async fn extract_authenticated_user_handler(req: AxumRequest<Body>) -> Result<impl IntoResponse, StatusCode> {
        match req.extensions().get::<AuthenticatedUser>() {
            Some(user) => Ok(Json(user.clone())),
            None => Err(StatusCode::INTERNAL_SERVER_ERROR), 
        }
    }

    // Mock AuthClient for testing OAuth path specifically
    struct MockAuthClientImpl {
        validate_should_succeed: bool,
        user_info_to_return: Option<UserInfo>,
        user_info_error: bool,
    }

    #[async_trait]
    impl AuthClientOperations for MockAuthClientImpl {
        async fn validate_token(&self, _token: &str) -> anyhow::Result<bool> {
            Ok(self.validate_should_succeed)
        }
        async fn get_user_info(&self, _token: &str) -> anyhow::Result<UserInfo> {
            if self.user_info_error {
                Err(anyhow::anyhow!("Mock: Failed to fetch user info"))
            }
            else if let Some(user_info) = &self.user_info_to_return {
                Ok(user_info.clone())
            }
             else {
                Err(anyhow::anyhow!("Mock: UserInfo not configured for test"))
            }
        }
        async fn get_authorization_url(&self) -> anyhow::Result<String> { unimplemented!("MockAuthClientImpl: get_authorization_url not needed for these tests") }
        async fn exchange_code(&self, _code: &str) -> anyhow::Result<TokenResponse> { unimplemented!("MockAuthClientImpl: exchange_code not needed for these tests") }
    }
    
    // Default AppState for API Key tests
    fn default_app_state_for_api_key_tests(api_key_store: Arc<dyn ApiKeyStore>) -> AppState {
        let app_config = AppConfig::default();
        let qdrant_client = Qdrant::from_url(&app_config.qdrant_url).build().unwrap();
        let config_arc = Arc::new(RwLock::new(app_config));
        let server = Server::new_for_test(config_arc.clone(), Arc::new(qdrant_client));
        
        let concrete_auth_client = crate::auth::AuthClient::new(None).unwrap();
        let auth_client_trait_obj: Arc<dyn AuthClientOperations + Send + Sync> = Arc::new(concrete_auth_client);

        AppState {
            server: Arc::new(server),
            active_connections: Arc::new(dashmap::DashMap::new()),
            auth_client: auth_client_trait_obj, 
            api_key_store,
            tenant_store: Arc::new(InMemoryTenantStore::default()),
            oauth_user_mapping_store: Arc::new(InMemoryOAuthUserTenantMappingStore::default()),
            rate_limit_state: Arc::new(LimitState::<TenantKey>::default()),
        }
    }

    // Test for valid API key (already exists, slightly adapted)
    #[tokio::test]
    async fn test_auth_layer_valid_api_key() {
        let api_key_store = Arc::new(InMemoryApiKeyStore::new());
        let test_tenant_id = "tenant_xyz_123".to_string();
        let created_key = api_key_store.create_key(test_tenant_id.clone(), None, None, vec![], None).await.unwrap();
        let app_state = default_app_state_for_api_key_tests(api_key_store.clone());
        let app = Router::new().route("/test", get(extract_authenticated_user_handler))
            .layer(middleware::from_fn_with_state(app_state, auth_layer));
        let request = AxumRequest::builder().uri("/test").header(API_KEY_HEADER, &created_key.key).body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let user: AuthenticatedUser = serde_json::from_slice(&axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert_eq!(user.tenant_id, test_tenant_id);
    }
    
    // ... other API key tests (invalid, revoked, no_auth_header) would use default_app_state_for_api_key_tests ...
    // For brevity, ensure they are refactored to use default_app_state_for_api_key_tests.
    // Example for one such refactor:
    #[tokio::test]
    async fn test_auth_layer_invalid_api_key_not_found() {
        let api_key_store = Arc::new(InMemoryApiKeyStore::new());
        let app_state = default_app_state_for_api_key_tests(api_key_store);
        let app = Router::new().route("/test", get(extract_authenticated_user_handler))
            .layer(middleware::from_fn_with_state(app_state, auth_layer));
        let request = AxumRequest::builder().uri("/test").header(API_KEY_HEADER, "non_existent_key").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_layer_oauth_user_mapped() {
        let oauth_mapping_store = Arc::new(InMemoryOAuthUserTenantMappingStore::new());
        let user_sub = "oauth_sub_mapped".to_string();
        let target_tenant_id = "tenant_mapped_123".to_string();
        oauth_mapping_store.add_mapping(OAuthUserTenantMapping::new(user_sub.clone(), target_tenant_id.clone())).await.unwrap();

        let mock_auth_client_ops: Arc<dyn AuthClientOperations + Send + Sync> = Arc::new(MockAuthClientImpl {
            validate_should_succeed: true,
            user_info_to_return: Some(UserInfo { sub: user_sub.clone(), name: None, email: None, picture: None }),
            user_info_error: false,
        });
 
        let app_config = AppConfig::default();
        let qdrant_client = Qdrant::from_url(&app_config.qdrant_url).build().unwrap();
        let config_arc = Arc::new(RwLock::new(app_config));
        let server = Server::new_for_test(config_arc.clone(), Arc::new(qdrant_client));
        
        let app_state_for_oauth_test = AppState {
            server: Arc::new(server),
            active_connections: Arc::new(dashmap::DashMap::new()),
            auth_client: mock_auth_client_ops,
            api_key_store: Arc::new(InMemoryApiKeyStore::default()),
            tenant_store: Arc::new(InMemoryTenantStore::default()),
            oauth_user_mapping_store: oauth_mapping_store.clone(),
            rate_limit_state: Arc::new(LimitState::<TenantKey>::default()),
        };

        let app = Router::new().route("/test_oauth", get(extract_authenticated_user_handler))
            .layer(middleware::from_fn_with_state(app_state_for_oauth_test.clone(), auth_layer));

        let request = AxumRequest::builder().uri("/test_oauth")
            .header(header::AUTHORIZATION, "Bearer some_valid_token")
            .body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let user: AuthenticatedUser = serde_json::from_slice(&axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert_eq!(user.tenant_id, target_tenant_id);
        assert_eq!(user.user_id, Some(user_sub));
    }

    #[tokio::test]
    async fn test_auth_layer_oauth_user_unmapped() {
        let oauth_mapping_store = Arc::new(InMemoryOAuthUserTenantMappingStore::new()); // Empty store
        let user_sub = "oauth_sub_unmapped".to_string();

        let mock_auth_client_ops: Arc<dyn AuthClientOperations + Send + Sync> = Arc::new(MockAuthClientImpl {
            validate_should_succeed: true,
            user_info_to_return: Some(UserInfo { sub: user_sub.clone(), name: None, email: None, picture: None }),
            user_info_error: false,
        });

        let app_config = AppConfig::default();
        let qdrant_client = Qdrant::from_url(&app_config.qdrant_url).build().unwrap();
        let config_arc = Arc::new(RwLock::new(app_config));
        let server = Server::new_for_test(config_arc.clone(), Arc::new(qdrant_client));
        
        let app_state_for_oauth_test = AppState {
            server: Arc::new(server),
            active_connections: Arc::new(dashmap::DashMap::new()),
            auth_client: mock_auth_client_ops,
            api_key_store: Arc::new(InMemoryApiKeyStore::default()),
            tenant_store: Arc::new(InMemoryTenantStore::default()),
            oauth_user_mapping_store: oauth_mapping_store.clone(),
            rate_limit_state: Arc::new(LimitState::<TenantKey>::default()),
        };

        let app = Router::new().route("/test_oauth", get(extract_authenticated_user_handler))
            .layer(middleware::from_fn_with_state(app_state_for_oauth_test.clone(), auth_layer));

        let request = AxumRequest::builder().uri("/test_oauth")
            .header(header::AUTHORIZATION, "Bearer some_valid_token")
            .body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let user: AuthenticatedUser = serde_json::from_slice(&axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert_eq!(user.tenant_id, UNMAPPED_OAUTH_TENANT_ID);
        assert_eq!(user.user_id, Some(user_sub));
    }
} 