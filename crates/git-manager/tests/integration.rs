//! Integration tests for git-manager
//!
//! These tests verify that all git operations work correctly end-to-end
//! and cover the Phase 1 requirements from the success plan.

use git_manager::{
    GitManager, GitResult, 
    RepositoryCloner, CloneOptions, CloneResult,
    BranchManager, CreateBranchOptions, BranchInfo,
    ChangeManager, CommitOptions, GitSignature,
    init_repository,
};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use std::fs;

/// Helper function to create a test repository with initial commit
fn create_test_repo(path: &Path) -> GitResult<()> {
    let repo = init_repository(path, false)?;
    
    // Configure test repository with default user
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Test User").unwrap();
    config.set_str("user.email", "test@example.com").unwrap();
    
    // Create an initial file and commit
    let test_file = path.join("README.md");
    fs::write(&test_file, "# Test Repository\n\nThis is a test repository.").unwrap();
    
    let change_manager = ChangeManager::new(path)?;
    change_manager.stage_files(&[PathBuf::from("README.md")])?;
    
    let commit_options = CommitOptions {
        message: "Initial commit".to_string(),
        author: Some(GitSignature {
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
        }),
        committer: None,
        allow_empty: false,
        amend: false,
    };
    
    change_manager.commit(commit_options)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test repository initialization and basic operations
    #[test]
    fn test_repository_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        
        // Initialize repository
        let result = create_test_repo(&repo_path);
        assert!(result.is_ok(), "Failed to create test repository: {:?}", result);
        
        // Verify repository exists and has content
        assert!(repo_path.exists());
        assert!(repo_path.join(".git").exists());
        assert!(repo_path.join("README.md").exists());
    }

    /// Test GitManager initialization and basic functionality
    #[tokio::test]
    async fn test_git_manager_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path).unwrap();
        
        let mut manager = GitManager::new();
        
        // Initialize repository in GitManager
        let repo_info = manager.initialize_repository(&repo_path).await.unwrap();
        assert_eq!(repo_info.path, repo_path);
        assert!(!repo_info.current_branch.is_empty());
        
        // Get repository info
        let info = manager.get_repository_info(&repo_path).unwrap();
        assert_eq!(info.path, repo_path);
    }

    /// Test complete branch operations workflow
    #[test]
    fn test_branch_operations_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path).unwrap();
        
        let branch_manager = BranchManager::new(&repo_path).unwrap();
        
        // Test listing branches
        let branches = branch_manager.list_branches(None).unwrap();
        assert!(!branches.is_empty());
        
        let main_branch = branches.iter().find(|b| b.is_current).unwrap();
        assert!(main_branch.name == "main" || main_branch.name == "master");
        
        // Test creating a new branch
        let create_options = CreateBranchOptions::default();
        let new_branch = branch_manager.create_branch("feature/test", create_options).unwrap();
        assert_eq!(new_branch.name, "feature/test");
        assert!(!new_branch.is_current);
        
        // Test branch exists
        assert!(branch_manager.branch_exists("feature/test").unwrap());
        
        // Test getting branch info
        let branch_info = branch_manager.get_branch_info("feature/test").unwrap();
        assert_eq!(branch_info.name, "feature/test");
        
        // Test switching branches
        branch_manager.switch_branch("feature/test", false).unwrap();
        let current = branch_manager.get_current_branch_name().unwrap();
        assert_eq!(current, Some("feature/test".to_string()));
        
        // Switch back to main
        let main_name = &main_branch.name;
        branch_manager.switch_branch(main_name, false).unwrap();
        
        // Test deleting branch
        branch_manager.delete_branch("feature/test", false).unwrap();
        assert!(!branch_manager.branch_exists("feature/test").unwrap());
    }

    /// Test complete change management workflow
    #[test]
    fn test_change_management_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path).unwrap();
        
        let change_manager = ChangeManager::new(&repo_path).unwrap();
        
        // Create a new file
        let new_file = repo_path.join("src").join("main.rs");
        fs::create_dir_all(new_file.parent().unwrap()).unwrap();
        fs::write(&new_file, "fn main() {\n    println!(\"Hello, world!\");\n}").unwrap();
        
        // Test staging specific files
        let staged = change_manager.stage_files(&[PathBuf::from("src/main.rs")]).unwrap();
        assert_eq!(staged, 1);
        
        // Test committing changes
        let commit_options = CommitOptions {
            message: "Add main.rs file".to_string(),
            author: Some(GitSignature {
                name: "Test Author".to_string(),
                email: "author@example.com".to_string(),
            }),
            committer: None,
            allow_empty: false,
            amend: false,
        };
        
        let commit_result = change_manager.commit(commit_options).unwrap();
        assert_eq!(commit_result.message, "Add main.rs file");
        assert!(!commit_result.commit_sha.is_empty());
        
        // Create multiple files and stage all
        fs::write(repo_path.join("file1.txt"), "Content 1").unwrap();
        fs::write(repo_path.join("file2.txt"), "Content 2").unwrap();
        
        let staged_all = change_manager.stage_files(&[]).unwrap(); // Empty array stages all
        assert!(staged_all >= 2);
        
        // Commit all changes
        let commit_options2 = CommitOptions {
            message: "Add multiple files".to_string(),
            author: None, // Use default signature
            committer: None,
            allow_empty: false,
            amend: false,
        };
        
        let commit_result2 = change_manager.commit(commit_options2).unwrap();
        assert_eq!(commit_result2.message, "Add multiple files");
    }

    /// Test repository cloning (basic functionality)
    #[tokio::test]
    async fn test_repository_cloner() {
        let cloner = RepositoryCloner::new();
        
        // Test cloner creation
        assert!(!cloner.is_cancelled());
        
        // Test cancellation
        cloner.cancel();
        assert!(cloner.is_cancelled());
        
        // Note: We can't test actual cloning without a real remote repository
        // In a real test environment, you would test with a known test repository
    }

    /// Test GitManager with branch operations
    #[tokio::test]
    async fn test_git_manager_branch_operations() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path).unwrap();
        
        let mut manager = GitManager::new();
        manager.initialize_repository(&repo_path).await.unwrap();
        
        // Test listing branches through GitManager
        let branches = manager.list_branches(&repo_path).unwrap();
        assert!(!branches.is_empty());
        
        // Test creating branch through GitManager
        manager.create_branch(&repo_path, "feature/manager-test", None).unwrap();
        
        let branches_after = manager.list_branches(&repo_path).unwrap();
        assert!(branches_after.len() > branches.len());
        
        // Test switching branch through GitManager
        let switch_result = manager.switch_branch(&repo_path, "feature/manager-test").await.unwrap();
        assert!(switch_result.success);
        assert_eq!(switch_result.new_branch, "feature/manager-test");
        
        // Test checking uncommitted changes
        assert!(!manager.has_uncommitted_changes(&repo_path).unwrap());
        
        // Create uncommitted change
        fs::write(repo_path.join("temp.txt"), "temporary content").unwrap();
        assert!(manager.has_uncommitted_changes(&repo_path).unwrap());
        
        // Test getting status
        let status = manager.get_status(&repo_path).unwrap();
        assert!(!status.is_empty());
    }

    /// Test error handling scenarios
    #[test]
    fn test_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let non_existent_path = temp_dir.path().join("non_existent_repo");
        
        // Test opening non-existent repository
        let branch_manager_result = BranchManager::new(&non_existent_path);
        assert!(branch_manager_result.is_err());
        
        let change_manager_result = ChangeManager::new(&non_existent_path);
        assert!(change_manager_result.is_err());
        
        // Test operations on valid repository
        let repo_path = temp_dir.path().join("test_repo");
        create_test_repo(&repo_path).unwrap();
        
        let branch_manager = BranchManager::new(&repo_path).unwrap();
        
        // Test creating duplicate branch
        let create_options = CreateBranchOptions::default();
        branch_manager.create_branch("test-branch", create_options.clone()).unwrap();
        
        let duplicate_result = branch_manager.create_branch("test-branch", create_options);
        assert!(duplicate_result.is_err());
        
        // Test deleting non-existent branch
        let delete_result = branch_manager.delete_branch("non-existent-branch", false);
        assert!(delete_result.is_err());
        
        // Test switching to non-existent branch
        let switch_result = branch_manager.switch_branch("non-existent-branch", false);
        assert!(switch_result.is_err());
    }

    /// Test complex workflow combining all operations
    #[tokio::test]
    async fn test_complete_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("complete_test_repo");
        create_test_repo(&repo_path).unwrap();
        
        // Initialize GitManager
        let mut git_manager = GitManager::new();
        git_manager.initialize_repository(&repo_path).await.unwrap();
        
        // Create and switch to feature branch
        git_manager.create_branch(&repo_path, "feature/complete-test", None).unwrap();
        let switch_result = git_manager.switch_branch(&repo_path, "feature/complete-test").await.unwrap();
        assert!(switch_result.success);
        
        // Create change manager for file operations
        let change_manager = ChangeManager::new(&repo_path).unwrap();
        
        // Add some files and commit
        let src_dir = repo_path.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        
        fs::write(src_dir.join("lib.rs"), "// Library code\npub fn hello() -> &'static str {\n    \"Hello, World!\"\n}").unwrap();
        fs::write(src_dir.join("main.rs"), "use crate::hello;\n\nfn main() {\n    println!(\"{}\", hello());\n}").unwrap();
        
        // Stage all new files
        change_manager.stage_files(&[]).unwrap();
        
        // Commit the changes
        let commit_options = CommitOptions {
            message: "Add library and main files".to_string(),
            author: Some(GitSignature {
                name: "Integration Test".to_string(),
                email: "integration@test.com".to_string(),
            }),
            committer: None,
            allow_empty: false,
            amend: false,
        };
        
        let commit_result = change_manager.commit(commit_options).unwrap();
        assert!(!commit_result.commit_sha.is_empty());
        
        // Switch back to main branch
        let main_branch_name = if git_manager.list_branches(&repo_path).unwrap()
            .iter().any(|b| b == "main") { "main" } else { "master" };
        
        git_manager.switch_branch(&repo_path, main_branch_name).await.unwrap();
        
        // Verify we're back on main and the files don't exist (they're on the feature branch)
        let current_branch = BranchManager::new(&repo_path).unwrap()
            .get_current_branch_name().unwrap().unwrap();
        assert_eq!(current_branch, main_branch_name);
        
        // The src directory shouldn't exist on main branch
        assert!(!repo_path.join("src").exists());
        
        // Switch back to feature branch and verify files exist
        git_manager.switch_branch(&repo_path, "feature/complete-test").await.unwrap();
        assert!(repo_path.join("src").exists());
        assert!(repo_path.join("src/lib.rs").exists());
        assert!(repo_path.join("src/main.rs").exists());
    }

    /// Test GitManager sync requirements calculation
    #[tokio::test]
    async fn test_sync_requirements() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("sync_test_repo");
        create_test_repo(&repo_path).unwrap();
        
        let mut git_manager = GitManager::new();
        git_manager.initialize_repository(&repo_path).await.unwrap();
        
        // Create a feature branch with changes
        git_manager.create_branch(&repo_path, "feature/sync-test", None).unwrap();
        git_manager.switch_branch(&repo_path, "feature/sync-test").await.unwrap();
        
        // Add files on feature branch
        fs::write(repo_path.join("feature_file.txt"), "Feature content").unwrap();
        let change_manager = ChangeManager::new(&repo_path).unwrap();
        change_manager.stage_files(&[PathBuf::from("feature_file.txt")]).unwrap();
        
        let commit_options = CommitOptions {
            message: "Add feature file".to_string(),
            author: None,
            committer: None,
            allow_empty: false,
            amend: false,
        };
        change_manager.commit(commit_options).unwrap();
        
        // Switch back to main and check sync requirements
        let main_branch = if git_manager.list_branches(&repo_path).unwrap()
            .iter().any(|b| b == "main") { "main" } else { "master" };
        
        git_manager.switch_branch(&repo_path, main_branch).await.unwrap();
        
        // Calculate sync requirements for switching back to feature branch
        let sync_req = git_manager.calculate_sync_requirements(&repo_path, "feature/sync-test").await.unwrap();
        
        // We expect some kind of sync to be needed due to the different file states
        // The exact sync type depends on the implementation, but we verify the structure works
        assert!(!sync_req.files_to_update.is_empty() || !sync_req.files_to_add.is_empty());
    }
} 