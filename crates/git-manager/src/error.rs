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