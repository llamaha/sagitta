//!
//! Top-level commands for the CLI application.

use anyhow::Result;
use clap::Args;
use colored::*;
use std::sync::Arc;
use vectordb_core::AppConfig;
use vectordb_core::repo_helpers::get_collection_name;
use qdrant_client::qdrant::qdrant_client::QdrantClient;
use vectordb_core::qdrant_client_trait::QdrantClientTrait;
use std::path::PathBuf;
use qdrant_client::qdrant::CountPointsBuilder;
use serde::Serialize;
use serde_json;
use crate::cli::commands::CliArgs;
use anyhow::Context;

// use super::commands::CODE_SEARCH_COLLECTION; // REMOVED

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct StatsArgs {
    /// Optional: Path to the configuration file.
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    pub config_file: Option<PathBuf>,
    // Removed unused optional repo name
    // /// Optional: Name of the repository to show stats for (defaults to active repository).
    // #[arg(long)]
    // pub repo_name: Option<String>,

    /// Output statistics in JSON format.
    #[arg(long)]
    pub json: bool,
}

// Define a simplified serializable struct for collection info
#[derive(Serialize)]
struct CollectionInfoStats {
    status: String,
    vectors_count: u64,
    segments_count: u64,
}

// Define a serializable struct for JSON output
#[derive(Serialize)]
struct CollectionStats {
    collection_name: String,
    repository_name: String,
    info: Option<CollectionInfoStats>,
    exact_point_count: Option<u64>,
    error: Option<String>,
}

// Accept qdrant_url as parameter
/// Handles the `stats` command, fetching and displaying collection statistics from Qdrant.
pub async fn handle_stats<C>(
    args: StatsArgs,
    config: AppConfig,
    client: Arc<C>,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let active_repo_name_opt = config.active_repository.clone(); // Clone to avoid borrowing issues
    let active_repo_name = match active_repo_name_opt {
        Some(name) => name,
        None => {
            if config.repositories.len() == 1 {
                // If only one repository exists, assume it's the target
                config.repositories[0].name.clone()
            } else {
                anyhow::bail!("No active repository set and multiple repositories exist. Please specify a repository with --name or use 'vectordb-cli repo use <repo_name>'.");
            }
        }
    };

    // Ensure active_repo_name is a String
    let active_repo_name_string = active_repo_name.to_string();

    println!("Fetching stats for repository: {}", active_repo_name_string.cyan());
    
    let collection_name = get_collection_name(&active_repo_name_string);

    // Fetch collection info
    let collection_info_result = client
        .get_collection_info(collection_name.clone())
        .await
        .context(format!("Failed to retrieve collection info for '{}'", collection_name));
    
    let count_result_opt = if collection_info_result.is_ok() {
        let count_request = CountPointsBuilder::new(&collection_name).exact(true).build();
        client.count(count_request).await.ok()
    } else {
        None
    };

    if args.json {
        // Extract and convert the info from the Result
        let info = collection_info_result.as_ref().ok().map(|info| CollectionInfoStats {
            status: info.status.to_string(),
            vectors_count: info.vectors_count.unwrap_or(0),
            segments_count: info.segments_count,
        });
        
        let exact_count = count_result_opt.as_ref().and_then(|r| r.result.as_ref().map(|c| c.count));
        let error_msg = collection_info_result.err().map(|e| e.to_string());

        let stats = CollectionStats {
            collection_name: collection_name.clone(),
            repository_name: active_repo_name_string.clone(),
            info,
            exact_point_count: exact_count,
            error: error_msg,
        };
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        // Original human-readable output
        println!("Fetching statistics for collection: {}", collection_name.cyan());

        // Check the Result for errors
        if let Err(e) = collection_info_result {
            println!("{}", "  Error: Could not retrieve collection info (collection might not exist yet). Run 'repo sync'?".red());
            // Propagate the error that occurred
            return Err(e.context(format!("Failed to get collection info for '{}'", collection_name)));
        }
        
        // Safe to unwrap the Result now because we checked for Err above
        let collection_info = collection_info_result.unwrap(); 
        let info = collection_info; // Use the unwrapped struct

        let exact_count = match count_result_opt {
            Some(count_result) => count_result.result.map(|r| r.count).unwrap_or(0),
            None => 0, // Or indicate error?
        };

        let info_status = info.status.to_string();
        let vectors_count = info.vectors_count.unwrap_or(0); 
        let segments_count = info.segments_count;

        println!("  Status: {}", info_status.green());
        println!("  Point count: {}", exact_count.to_string().yellow());
        println!("  Vector count: {}", vectors_count.to_string().yellow());
        println!("  Segments: {}", segments_count.to_string().yellow());
    }

    Ok(())
} 