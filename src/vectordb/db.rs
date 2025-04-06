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
use std::io::Read;
use log::{debug, info, warn, error, trace};
use crate::vectordb::repo_manager::RepoManager;
use crate::vectordb::auto_sync::AutoSyncDaemon;

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
    onnx_model_path: Option<String>,
    onnx_tokenizer_path: Option<String>,
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
    pub repo_manager: RepoManager,
    current_repo_id: Option<String>,
    current_branch: Option<String>,
    auto_sync_daemon: Option<AutoSyncDaemon>,
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
            repo_manager: self.repo_manager.clone(),
            current_repo_id: self.current_repo_id.clone(),
            current_branch: self.current_branch.clone(),
            auto_sync_daemon: self.auto_sync_daemon.clone(),
        }
    }
}

impl VectorDB {
    pub fn new(db_path: String) -> Result<Self> {
        debug!("Creating VectorDB with database path: {}", db_path);
        
        let (embeddings, hnsw_config, feedback, embedding_model_type, onnx_model_path, onnx_tokenizer_path) = if Path::new(&db_path).exists() {
            debug!("Database file exists, attempting to load");
            // Try to read the existing database file, but handle corruption gracefully
            match fs::read_to_string(&db_path) {
                Ok(contents) => {
                    debug!("Database file read successfully, parsing JSON");
                    match serde_json::from_str::<DBFile>(&contents) {
                        Ok(db_file) => {
                            debug!("Database parsed successfully: {} embeddings", db_file.embeddings.len());
                            (
                                db_file.embeddings, 
                                db_file.hnsw_config, 
                                db_file.feedback.unwrap_or_default(),
                                db_file.embedding_model_type.unwrap_or_default(),
                                db_file.onnx_model_path.map(PathBuf::from),
                                db_file.onnx_tokenizer_path.map(PathBuf::from),
                            )
                        },
                        Err(e) => {
                            // If JSON parsing fails, assume the file is corrupted
                            error!("Database file appears to be corrupted: {}", e);
                            eprintln!("Warning: Database file appears to be corrupted: {}", e);
                            eprintln!("Creating a new empty database.");
                            // Remove corrupted file
                            let _ = fs::remove_file(&db_path);
                            debug!("Creating a new empty database");
                            (HashMap::new(), Some(HNSWConfig::default()), FeedbackData::default(), EmbeddingModelType::Fast, None, None)
                        }
                    }
                }
                Err(e) => {
                    // Handle file read errors
                    error!("Couldn't read database file: {}", e);
                    eprintln!("Warning: Couldn't read database file: {}", e);
                    eprintln!("Creating a new empty database.");
                    debug!("Creating a new empty database");
                    (HashMap::new(), Some(HNSWConfig::default()), FeedbackData::default(), EmbeddingModelType::Fast, None, None)
                }
            }
        } else {
            // Create new database with default HNSW config
            debug!("Database file doesn't exist, creating new database");
            (HashMap::new(), Some(HNSWConfig::default()), FeedbackData::default(), EmbeddingModelType::Fast, None, None)
        };

        // Create cache in the same directory as the database
        let cache_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("cache.json")
            .to_string_lossy()
            .to_string();
        
        debug!("Creating embedding cache at: {}", cache_path);
        
        // Try to create cache, but handle potential cache corruption
        let mut cache = match EmbeddingCache::new(cache_path.clone()) {
            Ok(cache) => {
                debug!("Cache loaded successfully");
                cache
            },
            Err(e) => {
                error!("Couldn't load cache: {}", e);
                eprintln!("Warning: Couldn't load cache: {}", e);
                eprintln!("Creating a new empty cache.");
                // Try to remove the corrupted cache file
                let _ = fs::remove_file(&cache_path);
                // Create a new empty cache
                debug!("Creating a new empty cache");
                EmbeddingCache::new(cache_path)?
            }
        };
        
        // Configure the cache with the embedding model type
        debug!("Setting cache model type to: {:?}", embedding_model_type);
        cache.set_model_type(embedding_model_type.clone());
        
        // Create the repository manager config path
        let repo_manager_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("repositories.json");
        
        debug!("Creating repository manager at: {}", repo_manager_path.display());
        
        // Initialize the repository manager
        let repo_manager = match RepoManager::new(repo_manager_path) {
            Ok(manager) => {
                debug!("Repository manager initialized successfully");
                manager
            },
            Err(e) => {
                error!("Failed to initialize repository manager: {}", e);
                eprintln!("Warning: Failed to initialize repository manager: {}", e);
                eprintln!("Creating a new empty repository manager.");
                
                // Create parent directory if needed
                let parent = Path::new(&db_path).parent().unwrap_or_else(|| Path::new("."));
                let _ = fs::create_dir_all(parent);
                
                // Create new empty manager with default path
                let repo_config_path = parent.join("repositories.json");
                RepoManager::new(repo_config_path).unwrap_or_else(|_| {
                    panic!("Failed to create repository manager")
                })
            }
        };
        
        // Check for an HNSW index file
        let hnsw_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("hnsw_index.json");
        
        debug!("Looking for HNSW index at: {}", hnsw_path.display());
            
        // Try to load the index from file, or build a new one if config exists
        let hnsw_index = if hnsw_path.exists() {
            debug!("HNSW index file exists, attempting to load");
            match HNSWIndex::load_from_file(&hnsw_path) {
                Ok(index) => {
                    debug!("HNSW index loaded successfully");
                    Some(index)
                },
                Err(e) => {
                    // If loading fails, clean up and rebuild the index
                    error!("Couldn't load HNSW index: {}", e);
                    eprintln!("Warning: Couldn't load HNSW index: {}", e);
                    eprintln!("Creating a new index or rebuilding from embeddings.");
                    // Try to remove corrupted file
                    let _ = fs::remove_file(&hnsw_path);
                    
                    // Rebuild the index if we have a configuration
                    debug!("Rebuilding HNSW index from embeddings");
                    hnsw_config.map(|config| {
                        let mut index = HNSWIndex::new(config);
                        // Rebuild the index from embeddings
                        for (_, embedding) in &embeddings {
                            let _ = index.insert(embedding.clone());
                        }
                        debug!("HNSW index rebuilt with {} embeddings", embeddings.len());
                        index
                    })
                }
            }
        } else {
            // No index file, build from scratch with default or provided config
            debug!("No HNSW index file found, creating new index");
            let config = hnsw_config.unwrap_or_else(HNSWConfig::default);
            let mut index = HNSWIndex::new(config);
            // Build the index from embeddings if any exist
            for (_, embedding) in &embeddings {
                let _ = index.insert(embedding.clone());
            }
            debug!("New HNSW index created with {} embeddings", embeddings.len());
            Some(index)
        };
        
        debug!("VectorDB initialization complete");

        Ok(Self {
            embeddings,
            db_path,
            cache,
            hnsw_index,
            feedback,
            embedding_model_type,
            onnx_model_path,
            onnx_tokenizer_path,
            repo_manager,
            current_repo_id: None,
            current_branch: None,
            auto_sync_daemon: None,
        })
    }
    
