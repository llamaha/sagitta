use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use crate::vectordb::embedding::EmbeddingModelType;
use crate::vectordb::auto_sync::AutoSyncConfig;
use anyhow::{Result, anyhow};
use log::{debug, info, warn, error};
use std::time::SystemTime;

/// Configuration for a git repository tracked by vectordb
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitRepoConfig {
    /// Absolute path to the repository
    pub path: PathBuf,
    /// Repository name (derived from path or user-defined)
    pub name: String,
    /// Unique identifier for the repository (based on canonical path)
    pub id: String,
    /// Currently active branch
    pub active_branch: String,
    /// Branch name -> commit hash mapping for indexed branches
    pub indexed_branches: HashMap<String, String>,
    /// Repository-specific embedding model if overriding global setting
    pub embedding_model: Option<EmbeddingModelType>,
    /// File types to index for this repository
    pub file_types: Vec<String>,
    /// Time of last indexing
    pub last_indexed: Option<DateTime<Utc>>,
    /// Whether this repository is active
    pub active: bool,
    /// Auto-sync configuration for this repository
    pub auto_sync: AutoSyncConfig,
}

impl GitRepoConfig {
    /// Create a new repository configuration with sensible defaults
    pub fn new(path: PathBuf, name: Option<String>, id: String) -> Result<Self> {
        // Validate the path
        if !path.exists() {
            return Err(anyhow!("Repository path does not exist: {}", path.display()));
        }
        
        // Check if it's a git repository
        let git_dir = path.join(".git");
        if !git_dir.exists() {
            return Err(anyhow!("Not a git repository: {}", path.display()));
        }
        
        // Get default name from directory if not provided
        let repo_name = name.unwrap_or_else(|| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "unnamed-repo".to_string())
        });
        
        // Get current branch
        let branch = get_current_branch(&path).unwrap_or_else(|_| "main".to_string());
        
        // Get current commit hash
        let commit_hash = get_current_commit_hash(&path)?;
        
        // Initialize indexed branches with the current branch
        let mut indexed_branches = HashMap::new();
        indexed_branches.insert(branch.clone(), commit_hash);
        
        Ok(Self {
            path,
            name: repo_name,
            id,
            active_branch: branch,
            indexed_branches,
            embedding_model: None,
            file_types: vec!["rs".to_string(), "go".to_string(), "js".to_string(), "py".to_string()],
            last_indexed: None,
            active: true,
            auto_sync: AutoSyncConfig::default(),
        })
    }
    
    /// Updates the indexed commit for a branch
    pub fn update_indexed_commit<S: AsRef<str>>(&mut self, branch: &str, commit_hash: S) {
        self.indexed_branches.insert(branch.to_string(), commit_hash.as_ref().to_string());
        self.last_indexed = Some(SystemTime::now().into());
    }
    
    /// Check if a branch has been indexed
    pub fn is_branch_indexed(&self, branch: &str) -> bool {
        self.indexed_branches.contains_key(branch)
    }
    
    /// Get the indexed commit hash for a branch
    pub fn get_indexed_commit(&self, branch: &str) -> Option<&String> {
        self.indexed_branches.get(branch)
    }
    
    /// Enable auto-sync for this repository
    pub fn enable_auto_sync(&mut self, min_interval: Option<u64>) {
        self.auto_sync.enabled = true;
        if let Some(interval) = min_interval {
            self.auto_sync.min_interval = interval;
        }
    }
    
    /// Disable auto-sync for this repository
    pub fn disable_auto_sync(&mut self) {
        self.auto_sync.enabled = false;
    }
}

/// Generate a canonical unique ID for a repository based on its path
pub fn canonical_repo_id(repo_path: &Path) -> Result<String> {
    // Try to get the canonical absolute path
    let canonical = fs::canonicalize(repo_path)?;
    
    // Convert to string and normalize path separators
    let path_str = canonical.to_string_lossy().to_string().replace('\\', "/");
    
    // Create a hash of the path for a shorter ID
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    path_str.hash(&mut hasher);
    let path_hash = hasher.finish();
    
    // Use directory name + hash suffix for readability
    let dir_name = repo_path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());
    
    Ok(format!("{}-{:x}", dir_name, path_hash))
}

/// Generate a canonical unique ID for a repository based on its path and name
pub fn canonical_repo_id_with_name(repo_path: &Path, name: &str) -> Result<String> {
    // Try to get the canonical absolute path
    let canonical = fs::canonicalize(repo_path)?;
    
    // Convert to string and normalize path separators
    let path_str = canonical.to_string_lossy().to_string().replace('\\', "/");
    
    // Create a hash of the combined path and name for a shorter ID
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    path_str.hash(&mut hasher);
    name.hash(&mut hasher);
    let combined_hash = hasher.finish();
    
    // Use given name + hash suffix for readability
    Ok(format!("{}-{:x}", name, combined_hash))
}

