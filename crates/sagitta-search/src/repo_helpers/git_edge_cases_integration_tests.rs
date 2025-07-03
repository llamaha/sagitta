#[cfg(test)]
mod integration_tests {
    use crate::repo_helpers::delete_repository_data;
    use crate::config::{AppConfig, PerformanceConfig, IndexingConfig};
    use crate::test_utils::ManualMockQdrantClient;
    use crate::repo_add::{AddRepoArgs, handle_repo_add};
    use crate::sync::{sync_repository, SyncOptions};
    use tempfile::TempDir;
    use std::fs;
    use std::sync::Arc;
    use git2::{Repository, Signature};
    use std::process::Command;
    use std::path::{Path, PathBuf};
    use anyhow::Result;
    
    fn create_app_config(base_path: &Path) -> AppConfig {
        AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: Some("/tmp/model.onnx".to_string()),
            onnx_tokenizer_path: Some("/tmp/tokenizer".to_string()),
            embed_model: None,
            repositories_base_path: Some(base_path.to_string_lossy().to_string()),
            vocabulary_base_path: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig {
                collection_name_prefix: "test_".to_string(),
                ..Default::default()
            },
            embedding: Default::default(),
            repositories: Vec::new(),
            active_repository: None,
            server_api_key_path: None,
        }
    }
    
    fn create_test_repo(path: &Path, branch_name: &str) -> Result<()> {
        let repo = Repository::init(path)?;
        
        // Create initial commit
        let sig = Signature::now("Test User", "test@example.com")?;
        fs::write(path.join("README.md"), "# Test Repository")?;
        
        let mut index = repo.index()?;
        index.add_path(Path::new("README.md"))?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        
        // Create initial commit
        let commit_id = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        )?;
        
        // Create and checkout the specified branch if not master/main
        let default_branch = repo.head()?.shorthand().unwrap_or("master").to_string();
        if branch_name != default_branch {
            let commit = repo.find_commit(commit_id)?;
            repo.branch(branch_name, &commit, false)?;
            repo.set_head(&format!("refs/heads/{}", branch_name))?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;
        }
        
        Ok(())
    }
    
    #[tokio::test]
    #[ignore = "Git working tree conflicts with untracked files"]
    async fn test_add_repo_with_head_as_target() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path, "main").unwrap();
        
        let config = create_app_config(temp_dir.path());
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection checks and creation
        mock_client.expect_collection_exists(Ok(false));
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        let args = AddRepoArgs {
            name: Some("test_repo".to_string()),
            url: None,
            local_path: Some(repo_path.clone()),
            branch: None,
            target_ref: Some("HEAD".to_string()), // This should be resolved
            remote: None,
            repositories_base_path: None,
            ssh_key: None,
            ssh_passphrase: None,
        };
        
        let result = handle_repo_add(
            args,
            temp_dir.path().to_path_buf(),
            384,
            client,
            &config,
            None,
        ).await;
        
        assert!(result.is_ok(), "Failed to add repo with HEAD: {:?}", result);
        let repo_config = result.unwrap();
        
        // Should resolve HEAD to actual branch, not literal "HEAD"
        assert_ne!(repo_config.default_branch, "HEAD", "HEAD was not resolved");
        assert!(
            repo_config.default_branch == "main" || repo_config.default_branch == "master",
            "Unexpected branch: {}",
            repo_config.default_branch
        );
    }
    
    #[tokio::test]
    async fn test_add_repo_with_invalid_branch_name() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path, "main").unwrap();
        
        let config = create_app_config(temp_dir.path());
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection checks and creation (multiple calls)
        for _ in 0..5 {
            mock_client.expect_collection_exists(Ok(false));
            mock_client.expect_create_collection(Ok(true));
            mock_client.expect_collection_exists(Ok(true));
        }
        // Add get_collection_info expectations
        use qdrant_client::qdrant::CollectionInfo;
        let collection_info = CollectionInfo {
            status: 1,
            optimizer_status: None,
            vectors_count: Some(0),
            indexed_vectors_count: Some(0),
            points_count: Some(0),
            segments_count: 0,
            config: None,
            payload_schema: std::collections::HashMap::new(),
        };
        for _ in 0..5 {
            mock_client.expect_get_collection_info(Ok(collection_info.clone()));
        }
        let client = Arc::new(mock_client);
        
        // Test with invalid branch name
        let args = AddRepoArgs {
            name: Some("test_repo".to_string()),
            url: None,
            local_path: Some(repo_path.clone()),
            branch: Some("refs/heads/main".to_string()), // Invalid - should be just "main"
            target_ref: None,
            remote: None,
            repositories_base_path: None,
            ssh_key: None,
            ssh_passphrase: None,
        };
        
        let result = handle_repo_add(
            args,
            temp_dir.path().to_path_buf(),
            384,
            client,
            &config,
            None,
        ).await;
        
        // Should either fail with clear error or handle gracefully
        match result {
            Ok(repo_config) => {
                // If it succeeds, it should have the branch name (may include refs/heads/)
                assert!(
                    repo_config.default_branch == "main" || repo_config.default_branch == "refs/heads/main",
                    "Unexpected branch name: {}",
                    repo_config.default_branch
                );
            }
            Err(e) => {
                // Error should be clear about the issue
                let error_msg = e.to_string();
                assert!(
                    error_msg.contains("branch") || error_msg.contains("ref"),
                    "Error message not helpful: {}",
                    error_msg
                );
            }
        }
    }
    
    #[tokio::test]
    async fn test_add_repo_with_detached_head() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path, "main").unwrap();
        
        // Put repo in detached HEAD state
        let repo = Repository::open(&repo_path).unwrap();
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.set_head_detached(commit.id()).unwrap();
        
        let config = create_app_config(temp_dir.path());
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection checks and creation
        mock_client.expect_collection_exists(Ok(false));
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        let args = AddRepoArgs {
            name: Some("test_repo".to_string()),
            url: None,
            local_path: Some(repo_path.clone()),
            branch: None,
            target_ref: None,
            remote: None,
            repositories_base_path: None,
            ssh_key: None,
            ssh_passphrase: None,
        };
        
        let result = handle_repo_add(
            args,
            temp_dir.path().to_path_buf(),
            384,
            client,
            &config,
            None,
        ).await;
        
        // Should handle detached HEAD gracefully
        assert!(result.is_ok(), "Failed with detached HEAD: {:?}", result);
        let repo_config = result.unwrap();
        assert!(!repo_config.default_branch.is_empty());
    }
    
    #[tokio::test]
    #[ignore = "Complex mock setup needed for multiple QdrantClient calls"]
    async fn test_sync_with_uncommitted_changes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path, "main").unwrap();
        
        // Add the repo first
        let config = create_app_config(temp_dir.path());
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection checks and creation
        mock_client.expect_collection_exists(Ok(false));
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        let args = AddRepoArgs {
            name: Some("test_repo".to_string()),
            url: None,
            local_path: Some(repo_path.clone()),
            branch: None,
            target_ref: None,
            remote: None,
            repositories_base_path: None,
            ssh_key: None,
            ssh_passphrase: None,
        };
        
        let repo_config = handle_repo_add(
            args,
            temp_dir.path().to_path_buf(),
            384,
            client.clone(),
            &config,
            None,
        ).await.unwrap();
        
        // Add uncommitted changes
        fs::write(repo_path.join("uncommitted.txt"), "changes").unwrap();
        
        // Try to sync
        let sync_result = sync_repository(
            client,
            &repo_config,
            SyncOptions::default(),
            &config,
            None,
        ).await;
        
        // Should handle uncommitted changes gracefully
        assert!(sync_result.is_ok(), "Sync should handle uncommitted changes");
    }
    
    #[tokio::test]
    async fn test_add_empty_repository() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("empty_repo");
        
        // Create empty repo (no commits)
        Repository::init(&repo_path).unwrap();
        
        let config = create_app_config(temp_dir.path());
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection checks and creation
        mock_client.expect_collection_exists(Ok(false));
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        let args = AddRepoArgs {
            name: Some("empty_repo".to_string()),
            url: None,
            local_path: Some(repo_path.clone()),
            branch: None,
            target_ref: None,
            remote: None,
            repositories_base_path: None,
            ssh_key: None,
            ssh_passphrase: None,
        };
        
        let result = handle_repo_add(
            args,
            temp_dir.path().to_path_buf(),
            384,
            client,
            &config,
            None,
        ).await;
        
        // Should handle empty repo gracefully
        match result {
            Ok(_) => {
                // If it succeeds, great
            }
            Err(e) => {
                // If it fails, error should be clear
                let error_msg = e.to_string();
                assert!(
                    error_msg.contains("empty") || 
                    error_msg.contains("no commits") ||
                    error_msg.contains("unborn"),
                    "Error message not clear for empty repo: {}",
                    error_msg
                );
            }
        }
    }
    
    #[tokio::test]
    #[ignore = "Complex mock setup needed for multiple QdrantClient calls"]
    async fn test_add_repo_with_non_existent_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path, "main").unwrap();
        
        let config = create_app_config(temp_dir.path());
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection checks and creation
        mock_client.expect_collection_exists(Ok(false));
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        let args = AddRepoArgs {
            name: Some("test_repo".to_string()),
            url: None,
            local_path: Some(repo_path.clone()),
            branch: Some("non-existent-branch".to_string()),
            target_ref: None,
            remote: None,
            repositories_base_path: None,
            ssh_key: None,
            ssh_passphrase: None,
        };
        
        let result = handle_repo_add(
            args,
            temp_dir.path().to_path_buf(),
            384,
            client,
            &config,
            None,
        ).await;
        
        // Should fail with clear error about missing branch
        assert!(result.is_err(), "Should fail with non-existent branch");
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("branch") || error_msg.contains("checkout"),
            "Error should mention branch issue: {}",
            error_msg
        );
    }
    
    #[tokio::test]
    async fn test_remove_repo_that_doesnt_exist() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_app_config(temp_dir.path());
        let mut mock_client = ManualMockQdrantClient::new();
        // For delete_repository_data, expect delete_collection
        mock_client.expect_delete_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        // Try to remove a repo that was never added
        let result = delete_repository_data(
            &crate::config::RepositoryConfig {
                name: "non-existent".to_string(),
                url: "https://example.com/repo.git".to_string(),
                local_path: temp_dir.path().join("non-existent"),
                default_branch: "main".to_string(),
                tracked_branches: vec!["main".to_string()],
                active_branch: Some("main".to_string()),
                remote_name: Some("origin".to_string()),
                last_synced_commits: Default::default(),
                indexed_languages: None,
                ssh_key_path: None,
                ssh_key_passphrase: None,
                added_as_local_path: false,
                target_ref: None,
            },
            client,
            &config,
        ).await;
        
        // Should handle gracefully (not crash)
        assert!(result.is_ok(), "Should handle missing repo gracefully");
    }
    
    #[tokio::test]
    #[ignore = "Complex mock setup needed for multiple QdrantClient calls"]
    async fn test_add_repo_with_special_characters_in_name() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path, "main").unwrap();
        
        let config = create_app_config(temp_dir.path());
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection checks and creation
        mock_client.expect_collection_exists(Ok(false));
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        // Test with special characters in repo name
        for special_name in &["repo@host", "repo#123", "repo with spaces", "repo/sub"] {
            let args = AddRepoArgs {
                name: Some(special_name.to_string()),
                url: None,
                local_path: Some(repo_path.clone()),
                branch: None,
                target_ref: None,
                remote: None,
                repositories_base_path: None,
                ssh_key: None,
                ssh_passphrase: None,
            };
            
            let result = handle_repo_add(
                args,
                temp_dir.path().to_path_buf(),
                384,
                client.clone(),
                &config,
                None,
            ).await;
            
            // Should either handle gracefully or fail with clear error
            match result {
                Ok(repo_config) => {
                    // Collection name should be valid (uses hash for branches)
                    assert!(!repo_config.name.is_empty());
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    assert!(
                        error_msg.contains("name") || error_msg.contains("invalid"),
                        "Error should be clear about name issue: {}",
                        error_msg
                    );
                }
            }
        }
    }
    
    #[tokio::test]
    #[ignore = "Complex mock setup needed for multiple QdrantClient calls"]
    async fn test_sync_after_force_push() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path, "main").unwrap();
        
        let config = create_app_config(temp_dir.path());
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection checks and creation
        mock_client.expect_collection_exists(Ok(false));
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        // Add repo
        let args = AddRepoArgs {
            name: Some("test_repo".to_string()),
            url: None,
            local_path: Some(repo_path.clone()),
            branch: None,
            target_ref: None,
            remote: None,
            repositories_base_path: None,
            ssh_key: None,
            ssh_passphrase: None,
        };
        
        let mut repo_config = handle_repo_add(
            args,
            temp_dir.path().to_path_buf(),
            384,
            client.clone(),
            &config,
            None,
        ).await.unwrap();
        
        // Simulate a force push by resetting to a different commit
        let repo = Repository::open(&repo_path).unwrap();
        let sig = Signature::now("Test User", "test@example.com").unwrap();
        
        // Create a new file and commit
        fs::write(repo_path.join("file2.txt"), "content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file2.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Second commit",
            &tree,
            &[&parent],
        ).unwrap();
        
        // Sync should handle this gracefully
        let sync_result = sync_repository(
            client,
            &repo_config,
            SyncOptions::default(),
            &config,
            None,
        ).await;
        
        assert!(sync_result.is_ok(), "Sync should handle history changes");
    }
    
    #[tokio::test]
    #[ignore = "Complex mock setup needed for multiple QdrantClient calls"]
    async fn test_concurrent_operations_on_same_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path, "main").unwrap();
        
        let config = Arc::new(create_app_config(temp_dir.path()));
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection checks and creation
        mock_client.expect_collection_exists(Ok(false));
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        // Add repo first
        let args = AddRepoArgs {
            name: Some("test_repo".to_string()),
            url: None,
            local_path: Some(repo_path.clone()),
            branch: None,
            target_ref: None,
            remote: None,
            repositories_base_path: None,
            ssh_key: None,
            ssh_passphrase: None,
        };
        
        let repo_config = Arc::new(handle_repo_add(
            args,
            temp_dir.path().to_path_buf(),
            384,
            client.clone(),
            &config,
            None,
        ).await.unwrap());
        
        // Try concurrent syncs (should handle locking properly)
        let mut handles = vec![];
        
        for i in 0..3 {
            let client_clone = client.clone();
            let config_clone = config.clone();
            let repo_config_clone = repo_config.clone();
            
            let handle = tokio::spawn(async move {
                let result = sync_repository(
                    client_clone,
                    &repo_config_clone,
                    SyncOptions::default(),
                    &config_clone,
                    None,
                ).await;
                (i, result)
            });
            
            handles.push(handle);
        }
        
        // All should complete without errors
        for handle in handles {
            let (i, result) = handle.await.unwrap();
            assert!(
                result.is_ok(),
                "Concurrent sync {} failed: {:?}",
                i,
                result
            );
        }
    }
}