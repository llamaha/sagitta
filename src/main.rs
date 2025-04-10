#![allow(dead_code)]

use anyhow::Result;
use clap::Parser;
use dirs::data_local_dir;
use log::debug;
use std::path::PathBuf;
use tracing_subscriber;

mod cli;
mod utils;
mod vectordb;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: cli::commands::Command,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();
    // Initialize the logger

    let cli = Cli::parse();

    debug!("Initializing vectordb-cli with command: {:?}", cli.command);

    // Get the database path
    let db_path = data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vectordb-cli")
        .join("db.json")
        .to_string_lossy()
        .to_string();

    debug!("Using database path: {}", db_path);

    // Create or load the database
    let db = vectordb::VectorDB::new(db_path)?;

    // Execute the command
    let result = cli::commands::execute_command(cli.command, db.clone());

    // Clean up
    if !matches!(result, Ok(())) {
        // Handle cleanup on error if needed in the future
        debug!("Command execution resulted in an error: {:?}", result.as_ref().err());
    }

    result
}
