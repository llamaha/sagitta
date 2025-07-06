use anyhow::Result;
use sagitta_search::{AppConfig, RepositoryConfig, get_enhanced_repository_list};
use sagitta_cli::cli::repo_commands::list::{list_repositories, ListArgs};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::tempdir;

    fn create_test_config() -> AppConfig {
        AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
        repositories: vec![],
            active_repository: None,
        ..Default::default()
        }
    }

    fn create_test_repo_config(name: &str, url: &str, local_path: PathBuf, active_branch: Option<String>) -> RepositoryConfig {
        RepositoryConfig {
            name: name.to_string(),
            url: url.to_string(),
            local_path,
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string(), "dev".to_string()],
            active_branch,
            remote_name: Some("origin".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            last_synced_commits: HashMap::new(),
            indexed_languages: Some(vec!["rust".to_string()]),
            added_as_local_path: false,
            target_ref: None,
            dependencies: Vec::new(),
        }
    }
    
#[tokio::test]
async fn test_list_repositories_with_active() -> Result<()> {
    let temp_dir = tempdir()?;
        let mut config = create_test_config();
    let repo1_path = temp_dir.path().join("repo1");
    let repo2_path = temp_dir.path().join("repo2");
    
    // Create directories to make them exist
    std::fs::create_dir_all(&repo1_path)?;
    std::fs::create_dir_all(&repo2_path)?;
    
        let repo1 = create_test_repo_config(
            "test-repo-1",
            "https://github.com/test/repo1.git",
        repo1_path,
            Some("main".to_string())
        );
        let repo2 = create_test_repo_config(
            "test-repo-2",
            "https://github.com/test/repo2.git",
        repo2_path,
            Some("dev".to_string())
        );
        
        config.repositories = vec![repo1, repo2];
        config.active_repository = Some("test-repo-1".to_string());
        
    let result = list_repositories(&config, ListArgs { json: false, detailed: false, summary: false }).await;
        assert!(result.is_ok());
        Ok(())
    }
    
#[tokio::test]
async fn test_list_repositories_no_active() -> Result<()> {
    let temp_dir = tempdir()?;
        let mut config = create_test_config();
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir_all(&repo_path)?;
    
        let repo = create_test_repo_config(
            "test-repo",
            "https://github.com/test/repo.git",
        repo_path,
            None
        );
        config.repositories = vec![repo];
        
    let result = list_repositories(&config, ListArgs { json: false, detailed: false, summary: false }).await;
        assert!(result.is_ok());
        Ok(())
    }
    
#[tokio::test]
async fn test_list_repositories_empty() -> Result<()> {
        let config = AppConfig::default();
    let result = list_repositories(&config, ListArgs { json: false, detailed: false, summary: false }).await;
        assert!(result.is_ok());
        Ok(())
    }

#[tokio::test]
async fn test_list_repositories_json_output() -> Result<()> {
    let temp_dir = tempdir()?;
        let mut config = create_test_config();
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir_all(&repo_path)?;
    
        let repo = create_test_repo_config(
            "test-repo",
            "https://github.com/test/repo.git",
        repo_path,
            Some("main".to_string())
        );
        config.repositories = vec![repo];
        config.active_repository = Some("test-repo".to_string());

    let result = list_repositories(&config, ListArgs { json: true, detailed: false, summary: false }).await;
        assert!(result.is_ok());
        Ok(())
    }

