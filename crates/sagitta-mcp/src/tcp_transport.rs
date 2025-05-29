use crate::mcp::error_codes;
use crate::mcp::types::{Request, Response, ErrorObject};
use crate::server::Server; // Assuming Server is pub and in this path
use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{error, info, warn, instrument};
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;

#[instrument(skip(server))]
pub async fn run_tcp_server<C: QdrantClientTrait + Send + Sync + 'static>(
    addr: String,
    server: Arc<Server<C>>,
) -> Result<()> {
    let listener = TcpListener::bind(&addr).await?;
    info!(address = %addr, "MCP server listening on TCP");

    loop {
        match listener.accept().await {
            Ok((stream, client_addr)) => {
                info!(client = %client_addr, "Accepted new TCP connection");
                let server_clone = Arc::clone(&server);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, server_clone).await {
                        error!(client = %client_addr, error = %e, "Connection error");
                    }
                });
            }
            Err(e) => {
                error!(error = %e, "Failed to accept TCP connection");
                // Consider whether to continue or break here based on error type
            }
        }
    }
}

#[instrument(skip(stream, server), fields(client_addr = ?stream.peer_addr().ok()))]
async fn handle_connection<C: QdrantClientTrait + Send + Sync + 'static>(
    stream: TcpStream,
    server: Arc<Server<C>>,
) -> Result<()> {
    let (reader_half, writer_half) = stream.into_split();
    let mut reader = BufReader::new(reader_half);
    let mut writer = BufWriter::new(writer_half);
    let mut line_buf = String::new();

    loop {
        line_buf.clear();
        match reader.read_line(&mut line_buf).await {
            Ok(0) => {
                info!("TCP connection closed by client");
                break;
            }
            Ok(_) => {
                let trimmed_line = line_buf.trim();
                if trimmed_line.is_empty() {
                    continue;
                }

                info!(request = %trimmed_line, "Received TCP request");

                let response_to_send = match serde_json::from_str::<Request>(trimmed_line) {
                    Ok(request) => {
                        let request_id = request.id.clone();
                        
                        // Updated call to handle_request
                        let result = server.handle_request(request).await;

                        match result {
                            Ok(Some(res_val)) => Some(Response::success(res_val, request_id)),
                            Ok(None) => None, // For notifications that don't send a response
                            Err(err_obj) => Some(Response::error(err_obj, request_id)),
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to parse TCP request JSON");
                        Some(Response::error(
                            ErrorObject {
                                code: error_codes::PARSE_ERROR,
                                message: format!("Failed to parse request: {}", e),
                                data: None,
                            },
                            None, // No ID if request parsing failed
                        ))
                    }
                };

                if let Some(response) = response_to_send {
                    match serde_json::to_string(&response) {
                        Ok(response_json) => {
                            info!(response = %response_json, "Sending TCP response");
                            if let Err(e) = writer.write_all(response_json.as_bytes()).await {
                                error!(error = %e, "Failed to write TCP response");
                                break;
                            }
                            if let Err(e) = writer.write_all(b"\n").await {
                                error!(error = %e, "Failed to write newline for TCP response");
                                break;
                            }
                            if let Err(e) = writer.flush().await {
                                error!(error = %e, "Failed to flush TCP writer");
                                break;
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to serialize TCP response");
                            // Potentially send a generic error response back to client if possible
                        }
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Error reading from TCP stream");
                break; // Break on read error
            }
        }
    }
    Ok(())
} 