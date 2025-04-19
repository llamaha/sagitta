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
use crate::server::service::VectorDBServiceImpl;
use crate::edit::grpc::EditingServiceImpl;
use tonic::transport::Server;
use tracing::{info, error, warn};
use anyhow::Context;
use tokio::sync::oneshot;

#[cfg(feature = "server")]
use {
    // Use nested module path for VectorDbServiceServer from vectordb_proto
    vectordb_proto::vector_db_service_server::VectorDbServiceServer,
    // Use path from crate::grpc_generated::editing for EditingServiceServer
    crate::grpc_generated::editing::editing_service_server::EditingServiceServer,
    vectordb_proto::FILE_DESCRIPTOR_SET as VECTORDB_FILE_DESCRIPTOR_SET,
    crate::grpc_generated::EDITING_FILE_DESCRIPTOR_SET
};

/// Start the VectorDB gRPC server
///
/// # Arguments
/// * `addr` - The socket address to bind the server to
/// * `config` - The application configuration
/// * `client` - The Qdrant client
/// * `shutdown_signal` - Optional shutdown signal receiver
///
/// # Returns
/// * `crate::server::Result<()>` - The result of the server operation using local Result
#[cfg(feature = "server")]
pub async fn start_server(
    addr: SocketAddr,
    config: Arc<AppConfig>,
    client: Arc<Qdrant>,
    _api_key: Option<String>,
    _require_auth: bool,
    tls_config: Option<tonic::transport::server::ServerTlsConfig>,
    _max_concurrent_requests: Option<usize>,
    shutdown_rx: Option<oneshot::Receiver<()>>,
) -> Result<()> {
    info!("Entering start_server function (server feature enabled)");

    // Extract API key path from config
    let api_key_path = config.server_api_key_path.clone();

    // Create the service implementation, swapping args and adding api_key_path
    let vectordb_service = VectorDBServiceImpl::new(client.clone(), config.clone(), api_key_path)?;
    
    // Create the new editing service implementation instance
    info!("Creating EditingServiceImpl instance...");
    let editing_service_impl = EditingServiceImpl::default();
    
    // Register the reflection service
    info!("Building reflection service...");
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(VECTORDB_FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(EDITING_FILE_DESCRIPTOR_SET)
        .build_v1()?;

    // Start the server with or without TLS
    info!("Configuring server builder on {}", addr);
    
    info!("Adding services (Reflection, VectorDB, Editing)...");
    let server_builder = Server::builder()
        .add_service(reflection_service)
        .add_service(VectorDbServiceServer::new(vectordb_service))
        .add_service(EditingServiceServer::new(editing_service_impl));
    info!("Finished adding services.");
    
    if let Some(_tls_config) = tls_config {
        // TLS support is currently disabled for compilation
        error!("TLS support is temporarily disabled for compilation reasons");
        return Err(anyhow::anyhow!("TLS support is temporarily disabled").into());
        
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
        info!("Starting server without TLS...");
        let os_shutdown = shutdown_signal();
        let test_shutdown = async { 
            if let Some(rx) = shutdown_rx {
                let _ = rx.await;
                info!("Test shutdown signal received.");
            } else {
                std::future::pending::<()>().await;
            }
        };

        tokio::select! {
            biased;

            _ = os_shutdown => {
                info!("OS shutdown signal received, stopping server.");
            }
            _ = test_shutdown => {
                 info!("Test-specific shutdown signal received, stopping server.");
            }

            res = server_builder.serve(addr) => {
                if let Err(e) = res {
                    error!("Server failed to serve: {}", e);
                    return Err(e.into());
                }
                info!("Server finished serving normally.");
            }
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
    _api_key: Option<String>,
    _require_auth: bool,
    _tls_config: Option<tonic::transport::server::ServerTlsConfig>,
    _max_concurrent_requests: Option<usize>,
    _shutdown_rx: Option<oneshot::Receiver<()>>,
) -> anyhow::Result<()> {
    error!("Server feature not enabled. Compile with --features=server to enable server mode.");
    Err(anyhow::anyhow!("Server feature not enabled"))
}

/// Creates a function to start the server with default parameters
pub async fn start_default_server(
    port: u16,
    config: Arc<AppConfig>,
    client: Arc<Qdrant>,
) -> crate::server::Result<()> {
    let addr = format!("0.0.0.0:{}", port).parse()?;
    start_server(addr, config, client, None, false, None, None, None).await
}

#[cfg(feature = "server")]
async fn shutdown_signal() {
    // Wait for Ctrl+C or SIGTERM
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Termination signal received.");
}

#[cfg(feature = "server")]
pub async fn run_server(
    config: AppConfig,
    _api_key: Option<String>,
    _require_auth: bool,
    addr: SocketAddr,
    _max_concurrent_requests: Option<usize>,
    tls_config: Option<tonic::transport::server::ServerTlsConfig>,
) -> Result<()> {
    let config = Arc::new(config);
    info!("Server starting on {}", addr);
    
    let client = Arc::new(Qdrant::from_url(&config.qdrant_url).build()
        .context("Failed to build Qdrant client")?);

    let service = VectorDBServiceImpl::new(Arc::clone(&client), config, None)?;

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(vectordb_proto::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(crate::grpc_generated::EDITING_FILE_DESCRIPTOR_SET)
        .build_v1()
        .unwrap();
    
    let mut builder = Server::builder();

    if let Some(_tls_config) = tls_config {
        warn!("TLS configuration is provided but not yet implemented!");
    } else {
        info!("Starting server without TLS.");
    }

    builder
        .add_service(VectorDbServiceServer::new(service))
        .add_service(reflection_service)
        .serve(addr)
        .await
        .context("gRPC server failed")?;

    Ok(())
}