#[tokio::test]
async fn test_list_repositories_with_different_states() -> Result<()> {
    let temp_dir = tempdir()?;
        let mut config = create_test_config();
        
        // Create repositories with different states
    let repo1_path = temp_dir.path().join("repo1");
    let repo2_path = temp_dir.path().join("repo2");
    let repo3_path = temp_dir.path().join("repo3");
    
    std::fs::create_dir_all(&repo1_path)?;
    std::fs::create_dir_all(&repo2_path)?;
    // Don't create repo3_path to test nonexistent repos
    
        let repo1 = create_test_repo_config(
            "repo1",
            "https://github.com/test/repo1.git",
        repo1_path,
            Some("main".to_string())
        );
        
        let mut repo2 = create_test_repo_config(
            "repo2",
            "https://github.com/test/repo2.git",
        repo2_path,
            Some("dev".to_string())
        );
        repo2.indexed_languages = None; // No indexed languages
        
        let mut repo3 = create_test_repo_config(
            "repo3",
            "https://github.com/test/repo3.git",
        repo3_path,
            None // No active branch
        );
        repo3.tracked_branches = vec![]; // No tracked branches
        
        config.repositories = vec![repo1, repo2, repo3];
        config.active_repository = Some("repo1".to_string());
        
    let result = list_repositories(&config, ListArgs { json: false, detailed: false, summary: false }).await;
    assert!(result.is_ok());
    Ok(())
}

#[tokio::test]
async fn test_list_repositories_detailed() -> Result<()> {
    let temp_dir = tempdir()?;
    let mut config = create_test_config();
    let repo_path = temp_dir.path().join("detailed_repo");
    std::fs::create_dir_all(&repo_path)?;
    
    // Create some test files
    std::fs::write(repo_path.join("main.rs"), "fn main() {}")?;
    std::fs::write(repo_path.join("lib.rs"), "pub mod test;")?;
    std::fs::write(repo_path.join("README.md"), "# Test")?;
    
    let repo = create_test_repo_config(
        "detailed-repo",
        "https://github.com/test/detailed-repo.git",
        repo_path,
        Some("main".to_string())
    );
    config.repositories = vec![repo];
    
    let result = list_repositories(&config, ListArgs { json: false, detailed: true, summary: false }).await;
    assert!(result.is_ok());
    Ok(())
}

#[tokio::test]
async fn test_list_repositories_summary() -> Result<()> {
    let temp_dir = tempdir()?;
    let mut config = create_test_config();
    let repo_path = temp_dir.path().join("summary_repo");
    std::fs::create_dir_all(&repo_path)?;
    
    let repo = create_test_repo_config(
        "summary-repo",
        "https://github.com/test/summary-repo.git",
        repo_path,
        Some("main".to_string())
    );
    config.repositories = vec![repo];
    
    let result = list_repositories(&config, ListArgs { json: false, detailed: false, summary: true }).await;
        assert!(result.is_ok());
        Ok(())
    }

#[tokio::test]
async fn test_enhanced_repository_list_direct() -> Result<()> {
    let temp_dir = tempdir()?;
    let mut config = create_test_config();
    let repo_path = temp_dir.path().join("direct_test_repo");
    std::fs::create_dir_all(&repo_path)?;
    
    // Create test files with different extensions
    std::fs::write(repo_path.join("main.rs"), "fn main() {}")?;
    std::fs::write(repo_path.join("test.py"), "print('hello')")?;
    std::fs::write(repo_path.join("README.md"), "# Test")?;
    
    let repo = create_test_repo_config(
        "direct-test-repo",
        "https://github.com/test/direct-test-repo.git",
        repo_path,
        Some("main".to_string())
    );
    config.repositories = vec![repo];
    
    // Test the enhanced repository list function directly
    let enhanced_list = get_enhanced_repository_list(&config).await?;
    
    assert_eq!(enhanced_list.total_count, 1);
    assert_eq!(enhanced_list.repositories.len(), 1);
    
    let repo_info = &enhanced_list.repositories[0];
    assert_eq!(repo_info.name, "direct-test-repo");
    assert!(repo_info.filesystem_status.exists);
    assert!(repo_info.filesystem_status.total_files.unwrap() >= 3);
    assert!(!repo_info.file_extensions.is_empty());
    
    // Check summary
    assert_eq!(enhanced_list.summary.existing_count, 1);
    assert!(enhanced_list.summary.total_files >= 3);
    
    Ok(())
} 