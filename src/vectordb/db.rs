use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};
use walkdir::WalkDir;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::cache::EmbeddingCache;
use crate::vectordb::error::{Result, VectorDBError};
use crate::vectordb::hnsw::{HNSWIndex, HNSWConfig, HNSWStats};
use std::sync::{Arc, Mutex};
use rayon::prelude::*;
use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::atomic::AtomicBool;

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
            // Try to read the existing database file, but handle corruption gracefully
            match fs::read_to_string(&db_path) {
                Ok(contents) => {
                    match serde_json::from_str::<DBFile>(&contents) {
                        Ok(db_file) => (db_file.embeddings, db_file.hnsw_config),
                        Err(e) => {
                            // If JSON parsing fails, assume the file is corrupted
                            eprintln!("Warning: Database file appears to be corrupted: {}", e);
                            eprintln!("Creating a new empty database.");
                            // Remove corrupted file
                            let _ = fs::remove_file(&db_path);
                            (HashMap::new(), None)
                        }
                    }
                }
                Err(e) => {
                    // Handle file read errors
                    eprintln!("Warning: Couldn't read database file: {}", e);
                    eprintln!("Creating a new empty database.");
                    (HashMap::new(), None)
                }
            }
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
        
        // Try to create cache, but handle potential cache corruption
        let cache = match EmbeddingCache::new(cache_path.clone()) {
            Ok(cache) => cache,
            Err(e) => {
                eprintln!("Warning: Couldn't load cache: {}", e);
                eprintln!("Creating a new empty cache.");
                // Try to remove the corrupted cache file
                let _ = fs::remove_file(&cache_path);
                // Create a new empty cache
                EmbeddingCache::new(cache_path)?
            }
        };
        
        // Check for an HNSW index file
        let hnsw_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("hnsw_index.json");
            
        // Try to load the index from file, or build a new one if config exists
        let hnsw_index = if hnsw_path.exists() {
            match HNSWIndex::load_from_file(&hnsw_path) {
                Ok(index) => Some(index),
                Err(e) => {
                    // If loading fails, clean up and rebuild the index
                    eprintln!("Warning: Couldn't load HNSW index: {}", e);
                    eprintln!("Creating a new index or rebuilding from embeddings.");
                    // Try to remove corrupted file
                    let _ = fs::remove_file(&hnsw_path);
                    
                    // Rebuild the index if we have a configuration
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
        
        // Rebuild the index with the new config using parallel implementation
        if let Some(index) = &self.hnsw_index {
            let new_index = index.rebuild_with_config_parallel(new_config)?;
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
        
        // Collect all eligible files first
        let mut eligible_files = Vec::new();
        for entry in WalkDir::new(dir_path) {
            let entry = entry.map_err(|e| VectorDBError::DatabaseError(e.to_string()))?;
            if entry.file_type().is_file() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    let ext = ext.to_string_lossy().to_string();
                    if file_types.contains(&ext) {
                        eligible_files.push(path.to_path_buf());
                    }
                }
            }
        }
        
        let file_count = eligible_files.len();
        println!("Found {} files to index. Processing in parallel...", file_count);
        
        // Track if we were interrupted
        let mut was_interrupted = false;
        
        // Choose single-threaded or parallel indexing based on file count
        if file_count < 10 {
            // For small numbers of files, just process sequentially
            for file_path in eligible_files {
                self.index_file(&file_path)?;
                
                // Check for interruption
                unsafe {
                    if crate::cli::commands::INTERRUPT_RECEIVED {
                        was_interrupted = true;
                        println!("Interrupt received, saving progress...");
                        self.save()?;
                        break;
                    }
                }
            }
        } else {
            // Use parallel processing for larger file counts
            // Set up thread-local embedding model
            thread_local! {
                static EMBEDDING_MODEL: RefCell<Option<EmbeddingModel>> = RefCell::new(None);
            }
            
            // Shared resources with proper synchronization
            let embeddings = Arc::new(Mutex::new(HashMap::new()));
            let cache = Arc::new(Mutex::new(self.cache.clone()));
            let hnsw_index_option = self.hnsw_index.as_ref().map(|index| {
                let config = index.get_config();
                Arc::new(Mutex::new(HNSWIndex::new(config)))
            });
            
            // Track progress
            let processed_files = Arc::new(AtomicUsize::new(0));
            let save_triggered = Arc::new(AtomicBool::new(false));
            
            // Create a progress bar
            let progress_bar = ProgressBar::new(file_count as u64);
            progress_bar.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({eta})")
                .unwrap()
                .progress_chars("#>-"));
                
            // Periodically save progress to handle large indexing operations
            let save_interval = std::cmp::max(file_count / 10, 100); // Save after every 10% or 100 files, whichever is larger
            
            eligible_files.par_iter().for_each(|file_path| {
                // Skip if we've received an interrupt
                unsafe {
                    if crate::cli::commands::INTERRUPT_RECEIVED {
                        return;
                    }
                }
                
                // Get file path as string
                let file_path_str = file_path.to_string_lossy().to_string();
                
                // Check if file is already in cache
                let cached_embedding = cache.lock().unwrap().get(&file_path_str).map(|v| v.to_vec());
                if let Some(embedding) = cached_embedding {
                    embeddings.lock().unwrap().insert(file_path_str.clone(), embedding.clone());
                    
                    // Add to HNSW index if available
                    if let Some(ref index_lock) = hnsw_index_option {
                        index_lock.lock().unwrap().insert(embedding).ok();
                    }
                } else {
                    // Get or initialize thread-local embedding model
                    EMBEDDING_MODEL.with(|model_cell| {
                        let mut model_ref = model_cell.borrow_mut();
                        if model_ref.is_none() {
                            *model_ref = EmbeddingModel::new().ok();
                        }
                        
                        if let Some(model) = &*model_ref {
                            if let Ok(contents) = fs::read_to_string(file_path) {
                                if let Ok(embedding) = model.embed(&contents) {
                                    // Get file hash for cache
                                    if let Ok(file_hash) = EmbeddingCache::get_file_hash(file_path) {
                                        // Store in both cache and database
                                        let _ = cache.lock().unwrap().insert(file_path_str.clone(), embedding.clone(), file_hash);
                                        embeddings.lock().unwrap().insert(file_path_str, embedding.clone());
                                        
                                        // Add to HNSW index if available
                                        if let Some(index_lock) = &hnsw_index_option {
                                            index_lock.lock().unwrap().insert(embedding).ok();
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
                
                // Update progress
                let current = processed_files.fetch_add(1, Ordering::SeqCst) + 1;
                progress_bar.set_position(current as u64);
                
                // Check for interruption or trigger periodic save
                unsafe {
                    if crate::cli::commands::INTERRUPT_RECEIVED || current % save_interval == 0 {
                        save_triggered.store(true, Ordering::SeqCst);
                    }
                }
            });
            
            progress_bar.finish_with_message("Parallel indexing complete!");
            
            // Check if we were interrupted
            unsafe {
                was_interrupted = crate::cli::commands::INTERRUPT_RECEIVED;
            }
            
            // After parallel indexing is complete, update the main database with accumulated data
            self.embeddings = embeddings.lock().unwrap().clone();
            
            // Update the HNSW index if available
            if let Some(index_lock) = hnsw_index_option {
                self.hnsw_index = Some(index_lock.lock().unwrap().clone());
            }
            
            // Save the database
            println!("Saving database...");
            self.save()?;
            
            // Check if we should rebuild the HNSW index
            if !was_interrupted && self.hnsw_index.is_some() {
                println!("Optimizing HNSW index...");
                self.rebuild_hnsw_index()?;
            }
        }
        
        if was_interrupted {
            println!("Indexing was interrupted but your data has been saved safely.");
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
        
        // Create a temporary file first
        let temp_path = format!("{}.tmp", self.db_path);
        
        // Serialize to the temporary file first
        let contents = serde_json::to_string_pretty(&db_file)
            .map_err(VectorDBError::SerializationError)?;
        fs::write(&temp_path, contents)
            .map_err(|e| VectorDBError::FileWriteError {
                path: Path::new(&temp_path).to_path_buf(),
                source: e,
            })?;
            
        // Atomically rename the temporary file to the actual file
        // This ensures we don't end up with a partially written file if interrupted
        fs::rename(&temp_path, &self.db_path)
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
                
            // Use atomic write for HNSW index too
            let hnsw_temp_path = format!("{}.tmp", hnsw_path.to_string_lossy());
            index.save_to_file(Path::new(&hnsw_temp_path))?;
            fs::rename(&hnsw_temp_path, &hnsw_path)
                .map_err(|e| VectorDBError::FileWriteError {
                    path: hnsw_path.clone(),
                    source: e,
                })?;
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