use std::path::Path;
use crate::error::{GitError, GitResult};
use crate::core::{GitRepository, RepositoryState, StateManager};
use crate::sync::{MerkleManager, HashDiff};
use crate::{SwitchResult, SyncResult};
use tracing;
use std::sync::Arc;
use async_trait::async_trait;

/// Trait for vector database sync operations
/// This allows git-manager to integrate with any vector database sync implementation
/// without creating circular dependencies
#[async_trait]
pub trait VectorSyncTrait: Send + Sync {
    /// Perform a sync operation with the given file changes
    async fn sync_files(
        &self,
        repo_path: &Path,
        files_to_add: &[std::path::PathBuf],
        files_to_update: &[std::path::PathBuf], 
        files_to_delete: &[std::path::PathBuf],
        is_full_sync: bool,
    ) -> Result<VectorSyncResult, Box<dyn std::error::Error + Send + Sync>>;
}

/// Result of a vector sync operation
#[derive(Debug, Clone)]
pub struct VectorSyncResult {
    pub success: bool,
    pub files_indexed: usize,
    pub files_deleted: usize,
    pub message: String,
}

/// Dummy sync implementation that does nothing
/// Used as the default type parameter for GitManager without sync capabilities
#[derive(Debug, Clone)]
pub struct NoSync;

#[async_trait]
impl VectorSyncTrait for NoSync {
    async fn sync_files(
        &self,
        _repo_path: &Path,
        _files_to_add: &[std::path::PathBuf],
        _files_to_update: &[std::path::PathBuf], 
        _files_to_delete: &[std::path::PathBuf],
        _is_full_sync: bool,
    ) -> Result<VectorSyncResult, Box<dyn std::error::Error + Send + Sync>> {
        Ok(VectorSyncResult {
            success: true,
            files_indexed: 0,
            files_deleted: 0,
            message: "No sync implementation available".to_string(),
        })
    }
}

/// Options for branch switching operations
#[derive(Debug, Clone)]
pub struct SwitchOptions {
    /// Force switch even with uncommitted changes
    pub force: bool,
    /// Automatically resync after switch (default: true)
    pub auto_resync: bool,
    /// Options for the resync operation
    pub sync_options: SyncOptions,
}

impl Default for SwitchOptions {
    fn default() -> Self {
        Self {
            force: false,
            auto_resync: true,
            sync_options: SyncOptions::default(),
        }
    }
}

/// Options for sync operations
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// Whether to perform incremental sync (default: true)
    pub incremental: bool,
    /// Maximum number of files to process in a batch
    pub batch_size: usize,
    /// Whether to continue on errors
    pub continue_on_error: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            incremental: true,
            batch_size: 100,
            continue_on_error: false,
        }
    }
}

/// Type of sync operation required
#[derive(Debug, Clone, PartialEq)]
pub enum SyncType {
    /// No sync required
    None,
    /// Incremental sync with specific files
    Incremental,
    /// Full resync required
    Full,
}

/// Sync requirement calculation result
#[derive(Debug, Clone)]
pub struct SyncRequirement {
    /// Type of sync required
    pub sync_type: SyncType,
    /// Files that need to be added to the vector database
    pub files_to_add: Vec<std::path::PathBuf>,
    /// Files that need to be updated in the vector database
    pub files_to_update: Vec<std::path::PathBuf>,
    /// Files that need to be deleted from the vector database
    pub files_to_delete: Vec<std::path::PathBuf>,
}

impl SyncRequirement {
    /// Create a sync requirement with no sync needed
    pub fn none() -> Self {
        Self {
            sync_type: SyncType::None,
            files_to_add: Vec::new(),
            files_to_update: Vec::new(),
            files_to_delete: Vec::new(),
        }
    }

    /// Create a sync requirement for full sync
    pub fn full() -> Self {
        Self {
            sync_type: SyncType::Full,
            files_to_add: Vec::new(),
            files_to_update: Vec::new(),
            files_to_delete: Vec::new(),
        }
    }

