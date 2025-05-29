use crate::EmbeddingModelType; // Use re-export from core for type
use crate::error::{Result, SagittaError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{UNIX_EPOCH};
use log;
use chrono::{Utc};
use std::path::PathBuf;

/// Cache entry
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CacheEntry {
    timestamp: u64,
    file_hash: u64,
    model_type: EmbeddingModelType,
}

#[derive(Serialize, Deserialize, Debug)]
struct CacheFile {
    entries: HashMap<String, CacheEntry>,
}

/// Result of a cache check operation.
pub enum CacheCheckResult {
    /// Cache entry is valid and up-to-date.
    Hit,
    /// Cache entry is missing, expired, or invalid (e.g., file changed).
    /// Contains the new file hash if successfully calculated, otherwise None.
    Miss(Option<u64>), // Cache miss, contains Option<file_hash>
}

/// Cache structure
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EmbeddingCache {
    entries: HashMap<String, CacheEntry>,
    #[serde(skip)]
    cache_path: String,
    #[serde(skip)]
    ttl: u64, // Time-to-live in seconds
    #[serde(skip)]
    current_model_type: EmbeddingModelType, // Track current model type
}

impl EmbeddingCache {
    /// Creates a new `EmbeddingCache` or loads it from the specified path.
    /// Initializes with a default TTL and ONNX model type.
    pub fn new(cache_path: String) -> Result<Self> {
        let ttl = 86400 * 7; // Default TTL: 7 days

        if Path::new(&cache_path).exists() {
            let contents = fs::read_to_string(&cache_path)
                .map_err(|e| SagittaError::CacheError(e.to_string()))?;
            let mut cache: Self = serde_json::from_str(&contents)
                .map_err(|e| SagittaError::CacheError(e.to_string()))?;
            cache.cache_path = cache_path;
            cache.ttl = ttl;
            // Default model type on load, user should set it via db
            cache.current_model_type = EmbeddingModelType::Onnx; // Default to Onnx
            Ok(cache)
        } else {
            Ok(Self {
                entries: HashMap::new(),
                cache_path,
                ttl,
                current_model_type: EmbeddingModelType::Onnx, // Default to Onnx
            })
        }
    }

    /// Set the current model type
    pub fn set_model_type(&mut self, model_type: EmbeddingModelType) {
        self.current_model_type = model_type;
    }

    /// Clears all entries from the cache and saves the empty cache.
    pub fn clear(&mut self) -> Result<()> {
        self.entries.clear();
        self.save()?;
        Ok(())
    }

    /// Returns the number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Saves the current cache state to the file specified during creation.
    /// This operation is atomic (uses a temporary file).
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = Path::new(&self.cache_path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| SagittaError::DirectoryCreationError {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let cache_file = CacheFile {
            entries: self.entries.clone(),
        };

        // Create a temporary file first
        let temp_path = format!("{}.tmp", self.cache_path);

        // Write to temporary file first
        let contents =
            serde_json::to_string_pretty(&cache_file)
                .map_err(|e| SagittaError::SerializationError(e.to_string()))?;
        std::fs::write(&temp_path, contents).map_err(|e| SagittaError::FileWriteError {
            path: Path::new(&temp_path).to_path_buf(),
            source: e,
        })?;

        // Atomically rename the temporary file to the actual file
        std::fs::rename(&temp_path, &self.cache_path).map_err(|e| {
            SagittaError::FileWriteError {
                path: Path::new(&self.cache_path).to_path_buf(),
                source: e,
            }
        })?;

        Ok(())
    }

    /// Calculates a hash based on the file's modification time and size.
    /// This is used to detect if a file has changed since it was last cached.
    pub fn get_file_hash(path: &Path) -> Result<u64> {
        let metadata = std::fs::metadata(path).map_err(|e| SagittaError::MetadataError {
            path: path.to_path_buf(),
            source: e,
        })?;

        let modified_time = metadata.modified().map_err(|e| SagittaError::MetadataError {
            path: path.to_path_buf(),
            source: e,
        })?;
        let modified = modified_time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let size = metadata.len();

        Ok(modified.wrapping_mul(31).wrapping_add(size))
    }

    /// Cleans the cache by removing entries whose model type doesn't match the current one.
    pub fn invalidate_different_model_types(&mut self) {
        self.entries
            .retain(|_, entry| entry.model_type == self.current_model_type);
    }

    /// Checks the cache for a file, considering TTL, model type, and file modification.
    /// Returns Hit if valid, or Miss(Option<file_hash>) if missed or invalid.
    pub fn check_cache_and_get_hash(
        &self,
        file_path_str: &str,
        file_path: &Path,
    ) -> Result<CacheCheckResult> {
        if let Some(entry) = self.entries.get(file_path_str) {
            let now = Utc::now().timestamp() as u64;

            // 1. Check TTL
            if now.saturating_sub(entry.timestamp) >= self.ttl {
                // TTL expired, treat as miss but calculate hash
                let hash = Self::get_file_hash(file_path)?;
                return Ok(CacheCheckResult::Miss(Some(hash)));
            }

            // 2. Check model type
            if entry.model_type != self.current_model_type {
                // Model mismatch, treat as miss but calculate hash
                let hash = Self::get_file_hash(file_path)?;
                return Ok(CacheCheckResult::Miss(Some(hash)));
            }

            // 3. Check file hash (modification check)
            match Self::get_file_hash(file_path) {
                Ok(current_hash) => {
                    if entry.file_hash == current_hash {
                        // Cache hit and valid
                        Ok(CacheCheckResult::Hit) // Simplified Hit
                    } else {
                        // File modified, treat as miss, return new hash
                        Ok(CacheCheckResult::Miss(Some(current_hash)))
                    }
                }
                Err(e) => {
                    // Error getting current hash (e.g., file deleted), treat as cache miss
                    // Log the error for debugging
                    log::warn!(
                        "Could not get file hash for cache check {}: {}",
                        file_path.display(),
                        e
                    );
                    Ok(CacheCheckResult::Miss(None)) // Indicate hash couldn't be determined
                }
            }
        } else {
            // Not in cache map, treat as miss
            let hash_opt = Self::get_file_hash(file_path).ok();
            Ok(CacheCheckResult::Miss(hash_opt))
        }
    }

    /// Insert a file hash entry. Used after successful processing of a file's chunks.
    /// Does not save immediately.
    pub fn insert_file_hash(
        &mut self,
        file_path: String,
        file_hash: u64,
    ) -> Result<()> {
        let now = Utc::now().timestamp() as u64;
        let entry = CacheEntry {
            timestamp: now,
            file_hash,
            model_type: self.current_model_type.clone(),
        };

        self.entries.insert(file_path, entry);
        // No save here - intended for batch inserts
        Ok(())
    }

    /// Removes an entry from the cache if it exists.
    #[allow(dead_code)] // Suppress warning, used by Sagitta::remove
    pub fn remove(&mut self, key: &str) -> Result<Option<CacheEntry>> {
        let removed = self.entries.remove(key);
        if removed.is_some() {
            // Save the cache if an entry was actually removed
            self.save()?;
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    
    use crate::EmbeddingModelType;

    // Helper to create a cache for testing
    fn setup_cache_test() -> (tempfile::TempDir, String) {
        let temp_dir = tempdir().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");
        let cache_path_str = cache_path.to_str().unwrap().to_string();
        (temp_dir, cache_path_str)
    }

    #[test]
    fn test_cache_insert_get_save_load() -> Result<()> {
        let (_temp_dir, cache_path) = setup_cache_test();
        let mut cache = EmbeddingCache::new(cache_path.clone())?;
        cache.set_model_type(EmbeddingModelType::Onnx);

        let file_path_str = "test_file.rs".to_string();

        // Create the dummy file *before* calculating hash and inserting
        let dummy_file_path = PathBuf::from(&file_path_str);
        fs::write(&dummy_file_path, "content")?;

        // Calculate the actual hash of the dummy file
        let actual_file_hash = EmbeddingCache::get_file_hash(&dummy_file_path)?;

        // Insert with the actual hash
        cache.insert_file_hash(file_path_str.clone(), actual_file_hash)?;
        cache.save()?;

        let mut loaded_cache = EmbeddingCache::new(cache_path)?;
        assert_eq!(loaded_cache.len(), 1);
        loaded_cache.set_model_type(EmbeddingModelType::Onnx); // Ensure loaded cache instance has correct type for check

        // Check cache using the same file path
        let check_result = loaded_cache.check_cache_and_get_hash(&file_path_str, &dummy_file_path)?;
        match check_result {
            CacheCheckResult::Hit => { /* Cache hit as expected */ }
            CacheCheckResult::Miss(_) => panic!("Cache check should be a hit"),
        }

        Ok(())
    }

    #[test]
    fn test_cache_invalidate_types() -> Result<()> {
        let (_temp_dir, cache_path) = setup_cache_test();
        let mut cache = EmbeddingCache::new(cache_path)?;

        cache.set_model_type(crate::EmbeddingModelType::Onnx);
        cache.insert_file_hash("file1.rs".to_string(), 1)?; // ONNX entry
        // cache.set_model_type(EmbeddingModelType::Default); // Assume a Default type existed
        // cache.insert_file_hash("file2.py".to_string(), 2)?; // Default entry

        // Set current type to ONNX and invalidate
        cache.set_model_type(crate::EmbeddingModelType::Onnx);
        cache.invalidate_different_model_types();

        assert_eq!(cache.len(), 1); // Only file1 should remain
        assert!(cache.entries.contains_key("file1.rs"));
        // assert!(!cache.entries.contains_key("file2.py"));

        Ok(())
    }

    #[test]
    fn test_cache_basic() -> Result<()> {
        let (_temp_dir, cache_path) = setup_cache_test();
        let mut cache = EmbeddingCache::new(cache_path)?;

        assert_eq!(cache.len(), 0);

        // Create dummy file
        let file_path = _temp_dir.path().join("my_file.txt");
        fs::write(&file_path, "some content")?;
        let file_path_str = file_path.to_str().unwrap().to_string();
        let file_hash = EmbeddingCache::get_file_hash(&file_path)?;

        // Initial check: Miss
        match cache.check_cache_and_get_hash(&file_path_str, &file_path)? {
            CacheCheckResult::Miss(Some(hash)) => assert_eq!(hash, file_hash),
            _ => panic!("Initial check should be Miss"),
        }

        // Insert and check again: Hit
        cache.insert_file_hash(file_path_str.clone(), file_hash)?;
        match cache.check_cache_and_get_hash(&file_path_str, &file_path)? {
            CacheCheckResult::Hit => { /* Correct */ }
            _ => panic!("Second check should be Hit"),
        }

        // Modify file and check: Miss
        fs::write(&file_path, "new content")?;
        let new_hash = EmbeddingCache::get_file_hash(&file_path)?;
        match cache.check_cache_and_get_hash(&file_path_str, &file_path)? {
            CacheCheckResult::Miss(Some(hash)) => assert_eq!(hash, new_hash),
            _ => panic!("Third check should be Miss after modification"),
        }

        Ok(())
    }

    #[test]
    fn test_cache_ttl() -> Result<()> {
        let (_temp_dir, cache_path) = setup_cache_test();
        let mut cache = EmbeddingCache::new(cache_path)?;
        cache.ttl = 1; // Set TTL to 1 second for testing

        let file_path = _temp_dir.path().join("ttl_test.txt");
        fs::write(&file_path, "ttl content")?;
        let file_path_str = file_path.to_str().unwrap().to_string();
        let file_hash = EmbeddingCache::get_file_hash(&file_path)?;

        cache.insert_file_hash(file_path_str.clone(), file_hash)?;

        // Check immediately: Hit
        match cache.check_cache_and_get_hash(&file_path_str, &file_path)? {
            CacheCheckResult::Hit => { /* Correct */ }
            _ => panic!("Immediate check should be Hit"),
        }

        // Wait for TTL to expire
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Check after TTL: Miss
        match cache.check_cache_and_get_hash(&file_path_str, &file_path)? {
            CacheCheckResult::Miss(Some(hash)) => assert_eq!(hash, file_hash),
            _ => panic!("Check after TTL should be Miss"),
        }

        Ok(())
    }

    #[test]
    fn test_cache_model_type() -> Result<()> {
        let (_temp_dir, cache_path) = setup_cache_test();
        let mut cache = EmbeddingCache::new(cache_path)?;

        let file_path = _temp_dir.path().join("model_test.txt");
        fs::write(&file_path, "model content")?;
        let file_path_str = file_path.to_str().unwrap().to_string();
        let file_hash = EmbeddingCache::get_file_hash(&file_path)?;

        // Insert with ONNX model
        cache.set_model_type(crate::EmbeddingModelType::Onnx);
        cache.insert_file_hash(file_path_str.clone(), file_hash)?;

        // Check with ONNX: Hit
        cache.set_model_type(crate::EmbeddingModelType::Onnx);
        match cache.check_cache_and_get_hash(&file_path_str, &file_path)? {
            CacheCheckResult::Hit => { /* Correct */ }
            _ => panic!("Check with same model type should be Hit"),
        }

        // Check with Default: Miss
        cache.set_model_type(crate::EmbeddingModelType::Default);
        match cache.check_cache_and_get_hash(&file_path_str, &file_path)? {
            CacheCheckResult::Miss(Some(hash)) => assert_eq!(hash, file_hash),
            _ => panic!("Check with different model type should be Miss"),
        }

        Ok(())
    }

    #[test]
    fn test_file_hash_consistency() -> Result<()> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("consistent_hash.txt");

        // Create file
        fs::write(&file_path, "initial content")?;
        let hash1 = EmbeddingCache::get_file_hash(&file_path)?;

        // Get hash again without changes
        let hash2 = EmbeddingCache::get_file_hash(&file_path)?;
        assert_eq!(hash1, hash2, "Hash should be consistent for unchanged file");

        // Modify content
        fs::write(&file_path, "modified content")?;
        let hash3 = EmbeddingCache::get_file_hash(&file_path)?;
        assert_ne!(hash1, hash3, "Hash should change after content modification");

        // Introduce delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Modify timestamp (simulate touch)
        let current_time = std::time::SystemTime::now();
        let mtime = filetime::FileTime::from_system_time(current_time);
        filetime::set_file_mtime(&file_path, mtime)?;
        let hash4 = EmbeddingCache::get_file_hash(&file_path)?;
        assert_ne!(hash3, hash4, "Hash should change after timestamp modification");

        Ok(())
    }
} 