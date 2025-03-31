use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use crate::vectordb::error::{Result, VectorDBError};
use tempfile::tempdir;

const CACHE_TTL: u64 = 3600; // 1 hour in seconds

#[derive(Serialize, Deserialize, Clone, Debug)]
struct CacheEntry {
    embedding: Vec<f32>,
    timestamp: u64,
    file_hash: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct CacheFile {
    entries: HashMap<String, CacheEntry>,
}

pub struct EmbeddingCache {
    entries: HashMap<String, CacheEntry>,
    cache_path: String,
    ttl: u64,
}

impl Clone for EmbeddingCache {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            cache_path: self.cache_path.clone(),
            ttl: self.ttl,
        }
    }
}

impl EmbeddingCache {
    pub fn new(cache_path: String) -> Result<Self> {
        Self::new_with_ttl(cache_path, CACHE_TTL)
    }

    fn new_with_ttl(cache_path: String, ttl: u64) -> Result<Self> {
        let entries = if Path::new(&cache_path).exists() {
            let contents = std::fs::read_to_string(&cache_path)
                .map_err(|e| VectorDBError::FileReadError {
                    path: Path::new(&cache_path).to_path_buf(),
                    source: e,
                })?;
            let cache_file: CacheFile = serde_json::from_str(&contents)
                .map_err(VectorDBError::SerializationError)?;
            cache_file.entries
        } else {
            HashMap::new()
        };

        Ok(Self {
            entries,
            cache_path,
            ttl,
        })
    }

    pub fn get(&self, file_path: &str) -> Option<&Vec<f32>> {
        if let Some(entry) = self.entries.get(file_path) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| VectorDBError::CacheError(e.to_string()))
                .ok()?
                .as_secs();
            
            if now - entry.timestamp < self.ttl {
                Some(&entry.embedding)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn insert(&mut self, file_path: String, embedding: Vec<f32>, file_hash: u64) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| VectorDBError::CacheError(e.to_string()))?
            .as_secs();

        let entry = CacheEntry {
            embedding,
            timestamp: now,
            file_hash,
        };

        self.entries.insert(file_path, entry);
        self.save()?;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.entries.clear();
        self.save()?;
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = Path::new(&self.cache_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VectorDBError::DirectoryCreationError {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
        }
        
        let cache_file = CacheFile {
            entries: self.entries.clone(),
        };
        
        let contents = serde_json::to_string_pretty(&cache_file)
            .map_err(VectorDBError::SerializationError)?;
        
        std::fs::write(&self.cache_path, contents)
            .map_err(|e| VectorDBError::FileWriteError {
                path: Path::new(&self.cache_path).to_path_buf(),
                source: e,
            })?;
        
        Ok(())
    }

    pub fn get_file_hash(path: &Path) -> Result<u64> {
        let metadata = std::fs::metadata(path)
            .map_err(|e| VectorDBError::MetadataError {
                path: path.to_path_buf(),
                source: e,
            })?;
            
        let modified = metadata.modified()
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_persistence() -> Result<()> {
        let dir = tempdir()?;
        let cache_path = dir.path().join("cache.json");
        
        // Create and populate cache
        {
            let mut cache = EmbeddingCache::new(cache_path.to_string_lossy().to_string())?;
            let embedding = vec![0.1, 0.2, 0.3];
            cache.insert("test".to_string(), embedding.clone(), 123)?;
        }
        
        // Create new cache instance and verify data persisted
        let cache = EmbeddingCache::new(cache_path.to_string_lossy().to_string())?;
        let cached = cache.get("test");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), &vec![0.1, 0.2, 0.3]);

        Ok(())
    }

    #[test]
    fn test_cache_clear() -> Result<()> {
        let dir = tempdir()?;
        let cache_path = dir.path().join("cache.json");
        let mut cache = EmbeddingCache::new(cache_path.to_string_lossy().to_string())?;

        // Add some entries
        cache.insert("test1".to_string(), vec![0.1], 1)?;
        cache.insert("test2".to_string(), vec![0.2], 2)?;
        assert_eq!(cache.len(), 2);

        // Clear cache
        cache.clear()?;
        assert!(cache.is_empty());
        
        // Verify persistence of empty cache
        let cache = EmbeddingCache::new(cache_path.to_string_lossy().to_string())?;
        assert!(cache.is_empty());

        Ok(())
    }
}