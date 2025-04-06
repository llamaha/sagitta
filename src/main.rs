use clap::Parser;
use anyhow::Result;
use std::path::PathBuf;
use dirs::data_local_dir;
use log::{debug, info, warn, error};
use env_logger;

mod cli;
mod vectordb;
mod utils;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: cli::commands::Command,
}

fn main() -> Result<()> {
    // Initialize the logger
    env_logger::init();
    
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
    let mut db = vectordb::VectorDB::new(db_path)?;
    
    // Start auto-sync daemon if any repositories have auto-sync enabled
    if matches!(cli.command, cli::commands::Command::Repo { command: cli::commands::RepoCommand::AutoSync { command: cli::commands::AutoSyncCommand::Start } }) {
        // Don't start daemon if the command is already to start the daemon
        // to avoid conflicts
    } else if matches!(cli.command, cli::commands::Command::Repo { command: cli::commands::RepoCommand::AutoSync { command: cli::commands::AutoSyncCommand::Stop } }) {
        // Don't start daemon if the command is to stop the daemon
    } else {
        // Start auto-sync daemon if there are repositories with auto-sync enabled
        if !db.repo_manager.get_auto_sync_repos().is_empty() {
            debug!("Starting auto-sync daemon");
            if let Err(e) = db.start_auto_sync() {
                error!("Failed to start auto-sync daemon: {}", e);
            }
        }
    }
    
    // Execute the command
    let result = cli::commands::execute_command(cli.command, db.clone());
    
    // Clean up
    if !matches!(result, Ok(())) {
        // If there was an error, ensure we stop the auto-sync daemon
        if let Err(e) = db.stop_auto_sync() {
            error!("Failed to stop auto-sync daemon during cleanup: {}", e);
        }
    }
    
    result
}
