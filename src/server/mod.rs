//! Server module for vectordb-cli.
//!
//! This module implements the gRPC server functionality for the vectordb-cli tool.

pub mod service;
pub mod config;
mod auth;
mod error;
#[cfg(test)]
mod tests;

pub use config::ServerConfig;
pub use error::{ServerError, Result};

use std::net::SocketAddr;
use std::sync::Arc;
use anyhow;
use qdrant_client::Qdrant;
use crate::config::AppConfig;
use service::VectorDBServiceImpl;
use tokio::sync::oneshot;
use tonic::transport::Server;
use tracing::{info, error};

#[cfg(feature = "server")]
use vectordb_proto;

/// Start the VectorDB gRPC server
///
/// # Arguments
/// * `addr` - The socket address to bind the server to
/// * `config` - The application configuration
/// * `client` - The Qdrant client
/// * `shutdown_signal` - Optional shutdown signal receiver
///
/// # Returns
/// * `anyhow::Result<()>` - The result of the server operation
#[cfg(feature = "server")]
pub async fn start_server(
    addr: SocketAddr,
    config: Arc<AppConfig>,
    client: Arc<Qdrant>,
    shutdown_signal: Option<oneshot::Receiver<()>>,
    use_tls: bool,
    _cert_path: Option<String>,
    _key_path: Option<String>,
) -> anyhow::Result<()> {
    // Extract API key path from config
    let api_key_path = config.server_api_key_path.clone(); // Assuming this field exists

    // Create the service implementation, swapping args and adding api_key_path
    let service = VectorDBServiceImpl::new(client, config, api_key_path)?;
    
    // Register the reflection service
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(vectordb_proto::FILE_DESCRIPTOR_SET)
        .build()?;

    // Start the server with or without TLS
    info!("Starting gRPC server on {}", addr);
    
    let server = Server::builder()
        .add_service(reflection_service)
        .add_service(vectordb_proto::vector_db_service_server::VectorDbServiceServer::new(service));
    
    if use_tls {
        // TLS support is currently disabled for compilation
        error!("TLS support is temporarily disabled for compilation reasons");
        return Err(anyhow::anyhow!("TLS support is temporarily disabled"));
        
        /* TLS support code commented out for compilation - to be fixed in a future update
        if let (Some(cert_path), Some(key_path)) = (cert_path, key_path) {
            let cert = tokio::fs::read(cert_path).await?;
            let key = tokio::fs::read(key_path).await?;
            
            let identity = Identity::from_pem(cert, key);
            let tls_config = ServerTlsConfig::new().identity(identity);
            
            if let Some(signal) = shutdown_signal {
                server
                    .tls_config(tls_config)
                    .serve_with_shutdown(addr, async {
                        let _ = signal.await;
                        info!("Shutdown signal received, stopping server");
                    })
                    .await?;
            } else {
                server
                    .tls_config(tls_config)
                    .serve(addr)
                    .await?;
            }
        } else {
            error!("TLS requested but certificate or key path is missing");
            return Err(anyhow::anyhow!("TLS requested but certificate or key path is missing"));
        }
        */
    } else {
        if let Some(signal) = shutdown_signal {
            server
                .serve_with_shutdown(addr, async {
                    let _ = signal.await;
                    info!("Shutdown signal received, stopping server");
                })
                .await?;
        } else {
            server.serve(addr).await?;
        }
    }
    
    Ok(())
}

/// Non-server mode stub for compilation without the server feature
#[cfg(not(feature = "server"))]
pub async fn start_server(
    _addr: SocketAddr,
    _config: Arc<AppConfig>,
    _client: Arc<Qdrant>,
    _shutdown_signal: Option<oneshot::Receiver<()>>,
    _use_tls: bool,
    _cert_path: Option<String>,
    _key_path: Option<String>,
) -> anyhow::Result<()> {
    error!("Server feature not enabled. Compile with --features=server to enable server mode.");
    Err(anyhow::anyhow!("Server feature not enabled"))
}

/// Creates a function to start the server with default parameters
pub async fn start_default_server(
    port: u16,
    config: Arc<AppConfig>,
    client: Arc<Qdrant>,
) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port).parse()?;
    start_server(addr, config, client, None, false, None, None).await
} 