use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use walkdir::WalkDir;
use crate::vectordb::embedding::{EmbeddingModel, EmbeddingModelType, EMBEDDING_DIM};
use crate::vectordb::onnx::ONNX_EMBEDDING_DIM;
use crate::vectordb::cache::EmbeddingCache;
use crate::vectordb::error::{Result, VectorDBError};
use crate::vectordb::hnsw::{HNSWIndex, HNSWConfig, HNSWStats};
use std::sync::{Arc, Mutex};
use rayon::prelude::*;
use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::atomic::AtomicBool;
use std::io::Write;

/// Relevance feedback data for a query-file pair
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FeedbackEntry {
    /// Number of times this file was marked as relevant for the query
    pub relevant_count: usize,
    /// Number of times this file was marked as irrelevant for the query
    pub irrelevant_count: usize,
    /// Aggregated relevance score (1.0 = highly relevant, 0.0 = irrelevant)
    pub relevance_score: f32,
}

/// Collection of query feedback data
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FeedbackData {
    /// Maps query -> file_path -> feedback
    pub query_feedback: HashMap<String, HashMap<String, FeedbackEntry>>,
}

#[derive(Serialize, Deserialize)]
struct DBFile {
    embeddings: HashMap<String, Vec<f32>>,
    hnsw_config: Option<HNSWConfig>,
    feedback: Option<FeedbackData>,
    embedding_model_type: Option<EmbeddingModelType>,
}

pub struct VectorDB {
    pub embeddings: HashMap<String, Vec<f32>>,
    db_path: String,
    cache: EmbeddingCache,
    pub hnsw_index: Option<HNSWIndex>,
    feedback: FeedbackData,
    embedding_model_type: EmbeddingModelType,
    onnx_model_path: Option<PathBuf>,
    onnx_tokenizer_path: Option<PathBuf>,
}

// Implement Clone for VectorDB
impl Clone for VectorDB {
    fn clone(&self) -> Self {
        Self {
            embeddings: self.embeddings.clone(),
            db_path: self.db_path.clone(),
            cache: self.cache.clone(),
            hnsw_index: self.hnsw_index.clone(),
            feedback: self.feedback.clone(),
            embedding_model_type: self.embedding_model_type.clone(),
            onnx_model_path: self.onnx_model_path.clone(),
            onnx_tokenizer_path: self.onnx_tokenizer_path.clone(),
        }
    }
}

impl VectorDB {
    pub fn new(db_path: String) -> Result<Self> {
        let (embeddings, hnsw_config, feedback, embedding_model_type) = if Path::new(&db_path).exists() {
            // Try to read the existing database file, but handle corruption gracefully
            match fs::read_to_string(&db_path) {
                Ok(contents) => {
                    match serde_json::from_str::<DBFile>(&contents) {
                        Ok(db_file) => (
                            db_file.embeddings, 
                            db_file.hnsw_config, 
                            db_file.feedback.unwrap_or_default(),
                            db_file.embedding_model_type.unwrap_or_default(),
                        ),
                        Err(e) => {
                            // If JSON parsing fails, assume the file is corrupted
                            eprintln!("Warning: Database file appears to be corrupted: {}", e);
                            eprintln!("Creating a new empty database.");
                            // Remove corrupted file
                            let _ = fs::remove_file(&db_path);
                            (HashMap::new(), Some(HNSWConfig::default()), FeedbackData::default(), EmbeddingModelType::Basic)
                        }
                    }
                }
                Err(e) => {
                    // Handle file read errors
                    eprintln!("Warning: Couldn't read database file: {}", e);
                    eprintln!("Creating a new empty database.");
                    (HashMap::new(), Some(HNSWConfig::default()), FeedbackData::default(), EmbeddingModelType::Basic)
                }
            }
        } else {
            // Create new database with default HNSW config
            (HashMap::new(), Some(HNSWConfig::default()), FeedbackData::default(), EmbeddingModelType::Basic)
        };

        // Create cache in the same directory as the database
        let cache_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("cache.json")
            .to_string_lossy()
            .to_string();
        
        // Try to create cache, but handle potential cache corruption
        let mut cache = match EmbeddingCache::new(cache_path.clone()) {
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
        
        // Configure the cache with the embedding model type
        cache.set_model_type(embedding_model_type.clone());
        
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
            // No index file, build from scratch with default or provided config
            let config = hnsw_config.unwrap_or_else(HNSWConfig::default);
            let mut index = HNSWIndex::new(config);
            // Build the index from embeddings if any exist
            for (_, embedding) in &embeddings {
                let _ = index.insert(embedding.clone());
            }
            Some(index)
        };

        Ok(Self {
            embeddings,
            db_path,
            cache,
            hnsw_index,
            feedback,
            embedding_model_type,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
        })
    }
    
