use anyhow::Result;
use sagitta_search::config::{AppConfig, RepositoryConfig, IndexingConfig, PerformanceConfig};
use sagitta_cli::cli::repo_commands::r#use::{UseRepoArgs, use_repository};
use std::path::PathBuf;
use std::collections::HashMap;
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
        }
    }
    
    #[test]
    fn test_use_repository_existing() -> Result<()> {
        let temp_dir = tempdir()?;
        let config_path = temp_dir.path().join("config.toml");
        
        let mut config = create_test_config();
        
        // Add both repositories to the config
        let repo1 = RepositoryConfig {
            name: "test-repo-1".to_string(),
            url: "url1".to_string(),
            local_path: PathBuf::from("/tmp/repo1"),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            active_branch: Some("main".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            last_synced_commits: HashMap::new(),
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
            tenant_id: Some("test_tenant".to_string()),
        };
        config.repositories.push(repo1);

        let repo2 = RepositoryConfig {
            name: "test-repo-2".to_string(),
            url: "url2".to_string(),
            local_path: PathBuf::from("/tmp/repo2"),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            active_branch: Some("main".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            last_synced_commits: HashMap::new(),
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
            tenant_id: Some("test_tenant".to_string()),
        };
        config.repositories.push(repo2);
        
        config.active_repository = Some("test-repo-1".to_string());
        
        let args = UseRepoArgs {
            name: "test-repo-2".to_string(),
        };
        
        let result = use_repository(args, &mut config, Some(&config_path));
        assert!(result.is_ok());
        assert_eq!(config.active_repository, Some("test-repo-2".to_string()));
        
        Ok(())
    }
    
    #[test]
    fn test_use_repository_with_no_previous_active() -> Result<()> {
        let temp_dir = tempdir()?;
        let config_path = temp_dir.path().join("config.toml");
        
        let mut config = create_test_config();
        
        // Add the repository to the config
        let repo1 = RepositoryConfig {
            name: "test-repo-1".to_string(),
            url: "url1".to_string(),
            local_path: PathBuf::from("/tmp/repo1"),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            active_branch: Some("main".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            last_synced_commits: HashMap::new(),
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
            tenant_id: Some("test_tenant".to_string()),
        };
        config.repositories.push(repo1);
        
        config.active_repository = None; // No active repository
        
        let args = UseRepoArgs {
            name: "test-repo-1".to_string(),
        };
        
        let result = use_repository(args, &mut config, Some(&config_path));
        assert!(result.is_ok());
        assert_eq!(config.active_repository, Some("test-repo-1".to_string()));
        
        Ok(())
    }
    
    #[test]
    fn test_use_repository_nonexistent() -> Result<()> {
        let temp_dir = tempdir()?;
        let config_path = temp_dir.path().join("config.toml");
        
        let mut config = create_test_config();
        config.active_repository = Some("test-repo-1".to_string());
        let original_active = config.active_repository.clone();
        
        let args = UseRepoArgs {
            name: "nonexistent-repo".to_string(),
        };
        
        let result = use_repository(args, &mut config, Some(&config_path));
        assert!(result.is_err());
        // The active repository should not change on error
        assert_eq!(config.active_repository, original_active);
        
        // Check the error message
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Repository 'nonexistent-repo' not found"));
        
        Ok(())
    }
} 