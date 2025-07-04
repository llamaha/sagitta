use anyhow::{anyhow, Context, Result};
use git2::Repository;
use std::path::Path;
use tracing::{info, warn};

/// Resolves a git reference to an actual branch name or commit hash
/// Handles special cases like "HEAD", detached HEAD states, etc.
pub fn resolve_git_ref(repo: &Repository, ref_str: &str) -> Result<String> {
    // Special handling for HEAD
    if ref_str == "HEAD" {
        let head = repo.head()
            .context("Failed to get HEAD reference")?;
        
        if head.is_branch() {
            // HEAD points to a branch
            let branch_name = head.shorthand()
                .ok_or_else(|| anyhow!("Could not get branch name from HEAD"))?;
            info!("Resolved HEAD to branch: {}", branch_name);
            Ok(branch_name.to_string())
        } else {
            // Detached HEAD - return commit hash
            let commit_oid = head.target()
                .ok_or_else(|| anyhow!("HEAD has no target"))?;
            let commit_hash = commit_oid.to_string();
            warn!("HEAD is detached, using commit hash: {}", commit_hash);
            Ok(commit_hash)
        }
    } else {
        // For other refs, validate and return as-is
        validate_ref_name(ref_str)?;
        Ok(ref_str.to_string())
    }
}

/// Validates that a reference name is acceptable
pub fn validate_ref_name(ref_str: &str) -> Result<()> {
    // Check for obviously invalid patterns
    if ref_str.is_empty() {
        return Err(anyhow!("Reference name cannot be empty"));
    }
    
    // These patterns suggest confusion between refs and branch names
    if ref_str.starts_with("refs/heads/") {
        return Err(anyhow!(
            "Reference '{}' looks like a full ref path. Use just the branch name instead.", 
            ref_str
        ));
    }
    
    // Git's forbidden patterns
    if ref_str.contains("..") || ref_str.ends_with('.') || ref_str.contains("@{") {
        return Err(anyhow!("Reference '{}' contains forbidden patterns", ref_str));
    }
    
    // Warn about unusual but technically valid names
    if ref_str.contains('/') && !ref_str.starts_with("feature/") && !ref_str.starts_with("bugfix/") {
        warn!("Reference '{}' contains '/' which may cause issues", ref_str);
    }
    
    Ok(())
}

/// Detects the default branch of a repository by checking remote HEAD
pub async fn detect_default_branch(repo_path: &Path) -> Result<String> {
    use tokio::process::Command;
    
    // First try: git symbolic-ref refs/remotes/origin/HEAD
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(&["symbolic-ref", "refs/remotes/origin/HEAD"])
        .output()
        .await
        .context("Failed to execute git symbolic-ref")?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Output format: refs/remotes/origin/main
        if let Some(branch) = stdout.trim().split('/').last() {
            info!("Detected default branch from remote HEAD: {branch}");
            return Ok(branch.to_string());
        }
    }
    
    // Second try: Check common branch names
    let common_defaults = ["main", "master", "develop", "trunk", "default"];
    for branch_name in &common_defaults {
        let check = Command::new("git")
            .current_dir(repo_path)
            .args(&["show-ref", "--verify", &format!("refs/remotes/origin/{branch_name}")])
            .output()
            .await
            .context("Failed to execute git show-ref")?;
        
        if check.status.success() {
            info!("Found default branch by checking common names: {branch_name}");
            return Ok(branch_name.to_string());
        }
    }
    
    // Third try: List all remote branches and pick the first one
    let list_output = Command::new("git")
        .current_dir(repo_path)
        .args(&["branch", "-r", "--format=%(refname:short)"])
        .output()
        .await
        .context("Failed to list remote branches")?;
    
    if list_output.status.success() {
        let branches = String::from_utf8_lossy(&list_output.stdout);
        if let Some(first_branch) = branches.lines().next() {
            let branch_name = first_branch.trim().trim_start_matches("origin/");
            warn!("Could not detect default branch, using first remote branch: {branch_name}");
            return Ok(branch_name.to_string());
        }
    }
    
    // Final fallback
    warn!("Could not detect default branch, falling back to 'main'");
    Ok("main".to_string())
}

/// Checks if the working tree is clean (no uncommitted changes)
pub async fn check_working_tree_clean(repo_path: &Path) -> Result<bool> {
    use tokio::process::Command;
    
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(&["status", "--porcelain"])
        .output()
        .await
        .context("Failed to check git status")?;
    
    if !output.status.success() {
        return Err(anyhow!("Failed to get git status"));
    }
    
    Ok(output.stdout.is_empty())
}

/// Gets the current branch name, handling detached HEAD
pub fn get_current_branch(repo: &Repository) -> Result<String> {
    let head = repo.head()
        .context("Failed to get HEAD reference")?;
    
    if head.is_branch() {
        head.shorthand()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Could not get branch name"))
    } else {
        // Detached HEAD
        Err(anyhow!("Repository is in detached HEAD state"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    
    #[test]
    fn test_validate_ref_name() {
        // Valid names
        assert!(validate_ref_name("main").is_ok());
        assert!(validate_ref_name("feature/new-ui").is_ok());
        assert!(validate_ref_name("bugfix/issue-123").is_ok());
        
        // Invalid names
        assert!(validate_ref_name("").is_err());
        assert!(validate_ref_name("refs/heads/main").is_err());
        assert!(validate_ref_name("branch..name").is_err());
        assert!(validate_ref_name("branch.").is_err());
        assert!(validate_ref_name("branch@{upstream}").is_err());
    }
    
    #[test]
    fn test_resolve_head_on_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();
        
        // Create initial commit
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            fs::write(temp_dir.path().join("test.txt"), "test").unwrap();
            index.add_path(Path::new("test.txt")).unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();
        
        // HEAD should resolve to "master" (default branch name for git init)
        let resolved = resolve_git_ref(&repo, "HEAD").unwrap();
        assert!(resolved == "master" || resolved == "main"); // Depends on git config
    }
    
    #[tokio::test]
    async fn test_check_working_tree_clean() {
        let temp_dir = TempDir::new().unwrap();
        Repository::init(temp_dir.path()).unwrap();
        
        // Initially clean
        assert!(check_working_tree_clean(temp_dir.path()).await.unwrap());
        
        // Add untracked file
        fs::write(temp_dir.path().join("new_file.txt"), "content").unwrap();
        assert!(!check_working_tree_clean(temp_dir.path()).await.unwrap());
    }
}

// Include the edge case tests
#[cfg(test)]
#[path = "git_edge_cases_tests.rs"]
mod edge_case_tests;

// Include the integration tests
#[cfg(test)]
#[path = "git_edge_cases_integration_tests.rs"]
mod integration_tests;