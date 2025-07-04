//! Repository creation and cloning operations
//!
//! This module provides functionality for creating new repositories and cloning
//! existing repositories from remote sources with support for authentication
//! and progress reporting.

use crate::{GitError, GitResult};
use git2::{
    build::RepoBuilder, CredentialType, RemoteCallbacks, Repository, 
    FetchOptions
};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Options for repository cloning operations
pub struct CloneOptions {
    /// Specific branch to checkout (defaults to remote HEAD)
    pub branch: Option<String>,
    /// Whether to use bare repository (no working directory)
    pub bare: bool,
    /// Depth for shallow clone (None for full clone)
    pub depth: Option<i32>,
    /// SSH private key path for authentication
    pub ssh_private_key: Option<String>,
    /// SSH public key path for authentication
    pub ssh_public_key: Option<String>,
    /// SSH key passphrase
    pub ssh_passphrase: Option<String>,
    /// Username for authentication
    pub username: Option<String>,
    /// Personal access token or password
    pub password: Option<String>,
}

impl std::fmt::Debug for CloneOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloneOptions")
            .field("branch", &self.branch)
            .field("bare", &self.bare)
            .field("depth", &self.depth)
            .field("ssh_private_key", &self.ssh_private_key)
            .field("ssh_public_key", &self.ssh_public_key)
            .field("ssh_passphrase", &self.ssh_passphrase.as_ref().map(|_| "[REDACTED]"))
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl Clone for CloneOptions {
    fn clone(&self) -> Self {
        Self {
            branch: self.branch.clone(),
            bare: self.bare,
            depth: self.depth,
            ssh_private_key: self.ssh_private_key.clone(),
            ssh_public_key: self.ssh_public_key.clone(),
            ssh_passphrase: self.ssh_passphrase.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
        }
    }
}

impl Default for CloneOptions {
    fn default() -> Self {
        Self {
            branch: None,
            bare: false,
            depth: None,
            ssh_private_key: None,
            ssh_public_key: None,
            ssh_passphrase: None,
            username: None,
            password: None,
        }
    }
}

/// Result of a repository cloning operation
#[derive(Debug)]
pub struct CloneResult {
    /// Path where the repository was cloned
    pub path: std::path::PathBuf,
    /// URL that was cloned
    pub url: String,
    /// Branch that was checked out
    pub branch: String,
    /// Number of objects received during clone
    pub objects_received: u32,
    /// Total bytes received
    pub bytes_received: usize,
}

/// Repository cloner with authentication and progress support
pub struct RepositoryCloner {
    /// Whether the operation was cancelled
    cancelled: Arc<AtomicBool>,
}

impl Default for RepositoryCloner {
    fn default() -> Self {
        Self::new()
    }
}

