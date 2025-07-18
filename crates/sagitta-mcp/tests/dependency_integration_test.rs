use std::sync::Arc;
use tokio::sync::RwLock;
use tempfile::TempDir;
use std::path::PathBuf;
use std::env;

use sagitta_mcp::handlers::dependency::{
    handle_repository_add_dependency,
    handle_repository_remove_dependency, 
    handle_repository_list_dependencies,
};
use sagitta_mcp::mcp::types::{
    RepositoryDependencyParams,
    RepositoryListDependenciesParams,
};
use sagitta_search::config::{AppConfig, RepositoryConfig, RepositoryDependency, get_config_path_or_default, save_config};

/// Helper to create an isolated test environment with proper config handling
struct TestEnvironment {
    _temp_dir: TempDir,
    original_config_path: Option<String>,
    test_config_path: PathBuf,
}

impl TestEnvironment {
    fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        
        // Save original config path if set
        let original_config_path = env::var("SAGITTA_CONFIG_PATH").ok();
        
        // Set up test config path
        let test_config_path = base_path.join("test_config.toml");
        env::set_var("SAGITTA_CONFIG_PATH", &test_config_path);
        
        // CRITICAL: Set test mode to prevent overwriting real config
        env::set_var("SAGITTA_TEST_MODE", "1");
        
        Self {
            _temp_dir: temp_dir,
            original_config_path,
            test_config_path,
        }
    }
    
    fn create_test_config(&self) -> AppConfig {
        let base_path = self._temp_dir.path().to_path_buf();
        
        let config = AppConfig {
            repositories: vec![
                RepositoryConfig {
                    name: "main-repo".to_string(),
                    url: "https://github.com/user/main-repo.git".to_string(),
                    local_path: base_path.join("main-repo"),
                    default_branch: "main".to_string(),
                    tracked_branches: vec!["main".to_string()],
                    remote_name: Some("origin".to_string()),
                    last_synced_commits: Default::default(),
                    active_branch: Some("main".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    indexed_languages: None,
                    added_as_local_path: false,
                    target_ref: None,
                    dependencies: vec![],
                },
                RepositoryConfig {
                    name: "dep-repo-1".to_string(),
                    url: "https://github.com/user/dep-repo-1.git".to_string(),
                    local_path: base_path.join("dep-repo-1"),
                    default_branch: "main".to_string(),
                    tracked_branches: vec!["main".to_string()],
                    remote_name: Some("origin".to_string()),
                    last_synced_commits: Default::default(),
                    active_branch: Some("main".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    indexed_languages: None,
                    added_as_local_path: false,
                    target_ref: None,
                    dependencies: vec![],
                },
                RepositoryConfig {
                    name: "dep-repo-2".to_string(),
                    url: "https://github.com/user/dep-repo-2.git".to_string(),
                    local_path: base_path.join("dep-repo-2"),
                    default_branch: "main".to_string(),
                    tracked_branches: vec!["v2.0".to_string()],
                    remote_name: Some("origin".to_string()),
                    last_synced_commits: Default::default(),
                    active_branch: Some("v2.0".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    indexed_languages: None,
                    added_as_local_path: false,
                    target_ref: None,
                    dependencies: vec![],
                },
            ],
            ..Default::default()
        };
        
        // Save the test config to isolated path
        save_config(&config, Some(&self.test_config_path)).unwrap();
        config
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        // Restore original config path
        match &self.original_config_path {
            Some(path) => env::set_var("SAGITTA_CONFIG_PATH", path),
            None => env::remove_var("SAGITTA_CONFIG_PATH"),
        }
        
        // Clean up test mode
        env::remove_var("SAGITTA_TEST_MODE");
    }
}

#[tokio::test]
async fn test_add_dependency_success() {
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config()));
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: Some("v1.0".to_string()),
        purpose: Some("Utilities library".to_string()),
    };
    
    let result = handle_repository_add_dependency(params, config.clone()).await;
    assert!(result.is_ok());
    
    let result = result.unwrap();
    assert!(result.success);
    assert!(result.message.contains("Successfully added dependency"));
    
    // Verify dependency was actually added
    let config_guard = config.read().await;
    let main_repo = config_guard.repositories.iter()
        .find(|r| r.name == "main-repo")
        .unwrap();
    
    assert_eq!(main_repo.dependencies.len(), 1);
    assert_eq!(main_repo.dependencies[0].repository_name, "dep-repo-1");
    assert_eq!(main_repo.dependencies[0].target_ref, Some("v1.0".to_string()));
    assert_eq!(main_repo.dependencies[0].purpose, Some("Utilities library".to_string()));
}

