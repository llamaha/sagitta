//! Change management operations
//!
//! This module provides functionality for managing changes in a git repository
//! including staging files, committing changes, and synchronizing with remotes.

use crate::{GitError, GitResult};
use git2::{Repository, Signature, Oid, FetchOptions, RemoteCallbacks};
use std::path::{Path, PathBuf};

/// Options for committing changes
#[derive(Debug, Clone)]
pub struct CommitOptions {
    /// Commit message
    pub message: String,
    /// Author signature (if None, uses repository default)
    pub author: Option<GitSignature>,
    /// Committer signature (if None, uses repository default)
    pub committer: Option<GitSignature>,
    /// Whether to allow empty commits
    pub allow_empty: bool,
    /// Whether to amend the last commit
    pub amend: bool,
}

/// Git signature information
#[derive(Debug, Clone)]
pub struct GitSignature {
    pub name: String,
    pub email: String,
}

/// Options for pushing changes
#[derive(Debug, Clone)]
pub struct GitPushOptions {
    /// Remote name (defaults to "origin")
    pub remote: String,
    /// Branch to push (if None, pushes current branch)
    pub branch: Option<String>,
    /// Whether to force push
    pub force: bool,
    /// Whether to set upstream tracking
    pub set_upstream: bool,
}

impl Default for GitPushOptions {
    fn default() -> Self {
        Self {
            remote: "origin".to_string(),
            branch: None,
            force: false,
            set_upstream: false,
        }
    }
}

/// Options for pulling changes
#[derive(Debug, Clone)]
pub struct PullOptions {
    /// Remote name (defaults to "origin")
    pub remote: String,
    /// Branch to pull (if None, pulls current branch)
    pub branch: Option<String>,
    /// Whether to rebase instead of merge
    pub rebase: bool,
    /// Whether to fast-forward only
    pub fast_forward_only: bool,
}

impl Default for PullOptions {
    fn default() -> Self {
        Self {
            remote: "origin".to_string(),
            branch: None,
            rebase: false,
            fast_forward_only: false,
        }
    }
}

/// Result of a commit operation
#[derive(Debug)]
pub struct CommitResult {
    /// SHA of the created commit
    pub commit_sha: String,
    /// Commit message
    pub message: String,
    /// Number of files changed
    pub files_changed: usize,
    /// Number of insertions
    pub insertions: usize,
    /// Number of deletions
    pub deletions: usize,
}

/// Result of a push operation
#[derive(Debug)]
pub struct PushResult {
    /// Whether the push was successful
    pub success: bool,
    /// Remote that was pushed to
    pub remote: String,
    /// Branch that was pushed
    pub branch: String,
    /// Number of commits pushed
    pub commits_pushed: usize,
    /// Any error message
    pub error_message: Option<String>,
}

/// Result of a pull operation
#[derive(Debug)]
pub struct PullResult {
    /// Whether the pull was successful
    pub success: bool,
    /// Remote that was pulled from
    pub remote: String,
    /// Branch that was pulled
    pub branch: String,
    /// Number of commits pulled
    pub commits_pulled: usize,
    /// Whether a merge was performed
    pub merge_performed: bool,
    /// Any error message
    pub error_message: Option<String>,
}

/// Change manager for git operations
pub struct ChangeManager {
    repo: Repository,
}

impl ChangeManager {
    /// Create a new change manager for the given repository
    pub fn new(repo_path: &Path) -> GitResult<Self> {
        let repo = Repository::open(repo_path)
            .map_err(|_| GitError::RepositoryNotFound {
                path: repo_path.to_path_buf(),
            })?;

        Ok(Self { repo })
    }

