use axum::{
    routing::{get, post},
    Router,
    extract::{State, Query},
    response::{sse::Sse, sse::Event, IntoResponse},
    http::{StatusCode, HeaderMap},
    Json,
};
use dashmap::DashMap;
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, broadcast};
use tracing::{error, info, warn, instrument};
use uuid::Uuid;
use serde_json::json;
use serde::Deserialize;
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use async_stream;
use std::sync::RwLock;
use crate::tenant::InMemoryTenantStore;
use crate::api_key::InMemoryApiKeyStore;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use sagitta_search::config::AppConfig;
use tower_http::cors::{CorsLayer, Any as CorsAny};
use axum::http::Method as HttpMethod;

use crate::server::Server;
use qdrant_client::Qdrant;
use crate::mcp::types::{Request as McpRequest};
use crate::mcp::error_codes;
use crate::auth::{AuthClient, AuthClientOperations, UserInfo as AuthUserInfo};
use crate::api_key::ApiKeyStore;
use crate::handlers::api_key_handler::{create_api_key_handler, list_api_keys_handler, delete_api_key_handler};
use crate::middleware::auth_middleware::{auth_layer, AuthenticatedUser};
use axum::middleware;
use crate::tenant::TenantStore;
use crate::handlers::tenant_handler::{handle_create_tenant, handle_list_tenants, handle_get_tenant, handle_update_tenant, handle_delete_tenant};
use crate::middleware::rate_limit_middleware::TenantKey;
use axum_limit::LimitState;
use axum::extract::FromRef;
use crate::middleware::secure_headers_middleware;
use axum_server::tls_rustls::RustlsConfig;
use std::net::SocketAddr;
use anyhow::Context;
use crate::oauth_user_mapping::{OAuthUserTenantMappingStore, InMemoryOAuthUserTenantMappingStore};
use crate::handlers::oauth_mapping_handler;

// Shared state for the Axum application
#[derive(Clone)]
pub struct AppState {
    pub server: Arc<Server<Qdrant>>,
    pub active_connections: Arc<DashMap<Uuid, mpsc::Sender<String>>>,
    pub auth_client: Arc<dyn AuthClientOperations + Send + Sync>,
    pub api_key_store: Arc<dyn ApiKeyStore>,
    pub tenant_store: Arc<dyn TenantStore>,
    pub oauth_user_mapping_store: Arc<dyn OAuthUserTenantMappingStore + Send + Sync>,
    pub rate_limit_state: Arc<LimitState<TenantKey>>,
}

// Implement FromRef so Axum knows how to get LimitState<TenantKey> from AppState
// The axum_limit extractors will look for LimitState<K> directly.
impl FromRef<AppState> for LimitState<TenantKey> {
    fn from_ref(app_state: &AppState) -> Self {
        // LimitState should be Clone as it likely holds an Arc to the internal DashMap.
        (*app_state.rate_limit_state).clone()
    }
}

