use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};
use walkdir::WalkDir;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::cache::EmbeddingCache;
use crate::vectordb::error::{Result, VectorDBError};
use crate::vectordb::hnsw::{HNSWIndex, HNSWConfig, HNSWStats};
use rayon::prelude::*;

#[derive(Serialize, Deserialize)]
struct DBFile {
    embeddings: HashMap<String, Vec<f32>>,
    hnsw_config: Option<HNSWConfig>,
}

pub struct VectorDB {
    pub embeddings: HashMap<String, Vec<f32>>,
    db_path: String,
    cache: EmbeddingCache,
    hnsw_index: Option<HNSWIndex>,
}

// Implement Clone for VectorDB
impl Clone for VectorDB {
    fn clone(&self) -> Self {
        Self {
            embeddings: self.embeddings.clone(),
            db_path: self.db_path.clone(),
            cache: self.cache.clone(),
            hnsw_index: self.hnsw_index.clone(),
        }
    }
}

impl VectorDB {
    pub fn new(db_path: String) -> Result<Self> {
        let (embeddings, hnsw_config) = if Path::new(&db_path).exists() {
            let contents = fs::read_to_string(&db_path)
                .map_err(|e| VectorDBError::FileReadError {
                    path: Path::new(&db_path).to_path_buf(),
                    source: e,
                })?;
            let db_file: DBFile = serde_json::from_str(&contents)
                .map_err(VectorDBError::SerializationError)?;
            (db_file.embeddings, db_file.hnsw_config)
        } else {
            (HashMap::new(), None)
        };

        // Create cache in the same directory as the database
        let cache_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("cache.json")
            .to_string_lossy()
            .to_string();
        
        let cache = EmbeddingCache::new(cache_path)?;
        
        // Check for an HNSW index file
        let hnsw_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("hnsw_index.json");
            
        // Try to load the index from file, or build a new one if config exists
        let hnsw_index = if hnsw_path.exists() {
            match HNSWIndex::load_from_file(&hnsw_path) {
                Ok(index) => Some(index),
                Err(_) => {
                    // If loading fails, rebuild the index
                    hnsw_config.map(|config| {
                        let mut index = HNSWIndex::new(config);
                        // Rebuild the index from embeddings
                        for (_, embedding) in &embeddings {
                            let _ = index.insert(embedding.clone());
                        }
                        index
                    })
                }
            }
        } else {
            // No index file, build from scratch if config exists
            hnsw_config.map(|config| {
                let mut index = HNSWIndex::new(config);
                // Rebuild the index from embeddings
                for (_, embedding) in &embeddings {
                    let _ = index.insert(embedding.clone());
                }
                index
            })
        };

        Ok(Self {
            embeddings,
            db_path,
            cache,
            hnsw_index,
        })
    }

