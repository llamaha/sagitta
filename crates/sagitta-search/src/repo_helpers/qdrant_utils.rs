// Qdrant-related helper functions from repo_helpers.rs will be moved here. 

use std::path::PathBuf;
use qdrant_client::qdrant::{DeletePointsBuilder, Filter, Condition, 
    ScrollPointsBuilder, ScrollResponse, 
    PointId, PointsSelector, points_selector::PointsSelectorOneOf, PointsIdsList};
use anyhow::{Context, Result};
use crate::constants::{BATCH_SIZE, FIELD_BRANCH, FIELD_FILE_PATH};
use crate::error::SagittaError as Error;
use crate::QdrantClientTrait;
use crate::config::AppConfig;
use crate::sync_progress::{SyncProgressReporter, SyncStage, NoOpProgressReporter, SyncProgress};
use std::sync::Arc;
use sha2::{Sha256, Digest};

/// Generates the Qdrant collection name for a given repository name based on the config prefix.
/// This is the legacy function that doesn't include branch information.
pub fn get_collection_name(repo_name: &str, config: &AppConfig) -> String {
    format!("{}{}", config.performance.collection_name_prefix, repo_name)
}

/// Generates a branch-aware Qdrant collection name that includes branch/ref information.
/// This enables separate collections per branch to avoid unnecessary re-syncing.
/// 
/// Format: {prefix}{tenant_id}_{repo_name}_{branch_hash}
/// Where branch_hash is a short hash of the branch/ref name to keep collection names manageable.
pub fn get_branch_aware_collection_name(
    repo_name: &str, 
    branch_or_ref: &str, 
    config: &AppConfig
) -> String {
    // Create a short hash of the branch/ref name to keep collection names manageable
    // and handle special characters that might not be valid in collection names
    let mut hasher = Sha256::new();
    hasher.update(branch_or_ref.as_bytes());
    let hash = hasher.finalize();
    let branch_hash = format!("{:x}", hash)[..8].to_string(); // Use first 8 chars of hash
    
    format!("{}{}_br_{}", 
        config.performance.collection_name_prefix, 
        repo_name, 
        branch_hash
    )
}

/// Checks if a collection already exists for a specific repository + branch/ref combination.
/// This helps determine if we need to sync or can reuse existing indexed data.
pub async fn collection_exists_for_branch<C>(
    client: &C,
    repo_name: &str,
    branch_or_ref: &str,
    config: &AppConfig,
) -> Result<bool, Error>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let collection_name = get_branch_aware_collection_name(repo_name, branch_or_ref, config);
    client.collection_exists(collection_name).await
        .map_err(|e| Error::Other(format!("Failed to check collection existence: {}", e)))
}

/// Metadata about a repository's sync status for a specific branch/ref
#[derive(Debug, Clone)]
pub struct BranchSyncMetadata {
    pub collection_name: String,
    pub last_commit_hash: Option<String>,
    pub branch_or_ref: String,
    pub last_sync_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub files_count: usize,
}

/// Retrieves sync metadata for a specific repository branch/ref by checking the collection
/// and extracting metadata from stored points.
pub async fn get_branch_sync_metadata<C>(
    client: &C,
    repo_name: &str,
    branch_or_ref: &str,
    config: &AppConfig,
) -> Result<Option<BranchSyncMetadata>, Error>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let collection_name = get_branch_aware_collection_name(repo_name, branch_or_ref, config);
    
    // Check if collection exists
    let exists = client.collection_exists(collection_name.clone()).await
        .map_err(|e| Error::Other(format!("Failed to check collection existence: {}", e)))?;
    
    if !exists {
        return Ok(None);
    }
    
    // Get collection info to check if it has any points
    let collection_info = client.get_collection_info(collection_name.clone()).await
        .map_err(|e| Error::Other(format!("Failed to get collection info: {}", e)))?;
    
    let points_count = collection_info.points_count.unwrap_or(0) as usize;
    
    if points_count == 0 {
        return Ok(Some(BranchSyncMetadata {
            collection_name,
            last_commit_hash: None,
            branch_or_ref: branch_or_ref.to_string(),
            last_sync_timestamp: None,
            files_count: 0,
        }));
    }
    
    // TODO: In a future enhancement, we could store commit hash and timestamp 
    // as metadata in the collection or in specific points. For now, we just
    // return basic metadata indicating the collection exists and has content.
    
    Ok(Some(BranchSyncMetadata {
        collection_name,
        last_commit_hash: None, // Could be extracted from point metadata in future
        branch_or_ref: branch_or_ref.to_string(),
        last_sync_timestamp: None, // Could be extracted from collection metadata in future
        files_count: points_count,
    }))
}

/// Determines if a repository branch needs to be synced based on current commit vs last synced commit.
/// Returns true if sync is needed, false if the branch is already up to date.
pub fn should_sync_branch(
    current_commit_hash: &str,
    sync_metadata: Option<&BranchSyncMetadata>,
    force_sync: bool,
) -> bool {
    if force_sync {
        return true;
    }
    
    match sync_metadata {
        None => {
            // No existing collection/metadata, definitely need to sync
            true
        }
        Some(metadata) => {
            match &metadata.last_commit_hash {
                None => {
                    // Collection exists but no commit hash recorded, need to sync
                    true
                }
                Some(last_commit) => {
                    // Compare commit hashes
                    last_commit != current_commit_hash
                }
            }
        }
    }
}

/// Deletes points associated with specific file paths within a given branch from a Qdrant collection.
pub async fn delete_points_for_files<
    C: QdrantClientTrait + Send + Sync + 'static,
