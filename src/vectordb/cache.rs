use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use serde::{Serialize, Deserialize};
use std::time::Instant;
use std::fs;
use crate::vectordb::error::{Result, VectorDBError};
use std::io::{Read, Seek, SeekFrom};
use std::convert::TryInto;
use crate::vectordb::embedding::EmbeddingModelType;

const CACHE_TTL: u64 = 3600; // 1 hour in seconds

#[derive(Serialize, Deserialize, Clone, Debug)]
struct CacheEntry {
    embedding: Vec<f32>,
    timestamp: u64,
    file_hash: u64,
    model_type: EmbeddingModelType,
}

#[derive(Serialize, Deserialize, Debug)]
struct CacheFile {
    entries: HashMap<String, CacheEntry>,
}

pub struct EmbeddingCache {
    entries: HashMap<String, CacheEntry>,
    cache_path: String,
    ttl: u64,
    last_cleaned: Instant,
    current_model_type: EmbeddingModelType,
}

impl Clone for EmbeddingCache {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            cache_path: self.cache_path.clone(),
            ttl: self.ttl,
            last_cleaned: self.last_cleaned,
            current_model_type: self.current_model_type.clone(),
        }
    }
}

impl EmbeddingCache {
    pub fn new(cache_path: String) -> Result<Self> {
        Self::new_with_ttl(cache_path, CACHE_TTL)
    }

    fn new_with_ttl(cache_path: String, ttl: u64) -> Result<Self> {
        let entries = if Path::new(&cache_path).exists() {
            match std::fs::read_to_string(&cache_path) {
                Ok(contents) => {
                    match serde_json::from_str::<CacheFile>(&contents) {
                        Ok(cache_file) => cache_file.entries,
                        Err(e) => {
                            // Handle corrupted cache file
                            eprintln!("Warning: Cache file appears corrupted: {}", e);
                            // Don't return an error, just start with an empty cache
                            HashMap::new()
                        }
                    }
                }
                Err(e) => {
                    // Handle file reading error
                    eprintln!("Warning: Couldn't read cache file: {}", e);
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        Ok(Self {
            entries,
            cache_path,
            ttl,
            last_cleaned: Instant::now(),
            current_model_type: EmbeddingModelType::Basic,
        })
    }
    
    /// Set the current model type
    pub fn set_model_type(&mut self, model_type: EmbeddingModelType) {
        self.current_model_type = model_type;
    }
    
    /// Get the current model type
    pub fn model_type(&self) -> &EmbeddingModelType {
        &self.current_model_type
    }

    pub fn get(&self, file_path: &str) -> Option<&Vec<f32>> {
        if let Some(entry) = self.entries.get(file_path) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| VectorDBError::CacheError(e.to_string()))
                .ok()?
                .as_secs();
            
            // Check both TTL and model type match
            if now - entry.timestamp < self.ttl && entry.model_type == self.current_model_type {
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
            model_type: self.current_model_type.clone(),
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

    pub fn save(&self) -> Result<()> {
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
        
        // Create a temporary file first
        let temp_path = format!("{}.tmp", self.cache_path);
        
        // Write to temporary file first
        let contents = serde_json::to_string_pretty(&cache_file)
            .map_err(VectorDBError::SerializationError)?;
        std::fs::write(&temp_path, contents)
            .map_err(|e| VectorDBError::FileWriteError {
                path: Path::new(&temp_path).to_path_buf(),
                source: e,
            })?;
            
        // Atomically rename the temporary file to the actual file
        std::fs::rename(&temp_path, &self.cache_path)
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

    pub fn clean(&mut self) {
        self.entries.retain(|_, entry| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| VectorDBError::CacheError(e.to_string()))
                .ok()
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            
            // Only keep entries that are still valid and match the current model type
            now - entry.timestamp < self.ttl && entry.model_type == self.current_model_type
        });
        self.last_cleaned = Instant::now();
    }
    
    /// Invalidate cache entries that don't match the current model type
    pub fn invalidate_different_model_types(&mut self) {
        self.entries.retain(|_, entry| {
            entry.model_type == self.current_model_type
        });
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
        cache.insert("test".to_string(), embedding.clone(), file_hash)?;
        assert_eq!(cache.len(), 1);
        
        // Get the item
        let retrieved = cache.get("test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), &embedding);
        
        // Check persistence
        let cache2 = EmbeddingCache::new(cache_path)?;
        let retrieved2 = cache2.get("test");
        assert!(retrieved2.is_some());
        assert_eq!(retrieved2.unwrap(), &embedding);
        
        Ok(())
    }
    
    #[test]
    fn test_cache_ttl() -> Result<()> {
        let dir = tempdir()?;
        let cache_path = dir.path().join("cache.json").to_string_lossy().to_string();
        
        let mut cache = EmbeddingCache::new(cache_path)?;
        
        // Create an entry with an expired TTL
        let embedding = vec![1.0, 2.0, 3.0];
        let entry = CacheEntry {
            embedding: embedding.clone(),
            timestamp: 0, // Very old timestamp
            file_hash: 12345u64,
            model_type: EmbeddingModelType::Basic,
        };
        
        cache.entries.insert("test".to_string(), entry);
        
        // Try to get it - should be expired
        let retrieved = cache.get("test");
        assert!(retrieved.is_none());
        
        // Add a fresh entry
        let entry2 = CacheEntry {
            embedding: embedding.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            file_hash: 12345u64,
            model_type: EmbeddingModelType::Basic,
        };
        
        cache.entries.insert("test2".to_string(), entry2);
        
        // Should be retrievable
        let retrieved = cache.get("test2");
        assert!(retrieved.is_some());
        
        Ok(())
    }
    
    #[test]
    fn test_cache_model_type() -> Result<()> {
        let dir = tempdir()?;
        let cache_path = dir.path().join("cache.json").to_string_lossy().to_string();
        
        let mut cache = EmbeddingCache::new(cache_path.clone())?;
        assert_eq!(cache.len(), 0);
        
        // Insert an item with Basic model type
        let embedding = vec![1.0, 2.0, 3.0];
        let file_hash = 12345u64;
        cache.set_model_type(EmbeddingModelType::Basic);
        cache.insert("test".to_string(), embedding.clone(), file_hash)?;
        
        // Change model type to ONNX
        cache.set_model_type(EmbeddingModelType::ONNX);
        
        // Get the item - should be None because model type doesn't match
        let retrieved = cache.get("test");
        assert!(retrieved.is_none());
        
        // Insert a new item with ONNX model type
        let embedding2 = vec![4.0, 5.0, 6.0];
        cache.insert("test2".to_string(), embedding2.clone(), file_hash)?;
        
        // Get the ONNX item - should exist
        let retrieved = cache.get("test2");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), &embedding2);
        
        Ok(())
    }
}