    pub fn index_file(&mut self, file_path: &Path) -> Result<()> {
        let file_path_str = file_path.to_string_lossy().to_string();
        
        // Check cache first
        if let Some(cached_embedding) = self.cache.get(&file_path_str) {
            self.embeddings.insert(file_path_str.clone(), cached_embedding.to_vec());
            
            // Add to HNSW index if available
            if let Some(index) = &mut self.hnsw_index {
                index.insert(cached_embedding.to_vec())?;
            }
            
            self.save()?;
            return Ok(());
        }

        // If not in cache, generate new embedding
        let model = EmbeddingModel::new()
            .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))?;
        let contents = fs::read_to_string(file_path)
            .map_err(|e| VectorDBError::FileReadError {
                path: file_path.to_path_buf(),
                source: e,
            })?;
        let embedding = model.embed(&contents)
            .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))?;
        
        // Get file hash for cache
        let file_hash = EmbeddingCache::get_file_hash(file_path)?;
        
        // Store in both cache and database
        self.cache.insert(file_path_str.clone(), embedding.clone(), file_hash)?;
        self.embeddings.insert(file_path_str, embedding.clone());
        
        // Add to HNSW index if available
        if let Some(index) = &mut self.hnsw_index {
            index.insert(embedding)?;
        }
        
        self.save()?;
        
        Ok(())
    }

    pub fn index_directory(&mut self, dir: &str, file_types: &[String]) -> Result<()> {
        let dir_path = Path::new(dir);
        
        for entry in WalkDir::new(dir_path) {
            let entry = entry.map_err(|e| VectorDBError::DatabaseError(e.to_string()))?;
            if entry.file_type().is_file() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    let ext = ext.to_string_lossy().to_string();
                    if file_types.contains(&ext) {
                        self.index_file(path)?;
                    }
                }
            }
        }
        
        Ok(())
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = Path::new(&self.db_path).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| VectorDBError::DirectoryCreationError {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
        }
        
        // Get HNSW config if available
        let hnsw_config = self.hnsw_index.as_ref().map(|index| index.get_config());
        
        let db_file = DBFile {
            embeddings: self.embeddings.clone(),
            hnsw_config,
        };
        
        let contents = serde_json::to_string_pretty(&db_file)
            .map_err(VectorDBError::SerializationError)?;
        fs::write(&self.db_path, contents)
            .map_err(|e| VectorDBError::FileWriteError {
                path: Path::new(&self.db_path).to_path_buf(),
                source: e,
            })?;
            
        // Save the HNSW index to its own file if available
        if let Some(index) = &self.hnsw_index {
            let hnsw_path = Path::new(&self.db_path)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
            index.save_to_file(&hnsw_path)?;
        }
        
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.embeddings.clear();
        self.cache.clear()?;
        self.hnsw_index = None;
        
        // Remove the database file
        if Path::new(&self.db_path).exists() {
            fs::remove_file(&self.db_path)
                .map_err(|e| VectorDBError::FileWriteError {
                    path: Path::new(&self.db_path).to_path_buf(),
                    source: e,
                })?;
        }
        
        // Remove the HNSW index file
        let hnsw_path = Path::new(&self.db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("hnsw_index.json");
            
        if hnsw_path.exists() {
            fs::remove_file(&hnsw_path)
                .map_err(|e| VectorDBError::FileWriteError {
                    path: hnsw_path.clone(),
                    source: e,
                })?;
        }
        
        Ok(())
    }

    pub fn stats(&self) -> DBStats {
        let hnsw_stats = self.hnsw_index.as_ref().map(|index| index.stats());
        
        DBStats {
            indexed_files: self.embeddings.len(),
            embedding_dimension: self.embeddings.values().next().map(|v| v.len()).unwrap_or(0),
            db_path: self.db_path.clone(),
            cached_files: self.cache.len(),
            hnsw_stats,
        }
    }
    
    /// Build or rebuild the HNSW index
    pub fn build_hnsw_index(&mut self, config: Option<HNSWConfig>) -> Result<()> {
        let config = config.unwrap_or_else(HNSWConfig::default);
        let mut index = HNSWIndex::new(config);
        
        // Add all embeddings to the index
        for (_, embedding) in &self.embeddings {
            index.insert(embedding.clone())?;
        }
        
        self.hnsw_index = Some(index);
        self.save()?;
        
        // Save the index to its own file
        if let Some(index) = &self.hnsw_index {
            let hnsw_path = Path::new(&self.db_path)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
            index.save_to_file(&hnsw_path)?;
        }
        
        Ok(())
    }
    
    /// Build the HNSW index in parallel
    pub fn build_hnsw_index_parallel(&mut self, config: Option<HNSWConfig>) -> Result<()> {
        let config = config.unwrap_or_else(HNSWConfig::default);
        
        // Convert embeddings to a vector for parallel processing
        let embeddings: Vec<Vec<f32>> = self.embeddings.values().cloned().collect();
        
        // Create index after parallel processing
        let mut index = HNSWIndex::new(config);
        
        // Process embeddings sequentially as a fallback
        for embedding in embeddings {
            index.insert(embedding)?;
        }
        
        self.hnsw_index = Some(index);
        self.save()?;
        
        // Save the index to its own file
        if let Some(index) = &self.hnsw_index {
            let hnsw_path = Path::new(&self.db_path)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
            index.save_to_file(&hnsw_path)?;
        }
        
        Ok(())
    }
    
    /// Enable or disable the HNSW index
    pub fn set_hnsw_enabled(&mut self, enabled: bool) -> Result<()> {
        if enabled && self.hnsw_index.is_none() {
            self.build_hnsw_index(None)?;
        } else if !enabled {
            self.hnsw_index = None;
            self.save()?;
        }
        
        Ok(())
    }
    
    /// Get nearest vectors using HNSW index
    pub fn nearest_vectors(&mut self, query_embedding: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        if let Some(index) = &mut self.hnsw_index {
            // Use HNSW for search
            let results = index.search(query_embedding, k, 100)?;
            
            // Map index results back to file paths
            let mut nearest = Vec::with_capacity(results.len());
            
            // Create a mapping of vector to file path
            let file_paths: Vec<String> = self.embeddings.keys().cloned().collect();
            let vectors: Vec<Vec<f32>> = self.embeddings.values().cloned().collect();
            
            for (idx, dist) in results {
                // Convert distance to similarity score (1 - distance)
                let similarity = 1.0 - dist;
                
                // Find the corresponding file path
                if idx < file_paths.len() {
                    nearest.push((file_paths[idx].clone(), similarity));
                }
            }
            
            Ok(nearest)
        } else {
            // Fallback to linear search
            let mut nearest = Vec::new();
            
            for (file_path, embedding) in &self.embeddings {
                let similarity = cosine_similarity(query_embedding, embedding);
                nearest.push((file_path.clone(), similarity));
            }
            
            // Sort by similarity (descending)
            nearest.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            nearest.truncate(k);
            
            Ok(nearest)
        }
    }
}

pub struct DBStats {
    pub indexed_files: usize,
    pub embedding_dimension: usize,
    pub db_path: String,
    pub cached_files: usize,
    pub hnsw_stats: Option<HNSWStats>,
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot_product / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_vectordb() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("db.json");
        let mut db = VectorDB::new(db_path.to_string_lossy().to_string())?;

        // Create a test file
        let test_file = dir.path().join("test.txt");
        let mut file = File::create(&test_file)?;
        file.write_all(b"test content")?;

        // Test indexing
        db.index_file(&test_file)?;

        // Test stats
        let stats = db.stats();
        assert_eq!(stats.indexed_files, 1);
        assert!(stats.embedding_dimension > 0);
        assert!(stats.cached_files > 0);

        // Test clear
        db.clear()?;
        assert_eq!(db.embeddings.len(), 0);

        Ok(())
    }
} 