use anyhow::{Context, Result};
use clap::Args;
use colored::*;
use qdrant_client::Qdrant;
use std::io::{self, Write}; // Import io for confirmation prompt
use std::sync::Arc;
use crate::cli::commands::SIMPLE_INDEX_COLLECTION; // Import the renamed constant

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
    // config: AppConfig, // No longer needed
    client: Arc<Qdrant>, // Accept client
) -> Result<()> {
    // --- Target the Simple Collection Directly --- 
    let collection_name = SIMPLE_INDEX_COLLECTION;
    log::info!("Preparing to clear the simple index collection: '{}'", collection_name);

    // --- Confirmation --- 
    if !args.yes {
        print!(
            "{}",
            format!(
                "Are you sure you want to delete the simple index (collection '{}')? [y/N]: ",
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

    match client.delete_collection(collection_name).await {
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

#[cfg(test)]
mod tests {
    use super::*;
    use qdrant_client::Qdrant;
    use std::sync::Arc;
    use tokio::runtime::Runtime;
    // Mock Qdrant client setup would be needed here

    #[test]
    #[ignore] // Ignored because it requires a running Qdrant instance
    fn test_handle_clear_simple_index() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // --- Setup Mock Client ---
            // let mock_client = Qdrant::from_url("http://localhost:6334").build().unwrap(); // Replace with actual mock setup
            // For now, assume a dummy client that succeeds
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); // Placeholder

            // --- Prepare Args ---
            let args = ClearArgs { yes: true };

            // --- Expected Call --- 
            // Mock expectation: client.delete_collection(SIMPLE_INDEX_COLLECTION) called once
            // For simplicity, just run the handler and check Ok result

            // --- Execute --- 
            let result = handle_clear(args, client).await;

            // --- Assert --- 
            assert!(result.is_ok());
            // In a real test with mocks, verify delete_collection was called with SIMPLE_INDEX_COLLECTION
        });
    }
} 