//! # Git Manager
//!
//! A centralized git functionality crate for sagitta with enhanced branch management,
//! automatic resync capabilities, and merkle tree optimization for efficient change detection.
//!
//! ## Features
//!
//! - **Centralized Git Operations**: All git functionality in one place
//! - **Branch Management**: Advanced branch switching with automatic resync
//! - **Merkle Tree Optimization**: Efficient change detection between branches
//! - **State Management**: Track repository and branch states
//! - **Modular Architecture**: Clean separation of concerns
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use git_manager::{GitManager, SwitchOptions};
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut manager = GitManager::new();
//! let repo_path = PathBuf::from("/path/to/repo");
//!
//! // Initialize repository state
//! manager.initialize_repository(&repo_path).await?;
//!
//! // Switch branches with automatic resync detection
//! let result = manager.switch_branch(&repo_path, "feature-branch").await?;
//! println!("Switched to branch: {}", result.new_branch);
//!
//! // Check sync requirements before switching
//! let sync_req = manager.calculate_sync_requirements(&repo_path, "main").await?;
//! println!("Sync type needed: {:?}", sync_req.sync_type);
//! # Ok(())
//! # }
//! ```
//!
//! ## Migration from sagitta-search
//!
//! ### CLI Migration
//! ```rust,no_run
//! // Old way (sagitta-cli)
//! // use sagitta_search::repo_helpers::switch_repository_branch;
//! // switch_repository_branch(config, repo_name, branch_name)?;
//!
//! // New way (git-manager)
//! use git_manager::GitManager;
//! use std::path::PathBuf;
//! 
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut manager = GitManager::new();
//! let repo_path = PathBuf::from("/path/to/repo");
//! let result = manager.switch_branch(&repo_path, "feature-branch").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### MCP Migration
//! ```rust,no_run
//! // Old way (sagitta-mcp)
//! // Manual git operations scattered across handlers
//!
//! // New way (git-manager)
//! use git_manager::{GitManager, SwitchOptions};
//! use std::path::PathBuf;
//! 
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut manager = GitManager::new();
//! let repo_path = PathBuf::from("/path/to/repo");
//! let options = SwitchOptions::default();
//! let result = manager.switch_branch_with_options(&repo_path, "feature-branch", options).await?;
//! # Ok(())
//! # }
//! ```

pub mod core;
pub mod sync;
pub mod operations;
pub mod indexing;
pub mod error;

// Compatibility layer for migration from sagitta-search
pub mod compat;

// Re-export the most commonly used types and functions for easy access
pub use error::{GitError, GitResult};

// Core types for repository and state management
pub use core::{
    BranchState, RepositoryState, StateManager, GitRepository, RepositoryInfo
};

// Sync and merkle tree functionality
pub use sync::{
    MerkleManager, HashDiff, calculate_file_hash, calculate_merkle_root
};

// Operations for branch management and switching
pub use operations::{
    BranchSwitcher, SwitchOptions, SyncOptions, SyncType, SyncRequirement,
    switch_branch, switch_branch_no_sync,
    // Re-export create/clone operations
    RepositoryCloner, CloneOptions, CloneResult, init_repository,
    // Re-export change management operations  
    ChangeManager, CommitOptions, CommitResult, GitPushOptions, PushResult,
    PullOptions, PullResult, GitSignature,
};
pub use operations::switch::{VectorSyncTrait, VectorSyncResult, NoSync};

// Branch management operations
pub use core::branch::{BranchInfo, BranchManager, CreateBranchOptions};

// Indexing utilities for file processing
pub use indexing::{
    file_processor, batch_processor, language_detector, content_extractor
};

use std::sync::Arc;

