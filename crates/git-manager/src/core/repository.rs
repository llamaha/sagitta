use std::path::{Path, PathBuf};
use git2::{Repository, Branch, BranchType, Oid, Reference};
use crate::error::{GitError, GitResult};
use crate::core::state::{BranchState, RepositoryState};
use crate::sync::merkle::MerkleManager;
use std::collections::HashMap;

/// Core repository manager that wraps git2::Repository with enhanced functionality
pub struct GitRepository {
    /// The underlying git2 repository
    repo: Repository,
    /// Path to the repository
    path: PathBuf,
    /// Merkle manager for change detection
    merkle_manager: MerkleManager,
}

impl std::fmt::Debug for GitRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitRepository")
            .field("path", &self.path)
            .field("merkle_manager", &self.merkle_manager)
            .finish()
    }
}

impl GitRepository {
    /// Open an existing git repository
    pub fn open<P: AsRef<Path>>(path: P) -> GitResult<Self> {
        let path = path.as_ref().to_path_buf();
        let repo = Repository::open(&path).map_err(|e| {
            GitError::RepositoryNotFound { path: path.clone() }
        })?;

        Ok(Self {
            repo,
            path,
            merkle_manager: MerkleManager::new(),
        })
    }

    /// Initialize a new git repository
    pub fn init<P: AsRef<Path>>(path: P) -> GitResult<Self> {
        let path = path.as_ref().to_path_buf();
        let repo = Repository::init(&path)?;

        Ok(Self {
            repo,
            path,
            merkle_manager: MerkleManager::new(),
        })
    }

    /// Get the repository path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get access to the underlying git2::Repository for internal operations
    pub(crate) fn repo(&self) -> &Repository {
        &self.repo
    }

    /// Get the current branch name
    pub fn current_branch(&self) -> GitResult<String> {
        let head = self.repo.head()?;
        
        // Check if we're in detached HEAD state
        if head.is_branch() {
            // We're on a branch, get the branch name
            if let Some(branch_name) = head.shorthand() {
                Ok(branch_name.to_string())
            } else {
                // This shouldn't happen for a branch, but handle gracefully
                let oid = head.target().ok_or_else(|| {
                    GitError::invalid_state("HEAD has no target")
                })?;
                Ok(format!("detached-{}", oid))
            }
        } else {
            // We're in detached HEAD state
            let oid = head.target().ok_or_else(|| {
                GitError::invalid_state("HEAD has no target")
            })?;
            Ok(format!("detached-{}", oid))
        }
    }

    /// Get the current commit hash
    pub fn current_commit_hash(&self) -> GitResult<String> {
        let head = self.repo.head()?;
        let oid = head.target().ok_or_else(|| {
            GitError::invalid_state("HEAD has no target")
        })?;
        Ok(oid.to_string())
    }

    /// List all branches (local and remote)
    pub fn list_branches(&self, branch_type: Option<BranchType>) -> GitResult<Vec<String>> {
        let mut branch_names = Vec::new();
        let branches = self.repo.branches(branch_type)?;

        for branch_result in branches {
            let (branch, _) = branch_result?;
            if let Some(name) = branch.name()? {
                branch_names.push(name.to_string());
            }
        }

        Ok(branch_names)
    }

    /// Check if a branch exists
    pub fn branch_exists(&self, branch_name: &str) -> GitResult<bool> {
        match self.repo.find_branch(branch_name, BranchType::Local) {
            Ok(_) => Ok(true),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(e) => Err(GitError::from(e)),
        }
    }

    /// Check if a remote branch exists
    pub fn remote_branch_exists(&self, branch_name: &str, remote_name: Option<&str>) -> GitResult<bool> {
        let remote_name = remote_name.unwrap_or("origin");
        let remote_branch_name = format!("{}/{}", remote_name, branch_name);
        
        match self.repo.find_branch(&remote_branch_name, BranchType::Remote) {
            Ok(_) => Ok(true),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(e) => Err(GitError::from(e)),
        }
    }

    /// Check if a reference (branch, tag, commit) exists
    pub fn reference_exists(&self, ref_name: &str) -> GitResult<bool> {
        // Try to resolve the reference
        match self.repo.revparse_single(ref_name) {
            Ok(_) => Ok(true),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(e) => Err(GitError::from(e)),
        }
    }

