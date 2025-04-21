//!
//! Top-level commands for the CLI application.

use anyhow::Result;
use clap::Args;
use colored::*;
use std::sync::Arc;
use crate::config::AppConfig;
use crate::cli::repo_commands::helpers::get_collection_name;
use crate::vectordb::qdrant_client_trait::QdrantClientTrait;
use std::path::PathBuf;
use qdrant_client::qdrant::CountPointsBuilder;
use serde::Serialize;
use serde_json;

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
            let err_msg = "No active repository set. Use 'repo use <repo_name>' first.";
            if args.json {
                let stats = CollectionStats {
                    collection_name: "<N/A>".to_string(),
                    repository_name: "<N/A>".to_string(),
                    info: None,
                    exact_point_count: None,
                    error: Some(err_msg.to_string()),
                };
                // Print error JSON to stdout as per convention?
                println!("{}", serde_json::to_string_pretty(&stats)?);
                // Still return an error to signal failure
                return Err(anyhow::anyhow!(err_msg)); 
            } else {
                // Print error message to stderr for non-JSON output
                eprintln!("{}", err_msg.red());
                 // Return the error
                return Err(anyhow::anyhow!(err_msg));
            }
        }
    };

    let collection_name = get_collection_name(&active_repo_name);
    log::info!("Getting stats for repository: '{}', collection: '{}'", active_repo_name, collection_name);

    // Fetch collection info
    let collection_info_result = client.get_collection_info(collection_name.clone()).await;
    let count_result_opt = if collection_info_result.is_ok() {
        let count_request = CountPointsBuilder::new(&collection_name).exact(true).build();
        client.count(count_request).await.ok()
    } else {
        None
    };

    if args.json {
        // Extract and convert the info
        let info = collection_info_result.as_ref().ok().map(|info| CollectionInfoStats {
            status: info.status.to_string(),
            vectors_count: info.vectors_count.unwrap_or(0),
            segments_count: info.segments_count,
        });
        
        let exact_count = count_result_opt.as_ref().and_then(|r| r.result.as_ref().map(|c| c.count));
        let error_msg = collection_info_result.err().map(|e| e.to_string());

        let stats = CollectionStats {
            collection_name: collection_name.clone(),
            repository_name: active_repo_name.clone(),
            info,
            exact_point_count: exact_count,
            error: error_msg,
        };
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        // Original human-readable output
        println!("Fetching statistics for collection: {}", collection_name.cyan());

        if collection_info_result.is_err() {
            println!("{}", "  Error: Could not retrieve collection info (collection might not exist yet). Run 'repo sync'?".red());
            return Ok(());
        }
        let collection_info = collection_info_result.unwrap(); // Safe to unwrap after check
        let info = collection_info;

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