    /// Create a sync requirement from a hash diff
    pub fn from_diff(diff: HashDiff) -> Self {
        Self {
            sync_type: SyncType::Incremental,
            files_to_add: diff.added,
            files_to_update: diff.modified,
            files_to_delete: diff.deleted,
        }
    }

    /// Get total number of files to process
    pub fn total_files(&self) -> usize {
        self.files_to_add.len() + self.files_to_update.len() + self.files_to_delete.len()
    }

    /// Check if sync is required
    pub fn requires_sync(&self) -> bool {
        self.sync_type != SyncType::None
    }
}

/// Branch switcher with enhanced sync capabilities
#[derive(Debug)]
pub struct BranchSwitcher<S = NoSync> 
where 
    S: VectorSyncTrait + 'static,
{
    /// Merkle manager for change detection
    merkle_manager: MerkleManager,
    /// Optional vector sync implementation
    vector_sync: Option<Arc<S>>,
}

impl BranchSwitcher<NoSync> {
    /// Create a new BranchSwitcher without sync capabilities
    pub fn new() -> Self {
        Self {
            merkle_manager: MerkleManager::new(),
            vector_sync: None,
        }
    }
}

impl<S> BranchSwitcher<S> 
where 
    S: VectorSyncTrait + 'static,
{
    /// Create a new BranchSwitcher with real sync capabilities
    pub fn with_sync(vector_sync: Arc<S>) -> Self {
        Self {
            merkle_manager: MerkleManager::new(),
            vector_sync: Some(vector_sync),
        }
    }

    /// Check if this switcher has sync capabilities
    pub fn has_sync_capabilities(&self) -> bool {
        self.vector_sync.is_some()
    }

    /// Switch to a different branch with automatic resync detection
    pub async fn switch_branch_with_resync(
        &mut self,
        repo_path: &Path,
        target_branch: &str,
        state_manager: &mut StateManager,
        options: SwitchOptions,
    ) -> GitResult<SwitchResult> {
        tracing::info!("Switching to branch '{}' at path: {}", target_branch, repo_path.display());

        // Open repository
        let mut repo = GitRepository::open(repo_path)?;

        // Get current state
        let repo_path_buf = repo_path.to_path_buf();
        let current_state = state_manager.get_repository_state(&repo_path_buf);
        let current_branch = repo.current_branch()?;

        // Check for uncommitted changes unless force is enabled
        if !options.force && repo.has_uncommitted_changes()? {
            return Err(GitError::UncommittedChanges {
                branch: target_branch.to_string(),
            });
        }

        // Calculate sync requirements if auto_resync is enabled
        let sync_requirement = if options.auto_resync {
            self.calculate_sync_requirements(
                &mut repo,
                &current_branch,
                target_branch,
                current_state,
            ).await?
        } else {
            SyncRequirement::none()
        };

        tracing::info!(
            "Sync requirement: {:?}, {} files to process",
            sync_requirement.sync_type,
            sync_requirement.total_files()
        );

        // Perform the actual branch switch
        let previous_branch = repo.switch_branch_with_options(target_branch, options.force)?;

        // Update repository state
        let new_repo_state = repo.calculate_repository_state()?;
        state_manager.set_repository_state(repo_path_buf, new_repo_state);

        // Perform sync if required
        let sync_result = if sync_requirement.requires_sync() {
            Some(self.perform_sync(&sync_requirement, &options.sync_options, repo_path).await?)
        } else {
            None
        };

        // Calculate files changed - for full sync, estimate based on current directory
        let files_changed = if sync_requirement.sync_type == SyncType::Full {
            // For full sync, count current files as an estimate
            let (_, current_file_hashes) = self.merkle_manager
                .calculate_merkle_state(repo_path, None)?;
            current_file_hashes.len()
        } else {
            sync_requirement.total_files()
        };

        let result = SwitchResult {
            success: true,
            previous_branch,
            new_branch: target_branch.to_string(),
            sync_result,
            files_changed,
        };

        tracing::info!(
            "Successfully switched to branch '{}', {} files changed",
            target_branch,
            result.files_changed
        );

        Ok(result)
    }

    /// Calculate what sync operations are required after switching branches
    pub async fn calculate_sync_requirements(
        &mut self,
        repo: &mut GitRepository,
        current_branch: &str,
        target_branch: &str,
        current_state: Option<&RepositoryState>,
    ) -> GitResult<SyncRequirement> {
        tracing::info!("Calculating sync requirements: {} -> {}", current_branch, target_branch);
        
        // If it's the same branch, check if we have cached state
        if current_branch == target_branch {
            // If there's no cached state, this indicates the repository was cleared
            // and we need a full sync even though git trees are identical
            if current_state.is_none() {
                tracing::info!("Same branch but no cached state - repository was likely cleared, requiring full sync");
                return Ok(SyncRequirement::full());
            }
            
            // If we have cached state, check if the current branch has been synced
            if let Some(state) = current_state {
                if let Some(branch_state) = state.get_branch_state(current_branch) {
                    if !branch_state.is_synced {
                        tracing::info!("Same branch but not synced - requiring full sync");
                        return Ok(SyncRequirement::full());
                    }
                } else {
                    // No branch state exists for current branch - need full sync
                    tracing::info!("Same branch but no branch state - requiring full sync");
                    return Ok(SyncRequirement::full());
                }
            }
            
            tracing::info!("Same branch and already synced, no sync required");
            return Ok(SyncRequirement::none());
        }
        
        // Always use git to compare the actual tree objects between branches
        // This is the most reliable method and doesn't depend on cached state
        match self.calculate_git_tree_diff(repo, current_branch, target_branch) {
            Ok(diff) => {
                tracing::info!(
                    "Git tree diff: {} added, {} modified, {} deleted",
                    diff.added.len(),
                    diff.modified.len(),
                    diff.deleted.len()
                );
                Ok(SyncRequirement::from_diff(diff))
            }
            Err(e) => {
                tracing::warn!("Failed to calculate git tree diff: {}, falling back to full sync", e);
                // Fallback to full sync if we can't calculate the diff
                Ok(SyncRequirement::full())
            }
        }
    }

    /// Calculate file differences between two git branches using git tree objects
    fn calculate_git_tree_diff(
        &self,
        repo: &GitRepository,
        current_branch: &str,
        target_branch: &str,
    ) -> GitResult<HashDiff> {
        use git2::{DiffOptions, DiffDelta, Delta};
        
        let git_repo = repo.repo();
        
        // Get the tree for the current branch/commit
        let current_tree = if current_branch.starts_with("detached-") {
            // Handle detached HEAD state
            let commit_oid = git_repo.head()?.target()
                .ok_or_else(|| GitError::invalid_state("HEAD has no target"))?;
            git_repo.find_commit(commit_oid)?.tree()?
        } else {
            // Regular branch
            let current_ref = git_repo.find_branch(current_branch, git2::BranchType::Local)?;
            let current_commit = current_ref.get().peel_to_commit()?;
            current_commit.tree()?
        };
        
        // Get the tree for the target branch/commit
        let target_tree = if repo.branch_exists(target_branch)? {
            // Local branch exists
            let target_ref = git_repo.find_branch(target_branch, git2::BranchType::Local)?;
            let target_commit = target_ref.get().peel_to_commit()?;
            target_commit.tree()?
        } else {
            // Try to resolve as any reference (tag, commit, remote branch)
            let target_obj = git_repo.revparse_single(target_branch)?;
            let target_commit = target_obj.peel_to_commit()?;
            target_commit.tree()?
        };
        
        // Set up diff options
        let mut diff_opts = DiffOptions::new();
        diff_opts.ignore_whitespace(false);
        diff_opts.include_untracked(false);
        
        // Calculate the diff between trees
        let diff = git_repo.diff_tree_to_tree(
            Some(&current_tree),
            Some(&target_tree),
            Some(&mut diff_opts),
        )?;
        
        let mut hash_diff = HashDiff::new();
        
        // Process each delta (file change)
        diff.foreach(
            &mut |delta: DiffDelta, _progress: f32| -> bool {
                let old_file = delta.old_file();
                let new_file = delta.new_file();
                
                match delta.status() {
                    Delta::Added => {
                        if let Some(path) = new_file.path() {
                            hash_diff.added.push(path.to_path_buf());
                        }
                    }
                    Delta::Deleted => {
                        if let Some(path) = old_file.path() {
                            hash_diff.deleted.push(path.to_path_buf());
                        }
                    }
                    Delta::Modified => {
                        if let Some(path) = new_file.path() {
                            hash_diff.modified.push(path.to_path_buf());
                        }
                    }
                    Delta::Renamed => {
                        // Treat renames as delete + add for simplicity
                        if let Some(old_path) = old_file.path() {
                            hash_diff.deleted.push(old_path.to_path_buf());
                        }
                        if let Some(new_path) = new_file.path() {
                            hash_diff.added.push(new_path.to_path_buf());
                        }
                    }
                    Delta::Copied => {
                        // Treat copies as additions
                        if let Some(path) = new_file.path() {
                            hash_diff.added.push(path.to_path_buf());
                        }
                    }
                    Delta::Ignored | Delta::Untracked | Delta::Typechange | Delta::Unreadable | Delta::Conflicted | Delta::Unmodified => {
                        // Skip these types for now
                    }
                }
                true // Continue iteration
            },
            None, // No binary callback
            None, // No hunk callback  
            None, // No line callback
        )?;
        
        tracing::debug!(
            "Git tree diff completed: {} added, {} modified, {} deleted",
            hash_diff.added.len(),
            hash_diff.modified.len(),
            hash_diff.deleted.len()
        );
        
        Ok(hash_diff)
    }

    /// Perform the actual sync operation
    async fn perform_sync(
        &self,
        requirement: &SyncRequirement,
        options: &SyncOptions,
        repo_path: &Path,
    ) -> GitResult<SyncResult> {
        tracing::info!("Starting sync operation: {:?}", requirement.sync_type);

        match requirement.sync_type {
            SyncType::None => Ok(SyncResult {
                success: true,
                files_added: 0,
                files_updated: 0,
                files_removed: 0,
                error_message: None,
            }),
            SyncType::Full | SyncType::Incremental => {
                // Check if we have sync capabilities
                if let Some(vector_sync) = &self.vector_sync {
                    tracing::info!(
                        "Performing real sync: {} to add, {} to update, {} to delete",
                        requirement.files_to_add.len(),
                        requirement.files_to_update.len(),
                        requirement.files_to_delete.len()
                    );

                    // Call the real sync function
                    match vector_sync.sync_files(
                        repo_path,
                        &requirement.files_to_add,
                        &requirement.files_to_update,
                        &requirement.files_to_delete,
                        requirement.sync_type == SyncType::Full,
                    ).await {
                        Ok(vector_result) => {
                            tracing::info!("Sync completed successfully: {}", vector_result.message);
                            Ok(SyncResult {
                                success: vector_result.success,
                                files_added: requirement.files_to_add.len(),
                                files_updated: requirement.files_to_update.len(),
                                files_removed: vector_result.files_deleted,
                                error_message: if vector_result.success { None } else { Some(vector_result.message) },
                            })
                        }
                        Err(e) => {
                            tracing::error!("Sync failed: {}", e);
                            Ok(SyncResult {
                                success: false,
                                files_added: 0,
                                files_updated: 0,
                                files_removed: 0,
                                error_message: Some(format!("Sync failed: {}", e)),
                            })
                        }
                    }
                } else {
                    // Fallback to placeholder implementation
                    tracing::warn!("No sync capabilities available, returning placeholder result");
                    Ok(SyncResult {
                        success: true,
                        files_added: requirement.files_to_add.len(),
                        files_updated: requirement.files_to_update.len(),
                        files_removed: requirement.files_to_delete.len(),
                        error_message: Some("Sync capabilities not available - placeholder result".to_string()),
                    })
                }
            }
        }
    }
}

