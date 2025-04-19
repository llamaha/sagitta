use anyhow::Result;
use clap::{Args, Subcommand};
use std::sync::Arc;
use std::path::PathBuf;
use crate::config::AppConfig;
use tracing::error;
use crate::cli::commands::CliArgs;
use qdrant_client::Qdrant;

// Import server module only when server feature is enabled
#[cfg(feature = "server")]
use crate::server::ServerConfig;

#[cfg(feature = "server")]
use log::info;

#[cfg(feature = "server")]
use tonic::transport::server::ServerTlsConfig;

#[cfg(feature = "server")]
use tonic::transport::Identity;

#[cfg(feature = "server")]
use tokio::fs::read;

/// CLI arguments for the server command
#[derive(Debug, Clone, Args)]
pub struct ServerArgs {
    #[command(subcommand)]
    pub command: ServerCommands,
}

/// Server commands
#[derive(Debug, Clone, Subcommand)]
pub enum ServerCommands {
    /// Start the VectorDB server
    Start(ServerStartArgs),
}

/// Arguments for the server start command
#[derive(Debug, Clone, Args)]
pub struct ServerStartArgs {
    /// Port to listen on
    #[arg(short, long, default_value = "50051")]
    pub port: u16,
    
    /// Host address to bind to
    #[arg(short, long, default_value = "0.0.0.0")]
    pub host: String,
    
    /// Whether to use TLS
    #[arg(long)]
    pub tls: bool,
    
    /// Path to TLS certificate file
    #[arg(long = "tls-cert")]
    pub cert: Option<PathBuf>,
    
    /// Path to TLS key file
    #[arg(long = "tls-key")]
    pub key: Option<PathBuf>,
    
    /// API key for client authentication
    #[arg(long = "api-key")]
    pub api_key: Option<String>,
    
    /// Path to API key file for authentication (alternative to --api-key)
    #[arg(long = "api-key-file")]
    pub api_key_file: Option<PathBuf>,
    
    /// Whether authentication is required
    #[arg(long)]
    pub require_auth: bool,
    
    /// Maximum number of concurrent requests
    #[arg(long, default_value = "100")]
    pub max_concurrent_requests: usize,
}

/// Handle the server command
pub async fn handle_server_command(
    args: ServerArgs,
    _cli_args: &CliArgs,
    config: AppConfig,
    client: Arc<Qdrant>,
) -> Result<()> {
    match args.command {
        ServerCommands::Start(start_args) => handle_server_start(&config, start_args, client).await,
    }
}

// Server implementation when server feature is enabled
#[cfg(feature = "server")]
async fn handle_server_start(
    config: &AppConfig,
    args: ServerStartArgs,
    client: Arc<Qdrant>,
) -> Result<()> {
    // Set up server configuration
    let server_config = ServerConfig {
        port: args.port,
        host: args.host.clone(),
        use_tls: args.tls,
        cert_path: args.cert.clone(),
        key_path: args.key.clone(),
        api_key: args.api_key.clone(),
        api_key_file: args.api_key_file.clone(),
        require_auth: args.require_auth,
        max_concurrent_requests: args.max_concurrent_requests,
        max_batch_size: 128, // Default batch size
    };
    
    // Validate TLS configuration if enabled
    if let Err(e) = server_config.validate_tls() {
        error!("TLS configuration error: {}", e);
        return Err(anyhow::anyhow!("TLS configuration error: {}", e));
    }
    
    // Validate auth configuration if enabled
    if let Err(e) = server_config.validate_auth() {
        error!("Authentication configuration error: {}", e);
        return Err(anyhow::anyhow!("Authentication configuration error: {}", e));
    }
    
    // Get socket address
    let addr = match server_config.socket_addr() {
        Ok(addr) => addr,
        Err(e) => {
            error!("Invalid server address: {}", e);
            return Err(anyhow::anyhow!("Invalid server address: {}", e));
        }
    };
    
    // Start the server
    info!("Starting VectorDB server on {}...", addr);
    
    let app_config = Arc::new(config.clone());
    
    let tls_config = if let (Some(cert_path), Some(key_path)) = 
        (server_config.cert_path.as_ref(), server_config.key_path.as_ref())
    {
        info!("Loading TLS certificate from: {}", cert_path.display());
        info!("Loading TLS key from: {}", key_path.display());
        let cert = read(cert_path).await.map_err(|e| anyhow::anyhow!("Failed to read cert file: {}", e))?;
        let key = read(key_path).await.map_err(|e| anyhow::anyhow!("Failed to read key file: {}", e))?;
        let identity = Identity::from_pem(cert, key);
        Some(ServerTlsConfig::new().identity(identity))
    } else {
        info!("TLS not configured, starting server without encryption.");
        None
    };
    
    let require_auth = server_config.require_auth;
    let api_key = server_config.api_key;
    let max_requests = server_config.max_concurrent_requests;
    
    crate::server::start_server(addr, app_config, client, api_key, require_auth, tls_config, Some(max_requests), None).await?;
    
    Ok(())
}

