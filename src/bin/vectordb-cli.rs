#![allow(dead_code)]

use anyhow::{Result, Context};
use clap::Parser;
use std::sync::Arc;
use std::process::exit;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

// Import library modules
use vectordb_lib::{
    config,
    cli::commands::handle_command,
    cli::commands::CliArgs,
};
use qdrant_client::Qdrant;

#[tokio::main]
async fn main() -> Result<()> {
    // --- Setup Tracing --- 
    fmt::init(); // Initialize tracing subscriber

    // --- Parse Args --- 
    let args = CliArgs::parse();

    // --- Load Configuration --- 
    let mut config = config::load_config(None).context("Failed to load configuration")?;
    
    tracing::info!("Using Qdrant URL from config: {}", config.qdrant_url);

    // --- Initialize Qdrant Client (using config URL) ---
    tracing::debug!("Initializing Qdrant Client...");
    let qdrant_client_result = qdrant_client::Qdrant::from_url(&config.qdrant_url).build();

    let client: Arc<Qdrant> = match qdrant_client_result {
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

    // Pass mutable reference to config
    let result = handle_command(args, &mut config, client).await;

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
