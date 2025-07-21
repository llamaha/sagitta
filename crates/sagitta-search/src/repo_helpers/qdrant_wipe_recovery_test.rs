#[cfg(test)]
mod qdrant_wipe_recovery_tests {
    use crate::sync::{sync_repository, SyncOptions, SyncResult};
    use crate::config::{AppConfig, RepositoryConfig, PerformanceConfig};
    use crate::test_utils::ManualMockQdrantClient;
    use tempfile::TempDir;
    use std::collections::HashMap;
    use std::sync::Arc;
    use git2::{Repository, Signature};
    use std::fs;
    use std::path::Path;
    
    fn create_test_repo_with_commit(path: &Path) -> String {
        let repo = Repository::init(path).unwrap();
        
        // Create initial commit
        let sig = Signature::now("Test User", "test@example.com").unwrap();
        fs::write(path.join("README.md"), "# Test Repository").unwrap();
        
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        
        let commit_id = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();
        
        commit_id.to_string()
    }
    
    #[tokio::test]
    #[ignore = "Complex mock setup - sync_repository calls many QdrantClient methods"]
    async fn test_sync_recovers_from_qdrant_wipe() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        let commit_hash = create_test_repo_with_commit(&repo_path);
        
        // Create a repo config that claims to be synced
        let repo_config = RepositoryConfig {
            name: "test_repo".to_string(),
            url: "https://example.com/repo.git".to_string(),
            local_path: repo_path.clone(),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            active_branch: Some("main".to_string()),
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::from([
                ("main".to_string(), commit_hash.clone()),
            ]),
            indexed_languages: Some(vec!["markdown".to_string()]),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            added_as_local_path: true,
            target_ref: None,
            dependencies: Vec::new(),
            last_synced_commit: None,
        };
        
        let app_config = AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            performance: PerformanceConfig {
                collection_name_prefix: "test_".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        
        // Mock client that simulates Qdrant being wiped (collection doesn't exist)
        let mut client = ManualMockQdrantClient::new();
        
        // The collection name that will be checked
        let collection_name = "test_test_repo_br_6b86b273"; // Hash of "main"
        
        // First check: collection doesn't exist (Qdrant was wiped)
        client.expect_collection_exists(Ok(false));
        
        // After detecting missing collection, sync should create it (multiple times)
        for _ in 0..10 {
            client.expect_create_collection(Ok(true));
        }
        
        // Collection validation may require deletion and recreation (multiple times)
        for _ in 0..5 {
            client.expect_delete_collection(Ok(true));
        }
        
        // Multiple collection_exists checks during sync process
        for _ in 0..10 {
            client.expect_collection_exists(Ok(true));
        }
        
        // Multiple get_collection_info checks during sync process
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
        for _ in 0..10 {
            client.expect_get_collection_info(Ok(collection_info.clone()));
        }
        
        // Note: ManualMockQdrantClient doesn't have expect_upsert_points method
        
        let client = Arc::new(client);
        
        // Run sync - it should detect missing collection and re-sync
        let result = sync_repository(
            client,
            &repo_config,
            SyncOptions::default(),
            &app_config,
            None,
        ).await;
        
        assert!(result.is_ok(), "Sync should succeed: {:?}", result);
        let sync_result = result.unwrap();
        
        // Should have re-indexed the file
        assert_eq!(sync_result.files_indexed, 1);
        assert!(sync_result.success);
        assert!(sync_result.message.contains("Successfully synced"));
    }
    
    #[tokio::test]
    #[ignore = "Complex mock setup - sync_repository calls many QdrantClient methods"]
    async fn test_sync_recovers_from_empty_collection() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        let commit_hash = create_test_repo_with_commit(&repo_path);
        
        // Add more files to make it obvious when re-syncing
        fs::write(repo_path.join("file1.rs"), "fn main() {}").unwrap();
        fs::write(repo_path.join("file2.rs"), "fn test() {}").unwrap();
        
        // Update the commit
        let repo = Repository::open(&repo_path).unwrap();
        let sig = Signature::now("Test User", "test@example.com").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file1.rs")).unwrap();
        index.add_path(Path::new("file2.rs")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let new_commit = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Add files",
            &tree,
            &[&parent],
        ).unwrap();
        
        let repo_config = RepositoryConfig {
            name: "test_repo".to_string(),
            url: "https://example.com/repo.git".to_string(),
            local_path: repo_path.clone(),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            active_branch: Some("main".to_string()),
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::from([
                ("main".to_string(), new_commit.to_string()),
            ]),
            indexed_languages: Some(vec!["rust".to_string()]),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            added_as_local_path: true,
            target_ref: None,
            dependencies: Vec::new(),
            last_synced_commit: None,
        };
        
