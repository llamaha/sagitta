use vectordb_core::config::{AppConfig, RepositoryConfig};
use vectordb_cli::cli::stats::{StatsArgs, handle_stats};
use qdrant_client::Qdrant;
use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tempfile::tempdir;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> AppConfig {
        let mut config = AppConfig::default();
        
        // Add a test repository
        let repo = RepositoryConfig {
            name: "test-repo".to_string(),
            url: "file:///tmp/test-repo".to_string(), // Use file URL for local test repo
            local_path: PathBuf::from("/tmp/test-repo"),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            active_branch: Some("main".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            // Initialize last_synced_commits as empty
            last_synced_commits: HashMap::new(),
            indexed_languages: Some(vec!["rust".to_string()]), // Assuming Rust is indexed
            added_as_local_path: false, // Assuming added via URL/clone initially
            target_ref: None,
        };
        
        config.repositories.push(repo);
        config.active_repository = Some("test-repo".to_string());
        
        config
    }
    
    #[test]
    fn test_handle_stats() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
            let config = create_test_config();
            let args = StatsArgs { 
                config_file: None, 
                json: false 
            };
            
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

            // Add a second repository to trigger the specific error
            let repo2 = RepositoryConfig {
                name: "repo2".to_string(),
                url: "url2".to_string(),
                local_path: PathBuf::from("/tmp/repo2"),
                default_branch: "dev".to_string(),
                tracked_branches: vec!["dev".to_string()],
                remote_name: Some("origin".to_string()),
                active_branch: Some("dev".to_string()),
                ssh_key_path: None,
                ssh_key_passphrase: None,
                // Add missing fields
                last_synced_commits: HashMap::new(),
                indexed_languages: None,
                added_as_local_path: false,
                target_ref: None,
            };
            config.repositories.push(repo2);

            let args = StatsArgs {
                config_file: None,
                json: false,
            };
            
            let result = handle_stats(args, config, client).await;
            assert!(result.is_err());
            
            let err = result.unwrap_err().to_string();
            assert!(err.contains("No active repository set"));
        });
    }
} 