/// Main git manager struct that coordinates all git operations
///
/// This is the primary interface for all git operations in sagitta.
/// It provides a high-level API that coordinates between state management,
/// merkle tree operations, and branch switching.
///
/// # Examples
///
/// ## Basic Usage
/// ```rust,no_run
/// use git_manager::GitManager;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut manager = GitManager::new();
/// let repo_path = PathBuf::from("/path/to/repo");
///
/// // Initialize repository
/// let info = manager.initialize_repository(&repo_path).await?;
/// println!("Initialized {} on branch {}", info.path.display(), info.current_branch);
///
/// // List available branches
/// let branches = manager.list_branches(&repo_path)?;
/// println!("Available branches: {:?}", branches);
///
/// // Switch to a different branch
/// let result = manager.switch_branch(&repo_path, "develop").await?;
/// println!("Switched from {} to {}", result.previous_branch, result.new_branch);
/// # Ok(())
/// # }
/// ```
///
/// ## Advanced Branch Operations
/// ```rust,no_run
/// use git_manager::{GitManager, SwitchOptions, SyncType};
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut manager = GitManager::new();
/// let repo_path = PathBuf::from("/path/to/repo");
///
/// // Check what sync would be required
/// let sync_req = manager.calculate_sync_requirements(&repo_path, "main").await?;
/// match sync_req.sync_type {
///     SyncType::None => println!("No sync needed"),
///     SyncType::Incremental => println!("Incremental sync needed for {} files", sync_req.files_to_update.len()),
///     SyncType::Full => println!("Full resync required"),
/// }
///
/// // Switch with custom options
/// let options = SwitchOptions {
///     force: false,
///     auto_resync: true,
///     ..Default::default()
/// };
/// let result = manager.switch_branch_with_options(&repo_path, "main", options).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct GitManager<S = NoSync> 
where 
    S: VectorSyncTrait + 'static,
{
    /// State manager for tracking repository states
    state_manager: StateManager,
    /// Merkle manager for change detection
    merkle_manager: MerkleManager,
    /// Branch switcher for advanced branch operations
    branch_switcher: BranchSwitcher<S>,
}

impl GitManager<NoSync> {
    /// Create a new GitManager instance without sync capabilities
    ///
    /// # Examples
    /// ```rust
    /// use git_manager::GitManager;
    ///
    /// let manager = GitManager::new();
    /// ```
    pub fn new() -> Self {
        Self {
            state_manager: StateManager::new(),
            merkle_manager: MerkleManager::new(),
            branch_switcher: BranchSwitcher::new(),
        }
    }
}