#[tokio::test]
async fn test_add_dependency_nonexistent_main_repo() {
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config()));
    
    let params = RepositoryDependencyParams {
        repository_name: "nonexistent-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let result = handle_repository_add_dependency(params, config).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Repository 'nonexistent-repo' not found"));
}

#[tokio::test]
async fn test_add_dependency_nonexistent_dependency_repo() {
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config()));
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "nonexistent-dep".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let result = handle_repository_add_dependency(params, config).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Dependency repository 'nonexistent-dep' not found"));
}

#[tokio::test]
async fn test_add_dependency_update_existing() {
    let test_env = TestEnvironment::new();
    let mut config = test_env.create_test_config();
    // Pre-add a dependency
    config.repositories[0].dependencies.push(RepositoryDependency {
        repository_name: "dep-repo-1".to_string(),
        target_ref: Some("old-version".to_string()),
        purpose: Some("Old purpose".to_string()),
    });
    
    let config = Arc::new(RwLock::new(config));
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: Some("v2.0".to_string()),
        purpose: Some("Updated utilities library".to_string()),
    };
    
    let result = handle_repository_add_dependency(params, config.clone()).await;
    assert!(result.is_ok());
    
    let result = result.unwrap();
    assert!(result.success);
    assert!(result.message.contains("Successfully added dependency"));
    
    // Verify dependency was updated, not duplicated
    let config_guard = config.read().await;
    let main_repo = config_guard.repositories.iter()
        .find(|r| r.name == "main-repo")
        .unwrap();
    
    assert_eq!(main_repo.dependencies.len(), 1);
    assert_eq!(main_repo.dependencies[0].target_ref, Some("v2.0".to_string()));
    assert_eq!(main_repo.dependencies[0].purpose, Some("Updated utilities library".to_string()));
}

#[tokio::test]
async fn test_add_dependency_self_reference() {
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config()));
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "main-repo".to_string(), // Self-reference
        target_ref: None,
        purpose: None,
    };
    
    let _result = handle_repository_add_dependency(params, config).await;
    // This should either be rejected or handled gracefully
    // Currently the code doesn't prevent self-references - this is a bug!
    // The test will help us identify this issue
}

#[tokio::test] 
async fn test_remove_dependency_success() {
    let test_env = TestEnvironment::new();
    let mut config = test_env.create_test_config();
    // Pre-add dependencies
    config.repositories[0].dependencies = vec![
        RepositoryDependency {
            repository_name: "dep-repo-1".to_string(),
            target_ref: Some("v1.0".to_string()),
            purpose: Some("Utils".to_string()),
        },
        RepositoryDependency {
            repository_name: "dep-repo-2".to_string(),
            target_ref: None,
            purpose: None,
        },
    ];
    
    let config = Arc::new(RwLock::new(config));
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: None, // These fields are not used for removal
        purpose: None,
    };
    
    let result = handle_repository_remove_dependency(params, config.clone()).await;
    assert!(result.is_ok());
    
    let result = result.unwrap();
    assert!(result.success);
    assert!(result.message.contains("Successfully removed dependency"));
    
    // Verify dependency was removed
    let config_guard = config.read().await;
    let main_repo = config_guard.repositories.iter()
        .find(|r| r.name == "main-repo")
        .unwrap();
    
    assert_eq!(main_repo.dependencies.len(), 1);
    assert_eq!(main_repo.dependencies[0].repository_name, "dep-repo-2");
}