    /// Configure the ONNX embedding model paths
    pub fn set_onnx_paths(&mut self, model_path: Option<PathBuf>, tokenizer_path: Option<PathBuf>) -> Result<()> {
        // Validate paths if provided
        if let Some(model_path) = &model_path {
            if !model_path.exists() {
                return Err(VectorDBError::EmbeddingError(
                    format!("ONNX model file not found: {}", model_path.display())
                ));
            }
        }
        
        if let Some(tokenizer_path) = &tokenizer_path {
            if !tokenizer_path.exists() {
                return Err(VectorDBError::EmbeddingError(
                    format!("ONNX tokenizer file not found: {}", tokenizer_path.display())
                ));
            }
        }
        
        // If both paths are set, try to create the model to verify it works
        if let (Some(model_path_ref), Some(tokenizer_path_ref)) = (&model_path, &tokenizer_path) {
            match EmbeddingModel::new_onnx(model_path_ref, tokenizer_path_ref) {
                Ok(_) => {
                    // Model created successfully, safe to set the paths
                    self.onnx_model_path = model_path;
                    self.onnx_tokenizer_path = tokenizer_path;
                    
                    // Update cache model settings
                    self.cache.set_model_type(EmbeddingModelType::Onnx);
                    self.cache.invalidate_different_model_types();
                    
                    self.save()?;
                },
                Err(e) => {
                    return Err(VectorDBError::EmbeddingError(
                        format!("Failed to initialize ONNX model with provided paths: {}", e)
                    ));
                }
            }
        } else {
            // If clearing paths or only setting one, just update the paths
            self.onnx_model_path = model_path;
            self.onnx_tokenizer_path = tokenizer_path;
            self.save()?;
        }
        
        Ok(())
    }
    
    /// Set the embedding model type
    pub fn set_embedding_model_type(&mut self, model_type: EmbeddingModelType) -> Result<()> {
        // If switching to ONNX, validate that we can actually create an embedding model
        if model_type == EmbeddingModelType::Onnx {
            // Check if ONNX paths are set
            if self.onnx_model_path.is_none() || self.onnx_tokenizer_path.is_none() {
                return Err(VectorDBError::EmbeddingError(
                    "Cannot set ONNX model type: model or tokenizer paths not set".to_string()
                ));
            }
            
            // Validate that the ONNX model can actually be created
            let onnx_model_path = self.onnx_model_path.as_ref().unwrap();
            let onnx_tokenizer_path = self.onnx_tokenizer_path.as_ref().unwrap();
            
            // Verify the paths exist
            if !onnx_model_path.exists() {
                return Err(VectorDBError::EmbeddingError(
                    format!("ONNX model file not found: {}", onnx_model_path.display())
                ));
            }
            
            if !onnx_tokenizer_path.exists() {
                return Err(VectorDBError::EmbeddingError(
                    format!("ONNX tokenizer file not found: {}", onnx_tokenizer_path.display())
                ));
            }
            
            // Try to create the model to ensure it works
            match EmbeddingModel::new_onnx(onnx_model_path, onnx_tokenizer_path) {
                Ok(_) => {
                    // Model created successfully, safe to set the model type
                    self.embedding_model_type = model_type;
                },
                Err(e) => {
                    return Err(VectorDBError::EmbeddingError(
                        format!("Failed to initialize ONNX model: {}", e)
                    ));
                }
            }
        } else {
            // Fast model doesn't need validation
            self.embedding_model_type = model_type;
        }
        
        // Save the updated configuration
        self.save()?;
        
        Ok(())
    }
    
    /// Get the current embedding model type
    pub fn embedding_model_type(&self) -> &EmbeddingModelType {
        &self.embedding_model_type
    }
    
