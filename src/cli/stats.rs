use anyhow::{Context, Result};
use clap::Args;
use qdrant_client::Qdrant;
use std::sync::Arc;
use crate::config::AppConfig;
use crate::cli::repo_commands::get_collection_name;
use colored::Colorize;

// use super::commands::CODE_SEARCH_COLLECTION; // REMOVED

#[derive(Args, Debug)]
pub struct StatsArgs {
    // Removed Qdrant URL arg
    // /// Qdrant instance URL
    // #[arg(
    //     long,
    //     env = "QDRANT_URL",
    //     default_value = "http://localhost:6334"
    // )]
    // pub qdrant_url: String,
}

// Accept qdrant_url as parameter
/// Handles the `stats` command, fetching and displaying collection statistics from Qdrant.
pub async fn handle_stats(
    _args: StatsArgs, // Args currently unused
    config: AppConfig, // Take ownership
    client: Arc<Qdrant>, // Accept client
) -> Result<()> {
    // --- Get Active Repository and Collection --- 
    let active_repo_name = config.active_repository.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No active repository set. Use 'repo use <repo_name>' first.")
    })?;
    // No need to find repo_config unless we need specific details from it later
    let collection_name = get_collection_name(active_repo_name);
    log::info!("Getting stats for repository: '{}', collection: '{}'", active_repo_name, collection_name);

    // --- Get Collection Info --- 
    let collection_info = client
        .collection_info(&collection_name)
        .await
        .with_context(|| format!("Failed to get collection info for '{}'", collection_name))?;

    if let Some(info) = collection_info.result {
        println!("Collection: {}", collection_name.bold());
        println!("  Status: {:?}", info.status());
        println!("  Points Count: {}", info.points_count.unwrap_or(0));
        println!("  Segments Count: {}", info.segments_count);
        println!("  Vectors Count: {}", info.vectors_count.unwrap_or(0));
        println!("  Indexed Vectors Count: {}", info.indexed_vectors_count.unwrap_or(0));

        if let Some(config) = info.config {
            println!("  Configuration:");
            if let Some(params) = config.params {
                if let Some(vectors_config) = params.vectors_config {
                    if let Some(vector_params_map) = vectors_config.config {
                        match vector_params_map {
                            qdrant_client::qdrant::vectors_config::Config::Params(p) => {
                                println!("    Vector Params:");
                                println!("      Size: {}", p.size);
                                println!("      Distance: {:?}", p.distance());
                            }
                            qdrant_client::qdrant::vectors_config::Config::ParamsMap(map) => {
                                println!("    Vector Params Map:");
                                for (name, p) in map.map {
                                     println!("      - Name: {}, Size: {}, Distance: {:?}", name, p.size, p.distance());
                                }
                            }
                        }
                    }
                }
            }
            if let Some(hnsw_config) = config.hnsw_config {
                println!("    HNSW Config:");
                println!("      m: {:?}", hnsw_config.m);
                println!("      ef_construct: {:?}", hnsw_config.ef_construct);
            }
             if let Some(optimizer_config) = config.optimizer_config {
                println!("    Optimizer Config:");
                 println!("      deleted_threshold: {:?}", optimizer_config.deleted_threshold);
                 // Add other optimizer params if needed
             }
        }

        // Iterate directly over payload_schema (it's HashMap, not Option<HashMap>)
        println!("  Payload Schema:");
        // Check if the schema map exists and is not empty
        if !info.payload_schema.is_empty() { 
             for (field, schema) in info.payload_schema {
                 println!("    - {}: {:?}", field, schema.data_type());
             }
        } else {
            println!("    (Schema not defined or empty)");
        }
    } else {
        println!("Collection '{}' not found or info unavailable.", collection_name);
    }

    Ok(())
} 