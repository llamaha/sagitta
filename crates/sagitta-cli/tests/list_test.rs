use anyhow::Result;
use sagitta_search::config::{AppConfig, RepositoryConfig, IndexingConfig, PerformanceConfig};
use sagitta_cli::cli::repo_commands::list::list_repositories;
use std::path::PathBuf;
use std::collections::HashMap;
use sagitta_cli::cli::repo_commands::list::ListArgs;
use tempfile::tempdir;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> AppConfig {
        let temp_dir = tempdir().unwrap();
        let repo_base = temp_dir.path().join("repos");
        let vocab_base = temp_dir.path().join("vocab");

        AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories_base_path: Some(repo_base.to_string_lossy().into_owned()),
            vocabulary_base_path: Some(vocab_base.to_string_lossy().into_owned()),
            repositories: Vec::new(),
            active_repository: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig::default(),
            embedding: sagitta_search::config::EmbeddingEngineConfig::default(),
            oauth: None,
            tls_enable: false,
            tls_cert_path: None,
            tls_key_path: None,
            cors_allowed_origins: None,
            cors_allow_credentials: true,
            tenant_id: Some("test-tenant".to_string()),
            rayon_num_threads: 4,
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
            tenant_id: Some("test_tenant".to_string()),
        }
    }
    
    #[test]
    fn test_list_repositories_with_active() -> Result<()> {
        let mut config = create_test_config();
        let repo1 = create_test_repo_config(
            "test-repo-1",
            "https://github.com/test/repo1.git",
            PathBuf::from("/tmp/repo1"),
            Some("main".to_string())
        );
        let repo2 = create_test_repo_config(
            "test-repo-2",
            "https://github.com/test/repo2.git",
            PathBuf::from("/tmp/repo2"),
            Some("dev".to_string())
        );
        
        config.repositories = vec![repo1, repo2];
        config.active_repository = Some("test-repo-1".to_string());
        
        let result = list_repositories(&config, ListArgs { json: false });
        assert!(result.is_ok());
        Ok(())
    }
    
    #[test]
    fn test_list_repositories_no_active() -> Result<()> {
        let mut config = create_test_config();
        let repo = create_test_repo_config(
            "test-repo",
            "https://github.com/test/repo.git",
            PathBuf::from("/tmp/repo"),
            None
        );
        config.repositories = vec![repo];
        
        let result = list_repositories(&config, ListArgs { json: false });
        assert!(result.is_ok());
        Ok(())
    }
    
    #[test]
    fn test_list_repositories_empty() -> Result<()> {
        let config = AppConfig::default();
        let result = list_repositories(&config, ListArgs { json: false });
        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_list_repositories_json_output() -> Result<()> {
        let mut config = create_test_config();
        let repo = create_test_repo_config(
            "test-repo",
            "https://github.com/test/repo.git",
            PathBuf::from("/tmp/repo"),
            Some("main".to_string())
        );
        config.repositories = vec![repo];
        config.active_repository = Some("test-repo".to_string());

        let result = list_repositories(&config, ListArgs { json: true });
        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_list_repositories_with_different_states() -> Result<()> {
        let mut config = create_test_config();
        
        // Create repositories with different states
        let repo1 = create_test_repo_config(
            "repo1",
            "https://github.com/test/repo1.git",
            PathBuf::from("/tmp/repo1"),
            Some("main".to_string())
        );
        
        let mut repo2 = create_test_repo_config(
            "repo2",
            "https://github.com/test/repo2.git",
            PathBuf::from("/tmp/repo2"),
            Some("dev".to_string())
        );
        repo2.indexed_languages = None; // No indexed languages
        
        let mut repo3 = create_test_repo_config(
            "repo3",
            "https://github.com/test/repo3.git",
            PathBuf::from("/tmp/repo3"),
            None // No active branch
        );
        repo3.tracked_branches = vec![]; // No tracked branches
        
        config.repositories = vec![repo1, repo2, repo3];
        config.active_repository = Some("repo1".to_string());
        
        let result = list_repositories(&config, ListArgs { json: false });
        assert!(result.is_ok());
        Ok(())
    }
} 