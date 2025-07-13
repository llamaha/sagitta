#![allow(dead_code)]

use anyhow::{Result, anyhow};
use clap::Parser;
use std::sync::Arc;
use std::path::PathBuf;

// Import library modules from the lib target (now named sagitta_cli)
use sagitta_cli::cli::{handle_command, CliArgs};
use qdrant_client::Qdrant;

// Use items from the new core library
use sagitta_search::config::{AppConfig as CoreAppConfig, load_config as core_load_config};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger from RUST_LOG env var or default to error
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();

    // Parse command-line arguments
    let args = CliArgs::parse();

    // Determine configuration path
    let config_path_override: Option<PathBuf> = None; // Placeholder, fix if needed

    // Load configuration using core_load_config
    let mut config = match core_load_config(config_path_override.as_ref()) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error loading configuration: {e}");
            eprintln!("Using default configuration.");
            CoreAppConfig::default()
        }
    };


    // Handle Init command separately and exit early
    if matches!(args.command, sagitta_cli::cli::Commands::Init) {
        return sagitta_cli::cli::commands::execute_init_command(&mut config).await;
    }


    // Check if this is a simple command (they have special validation rules)
    let is_simple_command = matches!(args.command, sagitta_cli::cli::Commands::Simple(_));
    let is_simple_index_command = matches!(
        args.command, 
        sagitta_cli::cli::Commands::Simple(ref simple_args) 
            if matches!(simple_args.command, sagitta_cli::cli::simple::SimpleCommand::Index(_))
    );
    
    // For simple index commands, validate ONNX arguments/env vars before merging them into config
    if is_simple_index_command {
        if args.onnx_model_path_arg.is_some() || std::env::var("SAGITTA_ONNX_MODEL").is_ok() {
            return Err(anyhow!("For 'simple index', ONNX model path must be provided solely via the configuration file, not CLI arguments or environment variables."));
        }
        if args.onnx_tokenizer_dir_arg.is_some() || std::env::var("SAGITTA_ONNX_TOKENIZER_DIR").is_ok() {
            return Err(anyhow!("For 'simple index', ONNX tokenizer path must be provided solely via the configuration file, not CLI arguments or environment variables."));
        }
    } else if !is_simple_command {
        // For non-simple commands, handle ONNX model/tokenizer path overrides from CLI args
        if let Some(model_path_str) = &args.onnx_model_path_arg {
            config.onnx_model_path = Some(PathBuf::from(model_path_str).to_string_lossy().into_owned());
        }
        if let Some(tokenizer_path_str) = &args.onnx_tokenizer_dir_arg {
            config.onnx_tokenizer_path = Some(PathBuf::from(tokenizer_path_str).to_string_lossy().into_owned());
        }
    }

    // Ensure required model configuration exists (but not for simple commands which have their own validation)
    if !is_simple_command && config.embed_model.is_none() && (config.onnx_model_path.is_none() || config.onnx_tokenizer_path.is_none()) {
        return Err(anyhow!(
            "No embedding model configuration found.\n\
Please provide one of the following:\n\
1. Set 'embed_model' in config.toml (e.g., embed_model = \"bge-small-fast\", \"bge-small-fp32\", \"bge-medium\", or \"bge-large\")\n\
2. Provide ONNX paths via CLI arguments (--onnx-model-path, --onnx-tokenizer-dir)\n\
3. Set ONNX paths in config.toml:\n\
    onnx_model_path = \"/absolute/path/to/model.onnx\"\n\
    onnx_tokenizer_path = \"/absolute/path/to/tokenizer.json\"\n\
\nNote: 'embed_model' and ONNX paths cannot be used together."
        ));
    }
    
    // Validate configuration (but allow simple commands to handle their own validation)
    if !is_simple_command {
        if let Err(e) = config.validate() {
            return Err(anyhow!("Configuration validation error: {}", e));
        }
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
    ).await;

    // Handle potential errors from command execution
    if let Err(e) = command_result {
        eprintln!("Error executing command: {e}");
        // Consider exiting with a non-zero status code
        std::process::exit(1); 
    }

    Ok(())
}