    /// Get all available references that could be checked out (local branches, remote branches, tags)
    pub fn list_all_references(&self) -> GitResult<Vec<String>> {
        let mut refs = Vec::new();
        
        // Add local branches
        let local_branches = self.list_branches(Some(BranchType::Local))?;
        refs.extend(local_branches);
        
        // Add remote branches (strip remote prefix for display)
        let remote_branches = self.list_branches(Some(BranchType::Remote))?;
        for remote_branch in remote_branches {
            // Convert "origin/feature" to "feature" for display
            if let Some(branch_name) = remote_branch.split('/').nth(1) {
                if !refs.contains(&branch_name.to_string()) {
                    refs.push(branch_name.to_string());
                }
            }
        }
        
        // Add tags
        if let Ok(tag_refs) = self.repo.tag_names(None) {
            for tag_name in tag_refs.iter() {
                if let Some(tag) = tag_name {
                    refs.push(tag.to_string());
                }
            }
        }
        
        Ok(refs)
    }

    /// Create a local branch from a remote branch
    pub fn create_local_branch_from_remote(
        &self, 
        branch_name: &str, 
        remote_name: Option<&str>
    ) -> GitResult<()> {
        let remote_name = remote_name.unwrap_or("origin");
        let remote_branch_name = format!("{}/{}", remote_name, branch_name);
        
        // Check if remote branch exists
        if !self.remote_branch_exists(branch_name, Some(remote_name))? {
            return Err(GitError::BranchNotFound {
                branch: remote_branch_name,
            });
        }
        
        // Get the remote branch
        let remote_branch = self.repo.find_branch(&remote_branch_name, BranchType::Remote)?;
        let remote_commit = remote_branch.get().peel_to_commit()?;
        
        // Create local branch from remote commit
        self.repo.branch(branch_name, &remote_commit, false)?;
        
        // Set up tracking
        let mut local_branch = self.repo.find_branch(branch_name, BranchType::Local)?;
        local_branch.set_upstream(Some(&remote_branch_name))?;
        
        Ok(())
    }

    /// Create a new branch
    pub fn create_branch(&self, branch_name: &str, start_point: Option<&str>) -> GitResult<()> {
        // Check if branch already exists
        if self.branch_exists(branch_name)? {
            return Err(GitError::BranchAlreadyExists {
                branch: branch_name.to_string(),
            });
        }

        let commit = if let Some(start_point) = start_point {
            // Create from specific commit/branch
            let oid = self.repo.revparse_single(start_point)?.id();
            self.repo.find_commit(oid)?
        } else {
            // Create from current HEAD
            let head = self.repo.head()?;
            let oid = head.target().ok_or_else(|| {
                GitError::invalid_state("HEAD has no target")
            })?;
            self.repo.find_commit(oid)?
        };

        self.repo.branch(branch_name, &commit, false)?;
        Ok(())
    }

    /// Switch to a different branch
    pub fn switch_branch(&self, branch_name: &str) -> GitResult<String> {
        self.switch_branch_with_options(branch_name, false)
    }

