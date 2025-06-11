use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::utils::errors::SagittaCodeError;

/// Manages working directory context for tools
#[derive(Debug, Clone)]
pub struct WorkingDirectoryManager {
    /// Current working directory
    current_directory: Arc<RwLock<PathBuf>>,
    /// Stack of previous directories for pushd/popd functionality
    directory_stack: Arc<RwLock<Vec<PathBuf>>>,
    /// Base directory (workspace root)
    base_directory: PathBuf,
}

/// Information about the current working directory context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryContext {
    /// Current working directory
    pub current_directory: PathBuf,
    /// Base directory (workspace root)
    pub base_directory: PathBuf,
    /// Whether the current directory exists
    pub exists: bool,
    /// Whether the current directory is writable
    pub writable: bool,
    /// Directory stack depth
    pub stack_depth: usize,
}

/// Result of a directory change operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryChangeResult {
    /// Previous directory
    pub previous_directory: PathBuf,
    /// New directory
    pub new_directory: PathBuf,
    /// Whether the change was successful
    pub success: bool,
    /// Any warnings or messages
    pub message: Option<String>,
}

impl WorkingDirectoryManager {
    /// Create a new working directory manager
    pub fn new(base_directory: PathBuf) -> Result<Self, SagittaCodeError> {
        let current_directory = std::env::current_dir()
            .unwrap_or_else(|_| base_directory.clone());
        
        Ok(Self {
            current_directory: Arc::new(RwLock::new(current_directory)),
            directory_stack: Arc::new(RwLock::new(Vec::new())),
            base_directory,
        })
    }

    /// Get the current working directory
    pub async fn get_current_directory(&self) -> PathBuf {
        self.current_directory.read().await.clone()
    }

    /// Get the base directory (workspace root)
    pub fn get_base_directory(&self) -> &PathBuf {
        &self.base_directory
    }

    /// Change the current working directory
    pub async fn change_directory(&self, new_dir: PathBuf) -> Result<DirectoryChangeResult, SagittaCodeError> {
        let mut current = self.current_directory.write().await;
        let previous = current.clone();

        // Resolve the new directory path
        let resolved_path = if new_dir.is_absolute() {
            new_dir
        } else {
            current.join(new_dir)
        };

        // Canonicalize the path to resolve any .. or . components
        let canonical_path = resolved_path.canonicalize()
            .map_err(|e| SagittaCodeError::ToolError(format!(
                "Failed to resolve directory path '{}': {}. Check if the directory exists.",
                resolved_path.display(), e
            )))?;

        // Check if the directory exists and is actually a directory
        if !canonical_path.exists() {
            return Err(SagittaCodeError::ToolError(format!(
                "Directory does not exist: {}",
                canonical_path.display()
            )));
        }

        if !canonical_path.is_dir() {
            return Err(SagittaCodeError::ToolError(format!(
                "Path is not a directory: {}",
                canonical_path.display()
            )));
        }

        // Update the current directory
        *current = canonical_path.clone();

        // Also update the process working directory if possible
        let message = match std::env::set_current_dir(&canonical_path) {
            Ok(()) => None,
            Err(e) => Some(format!(
                "Updated tool working directory but failed to update process working directory: {}",
                e
            )),
        };

        Ok(DirectoryChangeResult {
            previous_directory: previous,
            new_directory: canonical_path,
            success: true,
            message,
        })
    }

    /// Push the current directory onto the stack and change to a new directory
    pub async fn push_directory(&self, new_dir: PathBuf) -> Result<DirectoryChangeResult, SagittaCodeError> {
        let current = self.get_current_directory().await;
        
        // Push current directory onto stack
        {
            let mut stack = self.directory_stack.write().await;
            stack.push(current);
        }

        // Change to new directory
        self.change_directory(new_dir).await
    }

    /// Pop the previous directory from the stack and change to it
    pub async fn pop_directory(&self) -> Result<DirectoryChangeResult, SagittaCodeError> {
        let previous_dir = {
            let mut stack = self.directory_stack.write().await;
            stack.pop()
        };

        match previous_dir {
            Some(dir) => self.change_directory(dir).await,
            None => Err(SagittaCodeError::ToolError(
                "Directory stack is empty. Cannot pop directory.".to_string()
            )),
        }
    }

