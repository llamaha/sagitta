use axum::{
    routing::{get, post},
    Router,
    extract::{State, Query, Extension},
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
use async_stream;
use std::sync::RwLock;
use crate::api_key::InMemoryApiKeyStore;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use sagitta_search::config::AppConfig;

use crate::server::Server;
use qdrant_client::Qdrant;
use crate::mcp::types::{Request as McpRequest};
use crate::mcp::error_codes;
use crate::api_key::ApiKeyStore;
use crate::handlers::api_key_handler::{create_api_key_handler, list_api_keys_handler, delete_api_key_handler};
use crate::middleware::auth_middleware::{auth_layer, AuthenticatedUser};
use axum::middleware;
use crate::middleware::secure_headers_middleware;
use std::net::SocketAddr;
use anyhow::Context;

// Shared state for the Axum application
#[derive(Clone)]
pub struct AppState {
    pub server: Arc<Server<Qdrant>>,
    pub active_connections: Arc<DashMap<Uuid, mpsc::Sender<String>>>,
    pub api_key_store: Arc<dyn ApiKeyStore>,
}

// No longer generic over C
pub async fn run_http_server(
    addr_str: String,
    mcp_server_concrete: Server<Qdrant>,
) -> anyhow::Result<()> {
    let config = mcp_server_concrete.get_config().await?;

    // Use the concrete type for bootstrapping, then cast to trait object
    let api_key_store_concrete = Arc::new(InMemoryApiKeyStore::default());

    // Bootstrap admin API key from env for test/dev
    if let Ok(bootstrap_admin_key) = std::env::var("SAGITTA_BOOTSTRAP_ADMIN_KEY") {
        println!("SAGITTA_BOOTSTRAP_ADMIN_KEY={:?}", std::env::var("SAGITTA_BOOTSTRAP_ADMIN_KEY"));
        if api_key_store_concrete.get_key_by_value(&bootstrap_admin_key).await.is_none() {
            let _ = api_key_store_concrete.insert_key_with_value(
                bootstrap_admin_key.clone(),
                Some("admin_user".to_string()),
                Some("Bootstrap Admin Key".to_string()),
                vec!["manage:all".to_string()],
                None
            ).await;
            tracing::info!("Admin key inserted: {}", bootstrap_admin_key);
        } else {
            tracing::info!("Admin key already exists: {}", bootstrap_admin_key);
        }
    }
    
    let api_key_store: Arc<dyn ApiKeyStore> = api_key_store_concrete;

    let active_connections = Arc::new(DashMap::new());
    let app_state = AppState {
        server: Arc::new(mcp_server_concrete),
        active_connections: active_connections.clone(),
        api_key_store: api_key_store.clone(),
    };

    // Initialize streaming progress reports for this server instance
    let _ = tokio::spawn(start_progress_broadcaster(active_connections.clone()));

    // Extract host and port from the addr string
    let server_url = format!("http://{}", addr_str);
    info!(server_url = %server_url, "Server URL for SSE endpoint");

    // Define API Key Management Routes with explicit prefix
    let api_key_routes_direct = Router::new()
        .route("/keys/", post(create_api_key_handler).get(list_api_keys_handler))
        .route("/keys/:key_id", axum::routing::delete(delete_api_key_handler));

    // --- MCP JSON-RPC over HTTP (if still needed alongside RESTful APIs) ---
    let mcp_route = Router::new().route("/mcp", post(mcp_json_rpc_handler));

    let api_v1_router = Router::new()
        .merge(api_key_routes_direct) // Use merge with the new direct-path router
        .route_layer(axum::middleware::from_fn_with_state(app_state.clone(), auth_layer));

    let app = Router::new()
        .route("/sse", get(sse_handler))
        .route("/message", post(message_handler))
        .nest("/api/v1", api_v1_router) // All /api/v1 routes are authenticated
        .merge(mcp_route) // MCP might have its own auth considerations or use the same /api/v1 layer if nested
        .route("/health", get(health_check_handler))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(axum::middleware::from_fn(secure_headers_middleware::secure_headers_middleware))
        )
        .with_state(app_state.clone())
        // Add catch-all route for unmatched requests
        .fallback(|req: axum::http::Request<axum::body::Body>| async move {
            println!("CATCH-ALL: Unmatched request: {} {}", req.method(), req.uri().path());
            (StatusCode::NOT_FOUND, format!("Not found: {} {}", req.method(), req.uri().path()))
        });

    let bind_addr: SocketAddr = addr_str.parse().context(format!("Invalid bind address: {}", addr_str))?;
    info!(address = %bind_addr, "Preparing to start HTTP server");

    info!(address = %bind_addr, "Starting HTTP server");
    let listener = tokio::net::TcpListener::bind(bind_addr).await
        .context("Failed to bind to address")?;
    axum::serve(listener, app)
        .await
        .context("HTTP server error")?;

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

    // Create a channel for this connection
    let (tx, mut rx) = mpsc::channel::<String>(100);
    app_state.active_connections.insert(session_id, tx);
    
    // Create the connection guard - it will clean up when the stream ends
    let guard = ConnectionGuard {
        session_id,
        active_connections: app_state.active_connections.clone(),
    };

    // Send initial connection confirmation with session ID in both content and custom headers
    let initial_event = Event::default()
        .event("connection")
        .id(&session_id.to_string())
        .data(json!({
            "type": "connection",
            SESSION_ID_HEADER: session_id.to_string(),
            "server_url": server_url, // Include server URL in initial connection
        }).to_string());

    let stream = async_stream::stream! {
        yield Ok(initial_event);

        while let Some(msg) = rx.recv().await {
            yield Ok(Event::default().data(msg));
        }

        // When rx ends, the stream ends and ConnectionGuard will drop
        drop(guard);
        info!(%session_id, "SSE stream ended.");
    };

    let boxed_stream: Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> = Box::pin(stream);
    Sse::new(boxed_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("ping")
    )
}

