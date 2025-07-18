use std::sync::Arc;
use tokio::sync::RwLock;
use tempfile::TempDir;
use std::path::PathBuf;
use std::env;

use sagitta_mcp::handlers::repository::handle_repository_list;
use sagitta_mcp::mcp::types::RepositoryListParams;
use sagitta_search::config::{AppConfig, RepositoryConfig, RepositoryDependency, save_config};

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
    
    fn create_test_config_with_dependencies(&self) -> AppConfig {
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
                    dependencies: vec![
                        RepositoryDependency {
                            repository_name: "dep-repo-1".to_string(),
                            target_ref: Some("v1.0".to_string()),
                            purpose: Some("Utilities library".to_string()),
                        },
                        RepositoryDependency {
                            repository_name: "dep-repo-2".to_string(),
                            target_ref: None,
                            purpose: Some("Test framework".to_string()),
                        },
                    ],
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
                    dependencies: vec![
                        RepositoryDependency {
                            repository_name: "external-lib".to_string(),
                            target_ref: Some("latest".to_string()),
                            purpose: Some("External dependency".to_string()),
                        },
                    ],
                },
                RepositoryConfig {
                    name: "standalone-repo".to_string(),
                    url: "https://github.com/user/standalone-repo.git".to_string(),
                    local_path: base_path.join("standalone-repo"),
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
                    dependencies: vec![], // No dependencies
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
async fn test_repository_list_current_behavior() {
    // This test documents the current behavior - dependencies are NOT included
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config_with_dependencies()));
    
    let params = RepositoryListParams {};
    let result = handle_repository_list(params, config, None).await;
    
    assert!(result.is_ok());
    let repository_list = result.unwrap();
    
    // Verify we have all repositories
    assert_eq!(repository_list.repositories.len(), 4);
    
    // Check that dependencies are NOT included in the current implementation
    for repo_info in &repository_list.repositories {
        // Current RepositoryInfo struct doesn't have a dependencies field
        // This is the bug we need to fix!
        
        match repo_info.name.as_str() {
            "main-repo" => {
                // main-repo has 2 dependencies but they're not exposed in RepositoryInfo
                assert!(repo_info.description.is_some());
                // Dependencies should be mentioned in description or separate field
            },
            "dep-repo-2" => {
                // dep-repo-2 has 1 dependency but it's not exposed
                assert!(repo_info.description.is_some());
            },
            "standalone-repo" => {
                // This repo has no dependencies - should be clear in output
                assert!(repo_info.description.is_some());
            },
            _ => {},
        }
    }
}

#[tokio::test]
async fn test_repository_list_should_include_dependencies() {
    // This test defines what the behavior SHOULD be after we fix it
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config_with_dependencies()));
    
    let params = RepositoryListParams {};
    let result = handle_repository_list(params, config, None).await;
    
    assert!(result.is_ok());
    let repository_list = result.unwrap();
    
    assert_eq!(repository_list.repositories.len(), 4);
    
    // After fixing, we should be able to access dependency information
    let main_repo = repository_list.repositories.iter()
        .find(|r| r.name == "main-repo")
        .unwrap();
    
    // FIXME: RepositoryInfo should include dependencies field
    // Something like:
    // assert_eq!(main_repo.dependencies.len(), 2);
    // assert!(main_repo.dependencies.iter().any(|d| d.repository_name == "dep-repo-1"));
    
    // For now, check if dependencies are mentioned in description
    if let Some(description) = &main_repo.description {
        // Dependencies should be mentioned somewhere in the description
        // This is a temporary workaround until we add proper dependencies field
        println!("Main repo description: {}", description);
    }
}

#[tokio::test]
async fn test_repository_list_dependency_status() {
    // Test that dependency availability status is reflected in repository list
    let test_env = TestEnvironment::new();
    let mut config = test_env.create_test_config_with_dependencies();
    
    // Add a dependency to a repository that doesn't exist in the config
    config.repositories[0].dependencies.push(RepositoryDependency {
        repository_name: "missing-dependency".to_string(),
        target_ref: Some("v1.0".to_string()),
        purpose: Some("This dependency is not available".to_string()),
    });
    
    let config = Arc::new(RwLock::new(config));
    
    let params = RepositoryListParams {};
    let result = handle_repository_list(params, config, None).await;
    
    assert!(result.is_ok());
    let repository_list = result.unwrap();
    
    let main_repo = repository_list.repositories.iter()
        .find(|r| r.name == "main-repo")
        .unwrap();
    
    // The description should indicate dependency status
    if let Some(description) = &main_repo.description {
        // Should mention available and missing dependencies differently
        println!("Description with missing dependency: {}", description);
        
        // FIXME: Need to enhance repository list to show dependency health
        // Should show: "Dependencies: 2 available, 1 missing" or similar
    }
}

