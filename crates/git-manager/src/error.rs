use std::path::PathBuf;
use thiserror::Error;

/// Git-specific error types for the git-manager crate
#[derive(Error, Debug)]
pub enum GitError {
    #[error("Repository not found at path: {path}")]
    RepositoryNotFound { path: PathBuf },

    #[error("Branch '{branch}' not found in repository")]
    BranchNotFound { branch: String },

    #[error("Branch '{branch}' already exists")]
    BranchAlreadyExists { branch: String },

    #[error("Cannot delete the current HEAD branch")]
    DeleteHeadBranch,

    #[error("Cannot switch to branch '{branch}': uncommitted changes present")]
    UncommittedChanges { branch: String },

    #[error("Remote '{remote}' not found")]
    RemoteNotFound { remote: String },

    #[error("Authentication failed for remote operation")]
    AuthenticationFailed,

    #[error("Network error during git operation: {message}")]
    NetworkError { message: String },

    #[error("Merge conflict detected in files: {files:?}")]
    MergeConflict { files: Vec<PathBuf> },

    #[error("Invalid repository state: {message}")]
    InvalidState { message: String },

    #[error("File system error: {message}")]
    FileSystemError { message: String },

    #[error("Merkle tree calculation failed: {message}")]
    MerkleError { message: String },

    #[error("Sync operation failed: {message}")]
    SyncError { message: String },

