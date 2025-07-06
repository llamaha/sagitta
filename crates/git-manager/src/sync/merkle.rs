use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;
use crate::error::{GitError, GitResult};

/// Calculate SHA-256 hash of file content and metadata
pub fn calculate_file_hash(path: &Path) -> GitResult<String> {
    let metadata = fs::metadata(path).map_err(|e| {
        GitError::filesystem_error(format!("Failed to read metadata for {}: {}", path.display(), e))
    })?;

    let content = fs::read(path).map_err(|e| {
        GitError::filesystem_error(format!("Failed to read file {}: {}", path.display(), e))
    })?;

    let mut hasher = Sha256::new();
    
    // Include file size and content in hash
    hasher.update(metadata.len().to_le_bytes());
    hasher.update(&content);
    
    // Include modification time if available
    if let Ok(modified) = metadata.modified() {
        if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
            hasher.update(duration.as_secs().to_le_bytes());
        }
    }

    let result = hasher.finalize();
    Ok(format!("{result:x}"))
}

/// Calculate merkle root from a collection of file hashes
pub fn calculate_merkle_root(file_hashes: &HashMap<PathBuf, String>) -> String {
    if file_hashes.is_empty() {
        return String::new();
    }

    // Sort paths for deterministic ordering
    let mut sorted_entries: Vec<_> = file_hashes.iter().collect();
    sorted_entries.sort_by(|a, b| a.0.cmp(b.0));

    let mut hasher = Sha256::new();
    for (path, hash) in sorted_entries {
        // Include both path and hash in the merkle calculation
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(hash.as_bytes());
    }

    let result = hasher.finalize();
    format!("{result:x}")
}

/// Calculate hashes for all files in a directory
pub fn calculate_directory_hashes(
    directory: &Path,
    ignore_patterns: &[&str],
) -> GitResult<HashMap<PathBuf, String>> {
    let mut file_hashes = HashMap::new();

    for entry in WalkDir::new(directory)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !should_ignore_entry(e, ignore_patterns))
    {
        let entry = entry.map_err(|e| {
            GitError::filesystem_error(format!("Error walking directory: {e}"))
        })?;

        if entry.file_type().is_file() {
            let path = entry.path();
            let relative_path = path.strip_prefix(directory).map_err(|_| {
                GitError::filesystem_error(format!(
                    "Failed to create relative path for {}",
                    path.display()
                ))
            })?;

            match calculate_file_hash(path) {
                Ok(hash) => {
                    file_hashes.insert(relative_path.to_path_buf(), hash);
                }
                Err(e) => {
                    tracing::warn!("Failed to hash file {}: {}", path.display(), e);
                    // Continue processing other files
                }
            }
        }
    }

    Ok(file_hashes)
}

/// Check if a directory entry should be ignored
fn should_ignore_entry(entry: &walkdir::DirEntry, ignore_patterns: &[&str]) -> bool {
    let path = entry.path();
    let path_str = path.to_string_lossy();

    // Always ignore .git directory
    if path_str.contains("/.git/") || path_str.ends_with("/.git") {
        return true;
    }

    // Check custom ignore patterns
    for pattern in ignore_patterns {
        // Handle wildcard patterns like "*.tmp"
        if let Some(extension) = pattern.strip_prefix("*.") {
            if let Some(file_extension) = path.extension() {
                if file_extension.to_string_lossy() == extension {
                    return true;
                }
            }
        }
        // Handle directory patterns or exact filename matches
        else if path_str.contains(pattern) || 
                path.file_name().is_some_and(|name| name.to_string_lossy() == *pattern) {
            return true;
        }
    }

    false
}

/// Represents the difference between two sets of file hashes
#[derive(Debug, Clone)]
pub struct HashDiff {
    /// Files that were added
    pub added: Vec<PathBuf>,
    /// Files that were modified
    pub modified: Vec<PathBuf>,
    /// Files that were deleted
    pub deleted: Vec<PathBuf>,
    /// Files that are unchanged
    pub unchanged: Vec<PathBuf>,
}

impl Default for HashDiff {
    fn default() -> Self {
        Self::new()
    }
}

impl HashDiff {
    /// Create a new empty HashDiff
    pub fn new() -> Self {
        Self {
            added: Vec::new(),
            modified: Vec::new(),
            deleted: Vec::new(),
            unchanged: Vec::new(),
        }
    }

    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.modified.is_empty() || !self.deleted.is_empty()
    }

    /// Get total number of changed files
    pub fn total_changes(&self) -> usize {
        self.added.len() + self.modified.len() + self.deleted.len()
    }

    /// Get all changed files
    pub fn changed_files(&self) -> Vec<PathBuf> {
        let mut changed = Vec::new();
        changed.extend(self.added.iter().cloned());
        changed.extend(self.modified.iter().cloned());
        changed.extend(self.deleted.iter().cloned());
        changed
    }
}

