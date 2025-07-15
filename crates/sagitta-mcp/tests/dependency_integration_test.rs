use sagitta_mcp::mcp::types::{
    RepositoryDependencyParams, RepositoryListDependenciesParams,
};
use sagitta_search::config::{AppConfig, RepositoryConfig};
use std::sync::Arc;
use tokio::sync::RwLock;
use tempfile::TempDir;
use std::env;

async fn create_test_config_with_repos() -> (Arc<RwLock<AppConfig>>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    
    // IMPORTANT: Set environment variable to ensure we're in test mode
    // This prevents any real config files from being touched
    env::set_var("SAGITTA_TEST_MODE", "1");
    
    // Create a completely isolated config that doesn't touch real paths
    // Use a subdirectory in temp to ensure complete isolation
    let test_base_path = temp_dir.path().join("test_repos");
    std::fs::create_dir_all(&test_base_path).unwrap();
    
    let config = AppConfig {
        // Use the isolated test directory
        repositories_base_path: Some(test_base_path.to_string_lossy().to_string()),
        // Ensure Qdrant URL is also isolated
        qdrant_url: "http://localhost:6333".to_string(),
        repositories: vec![
            RepositoryConfig {
                name: "test-main-app".to_string(),
                url: "https://github.com/test/main-app.git".to_string(),
                local_path: temp_dir.path().join("test-main-app"),
                default_branch: "main".to_string(),
                tracked_branches: vec!["main".to_string()],
                remote_name: Some("origin".to_string()),
                active_branch: Some("main".to_string()),
                ssh_key_path: None,
                ssh_key_passphrase: None,
                last_synced_commits: Default::default(),
                indexed_languages: None,
                added_as_local_path: false,
                target_ref: None,
                dependencies: vec![],
            },
            RepositoryConfig {
                name: "test-lib-a".to_string(),
                url: "https://github.com/test/lib-a.git".to_string(),
                local_path: temp_dir.path().join("test-lib-a"),
                default_branch: "main".to_string(),
                tracked_branches: vec!["main".to_string()],
                remote_name: Some("origin".to_string()),
                active_branch: Some("main".to_string()),
                ssh_key_path: None,
                ssh_key_passphrase: None,
                last_synced_commits: Default::default(),
                indexed_languages: None,
                added_as_local_path: false,
                target_ref: Some("v1.2.3".to_string()),
                dependencies: vec![],
            },
            RepositoryConfig {
                name: "test-lib-b".to_string(),
                url: "https://github.com/test/lib-b.git".to_string(),
                local_path: temp_dir.path().join("test-lib-b"),
                default_branch: "main".to_string(),
                tracked_branches: vec!["main".to_string()],
                remote_name: Some("origin".to_string()),
                active_branch: Some("develop".to_string()),
                ssh_key_path: None,
                ssh_key_passphrase: None,
                last_synced_commits: Default::default(),
                indexed_languages: None,
                added_as_local_path: false,
                target_ref: None,
                dependencies: vec![],
            },
        ],
        // Explicitly set model paths to temp directory to prevent any real file access
        onnx_model_path: Some(temp_dir.path().join("model.onnx").to_string_lossy().to_string()),
        onnx_tokenizer_path: Some(temp_dir.path().join("tokenizer").to_string_lossy().to_string()),
        embed_model: None,
        server_api_key_path: None,
        vocabulary_base_path: Some(temp_dir.path().join("vocab").to_string_lossy().to_string()),
        active_repository: None,
        indexing: Default::default(),
        performance: Default::default(),
        embedding: Default::default(),
    };
    
    (Arc::new(RwLock::new(config)), temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sagitta_mcp::handlers::dependency::{
        handle_repository_add_dependency,
        handle_repository_remove_dependency,
        handle_repository_list_dependencies,
    };
    
    // Helper to ensure test isolation
    struct TestGuard;
    
    impl Drop for TestGuard {
        fn drop(&mut self) {
            // Clean up test environment variable
            env::remove_var("SAGITTA_TEST_MODE");
        }
    }

    #[tokio::test]
    async fn test_add_dependency_success() {
        let _guard = TestGuard; // Ensures cleanup on test exit
        let (config, _temp_dir) = create_test_config_with_repos().await;
        
        let params = RepositoryDependencyParams {
            repository_name: "test-main-app".to_string(),
            dependency_name: "test-lib-a".to_string(),
            target_ref: Some("v1.0.0".to_string()),
            purpose: Some("Core utility library".to_string()),
        };
        
        let result = handle_repository_add_dependency(params, config.clone()).await.unwrap();
        assert!(result.success);
        assert!(result.message.contains("Successfully added dependency"));
        
        // Verify the dependency was added
        let config_read = config.read().await;
        let main_app = config_read.repositories.iter()
            .find(|r| r.name == "test-main-app")
            .unwrap();
        assert_eq!(main_app.dependencies.len(), 1);
        assert_eq!(main_app.dependencies[0].repository_name, "test-lib-a");
        assert_eq!(main_app.dependencies[0].target_ref, Some("v1.0.0".to_string()));
        assert_eq!(main_app.dependencies[0].purpose, Some("Core utility library".to_string()));
    }

    #[tokio::test]
    async fn test_add_dependency_nonexistent_main_repo() {
        let _guard = TestGuard;
        let (config, _temp_dir) = create_test_config_with_repos().await;
        
        let params = RepositoryDependencyParams {
            repository_name: "nonexistent".to_string(),
            dependency_name: "test-lib-a".to_string(),
            target_ref: None,
            purpose: None,
        };
        
        let result = handle_repository_add_dependency(params, config).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Repository 'nonexistent' not found"));
    }

    #[tokio::test]
    async fn test_add_dependency_nonexistent_dependency() {
        let _guard = TestGuard;
        let (config, _temp_dir) = create_test_config_with_repos().await;
        
        let params = RepositoryDependencyParams {
            repository_name: "test-main-app".to_string(),
            dependency_name: "nonexistent-lib".to_string(),
            target_ref: None,
            purpose: None,
        };
        
        let result = handle_repository_add_dependency(params, config).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found in repository manager"));
    }

    #[tokio::test]
    async fn test_update_existing_dependency() {
        let _guard = TestGuard;
        let (config, _temp_dir) = create_test_config_with_repos().await;
        
        // First add a dependency
        let params1 = RepositoryDependencyParams {
            repository_name: "test-main-app".to_string(),
            dependency_name: "test-lib-a".to_string(),
            target_ref: Some("v1.0.0".to_string()),
            purpose: Some("Initial purpose".to_string()),
        };
        handle_repository_add_dependency(params1, config.clone()).await.unwrap();
        
        // Update the same dependency
        let params2 = RepositoryDependencyParams {
            repository_name: "test-main-app".to_string(),
            dependency_name: "test-lib-a".to_string(),
            target_ref: Some("v2.0.0".to_string()),
            purpose: Some("Updated purpose".to_string()),
        };
        let result = handle_repository_add_dependency(params2, config.clone()).await.unwrap();
        assert!(result.success);
        
        // Verify the update
        let config_read = config.read().await;
        let main_app = config_read.repositories.iter()
            .find(|r| r.name == "test-main-app")
            .unwrap();
        assert_eq!(main_app.dependencies.len(), 1); // Still only one dependency
        assert_eq!(main_app.dependencies[0].target_ref, Some("v2.0.0".to_string()));
        assert_eq!(main_app.dependencies[0].purpose, Some("Updated purpose".to_string()));
    }

    #[tokio::test]
    async fn test_remove_dependency_success() {
        let _guard = TestGuard;
        let (config, _temp_dir) = create_test_config_with_repos().await;
        
        // First add a dependency
        let add_params = RepositoryDependencyParams {
            repository_name: "test-main-app".to_string(),
            dependency_name: "test-lib-a".to_string(),
            target_ref: Some("v1.0.0".to_string()),
            purpose: None,
        };
        handle_repository_add_dependency(add_params, config.clone()).await.unwrap();
        
        // Remove it
        let remove_params = RepositoryDependencyParams {
            repository_name: "test-main-app".to_string(),
            dependency_name: "test-lib-a".to_string(),
            target_ref: None,
            purpose: None,
        };
        let result = handle_repository_remove_dependency(remove_params, config.clone()).await.unwrap();
        assert!(result.success);
        
        // Verify removal
        let config_read = config.read().await;
        let main_app = config_read.repositories.iter()
            .find(|r| r.name == "test-main-app")
            .unwrap();
        assert_eq!(main_app.dependencies.len(), 0);
    }

    #[tokio::test]
    async fn test_remove_nonexistent_dependency() {
        let _guard = TestGuard;
        let (config, _temp_dir) = create_test_config_with_repos().await;
        
        let params = RepositoryDependencyParams {
            repository_name: "test-main-app".to_string(),
            dependency_name: "test-lib-a".to_string(),
            target_ref: None,
            purpose: None,
        };
        
        let result = handle_repository_remove_dependency(params, config).await.unwrap();
        assert!(!result.success);
        assert!(result.message.contains("not found"));
    }

    #[tokio::test]
    async fn test_list_dependencies() {
        let _guard = TestGuard;
        let (config, _temp_dir) = create_test_config_with_repos().await;
        
        // Add multiple dependencies
        let params1 = RepositoryDependencyParams {
            repository_name: "test-main-app".to_string(),
            dependency_name: "test-lib-a".to_string(),
            target_ref: Some("v1.0.0".to_string()),
            purpose: Some("Authentication library".to_string()),
        };
        handle_repository_add_dependency(params1, config.clone()).await.unwrap();
        
        let params2 = RepositoryDependencyParams {
            repository_name: "test-main-app".to_string(),
            dependency_name: "test-lib-b".to_string(),
            target_ref: None,
            purpose: Some("Database connector".to_string()),
        };
        handle_repository_add_dependency(params2, config.clone()).await.unwrap();
        
        // List dependencies
        let list_params = RepositoryListDependenciesParams {
            repository_name: "test-main-app".to_string(),
        };
        let result = handle_repository_list_dependencies(list_params, config).await.unwrap();
        
        assert_eq!(result.repository_name, "test-main-app");
        assert_eq!(result.dependencies.len(), 2);
        
        // Check first dependency
        let dep_a = result.dependencies.iter()
            .find(|d| d.repository_name == "test-lib-a")
            .unwrap();
        assert_eq!(dep_a.target_ref, Some("v1.0.0".to_string()));
        assert_eq!(dep_a.purpose, Some("Authentication library".to_string()));
        assert!(dep_a.is_available);
        assert_eq!(dep_a.current_ref, Some("main".to_string())); // From test data
        
        // Check second dependency
        let dep_b = result.dependencies.iter()
            .find(|d| d.repository_name == "test-lib-b")
            .unwrap();
        assert_eq!(dep_b.target_ref, None);
        assert_eq!(dep_b.purpose, Some("Database connector".to_string()));
        assert!(dep_b.is_available);
        assert_eq!(dep_b.current_ref, Some("develop".to_string())); // From test data
    }

    #[tokio::test]
    async fn test_list_dependencies_empty() {
        let _guard = TestGuard;
        let (config, _temp_dir) = create_test_config_with_repos().await;
        
        let params = RepositoryListDependenciesParams {
            repository_name: "test-main-app".to_string(),
        };
        
        let result = handle_repository_list_dependencies(params, config).await.unwrap();
        assert_eq!(result.repository_name, "test-main-app");
        assert_eq!(result.dependencies.len(), 0);
    }

    #[tokio::test]
    async fn test_self_dependency_allowed() {
        let _guard = TestGuard;
        let (config, _temp_dir) = create_test_config_with_repos().await;
        
        // Test that a repository can depend on itself (though unusual)
        let params = RepositoryDependencyParams {
            repository_name: "test-lib-a".to_string(),
            dependency_name: "test-lib-a".to_string(),
            target_ref: Some("v0.9.0".to_string()),
            purpose: Some("Self reference for testing".to_string()),
        };
        
        let result = handle_repository_add_dependency(params, config.clone()).await.unwrap();
        assert!(result.success);
        
        // Verify it was added
        let list_params = RepositoryListDependenciesParams {
            repository_name: "test-lib-a".to_string(),
        };
        let list_result = handle_repository_list_dependencies(list_params, config).await.unwrap();
        assert_eq!(list_result.dependencies.len(), 1);
        assert_eq!(list_result.dependencies[0].repository_name, "test-lib-a");
    }

    #[tokio::test]
    async fn test_multiple_repositories_with_dependencies() {
        let _guard = TestGuard;
        let (config, _temp_dir) = create_test_config_with_repos().await;
        
        // main-app depends on lib-a and lib-b
        let params1 = RepositoryDependencyParams {
            repository_name: "test-main-app".to_string(),
            dependency_name: "test-lib-a".to_string(),
            target_ref: Some("v1.0.0".to_string()),
            purpose: None,
        };
        handle_repository_add_dependency(params1, config.clone()).await.unwrap();
        
        let params2 = RepositoryDependencyParams {
            repository_name: "test-main-app".to_string(),
            dependency_name: "test-lib-b".to_string(),
            target_ref: None,
            purpose: None,
        };
        handle_repository_add_dependency(params2, config.clone()).await.unwrap();
        
        // lib-b depends on lib-a (transitive dependency)
        let params3 = RepositoryDependencyParams {
            repository_name: "test-lib-b".to_string(),
            dependency_name: "test-lib-a".to_string(),
            target_ref: Some("v0.9.0".to_string()),
            purpose: Some("Shared utilities".to_string()),
        };
        handle_repository_add_dependency(params3, config.clone()).await.unwrap();
        
        // Verify main-app dependencies
        let list_main = RepositoryListDependenciesParams {
            repository_name: "test-main-app".to_string(),
        };
        let result_main = handle_repository_list_dependencies(list_main, config.clone()).await.unwrap();
        assert_eq!(result_main.dependencies.len(), 2);
        
        // Verify lib-b dependencies
        let list_lib_b = RepositoryListDependenciesParams {
            repository_name: "test-lib-b".to_string(),
        };
        let result_lib_b = handle_repository_list_dependencies(list_lib_b, config).await.unwrap();
        assert_eq!(result_lib_b.dependencies.len(), 1);
        assert_eq!(result_lib_b.dependencies[0].repository_name, "test-lib-a");
        assert_eq!(result_lib_b.dependencies[0].target_ref, Some("v0.9.0".to_string()));
    }
}