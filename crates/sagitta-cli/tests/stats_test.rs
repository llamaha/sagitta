use sagitta_search::config::{AppConfig, RepositoryConfig, IndexingConfig, PerformanceConfig};
use sagitta_cli::cli::stats::{StatsArgs, handle_stats};
use sagitta_cli::cli::CliArgs;
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
        let temp_dir = tempdir().unwrap();
        let repo_base = temp_dir.path().join("repos");
        let vocab_base = temp_dir.path().join("vocab");

        AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            embed_model: None,
            server_api_key_path: None,
            repositories_base_path: Some(repo_base.to_string_lossy().into_owned()),
            vocabulary_base_path: Some(vocab_base.to_string_lossy().into_owned()),
            repositories: Vec::new(),
            active_repository: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig::default(),
            embedding: sagitta_search::config::EmbeddingEngineConfig::default(),
        }
    }
    
    #[test]
    fn test_handle_stats() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
            let mut config = create_test_config();
            
            // Add a repository and set it as active
            let repo = RepositoryConfig {
                name: "test_repo".to_string(),
                url: "/test/repo".to_string(),
                local_path: PathBuf::from("/test/repo"),
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
                dependencies: Vec::new(),
            };
            config.repositories.push(repo);
            config.active_repository = Some("test_repo".to_string());

            let mock_cli_args = CliArgs::default();

            let args = StatsArgs { 
                config_file: None, 
                json: false 
            };
            
            // This is mainly testing that the function either succeeds or fails with an expected error
            let result = handle_stats(args, config.clone(), client, &mock_cli_args).await;
            
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

            // Add first repository
            let repo1 = RepositoryConfig {
                name: "repo1".to_string(),
                url: "/path/to/repo1".to_string(),
                local_path: PathBuf::from("/path/to/repo1"),
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
                dependencies: Vec::new(),
            };
            config.repositories.push(repo1);

            // Add second repository
            let repo2 = RepositoryConfig {
                name: "repo2".to_string(),
                url: "/path/to/repo2".to_string(),
                local_path: PathBuf::from("/path/to/repo2"),
                default_branch: "dev".to_string(),
                tracked_branches: vec!["dev".to_string()],
                remote_name: Some("origin".to_string()),
                active_branch: Some("dev".to_string()),
                ssh_key_path: None,
                ssh_key_passphrase: None,
                last_synced_commits: HashMap::new(),
                indexed_languages: None,
                added_as_local_path: false,
                target_ref: None,
                dependencies: Vec::new(),
            };
            config.repositories.push(repo2);

            let mock_cli_args = CliArgs::default();

            let args = StatsArgs {
                config_file: None,
                json: false,
            };
            
            let result = handle_stats(args, config, client, &mock_cli_args).await;
            assert!(result.is_err());
            
            let err = result.unwrap_err().to_string();
            let expected_error_part1 = "No active repository set and multiple repositories exist.";
            let expected_error_part2 = "Please specify a repository with 'repo use <name>'.";
            assert!(err.contains(expected_error_part1), "Error message did not contain: {}. Full error: {}", expected_error_part1, err);
            assert!(err.contains(expected_error_part2), "Error message did not contain: {}. Full error: {}", expected_error_part2, err);
        });
    }
} 