//!
//! Top-level commands for the CLI application.

use anyhow::{Result, Context};
use clap::Args;
use colored::*;
use std::sync::Arc;
use crate::config::AppConfig;
use crate::cli::repo_commands::helpers::get_collection_name;
use crate::vectordb::qdrant_client_trait::QdrantClientTrait;
use std::path::PathBuf;
use qdrant_client::qdrant::CountPointsBuilder;

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
}

// Accept qdrant_url as parameter
/// Handles the `stats` command, fetching and displaying collection statistics from Qdrant.
pub async fn handle_stats<C>(
    _args: StatsArgs, 
    config: AppConfig,
    _client: Arc<C>, // Use generic trait
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    // --- Get Active Repository and Collection --- 
    let active_repo_name = config.active_repository.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No active repository set. Use 'repo use <repo_name>' first.")
    })?;
    let collection_name = get_collection_name(active_repo_name);
    log::info!("Getting stats for repository: '{}', collection: '{}'", active_repo_name, collection_name);

    println!("Fetching statistics for collection: {}", collection_name.cyan());

    // Uncomment the actual calls
    // Fetch collection info
    let collection_info_result = _client.get_collection_info(collection_name.clone()).await
        .context(format!("Failed to get info for collection '{}'", collection_name));
    
    if collection_info_result.is_err() {
        println!("{}", "  Error: Could not retrieve collection info (collection might not exist yet). Run 'repo sync'?".red());
        return Ok(());
    }
    let collection_info = collection_info_result.unwrap(); // Safe to unwrap after check
    
    // Use collection_info directly, it doesn't have a nested .result field
    let info = collection_info;
    
    // Fetch exact point count (count is more accurate than info.points_count)
    let count_request = CountPointsBuilder::new(&collection_name).exact(true).build();
    let count_result = _client.count(count_request).await
        .context(format!("Failed to count points in collection '{}'", collection_name))?;
    
    // Access count via result field
    let exact_count = count_result.result.map(|r| r.count).unwrap_or(0);

    // Assign real values from fetched info
    let info_status = info.status.to_string(); // Assuming status is an enum or similar
    let vectors_count = info.vectors_count.unwrap_or(0); 
    let segments_count = info.segments_count;
    // Comment out fields that might not exist
    // let disk_data_size = info.disk_data_size.unwrap_or(0);
    // let ram_data_size = info.ram_data_size.unwrap_or(0);

    println!("  Status: {}", info_status.green());
    println!("  Point count: {}", exact_count.to_string().yellow());
    println!("  Vector count: {}", vectors_count.to_string().yellow());
    println!("  Segments: {}", segments_count.to_string().yellow());
    // Comment out printing for potentially non-existent fields
    // println!("  Disk usage: {} MB", disk_data_size / (1024 * 1024));
    // println!("  RAM usage: {} MB", ram_data_size / (1024 * 1024));

    // TODO: Add details about indexed branches/commits if possible from config
    
    Ok(())
} 