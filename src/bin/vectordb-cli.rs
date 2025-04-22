#![allow(dead_code)]

use anyhow::{Result, anyhow};
use clap::Parser;
use std::sync::Arc;
use std::path::PathBuf;

// Import library modules
use vectordb_lib::{
    cli::commands::{handle_command, CliArgs},
};
use qdrant_client::Qdrant;

// Use items from the new core library
use vectordb_core::config::{AppConfig as CoreAppConfig, load_config as core_load_config};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger from RUST_LOG env var or default to info
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Parse command-line arguments
    let args = CliArgs::parse();

    // setup_logging(args.verbose); // E0609: no field `verbose` - Keep commented out for now
    
    // Determine configuration path
    // let config_path_override = args.config.clone(); // E0609: no field `config`
    let config_path_override: Option<PathBuf> = None; // Placeholder, fix if needed

    // Load configuration using core_load_config
    let mut config = match core_load_config(config_path_override.as_ref()) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            eprintln!("Using default configuration.");
            CoreAppConfig::default()
        }
    };

    // Handle ONNX model/tokenizer path overrides from CLI args
    if let Some(model_path_str) = &args.onnx_model_path_arg {
        config.onnx_model_path = Some(PathBuf::from(model_path_str).to_string_lossy().into_owned());
    }
    if let Some(tokenizer_path_str) = &args.onnx_tokenizer_dir_arg {
        config.onnx_tokenizer_path = Some(PathBuf::from(tokenizer_path_str).to_string_lossy().into_owned());
    }

    // Ensure required ONNX files exist if not provided by default config
    if config.onnx_model_path.is_none() || config.onnx_tokenizer_path.is_none() {
        // Simplified error message - check config/CLI args for paths
         return Err(anyhow!(
             "ONNX model path or tokenizer path not specified. \
              Please provide them via CLI arguments (--onnx-model-path, --onnx-tokenizer-dir) \
              or ensure they are set in the configuration file."
         ));
    }

    // Initialize Qdrant client
    let qdrant_client = match Qdrant::from_url(&config.qdrant_url).build() {
        Ok(client) => Arc::new(client),
        Err(e) => {
            eprintln!("Failed to connect to Qdrant at {}: {}", config.qdrant_url, e);
            return Err(anyhow!("Qdrant connection failed"));
        }
    };

    // Handle the command
    let command_result = handle_command(
        args,
        &mut config, // Pass config mutably
        qdrant_client,
       // config_path_override.as_ref() // Remove extra argument (E0061)
    ).await;

    // Handle potential errors from command execution
    if let Err(e) = command_result {
        eprintln!("Error executing command: {}", e);
        // Consider exiting with a non-zero status code
        std::process::exit(1); 
    }

    Ok(())
}