>(
    client: &C,
    collection_name: &str,
    branch_name: &str,
    relative_paths: &[PathBuf],
    progress_reporter: Option<Arc<dyn SyncProgressReporter>>,
) -> Result<(), Error> {
    let reporter = progress_reporter.unwrap_or_else(|| Arc::new(NoOpProgressReporter));
    if relative_paths.is_empty() {
        log::debug!("No files provided for deletion in branch '{}'.", branch_name);
        return Ok(());
    }
    log::info!("Deleting points for {} files in branch '{}' from collection '{}'...",
        relative_paths.len(), branch_name, collection_name);

    // Report start of deletion stage
    reporter.report(SyncProgress::new(SyncStage::DeleteFile {
        current_file: None,
        total_files: relative_paths.len(),
        current_file_num: 0,
        files_per_second: None,
        message: Some(format!("Starting deletion scan for {} files.", relative_paths.len())),
    })).await;

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

    let total_points_to_delete = point_ids_to_delete.len();
    let mut deleted_points_count = 0;

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
        deleted_points_count += chunk.len();
        // Report progress after each batch deletion
        reporter.report(SyncProgress::new(SyncStage::DeleteFile {
            current_file: None,
            total_files: relative_paths.len(),
            current_file_num: relative_paths.len(),
            files_per_second: None,
            message: Some(format!("Deleted {}/{} points associated with scanned files.", deleted_points_count, total_points_to_delete)),
        })).await;
    }
    log::info!("Successfully deleted {} points for {} files in branch '{}'.",
        point_ids_to_delete.len(), relative_paths.len(), branch_name);
    Ok(())
}

/// Ensures that a Qdrant collection exists for the repository, creating it if necessary.
/// Also sets up required payload indexes.
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

/// Creates a Qdrant filter to match points belonging to a specific branch.
pub fn create_branch_filter(branch_name: &str) -> Filter {
    Filter::must([
        Condition::matches(FIELD_BRANCH, branch_name.to_string()),
    ])
}

/// Deletes all points associated with a specific branch from a Qdrant collection.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, PerformanceConfig};

    fn create_test_config() -> AppConfig {
        AppConfig {
            performance: PerformanceConfig {
                batch_size: 256,
                collection_name_prefix: "test_repo_".to_string(),
                max_file_size_bytes: 5 * 1024 * 1024,
                vector_dimension: 384,
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_get_collection_name_legacy() {
        let config = create_test_config();
        let result = get_collection_name("my-repo", &config);
        assert_eq!(result, "test_repo_my-repo");
    }

    #[test]
    fn test_get_branch_aware_collection_name() {
        let config = create_test_config();
        
        // Test with main branch
        let result = get_branch_aware_collection_name("my-repo", "main", &config);
        assert!(result.starts_with("test_repo_my-repo_br_"));
        assert_eq!(result.len(), "test_repo_my-repo_br_".len() + 8); // 8-char hash
        
        // Test with different branch - should produce different hash
        let result2 = get_branch_aware_collection_name("my-repo", "feature-branch", &config);
        assert!(result2.starts_with("test_repo_my-repo_br_"));
        assert_ne!(result, result2); // Different branches should have different collection names
        
        // Test with special characters in branch name
        let result3 = get_branch_aware_collection_name("my-repo", "feature/special-chars_123", &config);
        assert!(result3.starts_with("test_repo_my-repo_br_"));
        assert!(result3.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')); // Should be safe for collection names
    }

    #[test]
    fn test_should_sync_branch_logic() {
        // Test force sync
        assert!(should_sync_branch("abc123", None, true));
        assert!(should_sync_branch("abc123", Some(&BranchSyncMetadata {
            collection_name: "test_repo_my-repo_br_main".to_string(),
            last_commit_hash: Some("def456".to_string()),
            branch_or_ref: "main".to_string(),
            last_sync_timestamp: None,
            files_count: 10,
        }), true));
        
        // Test no metadata (first sync)
        assert!(should_sync_branch("abc123", None, false));
        
        // Test metadata with no commit hash
        assert!(should_sync_branch("abc123", Some(&BranchSyncMetadata {
            collection_name: "test_repo_my-repo_br_main".to_string(),
            last_commit_hash: None,
            branch_or_ref: "main".to_string(),
            last_sync_timestamp: None,
            files_count: 0,
        }), false));
        
        // Test metadata with different commit hash (needs sync)
        assert!(should_sync_branch("abc123", Some(&BranchSyncMetadata {
            collection_name: "test_repo_my-repo_br_main".to_string(),
            last_commit_hash: Some("def456".to_string()),
            branch_or_ref: "main".to_string(),
            last_sync_timestamp: None,
            files_count: 10,
        }), false));
        
        // Test metadata with same commit hash (no sync needed)
        assert!(!should_sync_branch("abc123", Some(&BranchSyncMetadata {
            collection_name: "test_repo_my-repo_br_main".to_string(),
            last_commit_hash: Some("abc123".to_string()),
            branch_or_ref: "main".to_string(),
            last_sync_timestamp: None,
            files_count: 10,
        }), false));
    }

    #[test]
    fn test_branch_hash_consistency() {
        let config = create_test_config();
        
        // Same inputs should always produce same collection name
        let result1 = get_branch_aware_collection_name("repo1", "main", &config);
        let result2 = get_branch_aware_collection_name("repo1", "main", &config);
        assert_eq!(result1, result2);
        
        // Different branches should produce different names
        let main_name = get_branch_aware_collection_name("repo1", "main", &config);
        let dev_name = get_branch_aware_collection_name("repo1", "dev", &config);
        assert_ne!(main_name, dev_name);
        
        // Different repos should produce different names
        let repo1_name = get_branch_aware_collection_name("repo1", "main", &config);
        let repo2_name = get_branch_aware_collection_name("repo2", "main", &config);
        assert_ne!(repo1_name, repo2_name);
    }
} 