#[tokio::test]
async fn test_remove_dependency_nonexistent() {
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config()));
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "nonexistent-dep".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let result = handle_repository_remove_dependency(params, config).await;
    assert!(result.is_ok());
    
    let result = result.unwrap();
    assert!(!result.success);
    assert!(result.message.contains("Dependency 'nonexistent-dep' not found"));
}

#[tokio::test]
async fn test_remove_dependency_nonexistent_repo() {
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config()));
    
    let params = RepositoryDependencyParams {
        repository_name: "nonexistent-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let result = handle_repository_remove_dependency(params, config).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Repository 'nonexistent-repo' not found"));
}

#[tokio::test]
async fn test_list_dependencies_success() {
    let test_env = TestEnvironment::new();
    let mut config = test_env.create_test_config();
    // Pre-add dependencies
    config.repositories[0].dependencies = vec![
        RepositoryDependency {
            repository_name: "dep-repo-1".to_string(),
            target_ref: Some("v1.0".to_string()),
            purpose: Some("Utilities".to_string()),
        },
        RepositoryDependency {
            repository_name: "dep-repo-2".to_string(),
            target_ref: None,
            purpose: None,
        },
    ];
    
    let config = Arc::new(RwLock::new(config));
    
    let params = RepositoryListDependenciesParams {
        repository_name: "main-repo".to_string(),
    };
    
    let result = handle_repository_list_dependencies(params, config).await;
    assert!(result.is_ok());
    
    let result = result.unwrap();
    assert_eq!(result.repository_name, "main-repo");
    assert_eq!(result.dependencies.len(), 2);
    
    // Check first dependency
    let dep1 = &result.dependencies[0];
    assert_eq!(dep1.repository_name, "dep-repo-1");
    assert_eq!(dep1.target_ref, Some("v1.0".to_string()));
    assert_eq!(dep1.purpose, Some("Utilities".to_string()));
    assert!(dep1.is_available);
    assert!(dep1.local_path.is_some());
    assert_eq!(dep1.current_ref, Some("main".to_string()));
    
    // Check second dependency
    let dep2 = &result.dependencies[1];
    assert_eq!(dep2.repository_name, "dep-repo-2");
    assert_eq!(dep2.target_ref, None);
    assert_eq!(dep2.purpose, None);
    assert!(dep2.is_available);
    assert!(dep2.local_path.is_some());
    assert_eq!(dep2.current_ref, Some("v2.0".to_string()));
}

#[tokio::test]
async fn test_list_dependencies_unavailable_dependency() {
    let test_env = TestEnvironment::new();
    let mut config = test_env.create_test_config();
    // Add dependency to a repository that doesn't exist
    config.repositories[0].dependencies = vec![
        RepositoryDependency {
            repository_name: "missing-repo".to_string(),
            target_ref: Some("v1.0".to_string()),
            purpose: Some("Missing dependency".to_string()),
        },
    ];
    
    let config = Arc::new(RwLock::new(config));
    
    let params = RepositoryListDependenciesParams {
        repository_name: "main-repo".to_string(),
    };
    
    let result = handle_repository_list_dependencies(params, config).await;
    assert!(result.is_ok());
    
    let result = result.unwrap();
    assert_eq!(result.dependencies.len(), 1);
    
    let dep = &result.dependencies[0];
    assert_eq!(dep.repository_name, "missing-repo");
    assert!(!dep.is_available);
    assert!(dep.local_path.is_none());
    assert!(dep.current_ref.is_none());
}

#[tokio::test]
async fn test_list_dependencies_empty() {
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config()));
    
    let params = RepositoryListDependenciesParams {
        repository_name: "main-repo".to_string(),
    };
    
    let result = handle_repository_list_dependencies(params, config).await;
    assert!(result.is_ok());
    
    let result = result.unwrap();
    assert_eq!(result.repository_name, "main-repo");
    assert_eq!(result.dependencies.len(), 0);
}