impl Default for BranchSwitcher<NoSync> {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function for switching branches with default options
pub async fn switch_branch(
    repo_path: &Path,
    target_branch: &str,
    state_manager: &mut StateManager,
) -> GitResult<SwitchResult> {
    let mut switcher = BranchSwitcher::new();
    switcher.switch_branch_with_resync(
        repo_path,
        target_branch,
        state_manager,
        SwitchOptions::default(),
    ).await
}

/// Convenience function for switching branches without auto-resync
pub async fn switch_branch_no_sync(
    repo_path: &Path,
    target_branch: &str,
    state_manager: &mut StateManager,
) -> GitResult<SwitchResult> {
    let mut switcher = BranchSwitcher::new();
    let options = SwitchOptions {
        auto_resync: false,
        ..Default::default()
    };
    switcher.switch_branch_with_resync(
        repo_path,
        target_branch,
        state_manager,
        options,
    ).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[tokio::test]
    async fn test_sync_requirement_creation() {
        let req = SyncRequirement::none();
        assert_eq!(req.sync_type, SyncType::None);
        assert!(!req.requires_sync());

        let req = SyncRequirement::full();
        assert_eq!(req.sync_type, SyncType::Full);
        assert!(req.requires_sync());
    }

    #[tokio::test]
    async fn test_branch_switcher_creation() {
        let switcher = BranchSwitcher::new();
        // Just test that it can be created
        assert!(true);
    }

    #[test]
    fn test_switch_options_default() {
        let options = SwitchOptions::default();
        assert!(!options.force);
        assert!(options.auto_resync);
        assert!(options.sync_options.incremental);
    }

    #[tokio::test]
    async fn test_git_tree_diff_with_actual_changes() {
        use git2::{Repository, Signature, ObjectType};
        
        // Create a test repository with actual git history
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        
        // Set up git config
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        
        let signature = Signature::now("Test User", "test@example.com").unwrap();
        
        // Create initial commit on main branch
        fs::write(repo_path.join("file1.txt"), "content 1").unwrap();
        fs::write(repo_path.join("file2.txt"), "content 2").unwrap();
        
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file1.txt")).unwrap();
        index.add_path(std::path::Path::new("file2.txt")).unwrap();
        index.write().unwrap();
        
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        
        let initial_commit = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();
        
        // Create a new branch and make changes
        let initial_commit_obj = repo.find_commit(initial_commit).unwrap();
        repo.branch("feature", &initial_commit_obj, false).unwrap();
        repo.set_head("refs/heads/feature").unwrap();
        
        // Make changes on feature branch
        fs::write(repo_path.join("file1.txt"), "modified content 1").unwrap(); // Modified
        fs::write(repo_path.join("file3.txt"), "new content 3").unwrap(); // Added
        fs::remove_file(repo_path.join("file2.txt")).unwrap(); // Deleted
        
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file1.txt")).unwrap();
        index.add_path(std::path::Path::new("file3.txt")).unwrap();
        index.remove_path(std::path::Path::new("file2.txt")).unwrap();
        index.write().unwrap();
        
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Feature changes",
            &tree,
            &[&initial_commit_obj],
        ).unwrap();
        
        // Now test our git tree diff functionality
        let git_repo = crate::core::GitRepository::open(repo_path).unwrap();
        let mut switcher = BranchSwitcher::new();
        
        // Test diff from main to feature
        let diff = switcher.calculate_git_tree_diff(&git_repo, "main", "feature").unwrap();
        
        println!("Git tree diff results:");
        println!("  Added files: {:?}", diff.added);
        println!("  Modified files: {:?}", diff.modified);
        println!("  Deleted files: {:?}", diff.deleted);
        
        // Verify the expected changes
        assert_eq!(diff.added.len(), 1, "Should have 1 added file");
        assert_eq!(diff.modified.len(), 1, "Should have 1 modified file");
        assert_eq!(diff.deleted.len(), 1, "Should have 1 deleted file");
        
        assert!(diff.added.contains(&std::path::PathBuf::from("file3.txt")));
        assert!(diff.modified.contains(&std::path::PathBuf::from("file1.txt")));
        assert!(diff.deleted.contains(&std::path::PathBuf::from("file2.txt")));
        
        // Test reverse diff (feature to main)
        let reverse_diff = switcher.calculate_git_tree_diff(&git_repo, "feature", "main").unwrap();
        
        // Should be exactly reversed
        assert_eq!(reverse_diff.added.len(), 1, "Reverse: should have 1 added file");
        assert_eq!(reverse_diff.modified.len(), 1, "Reverse: should have 1 modified file");
        assert_eq!(reverse_diff.deleted.len(), 1, "Reverse: should have 1 deleted file");
        
        assert!(reverse_diff.added.contains(&std::path::PathBuf::from("file2.txt")));
        assert!(reverse_diff.modified.contains(&std::path::PathBuf::from("file1.txt")));
        assert!(reverse_diff.deleted.contains(&std::path::PathBuf::from("file3.txt")));
        
        // Test sync requirement calculation
        let mut state_manager = StateManager::new();
        let mut git_repo = crate::core::GitRepository::open(repo_path).unwrap();
        let sync_req = switcher.calculate_sync_requirements(
            &mut git_repo,
            "main",
            "feature", 
            None
        ).await.unwrap();
        
        assert_eq!(sync_req.sync_type, SyncType::Incremental);
        assert_eq!(sync_req.files_to_add.len(), 1);
        assert_eq!(sync_req.files_to_update.len(), 1);
        assert_eq!(sync_req.files_to_delete.len(), 1);
        assert_eq!(sync_req.total_files(), 3);
        
        println!("✅ Git tree diff test passed!");
    }