    #[error("Git operation failed: {message}")]
    GitOperationFailed { message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git2 error: {0}")]
    Git2(#[from] git2::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Path error: invalid UTF-8 in path")]
    InvalidPath,

    #[error("Configuration error: {message}")]
    ConfigError { message: String },
}

/// Result type alias for git operations
pub type GitResult<T> = Result<T, GitError>;

impl GitError {
    /// Create a new GitOperationFailed error
    pub fn operation_failed(message: impl Into<String>) -> Self {
        Self::GitOperationFailed {
            message: message.into(),
        }
    }

    /// Create a new InvalidState error
    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::InvalidState {
            message: message.into(),
        }
    }

    /// Create a new MerkleError
    pub fn merkle_error(message: impl Into<String>) -> Self {
        Self::MerkleError {
            message: message.into(),
        }
    }

    /// Create a new SyncError
    pub fn sync_error(message: impl Into<String>) -> Self {
        Self::SyncError {
            message: message.into(),
        }
    }

    /// Create a new FileSystemError
    pub fn filesystem_error(message: impl Into<String>) -> Self {
        Self::FileSystemError {
            message: message.into(),
        }
    }

    /// Create a new NetworkError
    pub fn network_error(message: impl Into<String>) -> Self {
        Self::NetworkError {
            message: message.into(),
        }
    }

    /// Create a new ConfigError
    pub fn config_error(message: impl Into<String>) -> Self {
        Self::ConfigError {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_not_found_error() {
        let path = PathBuf::from("/tmp/nonexistent");
        let error = GitError::RepositoryNotFound { path: path.clone() };
        assert_eq!(
            error.to_string(),
            format!("Repository not found at path: {}", path.display())
        );
    }

    #[test]
    fn test_branch_not_found_error() {
        let branch = "feature-xyz".to_string();
        let error = GitError::BranchNotFound {
            branch: branch.clone(),
        };
        assert_eq!(
            error.to_string(),
            format!("Branch '{}' not found in repository", branch)
        );
    }

    #[test]
    fn test_branch_already_exists_error() {
        let branch = "main".to_string();
        let error = GitError::BranchAlreadyExists {
            branch: branch.clone(),
        };
        assert_eq!(
            error.to_string(),
            format!("Branch '{}' already exists", branch)
        );
    }

    #[test]
    fn test_delete_head_branch_error() {
        let error = GitError::DeleteHeadBranch;
        assert_eq!(error.to_string(), "Cannot delete the current HEAD branch");
    }

    #[test]
    fn test_uncommitted_changes_error() {
        let branch = "develop".to_string();
        let error = GitError::UncommittedChanges {
            branch: branch.clone(),
        };
        assert_eq!(
            error.to_string(),
            format!(
                "Cannot switch to branch '{}': uncommitted changes present",
                branch
            )
        );
    }

    #[test]
    fn test_remote_not_found_error() {
        let remote = "origin".to_string();
        let error = GitError::RemoteNotFound {
            remote: remote.clone(),
        };
        assert_eq!(error.to_string(), format!("Remote '{}' not found", remote));
    }

    #[test]
    fn test_authentication_failed_error() {
        let error = GitError::AuthenticationFailed;
        assert_eq!(
            error.to_string(),
            "Authentication failed for remote operation"
        );
    }

    #[test]
    fn test_network_error() {
        let message = "Connection timeout".to_string();
        let error = GitError::NetworkError {
            message: message.clone(),
        };
        assert_eq!(
            error.to_string(),
            format!("Network error during git operation: {}", message)
        );
    }

    #[test]
    fn test_merge_conflict_error() {
        let files = vec![
            PathBuf::from("src/main.rs"),
            PathBuf::from("Cargo.toml"),
        ];
        let error = GitError::MergeConflict {
            files: files.clone(),
        };
        assert_eq!(
            error.to_string(),
            format!("Merge conflict detected in files: {:?}", files)
        );
    }

    #[test]
    fn test_invalid_state_error() {
        let message = "Repository in detached HEAD state".to_string();
        let error = GitError::InvalidState {
            message: message.clone(),
        };
        assert_eq!(
            error.to_string(),
            format!("Invalid repository state: {}", message)
        );
    }

    #[test]
    fn test_filesystem_error() {
        let message = "Permission denied".to_string();
        let error = GitError::FileSystemError {
            message: message.clone(),
        };
        assert_eq!(error.to_string(), format!("File system error: {}", message));
    }

    #[test]
    fn test_merkle_error() {
        let message = "Failed to compute hash".to_string();
        let error = GitError::MerkleError {
            message: message.clone(),
        };
        assert_eq!(
            error.to_string(),
            format!("Merkle tree calculation failed: {}", message)
        );
    }

    #[test]
    fn test_sync_error() {
        let message = "Remote repository unavailable".to_string();
        let error = GitError::SyncError {
            message: message.clone(),
        };
        assert_eq!(
            error.to_string(),
            format!("Sync operation failed: {}", message)
        );
    }

    #[test]
    fn test_git_operation_failed_error() {
        let message = "Failed to create commit".to_string();
        let error = GitError::GitOperationFailed {
            message: message.clone(),
        };
        assert_eq!(
            error.to_string(),
            format!("Git operation failed: {}", message)
        );
    }

    #[test]
    fn test_invalid_path_error() {
        let error = GitError::InvalidPath;
        assert_eq!(error.to_string(), "Path error: invalid UTF-8 in path");
    }

    #[test]
    fn test_config_error() {
        let message = "Missing required configuration".to_string();
        let error = GitError::ConfigError {
            message: message.clone(),
        };
        assert_eq!(
            error.to_string(),
            format!("Configuration error: {}", message)
        );
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let git_error = GitError::from(io_error);
        assert!(matches!(git_error, GitError::Io(_)));
        assert!(git_error.to_string().contains("File not found"));
    }

    #[test]
    fn test_helper_methods() {
        // Test operation_failed helper
        let error = GitError::operation_failed("Push failed");
        assert_eq!(error.to_string(), "Git operation failed: Push failed");

        // Test invalid_state helper
        let error = GitError::invalid_state("Corrupted index");
        assert_eq!(error.to_string(), "Invalid repository state: Corrupted index");

        // Test merkle_error helper
        let error = GitError::merkle_error("Hash mismatch");
        assert_eq!(error.to_string(), "Merkle tree calculation failed: Hash mismatch");

        // Test sync_error helper
        let error = GitError::sync_error("Connection lost");
        assert_eq!(error.to_string(), "Sync operation failed: Connection lost");

        // Test filesystem_error helper
        let error = GitError::filesystem_error("Disk full");
        assert_eq!(error.to_string(), "File system error: Disk full");

        // Test network_error helper
        let error = GitError::network_error("DNS resolution failed");
        assert_eq!(error.to_string(), "Network error during git operation: DNS resolution failed");

        // Test config_error helper
        let error = GitError::config_error("Invalid setting");
        assert_eq!(error.to_string(), "Configuration error: Invalid setting");
    }

    #[test]
    fn test_error_debug_format() {
        let error = GitError::BranchNotFound {
            branch: "test".to_string(),
        };
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("BranchNotFound"));
        assert!(debug_str.contains("test"));
    }
} 