use clap::Parser;
use anyhow::Result;
use std::path::PathBuf;
use dirs::data_local_dir;

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
    let cli = Cli::parse();
    
    // Get the database path
    let db_path = data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vectordb-cli")
        .join("db.json")
        .to_string_lossy()
        .to_string();
    
    // Create or load the database
    let db = vectordb::VectorDB::new(db_path)?;
    
    // Execute the command
    cli::commands::execute_command(cli.command, db)?;
    
    Ok(())
}
