#![allow(dead_code)]

use anyhow::Result;
use clap::Parser;
use dirs::data_local_dir;
use log::debug;
use std::path::PathBuf;
use tracing_subscriber;
use std::fs;

mod cli;
mod utils;
mod vectordb;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: cli::commands::Command,

    /// Optional path to the database file (defaults to system's local data dir)
    #[arg(long = "db-path", global = true)]
    db_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();
    // Initialize the logger

    let cli = Cli::parse();

    debug!("Initializing vectordb-cli with command: {:?}", cli.command);

    // Get the database path
    let base_db_path = match cli.db_path {
        Some(path) => PathBuf::from(path),
        None => {
            // Default path logic
            let default_dir = data_local_dir()
                .unwrap_or_else(|| PathBuf::from(".")) // Fallback to current dir if data_local_dir fails
                .join("vectordb-cli");

            // Ensure the default directory exists
            fs::create_dir_all(&default_dir)?;
            default_dir.join("db.json")
        }
    };

    // Ensure the directory for the specified or default db file exists
    if let Some(parent_dir) = base_db_path.parent() {
         if !parent_dir.exists() { // Only create if it doesn't exist
              debug!("Creating database directory: {}", parent_dir.display());
              fs::create_dir_all(parent_dir)?;
         }
    }

    let db_path_str = base_db_path.to_string_lossy().to_string();
    debug!("Using database path: {}", db_path_str);

    // Create or load the database using the determined path
    let db = vectordb::VectorDB::new(db_path_str)?;

    // Execute the command
    let result = cli::commands::execute_command(cli.command, db.clone());

    // Clean up
    if !matches!(result, Ok(())) {
        // Handle cleanup on error if needed in the future
        debug!("Command execution resulted in an error: {:?}", result.as_ref().err());
    }

    result
}
