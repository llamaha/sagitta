use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;
use vectordb_cli::utils::git::{ChangeSet, GitRepo};

fn setup_test_repo() -> Result<(tempfile::TempDir, GitRepo)> {
    let temp_dir = tempdir()?;
    let path = temp_dir.path();
    
    // Initialize git repo
    Command::new("git")
        .current_dir(path)
        .args(["init"])
        .output()?;
    
    // Configure git
    Command::new("git")
        .current_dir(path)
        .args(["config", "user.name", "Test User"])
        .output()?;
    
    Command::new("git")
        .current_dir(path)
        .args(["config", "user.email", "test@example.com"])
        .output()?;
    
    // Create the git repo object
    let git_repo = GitRepo::new(path.to_path_buf())?;
    
    Ok((temp_dir, git_repo))
}

fn create_commit(git_repo: &GitRepo, file_name: &str, content: &str) -> Result<String> {
    let file_path = git_repo.path.join(file_name);
    fs::write(&file_path, content)?;
    
    // Add and commit
    Command::new("git")
        .current_dir(&git_repo.path)
        .args(["add", file_name])
        .output()?;
    
    Command::new("git")
        .current_dir(&git_repo.path)
        .args(["commit", "-m", &format!("Add {}", file_name)])
        .output()?;
    
    // Get the commit hash
    git_repo.get_commit_hash("HEAD")
}

fn create_branch(git_repo: &GitRepo, branch_name: &str) -> Result<()> {
    Command::new("git")
        .current_dir(&git_repo.path)
        .args(["checkout", "-b", branch_name])
        .output()?;
    
    Ok(())
}

fn checkout_branch(git_repo: &GitRepo, branch_name: &str) -> Result<()> {
    Command::new("git")
        .current_dir(&git_repo.path)
        .args(["checkout", branch_name])
        .output()?;
    
    Ok(())
}

#[test]
fn test_find_common_ancestor() -> Result<()> {
    let (temp_dir, git_repo) = setup_test_repo()?;
    
    // Create initial commit on main
    let main_commit = create_commit(&git_repo, "main.txt", "main content")?;
    
    // Create a feature branch
    create_branch(&git_repo, "feature")?;
    
    // Add a commit on feature
    let feature_commit = create_commit(&git_repo, "feature.txt", "feature content")?;
    
    // Back to main
    checkout_branch(&git_repo, "main")?;
    
    // Add another commit on main
    let main_commit2 = create_commit(&git_repo, "main2.txt", "main content 2")?;
    
    // Find common ancestor
    let ancestor = git_repo.find_common_ancestor("main", "feature")?;
    
    // The common ancestor should be the first main commit
    assert_eq!(ancestor, main_commit);
    
    Ok(())
}

#[test]
fn test_is_ancestor_of() -> Result<()> {
    let (temp_dir, git_repo) = setup_test_repo()?;
    
    // Create initial commit on main
    let main_commit = create_commit(&git_repo, "main.txt", "main content")?;
    
    // Create a feature branch
    create_branch(&git_repo, "feature")?;
    
    // Add a commit on feature
    let feature_commit = create_commit(&git_repo, "feature.txt", "feature content")?;
    
    // Check relationships
    assert!(git_repo.is_ancestor_of(&main_commit, &feature_commit)?);
    assert!(!git_repo.is_ancestor_of(&feature_commit, &main_commit)?);
    
    Ok(())
}

#[test]
fn test_get_cross_branch_changes() -> Result<()> {
    let (temp_dir, git_repo) = setup_test_repo()?;
    
    // Create initial commit on main
    let main_commit = create_commit(&git_repo, "common.txt", "common content")?;
    
    // Create a feature branch
    create_branch(&git_repo, "feature")?;
    
    // Add commits on feature
    create_commit(&git_repo, "feature1.txt", "feature content 1")?;
    let feature_commit = create_commit(&git_repo, "feature2.txt", "feature content 2")?;
    
    // Back to main
    checkout_branch(&git_repo, "main")?;
    
    // Add commits on main
    create_commit(&git_repo, "main1.txt", "main content 1")?;
    let main_commit2 = create_commit(&git_repo, "main2.txt", "main content 2")?;
    
    // Get cross-branch changes from feature to main
    let changes = git_repo.get_cross_branch_changes("feature", "main")?;
    
    // Should include main1.txt and main2.txt as added files 
    assert_eq!(changes.added_files.len(), 2);
    
    // Get cross-branch changes from main to feature
    let changes = git_repo.get_cross_branch_changes("main", "feature")?;
    
    // Should include feature1.txt and feature2.txt as added files
    assert_eq!(changes.added_files.len(), 2);
    
    Ok(())
} 