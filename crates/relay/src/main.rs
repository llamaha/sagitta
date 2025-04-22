pub mod advisors;
pub mod chain;
pub mod cli;
pub mod config;
pub mod context;
pub mod investigation;
pub mod llm;
pub mod tools;
pub mod utils;

use anyhow::Result;
// Use tracing imports directly
use tracing::{info, error};
use tracing_subscriber::fmt;
use crate::config as relay_config;
use crate::context::AppContext;
use crate::llm::AnthropicClient;
use qdrant_client::Qdrant;
// Add the missing import alias for vectordb_core::config
use vectordb_core::config as vdb_config;
use std::sync::Arc;
use anyhow::Context;
use crate::cli::CliArgs;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber
    // Use RUST_LOG env var for level control (e.g., RUST_LOG=relay=debug,info)
    // Defaults to INFO level if RUST_LOG is not set.
    fmt::init();
    info!("Relay agent starting...");

    // --- Load Relay Configuration ---
    let relay_cfg = relay_config::load_config()
        .context("Failed to load Relay configuration")?;
    let relay_config_arc = Arc::new(relay_cfg);

    // --- Load vectordb_lib Configuration ---
    // Using the path from the Relay config might be needed if it's not default
    let vdb_cfg = vdb_config::load_config(None) // Load default path for now
         .context("Failed to load vectordb_lib configuration")?;
     let vdb_config_arc = Arc::new(vdb_cfg);

    // --- Initialize LLM Client (using RelayConfig) ---
    let llm_client = AnthropicClient::new(&relay_config_arc)
        .context("Failed to initialize Anthropic client")?;
    let llm_client_arc = Arc::new(llm_client);

    // --- Initialize Qdrant Client (using RelayConfig for URL) ---
    info!(url = %relay_config_arc.qdrant_url, "Initializing Qdrant client...");
    let qdrant_client = Qdrant::from_url(&relay_config_arc.qdrant_url).build()
        .context(format!("Failed to build Qdrant client at URL: {}", &relay_config_arc.qdrant_url))?;
     let qdrant_client_arc = Arc::new(qdrant_client);
     match qdrant_client_arc.health_check().await {
         Ok(health) => info!(?health, "Qdrant health check successful"),
         Err(e) => {
             let err_msg = format!("Qdrant health check failed: {}", e);
             error!(error = %err_msg, url = %relay_config_arc.qdrant_url);
             // Use anyhow context for clearer error propagation
             return Err(anyhow::anyhow!(err_msg).context("Qdrant health check failed"));
         }
     }

    // --- Create App Context --- 
    let app_context = AppContext {
        relay_config: relay_config_arc.clone(),
        vdb_config: vdb_config_arc.clone(),
        llm_client: llm_client_arc.clone(),
        qdrant_client: qdrant_client_arc.clone(),
    };

    // --- Parse CLI Arguments --- 
    let cli_args = CliArgs::parse();
    info!(prompt = %cli_args.prompt, "Parsed CLI arguments.");

    // --- Execute Main Logic --- 
    info!("Starting main execution...");
    investigation::run_basic_loop(app_context, cli_args.prompt).await?;
    
    info!("Relay agent finished.");
    Ok(())
}
