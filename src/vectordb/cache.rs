use crate::vectordb::embedding::EmbeddingModelType;
use crate::vectordb::error::{Result, VectorDBError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CacheEntry {
    // embedding: Vec<f32>, // Removed embedding
    timestamp: u64,
    file_hash: u64,
    model_type: EmbeddingModelType,
}

#[derive(Serialize, Deserialize, Debug)]
struct CacheFile {
    entries: HashMap<String, CacheEntry>,
}

pub enum CacheCheckResult {
    // Hit(Vec<f32>), // Removed Hit variant with embedding
    Hit,             // Simplified Hit variant
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
    pub fn new(cache_path: String) -> Result<Self> {
        let ttl = 86400 * 7; // Default TTL: 7 days

        if Path::new(&cache_path).exists() {
            let contents = fs::read_to_string(&cache_path)
                .map_err(|e| VectorDBError::CacheError(e.to_string()))?;
            let mut cache: Self = serde_json::from_str(&contents)
                .map_err(|e| VectorDBError::CacheError(e.to_string()))?;
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

    pub fn clear(&mut self) -> Result<()> {
        self.entries.clear();
        self.save()?;
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = Path::new(&self.cache_path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| VectorDBError::DirectoryCreationError {
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
            serde_json::to_string_pretty(&cache_file).map_err(VectorDBError::SerializationError)?;
        std::fs::write(&temp_path, contents).map_err(|e| VectorDBError::FileWriteError {
            path: Path::new(&temp_path).to_path_buf(),
            source: e,
        })?;

        // Atomically rename the temporary file to the actual file
        std::fs::rename(&temp_path, &self.cache_path).map_err(|e| {
            VectorDBError::FileWriteError {
                path: Path::new(&self.cache_path).to_path_buf(),
                source: e,
            }
        })?;

        Ok(())
    }

    pub fn get_file_hash(path: &Path) -> Result<u64> {
        let metadata = std::fs::metadata(path).map_err(|e| VectorDBError::MetadataError {
            path: path.to_path_buf(),
            source: e,
        })?;

        let modified = metadata
            .modified()
            .map_err(|e| VectorDBError::MetadataError {
                path: path.to_path_buf(),
                source: e,
            })?
            .duration_since(UNIX_EPOCH)
            .map_err(|e| VectorDBError::CacheError(e.to_string()))?
            .as_secs();

        let size = metadata.len();

        Ok(modified.wrapping_mul(31).wrapping_add(size as u64))
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
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| VectorDBError::CacheError(format!("System time error: {}", e)))?
                .as_secs();

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
                        // Ok(CacheCheckResult::Hit(entry.embedding.clone())) // Removed embedding
                        Ok(CacheCheckResult::Hit) // Simplified Hit
                    } else {
                        // File modified, treat as miss, return new hash
                        Ok(CacheCheckResult::Miss(Some(current_hash)))
                    }
                }
                Err(e) => {
                    // Error getting current hash (e.g., file deleted), treat as cache miss
                    // Log the error for debugging
                    eprintln!(
                        "Warning: Could not get file hash for cache check {}: {}",
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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| VectorDBError::CacheError(e.to_string()))?
            .as_secs();

        let entry = CacheEntry {
            // embedding, // Removed
            timestamp: now,
            file_hash,
            model_type: self.current_model_type.clone(),
        };

        self.entries.insert(file_path, entry);
        // No save here - intended for batch inserts
        Ok(())
    }

    /// Removes an entry from the cache if it exists.
    #[allow(dead_code)] // Suppress warning, used by VectorDB::remove
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

    #[test]
    fn test_cache_basic() -> Result<()> {
        let dir = tempdir()?;
        let cache_path = dir.path().join("cache.json").to_string_lossy().to_string();

        let mut cache = EmbeddingCache::new(cache_path.clone())?;
        assert_eq!(cache.len(), 0);

        // Insert an item
        // let embedding = vec![1.0, 2.0, 3.0]; // Removed
        let file_hash = 12345u64; // Example hash
        // cache.insert_with_hash("test".to_string(), embedding.clone(), file_hash)?; // Removed
        cache.insert_file_hash("test".to_string(), file_hash)?; // Use new method
        assert_eq!(cache.len(), 1);

        // Check cache hit
        let temp_file = dir.path().join("test_file.txt");
        fs::write(&temp_file, "content")?;
        let file_hash_check = EmbeddingCache::get_file_hash(&temp_file)?;
        // Need to insert again with the actual hash for check to work
        cache.insert_file_hash("test_file.txt".to_string(), file_hash_check)?;

        let check_result = cache.check_cache_and_get_hash("test_file.txt", &temp_file)?;
        match check_result {
            // CacheCheckResult::Hit(cached_embedding) => assert_eq!(cached_embedding, embedding),
            CacheCheckResult::Hit => { /* Correct */ },
            CacheCheckResult::Miss(_) => panic!("Expected cache hit"),
        }

        // Save and reload
        cache.save()?;
        let reloaded_cache = EmbeddingCache::new(cache_path)?;
        assert_eq!(reloaded_cache.len(), 2); // test and test_file.txt
        Ok(())
    }

    #[test]
    fn test_cache_ttl() -> Result<()> {
        let dir = tempdir()?;
        let cache_path = dir.path().join("cache.json").to_string_lossy().to_string();
        let mut cache = EmbeddingCache::new(cache_path.clone())?;
        cache.ttl = 1; // Set TTL to 1 second for testing

        let file_path = "ttl_test.txt".to_string();
        let temp_file_path = dir.path().join(&file_path);
        fs::write(&temp_file_path, "some data")?;
        let file_hash = EmbeddingCache::get_file_hash(&temp_file_path)?;

        // Insert entry
        cache.insert_file_hash(file_path.clone(), file_hash)?;

        // Check immediately (should be hit)
        match cache.check_cache_and_get_hash(&file_path, &temp_file_path)? {
            CacheCheckResult::Hit => { /* OK */ }
            _ => panic!("Expected immediate cache hit"),
        }

        // Wait for TTL to expire
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Check again (should be miss)
        match cache.check_cache_and_get_hash(&file_path, &temp_file_path)? {
            CacheCheckResult::Miss(Some(h)) => assert_eq!(h, file_hash),
            _ => panic!("Expected cache miss due to TTL expiry"),
        }

        Ok(())
    }

    #[test]
    fn test_cache_model_type() -> Result<()> {
        let dir = tempdir()?;
        let cache_path = dir.path().join("cache.json").to_string_lossy().to_string();
        let mut cache = EmbeddingCache::new(cache_path.clone())?;

        let file_path = "model_test.txt".to_string();
        let temp_file_path = dir.path().join(&file_path);
        fs::write(&temp_file_path, "data")?;
        let file_hash = EmbeddingCache::get_file_hash(&temp_file_path)?;

        // Set initial model type (e.g., Onnx) and insert
        cache.set_model_type(EmbeddingModelType::Onnx);
        cache.insert_file_hash(file_path.clone(), file_hash)?;
        assert_eq!(cache.len(), 1);

        // Check (should be hit)
        match cache.check_cache_and_get_hash(&file_path, &temp_file_path)? {
            CacheCheckResult::Hit => { /* OK */ }
            _ => panic!("Expected cache hit with matching model type"),
        }

        Ok(())
    }

    #[test]
    fn test_file_hash_consistency() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("hash_test.txt");

        // Create file
        fs::write(&file_path, "initial content")?;
        let hash1 = EmbeddingCache::get_file_hash(&file_path)?;

        // Check hash again without modification
        let hash2 = EmbeddingCache::get_file_hash(&file_path)?;
        assert_eq!(hash1, hash2);

        // Modify content
        fs::write(&file_path, "modified content")?;
        let hash3 = EmbeddingCache::get_file_hash(&file_path)?;
        assert_ne!(hash1, hash3);

        // Modify timestamp (tricky to do precisely, but changing content changes timestamp)
        // Let's just assert hash changes after modification

        Ok(())
    }
}