// No longer generic over C
pub async fn run_http_server(
    addr_str: String,
    mcp_server_concrete: Server<Qdrant>,
) -> anyhow::Result<()> {
    let config = mcp_server_concrete.get_config().await?;
    let concrete_auth_client = AuthClient::new(config.oauth.clone())?;
    let auth_client: Arc<dyn AuthClientOperations + Send + Sync> = Arc::new(concrete_auth_client);

    // Use the concrete type for bootstrapping, then cast to trait object
    let api_key_store_concrete = Arc::new(InMemoryApiKeyStore::default());

    // Bootstrap admin API key from env for test/dev
    if let Ok(bootstrap_admin_key) = std::env::var("SAGITTA_BOOTSTRAP_ADMIN_KEY") {
        println!("SAGITTA_BOOTSTRAP_ADMIN_KEY={:?}", std::env::var("SAGITTA_BOOTSTRAP_ADMIN_KEY"));
        if api_key_store_concrete.get_key_by_value(&bootstrap_admin_key).await.is_none() {
            let _ = api_key_store_concrete.insert_key_with_value(
                bootstrap_admin_key.clone(),
                "admin_tenant".to_string(),
                Some("admin_user".to_string()),
                Some("Bootstrap Admin Key".to_string()),
                vec!["manage:tenants".to_string()],
                None
            ).await;
            tracing::info!("Admin key inserted: {}", bootstrap_admin_key);
        } else {
            tracing::info!("Admin key already present: {}", bootstrap_admin_key);
        }
    } else {
        println!("SAGITTA_BOOTSTRAP_ADMIN_KEY not set");
    }

    // Now cast to trait object for AppState
    let api_key_store: Arc<dyn ApiKeyStore> = api_key_store_concrete.clone();

    let tenant_store: Arc<dyn TenantStore> = Arc::new(InMemoryTenantStore::default());
    let oauth_user_mapping_store: Arc<dyn OAuthUserTenantMappingStore + Send + Sync> = Arc::new(InMemoryOAuthUserTenantMappingStore::default());
    let rate_limit_state = Arc::new(LimitState::<TenantKey>::default());

    let app_state = AppState {
        server: Arc::new(mcp_server_concrete),
        active_connections: Arc::new(DashMap::new()),
        auth_client,
        api_key_store,
        tenant_store,
        oauth_user_mapping_store,
        rate_limit_state,
    };

    // TEMPORARY DEBUGGING ROUTE
    // let temp_debug_router = Router::new()
    //     .route("/api/v1/tenants/", post(handle_create_tenant))
    //     .route_layer(axum::middleware::from_fn_with_state(app_state.clone(), auth_layer));
    // END TEMPORARY DEBUGGING ROUTE

    // --- CORS Layer Setup ---
    let cors_layer = if let Some(allowed_origins) = &config.cors_allowed_origins {
        let origins: Vec<axum::http::HeaderValue> = allowed_origins.iter()
            .map(|origin| origin.parse().expect("Invalid CORS origin in config"))
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(vec![
                HttpMethod::GET, 
                HttpMethod::POST, 
                HttpMethod::PUT, 
                HttpMethod::DELETE, 
                HttpMethod::OPTIONS,
                HttpMethod::HEAD, // HEAD is often implicitly allowed with GET by some frameworks
                HttpMethod::PATCH,
            ])
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
                axum::http::header::ACCEPT,
                axum::http::header::HeaderName::from_static("x-api-key"),
            ])
            .allow_credentials(config.cors_allow_credentials)
    } else {
        // Default: restrictive or no CORS headers. 
        // Or use CorsLayer::very_permissive() for local dev if no origins configured.
        // For now, let's make it permissive if not configured, common for local dev.
        // In production, cors_allowed_origins should be explicitly set.
        CorsLayer::new()
            .allow_origin(CorsAny) // WARNING: Permissive default, ensure this is reviewed for production
            .allow_methods([
                HttpMethod::GET,
                HttpMethod::POST,
                HttpMethod::PUT,
                HttpMethod::DELETE,
                HttpMethod::OPTIONS,
                HttpMethod::HEAD,
                HttpMethod::PATCH,
            ])
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
                axum::http::header::ACCEPT,
                axum::http::header::HeaderName::from_static("x-api-key"),
            ])
            .allow_credentials(config.cors_allow_credentials) 
    };

    // Extract host and port from the addr string
    let server_url = format!("http://{}", addr_str);
    info!(server_url = %server_url, "Server URL for SSE endpoint");

    // Define OAuth routes (user-facing, typically less restricted or handled by IdP redirects)
    let oauth_routes = Router::new()
        .route("/auth/login", get(login_handler))
        .route("/auth/callback", get(callback_handler))
        .route("/auth/userinfo", get(userinfo_handler));
        // Note: auth_layer is applied to /api/v1 which might cover some of these if they were nested differently,
        // or they might have their own specific auth/session handling after code exchange.

    // Define API Key Management Routes with explicit prefix
    let api_key_routes_direct = Router::new()
        .route("/keys/", post(create_api_key_handler).get(list_api_keys_handler))
        .route("/keys/:key_id", axum::routing::delete(delete_api_key_handler));

    // Define Tenant Management API Routes for /api/v1/tenants
    let tenant_routes_direct = Router::new()
        .route("/tenants/", post(handle_create_tenant).get(handle_list_tenants)) // Explicit trailing slash for collection
        .route("/tenants/:id", get(handle_get_tenant).put(handle_update_tenant).delete(handle_delete_tenant));

    // Define OAuth User-Tenant Mapping Admin Routes for /api/v1/admin/oauth-mappings
    let api_v1_admin_oauth_mapping_routes = oauth_mapping_handler::oauth_mapping_admin_routes();

    // --- MCP JSON-RPC over HTTP (if still needed alongside RESTful APIs) ---
    let mcp_route = Router::new().route("/mcp", post(mcp_json_rpc_handler));

    let api_v1_router = Router::new()
        .merge(api_key_routes_direct) // Use merge with the new direct-path router
        .merge(tenant_routes_direct) // Tenant routes are already using this pattern
        .nest("/admin/oauth-mappings", api_v1_admin_oauth_mapping_routes)
        .route_layer(axum::middleware::from_fn_with_state(app_state.clone(), auth_layer));

    let app = Router::new()
        .route("/sse", get(sse_handler))
        .route("/message", post(message_handler))
        .merge(oauth_routes) // User-facing OAuth routes, typically not under /api/v1 or same auth as resource APIs
        // .merge(temp_debug_router) // Merge the temporary debug route
        .nest("/api/v1", api_v1_router) // All /api/v1 routes are authenticated
        .merge(mcp_route) // MCP might have its own auth considerations or use the same /api/v1 layer if nested
        .route("/health", get(health_check_handler))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(axum::middleware::from_fn(secure_headers_middleware::secure_headers_middleware))
                .layer(cors_layer)
        )
        .with_state(app_state.clone())
        // Add catch-all route for unmatched requests
        .fallback(|req: axum::http::Request<axum::body::Body>| async move {
            println!("CATCH-ALL: Unmatched request: {} {}", req.method(), req.uri().path());
            (StatusCode::NOT_FOUND, format!("Not found: {} {}", req.method(), req.uri().path()))
        });

    let bind_addr: SocketAddr = addr_str.parse().context(format!("Invalid bind address: {}", addr_str))?;
    info!(address = %bind_addr, "Preparing to start HTTP server");

    if config.tls_enable {
        if let (Some(cert_path), Some(key_path)) = (&config.tls_cert_path, &config.tls_key_path) {
            info!("TLS enabled. Attempting to load cert from '{}' and key from '{}'", cert_path, key_path);
            let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
                .await
                .context(format!("Failed to load TLS certificate/key from paths: cert='{}', key='{}'", cert_path, key_path))?;
            
            info!(address = %bind_addr, "Starting HTTPS server with TLS");
            axum_server::bind_rustls(bind_addr, tls_config)
                .serve(app.into_make_service())
                .await
                .context("HTTPS server error")?;
        } else {
            error!("TLS is enabled in config, but cert_path or key_path is missing. Falling back to HTTP.");
            info!(address = %bind_addr, "Starting HTTP server (TLS configuration incomplete)");
            axum_server::bind(bind_addr)
                .serve(app.into_make_service())
                .await
                .context("HTTP server error (fallback due to TLS config)")?;
        }
    } else {
        info!(address = %bind_addr, "Starting HTTP server (TLS disabled)");
        axum_server::bind(bind_addr)
            .serve(app.into_make_service())
            .await
            .context("HTTP server error")?;
    }

    Ok(())
}

