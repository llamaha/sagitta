use anyhow::{anyhow, Result};
use tracing::{info, warn};
use crate::QdrantClientTrait;
use crate::config::{AppConfig, RepositoryConfig};
use super::qdrant_utils::get_branch_aware_collection_name;

/// Validates that a repository's collections exist in Qdrant and match the config state.
/// Returns true if collections are valid, false if they need to be recreated.
pub async fn validate_repository_collections<C>(
    client: &C,
    repo_config: &RepositoryConfig,
    app_config: &AppConfig,
) -> Result<bool>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    // If there are no synced commits, nothing to validate
    if repo_config.last_synced_commits.is_empty() {
        info!("No synced commits for repository '{}', collections are valid", repo_config.name);
        return Ok(true);
    }
    
    let mut all_valid = true;
    
    // Check each branch that claims to be synced
    for (branch, commit) in &repo_config.last_synced_commits {
        let collection_name = get_branch_aware_collection_name(&repo_config.name, branch, app_config);
        
        match client.collection_exists(collection_name.clone()).await {
            Ok(exists) => {
                if !exists {
                    warn!(
                        "Collection '{}' for branch '{}' does not exist but config shows it was synced to commit {}",
                        collection_name, branch, commit
                    );
                    all_valid = false;
                } else {
                    // Collection exists, check if it has content
                    match client.get_collection_info(collection_name.clone()).await {
                        Ok(info) => {
                            let points_count = info.points_count.unwrap_or(0);
                            if points_count == 0 {
                                warn!(
                                    "Collection '{}' exists but is empty, while config shows it was synced",
                                    collection_name
                                );
                                all_valid = false;
                            } else {
                                info!(
                                    "Collection '{}' exists with {} points",
                                    collection_name, points_count
                                );
                            }
                        }
                        Err(e) => {
                            warn!("Failed to get info for collection '{}': {}", collection_name, e);
                            all_valid = false;
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to check existence of collection '{}': {}", collection_name, e);
                all_valid = false;
            }
        }
    }
    
    Ok(all_valid)
}

/// Clears sync metadata for a repository when collections are missing.
/// This forces a full re-sync on the next sync operation.
pub fn clear_sync_metadata(repo_config: &mut RepositoryConfig) {
    if !repo_config.last_synced_commits.is_empty() {
        warn!(
            "Clearing sync metadata for repository '{}' due to missing collections",
            repo_config.name
        );
        repo_config.last_synced_commits.clear();
    }
}

/// Validates and repairs repository state before operations.
/// Returns true if the repository is ready for operations, false if it needs repair.
pub async fn validate_and_repair_repository<C>(
    client: &C,
    repo_config: &mut RepositoryConfig,
    app_config: &AppConfig,
    auto_repair: bool,
) -> Result<bool>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    // Check if collections match the config state
    let collections_valid = validate_repository_collections(client, repo_config, app_config).await?;
    
    if !collections_valid {
        if auto_repair {
            info!("Auto-repairing repository '{}' by clearing sync metadata", repo_config.name);
            clear_sync_metadata(repo_config);
            Ok(true) // Repository is now ready for full sync
        } else {
            Err(anyhow!(
                "Repository '{}' has missing or empty collections but config shows it was synced. \
                Run with --force to perform a full re-sync, or use 'repo repair' to fix automatically.",
                repo_config.name
            ))
        }
    } else {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ManualMockQdrantClient;
    use std::collections::HashMap;
    
    #[tokio::test]
    async fn test_validate_collections_all_exist() {
        let mut client = ManualMockQdrantClient::new();
        client.expect_collection_exists(Ok(true));
        // Create a mock CollectionInfo response
        use qdrant_client::qdrant::CollectionInfo;
        let collection_info = CollectionInfo {
            status: 1, // Exists
            optimizer_status: None,
            vectors_count: Some(100),
            indexed_vectors_count: Some(100),
            points_count: Some(100),
            segments_count: 1,
            config: None,
            payload_schema: HashMap::new(),
        };
        client.expect_get_collection_info(Ok(collection_info));
        
        let mut repo_config = RepositoryConfig {
            name: "repo".to_string(),
            url: "https://example.com/repo.git".to_string(),
            local_path: "/tmp/repo".into(),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            active_branch: Some("main".to_string()),
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::from([
                ("main".to_string(), "abc123".to_string()),
            ]),
            indexed_languages: None,
            ssh_key_path: None,
            ssh_key_passphrase: None,
            added_as_local_path: false,
            target_ref: None,
            dependencies: Vec::new(),
            last_synced_commit: None,
        };
        
        let app_config = AppConfig {
            performance: crate::config::PerformanceConfig {
                collection_name_prefix: "test_".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        
        let result = validate_repository_collections(&client, &repo_config, &app_config).await;
        assert!(result.unwrap());
    }
    
    #[tokio::test]
    async fn test_validate_collections_missing() {
        let mut client = ManualMockQdrantClient::new();
        client.expect_collection_exists(Ok(false));
        
        let repo_config = RepositoryConfig {
            name: "repo".to_string(),
            url: "https://example.com/repo.git".to_string(),
            local_path: "/tmp/repo".into(),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            active_branch: Some("main".to_string()),
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::from([
                ("main".to_string(), "abc123".to_string()),
            ]),
            indexed_languages: None,
            ssh_key_path: None,
            ssh_key_passphrase: None,
            added_as_local_path: false,
            target_ref: None,
            dependencies: Vec::new(),
            last_synced_commit: None,
        };
        
        let app_config = AppConfig {
            performance: crate::config::PerformanceConfig {
                collection_name_prefix: "test_".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        
        let result = validate_repository_collections(&client, &repo_config, &app_config).await;
        assert!(!result.unwrap());
    }
    
    #[tokio::test]
    async fn test_clear_sync_metadata() {
        let mut repo_config = RepositoryConfig {
            name: "repo".to_string(),
            url: "https://example.com/repo.git".to_string(),
            local_path: "/tmp/repo".into(),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            active_branch: Some("main".to_string()),
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::from([
                ("main".to_string(), "abc123".to_string()),
                ("dev".to_string(), "def456".to_string()),
            ]),
            indexed_languages: None,
            ssh_key_path: None,
            ssh_key_passphrase: None,
            added_as_local_path: false,
            target_ref: None,
            dependencies: Vec::new(),
            last_synced_commit: None,
        };
        
        assert_eq!(repo_config.last_synced_commits.len(), 2);
        clear_sync_metadata(&mut repo_config);
        assert_eq!(repo_config.last_synced_commits.len(), 0);
    }
}

// Include the Qdrant wipe recovery tests
#[cfg(test)]
#[path = "qdrant_wipe_recovery_test.rs"]
mod qdrant_wipe_recovery_tests;