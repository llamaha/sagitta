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

use sagitta_mcp::handlers::repository::handle_repository_list;
use sagitta_mcp::mcp::types::RepositoryListParams;
use sagitta_search::config::{AppConfig, RepositoryConfig, RepositoryDependency};

fn create_test_config_with_dependencies() -> (Arc<RwLock<AppConfig>>, TempDir) {
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
                dependencies: vec![],
            },
        ],
        repositories_base_path: Some(base_path.to_string_lossy().into_owned()),
        vocabulary_base_path: Some(base_path.to_string_lossy().into_owned()),
        ..Default::default()
    };
    
    (Arc::new(RwLock::new(config)), temp_dir)
}


#[tokio::test]
async fn test_repository_list_includes_dependencies() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_dependencies();
    
    let params = RepositoryListParams {};
    
    let result = handle_repository_list(params, config, None).await.unwrap();
    
    assert_eq!(result.repositories.len(), 3);
    
    // Find main repo and check its dependencies
    let main_repo = result.repositories.iter()
        .find(|r| r.name == "main-repo")
        .expect("main-repo not found");
    
    assert_eq!(main_repo.dependencies.len(), 2);
    
    // Check first dependency
    let dep1 = main_repo.dependencies.iter()
        .find(|d| d.repository_name == "dep-repo-1")
        .expect("dep-repo-1 dependency not found");
    assert_eq!(dep1.target_ref, Some("v1.0".to_string()));
    assert_eq!(dep1.purpose, Some("Utilities library".to_string()));
    
    // Check second dependency
    let dep2 = main_repo.dependencies.iter()
        .find(|d| d.repository_name == "dep-repo-2")
        .expect("dep-repo-2 dependency not found");
    assert_eq!(dep2.target_ref, None);
    assert_eq!(dep2.purpose, Some("Test framework".to_string()));
}

#[tokio::test]
async fn test_repository_list_verbose_mode() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_dependencies();
    
    let params = RepositoryListParams {};
    
    let result = handle_repository_list(params, config, None).await.unwrap();
    
    // Check that repositories have expected fields
    for repo in &result.repositories {
        assert!(!repo.name.is_empty());
        assert!(!repo.remote.is_empty());
        // branch and last_updated are optional
    }
}

#[tokio::test]
async fn test_repository_list_non_verbose_mode() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_dependencies();
    
    let params = RepositoryListParams {};
    
    let result = handle_repository_list(params, config, None).await.unwrap();
    
    // In non-verbose mode, repository names should still be returned
    assert_eq!(result.repositories.len(), 3);
    assert!(result.repositories.iter().any(|r| r.name == "main-repo"));
    assert!(result.repositories.iter().any(|r| r.name == "dep-repo-1"));
    assert!(result.repositories.iter().any(|r| r.name == "dep-repo-2"));
}

#[tokio::test]
async fn test_repository_list_dependency_details() {
    std::env::set_var("SAGITTA_TEST_MODE", "1");
    let (config, _temp_dir) = create_test_config_with_dependencies();
    
    let params = RepositoryListParams {};
    
    let result = handle_repository_list(params, config, None).await.unwrap();
    
    let main_repo = result.repositories.iter()
        .find(|r| r.name == "main-repo")
        .unwrap();
    
    // Verify dependencies contain all fields
    for dep in &main_repo.dependencies {
        assert!(!dep.repository_name.is_empty());
        // target_ref and purpose are optional, but should match what we set
    }
}