impl RepositoryCloner {
    /// Create a new repository cloner
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Cancel the current cloning operation
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Check if the cloning operation has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Clone a repository from the given URL to the specified path
    ///
    /// # Arguments
    /// * `url` - Git repository URL (HTTP/HTTPS or SSH)
    /// * `path` - Local path where repository should be cloned
    /// * `options` - Cloning options including authentication and branch selection
    ///
    /// # Examples
    /// ```rust,no_run
    /// use git_manager::operations::create::{RepositoryCloner, CloneOptions};
    /// use std::path::PathBuf;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let cloner = RepositoryCloner::new();
    /// let options = CloneOptions::default();
    /// 
    /// let result = cloner.clone_repository(
    ///     "https://github.com/user/repo.git",
    ///     &PathBuf::from("/tmp/repo"),
    ///     options
    /// )?;
    /// 
    /// println!("Cloned to: {}", result.path.display());
    /// # Ok(())
    /// # }
    /// ```
    pub fn clone_repository(
        &self,
        url: &str,
        path: &Path,
        options: CloneOptions,
    ) -> GitResult<CloneResult> {
        // Reset cancellation flag
        self.cancelled.store(false, Ordering::Relaxed);

        let mut builder = RepoBuilder::new();
        let mut callbacks = RemoteCallbacks::new();
        let mut fetch_options = FetchOptions::new();

        // Set up progress callback
        callbacks.update_tips(|_refname: &str, _a: git2::Oid, _b: git2::Oid| -> bool {
            true
        });

        // Set up authentication
        self.setup_authentication(&mut callbacks, &options)?;

        // Set up progress reporting - using pack_progress instead of progress
        let pack_progress_closure = move |_stage: git2::PackBuilderStage, _current: usize, _total: usize| {
        };
        callbacks.pack_progress(pack_progress_closure);

        fetch_options.remote_callbacks(callbacks);

        // Configure builder
        builder.fetch_options(fetch_options);

        if options.bare {
            builder.bare(true);
        }

        if let Some(branch) = &options.branch {
            builder.branch(branch);
        }

        // Perform the clone operation directly (removed spawn_blocking)
        let repo = builder.clone(url, path)
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to clone repository: {}", e),
            })?;

        // Check if operation was cancelled
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(GitError::GitOperationFailed {
                message: "Clone operation was cancelled".to_string(),
            });
        }

        // Get information about the cloned repository
        let head_ref = repo.head()
            .map_err(|e| GitError::GitOperationFailed {
                message: format!("Failed to get HEAD reference: {}", e),
            })?;

        let branch_name = if let Some(name) = head_ref.shorthand() {
            name.to_string()
        } else {
            "HEAD".to_string()
        };

        Ok(CloneResult {
            path: path.to_path_buf(),
            url: url.to_string(),
            branch: branch_name,
            objects_received: 0, // TODO: Track this during progress
            bytes_received: 0,   // TODO: Track this during progress
        })
    }

    /// Clone a repository with default options
    pub fn clone_simple(
        &self,
        url: &str,
        path: &Path,
    ) -> GitResult<CloneResult> {
        self.clone_repository(url, path, CloneOptions::default())
    }

    /// Clone a specific branch
    pub fn clone_branch(
        &self,
        url: &str,
        path: &Path,
        branch: &str,
    ) -> GitResult<CloneResult> {
        let options = CloneOptions {
            branch: Some(branch.to_string()),
            ..Default::default()
        };
        self.clone_repository(url, path, options)
    }

    /// Set up authentication callbacks based on the provided options
    fn setup_authentication(
        &self,
        callbacks: &mut RemoteCallbacks,
        options: &CloneOptions,
    ) -> GitResult<()> {
        let ssh_private_key = options.ssh_private_key.clone();
        let ssh_public_key = options.ssh_public_key.clone();
        let ssh_passphrase = options.ssh_passphrase.clone();
        let username = options.username.clone();
        let password = options.password.clone();

        callbacks.credentials(move |_url, username_from_url, allowed_types| {
            if allowed_types.contains(CredentialType::SSH_KEY) {
                // Try SSH key authentication
                if let (Some(ref public_key), Some(ref private_key)) = 
                    (&ssh_public_key, &ssh_private_key) {
                    return git2::Cred::ssh_key(
                        username_from_url.unwrap_or("git"),
                        Some(Path::new(public_key)),
                        Path::new(private_key),
                        ssh_passphrase.as_deref(),
                    );
                }
            }

            if allowed_types.contains(CredentialType::USER_PASS_PLAINTEXT) {
                // Try username/password authentication
                if let (Some(ref user), Some(ref pass)) = 
                    (&username, &password) {
                    return git2::Cred::userpass_plaintext(user, pass);
                }
            }

            if allowed_types.contains(CredentialType::DEFAULT) {
                // Try default credentials (for SSH agent, etc.)
                return git2::Cred::default();
            }

            Err(git2::Error::from_str("No valid authentication method found"))
        });

        Ok(())
    }
}

/// Initialize a new bare repository at the specified path
pub fn init_repository(path: &Path, bare: bool) -> GitResult<Repository> {
    let repo = if bare {
        Repository::init_bare(path)
    } else {
        Repository::init(path)
    };

    repo.map_err(|e| GitError::GitOperationFailed {
        message: format!("Failed to initialize repository: {}", e),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_clone_options_default() {
        let options = CloneOptions::default();
        assert!(options.branch.is_none());
        assert!(!options.bare);
        assert!(options.depth.is_none());
    }

    #[test]
    fn test_repository_cloner_creation() {
        let cloner = RepositoryCloner::new();
        assert!(!cloner.cancelled.load(Ordering::Relaxed));
    }

    #[test]
    fn test_repository_cloner_cancel() {
        let cloner = RepositoryCloner::new();
        cloner.cancel();
        assert!(cloner.cancelled.load(Ordering::Relaxed));
    }

    #[test]
    fn test_init_repository() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        
        let repo = init_repository(&repo_path, false);
        assert!(repo.is_ok());
        
        // Verify repository was created
        let repo = repo.unwrap();
        assert!(repo.path().exists());
        assert!(!repo.is_bare());
    }

    #[test]
    fn test_init_bare_repository() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_bare_repo");
        
        let repo = init_repository(&repo_path, true);
        assert!(repo.is_ok());
        
        // Verify bare repository was created
        let repo = repo.unwrap();
        assert!(repo.path().exists());
        assert!(repo.is_bare());
    }
} 