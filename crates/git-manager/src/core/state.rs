use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use crate::error::{GitError, GitResult};

/// Represents the state of a specific branch in a repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchState {
    /// Name of the branch
    pub branch_name: String,
    /// Current commit hash
    pub commit_hash: String,
    /// Merkle root hash of all files in this branch
    pub merkle_root: String,
    /// Hash of individual files for change detection
    pub file_hashes: HashMap<PathBuf, String>,
    /// When this state was last updated
    pub last_updated: SystemTime,
    /// Whether this branch has been fully synced to the vector database
    pub is_synced: bool,
}

impl BranchState {
    /// Create a new BranchState
    pub fn new(
        branch_name: String,
        commit_hash: String,
        merkle_root: String,
        file_hashes: HashMap<PathBuf, String>,
    ) -> Self {
        Self {
            branch_name,
            commit_hash,
            merkle_root,
            file_hashes,
            last_updated: SystemTime::now(),
            is_synced: false,
        }
    }

    /// Mark this branch as synced
    pub fn mark_synced(&mut self) {
        self.is_synced = true;
        self.last_updated = SystemTime::now();
    }

    /// Mark this branch as needing sync
    pub fn mark_needs_sync(&mut self) {
        self.is_synced = false;
        self.last_updated = SystemTime::now();
    }

    /// Update the state with new commit and file information
    pub fn update(
        &mut self,
        commit_hash: String,
        merkle_root: String,
        file_hashes: HashMap<PathBuf, String>,
    ) {
        self.commit_hash = commit_hash;
        self.merkle_root = merkle_root;
        self.file_hashes = file_hashes;
        self.last_updated = SystemTime::now();
        self.is_synced = false; // Needs resync after update
    }
}

/// Manages the state of all branches in a repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryState {
    /// Path to the repository
    pub repository_path: PathBuf,
    /// Current active branch
    pub current_branch: String,
    /// State of each branch
    pub branch_states: HashMap<String, BranchState>,
    /// When this repository state was last updated
    pub last_updated: SystemTime,
}

impl RepositoryState {
    /// Create a new RepositoryState
    pub fn new(repository_path: PathBuf, current_branch: String) -> Self {
        Self {
            repository_path,
            current_branch,
            branch_states: HashMap::new(),
            last_updated: SystemTime::now(),
        }
    }

    /// Get the state of a specific branch
    pub fn get_branch_state(&self, branch_name: &str) -> Option<&BranchState> {
        self.branch_states.get(branch_name)
    }

    /// Get mutable reference to the state of a specific branch
    pub fn get_branch_state_mut(&mut self, branch_name: &str) -> Option<&mut BranchState> {
        self.branch_states.get_mut(branch_name)
    }

    /// Set the state of a specific branch
    pub fn set_branch_state(&mut self, branch_name: String, state: BranchState) {
        self.branch_states.insert(branch_name, state);
        self.last_updated = SystemTime::now();
    }

    /// Remove a branch state
    pub fn remove_branch_state(&mut self, branch_name: &str) -> Option<BranchState> {
        let result = self.branch_states.remove(branch_name);
        if result.is_some() {
            self.last_updated = SystemTime::now();
        }
        result
    }

    /// Switch to a different branch
    pub fn switch_branch(&mut self, new_branch: String) -> GitResult<String> {
        if !self.branch_states.contains_key(&new_branch) {
            return Err(GitError::BranchNotFound { branch: new_branch });
        }
        
        let old_branch = self.current_branch.clone();
        self.current_branch = new_branch;
        self.last_updated = SystemTime::now();
        Ok(old_branch)
    }

    /// Get the current branch state
    pub fn current_branch_state(&self) -> Option<&BranchState> {
        self.get_branch_state(&self.current_branch)
    }

    /// Get mutable reference to the current branch state
    pub fn current_branch_state_mut(&mut self) -> Option<&mut BranchState> {
        let current_branch = self.current_branch.clone();
        self.get_branch_state_mut(&current_branch)
    }

    /// List all branch names
    pub fn list_branches(&self) -> Vec<String> {
        self.branch_states.keys().cloned().collect()
    }

    /// Check if a branch exists
    pub fn has_branch(&self, branch_name: &str) -> bool {
        self.branch_states.contains_key(branch_name)
    }

    /// Get branches that need syncing
    pub fn branches_needing_sync(&self) -> Vec<String> {
        self.branch_states
            .iter()
            .filter(|(_, state)| !state.is_synced)
            .map(|(name, _)| name.clone())
            .collect()
    }
}

/// Global state manager for all repositories
#[derive(Debug, Default)]
pub struct StateManager {
    /// State of each repository by path
    repositories: HashMap<PathBuf, RepositoryState>,
}

impl StateManager {
    /// Create a new StateManager
    pub fn new() -> Self {
        Self {
            repositories: HashMap::new(),
        }
    }

    /// Get the state of a repository
    pub fn get_repository_state(&self, repo_path: &PathBuf) -> Option<&RepositoryState> {
        self.repositories.get(repo_path)
    }

    /// Get mutable reference to the state of a repository
    pub fn get_repository_state_mut(&mut self, repo_path: &PathBuf) -> Option<&mut RepositoryState> {
        self.repositories.get_mut(repo_path)
    }

    /// Set the state of a repository
    pub fn set_repository_state(&mut self, repo_path: PathBuf, state: RepositoryState) {
        self.repositories.insert(repo_path, state);
    }

    /// Remove a repository state
    pub fn remove_repository_state(&mut self, repo_path: &PathBuf) -> Option<RepositoryState> {
        self.repositories.remove(repo_path)
    }

    /// List all repository paths
    pub fn list_repositories(&self) -> Vec<PathBuf> {
        self.repositories.keys().cloned().collect()
    }

    /// Clear the cached state for a repository (used when repository is cleared)
    pub fn clear_repository_state(&mut self, repo_path: &PathBuf) {
        self.repositories.remove(repo_path);
    }

    /// Get or create repository state
    pub fn get_or_create_repository_state(
        &mut self,
        repo_path: PathBuf,
        current_branch: String,
    ) -> &mut RepositoryState {
        self.repositories
            .entry(repo_path.clone())
            .or_insert_with(|| RepositoryState::new(repo_path, current_branch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_state_creation() {
        let file_hashes = HashMap::new();
        let state = BranchState::new(
            "main".to_string(),
            "abc123".to_string(),
            "merkle123".to_string(),
            file_hashes,
        );

        assert_eq!(state.branch_name, "main");
        assert_eq!(state.commit_hash, "abc123");
        assert_eq!(state.merkle_root, "merkle123");
        assert!(!state.is_synced);
    }

    #[test]
    fn test_repository_state_branch_management() {
        let mut repo_state = RepositoryState::new(
            PathBuf::from("/test/repo"),
            "main".to_string(),
        );

        let branch_state = BranchState::new(
            "feature".to_string(),
            "def456".to_string(),
            "merkle456".to_string(),
            HashMap::new(),
        );

        repo_state.set_branch_state("feature".to_string(), branch_state);
        assert!(repo_state.has_branch("feature"));
        assert!(!repo_state.has_branch("nonexistent"));

        let branches = repo_state.list_branches();
        assert!(branches.contains(&"feature".to_string()));
    }

    #[test]
    fn test_state_manager() {
        let mut manager = StateManager::new();
        let repo_path = PathBuf::from("/test/repo");
        
        let state = manager.get_or_create_repository_state(
            repo_path.clone(),
            "main".to_string(),
        );
        
        assert_eq!(state.current_branch, "main");
        assert_eq!(state.repository_path, repo_path);
    }
} 