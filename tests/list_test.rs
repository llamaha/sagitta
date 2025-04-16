use anyhow::Result;
use vectordb_lib::config::{AppConfig, RepositoryConfig};
use vectordb_lib::cli::repo_commands::list::list_repositories;
use std::path::PathBuf;
use std::collections::HashMap;

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
    fn test_list_repositories_with_active() -> Result<()> {
        let mut config = create_test_config();
        config.active_repository = Some("test-repo-1".to_string());
        
        // Access list_repositories directly now that it's public
        let result = list_repositories(&config);
        assert!(result.is_ok());
        Ok(())
    }
    
    #[test]
    fn test_list_repositories_no_active() -> Result<()> {
        let config = create_test_config();
        // No active repository set
        
        let result = list_repositories(&config);
        assert!(result.is_ok());
        Ok(())
    }
    
    #[test]
    fn test_list_repositories_empty() -> Result<()> {
        let config = AppConfig::default();
        // No repositories in the config
        
        let result = list_repositories(&config);
        assert!(result.is_ok());
        Ok(())
    }
} 