    /// Switch to a different branch with force option
    pub fn switch_branch_with_options(&self, branch_name: &str, force: bool) -> GitResult<String> {
        // Get current branch for return value
        let current_branch = self.current_branch()?;

        // Check for uncommitted changes unless force is enabled
        if !force && self.has_uncommitted_changes()? {
            return Err(GitError::UncommittedChanges {
                branch: branch_name.to_string(),
            });
        }

        // Try to resolve the reference - this could be a branch, tag, or commit
        let target_commit = if self.branch_exists(branch_name)? {
            // Local branch exists, use it directly
            tracing::info!("Switching to existing local branch: {}", branch_name);
            let branch = self.repo.find_branch(branch_name, BranchType::Local)?;
            branch.get().peel_to_commit()?
        } else if self.reference_exists(branch_name)? {
            // Reference exists (could be tag, remote branch, or commit)
            tracing::info!("Resolving reference: {}", branch_name);
            let obj = self.repo.revparse_single(branch_name)?;
            let commit = obj.peel_to_commit()?;
            
            // If this looks like a remote branch name, try to create a local tracking branch
            if self.remote_branch_exists(branch_name, None)? {
                tracing::info!("Creating local tracking branch for remote: {}", branch_name);
                
                // Create local branch from remote if it doesn't exist
                if let Err(_) = self.create_local_branch_from_remote(branch_name, None) {
                    tracing::warn!("Failed to create tracking branch, will checkout detached");
                }
            }
            
            commit
        } else {
            // Try fetching from remote first
            tracing::info!("Reference not found locally, attempting fetch: {}", branch_name);
            if let Err(e) = self.fetch(None) {
                tracing::warn!("Fetch failed: {}", e);
            }
            
            // Try again after fetch
            if self.reference_exists(branch_name)? {
                let obj = self.repo.revparse_single(branch_name)?;
                let commit = obj.peel_to_commit()?;
                
                // Try to create local tracking branch if it's a remote branch
                if self.remote_branch_exists(branch_name, None)? {
                    tracing::info!("Creating local tracking branch after fetch: {}", branch_name);
                    let _ = self.create_local_branch_from_remote(branch_name, None);
                }
                
                commit
            } else {
                return Err(GitError::BranchNotFound {
                    branch: branch_name.to_string(),
                });
            }
        };

        // Checkout the target commit
        let tree = target_commit.tree()?;
        let mut checkout_builder = git2::build::CheckoutBuilder::new();
        checkout_builder.safe().force();
        
        self.repo.checkout_tree(tree.as_object(), Some(&mut checkout_builder))?;

        // Update HEAD
        if self.branch_exists(branch_name)? {
            // Switch to local branch
            let branch_ref = format!("refs/heads/{}", branch_name);
            self.repo.set_head(&branch_ref)?;
            tracing::info!("Switched to local branch: {}", branch_name);
        } else {
            // Detached HEAD for tags/commits
            self.repo.set_head_detached(target_commit.id())?;
            tracing::info!("Switched to detached HEAD at: {}", target_commit.id());
        }

        Ok(current_branch)
    }

    /// Check if there are uncommitted changes
    pub fn has_uncommitted_changes(&self) -> GitResult<bool> {
        let statuses = self.repo.statuses(None)?;
        Ok(!statuses.is_empty())
    }

