use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};
use walkdir::WalkDir;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::cache::EmbeddingCache;
use crate::vectordb::error::{Result, VectorDBError};
use crate::vectordb::hnsw::{HNSWIndex, HNSWConfig, HNSWStats, LayerStats};

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

    /// Set HNSW configuration - creates a new index if config is provided, removes it if None
    pub fn set_hnsw_config(&mut self, config: Option<HNSWConfig>) {
        if let Some(config) = config {
            let mut current_config = config;
            
            // If we have embeddings, use their count to optimize the number of layers
            if !self.embeddings.is_empty() {
                // Calculate optimal layer count based on dataset size
                let dataset_size = self.embeddings.len();
                let optimal_layers = HNSWConfig::calculate_optimal_layers(dataset_size);
                
                // Override the num_layers in the provided config
                current_config.num_layers = optimal_layers;
            }
            
            // Create a new index with the optimized config
            let mut index = HNSWIndex::new(current_config);
            
            // Rebuild the index from existing embeddings if any
            for (_, embedding) in &self.embeddings {
                let _ = index.insert(embedding.clone());
            }
            
            self.hnsw_index = Some(index);
        } else {
            self.hnsw_index = None;
        }
    }

    /// Rebuild the HNSW index with optimized configuration
    pub fn rebuild_hnsw_index(&mut self) -> Result<()> {
        if self.hnsw_index.is_none() {
            return Ok(());  // Nothing to do if no index exists
        }
        
        // Get current config
        let current_config = self.hnsw_index.as_ref().unwrap().get_config();
        
        // Calculate optimal layer count
        let dataset_size = self.embeddings.len();
        let optimal_layers = HNSWConfig::calculate_optimal_layers(dataset_size);
        
        // Check if we need to rebuild
        if current_config.num_layers == optimal_layers {
            return Ok(());  // No need to rebuild if layer count is already optimal
        }
        
        // Create a new config with the optimal layer count
        let mut new_config = current_config.clone();
        new_config.num_layers = optimal_layers;
        
        // Rebuild the index with the new config
        if let Some(index) = &self.hnsw_index {
            let new_index = index.rebuild_with_config(new_config)?;
            self.hnsw_index = Some(new_index);
            
            // Save the updated index
            self.save()?;
        }
        
        Ok(())
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
        
        // After indexing is complete, rebuild the HNSW index with optimized layers if needed
        if self.hnsw_index.is_some() {
            self.rebuild_hnsw_index()?;
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

    /// Get vector database stats
    pub fn stats(&self) -> DBStats {
        // Get HNSW stats directly from the index if available
        let hnsw_stats = self.hnsw_index.as_ref().map(|index| index.stats());
        
        DBStats {
            indexed_files: self.embeddings.len(),
            embedding_dimension: self.embeddings.values().next().map(|v| v.len()).unwrap_or(0),
            db_path: self.db_path.clone(),
            cached_files: self.cache.len(),
            hnsw_stats,
        }
    }
    
    /// Nearest vectors using HNSW index (if available)
    pub fn nearest_vectors(&mut self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        if let Some(ref mut index) = self.hnsw_index {
            // Use HNSW for efficient search
            let ef = 100; // Default search ef parameter
            let results = index.search(query, k, ef)?;
            
            // Convert node IDs to file paths
            let mut nearest = Vec::new();
            for (node_id, dist) in results {
                if let Some(file_path) = self.get_file_path(node_id) {
                    let file_path = file_path.clone();
                    let similarity = 1.0 - dist; // Convert distance to similarity
                    nearest.push((file_path, similarity));
                }
            }
            
            Ok(nearest)
        } else {
            // Fall back to brute force search
            let mut results: Vec<_> = self.embeddings.iter()
                .map(|(path, embedding)| {
                    let distance = Self::cosine_distance(embedding, query);
                    let similarity = 1.0 - distance;
                    (path.clone(), similarity)
                })
                .collect();
            
            // Sort by similarity (highest first)
            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            results.truncate(k);
            
            Ok(results)
        }
    }
    
    /// Calculate cosine distance between two vectors (for search)
    fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if norm_a > 0.0 && norm_b > 0.0 {
            1.0 - (dot_product / (norm_a * norm_b))
        } else {
            1.0 // Maximum distance if either vector is zero
        }
    }
    
    fn get_file_path(&self, node_id: usize) -> Option<&String> {
        if node_id < self.embeddings.len() {
            let file_path = self.embeddings.keys().nth(node_id);
            file_path
        } else {
            None
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
    use std::fs;
    use tempfile::tempdir;
    use std::io::Write;

    #[test]
    fn test_vectordb() -> Result<()> {
        // Create a temporary directory
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db").to_string_lossy().to_string();
        
        // Create a new database
        let mut db = VectorDB::new(db_path)?;
        
        // Test basic operations
        // ...
        
        Ok(())
    }
    
    #[test]
    fn test_optimal_layer_count() -> Result<()> {
        // Create a temporary directory
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db").to_string_lossy().to_string();
        
        // Create test files with different content
        let file_count = 20;
        for i in 0..file_count {
            let test_file = temp_dir.path().join(format!("test_{}.txt", i));
            let mut file = fs::File::create(&test_file)?;
            writeln!(file, "Test file content {}", i)?;
        }
        
        // Create a VectorDB with HNSW enabled
        let mut db = VectorDB::new(db_path)?;
        db.set_hnsw_config(Some(HNSWConfig::default()));
        
        // Index a directory
        db.index_directory(temp_dir.path().to_str().unwrap(), &["txt".to_string()])?;
        
        // Check if the HNSW index has the optimal number of layers
        if let Some(stats) = db.stats().hnsw_stats {
            // For 20 documents, optimal layer count should be log2(20) = 5 (rounded up)
            assert_eq!(stats.layers, 5);
            
            // Check if actual layer usage matches expectations
            let mut highest_populated_layer = 0;
            for (i, layer_stat) in stats.layer_stats.iter().enumerate() {
                if layer_stat.nodes > 0 {
                    highest_populated_layer = i;
                }
            }
            
            // There should be far fewer populated layers than the maximum 16
            assert!(highest_populated_layer < 16);
            
            // And the highest populated layer should not exceed our expected optimal count
            assert!(highest_populated_layer < 6);
        } else {
            assert!(false, "HNSW index should be present");
        }
        
        Ok(())
    }
} 