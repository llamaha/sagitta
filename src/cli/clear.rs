use anyhow::{Context, Result};
use clap::Args;
use colored::*;
use qdrant_client::Qdrant;
use std::io::{self, Write}; // Import io for confirmation prompt
use std::sync::Arc;
use crate::config::AppConfig;
use crate::cli::repo_commands::get_collection_name;

#[derive(Args, Debug)]
pub struct ClearArgs {
    /// Confirm deletion without prompting.
    #[arg(short, long)]
    yes: bool,
    // Removed: directory: Option<String>,
    // Removed: all: bool,
}

pub async fn handle_clear(
    args: ClearArgs,
    config: AppConfig, // Take ownership
    client: Arc<Qdrant>, // Accept client
) -> Result<()> {
    // --- Get Active Repository and Collection --- 
    let active_repo_name = config.active_repository.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No active repository set. Use 'repo use <repo_name>' first.")
    })?;
    let collection_name = get_collection_name(active_repo_name);
    log::info!("Preparing to clear data for repository: '{}', collection: '{}'", active_repo_name, collection_name);

    // --- Confirmation --- 
    if !args.yes {
        print!(
            "{}",
            format!(
                "Are you sure you want to delete ALL indexed data for repository '{}' (collection '{}')? [y/N]: ",
                active_repo_name.yellow().bold(),
                collection_name.yellow().bold()
            )
            .red()
        );
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut confirmation = String::new();
        io::stdin()
            .read_line(&mut confirmation)
            .context("Failed to read confirmation line")?;

        if confirmation.trim().to_lowercase() != "y" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    // --- Delete Collection --- 
    log::info!("Attempting to delete collection '{}'...", collection_name);
    println!("Deleting collection '{}'...", collection_name);

    match client.delete_collection(&collection_name).await {
        Ok(op_result) => {
            if op_result.result {
                println!(
                    "{}",
                    format!("Successfully deleted collection '{}'.", collection_name).green()
                );
                 log::info!("Collection '{}' deleted successfully.", collection_name);
            } else {
                 println!(
                     "{}",
                     format!("Collection '{}' might not have existed or deletion failed server-side.", collection_name).yellow()
                 );
                 log::warn!("Delete operation for collection '{}' returned false.", collection_name);
            }
        }
        Err(e) => {
             // Check if it's a "not found" type error - treat as success in clearing
             if e.to_string().contains("Not found") || e.to_string().contains("doesn't exist") {
                 println!(
                     "{}",
                     format!("Collection '{}' did not exist.", collection_name).yellow()
                 );
                 log::warn!("Collection '{}' not found during delete attempt.", collection_name);
             } else {
                 // For other errors, report them
                 eprintln!(
                     "{}",
                     format!("Failed to delete collection '{}': {}", collection_name, e).red()
                 );
                 return Err(e).context(format!("Failed to delete collection '{}'", collection_name));
             }
        }
    }

    Ok(())
} 