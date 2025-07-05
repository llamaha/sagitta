//! Compatibility layer for migrating from sagitta-search git functionality
//!
//! This module provides compatibility functions and types to ease the migration
//! from the old scattered git functionality to the new centralized git-manager.

use crate::{GitManager, GitResult, SwitchResult};
use std::path::{Path, PathBuf};

/// Compatibility type for old sync results
///
/// This type provides compatibility with old sync result structures
/// while internally using the new git-manager types.
#[derive(Debug, Clone)]
pub struct LegacySyncResult {
    pub success: bool,
    pub message: String,
    pub files_processed: usize,
    pub errors: Vec<String>,
}

impl From<SwitchResult> for LegacySyncResult {
    fn from(switch_result: SwitchResult) -> Self {
        let (files_processed, message) = if let Some(sync_result) = switch_result.sync_result {
            let total_files = sync_result.files_added + sync_result.files_updated + sync_result.files_removed;
            let message = if sync_result.success {
                format!("Branch switch completed: {total_files} files processed")
            } else {
                sync_result.error_message.unwrap_or_else(|| "Sync failed".to_string())
            };
            (total_files, message)
        } else {
            (0, "Branch switch completed without sync".to_string())
        };
        
        Self {
            success: switch_result.success,
            message,
            files_processed,
            errors: Vec::new(),
        }
    }
}

/// Compatibility wrapper for git status checking
///
/// Provides the same interface as old git status functions but uses
/// the new git-manager implementation.
pub fn check_repository_status(repo_path: &Path) -> GitResult<Vec<(PathBuf, git2::Status)>> {
    let git_manager = GitManager::new();
    git_manager.get_status(repo_path)
}

/// Compatibility wrapper for checking uncommitted changes
///
/// Provides the same interface as old uncommitted changes checking
/// but uses the new git-manager implementation.
pub fn has_uncommitted_changes(repo_path: &Path) -> GitResult<bool> {
    let git_manager = GitManager::new();
    git_manager.has_uncommitted_changes(repo_path)
}

/// Compatibility wrapper for listing branches
///
/// Provides the same interface as old branch listing functions
/// but uses the new git-manager implementation.
pub fn list_repository_branches(repo_path: &Path) -> GitResult<Vec<String>> {
    let git_manager = GitManager::new();
    git_manager.list_branches(repo_path)
}

/// Compatibility wrapper for getting current branch
///
/// Provides the same interface as old current branch detection
/// but uses the new git-manager implementation.
pub fn get_current_branch(repo_path: &Path) -> GitResult<String> {
    let git_manager = GitManager::new();
    let info = git_manager.get_repository_info(repo_path)?;
    Ok(info.current_branch)
}

/// Compatibility wrapper for getting current commit
///
/// Provides the same interface as old commit detection
/// but uses the new git-manager implementation.
pub fn get_current_commit(repo_path: &Path) -> GitResult<String> {
    let git_manager = GitManager::new();
    let info = git_manager.get_repository_info(repo_path)?;
    Ok(info.current_commit)
}

/// Performance monitoring utilities for migration
pub mod performance {
    use super::*;
    use std::time::{Duration, Instant};
    
    /// Performance metrics for migration operations
    #[derive(Debug, Clone)]
    pub struct MigrationMetrics {
        pub operation: String,
        pub duration: Duration,
        pub files_processed: usize,
        pub memory_used: usize,
        pub success: bool,
    }
    
    /// Benchmark a migration operation
    pub async fn benchmark_migration_operation<F, Fut, T>(
        operation_name: &str,
        operation: F,
    ) -> (GitResult<T>, MigrationMetrics)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = GitResult<T>>,
    {
        let start_time = Instant::now();
        let start_memory = get_memory_usage();
        
        let result = operation().await;
        
        let duration = start_time.elapsed();
        let end_memory = get_memory_usage();
        
        let metrics = MigrationMetrics {
            operation: operation_name.to_string(),
            duration,
            files_processed: 0, // Would need to be passed from operation
            memory_used: end_memory.saturating_sub(start_memory),
            success: result.is_ok(),
        };
        
        (result, metrics)
    }
    
    fn get_memory_usage() -> usize {
        // Simplified memory usage - in real implementation would use proper memory tracking
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyncResult;
    use tempfile::TempDir;
    use std::fs;
    
    #[test]
    fn test_legacy_sync_result_conversion() {
        let switch_result = SwitchResult {
            success: true,
            previous_branch: "main".to_string(),
            new_branch: "feature".to_string(),
            sync_result: Some(SyncResult {
                success: true,
                files_added: 5,
                files_updated: 3,
                files_removed: 1,
                error_message: None,
            }),
            files_changed: 9,
        };
        
        let legacy_result = LegacySyncResult::from(switch_result);
        assert!(legacy_result.success);
        assert_eq!(legacy_result.files_processed, 9);
        assert!(legacy_result.message.contains("9 files processed"));
    }
    
    #[tokio::test]
    async fn test_basic_git_operations() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        
        // Create a minimal git repository
        fs::create_dir_all(&repo_path).unwrap();
        let repo = git2::Repository::init(&repo_path).unwrap();
        
        // Create initial commit
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
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
        
        // Test basic operations
        let current_branch = get_current_branch(&repo_path).unwrap();
        assert!(!current_branch.is_empty());
        
        let current_commit = get_current_commit(&repo_path).unwrap();
        assert!(!current_commit.is_empty());
        
        let branches = list_repository_branches(&repo_path).unwrap();
        assert!(!branches.is_empty());
        
        let has_changes = has_uncommitted_changes(&repo_path).unwrap();
        assert!(!has_changes); // Should be false for clean repo
    }
} 