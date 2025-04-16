use anyhow::Result;
use vectordb_lib::config::{AppConfig, RepositoryConfig};
use vectordb_lib::cli::repo_commands::r#use::{UseRepoArgs, use_repository};
use std::path::PathBuf;
use std::collections::HashMap;
use tempfile::tempdir;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> AppConfig {
        let mut config = AppConfig::default();
        
        // Add some test repositories
        let repo1 = RepositoryConfig {
            name: "test-repo-1".to_string(),
            url: "https://github.com/test/repo1.git".to_string(),
            local_path: PathBuf::from("/tmp/test-repo-1"),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string(), "develop".to_string()],
            indexed_languages: Some(vec!["rust".to_string(), "python".to_string()]),
            active_branch: Some("main".to_string()),
            remote_name: Some("origin".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            last_synced_commits: HashMap::new(),
        };
        
        let repo2 = RepositoryConfig {
            name: "test-repo-2".to_string(),
            url: "https://github.com/test/repo2.git".to_string(),
            local_path: PathBuf::from("/tmp/test-repo-2"),
            default_branch: "master".to_string(),
            tracked_branches: vec!["master".to_string()],
            indexed_languages: None,
            active_branch: Some("master".to_string()),
            remote_name: Some("origin".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            last_synced_commits: HashMap::new(),
        };
        
        config.repositories.push(repo1);
        config.repositories.push(repo2);
        
        config
    }
    
    #[test]
    fn test_use_repository_existing() -> Result<()> {
        let temp_dir = tempdir()?;
        let config_path = temp_dir.path().join("config.toml");
        
        let mut config = create_test_config();
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