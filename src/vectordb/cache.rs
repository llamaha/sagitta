use crate::vectordb::embedding::EmbeddingModelType;
use crate::vectordb::error::{Result, VectorDBError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CacheEntry {
    embedding: Vec<f32>,
    timestamp: u64,
    file_hash: u64,
    model_type: EmbeddingModelType,
}

#[derive(Serialize, Deserialize, Debug)]
struct CacheFile {
    entries: HashMap<String, CacheEntry>,
}

pub enum CacheCheckResult {
    Hit(Vec<f32>),     // Cache hit, contains the embedding
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
    /// Returns the embedding if hit and valid, or the file hash if missed or invalid.
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
                        Ok(CacheCheckResult::Hit(entry.embedding.clone()))
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

    /// Insert an embedding with a pre-calculated hash. Used after batch processing.
    /// Does not save immediately.
    pub fn insert_with_hash(
        &mut self,
        file_path: String,
        embedding: Vec<f32>,
        file_hash: u64,
    ) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| VectorDBError::CacheError(e.to_string()))?
            .as_secs();

        let entry = CacheEntry {
            embedding,
            timestamp: now,
            file_hash,
            model_type: self.current_model_type.clone(),
        };

        self.entries.insert(file_path, entry);
        // No save here - intended for batch inserts
        Ok(())
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
        let embedding = vec![1.0, 2.0, 3.0];
        let file_hash = 12345u64; // Example hash
        cache.insert_with_hash("test".to_string(), embedding.clone(), file_hash)?;
        assert_eq!(cache.len(), 1);

        // Get the item via entries map
        let retrieved = cache.entries.get("test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().embedding, embedding);

        // Save before reloading
        cache.save()?;

        // Check persistence
        let cache2 = EmbeddingCache::new(cache_path)?;
        let retrieved2 = cache2.entries.get("test");
        assert!(retrieved2.is_some());
        assert_eq!(retrieved2.unwrap().embedding, embedding);

        Ok(())
    }

    #[test]
    fn test_cache_ttl() -> Result<()> {
        let dir = tempdir()?;
        let cache_path = dir.path().join("cache.json").to_string_lossy().to_string();

        let mut cache = EmbeddingCache::new(cache_path)?;
        cache.set_model_type(EmbeddingModelType::Onnx); // Ensure consistent model type

        // Create an entry with an expired TTL
        let embedding = vec![1.0, 2.0, 3.0];
        let entry = CacheEntry {
            embedding: embedding.clone(),
            timestamp: 0, // Very old timestamp
            file_hash: 12345u64,
            model_type: EmbeddingModelType::Onnx, // Use Onnx
        };

        cache.entries.insert("test".to_string(), entry);

        // Try to get it via entries map - should still exist in the map
        let retrieved = cache.entries.get("test");
        assert!(retrieved.is_some()); // It's still in the map

        // Now use check_cache_and_get_hash which respects TTL
        let test_path = dir.path().join("test_file_for_ttl.txt"); // Need a dummy path
        std::fs::write(&test_path, "dummy content")?; // Create the file
        let check_result = cache.check_cache_and_get_hash("test", &test_path)?;
        assert!(matches!(check_result, CacheCheckResult::Miss(_))); // Should be a miss due to TTL

        // Add a fresh entry using insert_with_hash
        let test_path2 = dir.path().join("test_file_for_ttl2.txt"); // Define path first
        std::fs::write(&test_path2, "dummy content 2")?; // Create the file
        let file_hash2 = EmbeddingCache::get_file_hash(&test_path2)?; // Calculate actual hash
        cache.insert_with_hash("test2".to_string(), embedding.clone(), file_hash2)?;

        // Check the fresh entry using check_cache_and_get_hash
        let check_result2 = cache.check_cache_and_get_hash("test2", &test_path2)?;
        assert!(matches!(check_result2, CacheCheckResult::Hit(_))); // Should be a hit

        // Verify the embedding from the hit
        if let CacheCheckResult::Hit(retrieved_embedding) = check_result2 {
            assert_eq!(retrieved_embedding, embedding);
        } else {
            panic!("Expected CacheCheckResult::Hit");
        }

        Ok(())
    }

    #[test]
    fn test_cache_model_type() -> Result<()> {
        let dir = tempdir()?;
        let cache_path = dir.path().join("cache.json").to_string_lossy().to_string();

        let mut cache = EmbeddingCache::new(cache_path.clone())?;
        assert_eq!(cache.len(), 0);

        // Create a dummy file for hash checks
        let test_file_path = dir.path().join("test_file_model_type.txt");
        std::fs::write(&test_file_path, "content for model type test")?;

        // Insert an item with Onnx model type (was Fast)
        let embedding = vec![1.0, 2.0, 3.0];
        let file_hash = EmbeddingCache::get_file_hash(&test_file_path)?;
        cache.set_model_type(EmbeddingModelType::Onnx);
        cache.insert_with_hash("test".to_string(), embedding.clone(), file_hash)?;
        cache.save()?; // Save after insert

        // Reload cache and check the entry with Onnx type
        let mut cache_reloaded = EmbeddingCache::new(cache_path.clone())?;
        cache_reloaded.set_model_type(EmbeddingModelType::Onnx);
        let check_result_onnx = cache_reloaded.check_cache_and_get_hash("test", &test_file_path)?;
        assert!(matches!(check_result_onnx, CacheCheckResult::Hit(_)));

        // Change model type to simulate a different (hypothetical) model
        // We can't change *to* Fast, so let's just clear and re-insert with Onnx again
        // cache_reloaded.set_model_type(EmbeddingModelType::Hypothetical); // This doesn't exist

        // Let's test invalidation instead
        cache_reloaded.invalidate_different_model_types(); // Should do nothing as current is Onnx
        let check_result_onnx_still_hit = cache_reloaded.check_cache_and_get_hash("test", &test_file_path)?;
        assert!(matches!(check_result_onnx_still_hit, CacheCheckResult::Hit(_)));

        // If we could set a different type and invalidate, the entry would be removed.
        // Since we only have Onnx, this test is less meaningful now.
        // We can keep it simple by checking the default is Onnx.
        assert_eq!(cache.current_model_type, EmbeddingModelType::Onnx);
        assert_eq!(cache_reloaded.current_model_type, EmbeddingModelType::Onnx);

        Ok(())
    }
}
