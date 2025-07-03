use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::fs;
use tracing::{info, warn, error};

/// Attempts to recover a repository that may be in a bad state
pub async fn recover_repository(repo_path: &Path) -> Result<()> {
    info!("Attempting to recover repository at {:?}", repo_path);
    
    // Check if it's a valid git repo
    if !repo_path.join(".git").exists() {
        return Err(anyhow!("Not a git repository: {:?}", repo_path));
    }
    
    // Try to clean up common issues
    
    // 1. Abort any in-progress operations
    abort_in_progress_operations(repo_path).await?;
    
    // 2. Clean up lock files
    clean_lock_files(repo_path)?;
    
    // 3. Reset to a clean state if needed
    if !super::git_edge_cases::check_working_tree_clean(repo_path).await? {
        warn!("Working tree has uncommitted changes");
        // Could offer to stash or reset here
    }
    
    info!("Repository recovery completed");
    Ok(())
}

/// Aborts any in-progress git operations (merge, rebase, etc.)
async fn abort_in_progress_operations(repo_path: &Path) -> Result<()> {
    use tokio::process::Command;
    
    // Check for merge in progress
    if repo_path.join(".git/MERGE_HEAD").exists() {
        warn!("Merge in progress, aborting...");
        Command::new("git")
            .current_dir(repo_path)
            .args(&["merge", "--abort"])
            .output()
            .await
            .context("Failed to abort merge")?;
    }
    
    // Check for rebase in progress
    if repo_path.join(".git/rebase-merge").exists() || 
       repo_path.join(".git/rebase-apply").exists() {
        warn!("Rebase in progress, aborting...");
        Command::new("git")
            .current_dir(repo_path)
            .args(&["rebase", "--abort"])
            .output()
            .await
            .context("Failed to abort rebase")?;
    }
    
    // Check for cherry-pick in progress
    if repo_path.join(".git/CHERRY_PICK_HEAD").exists() {
        warn!("Cherry-pick in progress, aborting...");
        Command::new("git")
            .current_dir(repo_path)
            .args(&["cherry-pick", "--abort"])
            .output()
            .await
            .context("Failed to abort cherry-pick")?;
    }
    
    Ok(())
}

/// Removes git lock files that may be preventing operations
fn clean_lock_files(repo_path: &Path) -> Result<()> {
    let git_dir = repo_path.join(".git");
    
    // Common lock files
    let lock_files = [
        "index.lock",
        "HEAD.lock",
        "config.lock",
    ];
    
    for lock_file in &lock_files {
        let lock_path = git_dir.join(lock_file);
        if lock_path.exists() {
            warn!("Removing lock file: {:?}", lock_path);
            fs::remove_file(&lock_path)
                .with_context(|| format!("Failed to remove lock file: {:?}", lock_path))?;
        }
    }
    
    Ok(())
}

/// Checks if a clone operation was interrupted and is incomplete
pub fn is_partial_clone(repo_path: &Path) -> bool {
    let git_dir = repo_path.join(".git");
    
    // Signs of incomplete clone
    if !git_dir.exists() {
        return false;
    }
    
    // Check if HEAD exists and is valid
    if let Ok(repo) = git2::Repository::open(repo_path) {
        if repo.head().is_err() {
            return true;
        }
    }
    
    // Check for clone-specific files that indicate incomplete state
    git_dir.join("shallow").exists() || 
    git_dir.join("objects/info/alternates").exists()
}

/// Attempts to clean up after a failed add operation
pub async fn cleanup_failed_add(
    repo_path: &Path,
    collection_name: Option<&str>,
    client: Option<&dyn crate::QdrantClientTrait>,
) -> Result<()> {
    info!("Cleaning up after failed repository add");
    
    // Remove the repository directory if it's a partial clone
    if repo_path.exists() && is_partial_clone(repo_path) {
        warn!("Removing partial clone at {:?}", repo_path);
        fs::remove_dir_all(repo_path)
            .with_context(|| format!("Failed to remove partial clone at {:?}", repo_path))?;
    }
    
    // Remove the Qdrant collection if it was created
    if let (Some(collection), Some(client)) = (collection_name, client) {
        warn!("Attempting to remove Qdrant collection: {}", collection);
        match client.delete_collection(collection.to_string()).await {
            Ok(_) => info!("Removed collection: {}", collection),
            Err(e) => warn!("Failed to remove collection {}: {}", collection, e),
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use git2::Repository;
    use std::path::Path;
    
    #[test]
    fn test_is_partial_clone() {
        let temp_dir = TempDir::new().unwrap();
        
        // Not a repo at all
        assert!(!is_partial_clone(temp_dir.path()));
        
        // Valid repo with initial commit
        let repo_path = temp_dir.path().join("valid_repo");
        let repo = Repository::init(&repo_path).unwrap();
        
        // Create initial commit so HEAD exists
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            fs::write(repo_path.join("README.md"), "# Test").unwrap();
            index.add_path(Path::new("README.md")).unwrap();
            index.write().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[]).unwrap();
        
        assert!(!is_partial_clone(&repo_path));
        
        // Simulate partial clone
        let partial_path = temp_dir.path().join("partial_repo");
        fs::create_dir_all(partial_path.join(".git/objects")).unwrap();
        fs::write(partial_path.join(".git/shallow"), "").unwrap();
        assert!(is_partial_clone(&partial_path));
    }
    
    #[test]
    fn test_clean_lock_files() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo");
        Repository::init(&repo_path).unwrap();
        
        // Create lock files
        let git_dir = repo_path.join(".git");
        fs::write(git_dir.join("index.lock"), "").unwrap();
        fs::write(git_dir.join("HEAD.lock"), "").unwrap();
        
        // Clean them
        clean_lock_files(&repo_path).unwrap();
        
        // Verify they're gone
        assert!(!git_dir.join("index.lock").exists());
        assert!(!git_dir.join("HEAD.lock").exists());
    }
}