impl<S> GitManager<S> 
where 
    S: VectorSyncTrait + 'static,
{
    /// Create a new GitManager instance with real sync capabilities
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::{GitManager, VectorSyncTrait, VectorSyncResult};
    /// use std::sync::Arc;
    /// use std::path::Path;
    ///
    /// // Example mock implementation for demonstration
    /// struct MockVectorSync;
    /// 
    /// #[async_trait::async_trait]
    /// impl VectorSyncTrait for MockVectorSync {
    ///     async fn sync_files(
    ///         &self,
    ///         _repo_path: &Path,
    ///         _files_to_add: &[std::path::PathBuf],
    ///         _files_to_update: &[std::path::PathBuf], 
    ///         _files_to_delete: &[std::path::PathBuf],
    ///         _is_full_sync: bool,
    ///     ) -> Result<VectorSyncResult, Box<dyn std::error::Error + Send + Sync>> {
    ///         Ok(VectorSyncResult {
    ///             success: true,
    ///             files_indexed: 0,
    ///             files_deleted: 0,
    ///             message: "Mock sync completed".to_string(),
    ///         })
    ///     }
    /// }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let vector_sync = Arc::new(MockVectorSync);
    /// let manager = GitManager::with_sync(vector_sync);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_sync(vector_sync: Arc<S>) -> Self {
        Self {
            state_manager: StateManager::new(),
            merkle_manager: MerkleManager::new(),
            branch_switcher: BranchSwitcher::with_sync(vector_sync),
        }
    }

    /// Check if this manager has sync capabilities
    pub fn has_sync_capabilities(&self) -> bool {
        self.branch_switcher.has_sync_capabilities()
    }

    /// Switch to a different branch with automatic resync
    ///
    /// This is the most commonly used method for branch switching.
    /// It automatically detects what type of sync is needed and performs it.
    ///
    /// # Arguments
    /// * `repo_path` - Path to the git repository
    /// * `target_branch` - Name of the branch to switch to
    ///
    /// # Returns
    /// Result of the switch operation including sync information
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::GitManager;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// let result = manager.switch_branch(&repo_path, "feature-branch").await?;
    /// println!("Switched from {} to {}", result.previous_branch, result.new_branch);
    /// if let Some(sync_result) = result.sync_result {
    ///     println!("Sync: {} files updated", sync_result.files_updated);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn switch_branch(
        &mut self,
        repo_path: &std::path::Path,
        target_branch: &str,
    ) -> GitResult<SwitchResult> {
        self.branch_switcher.switch_branch_with_resync(
            repo_path,
            target_branch,
            &mut self.state_manager,
            SwitchOptions::default(),
        ).await
    }

    /// Switch to a different branch with custom options
    ///
    /// Provides fine-grained control over the branch switching process.
    ///
    /// # Arguments
    /// * `repo_path` - Path to the git repository
    /// * `target_branch` - Name of the branch to switch to
    /// * `options` - Custom options for the switch operation
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::{GitManager, SwitchOptions};
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// let options = SwitchOptions {
    ///     force: true,  // Force switch even with uncommitted changes
    ///     auto_resync: false,  // Don't automatically resync
    ///     ..Default::default()
    /// };
    ///
    /// let result = manager.switch_branch_with_options(&repo_path, "main", options).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn switch_branch_with_options(
        &mut self,
        repo_path: &std::path::Path,
        target_branch: &str,
        options: SwitchOptions,
    ) -> GitResult<SwitchResult> {
        self.branch_switcher.switch_branch_with_resync(
            repo_path,
            target_branch,
            &mut self.state_manager,
            options,
        ).await
    }

    /// Get a reference to the state manager
    ///
    /// Useful for advanced state management operations.
    pub fn state_manager(&self) -> &StateManager {
        &self.state_manager
    }

    /// Get a mutable reference to the state manager
    ///
    /// Allows direct manipulation of repository states.
    pub fn state_manager_mut(&mut self) -> &mut StateManager {
        &mut self.state_manager
    }

    /// Get a reference to the merkle manager
    ///
    /// Useful for advanced merkle tree operations and change detection.
    pub fn merkle_manager(&self) -> &MerkleManager {
        &self.merkle_manager
    }

    /// Get a mutable reference to the merkle manager
    ///
    /// Allows direct manipulation of merkle tree state.
    pub fn merkle_manager_mut(&mut self) -> &mut MerkleManager {
        &mut self.merkle_manager
    }

    /// Initialize a repository for management
    ///
    /// This should be called once for each repository before performing
    /// any operations on it. It calculates the initial state and sets up
    /// the necessary data structures.
    ///
    /// # Arguments
    /// * `repo_path` - Path to the git repository
    ///
    /// # Returns
    /// Repository information including current branch and commit
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::GitManager;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// let info = manager.initialize_repository(&repo_path).await?;
    /// println!("Initialized {} on branch {}", info.path.display(), info.current_branch);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn initialize_repository(&mut self, repo_path: &std::path::Path) -> GitResult<RepositoryInfo> {
        tracing::info!("Initializing repository at: {}", repo_path.display());

        // Open the repository
        let mut repo = GitRepository::open(repo_path)?;
        
        // Calculate initial repository state
        let repo_state = repo.calculate_repository_state()?;
        
        // Store the state
        self.state_manager.set_repository_state(repo_path.to_path_buf(), repo_state);
        
        // Get repository information
        let info = repo.get_info()?;
        
        tracing::info!(
            "Initialized repository: {} (branch: {}, commit: {})",
            repo_path.display(),
            info.current_branch,
            &info.current_commit[..8]
        );
        
        Ok(info)
    }

    /// Get repository information
    ///
    /// Returns current information about the repository without modifying state.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::GitManager;
    /// use std::path::PathBuf;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// let info = manager.get_repository_info(&repo_path)?;
    /// println!("Current branch: {}", info.current_branch);
    /// println!("Current commit: {}", info.current_commit);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_repository_info(&self, repo_path: &std::path::Path) -> GitResult<RepositoryInfo> {
        let repo = GitRepository::open(repo_path)?;
        repo.get_info()
    }

    /// List all branches in a repository
    ///
    /// Returns a list of all local branches in the repository.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::GitManager;
    /// use std::path::PathBuf;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// let branches = manager.list_branches(&repo_path)?;
    /// for branch in branches {
    ///     println!("Branch: {}", branch);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_branches(&self, repo_path: &std::path::Path) -> GitResult<Vec<String>> {
        let repo = GitRepository::open(repo_path)?;
        repo.list_branches(Some(git2::BranchType::Local))
    }

    /// List all available references (local branches, remote branches, tags)
    ///
    /// Returns a comprehensive list of all references that can be checked out,
    /// including local branches, remote branches (without remote prefix), and tags.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::GitManager;
    /// use std::path::PathBuf;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// let refs = manager.list_all_references(&repo_path)?;
    /// for ref_name in refs {
    ///     println!("Reference: {}", ref_name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_all_references(&self, repo_path: &std::path::Path) -> GitResult<Vec<String>> {
        let repo = GitRepository::open(repo_path)?;
        repo.list_all_references()
    }

    /// List all tags in a repository
    ///
    /// Returns a list of all tags in the repository.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::GitManager;
    /// use std::path::PathBuf;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// let tags = manager.list_tags(&repo_path)?;
    /// for tag in tags {
    ///     println!("Tag: {}", tag);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_tags(&self, repo_path: &std::path::Path) -> GitResult<Vec<String>> {
        let repo = GitRepository::open(repo_path)?;
        let git2_repo = repo.repo();
        
        let mut tags = Vec::new();
        if let Ok(tag_names) = git2_repo.tag_names(None) {
            for tag_name in tag_names.iter() {
                if let Some(tag) = tag_name {
                    tags.push(tag.to_string());
                }
            }
        }
        
        Ok(tags)
    }

    /// Create a new branch
    ///
    /// Creates a new branch from the specified starting point.
    ///
    /// # Arguments
    /// * `repo_path` - Path to the git repository
    /// * `branch_name` - Name of the new branch
    /// * `start_point` - Optional starting point (commit, branch, or tag)
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::GitManager;
    /// use std::path::PathBuf;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// // Create branch from current HEAD
    /// manager.create_branch(&repo_path, "new-feature", None)?;
    ///
    /// // Create branch from specific commit
    /// manager.create_branch(&repo_path, "hotfix", Some("abc123"))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_branch(
        &self,
        repo_path: &std::path::Path,
        branch_name: &str,
        start_point: Option<&str>,
    ) -> GitResult<()> {
        let repo = GitRepository::open(repo_path)?;
        repo.create_branch(branch_name, start_point)
    }

    /// Delete a branch, with an option to force deletion
    pub fn delete_branch(&self, repo_path: &std::path::Path, branch_name: &str, force: bool) -> GitResult<()> {
        let repo = GitRepository::open(repo_path)?;
        let mut branch = repo.repo().find_branch(branch_name, git2::BranchType::Local)
            .map_err(|_| GitError::BranchNotFound { branch: branch_name.to_string() })?;

        if branch.is_head() {
            return Err(GitError::DeleteHeadBranch);
        }
        
        branch.delete()?;
        
        // The above only deletes the ref. To fully emulate `git branch -d/-D`, 
        // we might need to update config, but for most local cases this is sufficient.
        // The `force` parameter is not directly used by `branch.delete()`, which is more like `git branch -d`.
        // A true `git branch -D` would involve deleting even if not merged.
        // The current implementation is a "safe" delete. If a "force" delete is truly needed,
        // it would require more complex logic to bypass git2's safety checks, which isn't
        // straightforward. For now, we accept the `force` parameter but acknowledge this limitation.
        if force {
             log::warn!("'force' delete for branches is not fully implemented in git-manager; using standard delete.");
        }

        Ok(())
    }

    /// Check if a repository has uncommitted changes
    ///
    /// Returns true if there are any uncommitted changes in the working directory.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::GitManager;
    /// use std::path::PathBuf;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// if manager.has_uncommitted_changes(&repo_path)? {
    ///     println!("Repository has uncommitted changes");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn has_uncommitted_changes(&self, repo_path: &std::path::Path) -> GitResult<bool> {
        let repo = GitRepository::open(repo_path)?;
        repo.has_uncommitted_changes()
    }

    /// Get the status of files in a repository
    ///
    /// Returns a list of files and their git status.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::GitManager;
    /// use std::path::PathBuf;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// let status = manager.get_status(&repo_path)?;
    /// for (file, status) in status {
    ///     println!("{}: {:?}", file.display(), status);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_status(&self, repo_path: &std::path::Path) -> GitResult<Vec<(std::path::PathBuf, git2::Status)>> {
        let repo = GitRepository::open(repo_path)?;
        repo.get_status()
    }

    /// Calculate sync requirements between current state and target branch
    ///
    /// This method analyzes what type of sync would be required if switching
    /// to the target branch, without actually performing the switch.
    ///
    /// # Arguments
    /// * `repo_path` - Path to the git repository
    /// * `target_branch` - Branch to analyze sync requirements for
    ///
    /// # Returns
    /// Detailed information about what sync operations would be needed
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::{GitManager, SyncType};
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// let sync_req = manager.calculate_sync_requirements(&repo_path, "main").await?;
    /// match sync_req.sync_type {
    ///     SyncType::None => println!("No sync needed"),
    ///     SyncType::Incremental => {
    ///         println!("Incremental sync needed:");
    ///         println!("  Files to add: {}", sync_req.files_to_add.len());
    ///         println!("  Files to update: {}", sync_req.files_to_update.len());
    ///         println!("  Files to delete: {}", sync_req.files_to_delete.len());
    ///     },
    ///     SyncType::Full => println!("Full resync required"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn calculate_sync_requirements(
        &mut self,
        repo_path: &std::path::Path,
        target_branch: &str,
    ) -> GitResult<SyncRequirement> {
        let mut repo = GitRepository::open(repo_path)?;
        let current_branch = repo.current_branch()?;
        let current_state = self.state_manager
            .get_repository_state(&repo_path.to_path_buf());

        self.branch_switcher.calculate_sync_requirements(
            &mut repo,
            &current_branch,
            target_branch,
            current_state,
        ).await
    }

    /// Calculate sync requirements with force option
    ///
    /// This method forces a full sync regardless of git tree differences.
    /// Useful for scenarios like after a repository clear operation.
    ///
    /// # Arguments
    /// * `repo_path` - Path to the git repository
    /// * `target_branch` - Branch to analyze sync requirements for
    ///
    /// # Returns
    /// Always returns a full sync requirement
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::{GitManager, SyncType};
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut manager = GitManager::new();
    /// let repo_path = PathBuf::from("/path/to/repo");
    ///
    /// let sync_req = manager.calculate_sync_requirements_force(&repo_path, "main").await?;
    /// assert_eq!(sync_req.sync_type, SyncType::Full);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn calculate_sync_requirements_force(
        &mut self,
        _repo_path: &std::path::Path,
        _target_branch: &str,
    ) -> GitResult<SyncRequirement> {
        Ok(SyncRequirement::full())
    }
}

