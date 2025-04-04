use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};
use std::process::Command;
use chrono::{DateTime, Utc, TimeZone};
use log::{debug, info, warn, error};

/// Represents a git repository with utility functions
pub struct GitRepo {
    path: PathBuf,
}

/// Represents a git commit
#[derive(Debug, Clone)]
pub struct GitCommit {
    pub hash: String,
    pub author: String,
    pub date: DateTime<Utc>,
    pub message: String,
}

impl GitRepo {
    /// Create a new GitRepo instance
    pub fn new(path: PathBuf) -> Result<Self> {
        // Validate that this is a git repository
        let git_dir = path.join(".git");
        if !git_dir.exists() {
            return Err(anyhow!("Not a git repository: {}", path.display()));
        }
        
        Ok(Self { path })
    }
    
    /// Get the current branch name
    pub fn get_current_branch(&self) -> Result<String> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["branch", "--show-current"])
            .output()?;
        
        if output.status.success() {
            let branch = String::from_utf8(output.stdout)?
                .trim()
                .to_string();
            
            // If empty (detached HEAD), try to get from HEAD ref
            if branch.is_empty() {
                let head_output = Command::new("git")
                    .current_dir(&self.path)
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
                Ok("main".to_string())
            } else {
                Ok(branch)
            }
        } else {
            Err(anyhow!("Failed to get current branch: {}", 
                String::from_utf8_lossy(&output.stderr)))
        }
    }
    
    /// Get the commit hash for a branch
    pub fn get_commit_hash(&self, branch: &str) -> Result<String> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["rev-parse", branch])
            .output()?;
        
        if output.status.success() {
            Ok(String::from_utf8(output.stdout)?.trim().to_string())
        } else {
            Err(anyhow!("Failed to get commit hash for branch {}: {}", 
                branch, String::from_utf8_lossy(&output.stderr)))
        }
    }
    
    /// List all branches in the repository
    pub fn list_branches(&self) -> Result<Vec<String>> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["branch", "--format=%(refname:short)"])
            .output()?;
        
        if output.status.success() {
            let branches = String::from_utf8(output.stdout)?
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect();
            
            Ok(branches)
        } else {
            Err(anyhow!("Failed to list branches: {}", 
                String::from_utf8_lossy(&output.stderr)))
        }
    }
    
    /// List all remote branches in the repository
    pub fn list_remote_branches(&self) -> Result<Vec<String>> {
        // First, update remote information
        let _ = Command::new("git")
            .current_dir(&self.path)
            .args(["fetch", "--prune"])
            .output()?;
        
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["branch", "-r", "--format=%(refname:short)"])
            .output()?;
        
        if output.status.success() {
            let branches = String::from_utf8(output.stdout)?
                .lines()
                .map(|line| {
                    // Remove remote part (e.g., "origin/") from branch names
                    let parts: Vec<&str> = line.trim().split('/').collect();
                    if parts.len() > 1 {
                        parts[1..].join("/")
                    } else {
                        line.trim().to_string()
                    }
                })
                .filter(|line| !line.is_empty() && line != "HEAD")
                .collect();
            
            Ok(branches)
        } else {
            Err(anyhow!("Failed to list remote branches: {}", 
                String::from_utf8_lossy(&output.stderr)))
        }
    }
    
    /// Get files changed between two commits
    pub fn get_changed_files(&self, from_commit: &str, to_commit: &str) -> Result<Vec<PathBuf>> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["diff", "--name-status", from_commit, to_commit])
            .output()?;
        
        if output.status.success() {
            let files = String::from_utf8(output.stdout)?
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.trim().split_whitespace().collect();
                    if parts.len() >= 2 {
                        // Second part is the file path
                        Some(self.path.join(parts[1]))
                    } else {
                        None
                    }
                })
                .collect();
            
            Ok(files)
        } else {
            Err(anyhow!("Failed to get changed files between {} and {}: {}", 
                from_commit, to_commit, String::from_utf8_lossy(&output.stderr)))
        }
    }
    
    /// Get detailed changes between two commits
    pub fn get_change_set(&self, from_commit: &str, to_commit: &str) -> Result<ChangeSet> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["diff", "--name-status", from_commit, to_commit])
            .output()?;
        
        if output.status.success() {
            let mut added_files = Vec::new();
            let mut modified_files = Vec::new();
            let mut deleted_files = Vec::new();
            
            String::from_utf8(output.stdout)?.lines().for_each(|line| {
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.len() >= 2 {
                    let status = parts[0];
                    let file_path = self.path.join(parts[1]);
                    
                    match status.chars().next() {
                        Some('A') => added_files.push(file_path),
                        Some('M') => modified_files.push(file_path),
                        Some('D') => deleted_files.push(file_path),
                        Some('R') => {
                            // Renamed - treat as delete of old + add of new
                            if parts.len() >= 3 {
                                deleted_files.push(self.path.join(parts[1]));
                                added_files.push(self.path.join(parts[2]));
                            }
                        },
                        _ => {} // Ignore other changes
                    }
                }
            });
            
            Ok(ChangeSet {
                commit_before: from_commit.to_string(),
                commit_after: to_commit.to_string(),
                added_files,
                modified_files,
                deleted_files,
            })
        } else {
            Err(anyhow!("Failed to get change set between {} and {}: {}", 
                from_commit, to_commit, String::from_utf8_lossy(&output.stderr)))
        }
    }
    
    /// Check if a repository needs reindexing by comparing commits
    pub fn needs_reindexing(&self, branch: &str, last_indexed_commit: &str) -> Result<bool> {
        // Get the latest commit hash
        let current_commit = self.get_commit_hash(branch)?;
        
        // If the commits are the same, no reindexing needed
        if current_commit == last_indexed_commit {
            return Ok(false);
        }
        
        // Check if the last indexed commit still exists in the history
        let commit_exists = Command::new("git")
            .current_dir(&self.path)
            .args(["cat-file", "-e", &format!("{}^{{commit}}", last_indexed_commit)])
            .status()?
            .success();
        
        if !commit_exists {
            // Commit doesn't exist or isn't a commit, need full reindex
            return Ok(true);
        }
        
        // Get changed files count - if there are changes, reindexing is needed
        let changed_files = self.get_changed_files(last_indexed_commit, &current_commit)?;
        
        Ok(!changed_files.is_empty())
    }
    
    /// Get the file modification time from git history
    pub fn file_modification_time(&self, file_path: &Path) -> Result<DateTime<Utc>> {
        // Get relative path to the repository
        let rel_path = file_path.strip_prefix(&self.path)
            .map_err(|_| anyhow!("File path is not in repository"))?;
        
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["log", "-1", "--format=%at", "--", rel_path.to_str().unwrap()])
            .output()?;
        
        if output.status.success() {
            let timestamp = String::from_utf8(output.stdout)?
                .trim()
                .parse::<i64>()?;
            
            Ok(Utc.timestamp_opt(timestamp, 0).unwrap())
        } else {
            // If git command fails, fall back to file system time
            let metadata = file_path.metadata()?;
            let modified = metadata.modified()?;
            
            let system_time: DateTime<Utc> = modified.into();
            Ok(system_time)
        }
    }
    
    /// Get commit history for a file
    pub fn get_file_history(&self, file_path: &Path) -> Result<Vec<GitCommit>> {
        // Get relative path to the repository
        let rel_path = file_path.strip_prefix(&self.path)
            .map_err(|_| anyhow!("File path is not in repository"))?;
        
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["log", "--format=%H|%an|%at|%s", "--", rel_path.to_str().unwrap()])
            .output()?;
        
        if output.status.success() {
            let commits = String::from_utf8(output.stdout)?
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split('|').collect();
                    if parts.len() >= 4 {
                        // Parse timestamp
                        let timestamp = parts[2].parse::<i64>().ok()?;
                        let date = Utc.timestamp_opt(timestamp, 0).unwrap();
                        
                        Some(GitCommit {
                            hash: parts[0].to_string(),
                            author: parts[1].to_string(),
                            date,
                            message: parts[3].to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect();
            
            Ok(commits)
        } else {
            Err(anyhow!("Failed to get file history: {}", 
                String::from_utf8_lossy(&output.stderr)))
        }
    }
}

/// Represents a set of changes between two commits
#[derive(Debug, Clone)]
pub struct ChangeSet {
    pub commit_before: String,
    pub commit_after: String,
    pub added_files: Vec<PathBuf>,
    pub modified_files: Vec<PathBuf>,
    pub deleted_files: Vec<PathBuf>,
} 