// Qdrant-related helper functions from repo_helpers.rs will be moved here. 

use std::path::PathBuf;
use qdrant_client::qdrant::{DeletePointsBuilder, PointStruct, Filter, Condition, 
    ScrollPointsBuilder, ScrollResponse, PayloadIncludeSelector, UpsertPointsBuilder, 
    PointId, PointsSelector, points_selector::PointsSelectorOneOf, PointsIdsList};
use indicatif::ProgressBar;
use colored::Colorize;
use anyhow::{Context, Result};
use crate::constants::{BATCH_SIZE, FIELD_BRANCH, FIELD_FILE_PATH};
use crate::error::VectorDBError as Error;
use crate::QdrantClientTrait;
use crate::config::AppConfig;

pub fn get_collection_name(repo_name: &str, config: &AppConfig) -> String {
    format!("{}{}", config.performance.collection_name_prefix, repo_name)
}

pub async fn delete_points_for_files<
    C: QdrantClientTrait + Send + Sync + 'static,
>(
    client: &C,
    collection_name: &str,
    branch_name: &str,
    relative_paths: &[PathBuf],
) -> Result<(), Error> {
    if relative_paths.is_empty() {
        log::debug!("No files provided for deletion in branch '{}'.", branch_name);
        return Ok(());
    }
    log::info!("Deleting points for {} files in branch '{}' from collection '{}'...",
        relative_paths.len(), branch_name, collection_name);
    let mut point_ids_to_delete: Vec<PointId> = Vec::new();
    let filter = Filter::must([
        Condition::matches(FIELD_BRANCH, branch_name.to_string()),
        Filter {
            should: 
                relative_paths.iter().map(|p| {
                    Condition::matches(FIELD_FILE_PATH, p.to_string_lossy().into_owned())
                }).collect::<Vec<_>>(),
            min_should: None,
            must: Vec::new(),
            must_not: Vec::new(),
        }.into()
    ]);
    let mut offset: Option<PointId> = None;
    loop {
        let mut builder = ScrollPointsBuilder::new(collection_name)
            .filter(filter.clone())
            .limit(1000)
            .with_payload(false)
            .with_vectors(false);

        if let Some(o) = offset {
            builder = builder.offset(o);
        }
        
        let scroll_request = builder.into();
        let scroll_result: ScrollResponse = client.scroll(scroll_request).await
            .with_context(|| format!("Failed to scroll points for deletion in collection '{}'", collection_name))?;
        if scroll_result.result.is_empty() {
            break;
        }
        for point in scroll_result.result {
            if let Some(id) = point.id {
                 point_ids_to_delete.push(id);
            } else {
                log::warn!("Found point without ID during scroll for deletion: {:?}", point);
            }
        }
        offset = scroll_result.next_page_offset;
        if offset.is_none() {
            break;
        }
    }
    if point_ids_to_delete.is_empty() {
        log::info!("No points found matching the files to be deleted in branch '{}'.", branch_name);
        return Ok(());
    }
    log::debug!("Found {} points to delete for branch '{}'.", point_ids_to_delete.len(), branch_name);
    for chunk in point_ids_to_delete.chunks(BATCH_SIZE) {
         let _points_selector = PointsSelector {
             points_selector_one_of: Some(PointsSelectorOneOf::Points(
                 PointsIdsList { ids: chunk.to_vec() }
             ))
         };
         let delete_request = DeletePointsBuilder::new(collection_name)
            .points(chunk.to_vec());
         client.delete_points(delete_request.into()).await
             .with_context(|| format!("Failed to delete a batch of points from collection '{}'", collection_name))?;
        log::debug!("Deleted batch of {} points for branch '{}'.", chunk.len(), branch_name);
    }
    log::info!("Successfully deleted {} points for {} files in branch '{}'.",
        point_ids_to_delete.len(), relative_paths.len(), branch_name);
    Ok(())
}

pub async fn ensure_repository_collection_exists<C>(
    client: &C,
    collection_name: &str,
    vector_dim: u64,
) -> Result<(), Error>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    match client.collection_exists(collection_name.to_string()).await {
        Ok(exists) => {
            if exists {
                log::debug!("Collection '{}' already exists.", collection_name);
                // TODO: Optionally verify existing collection parameters?
                Ok(())
            } else {
                log::info!("Collection '{}' does not exist. Creating...", collection_name);
                // Create collection using the trait method
                client.create_collection(collection_name, vector_dim).await
                    .map_err(|e| Error::Other(format!("Failed to create collection '{}': {}", collection_name, e.to_string())))?;
                log::info!(
                    "Created Qdrant collection '{}' with dimension {}.",
                    collection_name,
                    vector_dim
                );
                Ok(())
            }
        }
        Err(e) => {
            log::error!("Failed to check or create collection '{}': {}", collection_name, e);
            Err(Error::Other(format!("Failed to ensure collection '{}' exists: {}", collection_name, e)))
        }
    }
}

pub fn create_branch_filter(branch_name: &str) -> Filter {
    Filter::must([
        Condition::matches(FIELD_BRANCH, branch_name.to_string()),
    ])
}

pub async fn delete_points_by_branch(
    client: &impl QdrantClientTrait,
    collection_name: &str,
    branch: &str,
    config: &AppConfig,
) -> Result<()> {
    let filter = Filter::must([
        Condition::matches(FIELD_BRANCH, branch.to_string()),
    ]);

    let mut point_ids_to_delete: Vec<PointId> = Vec::new();
    let mut offset: Option<PointId> = None;

    // Scroll through all points for this branch
    loop {
        let mut builder = ScrollPointsBuilder::new(collection_name)
            .filter(filter.clone())
            .limit(1000)
            .with_payload(false)
            .with_vectors(false);

        if let Some(o) = offset {
            builder = builder.offset(o);
        }
        
        let scroll_request = builder.into();
        let scroll_result: ScrollResponse = client.scroll(scroll_request).await
            .with_context(|| format!("Failed to scroll points for deletion in collection '{}'", collection_name))?;
        
        if scroll_result.result.is_empty() {
            break;
        }

        for point in scroll_result.result {
            if let Some(id) = point.id {
                point_ids_to_delete.push(id);
            } else {
                log::warn!("Found point without ID during scroll for deletion: {:?}", point);
            }
        }

        offset = scroll_result.next_page_offset;
        if offset.is_none() {
            break;
        }
    }

    if point_ids_to_delete.is_empty() {
        log::info!("No points found for branch '{}' in collection '{}'.", branch, collection_name);
        return Ok(());
    }

    log::debug!("Found {} points to delete for branch '{}'.", point_ids_to_delete.len(), branch);

    // Delete points in chunks
    for chunk in point_ids_to_delete.chunks(config.performance.batch_size) {
        let delete_request = DeletePointsBuilder::new(collection_name)
            .points(chunk.to_vec());
        client.delete_points(delete_request.into()).await
            .with_context(|| format!("Failed to delete a batch of points from collection '{}'", collection_name))?;
        log::debug!("Deleted batch of {} points for branch '{}'.", chunk.len(), branch);
    }

    log::info!("Successfully deleted {} points for branch '{}' in collection '{}'.",
        point_ids_to_delete.len(), branch, collection_name);
    Ok(())
} 