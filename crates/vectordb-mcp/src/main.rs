use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::EnvFilter;
use vectordb_core::config::AppConfig;
use vectordb_core::load_config;
// Import File for logging
// use std::fs::File;

// Remove mod declarations, they are in lib.rs now
// mod protocol; // Assuming this should have been mcp
// mod server;

// Import Server from the library crate
use vectordb_mcp::server::Server;
// Remove TCP transport import if no longer primary
// use vectordb_mcp::tcp_transport::run_tcp_server;
use vectordb_mcp::http_transport::run_http_server; // Import HTTP server runner
use clap::Parser; // Import clap parser

/// MCP Server for VectorDB
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Transport mode to use
    #[clap(subcommand)]
    transport: TransportMode,
}

#[derive(Parser, Debug)]
enum TransportMode {
    /// Use standard input/output for communication
    Stdio,
    /// Use HTTP with SSE for network communication
    Http {
        /// Host address to bind the HTTP server to
        #[clap(long, default_value = "0.0.0.0")]
        host: String,

        /// Port to bind the HTTP server to
        #[clap(short, long, default_value_t = 8080)] // Default HTTP port
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("vectordb_mcp=info".parse()?))
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();

    info!("Starting vectordb-mcp server...");

    // Load configuration
    let config_path = vectordb_core::get_config_path_or_default(None)?;
    let config_path_option: Option<PathBuf> = Some(config_path);

    let config: AppConfig = load_config(config_path_option.as_ref())
        .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;

    info!("Using configuration: {:?}", config);
    // info!("vectordb-mcp setup complete. Server logic to be implemented.");

    // Server is created once
    let server_instance = Server::new(config).await?;

    match args.transport {
        TransportMode::Stdio => {
            info!("Running in Stdio mode");
            // Stdio mode consumes the server instance
            if let Err(e) = server_instance.run().await {
                tracing::error!(error = %e, "Stdio server encountered an error");
                std::process::exit(1);
            }
        }
        TransportMode::Http { host, port } => {
            let addr = format!("{}:{}", host, port);
            info!(address = %addr, "Running in HTTP/SSE mode");
            // HTTP mode also consumes the server instance for its AppState
            if let Err(e) = run_http_server(addr, server_instance).await {
                tracing::error!("HTTP/SSE server encountered an error: {:?}", e);
                for cause in e.chain().skip(1) {
                    tracing::error!("Caused by: {}", cause);
                }
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