#[tokio::test]
async fn test_list_dependencies_nonexistent_repo() {
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config()));
    
    let params = RepositoryListDependenciesParams {
        repository_name: "nonexistent-repo".to_string(),
    };
    
    let result = handle_repository_list_dependencies(params, config).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Repository 'nonexistent-repo' not found"));
}

#[tokio::test] 
async fn test_dependency_circular_detection() {
    let test_env = TestEnvironment::new();
    let mut config = test_env.create_test_config();
    
    // Create a circular dependency scenario
    config.repositories[0].dependencies = vec![
        RepositoryDependency {
            repository_name: "dep-repo-1".to_string(),
            target_ref: None,
            purpose: None,
        },
    ];
    
    config.repositories[1].dependencies = vec![
        RepositoryDependency {
            repository_name: "dep-repo-2".to_string(),
            target_ref: None,
            purpose: None,
        },
    ];
    
    config.repositories[2].dependencies = vec![
        RepositoryDependency {
            repository_name: "main-repo".to_string(), // Creates circular dependency
            target_ref: None,
            purpose: None,
        },
    ];
    
    let config = Arc::new(RwLock::new(config));
    
    // The current implementation doesn't detect circular dependencies
    // This test will help us identify this missing feature
    
    let params = RepositoryListDependenciesParams {
        repository_name: "main-repo".to_string(),
    };
    
    let result = handle_repository_list_dependencies(params, config).await;
    assert!(result.is_ok());
    
    // TODO: Add circular dependency detection logic
    // For now, this just documents the issue
}

#[tokio::test]
async fn test_concurrent_dependency_operations() {
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config()));
    
    // Test concurrent add operations
    let config1 = config.clone();
    let config2 = config.clone();
    
    let add_task1 = tokio::spawn(async move {
        let params = RepositoryDependencyParams {
            repository_name: "main-repo".to_string(),
            dependency_name: "dep-repo-1".to_string(),
            target_ref: Some("v1.0".to_string()),
            purpose: Some("First dependency".to_string()),
        };
        handle_repository_add_dependency(params, config1).await
    });
    
    let add_task2 = tokio::spawn(async move {
        let params = RepositoryDependencyParams {
            repository_name: "main-repo".to_string(),
            dependency_name: "dep-repo-2".to_string(),
            target_ref: Some("v2.0".to_string()),
            purpose: Some("Second dependency".to_string()),
        };
        handle_repository_add_dependency(params, config2).await
    });
    
    let (result1, result2) = tokio::join!(add_task1, add_task2);
    
    assert!(result1.unwrap().is_ok());
    assert!(result2.unwrap().is_ok());
    
    // Verify both dependencies were added
    let config_guard = config.read().await;
    let main_repo = config_guard.repositories.iter()
        .find(|r| r.name == "main-repo")
        .unwrap();
    
    assert_eq!(main_repo.dependencies.len(), 2);
}

#[tokio::test]
async fn test_edge_case_empty_strings() {
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config()));
    
    let params = RepositoryDependencyParams {
        repository_name: "".to_string(), // Empty string
        dependency_name: "dep-repo-1".to_string(),
        target_ref: Some("".to_string()), // Empty target ref
        purpose: Some("".to_string()),    // Empty purpose
    };
    
    let result = handle_repository_add_dependency(params, config).await;
    // The current implementation might not handle empty strings properly
    // This test will help us identify the issue
    assert!(result.is_err());
}

#[tokio::test]
async fn test_edge_case_very_long_strings() {
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config()));
    
    let long_string = "a".repeat(10000); // Very long string
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: Some(long_string.clone()),
        purpose: Some(long_string),
    };
    
    let result = handle_repository_add_dependency(params, config).await;
    // Test how the system handles very long strings
    assert!(result.is_ok());
}