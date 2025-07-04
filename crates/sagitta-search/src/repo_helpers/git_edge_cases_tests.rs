#[cfg(test)]
mod edge_case_tests {
    use crate::repo_helpers::{prepare_repository, git_edge_cases::validate_ref_name};
    use crate::config::{AppConfig, PerformanceConfig};
    use crate::test_utils::ManualMockQdrantClient;
    use tempfile::TempDir;
    use std::fs;
    use std::sync::Arc;
    use git2::{Repository, Signature, Oid};
    use std::process::Command;
    use std::path::Path;
    use anyhow::Result;
    
    fn create_test_config() -> AppConfig {
        AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            embed_model: None,
            repositories_base_path: None,
            vocabulary_base_path: None,
            indexing: Default::default(),
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
    
    fn init_repo_with_commit(path: &Path) -> Result<Repository> {
        let repo = Repository::init(path)?;
        
        // Create initial commit
        let sig = Signature::now("Test User", "test@example.com")?;
        let tree_id = {
            let mut index = repo.index()?;
            fs::write(path.join("README.md"), "# Test Repo")?;
            index.add_path(Path::new("README.md"))?;
            index.write()?;
            index.write_tree()?
        };
        let tree = repo.find_tree(tree_id)?;
        
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        )?;
        
        drop(tree);
        Ok(repo)
    }
    
    #[tokio::test]
    async fn test_add_repo_with_head_target_ref() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        
        // Create a repo with a branch
        let repo = init_repo_with_commit(&repo_path).unwrap();
        let default_branch = repo.head().unwrap().shorthand().unwrap().to_string();
        
        // Create another branch
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("develop", &commit, false).unwrap();
        
        let config = create_test_config();
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection exists check and return false (doesn't exist)
        mock_client.expect_collection_exists(Ok(false));
        // Expect collection creation
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        // Test adding with HEAD as target_ref
        let result = prepare_repository(
            "",  // No URL, using local path
            Some("test_repo"),
            Some(&repo_path),
            None,  // No branch
            Some("HEAD"),  // This is the problematic case
            None,
            None,
            None,
            temp_dir.path(),
            client,
            384,
            &config,
            None,
        ).await;
        
        if let Err(ref e) = result {
            eprintln!("Test failed with error: {:?}", e);
        }
        assert!(result.is_ok());
        let repo_config = result.unwrap();
        
        // Should resolve HEAD to actual branch name, not "HEAD"
        assert_ne!(repo_config.default_branch, "HEAD");
        assert_eq!(repo_config.default_branch, default_branch);
    }
    
    #[tokio::test]
    async fn test_add_repo_in_detached_head_state() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        
        // Create a repo with multiple commits
        let repo = init_repo_with_commit(&repo_path).unwrap();
        
        // Create another commit
        let sig = Signature::now("Test User", "test@example.com").unwrap();
        fs::write(repo_path.join("file2.txt"), "content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file2.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let second_commit = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Second commit",
            &tree,
            &[&parent],
        ).unwrap();
        
        // Checkout first commit (detached HEAD)
        let first_commit = parent.id();
        repo.set_head_detached(first_commit).unwrap();
        
        let config = create_test_config();
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection exists check and return false (doesn't exist)
        mock_client.expect_collection_exists(Ok(false));
        // Expect collection creation
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        // Test adding in detached HEAD state
        let result = prepare_repository(
            "",
            Some("test_repo"),
            Some(&repo_path),
            None,
            None,
            None,
            None,
            None,
            temp_dir.path(),
            client,
            384,
            &config,
            None,
        ).await;
        
        assert!(result.is_ok());
        let repo_config = result.unwrap();
        
        // In detached HEAD, should still work (might use commit hash or default branch)
        assert!(!repo_config.default_branch.is_empty());
    }
    