// Continuously check for new progress reports and broadcast them
async fn start_progress_broadcaster(active_connections: Arc<DashMap<Uuid, mpsc::Sender<String>>>) {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    
    loop {
        interval.tick().await;
        
        // Get any pending progress messages from the server's global queue
        if let Some(messages) = crate::progress::take_pending_messages() {
            for msg in messages {
                // Broadcast to all active connections
                let connections: Vec<_> = active_connections.iter().map(|item| item.key().clone()).collect();
                for session_id in connections {
                    if let Some(tx) = active_connections.get(&session_id) {
                        let _ = tx.send(msg.clone()).await;
                    }
                }
            }
        }
    }
}

#[axum::debug_handler]
async fn message_handler(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<String, StatusCode> {
    
    let session_id = headers
        .get(SESSION_ID_HEADER)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok());

    info!(?session_id, body_len = %body.len(), "Received message");

    match session_id {
        Some(id) => {
            // Find the active connection
            if let Some(tx) = app_state.active_connections.get(&id) {
                info!(%id, "Broadcasting message to session");
                
                // Here you can process the message and send responses
                // For now, we'll echo it back
                let response = json!({
                    "type": "echo",
                    "original": body,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }).to_string();
                
                if tx.send(response).await.is_err() {
                    warn!(%id, "Failed to send message to session");
                    return Err(StatusCode::GONE);
                }
                
                Ok("Message sent".to_string())
            } else {
                warn!(%id, "Session not found");
                Err(StatusCode::NOT_FOUND)
            }
        }
        None => {
            warn!("Missing or invalid session ID header");
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

#[axum::debug_handler]
async fn mcp_json_rpc_handler(
    State(app_state): State<AppState>,
    _user: Option<Extension<AuthenticatedUser>>, // User might be None if auth is disabled or for certain endpoints
    Json(request): Json<McpRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    
    info!(method = %request.method, "Handling MCP JSON-RPC request");
    
    match app_state.server.handle_request(request).await {
        Ok(Some(response)) => Ok(Json(response)),
        Ok(None) => {
            // No response for notifications
            Ok(Json(json!(null)))
        }
        Err(e) => {
            error!(error = ?e, "Failed to handle MCP request");
            let error_response = json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": error_codes::INTERNAL_ERROR,
                    "message": format!("{:?}", e)
                },
                "id": null
            });
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

#[derive(Deserialize)]
struct ApiKeyQuery {
    key: String,
}

async fn health_check_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}