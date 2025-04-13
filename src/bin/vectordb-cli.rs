#![allow(dead_code)]

use anyhow::Result;
use clap::Parser;
// Remove log imports if env_logger is fully replaced
// use log::{debug, error, info};
use std::sync::Arc;
use std::process::exit;
use tracing::{info, debug, error};

// Import library modules
use vectordb_lib::cli::{self, CliArgs}; // Import CliArgs from lib, Commands not needed here
use vectordb_lib::config::{self}; // Import config
use qdrant_client::Qdrant;
// Comment out missing setup_logging import
// use vectordb_lib::setup_logging;

// CliArgs struct moved to src/cli/commands.rs
// #[derive(Parser, Debug)]
// #[command(author, version, about, long_about = None)]
// struct CliArgs {
//     #[command(subcommand)]
//     command: Commands,

//     // Arguments for ONNX paths (these might be moved to config later, but keep for now)
//     /// Path to ONNX model file (overrides config & env var)
//     #[arg(long = "onnx-model", global = true)]
//     onnx_model_path_arg: Option<String>,

//     /// Path to ONNX tokenizer config directory (overrides config & env var)
//     #[arg(long = "onnx-tokenizer-dir", global = true)] // Changed name for clarity
//     onnx_tokenizer_dir_arg: Option<String>,
// }

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // --- Setup Logging using tracing_subscriber ---
    tracing_subscriber::fmt::init(); // Initialize tracing subscriber
    // Remove: env_logger::init();

    let args = CliArgs::parse();

    // --- Load Configuration --- 
    // Use tracing::info, debug, error instead of log::*
    let config = config::load_config()
        .inspect_err(|e| tracing::error!("Configuration loading failed: {:?}", e))
        .unwrap_or_default(); // Use default config if loading fails
    
    tracing::info!("Using Qdrant URL from config: {}", config.qdrant_url);

    // --- Initialize Qdrant Client (using config URL) ---
    tracing::debug!("Initializing Qdrant Client...");
    let qdrant_client_result = qdrant_client::Qdrant::from_url(&config.qdrant_url).build();

    let _client: Arc<Qdrant> = match qdrant_client_result {
         Ok(client_instance) => {
             tracing::debug!("Qdrant client initialized successfully.");
             Arc::new(client_instance)
         },
         Err(e) => {
             tracing::error!("Failed to initialize Qdrant client: {}", e);
             eprintln!("Error initializing Qdrant client: {}", e);
             eprintln!("Please check Qdrant URL in config ({}) and ensure the server is running.", config.qdrant_url);
             exit(1);
         }
    };

    // --- Execute Command --- 
    tracing::info!("Executing command: {:?}", args.command);

    // Pass CliArgs and config down
    let result = cli::handle_command(args, config).await;

    // --- Handle Result ---
    if let Err(e) = result {
        tracing::error!("Command execution failed: {:?}", e);
        eprintln!("Error: {}", e);
        exit(1);
    } else {
         tracing::debug!("Command executed successfully.");
    }

    Ok(())
}