#[tokio::test]
async fn test_repository_list_performance_with_many_dependencies() {
    // Test performance with many dependencies
    let test_env = TestEnvironment::new();
    let mut config = test_env.create_test_config_with_dependencies();
    
    // Add many dependencies to test performance
    for i in 0..100 {
        config.repositories[0].dependencies.push(RepositoryDependency {
            repository_name: format!("dep-{}", i),
            target_ref: Some(format!("v{}.0", i)),
            purpose: Some(format!("Dependency number {}", i)),
        });
    }
    
    let config = Arc::new(RwLock::new(config));
    
    let start = std::time::Instant::now();
    let params = RepositoryListParams {};
    let result = handle_repository_list(params, config, None).await;
    let duration = start.elapsed();
    
    assert!(result.is_ok());
    
    // Repository list should complete quickly even with many dependencies
    assert!(duration.as_millis() < 1000, "Repository list took too long: {:?}", duration);
    
    println!("Repository list with 100 dependencies took: {:?}", duration);
}

#[tokio::test] 
async fn test_repository_list_dependency_circular_reference_handling() {
    // Test how repository list handles circular dependencies
    let test_env = TestEnvironment::new();
    let mut config = test_env.create_test_config_with_dependencies();
    
    // Create circular dependency
    config.repositories[1].dependencies.push(RepositoryDependency {
        repository_name: "main-repo".to_string(),
        target_ref: None,
        purpose: Some("Circular reference".to_string()),
    });
    
    let config = Arc::new(RwLock::new(config));
    
    let params = RepositoryListParams {};
    let result = handle_repository_list(params, config, None).await;
    
    // Should not crash or hang with circular dependencies
    assert!(result.is_ok());
    
    let repository_list = result.unwrap();
    assert_eq!(repository_list.repositories.len(), 4);
    
    // FIXME: Should detect and warn about circular dependencies
}

#[tokio::test]
async fn test_repository_list_dependency_info_consistency() {
    // Test that dependency information is consistent across different APIs
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config_with_dependencies()));
    
    // Get repository list
    let list_params = RepositoryListParams {};
    let list_result = handle_repository_list(list_params, config.clone(), None).await;
    assert!(list_result.is_ok());
    
    // Get detailed dependency list for main-repo  
    use sagitta_mcp::handlers::dependency::handle_repository_list_dependencies;
    use sagitta_mcp::mcp::types::RepositoryListDependenciesParams;
    
    let dep_params = RepositoryListDependenciesParams {
        repository_name: "main-repo".to_string(),
    };
    let dep_result = handle_repository_list_dependencies(dep_params, config).await;
    assert!(dep_result.is_ok());
    
    let detailed_deps = dep_result.unwrap();
    
    // FIXME: Repository list and dependency list should be consistent
    // The number of dependencies should match between APIs
    assert_eq!(detailed_deps.dependencies.len(), 2);
    
    // After we fix repository list to include dependencies:
    // let main_repo = list_result.unwrap().repositories.iter()
    //     .find(|r| r.name == "main-repo").unwrap();
    // assert_eq!(main_repo.dependencies.len(), detailed_deps.dependencies.len());
}

#[tokio::test]
async fn test_repository_list_dependency_metadata() {
    // Test that dependency metadata is properly included
    let test_env = TestEnvironment::new();
    let config = Arc::new(RwLock::new(test_env.create_test_config_with_dependencies()));
    
    let params = RepositoryListParams {};
    let result = handle_repository_list(params, config, None).await;
    
    assert!(result.is_ok());
    let repository_list = result.unwrap();
    
    for repo in &repository_list.repositories {
        println!("Repository: {}", repo.name);
        if let Some(description) = &repo.description {
            println!("  Description: {}", description);
        }
        
        // FIXME: After enhancement, should also print:
        // if let Some(dependencies) = &repo.dependencies {
        //     println!("  Dependencies: {}", dependencies.len());
        //     for dep in dependencies {
        //         println!("    - {} ({})", dep.repository_name, 
        //                  dep.target_ref.as_deref().unwrap_or("latest"));
        //     }
        // }
    }
}