use anyhow::Result;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;
use vectordb_core::config::AppConfig;
use vectordb_core::load_config;

// Remove mod declarations, they are in lib.rs now
// mod protocol; // Assuming this should have been mcp
// mod server;

// Import Server from the library crate
use vectordb_mcp::server::Server;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("vectordb_mcp=info".parse()?))
        .with_writer(std::io::stderr)
        .init();

    info!("Starting vectordb-mcp server...");

    // Load configuration
    let config_path = vectordb_core::get_config_path_or_default(None)?;
    let config_path_option: Option<PathBuf> = Some(config_path);

    let config: AppConfig = load_config(config_path_option.as_ref())
        .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;

    info!("Using configuration: {:?}", config);
    // info!("vectordb-mcp setup complete. Server logic to be implemented.");

    // Initialize the server (now async)
    let server = Server::new(config).await?;

    // Run the server
    if let Err(e) = server.run().await {
        tracing::error!(error = %e, "Server encountered an error");
        std::process::exit(1);
    }

    Ok(())
}
