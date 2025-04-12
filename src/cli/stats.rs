use anyhow::{Context, Result};
use clap::Args;
use qdrant_client::Qdrant;

use super::commands::CODE_SEARCH_COLLECTION;

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
pub async fn handle_stats(args: StatsArgs, qdrant_url: &str) -> Result<()> {
    log::info!("Starting stats process...");
    log::debug!("StatsArgs: {:?}", args);

    log::info!("Connecting to Qdrant at {}", qdrant_url);
    let client = Qdrant::from_url(qdrant_url).build()
        .context("Failed to connect to Qdrant")?;
    log::info!("Qdrant client connected.");

    log::info!("Fetching collection info for '{}'...", CODE_SEARCH_COLLECTION);

    match client.collection_info(CODE_SEARCH_COLLECTION).await {
        Ok(info_response) => {
            let info = info_response.result.context("Collection info result was empty")?;
            println!("Collection Status: {:?}", info.status);
            println!("Points Count: {}", info.points_count.unwrap_or(0));
            println!("Vectors Count: {}", info.vectors_count.unwrap_or(0));
            println!("Segments Count: {}", info.segments_count);
            
            if let Some(config) = info.config {
                if let Some(params) = config.params {
                     if let Some(vector_params) = params.vectors_config.and_then(|vc| vc.config) {
                        match vector_params {
                           qdrant_client::qdrant::vectors_config::Config::Params(p) => {
                               println!("Vector Dimension: {}", p.size);
                               println!("Vector Distance Metric: {:?}", qdrant_client::qdrant::Distance::try_from(p.distance).unwrap_or(qdrant_client::qdrant::Distance::UnknownDistance));
                           },
                           qdrant_client::qdrant::vectors_config::Config::ParamsMap(map_config) => {
                                // Handle map config if necessary, print basic info for now
                                println!("Vector Config: Using named vectors");
                                for (name, params) in map_config.map {
                                    println!("  - Name: {}, Dimension: {}, Distance: {:?}", 
                                        name, 
                                        params.size, 
                                        qdrant_client::qdrant::Distance::try_from(params.distance).unwrap_or(qdrant_client::qdrant::Distance::UnknownDistance)
                                    );
                                }
                            }
                        }
                    }
                    // Optionally print HNSW, Quantization configs if needed
                    // println!("HNSW Config: {:?}", params.hnsw_config);
                    // println!("Quantization Config: {:?}", params.quantization_config);
                    // println!("Optimizer Config: {:?}", config.optimizer_config);
                }
            }
             println!("Payload Schema: {:?}", info.payload_schema);
        }
        Err(e) => {
            // Check if the error indicates the collection doesn't exist
            if e.to_string().contains("Not found") { // Basic check, might need refinement
                log::warn!("Collection '{}' not found.", CODE_SEARCH_COLLECTION);
                println!("Collection '{}' does not exist.", CODE_SEARCH_COLLECTION);
                // Return Ok, as the command succeeded in determining the state (non-existent)
                return Ok(()); 
            } else {
                 log::error!("Failed to get collection info: {}", e);
                 // Propagate other errors
                 return Err(e).context(format!("Failed to get info for collection '{}'", CODE_SEARCH_COLLECTION)); 
            }
        }
    }

    log::info!("Stats process finished successfully.");
    Ok(())
} 