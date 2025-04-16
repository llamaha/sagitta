use anyhow::Result;
use vectordb_lib::config::{AppConfig, RepositoryConfig};
use vectordb_lib::cli::stats::{StatsArgs, handle_stats};
use qdrant_client::Qdrant;
use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> AppConfig {
        let mut config = AppConfig::default();
        
        // Add a test repository
        let repo = RepositoryConfig {
            name: "test-repo-1".to_string(),
            url: "https://github.com/test/repo1.git".to_string(),
            local_path: PathBuf::from("/tmp/test-repo-1"),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            indexed_languages: Some(vec!["rust".to_string()]),
            active_branch: Some("main".to_string()),
            remote_name: Some("origin".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            last_synced_commits: HashMap::new(),
        };
        
        config.repositories.push(repo);
        config.active_repository = Some("test-repo-1".to_string());
        
        config
    }
    
    #[test]
    fn test_handle_stats() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
            let config = create_test_config();
            let args = StatsArgs { config_file: None };
            
            // This is mainly testing that the function either succeeds or fails with an expected error
            let result = handle_stats(args, config.clone(), client).await;
            
            if result.is_err() {
                let err = result.unwrap_err().to_string();
                // It's expected to fail if the collection doesn't exist in testing environment
                if err.contains("Failed to get collection info") || 
                   err.contains("not found") || 
                   err.contains("doesn't exist") {
                    // This is fine - collection probably doesn't exist yet
                    println!("Test passed with expected error for non-existent collection");
                } else {
                    // For other errors, we should fail the test
                    panic!("Unexpected error: {}", err);
                }
            } else {
                // It succeeded, which is also fine
                assert!(result.is_ok());
            }
        });
    }
    
    #[test]
    fn test_handle_stats_no_active_repo() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
            let mut config = create_test_config();
            config.active_repository = None; // No active repository
            let args = StatsArgs { config_file: None };
            
            let result = handle_stats(args, config, client).await;
            assert!(result.is_err());
            
            let err = result.unwrap_err().to_string();
            assert!(err.contains("No active repository set"));
        });
    }
} 