    /// Stage files for commit
    ///
    /// # Arguments
    /// * `files` - List of file paths to stage (relative to repository root)
    ///   If empty, stages all modified files
    pub fn stage_files(&self, files: &[PathBuf]) -> GitResult<usize> {
        let mut index = self.repo.index()
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to get repository index: {}", e),
            })?;

        let files_staged = if files.is_empty() {
            // Stage all modified files
            index.add_all(&["*"], git2::IndexAddOption::DEFAULT, None)
                .map_err(|e| GitError::GitOperationFailed {
                    message: format!("Failed to stage all files: {}", e),
                })?;
            
            index.write()
                .map_err(|e| GitError::GitOperationFailed {
                    message: format!("Failed to write index: {}", e),
                })?;

            // Count staged files
            let statuses = self.repo.statuses(None)
                .map_err(|e| GitError::GitOperationFailed {
                    message: format!("Failed to get repository status: {}", e),
                })?;
            
            statuses.iter()
                .filter(|entry| entry.status().is_index_modified() || entry.status().is_index_new())
                .count()
        } else {
            // Stage specific files
            let mut count = 0;
            for file_path in files {
                match index.add_path(file_path) {
                    Ok(_) => count += 1,
                    Err(e) => return Err(GitError::GitOperationFailed {
                        message: format!("Failed to stage file '{}': {}", file_path.display(), e),
                    }),
                }
            }
            
            index.write()
                .map_err(|e| GitError::GitOperationFailed {
                    message: format!("Failed to write index: {}", e),
                })?;
            
            count
        };

        Ok(files_staged)
    }

    /// Unstage files
    pub fn unstage_files(&self, files: &[PathBuf]) -> GitResult<usize> {
        let mut index = self.repo.index()
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to get repository index: {}", e),
            })?;

        let mut count = 0;
        for file_path in files {
            match index.remove_path(file_path) {
                Ok(_) => count += 1,
                Err(e) => return Err(GitError::GitOperationFailed {
                    message: format!("Failed to unstage file '{}': {}", file_path.display(), e),
                }),
            }
        }

        index.write()
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to write index: {}", e),
            })?;

        Ok(count)
    }

    /// Commit staged changes
    pub fn commit(&self, options: CommitOptions) -> GitResult<CommitResult> {
        let mut index = self.repo.index()
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to get repository index: {}", e),
            })?;

        // Check if there are staged changes (unless allowing empty commits)
        if !options.allow_empty {
            let tree_id = index.write_tree()
                .map_err(|e| GitError::GitOperationFailed {
                    message: format!("Failed to write tree: {}", e),
                })?;

            // Check if this would be an empty commit
            if let Ok(head) = self.repo.head() {
                if let Ok(head_commit) = head.peel_to_commit() {
                    if head_commit.tree_id() == tree_id {
                        return Err(GitError::InvalidState {
                            message: "No changes to commit".to_string(),
                        });
                    }
                }
            }
        }

        // Get signatures
        let author = self.get_signature(options.author.as_ref())?;
        let committer = self.get_signature(options.committer.as_ref())?;

        // Write tree
        let tree_id = index.write_tree()
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to write tree: {}", e),
            })?;

        let tree = self.repo.find_tree(tree_id)
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to find tree: {}", e),
            })?;

        // Get parent commits
        let parents = if options.amend {
            // For amend, use parent commits of the current HEAD
            match self.repo.head() {
                Ok(head) => {
                    let head_commit = head.peel_to_commit()
                        .map_err(|e| GitError::GitOperationFailed {
                            message: format!("Failed to get HEAD commit: {}", e),
                        })?;
                    head_commit.parents().collect::<Vec<_>>()
                }
                Err(_) => vec![], // No HEAD (empty repository)
            }
        } else {
            // Normal commit, use current HEAD as parent
            match self.repo.head() {
                Ok(head) => {
                    let head_commit = head.peel_to_commit()
                        .map_err(|e| GitError::GitOperationFailed {
                            message: format!("Failed to get HEAD commit: {}", e),
                        })?;
                    vec![head_commit]
                }
                Err(_) => vec![], // No HEAD (empty repository)
            }
        };

        // Create commit
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        let commit_id = self.repo.commit(
            Some("HEAD"),
            &author,
            &committer,
            &options.message,
            &tree,
            &parent_refs,
        )
        .map_err(|e| GitError::GitOperationFailed {
            message: format!("Failed to create commit: {}", e),
        })?;

        // Calculate statistics
        let (files_changed, insertions, deletions) = self.calculate_commit_stats(&commit_id)?;

        Ok(CommitResult {
            commit_sha: commit_id.to_string(),
            message: options.message,
            files_changed,
            insertions,
            deletions,
        })
    }

    /// Push changes to remote
    pub fn push(&self, options: GitPushOptions) -> GitResult<PushResult> {
        let current_branch = self.get_current_branch_name()?
            .ok_or_else(|| GitError::InvalidState {
                message: "Not on any branch (detached HEAD)".to_string(),
            })?;

        let branch_to_push = options.branch.as_deref().unwrap_or(&current_branch);
        
        // Find the remote
        let mut remote = self.repo.find_remote(&options.remote)
            .map_err(|_| GitError::RemoteNotFound {
                remote: options.remote.clone(),
            })?;

        // Set up callbacks
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, _username_from_url, _allowed_types| {
            git2::Cred::default()
        });

        // Push the branch
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch_to_push, branch_to_push);
        let mut push_options = git2::PushOptions::new();
        push_options.remote_callbacks(callbacks);

        match remote.push(&[&refspec], Some(&mut push_options)) {
            Ok(_) => Ok(PushResult {
                success: true,
                remote: options.remote,
                branch: branch_to_push.to_string(),
                commits_pushed: 1, // TODO: Calculate actual number
                error_message: None,
            }),
            Err(e) => Ok(PushResult {
                success: false,
                remote: options.remote,
                branch: branch_to_push.to_string(),
                commits_pushed: 0,
                error_message: Some(format!("Push failed: {}", e)),
            }),
        }
    }

    /// Pull changes from remote
    pub fn pull(&self, options: PullOptions) -> GitResult<PullResult> {
        let current_branch = self.get_current_branch_name()?
            .ok_or_else(|| GitError::InvalidState {
                message: "Not on any branch (detached HEAD)".to_string(),
            })?;

        let branch_to_pull = options.branch.as_deref().unwrap_or(&current_branch);

        // Find the remote
        let mut remote = self.repo.find_remote(&options.remote)
            .map_err(|_| GitError::RemoteNotFound {
                remote: options.remote.clone(),
            })?;

        // Set up callbacks
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, _username_from_url, _allowed_types| {
            git2::Cred::default()
        });

        // Fetch from remote
        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let refspecs: Vec<String> = remote.fetch_refspecs()
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to get fetch refspecs: {}", e),
            })?
            .iter()
            .map(|s| s.unwrap().to_string())
            .collect();

        let refspec_strs: Vec<&str> = refspecs.iter().map(|s| s.as_str()).collect();
        
        match remote.fetch(&refspec_strs, Some(&mut fetch_options), None) {
            Ok(_) => {
                // TODO: Implement merge/rebase logic after fetch
                // For now, just return a successful fetch result
                Ok(PullResult {
                    success: true,
                    remote: options.remote,
                    branch: branch_to_pull.to_string(),
                    commits_pulled: 0, // TODO: Calculate actual number
                    merge_performed: false, // TODO: Track if merge was performed
                    error_message: None,
                })
            }
            Err(e) => Ok(PullResult {
                success: false,
                remote: options.remote,
                branch: branch_to_pull.to_string(),
                commits_pulled: 0,
                merge_performed: false,
                error_message: Some(format!("Fetch failed: {}", e)),
            }),
        }
    }

    /// Get the current branch name
    fn get_current_branch_name(&self) -> GitResult<Option<String>> {
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

    /// Get signature for commit
    fn get_signature(&self, sig_option: Option<&GitSignature>) -> GitResult<Signature> {
        if let Some(sig) = sig_option {
            Signature::now(&sig.name, &sig.email)
                .map_err(|e| GitError::GitOperationFailed {
                    message: format!("Failed to create signature: {}", e),
                })
        } else {
            // Use repository default signature
            self.repo.signature()
                .map_err(|e| GitError::GitOperationFailed {
                    message: format!("Failed to get default signature: {}", e),
                })
        }
    }

    /// Calculate commit statistics
    fn calculate_commit_stats(&self, _commit_id: &Oid) -> GitResult<(usize, usize, usize)> {
        // TODO: Implement proper commit statistics calculation
        // This would involve comparing the commit tree with its parent
        Ok((1, 0, 0)) // Placeholder values
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        let repo = Repository::init(&repo_path).unwrap();
        
        // Configure test repository with default user
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        
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
    fn test_change_manager_creation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = ChangeManager::new(&repo_path);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_stage_files() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = ChangeManager::new(&repo_path).unwrap();
        
        // Create a test file
        let test_file = repo_path.join("test.txt");
        fs::write(&test_file, "test content").unwrap();
        
        // Stage the file
        let result = manager.stage_files(&[PathBuf::from("test.txt")]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn test_commit() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = ChangeManager::new(&repo_path).unwrap();
        
        // Create and stage a test file
        let test_file = repo_path.join("test.txt");
        fs::write(&test_file, "test content").unwrap();
        manager.stage_files(&[PathBuf::from("test.txt")]).unwrap();
        
        // Commit the changes
        let options = CommitOptions {
            message: "Test commit".to_string(),
            author: Some(GitSignature {
                name: "Test Author".to_string(),
                email: "test@example.com".to_string(),
            }),
            committer: None,
            allow_empty: false,
            amend: false,
        };
        
        let result = manager.commit(options);
        assert!(result.is_ok());
        
        let commit_result = result.unwrap();
        assert_eq!(commit_result.message, "Test commit");
        assert!(!commit_result.commit_sha.is_empty());
    }

    #[test]
    fn test_stage_all_files() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = ChangeManager::new(&repo_path).unwrap();
        
        // Create multiple test files
        fs::write(repo_path.join("test1.txt"), "content1").unwrap();
        fs::write(repo_path.join("test2.txt"), "content2").unwrap();
        
        // Stage all files (empty vec)
        let result = manager.stage_files(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_commit_with_custom_signature() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = ChangeManager::new(&repo_path).unwrap();
        
        // Create and stage a test file
        let test_file = repo_path.join("test.txt");
        fs::write(&test_file, "test content").unwrap();
        manager.stage_files(&[PathBuf::from("test.txt")]).unwrap();
        
        // Commit with custom signature
        let options = CommitOptions {
            message: "Custom signature commit".to_string(),
            author: Some(GitSignature {
                name: "Custom Author".to_string(),
                email: "custom@example.com".to_string(),
            }),
            committer: Some(GitSignature {
                name: "Custom Committer".to_string(),
                email: "committer@example.com".to_string(),
            }),
            allow_empty: false,
            amend: false,
        };
        
        let result = manager.commit(options);
        assert!(result.is_ok());
    }
} 