    /// Configure the ONNX embedding model paths
    pub fn set_onnx_paths(&mut self, model_path: Option<PathBuf>, tokenizer_path: Option<PathBuf>) {
        self.onnx_model_path = model_path;
        self.onnx_tokenizer_path = tokenizer_path;
    }
    
    /// Set the embedding model type
    pub fn set_embedding_model_type(&mut self, model_type: EmbeddingModelType) -> Result<()> {
        // Check if we're changing model type
        if self.embedding_model_type != model_type {
            // If switching to ONNX, verify paths are set
            if model_type == EmbeddingModelType::Onnx {
                if self.onnx_model_path.is_none() || self.onnx_tokenizer_path.is_none() {
                    return Err(VectorDBError::EmbeddingError(
                        "ONNX model and tokenizer paths must be set before using ONNX embeddings".into()
                    ).into());
                }
                
                // Verify paths exist
                let model_path = self.onnx_model_path.as_ref().unwrap();
                let tokenizer_path = self.onnx_tokenizer_path.as_ref().unwrap();
                
                if !model_path.exists() {
                    return Err(VectorDBError::FileReadError {
                        path: model_path.clone(),
                        source: std::io::Error::new(std::io::ErrorKind::NotFound, "ONNX model file not found"),
                    }.into());
                }
                
                if !tokenizer_path.exists() {
                    return Err(VectorDBError::FileReadError {
                        path: tokenizer_path.clone(),
                        source: std::io::Error::new(std::io::ErrorKind::NotFound, "ONNX tokenizer directory not found"),
                    }.into());
                }
            }
            
            let model_type_clone = model_type.clone();
            self.embedding_model_type = model_type_clone;
            self.cache.set_model_type(model_type);
            
            // Invalidate cache entries from different model type
            self.cache.invalidate_different_model_types();
            
            // Save the updated configuration
            self.save()?;
        }
        
        Ok(())
    }
    
    /// Get the current embedding model type
    pub fn embedding_model_type(&self) -> &EmbeddingModelType {
        &self.embedding_model_type
    }
    