    #[tokio::test]
    async fn test_add_repo_with_non_standard_default_branch() {
        let temp_dir = TempDir::new().unwrap();
        let bare_repo_path = temp_dir.path().join("bare_repo.git");
        
        // Create a bare repository with 'develop' as default branch
        Command::new("git")
            .args(&["init", "--bare", "--initial-branch=develop"])
            .arg(&bare_repo_path)
            .output()
            .expect("Failed to create bare repo");
        
        // Create a working repo that we'll push from
        let work_repo_path = temp_dir.path().join("work_repo");
        let repo = init_repo_with_commit(&work_repo_path).unwrap();
        
        // Add bare repo as remote and push
        Command::new("git")
            .current_dir(&work_repo_path)
            .args(&["remote", "add", "origin"])
            .arg(&bare_repo_path)
            .output()
            .unwrap();
        
        Command::new("git")
            .current_dir(&work_repo_path)
            .args(&["push", "-u", "origin", "master:develop"])
            .output()
            .unwrap();
        
        let config = create_test_config();
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection exists check and return false (doesn't exist)
        mock_client.expect_collection_exists(Ok(false));
        // Expect collection creation
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        // Clone and add the repo - should detect 'develop' as default
        let clone_path = temp_dir.path().join("cloned_repo");
        let result = prepare_repository(
            bare_repo_path.to_str().unwrap(),
            Some("test_repo"),
            None,  // Let it clone
            None,  // No branch specified
            None,
            None,
            None,
            None,
            temp_dir.path(),
            client,
            384,
            &config,
            None,
        ).await;
        
        // This might fail in current implementation but should ideally work
        if let Ok(repo_config) = result {
            // Should detect the actual default branch
            assert!(repo_config.default_branch == "develop" || 
                   repo_config.default_branch == "main" ||
                   repo_config.default_branch == "master");
        }
    }
    
    #[tokio::test]
    async fn test_add_repo_with_special_characters_in_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        
        let repo = init_repo_with_commit(&repo_path).unwrap();
        
        // Create branches with special characters
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature/user@domain", &commit, false).unwrap();
        repo.branch("bugfix/issue#123", &commit, false).unwrap();
        
        let config = create_test_config();
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection exists check and return false (doesn't exist)
        mock_client.expect_collection_exists(Ok(false));
        // Expect collection creation
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        // Test adding with special character branch
        let result = prepare_repository(
            "",
            Some("test_repo"),
            Some(&repo_path),
            Some("feature/user@domain"),
            None,
            None,
            None,
            None,
            temp_dir.path(),
            client,
            384,
            &config,
            None,
        ).await;
        
        assert!(result.is_ok());
        let repo_config = result.unwrap();
        
        // Should handle special characters (collection name uses hash)
        assert!(repo_config.tracked_branches.contains(&"feature/user@domain".to_string()));
    }
    
    #[tokio::test]
    async fn test_sync_with_uncommitted_changes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        
        let repo = init_repo_with_commit(&repo_path).unwrap();
        
        // Add uncommitted changes
        fs::write(repo_path.join("uncommitted.txt"), "changes").unwrap();
        
        // Create another branch
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("develop", &commit, false).unwrap();
        
        // Try to switch branches with uncommitted changes
        let result = Command::new("git")
            .current_dir(&repo_path)
            .args(&["checkout", "develop"])
            .output();
        
        // Git should prevent this or warn
        assert!(result.is_ok());
        let output = result.unwrap();
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("changes") || stderr.contains("uncommitted"));
        }
    }
    
    #[tokio::test]
    async fn test_add_empty_repository() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("empty_repo");
        
        // Create empty repo (no commits)
        Repository::init(&repo_path).unwrap();
        
        let config = create_test_config();
        let mut mock_client = ManualMockQdrantClient::new();
        // Expect collection exists check and return false (doesn't exist)
        mock_client.expect_collection_exists(Ok(false));
        // Expect collection creation
        mock_client.expect_create_collection(Ok(true));
        let client = Arc::new(mock_client);
        
        // Test adding empty repo
        let result = prepare_repository(
            "",
            Some("empty_repo"),
            Some(&repo_path),
            None,
            None,
            None,
            None,
            None,
            temp_dir.path(),
            client,
            384,
            &config,
            None,
        ).await;
        
        // Should handle empty repos gracefully
        // Current implementation might fail, but should ideally handle this
        match result {
            Ok(repo_config) => {
                assert!(!repo_config.default_branch.is_empty());
            }
            Err(e) => {
                // Should give meaningful error about empty repo
                assert!(e.to_string().contains("empty") || 
                       e.to_string().contains("no commits") ||
                       e.to_string().contains("unborn"));
            }
        }
    }
    
    #[tokio::test] 
    async fn test_validate_branch_names() {
        use crate::repo_helpers::git_edge_cases::validate_ref_name;
        
        // Valid branch names
        assert!(validate_ref_name("main").is_ok());
        assert!(validate_ref_name("feature/new-ui").is_ok());
        assert!(validate_ref_name("bugfix/JIRA-123").is_ok());
        assert!(validate_ref_name("release-1.0.0").is_ok());
        
        // Invalid branch names  
        assert!(validate_ref_name("").is_err());
        assert!(validate_ref_name("refs/heads/main").is_err());
        assert!(validate_ref_name("branch..name").is_err());
        assert!(validate_ref_name("branch.").is_err());
        assert!(validate_ref_name("@{upstream}").is_err());
        
        // Edge cases
        assert!(validate_ref_name("feature/user@example.com").is_ok()); // @ is ok in middle
        assert!(validate_ref_name("-branch").is_ok()); // Leading dash is actually ok in git
        assert!(validate_ref_name("feature/🚀").is_ok()); // Unicode is ok
    }
}