const SESSION_ID_HEADER: &str = "X-Session-ID";

// RAII guard to ensure connection cleanup from DashMap
struct ConnectionGuard {
    session_id: Uuid,
    active_connections: Arc<DashMap<Uuid, mpsc::Sender<String>>>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        info!(session_id = %self.session_id, "ConnectionGuard: Removing session from active list.");
        self.active_connections.remove(&self.session_id);
    }
}

#[axum::debug_handler]
#[instrument(skip(app_state, headers), fields(client_addr = ?headers.get(axum::http::header::FORWARDED).and_then(|h| h.to_str().ok())))]
async fn sse_handler(
    State(app_state): State<AppState>, 
    headers: HeaderMap, 
) -> Sse<Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>> {
    let session_id = Uuid::new_v4(); 
    info!(%session_id, "New SSE connection (/sse), establishing session.");
    info!(%session_id, headers = ?headers, "SSE connection headers");

    // Get the host from the request headers
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost:8080");
    
    let server_url = format!("http://{}", host);
    info!(%session_id, server_url = %server_url, "Using server URL from request headers");

    let (client_request_tx, mut client_request_rx) = mpsc::channel::<String>(10);
    let (sse_event_tx, _) = broadcast::channel::<String>(100);

    app_state.active_connections.insert(session_id, client_request_tx.clone());
    info!(%session_id, "Inserted session into active_connections");

    let server_arc_clone = Arc::clone(&app_state.server);
    let sse_event_tx_clone_for_task = sse_event_tx.clone();
    let active_connections_clone_for_task = Arc::clone(&app_state.active_connections);
    
    tokio::spawn(async move {
        let _scope_guard = ConnectionGuard { 
            session_id, 
            active_connections: active_connections_clone_for_task,
        };
        loop {
            tokio::select! {
                biased;
                Some(request_str) = client_request_rx.recv() => {
                    info!(%session_id, request = %request_str, "Processing request for session");
                    if let Some(response_str) = server_arc_clone.process_json_rpc_request_str(&request_str).await {
                        info!(%session_id, response = %response_str, "Sending response to SSE broadcast");
                        if let Err(e) = sse_event_tx_clone_for_task.send(response_str) {
                            error!(%session_id, error = %e, "Failed to send to SSE broadcast for session. Client likely disconnected.");
                            break;
                        }
                    } else {
                        warn!(%session_id, "No response generated for request");
                    }
                }
                else => {
                    info!(%session_id, "Request channel closed for session or task aborted. Terminating.");
                    break; 
                }
            }
        }
        info!(%session_id, "Exiting session processing task.");
    });

    let mut sse_broadcast_rx = sse_event_tx.subscribe();
    let response_stream = async_stream::stream! {
        info!(%session_id, "SSE response stream initiated for session on /sse.");

        // 1. Send the 'endpoint' event telling the client where to POST for this session.
        let endpoint_data = format!("{}/message?sessionId={}", server_url, session_id);
        info!(%session_id, endpoint_url = %endpoint_data, "Sending 'endpoint' event");
        yield Ok(Event::default().event("endpoint").data(endpoint_data));
        
        // 2. Now listen on the broadcast channel for responses to requests made via POST.
        info!(%session_id, "Now listening to broadcast channel for messages for session.");
        loop {
            match sse_broadcast_rx.recv().await {
                Ok(response_json_str) => {
                    info!(%session_id, response = %response_json_str, "Sending message from broadcast over SSE for session");
                    yield Ok(Event::default().event("message").data(response_json_str));
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(%session_id, count = n, "SSE stream lagged for session. Some messages missed.");
                    yield Ok(Event::default().event("error").data(format!("SSE stream lagged, {} messages missed", n)));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!(%session_id, "SSE broadcast channel closed for session. Ending SSE stream.");
                    break;
                }
            }
        }
        info!(%session_id, "SSE response stream ended for session.");
    };

    Sse::new(Box::pin(response_stream) as Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(10))
                .text("heartbeat")
        )
}

