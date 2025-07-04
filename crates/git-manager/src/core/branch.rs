//! Branch operations and management
//!
//! This module provides comprehensive branch management functionality including
//! creating, deleting, listing, and information about git branches.

use crate::{GitError, GitResult};
use git2::{Branch, BranchType, Repository};
use std::collections::HashMap;
use std::path::Path;

/// Information about a git branch
#[derive(Debug, Clone)]
pub struct BranchInfo {
    /// Name of the branch
    pub name: String,
    /// Whether this is a local or remote branch
    pub branch_type: BranchType,
    /// Whether this is the current active branch
    pub is_current: bool,
    /// SHA of the commit this branch points to
    pub commit_sha: String,
    /// Commit message of the HEAD commit
    pub commit_message: String,
    /// Author of the HEAD commit
    pub author: String,
    /// Timestamp of the HEAD commit
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Whether this branch has uncommitted changes (only for current branch)
    pub has_uncommitted_changes: Option<bool>,
    /// Remote tracking branch (for local branches)
    pub upstream: Option<String>,
    /// Number of commits ahead of upstream
    pub ahead: Option<usize>,
    /// Number of commits behind upstream
    pub behind: Option<usize>,
}

/// Options for creating a new branch
#[derive(Debug, Clone)]
pub struct CreateBranchOptions {
    /// Starting point for the new branch (commit SHA, branch name, or tag)
    pub start_point: Option<String>,
    /// Whether to force creation (overwrite if exists)
    pub force: bool,
    /// Whether to set up tracking to a remote branch
    pub track: Option<String>,
}

impl Default for CreateBranchOptions {
    fn default() -> Self {
        Self {
            start_point: None,
            force: false,
            track: None,
        }
    }
}

/// Branch manager for git operations
pub struct BranchManager {
    repo: Repository,
}

impl BranchManager {
    /// Create a new branch manager for the given repository
    pub fn new(repo_path: &Path) -> GitResult<Self> {
        let repo = Repository::open(repo_path)
            .map_err(|_e| GitError::RepositoryNotFound {
                path: repo_path.to_path_buf(),
            })?;

        Ok(Self { repo })
    }

    /// List all branches in the repository
    pub fn list_branches(&self, branch_type: Option<BranchType>) -> GitResult<Vec<BranchInfo>> {
        let mut branches = Vec::new();
        let current_branch = self.get_current_branch_name()?;

        let branch_iter = match branch_type {
            Some(bt) => self.repo.branches(Some(bt)),
            None => self.repo.branches(None),
        }
        .map_err(|e| GitError::GitOperationFailed {
            message: format!("Failed to list branches: {}", e),
        })?;

        for branch_result in branch_iter {
            let (branch, branch_type) = branch_result
                .map_err(|e| GitError::GitOperationFailed {
                    message: format!("Failed to process branch: {}", e),
                })?;

            if let Some(branch_info) = self.extract_branch_info(&branch, branch_type, &current_branch)? {
                branches.push(branch_info);
            }
        }

        Ok(branches)
    }

    /// Get information about a specific branch
    pub fn get_branch_info(&self, branch_name: &str) -> GitResult<BranchInfo> {
        let branch = self.repo.find_branch(branch_name, BranchType::Local)
            .or_else(|_| self.repo.find_branch(branch_name, BranchType::Remote))
            .map_err(|_| GitError::BranchNotFound {
                branch: branch_name.to_string(),
            })?;

        let current_branch = self.get_current_branch_name()?;
        
        // Determine branch type by checking if the branch name contains "remotes/"
        let branch_type = if branch.name().ok().flatten().map_or(false, |name| name.contains("remotes/")) {
            BranchType::Remote
        } else {
            BranchType::Local
        };

        self.extract_branch_info(&branch, branch_type, &current_branch)?
            .ok_or_else(|| GitError::BranchNotFound {
                branch: branch_name.to_string(),
            })
    }