// Stub implementation when server feature is not enabled
#[cfg(not(feature = "server"))]
async fn handle_server_start(
    _config: &AppConfig,
    _args: ServerStartArgs,
    _client: Arc<Qdrant>,
) -> Result<()> {
    error!("Server feature not enabled. Compile with --features=server to enable server mode.");
    Err(anyhow::anyhow!("Server feature not enabled. Use `cargo build --features server` to compile with server support."))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_server_args_structure() {
        // Test the structure of ServerArgs
        let cmd = ServerCommands::Start(ServerStartArgs {
            port: 8080,
            host: "127.0.0.1".to_string(),
            tls: false,
            cert: None,
            key: None,
            api_key: None,
            api_key_file: None,
            require_auth: false,
            max_concurrent_requests: 100,
        });
        
        let args = ServerArgs { command: cmd };
        
        // Verify the command can be matched
        match args.command {
            ServerCommands::Start(start_args) => {
                assert_eq!(start_args.port, 8080);
                assert_eq!(start_args.host, "127.0.0.1");
            }
        }
    }
    
    #[test]
    fn test_server_start_args_defaults() {
        // This test checks that the default values are correctly defined in annotations
        let default_port = 50051;
        let default_host = "0.0.0.0";
        let default_concurrent_requests = 100;
        
        // Create an instance with default values
        let args = ServerStartArgs {
            port: default_port,
            host: default_host.to_string(),
            tls: false,
            cert: None,
            key: None,
            api_key: None,
            api_key_file: None,
            require_auth: false,
            max_concurrent_requests: default_concurrent_requests,
        };
        
        // Verify default values
        assert_eq!(args.port, default_port, "Default port should be {}", default_port);
        assert_eq!(args.host, default_host, "Default host should be {}", default_host);
        assert_eq!(args.tls, false, "TLS should be disabled by default");
        assert_eq!(args.require_auth, false, "Auth should not be required by default");
        assert_eq!(args.max_concurrent_requests, default_concurrent_requests, 
            "Default max concurrent requests should be {}", default_concurrent_requests);
    }
    
    #[test]
    fn test_server_start_args_custom_values() {
        // Create a ServerStartArgs with custom values
        let args = ServerStartArgs {
            port: 8080,
            host: "127.0.0.1".to_string(),
            tls: true,
            cert: Some(PathBuf::from("/path/to/cert.pem")),
            key: Some(PathBuf::from("/path/to/key.pem")),
            api_key: Some("test-key".to_string()),
            api_key_file: Some(PathBuf::from("/path/to/api-key.txt")),
            require_auth: true,
            max_concurrent_requests: 200,
        };
        
        // Verify custom values
        assert_eq!(args.port, 8080);
        assert_eq!(args.host, "127.0.0.1");
        assert_eq!(args.tls, true);
        assert_eq!(args.cert, Some(PathBuf::from("/path/to/cert.pem")));
        assert_eq!(args.key, Some(PathBuf::from("/path/to/key.pem")));
        assert_eq!(args.api_key, Some("test-key".to_string()));
        assert_eq!(args.api_key_file, Some(PathBuf::from("/path/to/api-key.txt")));
        assert_eq!(args.require_auth, true);
        assert_eq!(args.max_concurrent_requests, 200);
    }
} 