/// Compare two sets of file hashes and return the differences
pub fn compare_file_hashes(
    old_hashes: &HashMap<PathBuf, String>,
    new_hashes: &HashMap<PathBuf, String>,
) -> HashDiff {
    let mut diff = HashDiff::new();

    // Find added and modified files
    for (path, new_hash) in new_hashes {
        match old_hashes.get(path) {
            Some(old_hash) => {
                if old_hash != new_hash {
                    diff.modified.push(path.clone());
                } else {
                    diff.unchanged.push(path.clone());
                }
            }
            None => {
                diff.added.push(path.clone());
            }
        }
    }

    // Find deleted files
    for path in old_hashes.keys() {
        if !new_hashes.contains_key(path) {
            diff.deleted.push(path.clone());
        }
    }

    diff
}

/// Merkle tree manager for efficient change detection
#[derive(Debug)]
pub struct MerkleManager {
    /// Default ignore patterns for file scanning
    default_ignore_patterns: Vec<String>,
}

impl MerkleManager {
    /// Create a new MerkleManager
    pub fn new() -> Self {
        Self {
            default_ignore_patterns: vec![
                ".git".to_string(),
                "target".to_string(),
                "node_modules".to_string(),
                ".DS_Store".to_string(),
                "*.tmp".to_string(),
                "*.log".to_string(),
            ],
        }
    }

    /// Add an ignore pattern
    pub fn add_ignore_pattern(&mut self, pattern: String) {
        self.default_ignore_patterns.push(pattern);
    }

    /// Calculate merkle state for a directory
    pub fn calculate_merkle_state(
        &self,
        directory: &Path,
        additional_ignores: Option<&[&str]>,
    ) -> GitResult<(String, HashMap<PathBuf, String>)> {
        let mut ignore_patterns: Vec<&str> = self
            .default_ignore_patterns
            .iter()
            .map(|s| s.as_str())
            .collect();

        if let Some(additional) = additional_ignores {
            ignore_patterns.extend(additional);
        }

        let file_hashes = calculate_directory_hashes(directory, &ignore_patterns)?;
        let merkle_root = calculate_merkle_root(&file_hashes);

        Ok((merkle_root, file_hashes))
    }

    /// Compare two merkle states and return differences
    pub fn compare_states(
        &self,
        old_hashes: &HashMap<PathBuf, String>,
        new_hashes: &HashMap<PathBuf, String>,
    ) -> HashDiff {
        compare_file_hashes(old_hashes, new_hashes)
    }

    /// Quick check if two merkle roots are different
    pub fn roots_differ(&self, root1: &str, root2: &str) -> bool {
        root1 != root2
    }
}

impl Default for MerkleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_calculate_file_hash() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!").unwrap();

        let hash = calculate_file_hash(&file_path).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA-256 produces 64 character hex string
    }

    #[test]
    fn test_calculate_merkle_root() {
        let mut file_hashes = HashMap::new();
        file_hashes.insert(PathBuf::from("file1.txt"), "hash1".to_string());
        file_hashes.insert(PathBuf::from("file2.txt"), "hash2".to_string());

        let root = calculate_merkle_root(&file_hashes);
        assert!(!root.is_empty());

        // Same files should produce same root
        let root2 = calculate_merkle_root(&file_hashes);
        assert_eq!(root, root2);

        // Different order should produce same root (deterministic)
        let mut file_hashes2 = HashMap::new();
        file_hashes2.insert(PathBuf::from("file2.txt"), "hash2".to_string());
        file_hashes2.insert(PathBuf::from("file1.txt"), "hash1".to_string());
        let root3 = calculate_merkle_root(&file_hashes2);
        assert_eq!(root, root3);
    }

    #[test]
    fn test_compare_file_hashes() {
        let mut old_hashes = HashMap::new();
        old_hashes.insert(PathBuf::from("file1.txt"), "hash1".to_string());
        old_hashes.insert(PathBuf::from("file2.txt"), "hash2".to_string());

        let mut new_hashes = HashMap::new();
        new_hashes.insert(PathBuf::from("file1.txt"), "hash1_modified".to_string());
        new_hashes.insert(PathBuf::from("file3.txt"), "hash3".to_string());

        let diff = compare_file_hashes(&old_hashes, &new_hashes);

        assert_eq!(diff.modified.len(), 1);
        assert!(diff.modified.contains(&PathBuf::from("file1.txt")));

        assert_eq!(diff.added.len(), 1);
        assert!(diff.added.contains(&PathBuf::from("file3.txt")));

        assert_eq!(diff.deleted.len(), 1);
        assert!(diff.deleted.contains(&PathBuf::from("file2.txt")));

        assert!(diff.has_changes());
        assert_eq!(diff.total_changes(), 3);
    }

    #[test]
    fn test_merkle_manager() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!").unwrap();

        let manager = MerkleManager::new();
        let (root, hashes) = manager
            .calculate_merkle_state(temp_dir.path(), None)
            .unwrap();

        assert!(!root.is_empty());
        assert_eq!(hashes.len(), 1);
        assert!(hashes.contains_key(&PathBuf::from("test.txt")));
    }
} 