    /// Create a new branch
    pub fn create_branch(
        &self,
        branch_name: &str,
        options: CreateBranchOptions,
    ) -> GitResult<BranchInfo> {
        // Check if branch already exists
        if !options.force && self.branch_exists(branch_name)? {
            return Err(GitError::BranchAlreadyExists {
                branch: branch_name.to_string(),
            });
        }

        // Find the starting commit
        let commit = if let Some(start_point) = &options.start_point {
            self.find_commit(start_point)?
        } else {
            // Use HEAD as default starting point
            self.repo.head()
                .and_then(|head| head.peel_to_commit())
                .map_err(|e| GitError::GitOperationFailed {
                    message: format!("Failed to get HEAD commit: {}", e),
                })?
        };

        // Create the branch
        let branch = self.repo.branch(branch_name, &commit, options.force)
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to create branch '{}': {}", branch_name, e),
            })?;

        // Set up tracking if requested
        if let Some(upstream) = &options.track {
            self.set_upstream(&branch, upstream)?;
        }

        // Get branch info
        let current_branch = self.get_current_branch_name()?;
        self.extract_branch_info(&branch, BranchType::Local, &current_branch)?
            .ok_or_else(|| GitError::GitOperationFailed {
                message: "Failed to get info for newly created branch".to_string(),
            })
    }

    /// Delete a branch
    pub fn delete_branch(&self, branch_name: &str, force: bool) -> GitResult<()> {
        let mut branch = self.repo.find_branch(branch_name, BranchType::Local)
            .map_err(|_| GitError::BranchNotFound {
                branch: branch_name.to_string(),
            })?;

        // Check if it's the current branch
        let current_branch = self.get_current_branch_name()?;
        if current_branch.as_deref() == Some(branch_name) {
            return Err(GitError::InvalidState {
                message: "Cannot delete the current branch".to_string(),
            });
        }

        // Check if branch has unmerged commits (unless force is true)
        if !force && self.has_unmerged_commits(&branch)? {
            return Err(GitError::InvalidState {
                message: format!("Branch '{}' has unmerged commits. Use force to delete anyway.", branch_name),
            });
        }

        branch.delete()
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to delete branch '{}': {}", branch_name, e),
            })
    }

    /// Rename a branch
    pub fn rename_branch(&self, old_name: &str, new_name: &str, force: bool) -> GitResult<()> {
        let mut branch = self.repo.find_branch(old_name, BranchType::Local)
            .map_err(|_| GitError::BranchNotFound {
                branch: old_name.to_string(),
            })?;

        branch.rename(new_name, force)
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to rename branch '{}' to '{}': {}", old_name, new_name, e),
            })
            .map(|_| ()) // Discard the returned Branch and return ()
    }

    /// Switch to a different branch
    pub fn switch_branch(&self, branch_name: &str, force: bool) -> GitResult<()> {
        // Check if branch exists
        let branch = self.repo.find_branch(branch_name, BranchType::Local)
            .map_err(|_| GitError::BranchNotFound {
                branch: branch_name.to_string(),
            })?;

        // Check for uncommitted changes (unless force is true)
        if !force && self.has_uncommitted_changes()? {
            return Err(GitError::UncommittedChanges {
                branch: branch_name.to_string(),
            });
        }

        // Get the target commit
        let _commit = branch.get().peel_to_commit()
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to get commit for branch '{}': {}", branch_name, e),
            })?;

        // Switch HEAD to point to the branch
        self.repo.set_head(&format!("refs/heads/{}", branch_name))
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to switch HEAD to branch '{}': {}", branch_name, e),
            })?;

        // Update working directory
        self.repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to checkout branch '{}': {}", branch_name, e),
            })?;

        Ok(())
    }

    /// Check if a branch exists
    pub fn branch_exists(&self, branch_name: &str) -> GitResult<bool> {
        Ok(self.repo.find_branch(branch_name, BranchType::Local).is_ok() ||
           self.repo.find_branch(branch_name, BranchType::Remote).is_ok())
    }

    /// Get the current branch name
    pub fn get_current_branch_name(&self) -> GitResult<Option<String>> {
        match self.repo.head() {
            Ok(head) => {
                if let Some(shorthand) = head.shorthand() {
                    Ok(Some(shorthand.to_string()))
                } else {
                    Ok(None) // Detached HEAD
                }
            }
            Err(_) => Ok(None), // No HEAD (empty repository)
        }
    }

    /// Check if repository has uncommitted changes
    pub fn has_uncommitted_changes(&self) -> GitResult<bool> {
        let statuses = self.repo.statuses(None)
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to get repository status: {}", e),
            })?;

        Ok(!statuses.is_empty())
    }

    /// Get detailed status of the working directory
    pub fn get_status(&self) -> GitResult<HashMap<String, git2::Status>> {
        let mut status_map = HashMap::new();
        let statuses = self.repo.statuses(None)
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to get repository status: {}", e),
            })?;

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                status_map.insert(path.to_string(), entry.status());
            }
        }

        Ok(status_map)
    }

    // Helper methods

    fn extract_branch_info(
        &self,
        branch: &Branch,
        branch_type: BranchType,
        current_branch: &Option<String>,
    ) -> GitResult<Option<BranchInfo>> {
        let name = match branch.name() {
            Ok(Some(name)) => name.to_string(),
            Ok(None) => return Ok(None), // Skip branches with invalid names
            Err(_) => return Ok(None),
        };

        let commit = match branch.get().peel_to_commit() {
            Ok(commit) => commit,
            Err(_) => return Ok(None), // Skip branches that can't be resolved to commits
        };

        let is_current = current_branch.as_ref() == Some(&name);
        let commit_sha = commit.id().to_string();
        let commit_message = commit.message().unwrap_or("").to_string();
        let author = commit.author().name().unwrap_or("Unknown").to_string();
        let timestamp = chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
            .unwrap_or_else(|| chrono::Utc::now());

        let has_uncommitted_changes = if is_current {
            Some(self.has_uncommitted_changes()?)
        } else {
            None
        };

        // TODO: Implement upstream tracking and ahead/behind calculation
        let upstream = None;
        let ahead = None;
        let behind = None;

        Ok(Some(BranchInfo {
            name,
            branch_type,
            is_current,
            commit_sha,
            commit_message,
            author,
            timestamp,
            has_uncommitted_changes,
            upstream,
            ahead,
            behind,
        }))
    }

    fn find_commit(&self, rev_spec: &str) -> GitResult<git2::Commit> {
        let obj = self.repo.revparse_single(rev_spec)
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to resolve '{}': {}", rev_spec, e),
            })?;

        obj.peel_to_commit()
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("'{}' does not point to a commit: {}", rev_spec, e),
            })
    }

    fn set_upstream(&self, _branch: &Branch, _upstream: &str) -> GitResult<()> {
        // TODO: Implement upstream tracking
        Ok(())
    }

    fn has_unmerged_commits(&self, _branch: &Branch) -> GitResult<bool> {
        // TODO: Implement unmerged commit detection
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::path::PathBuf;

    fn create_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        let repo = Repository::init(&repo_path).unwrap();
        
        // Create initial commit
        let sig = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
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

        drop(tree);
        drop(repo);

        (temp_dir, repo_path)
    }

    #[test]
    fn test_branch_manager_creation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = BranchManager::new(&repo_path).unwrap();
        
        // Should be able to get current branch (main/master)
        let current = manager.get_current_branch_name().unwrap();
        assert!(current.is_some());
    }

    #[test]
    fn test_list_branches() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = BranchManager::new(&repo_path).unwrap();
        
        let branches = manager.list_branches(Some(BranchType::Local)).unwrap();
        assert!(!branches.is_empty());
        
        // Should have at least the main/master branch
        let main_branch = branches.iter().find(|b| b.is_current).unwrap();
        assert!(main_branch.name == "main" || main_branch.name == "master");
    }

    #[test]
    fn test_create_branch() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = BranchManager::new(&repo_path).unwrap();
        
        let options = CreateBranchOptions::default();
        let branch_info = manager.create_branch("feature/test", options).unwrap();
        
        assert_eq!(branch_info.name, "feature/test");
        assert_eq!(branch_info.branch_type, BranchType::Local);
        assert!(!branch_info.is_current);
    }

    #[test]
    fn test_branch_exists() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = BranchManager::new(&repo_path).unwrap();
        
        // Create a test branch
        let options = CreateBranchOptions::default();
        manager.create_branch("test-branch", options).unwrap();
        
        assert!(manager.branch_exists("test-branch").unwrap());
        assert!(!manager.branch_exists("non-existent-branch").unwrap());
    }

    #[test]
    fn test_delete_branch() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = BranchManager::new(&repo_path).unwrap();
        
        // Create and then delete a test branch
        let options = CreateBranchOptions::default();
        manager.create_branch("temp-branch", options).unwrap();
        assert!(manager.branch_exists("temp-branch").unwrap());
        
        manager.delete_branch("temp-branch", false).unwrap();
        assert!(!manager.branch_exists("temp-branch").unwrap());
    }

    #[test]
    fn test_switch_branch() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = BranchManager::new(&repo_path).unwrap();
        
        // Create a new branch
        let options = CreateBranchOptions::default();
        manager.create_branch("switch-test", options).unwrap();
        
        // Switch to the new branch
        manager.switch_branch("switch-test", false).unwrap();
        
        let current = manager.get_current_branch_name().unwrap();
        assert_eq!(current, Some("switch-test".to_string()));
    }

    #[test]
    fn test_get_branch_info() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = BranchManager::new(&repo_path).unwrap();
        
        // Get info for the current branch
        let current_name = manager.get_current_branch_name().unwrap().unwrap();
        let branch_info = manager.get_branch_info(&current_name).unwrap();
        
        assert_eq!(branch_info.name, current_name);
        assert!(branch_info.is_current);
        assert_eq!(branch_info.branch_type, BranchType::Local);
    }
} 