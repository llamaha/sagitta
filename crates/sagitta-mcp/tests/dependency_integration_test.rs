// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// WARNING: NEVER EVER CALL save_config() IN TESTS!
// 
// This has been a recurring issue where tests overwrite user configuration files.
// This is the 5th+ time this has been fixed. DO NOT REINTRODUCE THIS BUG.
//
// Rules for config tests:
// 1. NEVER call save_config() directly
// 2. NEVER save configs to disk in tests
// 3. ALWAYS use in-memory configs only
// 4. ALWAYS use temp directories for any file operations
// 5. The handlers may call save_config internally - that's protected by SAGITTA_TEST_MODE
//
// If you're thinking "but I need to test saving config":
// - No, you don't. The handlers test that internally.
// - If you really need to test file I/O, use a temp directory and custom path
// - But seriously, you probably don't need to test that
//
// Breaking this rule will overwrite real user configs and cause data loss!
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

use std::sync::Arc;
use tokio::sync::RwLock;
use tempfile::TempDir;

use sagitta_mcp::handlers::dependency::{
    handle_repository_add_dependency,
    handle_repository_remove_dependency, 
    handle_repository_list_dependencies,
};
use sagitta_mcp::mcp::types::{
    RepositoryDependencyParams,
    RepositoryListDependenciesParams,
};
use sagitta_search::config::{AppConfig, RepositoryConfig, RepositoryDependency};

fn create_test_config_with_repos() -> (Arc<RwLock<AppConfig>>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();
    
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
                last_synced_commit: None,
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
                last_synced_commit: None,
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
                last_synced_commit: None,
            },
        ],
        repositories_base_path: Some(base_path.to_string_lossy().into_owned()),
        vocabulary_base_path: Some(base_path.to_string_lossy().into_owned()),
        ..Default::default()
    };
    
    (Arc::new(RwLock::new(config)), temp_dir)
}

#[tokio::test]
async fn test_add_dependency_success() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_repos();
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let result = handle_repository_add_dependency(params, config.clone()).await;
    assert!(result.is_ok());
    
    // Verify the dependency was added
    let config_guard = config.read().await;
    let main_repo = config_guard.repositories.iter()
        .find(|r| r.name == "main-repo")
        .unwrap();
    
    assert_eq!(main_repo.dependencies.len(), 1);
    assert_eq!(main_repo.dependencies[0].repository_name, "dep-repo-1");
}

#[tokio::test]
async fn test_add_dependency_not_found() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_repos();
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "non-existent-repo".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let result = handle_repository_add_dependency(params, config).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Dependency repository 'non-existent-repo' not found"));
}

#[tokio::test]
async fn test_add_dependency_duplicate() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_repos();
    
    // First add a dependency
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let result = handle_repository_add_dependency(params, config.clone()).await;
    assert!(result.is_ok());
    
    // Try to add the same dependency again
    let params2 = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let result2 = handle_repository_add_dependency(params2, config).await;
    assert!(result2.is_err());
    assert!(result2.unwrap_err().to_string().contains("already depends on"));
}

#[tokio::test]
async fn test_remove_dependency_success() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_repos();
    
    // First add a dependency
    let add_params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let add_result = handle_repository_add_dependency(add_params, config.clone()).await;
    assert!(add_result.is_ok());
    
    // Now remove it
    let remove_params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let remove_result = handle_repository_remove_dependency(remove_params, config.clone()).await;
    assert!(remove_result.is_ok());
    
    // Verify the dependency was removed
    let config_guard = config.read().await;
    let main_repo = config_guard.repositories.iter()
        .find(|r| r.name == "main-repo")
        .unwrap();
    
    assert_eq!(main_repo.dependencies.len(), 0);
}

#[tokio::test]
async fn test_remove_dependency_not_found() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_repos();
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let result = handle_repository_remove_dependency(params, config).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("does not have a dependency on"));
}

#[tokio::test]
async fn test_list_dependencies() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_repos();
    
    // Add some dependencies
    let params1 = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: Some("v1.0".to_string()),
        purpose: Some("Utilities library".to_string()),
    };
    
    handle_repository_add_dependency(params1, config.clone()).await.unwrap();
    
    let params2 = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-2".to_string(),
        target_ref: None,
        purpose: Some("Test framework".to_string()),
    };
    
    handle_repository_add_dependency(params2, config.clone()).await.unwrap();
    
    // List dependencies
    let list_params = RepositoryListDependenciesParams {
        repository_name: "main-repo".to_string(),
    };
    
    let result = handle_repository_list_dependencies(list_params, config).await.unwrap();
    
    assert_eq!(result.dependencies.len(), 2);
    
    let dep1 = result.dependencies.iter()
        .find(|d| d.repository_name == "dep-repo-1")
        .unwrap();
    assert_eq!(dep1.repository_name, "dep-repo-1");
    assert_eq!(dep1.target_ref, Some("v1.0".to_string()));
    assert_eq!(dep1.purpose, Some("Utilities library".to_string()));
    
    let dep2 = result.dependencies.iter()
        .find(|d| d.repository_name == "dep-repo-2")
        .unwrap();
    assert_eq!(dep2.repository_name, "dep-repo-2");
    assert_eq!(dep2.target_ref, None);
    assert_eq!(dep2.purpose, Some("Test framework".to_string()));
}

#[tokio::test]
async fn test_cross_dependency_prevention() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_repos();
    
    // First add dep-repo-1 as dependency of main-repo
    let params1 = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    handle_repository_add_dependency(params1, config.clone()).await.unwrap();
    
    // Now try to add main-repo as dependency of dep-repo-1 (should fail)
    let params2 = RepositoryDependencyParams {
        repository_name: "dep-repo-1".to_string(),
        dependency_name: "main-repo".to_string(),
        target_ref: None,
        purpose: None,
    };
    
    let result = handle_repository_add_dependency(params2, config).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("would create a circular dependency"));
}

#[tokio::test]
async fn test_dependency_with_target_ref() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_repos();
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: Some("v1.2.3".to_string()),
        purpose: None,
    };
    
    let result = handle_repository_add_dependency(params, config.clone()).await;
    assert!(result.is_ok());
    
    // Verify the dependency was added with target ref
    let config_guard = config.read().await;
    let main_repo = config_guard.repositories.iter()
        .find(|r| r.name == "main-repo")
        .unwrap();
    
    assert_eq!(main_repo.dependencies.len(), 1);
    assert_eq!(main_repo.dependencies[0].target_ref, Some("v1.2.3".to_string()));
}

#[tokio::test]
async fn test_dependency_with_purpose() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_repos();
    
    let params = RepositoryDependencyParams {
        repository_name: "main-repo".to_string(),
        dependency_name: "dep-repo-1".to_string(),
        target_ref: None,
        purpose: Some("Authentication library".to_string()),
    };
    
    let result = handle_repository_add_dependency(params, config.clone()).await;
    assert!(result.is_ok());
    
    // Verify the dependency was added with purpose
    let config_guard = config.read().await;
    let main_repo = config_guard.repositories.iter()
        .find(|r| r.name == "main-repo")
        .unwrap();
    
    assert_eq!(main_repo.dependencies.len(), 1);
    assert_eq!(main_repo.dependencies[0].purpose, Some("Authentication library".to_string()));
}