impl Default for GitManager<NoSync> {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a branch switch operation
///
/// Contains detailed information about the branch switch and any sync operations
/// that were performed as part of the switch.
#[derive(Debug, Clone)]
pub struct SwitchResult {
    /// Whether the switch was successful
    pub success: bool,
    /// The previous branch name
    pub previous_branch: String,
    /// The new branch name
    pub new_branch: String,
    /// Result of any sync operation that was performed
    pub sync_result: Option<SyncResult>,
    /// Number of files that changed between branches
    pub files_changed: usize,
}

/// Result of a sync operation
///
/// Contains detailed information about what was synchronized during
/// a branch switch or manual sync operation.
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// Whether the sync was successful
    pub success: bool,
    /// Number of files that were added to the vector database
    pub files_added: usize,
    /// Number of files that were updated in the vector database
    pub files_updated: usize,
    /// Number of files that were removed from the vector database
    pub files_removed: usize,
    /// Any error message if the sync failed
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_manager_creation() {
        let manager = GitManager::new();
        assert!(manager.state_manager().list_repositories().is_empty());
    }

    #[test]
    fn test_git_manager_default() {
        let manager = GitManager::default();
        assert!(manager.state_manager().list_repositories().is_empty());
    }

    #[tokio::test]
    async fn test_switch_result_creation() {
        let result = SwitchResult {
            success: true,
            previous_branch: "main".to_string(),
            new_branch: "feature".to_string(),
            sync_result: None,
            files_changed: 0,
        };
        assert!(result.success);
        assert_eq!(result.previous_branch, "main");
        assert_eq!(result.new_branch, "feature");
    }
} 