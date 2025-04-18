#![allow(dead_code)]

use anyhow::{Result, Context};
use clap::Parser;
use std::sync::Arc;
use std::process::exit;
use tracing_subscriber::fmt;
use std::time::Instant;

// Import library modules
use vectordb_lib::{
    config::{self},
    cli::commands::handle_command,
    cli::commands::CliArgs,
    cli::repo_commands::handle_repo_command,
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

    // --- Pre-Command Config Checks --- 
    // Check for required ONNX paths specifically for commands that need them
    match &args.command {
        vectordb_lib::cli::commands::Commands::Repo(repo_cmd) => {
            match &repo_cmd.command {
                vectordb_lib::cli::repo_commands::RepoCommand::Add(_) => {
                    if config.onnx_model_path.is_none() {
                        anyhow::bail!("ONNX model path must be provided in config for 'repo add'");
                    }
                    if config.onnx_tokenizer_path.is_none() {
                        anyhow::bail!("ONNX tokenizer path must be provided in config for 'repo add'");
                    }
                }
                 vectordb_lib::cli::repo_commands::RepoCommand::Sync(_) => {
                    if config.onnx_model_path.is_none() || config.onnx_tokenizer_path.is_none() {
                        anyhow::bail!("ONNX paths must be provided in config for 'repo sync'");
                    }
                 }
                _ => {}
            }
        }
        vectordb_lib::cli::commands::Commands::Simple(simple_cmd) => {
             // Assuming SimpleArgs structure allows access to its command
             // If SimpleArgs.command is private, it needs a similar getter
             match &simple_cmd.command {
                 vectordb_lib::cli::simple::SimpleCommand::Index(_) => {
                     if config.onnx_model_path.is_none() || config.onnx_tokenizer_path.is_none() {
                         anyhow::bail!("ONNX paths must be provided in config for 'simple index'");
                     }
                 }
                 _ => {}
             }
        }
        // Re-add wildcard arm to make match exhaustive
        _ => {}
    }

    // --- Execute Command --- 
    tracing::info!("Executing command: {:?}", args.command);

    // Pass mutable reference to config
    let command_result = handle_command(args, &mut config, client).await;
    tracing::info!("DEBUG: handle_command returned: {:?}", command_result);

    // --- Handle Result ---
    if let Err(e) = command_result {
        tracing::error!("Command execution failed: {:?}", e);
        eprintln!("Error: {}", e);
        return Err(e);
    } else {
         tracing::debug!("Command executed successfully.");
    }

    Ok(())
}
