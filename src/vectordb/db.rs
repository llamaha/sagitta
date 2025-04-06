use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use walkdir::WalkDir;
use crate::vectordb::embedding::{EmbeddingModel, EmbeddingModelType, EMBEDDING_DIM};
use crate::vectordb::onnx::ONNX_EMBEDDING_DIM;
use crate::vectordb::cache::{EmbeddingCache, CacheCheckResult};
use crate::vectordb::error::{Result, VectorDBError};
use crate::vectordb::hnsw::{HNSWIndex, HNSWConfig, HNSWStats};
use std::sync::{Arc, Mutex};
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::atomic::AtomicBool;
use std::io::Read;
use log::{debug, info, warn, error};
use crate::vectordb::repo_manager::RepoManager;
use crate::vectordb::auto_sync::AutoSyncDaemon;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::iter::ParallelIterator; // Ensure ParallelIterator is imported
use std::sync::mpsc::{self, Receiver, Sender}; // Ensure mpsc is imported
use num_cpus;

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
    pub fn create_embedding_model(&self) -> Result<EmbeddingModel> {
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
        
        // Create the embedding model
        let model = Arc::new(self.create_embedding_model()?);
        
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
        
        // Use the parallel implementation
        self.index_directory_parallel(files, model, batch_size)?;
        
        // Update repository information if needed
        if let (Some(repo_id), Some(branch)) = (&self.current_repo_id, &self.current_branch) {
            if let Some(repo) = self.repo_manager.get_repository(repo_id) {
                if let Ok(git_repo) = crate::utils::git::GitRepo::new(repo.path.clone()) {
                    if let Ok(commit_hash) = git_repo.get_commit_hash(branch) {
                        if let Err(e) = self.repo_manager.update_indexed_commit(repo_id, branch, &commit_hash) {
                            error!("Failed to update indexed commit: {}", e);
                        }
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
            // Programming languages with full parsers
            "rs".to_string(),  // Rust
            "rb".to_string(),  // Ruby
            "go".to_string(),  // Go
            "js".to_string(),  // JavaScript
            "ts".to_string(),  // TypeScript
            // Documentation (regex-based parsing)
            "md".to_string(),   // Markdown
            // Configuration (basic parsing)
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
        let _repo_active_branch = repo.active_branch.clone();
        
        // Store the previous branch before switching
        let _previous_branch = self.current_branch.clone();
        
        // If we're already in this repository and branch, nothing to do
        if self.current_repo_id.as_deref() == Some(repo_id) && 
           self.current_branch.as_deref() == Some(&branch_name) {
            debug!("Already in repository {} branch {}", repo_id, branch_name);
            return Ok(());
        }
        
        // Save current state if we're in a repository context
        if let (Some(current_repo), Some(current_branch)) = (&self.current_repo_id, &self.current_branch) {
            debug!("Saving state of current repository {}, branch {}", current_repo, current_branch);
            
            // If we're switching branches in the same repository, update branch relationship
            if current_repo == repo_id && current_branch != &branch_name {
                debug!("Switching branches within repository: {} -> {}", current_branch, branch_name);
                
                // Create git repo to find common ancestor
                if let Ok(git_repo) = crate::utils::git::GitRepo::new(repo_path.clone()) {
                    // Try to find the common ancestor
                    if let (Some(prev_commit), Ok(curr_commit)) = (
                        self.repo_manager.get_repository(repo_id)
                            .and_then(|r| r.get_indexed_commit(current_branch)),
                        git_repo.get_commit_hash(&branch_name)
                    ) {
                        // Find common ancestor
                        if let Ok(common_ancestor) = git_repo.find_common_ancestor(prev_commit, &curr_commit) {
                            debug!("Found common ancestor between branches {} and {}: {}", 
                                  current_branch, branch_name, common_ancestor);
                            
                            // Update branch relationship
                            if let Err(e) = self.repo_manager.update_branch_relationship(
                                repo_id, current_branch, &branch_name, &common_ancestor
                            ) {
                                warn!("Failed to update branch relationship: {}", e);
                            }
                        }
                    }
                }
            }
            
            // Save current state (this will also update the active branch)
            if let Err(e) = self.repo_manager.set_active_repository(repo_id) {
                warn!("Failed to update active repository: {}", e);
            }
        }
        
        // Set the current repository and branch
        self.current_repo_id = Some(repo_id.to_string());
        self.current_branch = Some(branch_name.clone());
        
        // Create the repository cache path if it doesn't exist
        let cache_path = self.get_cache_path_for_repo(repo_id, &branch_name);
        if let Some(parent) = cache_path.parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    warn!("Failed to create repository cache directory: {}", e);
                }
            }
        }
        
        // Update the active branch in the repository config
        let mut repo_updated = false;
        {
            if let Some(repo) = self.repo_manager.get_repository_mut(repo_id) {
                if repo.active_branch != branch_name {
                    debug!("Updating active branch for repository {}: {} -> {}", 
                          repo_id, repo.active_branch, branch_name);
                    repo.active_branch = branch_name.clone();
                    repo_updated = true;
                }
            }
        }
        
        // Save repository config changes
        if repo_updated {
            if let Err(e) = self.repo_manager.save() {
                warn!("Failed to save repository changes: {}", e);
            }
        }
        
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
        
        // Display a message about indexing progress
        println!("Starting indexing... The progress bar will appear shortly");
        
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
                branch.to_string(),         // branch
                repo.active_branch.clone()  // previous active branch
            )
        };
        
        let (repo_path, repo_name, last_commit, file_types, repo_id, branch, prev_active_branch) = repo_data;
        
        // Create a git repository object
        let git_repo = crate::utils::git::GitRepo::new(repo_path.clone())
            .map_err(|e| VectorDBError::RepositoryError(e.to_string()))?;
        
        // Get the current commit hash
        let current_commit = git_repo.get_commit_hash(&branch)
            .map_err(|e| VectorDBError::RepositoryError(e.to_string()))?;
        
        // If no previous commit for this branch, check if we're switching branches
        // and can do an efficient cross-branch sync
        if last_commit.is_none() && branch != prev_active_branch {
            info!("No previous index found for branch {}, checking for cross-branch sync from {}",
                 branch, prev_active_branch);
            println!("Attempting efficient cross-branch sync from '{}' to '{}'...", prev_active_branch, branch);
            
            // Check if the previous branch was indexed
            let prev_branch_commit = self.repo_manager.get_repository(&repo_id)
                .and_then(|repo| repo.get_indexed_commit(&prev_active_branch).cloned());
            
            if let Some(prev_commit) = prev_branch_commit {
                println!("Previous branch '{}' is indexed at commit {}...", prev_active_branch, &prev_commit[..8]);
                // We have a previous commit in the previous branch, try to find common ancestor
                match git_repo.find_common_ancestor(&prev_commit, &current_commit) {
                    Ok(common_ancestor) => {
                        info!("Found common ancestor between branches: {}", common_ancestor);
                        println!("Found common ancestor commit: {}...", &common_ancestor[..8]);
                        
                        // Update branch relationship information
                        if let Err(e) = self.repo_manager.update_branch_relationship(
                            &repo_id, &prev_active_branch, &branch, &common_ancestor
                        ) {
                            warn!("Failed to update branch relationship: {}", e);
                        }
                        
                        // Switch to this repository context
                        self.switch_repository(&repo_id, Some(&branch))?;
                        
                        // Get changes since the common ancestor
                        info!("Performing efficient cross-branch sync from {} to {}", 
                              prev_active_branch, branch);
                        
                        let changes = git_repo.get_cross_branch_changes(&common_ancestor, &current_commit)
                            .map_err(|e| VectorDBError::RepositoryError(e.to_string()))?;
                        
                        let total_changes = changes.added_files.len() + changes.modified_files.len() + changes.deleted_files.len();
                        
                        if total_changes == 0 {
                            info!("No changes detected between branches");
                            println!("No changes detected between branches that need indexing.");
                            
                            // Still need to update the indexed commit hash
                            self.repo_manager.update_indexed_commit(&repo_id, &branch, &current_commit)?;
                            self.save()?;
                            
                            info!("Repository {} branch {} indexed successfully (no changes)", 
                                repo_name, branch);
                            return Ok(());
                        }
                        
                        info!("Cross-branch changes detected: {} added, {} modified, {} deleted files",
                             changes.added_files.len(), changes.modified_files.len(), changes.deleted_files.len());
                        
                        println!("Found {} changes between branches:", total_changes);
                        println!("  - {} files added", changes.added_files.len());
                        println!("  - {} files modified", changes.modified_files.len());
                        println!("  - {} files deleted", changes.deleted_files.len());
                        
                        // Process added and modified files
                        let files_to_process = [&changes.added_files[..], &changes.modified_files[..]].concat();
                        
                        // Filter for file types we care about
                        let filtered_files: Vec<PathBuf> = files_to_process.iter()
                            .filter(|path| {
                                if let Some(ext) = path.extension() {
                                    let ext_str = ext.to_string_lossy().to_string();
                                    file_types.contains(&ext_str)
                                } else {
                                    false
                                }
                            })
                            .cloned()
                            .collect();
                        
                        let relevant_file_count = filtered_files.len();
                        
                        if relevant_file_count == 0 {
                            info!("No relevant files found to index after filtering");
                            println!("No files with relevant extensions found among the changes.");
                            
                            // Update commit hash and save even though no files were processed
                            self.repo_manager.update_indexed_commit(&repo_id, &branch, &current_commit)?;
                            self.save()?;
                            
                            info!("Repository {} branch {} indexed successfully (no relevant files)", 
                                repo_name, branch);
                            return Ok(());
                        }
                        
                        info!("Indexing {} relevant files", relevant_file_count);
                        println!("Processing {} changed files with relevant extensions...", relevant_file_count);
                        
                        // Use the parallel implementation for efficiency
                        let model = Arc::new(self.create_embedding_model()?);
                        let batch_size = std::cmp::min(20, relevant_file_count);
                        
                        // Use the parallel implementation
                        self.index_directory_parallel(filtered_files, model, batch_size)?;
                        
                        // Handle deleted files
                        if !changes.deleted_files.is_empty() {
                            info!("Processing {} deleted files", changes.deleted_files.len());
                            println!("Removing {} deleted files from index...", changes.deleted_files.len());
                            
                            for file_path in &changes.deleted_files {
                                debug!("Removing deleted file from index: {}", file_path.display());
                                let path_str = file_path.to_string_lossy().to_string();
                                self.embeddings.remove(&path_str);
                                
                                // Also remove from HNSW index if present
                                if let Some(idx) = &mut self.hnsw_index {
                                    idx.mark_dirty();
                                }
                            }
                        }
                        
                        // Update the indexed commit hash
                        self.repo_manager.update_indexed_commit(&repo_id, &branch, &current_commit)?;
                        
                        // Save the updates
                        self.save()?;
                        
                        info!("Repository {} branch {} indexed successfully via cross-branch sync", 
                              repo_name, branch);
                        
                        println!("Branch '{}' successfully synced from '{}' based on {} changes", 
                                branch, prev_active_branch, total_changes);
                        
                        return Ok(());
                    },
                    Err(e) => {
                        warn!("Failed to find common ancestor between branches: {}", e);
                        println!("Could not find common ancestor between branches: {}", e);
                        println!("Falling back to full indexing...");
                        // Fall back to full index
                    }
                }
            } else {
                println!("Previous branch '{}' is not indexed yet, cannot use for efficient syncing.", 
                      prev_active_branch);
            }
            
            // If we reach here, we couldn't do an efficient cross-branch sync, so do a full index
            info!("No previous index found for repository {} branch {}, doing full index",
                 repo_name, branch);
            println!("Performing full indexing of branch '{}'...", branch);
            return self.index_repository_full(&repo_id, &branch);
        } else if last_commit.is_none() {
            // No previous commit and not switching branches, do a full index
            info!("No previous index found for repository {} branch {}, doing full index",
                 repo_name, branch);
            return self.index_repository_full(&repo_id, &branch);
        }
        
        let last_commit = last_commit.unwrap();
        
        // If commits are the same, nothing to do
        if last_commit == current_commit {
            info!("Repository {} branch {} is already up to date at commit {}",
                 repo_name, branch, current_commit);
            println!("Branch is already at latest commit ({}) - no changes to index.", &current_commit[..8]);
            return Ok(());
        }
        
        // Switch to this repository context
        self.switch_repository(&repo_id, Some(&branch))?;
        
        // Get the change set between commits
        let changes = git_repo.get_change_set(&last_commit, &current_commit)
            .map_err(|e| VectorDBError::RepositoryError(e.to_string()))?;
        
        info!("Incremental indexing of repository {} branch {} from commit {} to {}",
             repo_name, branch, last_commit, current_commit);
        
        let total_changes = changes.added_files.len() + changes.modified_files.len() + changes.deleted_files.len();
        println!("Found {} changes between commits {}.. and {}..:", 
                total_changes, &last_commit[..8], &current_commit[..8]);
        println!("  - {} files added", changes.added_files.len());
        println!("  - {} files modified", changes.modified_files.len());
        println!("  - {} files deleted", changes.deleted_files.len());
        
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
            let progress = ProgressBar::new(total as u64);
            progress.set_style(ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files processed ({eta}) {msg}")
                .unwrap()
                .progress_chars("#>-"));
            
            // Determine batch size based on number of files
            let batch_size = if total < 50 { 10 } else if total < 200 { 20 } else { 50 };
            
            // Create the embedding model
            let model = Arc::new(self.create_embedding_model()?);
            
            // Use the parallel implementation
            self.index_directory_parallel(filtered_files, model, batch_size)?;
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

    /// Parallel directory indexing function
    fn index_directory_parallel(&mut self, files: Vec<PathBuf>, model: Arc<EmbeddingModel>, batch_size: usize) -> Result<()> {
        let num_threads = num_cpus::get();
        let file_count = files.len();
        
        info!("Starting parallel indexing for {} files using {} threads.", file_count, num_threads);

        // Create progress bar for embedded files
        let progress = ProgressBar::new(file_count as u64);
        progress.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files indexed ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-"));

        // Create thread-safe cache reference
        let cache_arc = Arc::new(Mutex::new(self.cache.clone()));
        
        // First phase: Check cache for all files and collect those needing embedding
        let mut files_to_embed: Vec<(PathBuf, Option<u64>)> = Vec::with_capacity(file_count);
        let mut files_from_cache = 0;
        let mut cache_errors = 0;
        
        debug!("Checking cache for {} files...", file_count);
        { // Scope for cache_guard lock
            let mut cache_guard = cache_arc.lock().unwrap();
            for file_path in &files {
                let path_str = file_path.to_string_lossy().to_string();
                match cache_guard.check_cache_and_get_hash(&path_str, file_path) {
                    Ok(CacheCheckResult::Hit(embedding)) => {
                        // Release cache lock before potentially locking HNSW index
                        drop(cache_guard);
                        self.embeddings.insert(path_str, embedding.clone());
                        if let Some(index) = &mut self.hnsw_index {
                            if let Err(e) = index.insert(embedding) {
                                error!("Failed to insert cached embedding into HNSW index for {}: {}", file_path.display(), e);
                            }
                        }
                        files_from_cache += 1;
                        progress.inc(1); // Increment progress for cached file
                        cache_guard = cache_arc.lock().unwrap(); // Re-acquire lock
                    }
                    Ok(CacheCheckResult::Miss(hash_opt)) => {
                        files_to_embed.push((file_path.clone(), hash_opt));
                    }
                    Err(e) => {
                        error!("Cache check/hash error for {}: {}. Queuing for embedding.", file_path.display(), e);
                        cache_errors += 1;
                        files_to_embed.push((file_path.clone(), None));
                    }
                }
            }
        } // cache_guard lock released here

        if cache_errors > 0 {
            progress.println(format!("Warning: Encountered {} cache read/hash errors.", cache_errors));
        }
        
        let files_to_embed_count = files_to_embed.len();
        debug!("Found {} files in cache. {} files need embedding.", files_from_cache, files_to_embed_count);

        if files_to_embed_count == 0 {
            progress.finish_with_message(format!("Processed {} files (all from cache).", files_from_cache));
            if let Err(e) = self.save() {
                error!("Failed to save database after cache check: {}", e);
            }
            return Ok(());
        }

        // Second phase: Prepare for batched parallel processing
        let model_clone = Arc::clone(&model);
        let effective_batch_size = std::cmp::min(batch_size, 128); // Cap batch size
        let file_chunks: Vec<Vec<(PathBuf, Option<u64>)>> = files_to_embed
            .chunks(effective_batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();
        
        let chunk_count = file_chunks.len();
        info!("Processing {} files needing embedding in {} chunks (batch size ~{}).", 
              files_to_embed_count, chunk_count, effective_batch_size);

        // Print an initial progress message
        progress.println(format!(
            "Scanning and preparing {} files for embedding...",
            files_to_embed_count
        ));

        // Third phase: Parallel processing with rayon
        let pool = rayon::ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();
        let (tx, rx): (Sender<Vec<(PathBuf, Option<u64>, Result<Vec<f32>>)>>, Receiver<_>) = mpsc::channel();

        // Start a separate thread to report progress periodically, even if processing is slow
        let progress_clone = progress.clone();
        let processed = Arc::new(AtomicUsize::new(0));
        let processed_clone = processed.clone();
        let should_continue = Arc::new(AtomicBool::new(true));
        let should_continue_clone = should_continue.clone();
        let files_to_embed_count_clone = files_to_embed_count;
        
        std::thread::spawn(move || {
            let mut last_count = 0;
            let start = std::time::Instant::now();
            
            // Set initial progress bar message
            progress_clone.set_message("Starting file processing...");
            
            while should_continue_clone.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let current = processed_clone.load(Ordering::SeqCst);
                
                if current > last_count {
                    // Processing is happening, update the progress bar
                    let progress_delta = current - last_count;
                    let elapsed = start.elapsed().as_secs_f32();
                    let rate = if elapsed > 0.1 { current as f32 / elapsed } else { 0.0 };
                    
                    // Update the progress bar with rate information
                    progress_clone.set_message(format!(
                        "Processing files ({:.1} files/sec)",
                        rate
                    ));
                    
                    // Also update the position
                    progress_clone.set_position(current as u64);
                    
                    last_count = current;
                } else if current > 0 {
                    // No new progress, but processing has started
                    progress_clone.set_message("Processing (waiting for results)...");
                } else {
                    // No progress at all yet
                    progress_clone.set_message("Preparing for embedding...");
                }
            }
        });

        pool.install(move || {
            file_chunks.into_par_iter().for_each_with(tx, |tx_clone, chunk_data| {
                let mut texts_to_embed: Vec<String> = Vec::with_capacity(chunk_data.len());
                let mut paths_and_hashes: Vec<(PathBuf, Option<u64>)> = Vec::with_capacity(chunk_data.len());
                let mut results_for_chunk: Vec<(PathBuf, Option<u64>, Result<Vec<f32>>)> = Vec::with_capacity(chunk_data.len());

                // Read file contents
                for (path, hash_opt) in chunk_data {
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            texts_to_embed.push(content);
                            paths_and_hashes.push((path, hash_opt));
                        }
                        Err(e) => {
                            results_for_chunk.push((path, hash_opt, Err(VectorDBError::IOError(e))));
                        }
                    }
                }

                // Embed batch
                if !texts_to_embed.is_empty() {
                    let text_refs: Vec<&str> = texts_to_embed.iter().map(String::as_str).collect();
                    match model_clone.embed_batch(&text_refs) {
                        Ok(embeddings) => {
                            assert_eq!(embeddings.len(), paths_and_hashes.len());
                            for ((path, hash_opt), embedding) in paths_and_hashes.into_iter().zip(embeddings) {
                                results_for_chunk.push((path, hash_opt, Ok(embedding)));
                            }
                        }
                        Err(e) => {
                            let error_message = format!("Batch embedding failed: {}", e);
                            for (path, hash_opt) in paths_and_hashes {
                                results_for_chunk.push((path, hash_opt, Err(VectorDBError::EmbeddingError(error_message.clone()))));
                            }
                        }
                    }
                }

                // Send results
                if !results_for_chunk.is_empty() {
                    // Update processed count before sending
                    processed.fetch_add(results_for_chunk.len(), Ordering::SeqCst);
                    
                    if tx_clone.send(results_for_chunk).is_err() {
                        warn!("Failed to send chunk results to main thread.");
                    }
                }
            });
        }); // pool.install blocks until completion

        // Signal the progress thread to stop
        should_continue.store(false, Ordering::SeqCst);

        // Fourth phase: Process results
        let mut successful_embeddings = 0;
        let mut save_counter = 0;
        let start_time = std::time::Instant::now();
        let mut last_report_time = start_time;
        let mut processed_new_files = 0;

        while let Ok(chunk_results) = rx.recv() {
            let mut cache_guard = cache_arc.lock().unwrap();
            for (file_path, hash_opt, result) in chunk_results {
                processed_new_files += 1;
                match result {
                    Ok(embedding) => {
                        let file_path_str = file_path.to_string_lossy().to_string();
                        // Cache insert logic
                        if let Some(hash) = hash_opt {
                            if let Err(e) = cache_guard.insert_with_hash(file_path_str.clone(), embedding.clone(), hash) {
                                error!("Failed to insert into cache for {}: {}", file_path.display(), e);
                            }
                        } else {
                            // Try getting hash again if it wasn't available initially
                            match EmbeddingCache::get_file_hash(&file_path) {
                                Ok(new_hash) => {
                                    if let Err(e) = cache_guard.insert_with_hash(file_path_str.clone(), embedding.clone(), new_hash) {
                                        error!("Failed to insert into cache (retry hash) for {}: {}", file_path.display(), e);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed again to get hash for cache insertion for {}: {}", file_path.display(), e);
                                }
                            }
                        }
                        
                        // DB insert
                        self.embeddings.insert(file_path_str.clone(), embedding.clone());
                        
                        // HNSW insert (outside lock to avoid deadlock)
                        drop(cache_guard);
                        if let Some(index) = &mut self.hnsw_index {
                            if let Err(e) = index.insert(embedding) {
                                error!("Failed to insert into HNSW index for {}: {}", file_path.display(), e);
                                progress.println(format!("Warning: Failed to add {} to HNSW index: {}", file_path_str, e));
                            }
                        }
                        cache_guard = cache_arc.lock().unwrap(); // Re-acquire lock

                        successful_embeddings += 1;
                        save_counter += 1;
                    }
                    Err(error) => {
                        progress.println(format!("Error indexing {}: {}", file_path.display(), error));
                    }
                }
                progress.inc(1); // Increment progress bar for each processed file
            }
            drop(cache_guard); // Release lock after processing the chunk

            // Periodic save
            if save_counter >= effective_batch_size {
                if let Err(e) = self.save() {
                    error!("Failed to save database during batch processing: {}", e);
                    progress.println(format!("Warning: Failed to save database: {}", e));
                } else {
                    debug!("Saved database after processing {} new files", save_counter);
                }
                save_counter = 0;
            }

            // Periodic progress report - more frequent (every 5 seconds)
            let now = std::time::Instant::now();
            if now.duration_since(last_report_time).as_secs() >= 5 {
                last_report_time = now;
                let elapsed_secs = now.duration_since(start_time).as_secs_f32();
                let rate = if elapsed_secs > 0.1 { processed_new_files as f32 / elapsed_secs } else { 0.0 };
                let total_processed = files_from_cache + processed_new_files;
                
                // Update progress bar message instead of printing
                progress.set_message(format!(
                    "Storing embeddings ({:.1} files/sec)",
                    rate
                ));
            }
        }

        // Fifth phase: Finalization
        if save_counter > 0 {
            if let Err(e) = self.save() {
                error!("Failed to save database at end of indexing: {}", e);
                progress.println(format!("Warning: Failed to save database: {}", e));
            } else {
                debug!("Final save completed.");
            }
        }

        let elapsed_total = start_time.elapsed().as_secs_f32();
        let rate_final = if elapsed_total > 0.1 { successful_embeddings as f32 / elapsed_total } else { 0.0 };
        let total_files_in_db = files_from_cache + successful_embeddings;
        
        // Show breakdown of files processed
        if files_from_cache > 0 {
            progress.finish_with_message(format!(
                "Indexed {} files ({} new embeddings, {} from cache) in {:.1}s ({:.1} new/sec)",
                total_files_in_db, successful_embeddings, files_from_cache, elapsed_total, rate_final
            ));
        } else {
            progress.finish_with_message(format!(
                "Indexed {} files ({} new embeddings) in {:.1}s ({:.1} new/sec)",
                total_files_in_db, successful_embeddings, elapsed_total, rate_final
            ));
        }

        // Update commit hash if in repo context
        if let (Some(repo_id), Some(branch)) = (&self.current_repo_id, &self.current_branch) {
            debug!("Updating commit hash for repository {} branch {}", repo_id, branch);
            let repo_id_clone = repo_id.clone();
            let branch_clone = branch.clone();
            if let Some(repo) = self.repo_manager.get_repository(&repo_id_clone) {
                if let Ok(git_repo) = crate::utils::git::GitRepo::new(repo.path.clone()) {
                    if let Ok(commit_hash) = git_repo.get_commit_hash(&branch_clone) {
                        if let Err(e) = self.repo_manager.update_indexed_commit(&repo_id_clone, &branch_clone, &commit_hash) {
                            error!("Failed to update indexed commit for {}/{}: {}", repo_id_clone, branch_clone, e);
                        }
                    } else {
                        warn!("Could not get current commit hash for {}/{}", repo_id_clone, branch_clone);
                    }
                } else {
                    warn!("Could not open git repo at {}", repo.path.display());
                }
            } else {
                warn!("Repository config not found for {}", repo_id_clone);
            }
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
    fn test_optimal_layer_count() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_string_lossy().to_string(); // Convert to String
        
        let _db = VectorDB::new(db_path).unwrap(); // Use the String
        
        let optimal = HNSWConfig::calculate_optimal_layers(1_000);
        assert!(optimal > 0);
        
        let optimal = HNSWConfig::calculate_optimal_layers(10_000);
        assert!(optimal > 0);
        
        let optimal = HNSWConfig::calculate_optimal_layers(100_000);
        assert!(optimal > 0);
    }
} 