    #[tokio::test]
    async fn test_git_tree_diff_no_changes() {
        use git2::{Repository, Signature};
        
        // Create a test repository
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        
        // Set up git config
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        
        let signature = Signature::now("Test User", "test@example.com").unwrap();
        
        // Create initial commit
        fs::write(repo_path.join("file1.txt"), "content 1").unwrap();
        
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file1.txt")).unwrap();
        index.write().unwrap();
        
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();
        
        // Test diff between same branch with no cached state
        let mut git_repo = crate::core::GitRepository::open(repo_path).unwrap();
        let mut switcher = BranchSwitcher::new();
        
        // With no cached state (None), should require full sync even for same branch
        let sync_req = switcher.calculate_sync_requirements(
            &mut git_repo,
            "main",
            "main", 
            None
        ).await.unwrap();
        
        assert_eq!(sync_req.sync_type, SyncType::Full);
        assert_eq!(sync_req.total_files(), 0);
        
        // Now test with cached state that has the branch marked as synced
        let mut state_manager = StateManager::new();
        let repo_state = git_repo.calculate_repository_state().unwrap();
        state_manager.set_repository_state(repo_path.to_path_buf(), repo_state);
        
        // Mark the branch as synced
        if let Some(repo_state) = state_manager.get_repository_state_mut(&repo_path.to_path_buf()) {
            if let Some(branch_state) = repo_state.get_branch_state_mut("main") {
                branch_state.mark_synced();
            }
        }
        
        // Now with cached state and synced branch, should not require sync
        let sync_req_with_state = switcher.calculate_sync_requirements(
            &mut git_repo,
            "main",
            "main", 
            state_manager.get_repository_state(&repo_path.to_path_buf())
        ).await.unwrap();
        
        assert_eq!(sync_req_with_state.sync_type, SyncType::None);
        assert_eq!(sync_req_with_state.total_files(), 0);
        
        println!("✅ No changes test passed!");
    }
} 