    /// Create the appropriate embedding model based on configuration
    fn create_embedding_model(&self) -> Result<EmbeddingModel> {
        match &self.embedding_model_type {
            EmbeddingModelType::Fast => {
                Ok(EmbeddingModel::new())
            },
            EmbeddingModelType::Onnx => {
                if let (Some(model_path), Some(tokenizer_path)) = (&self.onnx_model_path, &self.onnx_tokenizer_path) {
                    EmbeddingModel::new_onnx(model_path, tokenizer_path)
                        .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))
                } else {
                    // Error instead of fallback since environment variables are now mandatory
                    Err(VectorDBError::EmbeddingError(
                        "ONNX model paths not set. Environment variables VECTORDB_ONNX_MODEL and VECTORDB_ONNX_TOKENIZER are required".to_string()
                    ))
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
        self.index_file_without_save(file_path)?;
        self.save()?;
        Ok(())
    }

    pub fn index_file_without_save(&mut self, file_path: &Path) -> Result<()> {
        let file_path_str = file_path.to_string_lossy().to_string();
        
        // Check cache first
        if let Some(cached_embedding) = self.cache.get(&file_path_str) {
            self.embeddings.insert(file_path_str.clone(), cached_embedding.to_vec());
            
            // Add to HNSW index if available
            if let Some(index) = &mut self.hnsw_index {
                index.insert(cached_embedding.to_vec())?;
            }
            
            return Ok(());
        }

        // If not in cache, generate new embedding for the entire file
        let model = self.create_embedding_model()
            .map_err(|e| {
                // Log the error with more detail
                if self.embedding_model_type == EmbeddingModelType::Onnx {
                    eprintln!("Error creating ONNX embedding model: {}", e);
                    if self.onnx_model_path.is_none() || self.onnx_tokenizer_path.is_none() {
                        eprintln!("ONNX model paths missing - model: {:?}, tokenizer: {:?}", 
                                 self.onnx_model_path, self.onnx_tokenizer_path);
                    } else {
                        eprintln!("ONNX model paths are set but model creation failed");
                    }
                }
                VectorDBError::EmbeddingError(e.to_string())
            })?;
        
        let contents = fs::read_to_string(file_path)
            .map_err(|e| VectorDBError::IOError(e))?;
            
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
        
        Ok(())
    }

    /// Directory indexing function with repository awareness
    pub fn index_directory(&mut self, dir: &str, file_types: &[String]) -> Result<()> {
        let dir_path = Path::new(dir);
        if !dir_path.exists() || !dir_path.is_dir() {
            return Err(VectorDBError::DirectoryNotFound(dir.to_string()));
        }
        
        debug!("Starting directory scan for files to index in {}", dir);
        
        // Scan directory for files to index first
        let files: Vec<PathBuf> = if file_types.is_empty() && self.embedding_model_type == EmbeddingModelType::Fast {
            // If file_types is empty and we're using the fast model, index all non-binary files
            debug!("Using fast model with no file types specified - indexing all non-binary files");
            WalkDir::new(dir_path)
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    // Only include files (not directories)
                    if !entry.file_type().is_file() {
                        return false;
                    }
                    
                    // Check if it's likely a binary file (simple heuristic)
                    // Try to read the first few bytes to see if it contains NUL bytes
                    if let Ok(file) = std::fs::File::open(entry.path()) {
                        let mut buffer = [0u8; 512];
                        let mut reader = std::io::BufReader::new(file);
                        if let Ok(bytes_read) = reader.read(&mut buffer) {
                            if bytes_read > 0 {
                                // If we find a NUL byte, assume it's binary
                                return !buffer[..bytes_read].contains(&0);
                            }
                        }
                    }
                    
                    // Default to including the file if we couldn't determine binary status
                    true
                })
                .map(|entry| entry.path().to_path_buf())
                .collect()
        } else {
            // Regular mode - only include files with specified extensions
            WalkDir::new(dir_path)
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.file_type().is_file() && match entry.path().extension() {
                        Some(ext) => file_types.contains(&ext.to_string_lossy().to_string()),
                        None => false,
                    }
                })
                .map(|entry| entry.path().to_path_buf())
                .collect()
        };
        
        let file_count = files.len();
        
        if file_count == 0 {
            println!("No matching files found in the directory.");
            return Ok(());
        }
        
        println!("Found {} files to index.", file_count);
        
        // The batch size determines how many files to process before saving
        // For small repos (< 1000 files), save after every 100 files
        // For larger repos, save less frequently to reduce overhead
        let batch_size = if file_count < 1000 {
            100
        } else if file_count < 10000 {
            500
        } else {
            1000
        };
        
        debug!("Using batch size of {} for {} files", batch_size, file_count);
        
        // Create progress bar
        let progress = indicatif::ProgressBar::new(file_count as u64);
        progress.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files indexed ({eta}) {msg}")
                .unwrap()
                .progress_chars("#>-")
        );
        
        // Set up shared state for parallel processing
        let processed = Arc::new(AtomicUsize::new(0));
        let interrupt = Arc::new(AtomicBool::new(false));
        let indexed_files_mutex = Arc::new(Mutex::new(Vec::<String>::with_capacity(file_count)));
        
        // Create channels for processing results
        let (tx, rx) = std::sync::mpsc::channel();
        
        // Process embeddings in parallel
        let num_threads = rayon::current_num_threads();
        progress.set_message(format!("(using {} threads)", num_threads));
        
        // Clone references needed for parallel processing
        let progress_clone = progress.clone();
        let processed_clone = processed.clone();
        let interrupt_clone = interrupt.clone();
        
        // Prepare a safe clone of all necessary components to enable parallel processing
        let embedding_model_type = self.embedding_model_type.clone();
        let onnx_model_path = self.onnx_model_path.clone();
        let onnx_tokenizer_path = self.onnx_tokenizer_path.clone();
        let cache = self.cache.clone();
        
        // Launch parallel processing
        let processor_handle = std::thread::spawn(move || {
            // Process files in parallel
            files.par_iter().for_each(|file_path| {
                // Check for interruption
                if unsafe { crate::cli::commands::INTERRUPT_RECEIVED } || interrupt_clone.load(Ordering::SeqCst) {
                    return;
                }
                
                let file_path_str = file_path.to_string_lossy().to_string();
                
                // Try to load from cache first to avoid redundant work
                let cached_embedding = cache.get(&file_path_str);
                
                // If not in cache, generate new embedding
                let result = if let Some(embedding) = cached_embedding {
                    // Found in cache
                    debug!("Cache hit for file: {}", file_path_str);
                    Ok((file_path.clone(), embedding.to_vec(), true))
                } else {
                    // Not in cache, generate new embedding
                    debug!("Generating embedding for file: {}", file_path_str);
                    
                    // Create a model based on the configured type
                    let model = match embedding_model_type {
                        EmbeddingModelType::Fast => {
                            Ok(EmbeddingModel::new())
                        },
                        EmbeddingModelType::Onnx => {
                            if let (Some(model_path), Some(tokenizer_path)) = (&onnx_model_path, &onnx_tokenizer_path) {
                                EmbeddingModel::new_onnx(model_path, tokenizer_path)
                            } else {
                                // Fallback to fast model if paths aren't set
                                Ok(EmbeddingModel::new())
                            }
                        }
                    };
                    
                    // If model creation failed, return the error
                    let model = match model {
                        Ok(m) => m,
                        Err(e) => {
                            return tx.send(Err((file_path_str, VectorDBError::EmbeddingError(e.to_string())))).unwrap();
                        }
                    };
                    
                    // Read the file contents
                    match fs::read_to_string(file_path) {
                        Ok(contents) => {
                            // Generate embedding
                            match model.embed(&contents) {
                                Ok(embedding) => {
                                    // Calculate file hash for caching
                                    match EmbeddingCache::get_file_hash(file_path) {
                                        Ok(file_hash) => {
                                            // Successfully generated embedding
                                            Ok((file_path.clone(), embedding, false))
                                        },
                                        Err(e) => {
                                            Err((file_path_str, VectorDBError::EmbeddingError(e.to_string())))
                                        }
                                    }
                                },
                                Err(e) => {
                                    Err((file_path_str, VectorDBError::EmbeddingError(e.to_string())))
                                }
                            }
                        },
                        Err(e) => {
                            Err((file_path_str, VectorDBError::IOError(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))))
                        }
                    }
                };
                
                // Send result back to main thread
                tx.send(result).unwrap();
                
                // Update progress
                let count = processed_clone.fetch_add(1, Ordering::SeqCst) + 1;
                progress_clone.set_position(count as u64);
                
                // Show progress message occasionally but don't overwhelm the output
                if count % 10 == 0 || count == file_count {
                    progress_clone.set_message(format!("(using {} threads)", num_threads));
                }
            });
        });
        
        // Track embeddings added so far
        let mut successful_embeddings = 0;
        let mut save_counter = 0;
        let start_time = std::time::Instant::now();
        let mut last_report_time = start_time;
        
        // Process results from worker threads
        for _ in 0..file_count {
            match rx.recv() {
                Ok(result) => {
                    match result {
                        Ok((file_path, embedding, from_cache)) => {
                            // Add to our database
                            let file_path_str = file_path.to_string_lossy().to_string();
                            
                            // Store in database (cache was already updated in the worker thread if needed)
                            self.embeddings.insert(file_path_str.clone(), embedding.clone());
                            
                            // Add to HNSW index if available
                            if let Some(index) = &mut self.hnsw_index {
                                if let Err(e) = index.insert(embedding) {
                                    error!("Failed to insert into HNSW index: {}", e);
                                    progress.println(format!("Warning: Failed to add {} to HNSW index: {}", file_path_str, e));
                                }
                            }
                            
                            // Track for reporting
                            indexed_files_mutex.lock().unwrap().push(file_path_str.clone());
                            successful_embeddings += 1;
                            
                            // Print occasional progress for large batches
                            let now = std::time::Instant::now();
                            if now.duration_since(last_report_time).as_secs() >= 10 {
                                last_report_time = now;
                                let elapsed = now.duration_since(start_time).as_secs();
                                let rate = if elapsed > 0 { successful_embeddings as f64 / elapsed as f64 } else { 0.0 };
                                progress.println(format!("Processed {}/{} files ({:.1} files/sec)", 
                                    successful_embeddings, file_count, rate));
                            }
                            
                            // Save periodically
                            save_counter += 1;
                            if save_counter >= batch_size {
                                // Save the database and reset counter
                                if let Err(e) = self.save() {
                                    error!("Failed to save database during batch processing: {}", e);
                                    progress.println(format!("Warning: Failed to save database: {}", e));
                                } else {
                                    debug!("Saved database after processing {} files", save_counter);
                                }
                                save_counter = 0;
                            }
                        },
                        Err((file_path, error)) => {
                            // Show error but continue with other files
                            progress.println(format!("Error indexing {}: {}", file_path, error));
                        }
                    }
                },
                Err(e) => {
                    error!("Channel error: {}", e);
                    progress.println(format!("Error communicating with worker threads: {}", e));
                    break;
                }
            }
        }
        
        // Wait for the processor thread to finish
        processor_handle.join().unwrap();
        
        // Final save if any unsaved changes
        if save_counter > 0 {
            if let Err(e) = self.save() {
                error!("Failed to save database at end of indexing: {}", e);
                progress.println(format!("Warning: Failed to save database: {}", e));
            }
        }
        
        // Report final statistics
        let elapsed = start_time.elapsed().as_secs();
        let rate = if elapsed > 0 { successful_embeddings as f64 / elapsed as f64 } else { 0.0 };
        let indexed_files = indexed_files_mutex.lock().unwrap();
        
        progress.finish_with_message(format!("Indexed {} files successfully in {}s ({:.1} files/sec)", 
            successful_embeddings, elapsed, rate));
        
        // Check if we're in a repository context and update the commit hash
        if let (Some(repo_id), Some(branch)) = (&self.current_repo_id, &self.current_branch) {
            debug!("Updating commit hash for repository {} branch {}", repo_id, branch);
            
            // Clone repo_id and branch to avoid borrow checker issues
            let repo_id_clone = repo_id.clone();
            let branch_clone = branch.clone();
            
            // Only update if we actually have a repository
            if let Some(repo) = self.repo_manager.get_repository(&repo_id_clone) {
                // Create a git repository object
                if let Ok(git_repo) = crate::utils::git::GitRepo::new(repo.path.clone()) {
                    // Get the current commit hash
                    if let Ok(commit_hash) = git_repo.get_commit_hash(&branch_clone) {
                        // Update the indexed commit hash
                        self.repo_manager.update_indexed_commit(&repo_id_clone, &branch_clone, &commit_hash)?;
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Override the save method to handle repository-specific paths
    pub fn save(&mut self) -> Result<()> {
        debug!("Saving VectorDB to {}", self.db_path);
        
        // Create the parent directory if it doesn't exist
        let path = Path::new(&self.db_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| VectorDBError::DirectoryCreationError {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        
        // Prepare the DB file
        let db_file = DBFile {
            embeddings: self.embeddings.clone(),
            hnsw_config: self.hnsw_index.as_ref().map(|idx| idx.get_config()),
            feedback: Some(self.feedback.clone()),
            embedding_model_type: Some(self.embedding_model_type.clone()),
            onnx_model_path: self.onnx_model_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            onnx_tokenizer_path: self.onnx_tokenizer_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        };
        
        // Serialize and save
        let json = serde_json::to_string_pretty(&db_file)?;
        fs::write(&self.db_path, &json).map_err(|e| VectorDBError::FileWriteError {
            path: path.to_path_buf(),
            source: e,
        })?;
        
        // Save the HNSW index if present
        if let Some(index) = &self.hnsw_index {
            let hnsw_path = if self.is_in_repository_context() {
                // Use repository-specific path
                let repo_id = self.current_repo_id.as_ref().unwrap();
                let branch = self.current_branch.as_ref().unwrap();
                self.get_hnsw_path_for_repo(repo_id, branch)
            } else {
                // Use the original path
                Path::new(&self.db_path)
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("hnsw_index.json")
            };
            
            // Make sure the parent directory exists
            if let Some(parent) = hnsw_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            
            debug!("Saving HNSW index to {}", hnsw_path.display());
            if let Err(e) = index.save_to_file(&hnsw_path) {
                error!("Failed to save HNSW index: {}", e);
                eprintln!("Warning: Failed to save HNSW index: {}", e);
            }
        }
        
        // Save the repository manager if we're in a repository context
        if self.is_in_repository_context() {
            // Update the last_indexed timestamp for the current repository/branch
            if let (Some(repo_id), Some(branch)) = (&self.current_repo_id, &self.current_branch) {
                debug!("Updating last_indexed for repository {}, branch {}", repo_id, branch);
                
                // Only update if we actually have a repository manager
                if let Some(repo) = self.repo_manager.get_repository_mut(repo_id) {
                    repo.last_indexed = Some(chrono::Utc::now());
                }
            }
            
            debug!("Saving repository manager");
            if let Err(e) = self.repo_manager.save() {
                error!("Failed to save repository manager: {}", e);
                eprintln!("Warning: Failed to save repository manager: {}", e);
            }
        }
        
        // Save the cache
        self.cache.save()?;
        
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
    
    /// Clear data for a specific repository
    pub fn clear_repository(&mut self, repo_name: &str) -> Result<()> {
        // Resolve repository name to ID
        let repo_id = self.repo_manager.resolve_repo_name_to_id(repo_name)?;
        
        // Get the repository
        let repo_config = self.repo_manager.get_repository(&repo_id)
            .ok_or_else(|| VectorDBError::RepositoryNotFound(repo_name.to_string()))?;
            
        let current_branch = repo_config.active_branch.clone();
            
        // Check if we're currently in the context of the repository being cleared
        let is_current_repo = self.current_repo_id.as_ref().map_or(false, |id| id == &repo_id) &&
                             self.current_branch.as_ref().map_or(false, |b| b == &current_branch);
                             
        if is_current_repo {
            // If we're in the context of this repository, need to clear in-memory data
            debug!("Clearing in-memory data for current repository: {}", repo_name);
            
            // Clear embeddings
            self.embeddings.clear();
            
            // Reset HNSW index if one exists
            if let Some(index) = &self.hnsw_index {
                let config = index.get_config();
                self.hnsw_index = Some(HNSWIndex::new(config));
            }
            
            // Clear the cache
            self.cache.clear()?;
            
            // Clear feedback data (for simplicity, we clear all feedback)
            self.feedback = FeedbackData::default();
            
            // Save the changes
            self.save()?;
        } else {
            // If not the current repository, we can just update the repository config
            debug!("Repository {} is not the current active repository", repo_name);
        }
        
        // Update the repository configuration to reflect that the branches are no longer indexed
        if let Some(repo) = self.repo_manager.get_repository_mut(&repo_id) {
            repo.indexed_branches.clear();
            
            // Save the repository manager
            self.repo_manager.save()?;
        }
        
        debug!("Repository {} cleared successfully", repo_name);
        Ok(())
    }

    pub fn stats(&self) -> DBStats {
        let embedding_dimension = if !self.embeddings.is_empty() {
            self.embeddings.values().next().unwrap().len()
        } else {
            match &self.embedding_model_type {
                EmbeddingModelType::Fast => EMBEDDING_DIM,
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
    pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
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
                   filename.ends_with(".rb") || 
                   filename.ends_with(".go") {
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

    // Add a method to safely access the HNSW index
    pub fn hnsw_index(&self) -> Option<&HNSWIndex> {
        if let Some(index) = &self.hnsw_index {
            debug!("HNSW index accessed: {} nodes, {} layers", 
                   index.stats().total_nodes, 
                   index.stats().layers);
            Some(index)
        } else {
            debug!("HNSW index requested but not available");
            None
        }
    }

    /// Get a list of all supported file types
    pub fn get_supported_file_types() -> Vec<String> {
        // Return a comprehensive list of supported file extensions
        // This list is based on file_type_weights in code_ranking.rs
        vec![
            // Programming languages
            "rs".to_string(),  // Rust
            "rb".to_string(),  // Ruby
            "go".to_string(),  // Go
            "py".to_string(),  // Python
            "js".to_string(),  // JavaScript
            "ts".to_string(),  // TypeScript
            // C/C++
            "c".to_string(),   // C
            "cpp".to_string(), // C++
            "h".to_string(),   // C/C++ header
            "hpp".to_string(), // C++ header
            // Interface definitions
            "proto".to_string(), // Protocol Buffers
            // Documentation
            "md".to_string(),   // Markdown
            "txt".to_string(),  // Text
            "rst".to_string(),  // reStructuredText
            // Configuration
            "json".to_string(), // JSON
            "yaml".to_string(), // YAML
            "yml".to_string(),  // YAML alternative
            "toml".to_string(), // TOML
            "xml".to_string(),  // XML
        ]
    }

    /// Get the database path for a specific repository and branch
    pub fn get_db_path_for_repo(&self, repo_id: &str, branch: &str) -> PathBuf {
        let repo_name = self.repo_manager.get_repository(repo_id)
            .map(|repo| repo.name.clone())
            .unwrap_or_else(|| repo_id.to_string());
        
        Path::new(&self.db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("repositories")
            .join(repo_name)
            .join(branch)
            .join("db.json")
    }
    
    /// Get the HNSW index path for a specific repository and branch
    pub fn get_hnsw_path_for_repo(&self, repo_id: &str, branch: &str) -> PathBuf {
        let repo_name = self.repo_manager.get_repository(repo_id)
            .map(|repo| repo.name.clone())
            .unwrap_or_else(|| repo_id.to_string());
        
        Path::new(&self.db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("repositories")
            .join(repo_name)
            .join(branch)
            .join("hnsw_index.json")
    }
    
    /// Get the cache path for a specific repository and branch
    pub fn get_cache_path_for_repo(&self, repo_id: &str, branch: &str) -> PathBuf {
        let repo_name = self.repo_manager.get_repository(repo_id)
            .map(|repo| repo.name.clone())
            .unwrap_or_else(|| repo_id.to_string());
        
        Path::new(&self.db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("repositories")
            .join(repo_name)
            .join(branch)
            .join("cache.json")
    }
    
    /// Switch to a specific repository and branch context
    pub fn switch_repository(&mut self, repo_id: &str, branch: Option<&str>) -> Result<()> {
        debug!("Switching to repository: {}", repo_id);
        
        // Get the repository
        let repo = self.repo_manager.get_repository(repo_id)
            .ok_or_else(|| VectorDBError::RepositoryError(
                format!("Repository with ID '{}' not found", repo_id)
            ))?;
        
        // Determine which branch to use
        let branch_name = branch.unwrap_or(&repo.active_branch);
        
        debug!("Switching to branch: {}", branch_name);
        
        // Clone the necessary data to avoid borrow checker issues
        let branch_name = branch_name.to_string();
        let repo_path = repo.path.clone();
        let repo_active_branch = repo.active_branch.clone();
        
        // If we're already in this repository and branch, nothing to do
        if self.current_repo_id.as_deref() == Some(repo_id) && 
           self.current_branch.as_deref() == Some(&branch_name) {
            debug!("Already in repository {} branch {}", repo_id, branch_name);
            return Ok(());
        }
        
        // Save current state if we're in a repository context
        if let (Some(current_repo), Some(current_branch)) = (&self.current_repo_id, &self.current_branch) {
            debug!("Saving current state for repository {} branch {}", current_repo, current_branch);
            
            // Clone these values before calling save
            let current_repo_clone = current_repo.clone();
            let current_branch_clone = current_branch.clone();
            
            // Save the current state
            let db_path = self.get_db_path_for_repo(&current_repo_clone, &current_branch_clone);
            
            // Create parent directory if needed
            if let Some(parent) = db_path.parent() {
                fs::create_dir_all(parent)?;
            }
            
            // Save manually rather than using self.save() to avoid borrowing issues
            // This is a partial save that just writes the current database state
            
            // Prepare the DB file
            let db_file = DBFile {
                embeddings: self.embeddings.clone(),
                hnsw_config: self.hnsw_index.as_ref().map(|idx| idx.get_config()),
                feedback: Some(self.feedback.clone()),
                embedding_model_type: Some(self.embedding_model_type.clone()),
                onnx_model_path: self.onnx_model_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                onnx_tokenizer_path: self.onnx_tokenizer_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            };
            
            // Serialize and save
            let json = serde_json::to_string_pretty(&db_file)?;
            fs::write(&db_path, &json).map_err(|e| VectorDBError::FileWriteError {
                path: db_path.clone(),
                source: e,
            })?;
            
            // Save the HNSW index if present
            if let Some(index) = &self.hnsw_index {
                let hnsw_path = self.get_hnsw_path_for_repo(&current_repo_clone, &current_branch_clone);
                if let Some(parent) = hnsw_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                
                debug!("Saving HNSW index to {}", hnsw_path.display());
                if let Err(e) = index.save_to_file(&hnsw_path) {
                    error!("Failed to save HNSW index: {}", e);
                    eprintln!("Warning: Failed to save HNSW index: {}", e);
                }
            }
            
            // Also save cache
            self.cache.save()?;
        }
        
        // Get the new DB path
        let db_path = self.get_db_path_for_repo(repo_id, &branch_name);
        debug!("New database path: {}", db_path.display());
        
        // Try to load the new database
        if db_path.exists() {
            debug!("Loading existing repository database");
            
            // Read the file
            let contents = fs::read_to_string(&db_path)
                .map_err(|e| VectorDBError::IOError(e))?;
            
            // Parse the JSON
            let db_file: DBFile = serde_json::from_str(&contents)
                .map_err(|e| VectorDBError::DeserializationError(e.to_string()))?;
            
            // Extract the data
            self.embeddings = db_file.embeddings;
            self.feedback = db_file.feedback.unwrap_or_default();
            
            // Handle model type
            if let Some(model_type) = db_file.embedding_model_type {
                self.embedding_model_type = model_type;
            }
            
            // Handle ONNX paths
            self.onnx_model_path = db_file.onnx_model_path.map(PathBuf::from);
            self.onnx_tokenizer_path = db_file.onnx_tokenizer_path.map(PathBuf::from);
            
            // Load HNSW index if it exists
            let hnsw_path = self.get_hnsw_path_for_repo(repo_id, &branch_name);
            self.hnsw_index = if hnsw_path.exists() {
                match HNSWIndex::load_from_file(&hnsw_path) {
                    Ok(index) => Some(index),
                    Err(e) => {
                        warn!("Failed to load HNSW index: {}", e);
                        // Create a new index with default config
                        let mut index = HNSWIndex::new(HNSWConfig::default());
                        // Populate with embeddings
                        for (_, embedding) in &self.embeddings {
                            let _ = index.insert(embedding.clone());
                        }
                        Some(index)
                    }
                }
            } else if let Some(config) = db_file.hnsw_config {
                // Create a new index with the config
                let mut index = HNSWIndex::new(config);
                // Populate with embeddings
                for (_, embedding) in &self.embeddings {
                    let _ = index.insert(embedding.clone());
                }
                Some(index)
            } else {
                None
            };
            
            // Load cache
            let cache_path = self.get_cache_path_for_repo(repo_id, &branch_name);
            
            // Try to load the cache, but handle potential errors
            self.cache = if cache_path.exists() {
                match EmbeddingCache::new(cache_path.to_string_lossy().to_string()) {
                    Ok(cache) => cache,
                    Err(e) => {
                        warn!("Failed to load cache: {}", e);
                        // Create a new cache
                        EmbeddingCache::new(cache_path.to_string_lossy().to_string())?
                    }
                }
            } else {
                // Create a new cache
                EmbeddingCache::new(cache_path.to_string_lossy().to_string())?
            };
            
            // Configure the cache with the embedding model type
            self.cache.set_model_type(self.embedding_model_type.clone());
        } else {
            debug!("No existing repository database, creating empty one");
            
            // Create a new empty database
            self.embeddings = HashMap::new();
            self.feedback = FeedbackData::default();
            
            // Set model type to the repository-specific one if available
            if let Some(model_type) = repo.embedding_model.clone() {
                self.embedding_model_type = model_type;
            }
            
            // Create a new empty HNSW index
            let config = HNSWConfig::default();
            self.hnsw_index = Some(HNSWIndex::new(config));
            
            // Create a new empty cache
            let cache_path = self.get_cache_path_for_repo(repo_id, &branch_name);
            self.cache = EmbeddingCache::new(cache_path.to_string_lossy().to_string())?;
            self.cache.set_model_type(self.embedding_model_type.clone());
        }
        
        // Update the repository and branch context
        self.current_repo_id = Some(repo_id.to_string());
        self.current_branch = Some(branch_name);
        
        // Update the active repository in the manager
        self.repo_manager.set_active_repository(repo_id)?;
        
        // Update the db_path to point to the repository-specific path
        self.db_path = db_path.to_string_lossy().to_string();
        
        Ok(())
    }
    
    /// Switch to a different branch in the current repository
    pub fn switch_branch(&mut self, branch: &str) -> Result<()> {
        // Check if we're in a repository context
        if let Some(repo_id) = self.current_repo_id.clone() {
            debug!("Switching to branch {} in repository {}", branch, repo_id);
            self.switch_repository(&repo_id, Some(branch))
        } else {
            Err(VectorDBError::RepositoryError(
                "Not in a repository context".to_string()
            ))
        }
    }
    
    /// Get the current repository ID
    pub fn current_repo_id(&self) -> Option<&String> {
        self.current_repo_id.as_ref()
    }
    
    /// Get the current branch
    pub fn current_branch(&self) -> Option<&String> {
        self.current_branch.as_ref()
    }
    
    /// Check if we're in a repository context
    pub fn is_in_repository_context(&self) -> bool {
        self.current_repo_id.is_some() && self.current_branch.is_some()
    }

    /// Index a repository on a specific branch
    pub fn index_repository_full(&mut self, repo_id: &str, branch: &str) -> Result<()> {
        debug!("Full indexing of repository {} branch {}", repo_id, branch);
        
        // Clone all necessary data to avoid borrow checker issues
        let repo_data = {
            // Get the repository
            let repo = self.repo_manager.get_repository(repo_id)
                .ok_or_else(|| VectorDBError::RepositoryError(
                    format!("Repository with ID '{}' not found", repo_id)
                ))?;
            
            // Clone what we need
            (
                repo.path.clone(),          // repo_path
                repo.name.clone(),          // repo_name
                if repo.file_types.is_empty() {
                    Self::get_supported_file_types()
                } else {
                    repo.file_types.clone()
                },                          // file_types
                repo_id.to_string(),        // repo_id
                branch.to_string()          // branch
            )
        };
        
        let (repo_path, repo_name, file_types, repo_id, branch) = repo_data;
        
        // Switch to this repository context
        self.switch_repository(&repo_id, Some(&branch))?;
        
        // Create a git repository object
        let git_repo = crate::utils::git::GitRepo::new(repo_path.clone())
            .map_err(|e| VectorDBError::RepositoryError(e.to_string()))?;
        
        // Get the current commit hash
        let commit_hash = git_repo.get_commit_hash(&branch)
            .map_err(|e| VectorDBError::RepositoryError(e.to_string()))?;
        
        // Run a full indexing on the repository
        info!("Indexing repository {} ({}), branch {} at commit {}",
              repo_name, repo_id, branch, commit_hash);
        
        // Print file extension types we're indexing
        info!("Indexing files with extensions: {}", file_types.join(", "));
        
        // Index the repository directory
        self.index_directory(&repo_path.to_string_lossy(), &file_types)?;
        
        // Update the indexed commit hash
        self.repo_manager.update_indexed_commit(&repo_id, &branch, &commit_hash)?;
        
        // Save the updates
        self.save()?;
        
        info!("Repository {} branch {} indexed successfully", repo_name, branch);
        
        Ok(())
    }

    /// Index a repository incrementally based on changes
    pub fn index_repository_changes(&mut self, repo_id: &str, branch: &str) -> Result<()> {
        debug!("Incremental indexing of repository {} branch {}", repo_id, branch);
        
        // Clone all necessary data to avoid borrow checker issues
        let repo_data = {
            let repo = self.repo_manager.get_repository(repo_id)
                .ok_or_else(|| VectorDBError::RepositoryError(
                    format!("Repository with ID '{}' not found", repo_id)
                ))?;
            
            // Clone what we need
            (
                repo.path.clone(),          // repo_path
                repo.name.clone(),          // repo_name
                repo.get_indexed_commit(branch).cloned(), // last_commit
                if repo.file_types.is_empty() {
                    Self::get_supported_file_types()
                } else {
                    repo.file_types.clone()
                },                          // file_types
                repo_id.to_string(),        // repo_id
                branch.to_string()          // branch
            )
        };
        
        let (repo_path, repo_name, last_commit, file_types, repo_id, branch) = repo_data;
        
        // Create a git repository object
        let git_repo = crate::utils::git::GitRepo::new(repo_path.clone())
            .map_err(|e| VectorDBError::RepositoryError(e.to_string()))?;
        
        // Get the current commit hash
        let current_commit = git_repo.get_commit_hash(&branch)
            .map_err(|e| VectorDBError::RepositoryError(e.to_string()))?;
        
        // If no previous commit, do a full index
        if last_commit.is_none() {
            info!("No previous index found for repository {} branch {}, doing full index",
                 repo_name, branch);
            return self.index_repository_full(&repo_id, &branch);
        }
        
        let last_commit = last_commit.unwrap();
        
        // If commits are the same, nothing to do
        if last_commit == current_commit {
            info!("Repository {} branch {} is already up to date at commit {}",
                 repo_name, branch, current_commit);
            return Ok(());
        }
        
        // Switch to this repository context
        self.switch_repository(&repo_id, Some(&branch))?;
        
        // Get the change set between commits
        let changes = git_repo.get_change_set(&last_commit, &current_commit)
            .map_err(|e| VectorDBError::RepositoryError(e.to_string()))?;
        
        info!("Incremental indexing of repository {} branch {} from commit {} to {}",
             repo_name, branch, last_commit, current_commit);
        
        info!("Changes detected: {} added, {} modified, {} deleted files",
             changes.added_files.len(), changes.modified_files.len(), changes.deleted_files.len());
        
        // Process added and modified files
        let files_to_process = [&changes.added_files[..], &changes.modified_files[..]].concat();
        
        // Filter for file types we care about
        let filtered_files: Vec<_> = files_to_process.iter()
            .filter(|path| {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_string();
                    file_types.contains(&ext_str)
                } else {
                    false
                }
            })
            .map(|p| p.clone())  // Clone to own the PathBuf
            .collect();
        
        let total = filtered_files.len();
        
        if total > 0 {
            // Create progress bar
            let progress = indicatif::ProgressBar::new(total as u64);
            progress.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files processed ({eta}) {msg}")
                    .unwrap()
                    .progress_chars("#>-")
            );
            
            // Determine batch size based on number of files
            let batch_size = if total < 50 { 10 } else if total < 200 { 20 } else { 50 };
            
            // Set up shared state for parallel processing
            let processed = Arc::new(AtomicUsize::new(0));
            let interrupt = Arc::new(AtomicBool::new(false));
            
            // Create channels for processing results
            let (tx, rx) = std::sync::mpsc::channel();
            
            // Process embeddings in parallel
            let num_threads = rayon::current_num_threads();
            progress.set_message(format!("(using {} threads)", num_threads));
            
            // Clone references needed for parallel processing
            let progress_clone = progress.clone();
            let processed_clone = processed.clone();
            let interrupt_clone = interrupt.clone();
            
            // Prepare a safe clone of all necessary components to enable parallel processing
            let embedding_model_type = self.embedding_model_type.clone();
            let onnx_model_path = self.onnx_model_path.clone();
            let onnx_tokenizer_path = self.onnx_tokenizer_path.clone();
            let cache = self.cache.clone();
            
            // Launch parallel processing
            let processor_handle = std::thread::spawn(move || {
                // Process files in parallel
                filtered_files.par_iter().for_each(|file_path| {
                    // Check for interruption
                    if unsafe { crate::cli::commands::INTERRUPT_RECEIVED } || interrupt_clone.load(Ordering::SeqCst) {
                        return;
                    }
                    
                    let file_path_str = file_path.to_string_lossy().to_string();
                    
                    // Try to load from cache first to avoid redundant work
                    let cached_embedding = cache.get(&file_path_str);
                    
                    // If not in cache, generate new embedding
                    let result = if let Some(embedding) = cached_embedding {
                        // Found in cache
                        debug!("Cache hit for file: {}", file_path_str);
                        Ok((file_path.clone(), embedding.to_vec(), true))
                    } else {
                        // Not in cache, generate new embedding
                        debug!("Generating embedding for file: {}", file_path_str);
                        
                        // Create a model based on the configured type
                        let model = match embedding_model_type {
                            EmbeddingModelType::Fast => {
                                Ok(EmbeddingModel::new())
                            },
                            EmbeddingModelType::Onnx => {
                                if let (Some(model_path), Some(tokenizer_path)) = (&onnx_model_path, &onnx_tokenizer_path) {
                                    EmbeddingModel::new_onnx(model_path, tokenizer_path)
                                } else {
                                    // Fallback to fast model if paths aren't set
                                    Ok(EmbeddingModel::new())
                                }
                            }
                        };
                        
                        // If model creation failed, return the error
                        let model = match model {
                            Ok(m) => m,
                            Err(e) => {
                                return tx.send(Err((file_path_str, VectorDBError::EmbeddingError(e.to_string())))).unwrap();
                            }
                        };
                        
                        // Read the file contents
                        match fs::read_to_string(file_path) {
                            Ok(contents) => {
                                // Generate embedding
                                match model.embed(&contents) {
                                    Ok(embedding) => {
                                        // Calculate file hash for caching
                                        match EmbeddingCache::get_file_hash(file_path) {
                                            Ok(file_hash) => {
                                                // Successfully generated embedding
                                                Ok((file_path.clone(), embedding, false))
                                            },
                                            Err(e) => {
                                                Err((file_path_str, VectorDBError::EmbeddingError(e.to_string())))
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        Err((file_path_str, VectorDBError::EmbeddingError(e.to_string())))
                                    }
                                }
                            },
                            Err(e) => {
                                Err((file_path_str, VectorDBError::IOError(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))))
                            }
                        }
                    };
                    
                    // Send result back to main thread
                    tx.send(result).unwrap();
                    
                    // Update progress
                    let count = processed_clone.fetch_add(1, Ordering::SeqCst) + 1;
                    progress_clone.set_position(count as u64);
                    
                    // Show progress message occasionally but don't overwhelm the output
                    if count % 10 == 0 || count == total {
                        progress_clone.set_message(format!("(using {} threads)", num_threads));
                    }
                });
            });
            
            // Track embeddings added so far
            let mut successful_embeddings = 0;
            let mut save_counter = 0;
            let start_time = std::time::Instant::now();
            let mut last_report_time = start_time;
            
            // Process results from worker threads
            for _ in 0..total {
                match rx.recv() {
                    Ok(result) => {
                        match result {
                            Ok((file_path, embedding, from_cache)) => {
                                // Add to our database
                                let file_path_str = file_path.to_string_lossy().to_string();
                                
                                // Store in database (cache was already updated in the worker thread if needed)
                                self.embeddings.insert(file_path_str.clone(), embedding.clone());
                                
                                // Add to HNSW index if available
                                if let Some(index) = &mut self.hnsw_index {
                                    if let Err(e) = index.insert(embedding) {
                                        error!("Failed to insert into HNSW index: {}", e);
                                        progress.println(format!("Warning: Failed to add {} to HNSW index: {}", file_path_str, e));
                                    }
                                }
                                
                                successful_embeddings += 1;
                                
                                // Print occasional progress for large batches
                                let now = std::time::Instant::now();
                                if now.duration_since(last_report_time).as_secs() >= 10 {
                                    last_report_time = now;
                                    let elapsed = now.duration_since(start_time).as_secs();
                                    let rate = if elapsed > 0 { successful_embeddings as f64 / elapsed as f64 } else { 0.0 };
                                    progress.println(format!("Processed {}/{} files ({:.1} files/sec)", 
                                        successful_embeddings, total, rate));
                                }
                                
                                // Save periodically
                                save_counter += 1;
                                if save_counter >= batch_size {
                                    // Save the database and reset counter
                                    if let Err(e) = self.save() {
                                        error!("Failed to save database during batch processing: {}", e);
                                        progress.println(format!("Warning: Failed to save database: {}", e));
                                    } else {
                                        debug!("Saved database after processing {} files", save_counter);
                                    }
                                    save_counter = 0;
                                }
                            },
                            Err((file_path, error)) => {
                                // Show error but continue with other files
                                progress.println(format!("Error indexing {}: {}", file_path, error));
                            }
                        }
                    },
                    Err(e) => {
                        error!("Channel error: {}", e);
                        progress.println(format!("Error communicating with worker threads: {}", e));
                        break;
                    }
                }
            }
            
            // Wait for the processor thread to finish
            processor_handle.join().unwrap();
            
            // Final save if any unsaved changes
            if save_counter > 0 {
                if let Err(e) = self.save() {
                    error!("Failed to save database at end of incremental indexing: {}", e);
                    progress.println(format!("Warning: Failed to save database: {}", e));
                }
            }
            
            // Report final statistics
            let elapsed = start_time.elapsed().as_secs();
            let rate = if elapsed > 0 { successful_embeddings as f64 / elapsed as f64 } else { 0.0 };
            
            progress.finish_with_message(format!("Processed {} files successfully in {}s ({:.1} files/sec)", 
                successful_embeddings, elapsed, rate));
        }
        
        // Process deleted files
        if !changes.deleted_files.is_empty() {
            println!("Processing {} deleted files...", changes.deleted_files.len());
            for file_path in &changes.deleted_files {
                debug!("Removing deleted file from index: {}", file_path.display());
                let path_str = file_path.to_string_lossy().to_string();
                self.embeddings.remove(&path_str);
                
                // Also remove from HNSW index if present
                // Note: We'd need to rebuild HNSW index to fully remove it, which is expensive
                // So just flag that we need to rebuild on next save
                if let Some(idx) = &mut self.hnsw_index {
                    idx.mark_dirty();
                }
            }
        }
        
        // Update the indexed commit hash
        self.repo_manager.update_indexed_commit(&repo_id, &branch, &current_commit)?;
        
        // Save the updates
        self.save()?;
        
        info!("Repository {} branch {} indexed successfully", repo_name, branch);
        
        Ok(())
    }
    
    /// Start the auto-sync daemon
    pub fn start_auto_sync(&mut self) -> Result<()> {
        // Check if we have any repositories with auto-sync enabled
        let has_auto_sync_repos = !self.repo_manager.get_auto_sync_repos().is_empty();
        
        if has_auto_sync_repos {
            debug!("Starting auto-sync daemon");
            
            // Clone self to use for auto-sync daemon
            let db_clone = self.clone();
            
            // Create and start auto-sync daemon
            let mut daemon = AutoSyncDaemon::new(db_clone);
            daemon.start()?;
            
            // Store daemon
            self.auto_sync_daemon = Some(daemon);
            
            info!("Auto-sync daemon started");
        } else {
            debug!("No repositories with auto-sync enabled, not starting daemon");
        }
        
        Ok(())
    }
    
    /// Stop the auto-sync daemon
    pub fn stop_auto_sync(&mut self) -> Result<()> {
        if let Some(mut daemon) = self.auto_sync_daemon.take() {
            debug!("Stopping auto-sync daemon");
            daemon.stop()?;
            info!("Auto-sync daemon stopped");
        }
        
        Ok(())
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

impl Clone for DBStats {
    fn clone(&self) -> Self {
        Self {
            indexed_files: self.indexed_files,
            embedding_dimension: self.embedding_dimension,
            db_path: self.db_path.clone(),
            cached_files: self.cached_files,
            hnsw_stats: self.hnsw_stats.clone(),
            embedding_model_type: self.embedding_model_type.clone(),
        }
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