    /// Resolve a relative path against the current working directory
    pub async fn resolve_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let path = path.as_ref();
        
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            let current = self.get_current_directory().await;
            current.join(path)
        }
    }

    /// Resolve a path that might be relative to either current directory or base directory
    pub async fn smart_resolve_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let path = path.as_ref();
        
        if path.is_absolute() {
            return path.to_path_buf();
        }

        // Try resolving against current directory first
        let current_resolved = self.resolve_path(path).await;
        if current_resolved.exists() {
            return current_resolved;
        }

        // If not found, try resolving against base directory
        let base_resolved = self.base_directory.join(path);
        if base_resolved.exists() {
            return base_resolved;
        }

        // If neither exists, return the current directory resolution
        current_resolved
    }

    /// Get context information about the current directory
    pub async fn get_directory_context(&self) -> DirectoryContext {
        let current = self.get_current_directory().await;
        let stack_depth = self.directory_stack.read().await.len();

        let exists = current.exists();
        let writable = exists && {
            // Test writability by trying to create a temporary file
            use std::fs::OpenOptions;
            let test_file = current.join(".sagitta_write_test");
            match OpenOptions::new().create(true).write(true).open(&test_file) {
                Ok(_) => {
                    let _ = std::fs::remove_file(&test_file); // Clean up
                    true
                }
                Err(_) => false,
            }
        };

        DirectoryContext {
            current_directory: current,
            base_directory: self.base_directory.clone(),
            exists,
            writable,
            stack_depth,
        }
    }

    /// Make a path relative to the current working directory if possible
    pub async fn make_relative<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let path = path.as_ref();
        let current = self.get_current_directory().await;

        match path.strip_prefix(&current) {
            Ok(relative) => relative.to_path_buf(),
            Err(_) => path.to_path_buf(),
        }
    }

    /// Check if a path is within the workspace (base directory)
    pub fn is_within_workspace<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();
        
        // Convert to absolute path for comparison
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        };

        // Canonicalize both paths for proper comparison
        if let (Ok(canonical_path), Ok(canonical_base)) = 
            (absolute_path.canonicalize(), self.base_directory.canonicalize()) {
            canonical_path.starts_with(canonical_base)
        } else {
            // Fallback to simple prefix check if canonicalization fails
            absolute_path.starts_with(&self.base_directory)
        }
    }

    /// Create a directory if it doesn't exist
    pub async fn ensure_directory_exists<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf, SagittaCodeError> {
        let path = self.resolve_path(path).await;
        
        if path.exists() {
            if !path.is_dir() {
                return Err(SagittaCodeError::ToolError(format!(
                    "Path exists but is not a directory: {}",
                    path.display()
                )));
            }
            return Ok(path);
        }

        std::fs::create_dir_all(&path)
            .map_err(|e| SagittaCodeError::ToolError(format!(
                "Failed to create directory '{}': {}",
                path.display(), e
            )))?;

        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[tokio::test]
    async fn test_working_directory_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkingDirectoryManager::new(temp_dir.path().to_path_buf()).unwrap();
        assert_eq!(manager.get_base_directory(), temp_dir.path());
    }

    #[tokio::test]
    async fn test_change_directory() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let manager = WorkingDirectoryManager::new(temp_dir.path().to_path_buf()).unwrap();
        
        let result = manager.change_directory(sub_dir.clone()).await.unwrap();
        assert_eq!(result.new_directory, sub_dir.canonicalize().unwrap());
        assert!(result.success);

        let current = manager.get_current_directory().await;
        assert_eq!(current, sub_dir.canonicalize().unwrap());
    }

    #[tokio::test]
    async fn test_push_pop_directory() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let manager = WorkingDirectoryManager::new(temp_dir.path().to_path_buf()).unwrap();
        let initial_dir = manager.get_current_directory().await;

        // Push and change directory
        let _push_result = manager.push_directory(sub_dir.clone()).await.unwrap();
        let current = manager.get_current_directory().await;
        assert_eq!(current, sub_dir.canonicalize().unwrap());

        // Pop back to original directory
        let pop_result = manager.pop_directory().await.unwrap();
        assert_eq!(pop_result.new_directory, initial_dir);
    }

    #[tokio::test]
    async fn test_resolve_path() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkingDirectoryManager::new(temp_dir.path().to_path_buf()).unwrap();

        // Test absolute path
        let abs_path = temp_dir.path().join("test.txt");
        let resolved = manager.resolve_path(&abs_path).await;
        assert_eq!(resolved, abs_path);

        // Test relative path
        let rel_path = Path::new("test.txt");
        let resolved = manager.resolve_path(rel_path).await;
        let current = manager.get_current_directory().await;
        assert_eq!(resolved, current.join("test.txt"));
    }

    #[tokio::test]
    async fn test_smart_resolve_path() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create a file in the base directory
        let base_file = temp_dir.path().join("base_file.txt");
        fs::write(&base_file, "base content").unwrap();

        // Create subdirectory with a different file
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        let sub_file = sub_dir.join("sub_file.txt");
        fs::write(&sub_file, "sub content").unwrap();

        let manager = WorkingDirectoryManager::new(temp_dir.path().to_path_buf()).unwrap();
        manager.change_directory(sub_dir).await.unwrap();

        // Should find file in base directory when not in current
        let resolved = manager.smart_resolve_path("base_file.txt").await;
        assert_eq!(resolved, base_file);

        // Should find file in current directory
        let resolved = manager.smart_resolve_path("sub_file.txt").await;
        assert!(resolved.ends_with("sub_file.txt"));
        assert!(resolved.exists());
    }

    #[tokio::test]
    async fn test_directory_context() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkingDirectoryManager::new(temp_dir.path().to_path_buf()).unwrap();

        let context = manager.get_directory_context().await;
        assert!(context.exists);
        assert_eq!(context.base_directory, temp_dir.path().to_path_buf());
        assert_eq!(context.stack_depth, 0);
    }

    #[tokio::test]
    async fn test_is_within_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkingDirectoryManager::new(temp_dir.path().to_path_buf()).unwrap();

        // Path within workspace
        let internal_path = temp_dir.path().join("internal/file.txt");
        assert!(manager.is_within_workspace(&internal_path));

        // Path outside workspace
        let external_path = Path::new("/tmp/external.txt");
        assert!(!manager.is_within_workspace(external_path));
    }

    #[tokio::test]
    async fn test_ensure_directory_exists() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkingDirectoryManager::new(temp_dir.path().to_path_buf()).unwrap();

        let new_dir = "deeply/nested/directory";
        let result = manager.ensure_directory_exists(new_dir).await.unwrap();
        
        assert!(result.exists());
        assert!(result.is_dir());
    }
} 