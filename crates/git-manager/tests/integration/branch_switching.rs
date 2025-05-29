use git_manager::{GitManager, GitRepository, SwitchOptions};
use std::fs;
use tempfile::TempDir;
use git2::{Repository, Signature};

/// Helper function to create a test repository with multiple branches and commits
fn create_test_repo_with_branches() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();
    
    // Initialize repository
    let repo = Repository::init(&repo_path).unwrap();
    let signature = Signature::now("Test User", "test@example.com").unwrap();
    
    // Create initial commit on main branch
    fs::write(repo_path.join("README.md"), "# Test Repository\n\nInitial content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("README.md")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    
    let initial_commit = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    ).unwrap();
    
    // Create feature branch
    let initial_commit_obj = repo.find_commit(initial_commit).unwrap();
    repo.branch("feature", &initial_commit_obj, false).unwrap();
    
    // Switch to feature branch and add a commit
    repo.set_head("refs/heads/feature").unwrap();
    repo.checkout_head(Some(
        git2::build::CheckoutBuilder::new()
            .safe()
            .force()
    )).unwrap();
    
    fs::write(repo_path.join("feature.txt"), "Feature content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("feature.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Add feature file",
        &tree,
        &[&initial_commit_obj],
    ).unwrap();
    
    // Switch back to main and ensure clean state
    repo.set_head("refs/heads/main").unwrap();
    repo.checkout_head(Some(
        git2::build::CheckoutBuilder::new()
            .safe()
            .force()
    )).unwrap();
    
    // Ensure index is clean
    let mut index = repo.index().unwrap();
    index.read(true).unwrap();
    
    (temp_dir, repo_path)
}

#[tokio::test]
async fn test_initialize_repository() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    let info = manager.initialize_repository(&repo_path).await.unwrap();
    
    assert_eq!(info.current_branch, "main");
    assert!(!info.current_commit.is_empty());
    assert!(info.is_clean);
    assert_eq!(info.path, repo_path);
}

#[tokio::test]
async fn test_basic_branch_switching() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    // Initialize repository
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Switch to feature branch
    let result = manager.switch_branch(&repo_path, "feature").await.unwrap();
    
    assert!(result.success);
    assert_eq!(result.previous_branch, "main");
    assert_eq!(result.new_branch, "feature");
    
    // Verify we're on the feature branch
    let info = manager.get_repository_info(&repo_path).unwrap();
    assert_eq!(info.current_branch, "feature");
    
    // Verify feature.txt exists (from feature branch)
    assert!(repo_path.join("feature.txt").exists());
}

#[tokio::test]
async fn test_branch_switching_with_file_changes() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    // Initialize repository
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Switch to feature branch
    let result = manager.switch_branch(&repo_path, "feature").await.unwrap();
    assert!(result.success);
    assert!(result.files_changed > 0); // Should detect file changes
    
    // Switch back to main
    let result = manager.switch_branch(&repo_path, "main").await.unwrap();
    assert!(result.success);
    assert_eq!(result.new_branch, "main");
    
    // Verify feature.txt no longer exists (back on main)
    assert!(!repo_path.join("feature.txt").exists());
}

#[tokio::test]
async fn test_branch_switching_with_uncommitted_changes() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    // Initialize repository
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Create uncommitted changes
    fs::write(repo_path.join("uncommitted.txt"), "Uncommitted content").unwrap();
    
    // Try to switch branches - should fail
    let result = manager.switch_branch(&repo_path, "feature").await;
    assert!(result.is_err());
    
    // Try with force option - should succeed
    let options = SwitchOptions {
        force: true,
        ..Default::default()
    };
    let result = manager.switch_branch_with_options(&repo_path, "feature", options).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn test_branch_switching_no_sync() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    // Initialize repository
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Switch with auto_resync disabled
    let options = SwitchOptions {
        auto_resync: false,
        ..Default::default()
    };
    let result = manager.switch_branch_with_options(&repo_path, "feature", options).await.unwrap();
    
    assert!(result.success);
    assert_eq!(result.files_changed, 0); // No sync performed
    assert!(result.sync_result.is_none());
}

#[tokio::test]
async fn test_nonexistent_branch() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    // Initialize repository
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Try to switch to nonexistent branch
    let result = manager.switch_branch(&repo_path, "nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_repository_operations() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    // Initialize repository
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // List branches
    let branches = manager.list_branches(&repo_path).unwrap();
    assert!(branches.contains(&"main".to_string()));
    assert!(branches.contains(&"feature".to_string()));
    
    // Create new branch
    manager.create_branch(&repo_path, "test-branch", None).unwrap();
    let branches = manager.list_branches(&repo_path).unwrap();
    assert!(branches.contains(&"test-branch".to_string()));
    
    // Delete branch
    manager.delete_branch(&repo_path, "test-branch").unwrap();
    let branches = manager.list_branches(&repo_path).unwrap();
    assert!(!branches.contains(&"test-branch".to_string()));
}

#[tokio::test]
async fn test_sync_requirements_calculation() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    // Initialize repository
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Calculate sync requirements for switching to feature branch
    let requirements = manager.calculate_sync_requirements(&repo_path, "feature").await.unwrap();
    
    // Should require some form of sync due to file differences
    assert!(requirements.requires_sync());
}

