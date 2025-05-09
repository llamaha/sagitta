use axum::{
    routing::{get, post},
    Router,
    extract::{State, Query},
    response::{sse::Sse, sse::Event, IntoResponse, Response as AxumResponse},
    http::{StatusCode, HeaderMap},
    Json,
};
use dashmap::DashMap;
use futures_util::stream::{Stream, Abortable, AbortHandle};
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, broadcast};
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tracing::{error, info, warn, instrument};
use uuid::Uuid;
use futures_util::stream::{StreamExt, AbortRegistration};
use tokio_stream::StreamMap; // For managing multiple streams if needed, not directly for this
use async_stream; // Make sure it's in scope if `async_stream::stream!` is used
use serde_json::json; // Add this import
use serde::Deserialize; // For Query extractor

use crate::server::Server;
use qdrant_client::Qdrant;
use crate::mcp::types::{Request as McpRequest, Response as McpResponse, ErrorObject};
use crate::mcp::error_codes;

// Shared state for the Axum application
#[derive(Clone)]
pub struct AppState {
    pub server: Arc<Server<Qdrant>>,
    pub active_connections: Arc<DashMap<Uuid, mpsc::Sender<String>>>,
}

// No longer generic over C
pub async fn run_http_server(
    addr: String,
    mcp_server_concrete: Server<Qdrant>,
) -> anyhow::Result<()> {
    let shared_state = AppState {
        server: Arc::new(mcp_server_concrete),
        active_connections: Arc::new(DashMap::new()),
    };

    // Extract host and port from the addr string
    let server_url = format!("http://{}", addr);
    info!(server_url = %server_url, "Server URL for SSE endpoint");

    let app = Router::new()
        .route("/sse", get(sse_handler))
        .route("/message", post(message_handler))
        .with_state(shared_state);

    info!(address = %addr, "MCP HTTP server listening (SSE on /sse, POST on /message)");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

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
