use anyhow::{anyhow, Context, Result};
use clap::Args;
use qdrant_client::{
    Qdrant,
    qdrant::{
        Filter, Condition,
        DeletePointsBuilder,
    },
};
use std::{fs, path::PathBuf, io::{stdin, stdout, Write}};

use super::commands::{CODE_SEARCH_COLLECTION, FIELD_DIR_PATH};

#[derive(Args, Debug)]
#[clap(group = clap::ArgGroup::new("scope").required(true).args(["all", "directory"]))]
pub struct ClearArgs {
    /// Clear all indexed data by deleting the entire collection.
    #[arg(long, short = 'a', group = "scope")]
    pub all: bool,

    /// Clear indexed data associated with a specific source directory.
    #[arg(long, short = 'd', value_name = "PATH", group = "scope")]
    pub directory: Option<PathBuf>,

    /// Skip confirmation prompt.
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Handles the `clear` command, deleting the collection or specific points from Qdrant.
pub async fn handle_clear(args: ClearArgs, qdrant_url: &str) -> Result<()> {
    log::info!("Starting clear process...");
    log::debug!("ClearArgs: {:?}", args);

    // --- Confirmation --- 
    if !args.yes {
        let scope_msg = if args.all {
            format!("delete the ENTIRE collection '{}'", CODE_SEARCH_COLLECTION)
        } else if let Some(dir) = &args.directory {
            format!("remove all indexed data associated with the directory '{}'", dir.display())
        } else {
            // This case should be unreachable due to clap group requirement
            return Err(anyhow!("Invalid clear scope: neither --all nor --directory specified."));
        };

        print!("WARNING: This will {}. This action cannot be undone. Proceed? (y/N): ", scope_msg);
        stdout().flush()?; // Ensure prompt is displayed before input
        let mut confirmation = String::new();
        stdin().read_line(&mut confirmation)?;
        if confirmation.trim().to_lowercase() != "y" {
            println!("Clear operation cancelled.");
            return Ok(());
        }
    }

    // --- Connect to Qdrant --- 
    log::info!("Connecting to Qdrant at {}", qdrant_url);
    let client = Qdrant::from_url(qdrant_url).build()
        .context("Failed to connect to Qdrant")?;
    log::info!("Qdrant client connected.");

    // --- Execute Clear Operation --- 
    if args.all {
        log::warn!("Deleting entire collection: {}", CODE_SEARCH_COLLECTION);
        match client.delete_collection(CODE_SEARCH_COLLECTION).await {
            Ok(_) => {
                println!("Successfully deleted collection '{}'.", CODE_SEARCH_COLLECTION);
                log::info!("Collection '{}' deleted.", CODE_SEARCH_COLLECTION);
            }
            Err(e) => {
                 if e.to_string().contains("Not found") { // Handle case where collection doesn't exist
                     println!("Collection '{}' does not exist, nothing to clear.", CODE_SEARCH_COLLECTION);
                     log::warn!("Attempted to clear non-existent collection '{}'.", CODE_SEARCH_COLLECTION);
                 } else {
                    log::error!("Failed to delete collection '{}': {}", CODE_SEARCH_COLLECTION, e);
                    return Err(e).context(format!("Failed to delete collection '{}'", CODE_SEARCH_COLLECTION));
                 }
            }
        }
    } else if let Some(dir) = args.directory {
        log::info!("Clearing data for directory: {:?}", dir);
        let canonical_dir = fs::canonicalize(&dir)
            .with_context(|| format!("Failed to canonicalize directory path: {}", dir.display()))?;
        let canonical_dir_str = canonical_dir.to_string_lossy().to_string();
        log::info!("Canonical path for removal: {}", canonical_dir_str);

        let filter = Filter::must([
            Condition::matches(FIELD_DIR_PATH, canonical_dir_str)
        ]);

        let delete_request = DeletePointsBuilder::new(CODE_SEARCH_COLLECTION)
            .points(filter)
            .wait(true)
            .build();
            
        match client.delete_points(delete_request).await {
             Ok(response) => {
                if let Some(result) = response.result {
                     println!(
                        "Successfully removed points for directory '{}'. Status: {:?}", 
                        dir.display(), 
                        qdrant_client::qdrant::UpdateStatus::try_from(result.status).unwrap_or_default()
                    );
                    log::info!("Points removal for '{}' completed with status: {:?}", dir.display(), result.status);
                 } else {
                    println!("Points removal for directory '{}' finished, but status unknown.", dir.display());
                    log::warn!("Points removal for '{}' response missing result.", dir.display());
                 }
             }
             Err(e) => {
                 // Check if collection not found vs other errors
                if e.to_string().contains("Not found") {
                     println!("Collection '{}' does not exist, cannot remove directory data.", CODE_SEARCH_COLLECTION);
                     log::warn!("Attempted to remove directory from non-existent collection '{}'.", CODE_SEARCH_COLLECTION);
                } else {
                    log::error!("Failed to remove points for directory '{}': {}", dir.display(), e);
                    return Err(e).context(format!("Failed to remove points for directory '{}'", dir.display()));
                 }
            }
        }
    } else {
        unreachable!(); // Clap group should prevent this
    }

    log::info!("Clear process finished successfully.");
    Ok(())
} 