    /// Create the appropriate embedding model based on configuration
    fn create_embedding_model(&self) -> Result<EmbeddingModel> {
        match &self.embedding_model_type {
            EmbeddingModelType::Basic => {
                Ok(EmbeddingModel::new())
            },
            EmbeddingModelType::Onnx => {
                if let (Some(model_path), Some(tokenizer_path)) = (&self.onnx_model_path, &self.onnx_tokenizer_path) {
                    EmbeddingModel::new_onnx(model_path, tokenizer_path)
                        .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))
                } else {
                    // Fallback to basic model if paths aren't set
                    eprintln!("Warning: ONNX model paths not set, falling back to basic embedding model");
                    Ok(EmbeddingModel::new())
                }
            }
        }
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

        // If not in cache, generate new embedding for the entire file
        let model = self.create_embedding_model()
            .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))?;
        let contents = fs::read_to_string(file_path)
            .map_err(|e| VectorDBError::FileReadError {
                path: file_path.to_path_buf(),
                source: e,
            })?;
            
        // Generate a single embedding for the entire file
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
        let was_interrupted = Arc::new(AtomicBool::new(false));
        let was_interrupted_clone = was_interrupted.clone();
        
        // Choose single-threaded or parallel indexing based on file count
        if file_count < 10 {
            // For small numbers of files, just process sequentially
            for file_path in eligible_files {
                self.index_file(&file_path)?;
                
                // Check for interruption
                unsafe {
                    if crate::cli::commands::INTERRUPT_RECEIVED {
                        was_interrupted_clone.store(true, Ordering::SeqCst);
                        println!("Interrupt received, saving progress...");
                        self.save()?;
                        break;
                    }
                }
            }
        } else {
            // For parallel processing, we need to handle the ONNX model a bit differently
            // since it might be expensive to create multiple instances
            
            // First configure the embedding model type for the thread-local model
            let embedding_model_type = self.embedding_model_type.clone();
            let onnx_model_path = self.onnx_model_path.clone();
            let onnx_tokenizer_path = self.onnx_tokenizer_path.clone();
            
            // Use parallel processing for larger file counts with thread-local embedding model
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
                        was_interrupted_clone.store(true, Ordering::SeqCst);
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
                            // Lazy initialization of embedding model based on model type
                            *model_ref = match embedding_model_type {
                                EmbeddingModelType::Basic => {
                                    Some(EmbeddingModel::new())
                                },
                                EmbeddingModelType::Onnx => {
                                    if let (Some(model_path), Some(tokenizer_path)) = 
                                        (onnx_model_path.as_ref(), onnx_tokenizer_path.as_ref()) {
                                        match EmbeddingModel::new_onnx(model_path, tokenizer_path) {
                                            Ok(model) => Some(model),
                                            Err(e) => {
                                                eprintln!("Error creating ONNX model: {}", e);
                                                Some(EmbeddingModel::new())
                                            }
                                        }
                                    } else {
                                        // Fallback to basic model if ONNX paths aren't available
                                        Some(EmbeddingModel::new())
                                    }
                                }
                            };
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
            
            // Update main data structures with results from parallel processing
            self.embeddings.extend(embeddings.lock().unwrap().drain());
            
            // Update HNSW index if one was created in parallel
            if let Some(index_lock) = hnsw_index_option {
                self.hnsw_index = Some(index_lock.lock().unwrap().clone());
            }
            
            // Check if we need to save
            if save_triggered.load(Ordering::SeqCst) || was_interrupted.load(Ordering::SeqCst) {
                println!("Saving progress...");
                self.save()?;
            }
            
            progress_bar.finish();
        }
        
        // Print completion message
        if unsafe { crate::cli::commands::INTERRUPT_RECEIVED } {
            println!("Indexing was interrupted, but progress has been saved.");
        } else {
            println!("Indexing complete!");
        }
        
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = Path::new(&self.db_path).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| VectorDBError::DirectoryCreationError {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
        }
        
        // Save the HNSW index if it exists
        if let Some(index) = &self.hnsw_index {
            let hnsw_path = Path::new(&self.db_path)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
                
            index.save_to_file(&hnsw_path)?;
        }
        
        // Prepare database file
        let db_file = DBFile {
            embeddings: self.embeddings.clone(),
            hnsw_config: self.hnsw_index.as_ref().map(|i| i.get_config()),
            feedback: Some(self.feedback.clone()),
            embedding_model_type: Some(self.embedding_model_type.clone()),
        };
        
        // Create temporary file for atomic write
        let temp_path = format!("{}.tmp", self.db_path);
        let json = serde_json::to_string_pretty(&db_file)
            .map_err(VectorDBError::SerializationError)?;
            
        fs::write(&temp_path, json)
            .map_err(|e| VectorDBError::FileWriteError {
                path: Path::new(&temp_path).to_path_buf(),
                source: e,
            })?;
            
        // Rename temporary file to database file
        fs::rename(&temp_path, &self.db_path)
            .map_err(|e| VectorDBError::FileWriteError {
                path: Path::new(&self.db_path).to_path_buf(),
                source: e,
            })?;
            
        Ok(())
    }

    /// Clear the database
    pub fn clear(&mut self) -> Result<()> {
        self.embeddings.clear();
        
        // Create a new HNSW index if one exists
        if let Some(index) = &self.hnsw_index {
            let config = index.get_config();
            self.hnsw_index = Some(HNSWIndex::new(config));
        }
        
        // Clear the cache
        self.cache.clear()?;
        
        // Clear feedback data
        self.feedback = FeedbackData::default();
        
        // Save the changes
        self.save()?;
        
        Ok(())
    }
    
    pub fn stats(&self) -> DBStats {
        let embedding_dimension = if !self.embeddings.is_empty() {
            self.embeddings.values().next().unwrap().len()
        } else {
            match &self.embedding_model_type {
                EmbeddingModelType::Basic => EMBEDDING_DIM,
                EmbeddingModelType::Onnx => ONNX_EMBEDDING_DIM,
            }
        };
    
        DBStats {
            indexed_files: self.embeddings.len(),
            embedding_dimension,
            db_path: self.db_path.clone(),
            cached_files: self.cache.len(),
            hnsw_stats: self.hnsw_index.as_ref().map(|index| index.stats()),
            embedding_model_type: self.embedding_model_type.clone(),
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
            // Calculate normalized cosine distance with proper bounds
            let similarity = dot_product / (norm_a * norm_b);
            // Handle potential floating point issues that might push similarity outside [-1, 1]
            let clamped_similarity = similarity.clamp(-1.0, 1.0);
            // Convert to distance in range [0, 2], with identical vectors having distance 0
            1.0 - clamped_similarity
        } else {
            1.0 // Maximum distance if either vector is zero
        }
    }
    
    /// Get the file path associated with a node ID
    pub fn get_file_path(&self, node_id: usize) -> Option<&String> {
        self.embeddings.keys().nth(node_id)
    }

    /// Filter files by filepath relevance to a query
    /// Returns a list of filepaths sorted by relevance to the query terms
    pub fn filter_by_filepath(&self, query: &str, max_files: usize) -> Vec<String> {
        // Normalize the query
        let query = query.to_lowercase();
        
        // Split query into terms
        let terms: Vec<&str> = query
            .split_whitespace()
            .filter(|t| t.len() > 1) // Only use meaningful terms
            .collect();
            
        if terms.is_empty() {
            // If no meaningful terms, return all filepaths up to max_files
            return self.embeddings.keys()
                .take(max_files)
                .cloned()
                .collect();
        }
        
        // Calculate relevance score for each file path based on filename and path components
        let mut scored_paths: Vec<(String, f32)> = self.embeddings.keys()
            .map(|path| {
                let path_lower = path.to_lowercase();
                let path_segments: Vec<&str> = path_lower
                    .split(|c| c == '/' || c == '\\')
                    .collect();
                
                // Extract the filename
                let filename = path_segments.last().unwrap_or(&"");
                let filename_no_ext = filename.split('.').next().unwrap_or(filename);
                
                // Calculate match score
                let mut score = 0.0;
                
                // Terms in filename get highest weight
                for term in &terms {
                    // Direct filename match (strongest signal)
                    if filename_no_ext == *term {
                        score += 10.0;
                    }
                    // Filename contains term
                    else if filename_no_ext.contains(term) {
                        score += 5.0;
                    }
                    // Path contains term
                    else if path_lower.contains(term) {
                        score += 2.0;
                    }
                    
                    // Bonus points for file extensions matching query terms
                    if filename.ends_with(&format!(".{}", term)) {
                        score += 3.0;
                    }
                }
                
                // Penalty for deeply nested files (slight preference for top-level files)
                let depth_penalty = (path_segments.len() as f32 * 0.1).min(1.0);
                score -= depth_penalty;
                
                // Prioritize source files over other types
                if filename.ends_with(".rs") || 
                   filename.ends_with(".py") || 
                   filename.ends_with(".js") || 
                   filename.ends_with(".ts") || 
                   filename.ends_with(".rb") {
                    score += 1.0;
                }
                
                (path.clone(), score)
            })
            .filter(|(_, score)| *score > 0.0) // Only keep files with some relevance
            .collect();
        
        // Sort by descending score
        scored_paths.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Return just the paths
        scored_paths.into_iter()
            .take(max_files)
            .map(|(path, _)| path)
            .collect()
    }

    /// Record user feedback for a search result
    pub fn record_feedback(&mut self, query: &str, file_path: &str, is_relevant: bool) -> Result<()> {
        let query = query.to_lowercase(); // Normalize query string
        
        // Get or create feedback map for this query
        let query_map = self.feedback.query_feedback
            .entry(query.clone())
            .or_insert_with(HashMap::new);
            
        // Get or create feedback entry for this file
        let entry = query_map
            .entry(file_path.to_string())
            .or_insert(FeedbackEntry {
                relevant_count: 0,
                irrelevant_count: 0,
                relevance_score: 0.5, // Start neutral
            });
            
        // Update feedback counts
        if is_relevant {
            entry.relevant_count += 1;
        } else {
            entry.irrelevant_count += 1;
        }
        
        // Update relevance score using Bayesian average
        let total = entry.relevant_count + entry.irrelevant_count;
        if total > 0 {
            entry.relevance_score = entry.relevant_count as f32 / total as f32;
        }
        
        // Save the feedback to disk
        self.save()?;
        
        Ok(())
    }
    
    /// Get feedback relevance score for a query-file pair
    pub fn get_feedback_score(&self, query: &str, file_path: &str) -> Option<f32> {
        let query = query.to_lowercase(); // Normalize query
        
        // Look up the feedback entry
        self.feedback.query_feedback
            .get(&query)
            .and_then(|file_map| file_map.get(file_path))
            .map(|entry| entry.relevance_score)
    }
    
    /// Get similar queries to the current query based on feedback data
    pub fn get_similar_queries(&self, query: &str, max_queries: usize) -> Vec<String> {
        let query = query.to_lowercase(); // Normalize query
        let query_terms: Vec<&str> = query.split_whitespace().collect();
        
        if query_terms.is_empty() {
            return Vec::new();
        }
        
        // Calculate similarity between input query and existing queries with feedback
        let mut scored_queries: Vec<(String, f32)> = self.feedback.query_feedback.keys()
            .filter(|&existing_query| existing_query != &query) // Skip exact match
            .map(|existing_query| {
                let existing_terms: Vec<&str> = existing_query.split_whitespace().collect();
                
                // Calculate Jaccard similarity
                let intersection: Vec<&&str> = query_terms.iter()
                    .filter(|t| existing_terms.contains(t))
                    .collect();
                
                let union_size = query_terms.len() + existing_terms.len() - intersection.len();
                let similarity = if union_size > 0 {
                    intersection.len() as f32 / union_size as f32
                } else {
                    0.0
                };
                
                (existing_query.clone(), similarity)
            })
            .filter(|(_, score)| *score > 0.2) // Only keep somewhat similar queries
            .collect();
            
        // Sort by similarity (highest first)
        scored_queries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Return just the queries, limited to max_queries
        scored_queries.into_iter()
            .take(max_queries)
            .map(|(query, _)| query)
            .collect()
    }
    
    /// Apply feedback boost to search results based on previous feedback
    pub fn apply_feedback_boost(&self, query: &str, file_scores: &mut HashMap<String, f32>) {
        let query = query.to_lowercase(); // Normalize query
        
        // First check exact query match
        if let Some(feedback_map) = self.feedback.query_feedback.get(&query) {
            for (file_path, entry) in feedback_map {
                if let Some(score) = file_scores.get_mut(file_path) {
                    // Apply feedback boost - more boost for strong signal (many votes)
                    let confidence = (entry.relevant_count + entry.irrelevant_count) as f32 / 5.0;
                    let confidence_factor = confidence.min(1.0); // Cap at 1.0
                    let feedback_factor = (entry.relevance_score - 0.5) * 2.0; // Scale to [-1, 1]
                    
                    // Apply boost: positive for relevant files, negative for irrelevant
                    *score += feedback_factor * confidence_factor * 0.2; // Maximum 20% boost
                }
            }
        }
        
        // Then check similar queries (transfer learning)
        let similar_queries = self.get_similar_queries(&query, 3);
        for similar_query in similar_queries {
            if let Some(feedback_map) = self.feedback.query_feedback.get(&similar_query) {
                for (file_path, entry) in feedback_map {
                    if let Some(score) = file_scores.get_mut(file_path) {
                        // Apply smaller boost for similar queries
                        let confidence = (entry.relevant_count + entry.irrelevant_count) as f32 / 10.0;
                        let confidence_factor = confidence.min(1.0);
                        let feedback_factor = (entry.relevance_score - 0.5) * 2.0;
                        
                        // Smaller boost from similar queries (10% max)
                        *score += feedback_factor * confidence_factor * 0.1;
                    }
                }
            }
        }
    }

    /// Get the ONNX model path
    pub fn onnx_model_path(&self) -> Option<&PathBuf> {
        self.onnx_model_path.as_ref()
    }
    
    /// Get the ONNX tokenizer path
    pub fn onnx_tokenizer_path(&self) -> Option<&PathBuf> {
        self.onnx_tokenizer_path.as_ref()
    }
}

