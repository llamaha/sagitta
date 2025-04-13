use anyhow::{Context, Result};
use clap::Args;
use qdrant_client::{
    Qdrant,
    qdrant::{
        ScrollPointsBuilder,
        ScrollPoints, PayloadIncludeSelector,
    },
};
use std::collections::HashSet;

use super::commands::{CODE_SEARCH_COLLECTION, FIELD_DIR_PATH};

#[derive(Args, Debug)]
pub struct ListArgs {
    // No arguments needed for list currently
}

/// Handles the `list` command, retrieving and displaying unique indexed directories from Qdrant.
pub async fn handle_list(args: ListArgs, qdrant_url: &str) -> Result<()> {
    log::info!("Starting list process...");
    log::debug!("ListArgs: {:?}", args);

    log::info!("Connecting to Qdrant at {}", qdrant_url);
    let client = Qdrant::from_url(qdrant_url).build()
        .context("Failed to connect to Qdrant")?;
    log::info!("Qdrant client connected.");

    log::info!("Fetching distinct indexed directories from '{}'...", CODE_SEARCH_COLLECTION);

    let mut distinct_dirs = HashSet::new();
    let mut offset: Option<qdrant_client::qdrant::PointId> = None;
    let batch_size: u32 = 100; // Process in batches
    let payload_selector = PayloadIncludeSelector {
        fields: vec![FIELD_DIR_PATH.to_string()],
    };

    loop {
        let mut scroll_builder = ScrollPointsBuilder::new(CODE_SEARCH_COLLECTION)
            .limit(batch_size)
            // Correct way to select specific payload fields
            .with_payload(payload_selector.clone()); 
            
        // Only add offset if it's Some
        if let Some(current_offset) = offset {
            scroll_builder = scroll_builder.offset(current_offset);
        }
        
        let scroll_request: ScrollPoints = scroll_builder.build();

        log::trace!("Scrolling points with offset: {:?}", scroll_request.offset);
        let scroll_response = client.scroll(scroll_request).await
            .context("Failed to scroll points for listing directories")?;

        let points = scroll_response.result;
        let next_page_offset = scroll_response.next_page_offset;

        if points.is_empty() {
            log::debug!("Scroll returned empty result, finishing list.");
            break; // No more points
        }

        log::debug!("Processing batch of {} points.", points.len());
        for point in points {
            if let Some(dir_path_value) = point.payload.get(FIELD_DIR_PATH) {
                if let Some(dir_path_str) = dir_path_value.as_str() {
                    distinct_dirs.insert(dir_path_str.to_string());
                } else {
                    log::warn!("Found point ID {} with non-string dir_path payload: {:?}", point.id.map(|id| format!("{:?}", id)).unwrap_or_else(|| "N/A".into()), dir_path_value);
                }
            } else {
                 log::warn!("Found point ID {} without dir_path payload.", point.id.map(|id| format!("{:?}", id)).unwrap_or_else(|| "N/A".into()));
            }
        }

        offset = next_page_offset;
        if offset.is_none() {
            log::debug!("Scroll returned no next page offset, finishing list.");
            break; // Reached the end
        }
    }

    if distinct_dirs.is_empty() {
        println!("No indexed directories found in collection '{}'.", CODE_SEARCH_COLLECTION);
    } else {
        println!("Indexed Directories:");
        // Sort for consistent output
        let mut sorted_dirs: Vec<String> = distinct_dirs.into_iter().collect();
        sorted_dirs.sort(); 
        for dir in sorted_dirs {
            println!("- {}", dir);
        }
    }

    log::info!("List process finished successfully.");
    Ok(())
} 