use anyhow::{Context, Result};
use clap::Args;
use colored::*;
use qdrant_client::{ Qdrant, qdrant::{ScrollPointsBuilder, PayloadIncludeSelector, PointId} }; // Removed with_payload_selector
use std::collections::HashSet;
use std::sync::Arc;
use crate::config::AppConfig;
use crate::cli::repo_commands::get_collection_name;

// use super::commands::{CODE_SEARCH_COLLECTION, FIELD_DIR_PATH}; // REMOVED

#[derive(Args, Debug)]
pub struct ListArgs {
    // No arguments needed for list currently
}

/// Handles the `list` command, retrieving and displaying unique indexed directories from Qdrant.
pub async fn handle_list(
    _args: ListArgs, // Args unused for now
    config: AppConfig, // Take ownership
    client: Arc<Qdrant>, // Accept client
) -> Result<()> {
    // --- Get Active Repository and Collection --- 
    let active_repo_name = config.active_repository.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No active repository set. Use 'repo use <repo_name>' first.")
    })?;
    let collection_name = get_collection_name(active_repo_name);
    log::info!("Listing indexed items for repository: '{}', collection: '{}'", active_repo_name, collection_name);

    // --- Check if Collection Exists --- 
    if !client.collection_exists(&collection_name).await? {
        println!("Repository '{}' (collection '{}') has not been indexed yet.", active_repo_name, collection_name);
        return Ok(());
    }

    // --- Scroll Points to Get Unique FIELD_FILE_PATH --- 
    // (This replaces the old FIELD_DIR_PATH logic)
    let mut unique_files = HashSet::new();
    let mut next_offset: Option<PointId> = None;
    const SCROLL_LIMIT: u32 = 256;

    println!("Indexed files in repository '{}':", active_repo_name.cyan());

    loop {
        // Define the PayloadIncludeSelector directly
        let payload_selector = PayloadIncludeSelector {
            fields: vec![super::commands::FIELD_FILE_PATH.to_string()],
        };

        let mut builder = ScrollPointsBuilder::new(&collection_name)
            .limit(SCROLL_LIMIT)
            .with_payload(payload_selector) // Pass PayloadIncludeSelector
            .with_vectors(false);
        
        if let Some(offset_value) = next_offset.clone() {
            builder = builder.offset(offset_value);
        }

        let scroll_request = builder.build();
        
        let scroll_response = client.scroll(scroll_request).await
            .with_context(|| format!("Failed to scroll points in collection '{}'", collection_name))?;

        for point in scroll_response.result {
            // Access payload map directly (it's not Option)
            let payload_map = point.payload; 
            if let Some(file_path_value) = payload_map.get(super::commands::FIELD_FILE_PATH) {
                if let Some(file_path) = file_path_value.as_str() {
                    if unique_files.insert(file_path.to_string()) {
                        println!("  - {}", file_path);
                    }
                }
            }
        }

        if let Some(next_page_offset) = scroll_response.next_page_offset {
            next_offset = Some(next_page_offset);
        } else {
            break; // No more pages
        }
    }

     if unique_files.is_empty() {
         println!("  (No files found in index for this repository)");
     }

    Ok(())
} 