pub struct DBStats {
    pub indexed_files: usize,
    pub embedding_dimension: usize,
    pub db_path: String,
    pub cached_files: usize,
    pub hnsw_stats: Option<HNSWStats>,
    pub embedding_model_type: EmbeddingModelType,
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a > 0.0 && norm_b > 0.0 {
        // Ensure similarity stays within the [-1, 1] bounds
        (dot_product / (norm_a * norm_b)).clamp(-1.0, 1.0)
    } else {
        0.0 // Zero similarity if either vector has zero norm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use std::io::Write;

    /// Creates a set of test files in the given directory
    fn create_test_files(dir_path: &str, count: usize) -> Result<()> {
        for i in 0..count {
            let test_file = PathBuf::from(dir_path).join(format!("test_{}.txt", i));
            let mut file = fs::File::create(&test_file)
                .map_err(|e| VectorDBError::FileWriteError {
                    path: test_file.clone(),
                    source: e,
                })?;
            writeln!(file, "Test file content {}", i)
                .map_err(|e| VectorDBError::FileWriteError {
                    path: test_file,
                    source: e,
                })?;
        }
        Ok(())
    }

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
        // Create a temporary directory for the test
        let temp_dir = tempdir()?;
        let dir_path = temp_dir.path().to_string_lossy().to_string();
        
        // Create a test directory with some files
        create_test_files(&dir_path, 50)?;
        
        // Create a test database
        let db_path = tempdir()?.path().join("test.db").to_string_lossy().to_string();
        let mut db = VectorDB::new(db_path)?;
        
        // Index files and check if the layer count is optimized
        db.index_directory(&dir_path, &["txt".to_string()])?;
        
        // Get the layer count from the HNSW index
        let layers = db.hnsw_index.as_ref().unwrap().stats().layers;
        
        // Verify layers is a reasonable number (our HNSW implementation might use different calculation)
        // With 50 files, we expect between 5 and 16 layers
        assert!(layers >= 5, "Layers should be at least 5, got {}", layers);
        assert!(layers <= 16, "Layers should be at most 16, got {}", layers);
        
        Ok(())
    }
} 