#[derive(Deserialize, Debug)]
struct MessageParams {
    #[serde(rename = "sessionId")]
    session_id: Uuid,
}

#[axum::debug_handler]
#[instrument(skip(app_state, headers, body_bytes), fields(query_params = ?query_params))]
async fn message_handler(
    State(app_state): State<AppState>, 
    Query(query_params): Query<MessageParams>,
    headers: HeaderMap,
    body_bytes: axum::body::Bytes,
) -> impl IntoResponse {
    info!(headers = ?headers, "Message handler headers");
    
    let body = match String::from_utf8(body_bytes.to_vec()) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "Failed to decode request body as UTF-8");
            let err_obj = json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {"code": -32700, "message": "Parse error: Invalid UTF-8 in request body"}
            });
            return (StatusCode::BAD_REQUEST, Json(err_obj)).into_response();
        }
    };
    
    let session_id = query_params.session_id;
    info!(%session_id, "Processing message for session");

    let rpc_id_for_ack: Option<serde_json::Value>;
    let rpc_method_for_ack: String;

    match serde_json::from_str::<McpRequest>(&body) {
        Ok(parsed_request) => {
            info!(%session_id, method = %parsed_request.method, "Successfully parsed MCP request");
            rpc_id_for_ack = parsed_request.id.clone();
            rpc_method_for_ack = parsed_request.method.clone();
        }
        Err(e) => {
            warn!(error = %e, body = %body, "Failed to parse body as McpRequest");
            rpc_id_for_ack = None;
            rpc_method_for_ack = "unknown_method".to_string();

            let generic_json_id: Option<serde_json::Value> = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|val: serde_json::Value| val.get("id").cloned());
            
            let err_obj = json!({
                "jsonrpc": "2.0",
                "id": generic_json_id.or_else(|| Some(serde_json::Value::Null)),
                "error": {"code": error_codes::PARSE_ERROR, "message": "Parse error: Invalid JSON request structure"}
            });
            return (StatusCode::BAD_REQUEST, Json(err_obj)).into_response();
        }
    }

    info!(%session_id, method = %rpc_method_for_ack, request_body = %body, "Message handler invoked for session via query param");

    match app_state.active_connections.get(&session_id) { 
        Some(entry) => {
            let client_tx = entry.value().clone(); 
            
            // First, send minimal HTTP ack
            let ack_response = json!({
                "jsonrpc": "2.0",
                "id": rpc_id_for_ack,
                "result": { "ack": format!("Received {}", rpc_method_for_ack) }
            });
            
            // Then send the actual request to be processed via SSE
            match client_tx.try_send(body) { 
                Ok(_) => {
                    info!(%session_id, "Successfully sent request to SSE channel");
                    (StatusCode::OK, Json(ack_response)).into_response()
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    warn!(%session_id, "Request queue full");
                    let err_obj = json!({
                        "jsonrpc": "2.0", 
                        "id": rpc_id_for_ack,
                        "error": { "code": -32000, "message": "Server busy, request queue full"}
                    });
                    (StatusCode::SERVICE_UNAVAILABLE, Json(err_obj)).into_response()
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    warn!(%session_id, "Session no longer active");
                    app_state.active_connections.remove(&session_id); 
                    let err_obj = json!({
                        "jsonrpc": "2.0", 
                        "id": rpc_id_for_ack,
                        "error": { "code": -32000, "message": "Session no longer active"}
                    });
                    (StatusCode::GONE, Json(err_obj)).into_response()
                }
            }
        }
        None => {
            warn!(%session_id, "Session ID not found");
            let err_obj = json!({
                "jsonrpc": "2.0", 
                "id": rpc_id_for_ack,
                "error": { "code": -32000, "message": "Session ID not found or expired"}
            });
            (StatusCode::NOT_FOUND, Json(err_obj)).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct LoginQuery {
    redirect_uri: Option<String>,
}

#[axum::debug_handler]
async fn login_handler(
    State(state): State<AppState>,
    Query(query): Query<LoginQuery>,
) -> impl IntoResponse {
    match state.auth_client.get_authorization_url().await {
        Ok(url) => {
            if let Some(redirect_uri) = query.redirect_uri {
                (StatusCode::FOUND, [("Location", url)]).into_response()
            } else {
                Json(url).into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(format!("Failed to generate authorization URL: {}", e)),
        ).into_response()
    }
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: String,
    state: Option<String>,
}

#[axum::debug_handler]
async fn callback_handler(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> impl IntoResponse {
    match state.auth_client.exchange_code(&query.code).await {
        Ok(token_response) => {
            // TODO: Store token in session/database
            Json(token_response).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(format!("Failed to exchange code: {}", e)),
        ).into_response()
    }
}

#[derive(Debug, Deserialize)]
struct AuthHeader {
    authorization: String,
}

#[axum::debug_handler]
async fn userinfo_handler(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.auth_client.get_user_info(auth.token()).await {
        Ok(user_info) => Json(user_info).into_response(),
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(format!("Failed to get user info: {}", e)),
        ).into_response()
    }
}

// Helper function to create the main app router for testing purposes
#[cfg(test)] // Only compile this for test builds
pub fn app_router_for_tests(app_state: AppState) -> Router {
    let api_v1_key_routes = Router::new()
        .route("/keys", post(create_api_key_handler).get(list_api_keys_handler))
        .route("/keys/:key_id", axum::routing::delete(delete_api_key_handler));

    let api_v1_tenant_routes = Router::new()
        .route("/", post(handle_create_tenant).get(handle_list_tenants))
        .route("/:id", get(handle_get_tenant).put(handle_update_tenant).delete(handle_delete_tenant));

    let api_v1_router_for_test = Router::new()
        .merge(api_v1_key_routes)
        .nest("/tenants", api_v1_tenant_routes)
        .route_layer(axum::middleware::from_fn_with_state(app_state.clone(), auth_layer));

    Router::new()
        .nest("/api/v1", api_v1_router_for_test)
        .with_state(app_state)
}

// Stub for mcp_json_rpc_handler
async fn mcp_json_rpc_handler(State(app_state): State<AppState>, Json(request): Json<serde_json::Value>) -> impl IntoResponse {
    info!("MCP JSON-RPC request: {:?}", request);
    // In a real scenario, this would call app_state.server.process_json_rpc_request_str or similar
    (StatusCode::NOT_IMPLEMENTED, "MCP JSON-RPC handler not fully implemented in HTTP transport").into_response()
}

// Updated health_check_handler
#[axum::debug_handler] // Ensures handler function signature is compatible with Axum
async fn health_check_handler(State(app_state): State<AppState>) -> impl IntoResponse {
    // Potentially check Qdrant status in the future
    // let qdrant_ok = app_state.qdrant_client.health_check().await.is_ok();
    
    let server_name = "sagitta-mcp";
    // Ideally, get version from Cargo.toml or build script
    let version = env!("CARGO_PKG_VERSION"); 

    (StatusCode::OK, Json(json!({ 
        "status": "ok",
        "server_name": server_name,
        "version": version,
        // "qdrant_status": if qdrant_ok { "ok" } else { "error" } 
    }))).into_response()
} 
