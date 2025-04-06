use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use crate::vectordb::embedding::EmbeddingModelType;
use crate::vectordb::auto_sync::AutoSyncConfig;
use anyhow::{Result, anyhow};
use log::{debug, info, warn, error};

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
        
        Ok(Self {
            path,
            name: repo_name,
            id,
            active_branch: branch,
            indexed_branches: HashMap::new(),
            embedding_model: None,
            file_types: vec!["rs".to_string(), "go".to_string(), "js".to_string(), "py".to_string()],
            last_indexed: None,
            active: true,
            auto_sync: AutoSyncConfig::default(),
        })
    }
    
    /// Update the indexed commit hash for a branch
    pub fn update_indexed_commit(&mut self, branch: &str, commit_hash: &str) {
        self.indexed_branches.insert(branch.to_string(), commit_hash.to_string());
        self.last_indexed = Some(Utc::now());
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