        let app_config = AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            performance: PerformanceConfig {
                collection_name_prefix: "test_".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        
        // Mock client where collection exists but is empty
        let mut client = ManualMockQdrantClient::new();
        
        let collection_name = "test_test_repo_br_6b86b273"; // Hash of "main"
        
        // Collection exists but is empty (0 points)
        client.expect_collection_exists(Ok(true));
        // Mock get_collection_info to return empty collection
        use qdrant_client::qdrant::CollectionInfo;
        let empty_collection_info = CollectionInfo {
            status: 1, // Exists
            optimizer_status: None,
            vectors_count: Some(0),
            indexed_vectors_count: Some(0),
            points_count: Some(0), // Empty collection
            segments_count: 0,
            config: None,
            payload_schema: HashMap::new(),
        };
        client.expect_get_collection_info(Ok(empty_collection_info.clone()));
        
        // Collection validation may require deletion and recreation (multiple times)
        for _ in 0..10 {
            client.expect_delete_collection(Ok(true));
            client.expect_create_collection(Ok(true));
        }
        
        // Multiple get_collection_info calls during sync
        for _ in 0..10 {
            client.expect_get_collection_info(Ok(empty_collection_info.clone()));
        }
        
        // Multiple collection_exists checks during sync process
        for _ in 0..10 {
            client.expect_collection_exists(Ok(true));
        }
        
        // Note: ManualMockQdrantClient doesn't have expect_upsert_points method
        
        let client = Arc::new(client);
        
        // Run sync
        let result = sync_repository(
            client,
            &repo_config,
            SyncOptions::default(),
            &app_config,
            None,
        ).await;
        
        assert!(result.is_ok(), "Sync should succeed: {:?}", result);
        let sync_result = result.unwrap();
        
        // Should have re-indexed all files
        assert_eq!(sync_result.files_indexed, 3);
        assert!(sync_result.success);
    }
    
    #[tokio::test]
    #[ignore = "Complex mock setup - sync_repository calls many QdrantClient methods"]
    async fn test_force_sync_always_reindexes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        let commit_hash = create_test_repo_with_commit(&repo_path);
        
        let repo_config = RepositoryConfig {
            name: "test_repo".to_string(),
            url: "https://example.com/repo.git".to_string(),
            local_path: repo_path.clone(),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            active_branch: Some("main".to_string()),
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::from([
                ("main".to_string(), commit_hash.clone()),
            ]),
            indexed_languages: None,
            ssh_key_path: None,
            ssh_key_passphrase: None,
            added_as_local_path: true,
            target_ref: None,
            dependencies: Vec::new(),
            last_synced_commit: None,
        };
        
        let app_config = AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            performance: PerformanceConfig {
                collection_name_prefix: "test_".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        
        // Mock client - collection exists and has content
        let mut client = ManualMockQdrantClient::new();
        
        let collection_name = "test_test_repo_br_6b86b273";
        
        // Even though collection exists with content, force sync should re-index
        client.expect_collection_exists(Ok(true));
        // Mock get_collection_info to return collection with content
        use qdrant_client::qdrant::CollectionInfo;
        let collection_info = CollectionInfo {
            status: 1, // Exists
            optimizer_status: None,
            vectors_count: Some(10),
            indexed_vectors_count: Some(10),
            points_count: Some(10), // Has content
            segments_count: 1,
            config: None,
            payload_schema: HashMap::new(),
        };
        client.expect_get_collection_info(Ok(collection_info.clone()));
        
        // Collection validation may require deletion and recreation during force sync (multiple times)
        for _ in 0..10 {
            client.expect_delete_collection(Ok(true));
            client.expect_create_collection(Ok(true));
        }
        
        // Multiple get_collection_info calls during force sync
        for _ in 0..10 {
            client.expect_get_collection_info(Ok(collection_info.clone()));
        }
        
        // Multiple collection_exists checks during force sync process
        for _ in 0..10 {
            client.expect_collection_exists(Ok(true));
        }
        
        let client = Arc::new(client);
        
        // Run sync with force option
        let result = sync_repository(
            client,
            &repo_config,
            SyncOptions { force: true, ..Default::default() },
            &app_config,
            None,
        ).await;
        
        assert!(result.is_ok(), "Force sync should succeed: {:?}", result);
        let sync_result = result.unwrap();
        
        // Should have re-indexed despite being up to date
        assert_eq!(sync_result.files_indexed, 1);
        assert!(sync_result.success);
    }
}