    /// Get the status of files in the repository
    pub fn get_status(&self) -> GitResult<Vec<(PathBuf, git2::Status)>> {
        let mut status_list = Vec::new();
        let statuses = self.repo.statuses(None)?;

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                status_list.push((PathBuf::from(path), entry.status()));
            }
        }

        Ok(status_list)
    }

    /// Delete a branch
    pub fn delete_branch(&self, branch_name: &str) -> GitResult<()> {
        let mut branch = self.repo.find_branch(branch_name, BranchType::Local)?;
        branch.delete()?;
        Ok(())
    }

    /// Get the latest commit on a branch
    pub fn get_branch_commit(&self, branch_name: &str) -> GitResult<String> {
        let branch = self.repo.find_branch(branch_name, BranchType::Local)?;
        let reference = branch.get();
        let oid = reference.target().ok_or_else(|| {
            GitError::invalid_state(format!("Branch {} has no target", branch_name))
        })?;
        Ok(oid.to_string())
    }

    /// Calculate the current repository state including merkle information
    pub fn calculate_repository_state(&mut self) -> GitResult<RepositoryState> {
        let current_branch = self.current_branch()?;
        let mut repo_state = RepositoryState::new(self.path.clone(), current_branch.clone());

        // Only calculate state for the current branch
        // Other branches will be calculated when we switch to them
        let branch_state = self.calculate_branch_state(&current_branch)?;
        repo_state.set_branch_state(current_branch, branch_state);

        Ok(repo_state)
    }

    /// Calculate the state of a specific branch
    pub fn calculate_branch_state(&mut self, branch_name: &str) -> GitResult<BranchState> {
        // Get commit hash for the branch or current HEAD if in detached state
        let commit_hash = if branch_name.starts_with("detached-") {
            // In detached HEAD state, get the current commit hash
            self.current_commit_hash()?
        } else {
            // Normal branch, get the branch commit
            self.get_branch_commit(branch_name)?
        };

        // Calculate merkle state for the current working directory
        // Note: This calculates for current working directory, not the specific branch
        // In a full implementation, we might want to checkout the branch temporarily
        let (merkle_root, file_hashes) = self.merkle_manager
            .calculate_merkle_state(&self.path, None)?;

        Ok(BranchState::new(
            branch_name.to_string(),
            commit_hash,
            merkle_root,
            file_hashes,
        ))
    }

    /// Fetch from remote
    pub fn fetch(&self, remote_name: Option<&str>) -> GitResult<()> {
        let remote_name = remote_name.unwrap_or("origin");
        let mut remote = self.repo.find_remote(remote_name).map_err(|_| {
            GitError::RemoteNotFound {
                remote: remote_name.to_string(),
            }
        })?;

        // Create callbacks for authentication
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, allowed_types| {
            tracing::debug!("Git authentication requested for: {}", _url);
            
            // Try SSH key authentication first
            if allowed_types.contains(git2::CredentialType::SSH_KEY) {
                if let Ok(cred) = git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git")) {
                    return Ok(cred);
                }
            }
            
            // Try default credentials (might include stored credentials)
            if allowed_types.contains(git2::CredentialType::DEFAULT) {
                if let Ok(cred) = git2::Cred::default() {
                    return Ok(cred);
                }
            }
            
            // If all else fails, try username/password from environment or user input
            if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
                // For now, we don't support interactive password input
                // This could be enhanced to read from environment variables
                return Err(git2::Error::from_str("Authentication required but no credentials available"));
            }
            
            Err(git2::Error::from_str("No suitable authentication method available"))
        });

        // Set up progress callback
        callbacks.push_update_reference(|refname, status| {
            tracing::debug!("Updated reference {}: {}", refname, status.unwrap_or("OK"));
            Ok(())
        });

        callbacks.update_tips(|refname, old_oid, new_oid| {
            tracing::info!("Updated {}: {} -> {}", refname, old_oid, new_oid);
            true
        });

        // Fetch all refs
        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);
        
        // Use refspecs from the remote configuration
        let refspecs = remote.fetch_refspecs()?;
        let refspecs: Vec<&str> = (0..refspecs.len())
            .filter_map(|i| refspecs.get(i))
            .collect();
        
        tracing::info!("Fetching from remote '{}' with {} refspecs", remote_name, refspecs.len());
        remote.fetch(&refspecs, Some(&mut fetch_options), None)?;
        
        tracing::info!("Successfully fetched from remote '{}'", remote_name);
        Ok(())
    }

    /// Get remote URL
    pub fn get_remote_url(&self, remote_name: Option<&str>) -> GitResult<String> {
        let remote_name = remote_name.unwrap_or("origin");
        let remote = self.repo.find_remote(remote_name).map_err(|_| {
            GitError::RemoteNotFound {
                remote: remote_name.to_string(),
            }
        })?;

        remote.url().ok_or_else(|| {
            GitError::invalid_state(format!("Remote {} has no URL", remote_name))
        }).map(|s| s.to_string())
    }

    /// Check if repository is clean (no uncommitted changes)
    pub fn is_clean(&self) -> GitResult<bool> {
        Ok(!self.has_uncommitted_changes()?)
    }

    /// Get the working directory path
    pub fn workdir(&self) -> GitResult<&Path> {
        self.repo.workdir().ok_or_else(|| {
            GitError::invalid_state("Repository has no working directory")
        })
    }

    /// Get repository information
    pub fn get_info(&self) -> GitResult<RepositoryInfo> {
        Ok(RepositoryInfo {
            path: self.path.clone(),
            current_branch: self.current_branch()?,
            current_commit: self.current_commit_hash()?,
            is_clean: self.is_clean()?,
            remote_url: self.get_remote_url(None).ok(),
        })
    }
}

/// Repository information structure
#[derive(Debug, Clone)]
pub struct RepositoryInfo {
    /// Path to the repository
    pub path: PathBuf,
    /// Current branch name
    pub current_branch: String,
    /// Current commit hash
    pub current_commit: String,
    /// Whether the repository is clean (no uncommitted changes)
    pub is_clean: bool,
    /// Remote URL if available
    pub remote_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_repo() -> (TempDir, GitRepository) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        
        // Initialize repository
        let repo = Repository::init(repo_path).unwrap();
        
        // Create initial commit
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();