#[tokio::test]
async fn test_repository_status() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    // Initialize repository
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Check clean status
    assert!(!manager.has_uncommitted_changes(&repo_path).unwrap());
    
    // Create uncommitted file
    fs::write(repo_path.join("new_file.txt"), "New content").unwrap();
    
    // Check dirty status
    assert!(manager.has_uncommitted_changes(&repo_path).unwrap());
    
    // Get detailed status
    let status = manager.get_status(&repo_path).unwrap();
    assert!(!status.is_empty());
}

#[tokio::test]
async fn test_detached_head_initialization() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    
    // Put the repository in detached HEAD state first
    {
        let repo = Repository::open(&repo_path).unwrap();
        let head = repo.head().unwrap();
        let commit_id = head.target().unwrap();
        repo.set_head_detached(commit_id).unwrap();
    }
    
    // Now test that GitManager can initialize a repository in detached HEAD state
    let mut manager = GitManager::new();
    let result = manager.initialize_repository(&repo_path).await;
    
    // This should NOT fail (this was the original bug)
    assert!(result.is_ok(), "GitManager should handle detached HEAD state during initialization");
    
    let info = result.unwrap();
    assert!(info.current_branch.starts_with("detached-"));
    assert!(!info.current_commit.is_empty());
}

#[tokio::test]
async fn test_detached_head_branch_switching() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    // Initialize repository (should be on main branch)
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Create a tag to switch to
    {
        let repo = Repository::open(&repo_path).unwrap();
        let head = repo.head().unwrap();
        let commit_id = head.target().unwrap();
        let commit = repo.find_commit(commit_id).unwrap();
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        repo.tag("v1.0.0", commit.as_object(), &signature, "Test tag", false).unwrap();
    }
    
    // Switch to the tag (should create detached HEAD)
    let result = manager.switch_branch(&repo_path, "v1.0.0").await.unwrap();
    
    assert_eq!(result.previous_branch, "main");
    assert!(result.new_branch.starts_with("detached-") || result.new_branch == "v1.0.0");
    assert!(result.success);
    
    // Verify we can get repository info in detached state
    let info = manager.get_repository_info(&repo_path).unwrap();
    assert!(info.current_branch.starts_with("detached-"));
}

#[tokio::test]
async fn test_detached_head_state_persistence() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    // Initialize repository
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Create and switch to a tag
    {
        let repo = Repository::open(&repo_path).unwrap();
        let head = repo.head().unwrap();
        let commit_id = head.target().unwrap();
        let commit = repo.find_commit(commit_id).unwrap();
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        repo.tag("v2.0.0", commit.as_object(), &signature, "Test tag v2", false).unwrap();
    }
    
    manager.switch_branch(&repo_path, "v2.0.0").await.unwrap();
    
    // Create a new GitManager instance to test state persistence
    let mut new_manager = GitManager::new();
    
    // Initialize repository again (simulating restart)
    let result = new_manager.initialize_repository(&repo_path).await;
    assert!(result.is_ok(), "New GitManager should handle existing detached HEAD state");
    
    let info = result.unwrap();
    assert!(info.current_branch.starts_with("detached-"));
}

#[tokio::test]
async fn test_detached_head_merkle_calculation() {
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    let mut manager = GitManager::new();
    
    // Initialize repository
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Put in detached HEAD state
    {
        let repo = Repository::open(&repo_path).unwrap();
        let head = repo.head().unwrap();
        let commit_id = head.target().unwrap();
        repo.set_head_detached(commit_id).unwrap();
    }
    
    // Re-initialize to test merkle calculation in detached state
    let result = manager.initialize_repository(&repo_path).await;
    assert!(result.is_ok(), "Merkle calculation should work in detached HEAD state");
    
    // Verify we can calculate sync requirements
    let sync_req = manager.calculate_sync_requirements(&repo_path, "main").await;
    assert!(sync_req.is_ok(), "Sync requirement calculation should work from detached HEAD");
}

#[tokio::test]
async fn test_regression_original_bug_scenario() {
    // This test recreates the exact scenario from the original bug report
    let (_temp_dir, repo_path) = create_test_repo_with_branches();
    
    // Create a tag to simulate switching to "0.31.1"
    {
        let repo = Repository::open(&repo_path).unwrap();
        let head = repo.head().unwrap();
        let commit_id = head.target().unwrap();
        let commit = repo.find_commit(commit_id).unwrap();
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        repo.tag("0.31.1", commit.as_object(), &signature, "Release 0.31.1", false).unwrap();
    }
    
    // Start with repository in detached HEAD state (the problematic initial condition)
    {
        let repo = Repository::open(&repo_path).unwrap();
        let head = repo.head().unwrap();
        let commit_id = head.target().unwrap();
        repo.set_head_detached(commit_id).unwrap();
    }
    
    // This sequence should work without the "Failed to initialize repository" error
    let mut manager = GitManager::new();
    
    // Step 1: Initialize repository (this was failing before the fix)
    let init_result = manager.initialize_repository(&repo_path).await;
    assert!(init_result.is_ok(), "Repository initialization should succeed in detached HEAD state");
    
    let info = init_result.unwrap();
    assert!(info.current_branch.starts_with("detached-"));
    
    // Step 2: Switch to tag (this should also work)
    let switch_result = manager.switch_branch(&repo_path, "0.31.1").await;
    assert!(switch_result.is_ok(), "Switching to tag should succeed from detached HEAD");
    
    let switch_info = switch_result.unwrap();
    assert!(switch_info.success);
    assert!(switch_info.previous_branch.starts_with("detached-"));
} 