/// Get the current branch of a git repository
fn get_current_branch(repo_path: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["branch", "--show-current"])
        .output()?;
    
    if output.status.success() {
        let branch = String::from_utf8(output.stdout)?
            .trim()
            .to_string();
        
        // If empty (detached HEAD), try to get from HEAD ref
        if branch.is_empty() {
            let head_output = std::process::Command::new("git")
                .current_dir(repo_path)
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output()?;
            
            if head_output.status.success() {
                let head_ref = String::from_utf8(head_output.stdout)?
                    .trim()
                    .to_string();
                
                if head_ref != "HEAD" {
                    return Ok(head_ref);
                }
            }
            
            // Fall back to "main" if we can't determine
            return Ok("main".to_string());
        }
        
        Ok(branch)
    } else {
        Err(anyhow!("Failed to get current branch: {}", 
            String::from_utf8_lossy(&output.stderr)))
    }
}

/// Get the current commit hash of a git repository
fn get_current_commit_hash(repo_path: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", "HEAD"])
        .output()?;
    
    if output.status.success() {
        let commit_hash = String::from_utf8(output.stdout)?
            .trim()
            .to_string();
        
        Ok(commit_hash)
    } else {
        Err(anyhow!("Failed to get current commit hash: {}", 
            String::from_utf8_lossy(&output.stderr)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::process::Command;
    use std::fs;

    // Helper function to create a git repo for testing
    fn setup_git_repo() -> Result<(tempfile::TempDir, PathBuf)> {
        // Create a temporary directory that will be automatically deleted when it goes out of scope
        let temp_dir = tempdir()?;
        let repo_path = temp_dir.path().to_path_buf();
        
        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()?;
            
        // Configure git identity for commits
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()?;
            
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()?;
            
        // Create a test file
        let test_file = repo_path.join("test.txt");
        fs::write(&test_file, "Test content")?;
        
        // Add and commit the file
        Command::new("git")
            .args(["add", "test.txt"])
            .current_dir(&repo_path)
            .output()?;
            
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()?;
        
        // Find out what branch we're on for debugging
        let branch_output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&repo_path)
            .output()?;
        
        let branch_name = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();
        println!("Current branch name in test repo: '{}'", branch_name);
        
        Ok((temp_dir, repo_path))
    }
    
    #[test]
    fn test_repo_config_new() -> Result<()> {
        // Keep the temp_dir alive for the duration of the test
        let (temp_dir, repo_path) = setup_git_repo()?;
        let repo_id = "test-repo";
        
        let config = GitRepoConfig::new(repo_path.clone(), None, repo_id.to_string())?;
        assert!(config.path.exists());
        assert!(!config.indexed_branches.is_empty());
        
        // temp_dir will be automatically cleaned up when it goes out of scope
        drop(temp_dir);
        Ok(())
    }
    
    #[test]
    fn test_branch_indexed_status() -> Result<()> {
        let (temp_dir, repo_path) = setup_git_repo()?;
        let repo_id = "test-repo";
        
        let config = GitRepoConfig::new(repo_path.clone(), None, repo_id.to_string())?;
        println!("Active branch: {}", config.active_branch);
        println!("Indexed branches: {:?}", config.indexed_branches);
        
        // Use the actual active branch instead of hardcoding "master"
        let current_branch = config.active_branch.clone(); 
        assert!(config.is_branch_indexed(&current_branch));
        
        // Non-existent branch should not be indexed
        assert!(!config.is_branch_indexed("nonexistent-branch"));
        
        drop(temp_dir);
        Ok(())
    }
    
    #[test]
    fn test_auto_sync_config() -> Result<()> {
        let (temp_dir, repo_path) = setup_git_repo()?;
        let repo_id = "test-repo";
        
        // Create with auto-sync disabled
        let config = GitRepoConfig::new(repo_path.clone(), None, repo_id.to_string())?;
        assert!(!config.auto_sync.enabled);
        
        // Create with auto-sync enabled
        let mut config = GitRepoConfig::new(repo_path.clone(), None, repo_id.to_string())?;
        config.auto_sync.enabled = true;
        assert!(config.auto_sync.enabled);
        
        drop(temp_dir);
        Ok(())
    }
    
    #[test]
    fn test_canonical_repo_id_with_name() -> Result<()> {
        let (temp_dir, repo_path) = setup_git_repo()?;
        
        // Test with a custom name
        let name = "test-repo";
        let config = GitRepoConfig::new(repo_path.clone(), Some(name.to_string()), name.to_string())?;
        assert_eq!(config.id, name);
        
        // Test without a name (should use id directly)
        let id = "test-repo-id";
        let config = GitRepoConfig::new(repo_path.clone(), None, id.to_string())?;
        assert_eq!(config.id, id);
        
        drop(temp_dir);
        Ok(())
    }
    
    #[test]
    fn test_update_indexed_commit() -> Result<()> {
        let (temp_dir, repo_path) = setup_git_repo()?;
        let repo_id = "test-repo";
        
        let mut config = GitRepoConfig::new(repo_path.clone(), None, repo_id.to_string())?;
        println!("Active branch: {}", config.active_branch);
        println!("Indexed branches: {:?}", config.indexed_branches);
        
        // Use the actual active branch instead of hardcoding "master"
        let current_branch = config.active_branch.clone();
        assert!(config.indexed_branches.contains_key(&current_branch));
        
        // Update the commit ID
        let new_commit = "abc123def456";
        config.update_indexed_commit(&current_branch, new_commit);
        
        // Check that it was updated
        assert_eq!(
            config.indexed_branches.get(&current_branch).unwrap(),
            &new_commit.to_string()
        );
        
        drop(temp_dir);
        Ok(())
    }
} 