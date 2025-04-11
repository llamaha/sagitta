#![allow(dead_code)]

use vectordb_lib::vectordb::VectorDB;
use anyhow::Result;
use clap::Parser;
use log::{debug, error};
use std::path::PathBuf;
use std::fs;
use dirs::data_local_dir;
use tracing_subscriber;

use vectordb_lib::cli;
use vectordb_lib::VectorDBConfig;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: cli::commands::Command,

    /// Optional path to the database file (defaults to system's local data dir)
    // #[arg(long = "db-path", global = true)]
    // db_path: Option<PathBuf>,

    // Add global args for ONNX paths, mirroring the Index command args
    /// Path to ONNX model file (can also be set via VECTORDB_ONNX_MODEL env var)
    #[arg(long = "onnx-model", global = true)]
    onnx_model_path_arg: Option<String>,

    /// Path to ONNX tokenizer file (can also be set via VECTORDB_ONNX_TOKENIZER env var)
    #[arg(long = "onnx-tokenizer", global = true)]
    onnx_tokenizer_path_arg: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    debug!("Initializing vectordb-cli with command: {:?}", cli.command);

    // --- Determine Database Path (Always use default) ---
    let default_dir = data_local_dir()
        .unwrap_or_else(|| PathBuf::from(".")) // Fallback to current dir
        .join("vectordb-cli");
    // Ensure the default directory exists (create_dir_all is idempotent)
    fs::create_dir_all(&default_dir)?;
    let db_file_path_str = default_dir.join("db.json").to_string_lossy().into_owned();

    debug!("Using database path: {}", db_file_path_str);

    // --- Determine ONNX Model and Tokenizer Paths ---
    // Priority: CLI arg > Env Var > Default (if applicable, none here)
    let onnx_model_path_str = cli.onnx_model_path_arg
        .or_else(|| std::env::var("VECTORDB_ONNX_MODEL").ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "ONNX model path must be provided via --onnx-model argument or VECTORDB_ONNX_MODEL env var"
            )
        })?;

    let onnx_tokenizer_path_str = cli.onnx_tokenizer_path_arg
        .or_else(|| std::env::var("VECTORDB_ONNX_TOKENIZER").ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "ONNX tokenizer path must be provided via --onnx-tokenizer argument or VECTORDB_ONNX_TOKENIZER env var"
            )
        })?;

    let onnx_model_path = PathBuf::from(onnx_model_path_str);
    let onnx_tokenizer_path = PathBuf::from(onnx_tokenizer_path_str);
    debug!("Using ONNX model: {}", onnx_model_path.display());
    debug!("Using ONNX tokenizer: {}", onnx_tokenizer_path.display());

    // --- Create VectorDB Config ---
    let db_config = VectorDBConfig {
        db_path: db_file_path_str,
        onnx_model_path,
        onnx_tokenizer_path,
    };

    // --- Initialize VectorDB ---
    debug!("Initializing VectorDB...");
    let db_result = VectorDB::new(db_config);

    let db = match db_result {
         Ok(db_instance) => {
             debug!("VectorDB initialized successfully.");
             db_instance
         },
         Err(e) => {
             error!("Failed to initialize VectorDB: {}", e);
             // Print user-friendly error message
             eprintln!("Error initializing database: {}", e);
             eprintln!("Please check configuration (db path, model paths) and file permissions.");
             // Exit gracefully
             return Err(e.into());
         }
    };

    // --- Execute Command ---
    debug!("Executing command: {:?}", cli.command);
    let result = cli::commands::execute_command(cli.command, db.clone());

    // --- Handle Result ---
    if let Err(e) = result {
        error!("Command execution failed: {}", e);
        // Print user-friendly error message (execute_command should ideally return specific errors)
        eprintln!("Error: {}", e);
        // Propagate error for non-zero exit code
        return Err(e);
    } else {
         debug!("Command executed successfully.");
    }

    Ok(())
}