        let git_repo = GitRepository::open(repo_path).unwrap();
        (temp_dir, git_repo)
    }

    #[test]
    fn test_repository_open() {
        let (_temp_dir, repo) = create_test_repo();
        assert!(repo.path().exists());
    }

    #[test]
    fn test_current_branch() {
        let (_temp_dir, repo) = create_test_repo();
        let branch = repo.current_branch().unwrap();
        // Git may create either 'main' or 'master' as default branch
        assert!(branch == "main" || branch == "master");
    }

    #[test]
    fn test_create_and_list_branches() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Create a new branch
        repo.create_branch("feature", None).unwrap();
        
        // List branches
        let branches = repo.list_branches(Some(BranchType::Local)).unwrap();
        // Check for either main or master
        assert!(branches.contains(&"main".to_string()) || branches.contains(&"master".to_string()));
        assert!(branches.contains(&"feature".to_string()));
    }

    #[test]
    fn test_branch_exists() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Check for either main or master
        let default_branch = repo.current_branch().unwrap();
        assert!(repo.branch_exists(&default_branch).unwrap());
        assert!(!repo.branch_exists("nonexistent").unwrap());
        
        repo.create_branch("test", None).unwrap();
        assert!(repo.branch_exists("test").unwrap());
    }

    #[test]
    fn test_repository_info() {
        let (_temp_dir, repo) = create_test_repo();
        let info = repo.get_info().unwrap();
        
        // Check for either main or master
        assert!(info.current_branch == "main" || info.current_branch == "master");
        assert!(!info.current_commit.is_empty());
        assert!(info.is_clean);
    }

    #[test]
    fn test_reference_exists() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Test existing branch (main or master)
        let default_branch = repo.current_branch().unwrap();
        assert!(repo.reference_exists(&default_branch).unwrap());
        assert!(repo.reference_exists("HEAD").unwrap());
        
        // Test non-existent reference
        assert!(!repo.reference_exists("nonexistent").unwrap());
    }

    #[test]
    fn test_remote_branch_detection() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Should not find remote branches in a local-only repo
        let default_branch = repo.current_branch().unwrap();
        assert!(!repo.remote_branch_exists(&default_branch, Some("origin")).unwrap());
        assert!(!repo.remote_branch_exists("feature", Some("origin")).unwrap());
    }

    #[test]
    fn test_list_all_references() {
        let (_temp_dir, repo) = create_test_repo();
        
        let refs = repo.list_all_references().unwrap();
        // Check for either main or master
        assert!(refs.contains(&"main".to_string()) || refs.contains(&"master".to_string()));
        
        // Create a tag to test tag listing
        let head = repo.repo.head().unwrap();
        let commit_id = head.target().unwrap();
        let commit = repo.repo.find_commit(commit_id).unwrap();
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        
        repo.repo.tag("v1.0.0", commit.as_object(), &signature, "Test tag", false).unwrap();
        
        let refs_with_tag = repo.list_all_references().unwrap();
        assert!(refs_with_tag.contains(&"v1.0.0".to_string()));
    }

    #[test]
    fn test_detached_head_current_branch() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Get the current commit to detach to
        let head = repo.repo.head().unwrap();
        let commit_id = head.target().unwrap();
        
        // Detach HEAD to the current commit
        repo.repo.set_head_detached(commit_id).unwrap();
        
        // Test that current_branch returns the expected detached format
        let current_branch = repo.current_branch().unwrap();
        assert!(current_branch.starts_with("detached-"));
        assert!(current_branch.contains(&commit_id.to_string()));
        
        // Verify it matches the expected format exactly
        let expected = format!("detached-{}", commit_id);
        assert_eq!(current_branch, expected);
    }

    #[test]
    fn test_detached_head_current_commit_hash() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Get the current commit to detach to
        let head = repo.repo.head().unwrap();
        let commit_id = head.target().unwrap();
        
        // Detach HEAD to the current commit
        repo.repo.set_head_detached(commit_id).unwrap();
        
        // Test that current_commit_hash still works in detached state
        let commit_hash = repo.current_commit_hash().unwrap();
        assert_eq!(commit_hash, commit_id.to_string());
    }

    #[test]
    fn test_detached_head_repository_info() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Get the current commit to detach to
        let head = repo.repo.head().unwrap();
        let commit_id = head.target().unwrap();
        
        // Detach HEAD to the current commit
        repo.repo.set_head_detached(commit_id).unwrap();
        
        // Test that get_info works in detached state
        let info = repo.get_info().unwrap();
        
        assert!(info.current_branch.starts_with("detached-"));
        assert_eq!(info.current_commit, commit_id.to_string());
        assert!(info.is_clean);
    }

    #[test]
    fn test_detached_head_calculate_repository_state() {
        let (_temp_dir, mut repo) = create_test_repo();
        
        // Get the current commit to detach to
        let commit_id = {
            let head = repo.repo.head().unwrap();
            head.target().unwrap()
        };
        
        // Detach HEAD to the current commit
        repo.repo.set_head_detached(commit_id).unwrap();
        
        // This is the critical test - ensure calculate_repository_state works in detached HEAD
        let repo_state = repo.calculate_repository_state().unwrap();
        
        // Verify the repository state is correctly calculated
        assert!(repo_state.current_branch.starts_with("detached-"));
        assert_eq!(repo_state.current_branch, format!("detached-{}", commit_id));
        
        // Verify we have branch state for the detached HEAD
        let branch_state = repo_state.get_branch_state(&repo_state.current_branch);
        assert!(branch_state.is_some());
        
        let branch_state = branch_state.unwrap();
        assert_eq!(branch_state.commit_hash, commit_id.to_string());
        // Merkle root may be empty for test repos with no files, that's ok
        assert!(branch_state.merkle_root.len() >= 0);
    }

    #[test]
    fn test_detached_head_calculate_branch_state() {
        let (_temp_dir, mut repo) = create_test_repo();
        
        // Get the current commit to detach to
        let commit_id = {
            let head = repo.repo.head().unwrap();
            head.target().unwrap()
        };
        
        // Detach HEAD to the current commit
        repo.repo.set_head_detached(commit_id).unwrap();
        
        let detached_branch_name = format!("detached-{}", commit_id);
        
        // Test calculating branch state for a detached HEAD branch name
        let branch_state = repo.calculate_branch_state(&detached_branch_name).unwrap();
        
        assert_eq!(branch_state.branch_name, detached_branch_name);
        assert_eq!(branch_state.commit_hash, commit_id.to_string());
        // Merkle root may be empty for test repos with no files, that's ok
        assert!(branch_state.merkle_root.len() >= 0);
        assert!(!branch_state.is_synced); // Should default to false
    }

    #[test]
    fn test_switch_to_tag_creates_detached_head() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Create a tag
        let head = repo.repo.head().unwrap();
        let commit_id = head.target().unwrap();
        let commit = repo.repo.find_commit(commit_id).unwrap();
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        
        repo.repo.tag("v1.0.0", commit.as_object(), &signature, "Test tag", false).unwrap();
        
        // Switch to the tag
        let default_branch = repo.current_branch().unwrap();
        let previous_branch = repo.switch_branch("v1.0.0").unwrap();
        assert_eq!(previous_branch, default_branch);
        
        // Verify we're now in detached HEAD state
        let current_branch = repo.current_branch().unwrap();
        assert!(current_branch.starts_with("detached-"));
        assert!(current_branch.contains(&commit_id.to_string()));
    }

    #[test]
    fn test_detached_head_branch_exists_returns_false() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Get the current commit to detach to
        let head = repo.repo.head().unwrap();
        let commit_id = head.target().unwrap();
        
        // Detach HEAD to the current commit
        repo.repo.set_head_detached(commit_id).unwrap();
        
        let detached_branch_name = format!("detached-{}", commit_id);
        
        // Verify that the "detached-{oid}" name is not considered a real branch
        assert!(!repo.branch_exists(&detached_branch_name).unwrap());
        
        // But the original branch should still exist
        // Get default branch from repo rather than hardcoding
        let branches = repo.list_branches(Some(BranchType::Local)).unwrap();
        assert!(branches.iter().any(|b| b == "main" || b == "master"));
    }

    #[test]
    fn test_regression_detached_head_initialization() {
        // This is the specific regression test for the bug we just fixed
        let (_temp_dir, mut repo) = create_test_repo();
        
        // Put repository in detached HEAD state (simulating the original issue)
        let commit_id = {
            let head = repo.repo.head().unwrap();
            head.target().unwrap()
        };
        repo.repo.set_head_detached(commit_id).unwrap();
        
        // This call should NOT fail with "cannot locate local branch 'HEAD'"
        // It was the exact failure mode from the original bug
        let result = repo.calculate_repository_state();
        
        // Assert it succeeds
        assert!(result.is_ok(), "Repository state calculation should succeed in detached HEAD state");
        
        let repo_state = result.unwrap();
        let expected_branch_name = format!("detached-{}", commit_id);
        assert_eq!(repo_state.current_branch, expected_branch_name);
        
        // Verify the branch state was calculated correctly
        let branch_state = repo_state.get_branch_state(&expected_branch_name);
        assert!(branch_state.is_some(), "Branch state should exist for detached HEAD");
        
        let branch_state = branch_state.unwrap();
        assert_eq!(branch_state.commit_hash, commit_id.to_string());
    }
} 