use std::collections::HashMap;
use std::path::Path;
use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

const CACHE_TTL: u64 = 3600; // 1 hour in seconds

#[derive(Serialize, Deserialize, Clone)]
struct CacheEntry {
    embedding: Vec<f32>,
    timestamp: u64,
    file_hash: u64,
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    entries: HashMap<String, CacheEntry>,
}

pub struct EmbeddingCache {
    entries: HashMap<String, CacheEntry>,
    cache_path: String,
}

impl EmbeddingCache {
    pub fn new(cache_path: String) -> Result<Self> {
        let entries = if Path::new(&cache_path).exists() {
            let contents = std::fs::read_to_string(&cache_path)?;
            let cache_file: CacheFile = serde_json::from_str(&contents)?;
            cache_file.entries
        } else {
            HashMap::new()
        };

        Ok(Self {
            entries,
            cache_path,
        })
    }

    pub fn get(&self, file_path: &str) -> Option<&Vec<f32>> {
        if let Some(entry) = self.entries.get(file_path) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            if now - entry.timestamp < CACHE_TTL {
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
            .unwrap()
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

    pub fn remove(&mut self, file_path: &str) -> Result<()> {
        self.entries.remove(file_path);
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

    fn save(&self) -> Result<()> {
        if let Some(parent) = Path::new(&self.cache_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let cache_file = CacheFile {
            entries: self.entries.clone(),
        };
        
        let contents = serde_json::to_string_pretty(&cache_file)?;
        std::fs::write(&self.cache_path, contents)?;
        Ok(())
    }

    pub fn get_file_hash(path: &Path) -> Result<u64> {
        let metadata = std::fs::metadata(path)?;
        let modified = metadata.modified()?
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        let size = metadata.len();
        
        // Combine modification time and size for a simple hash
        Ok(modified.wrapping_mul(31).wrapping_add(size as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_cache() -> Result<()> {
        let dir = tempdir()?;
        let cache_path = dir.path().join("cache.json");
        let mut cache = EmbeddingCache::new(cache_path.to_string_lossy().to_string())?;

        // Create a test file
        let test_file = dir.path().join("test.txt");
        let mut file = File::create(&test_file)?;
        file.write_all(b"test content")?;

        // Test file hash
        let hash = EmbeddingCache::get_file_hash(&test_file)?;
        assert!(hash > 0);

        // Test cache insertion and retrieval
        let embedding = vec![0.1, 0.2, 0.3];
        cache.insert(test_file.to_string_lossy().to_string(), embedding.clone(), hash)?;

        let cached = cache.get(&test_file.to_string_lossy().to_string());
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), &embedding);

        // Test cache removal
        cache.remove(&test_file.to_string_lossy().to_string())?;
        let cached = cache.get(&test_file.to_string_lossy().to_string());
        assert!(cached.is_none());

        Ok(())
    }
} 