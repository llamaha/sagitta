use crate::vectordb::cache::{CacheCheckResult, EmbeddingCache};
use crate::vectordb::embedding::{EmbeddingModel, EmbeddingModelType};
use crate::vectordb::error::{Result, VectorDBError};
use crate::vectordb::hnsw::{HNSWConfig, HNSWIndex, HNSWStats};
use indicatif::{ProgressBar, ProgressStyle};
use indicatif::style::TemplateError;
use log::{debug, error, warn};
use rayon::iter::ParallelIterator;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, canonicalize};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use std::time::Instant;

// Add From implementation here
impl From<TemplateError> for VectorDBError {
    fn from(error: TemplateError) -> Self {
        VectorDBError::GeneralError(format!("Progress bar template error: {}", error))
    }
}

/// Relevance feedback data for a query-file pair
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FeedbackEntry {
    pub relevant_count: usize,
    pub irrelevant_count: usize,
    pub relevance_score: f32,
}

/// Collection of query feedback data
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FeedbackData {
    pub query_feedback: HashMap<String, HashMap<String, FeedbackEntry>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct DBFile {
    embeddings: HashMap<String, Vec<f32>>,
    hnsw_config: Option<HNSWConfig>,
    feedback: Option<FeedbackData>,
    embedding_model_type: Option<EmbeddingModelType>,
    onnx_model_path: Option<String>,
    onnx_tokenizer_path: Option<String>,
    #[serde(default)]
    indexed_roots: HashSet<String>,
}

pub struct VectorDB {
    pub embeddings: HashMap<String, Vec<f32>>,
    db_path: String,
    pub cache: EmbeddingCache,
    pub hnsw_index: Option<HNSWIndex>,
    feedback: FeedbackData,
    pub embedding_model_type: EmbeddingModelType,
    onnx_model_path: Option<PathBuf>,
    onnx_tokenizer_path: Option<PathBuf>,
    indexed_roots: HashSet<String>,
}

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
            indexed_roots: self.indexed_roots.clone(),
        }
    }
}

impl VectorDB {
    pub fn new(db_path: String) -> Result<Self> {
        debug!("Creating VectorDB with database path: {}", db_path);

        let (
            embeddings,
            hnsw_config,
            feedback,
            embedding_model_type,
            onnx_model_path,
            onnx_tokenizer_path,
            indexed_roots,
        ) = if Path::new(&db_path).exists() {
            debug!("Database file exists, attempting to load");
            match fs::read_to_string(&db_path) {
                Ok(contents) => {
                    debug!("Database file read successfully, parsing JSON");
                    let db_file: DBFile = serde_json::from_str(&contents)?;

                    // Determine model type - default to Onnx if missing or if Fast is found (treat Fast as Onnx now)
                    let loaded_model_type = db_file.embedding_model_type.unwrap_or(EmbeddingModelType::Onnx);
                    // if loaded_model_type == EmbeddingModelType::Fast { // Remove this check
                    //     warn!("Loaded DB file used deprecated Fast model type. Treating as Onnx.");
                    //     loaded_model_type = EmbeddingModelType::Onnx;
                    // }

                    debug!(
                        "Database parsed successfully: {} embeddings, {} indexed roots",
                        db_file.embeddings.len(),
                        db_file.indexed_roots.len()
                    );
                    (
                        db_file.embeddings,
                        db_file.hnsw_config,
                        db_file.feedback.unwrap_or_default(),
                        loaded_model_type,
                        db_file.onnx_model_path.map(PathBuf::from),
                        db_file.onnx_tokenizer_path.map(PathBuf::from),
                        db_file.indexed_roots,
                    )
                }
                Err(e) => {
                    error!("Couldn't read database file: {}", e);
                    eprintln!("Warning: Couldn't read database file: {}", e);
                    eprintln!("Creating a new empty database.");
                    debug!("Creating a new empty database");
                    (
                        HashMap::new(),
                        Some(HNSWConfig::default()),
                        FeedbackData::default(),
                        EmbeddingModelType::Onnx,
                        None,
                        None,
                        HashSet::new(),
                    )
                }
            }
        } else {
            debug!("Database file doesn't exist, creating new database");
            (
                HashMap::new(),
                Some(HNSWConfig::default()),
                FeedbackData::default(),
                EmbeddingModelType::Onnx,
                None,
                None,
                HashSet::new(),
            )
        };

        let cache_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("cache.json")
            .to_string_lossy()
            .to_string();
        debug!("Creating embedding cache at: {}", cache_path);

        let mut cache = match EmbeddingCache::new(cache_path.clone()) {
            Ok(cache) => {
                debug!("Cache loaded successfully");
                cache
            }
            Err(e) => {
                error!("Couldn't load cache: {}", e);
                eprintln!("Warning: Couldn't load cache: {}", e);
                eprintln!("Creating a new empty cache.");
                let _ = fs::remove_file(&cache_path);
                debug!("Creating a new empty cache");
                EmbeddingCache::new(cache_path)?
            }
        };

        debug!("Setting cache model type to: {:?}", embedding_model_type);
        cache.set_model_type(embedding_model_type.clone());

        let hnsw_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("hnsw_index.json");
        debug!("Looking for HNSW index at: {}", hnsw_path.display());

        let hnsw_index = if hnsw_path.exists() {
            debug!("HNSW index file exists, attempting to load");
            match HNSWIndex::load_from_file(&hnsw_path) {
                Ok(index) => {
                    // ** Check dimension compatibility with loaded embeddings/model type **
                    // We need the expected dimension here. Let's try getting it from the potentially loaded hnsw_config
                    // or fall back to the dimension associated with the loaded embedding_model_type.
                    // NOTE: This might still be imperfect if db.json/hnsw_config is missing/wrong,
                    // the definitive check will happen during indexing.
                    let expected_dim = hnsw_config.map(|c| c.dimension)
                        .unwrap_or_else(|| embedding_model_type.default_dimension()); // Helper needed

                    if index.get_config().dimension == expected_dim {
                         debug!("HNSW index loaded successfully with matching dimension {}", expected_dim);
                         Some(index)
                    } else {
                        warn!(
                            "Loaded HNSW index dimension ({}) does not match expected dimension ({}) based on db.json/model type. Discarding loaded index.",
                            index.get_config().dimension, expected_dim
                        );
                         let _ = fs::remove_file(&hnsw_path); // Remove incompatible index
                         None // Discard the loaded index
                    }
                }
                Err(e) => {
                    error!("Couldn't load HNSW index: {}. Discarding invalid index file.", e);
                    eprintln!("Warning: Couldn't load existing HNSW index: {}. It will be rebuilt on next index command.", e);
                    let _ = fs::remove_file(&hnsw_path); // Remove the corrupted file
                    None // Set to None, index will be created on demand later
                }
            }
        } else {
            debug!("No HNSW index file found. It will be created on the next index command.");
            None // Set to None, index will be created on demand later
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
            indexed_roots,
        })
    }

    pub fn set_onnx_paths(
        &mut self,
        model_path: Option<PathBuf>,
        tokenizer_path: Option<PathBuf>,
    ) -> Result<()> {
        if let Some(model_path) = &model_path {
            if !model_path.exists() {
                return Err(VectorDBError::EmbeddingError(format!(
                    "ONNX model file not found: {}",
                    model_path.display()
                )));
            }
        }
        if let Some(tokenizer_path) = &tokenizer_path {
            if !tokenizer_path.exists() {
                return Err(VectorDBError::EmbeddingError(format!(
                    "ONNX tokenizer file not found: {}",
                    tokenizer_path.display()
                )));
            }
        }

        if let (Some(model_path_ref), Some(tokenizer_path_ref)) = (&model_path, &tokenizer_path) {
            match EmbeddingModel::new_onnx(model_path_ref, tokenizer_path_ref) {
                Ok(_) => {
                    self.onnx_model_path = model_path;
                    self.onnx_tokenizer_path = tokenizer_path;
                    self.cache.set_model_type(EmbeddingModelType::Onnx);
                    self.cache.invalidate_different_model_types();
                    self.save()?;
                }
                Err(e) => {
                    return Err(VectorDBError::EmbeddingError(format!(
                        "Failed to initialize ONNX model with provided paths: {}",
                        e
                    )));
                }
            }
        } else {
            self.onnx_model_path = model_path;
            self.onnx_tokenizer_path = tokenizer_path;
            self.save()?;
        }
        Ok(())
    }

    pub fn create_embedding_model(&self) -> Result<EmbeddingModel> {
        if let (Some(model_path), Some(tokenizer_path)) =
            (&self.onnx_model_path, &self.onnx_tokenizer_path)
        {
            EmbeddingModel::new_onnx(model_path, tokenizer_path)
                .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))
        } else {
            Err(VectorDBError::EmbeddingError(
                "ONNX model paths not set. Required via set_onnx_model_paths or env vars.".to_string()
            ))
        }
    }

    pub fn index_directory(&mut self, dir_path: &str, file_patterns: &[String]) -> Result<()> {
        // Canonicalize the input directory path immediately
        let root_path = canonicalize(Path::new(dir_path)).map_err(|e| {
            VectorDBError::IndexingError(format!(
                "Failed to canonicalize root directory {}: {}",
                dir_path, e
            ))
        })?;
        let root_path_str = root_path.to_string_lossy().to_string();
        debug!(
            "Starting indexing for canonical path: {}",
            root_path_str
        );

        let model = self.create_embedding_model()?;
        let model_arc = Arc::new(model);

        let file_list = self.collect_files(&root_path_str, file_patterns)?;

        if file_list.is_empty() {
            println!("No files matching the patterns found in {}.", root_path_str);
            return Ok(());
        }

        self.index_directory_parallel(file_list, model_arc, 10)?;
        self.save()?;

        Ok(())
    }

    fn collect_files(&self, canonical_dir_path: &str, file_patterns: &[String]) -> Result<Vec<PathBuf>> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} Collecting files... {pos} found")?,
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        let path = Path::new(canonical_dir_path);
        let mut files = Vec::new();
        let patterns: HashSet<_> = file_patterns.iter().map(|s| s.as_str()).collect();

        for entry in WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();
            if entry_path.is_file() {
                let extension = entry_path.extension().and_then(|s| s.to_str()).unwrap_or("");
                if patterns.is_empty() || patterns.contains(extension) {
                    // Canonicalize the found file path before adding it
                    match canonicalize(entry_path) {
                        Ok(canonical_entry_path) => {
                            files.push(canonical_entry_path);
                            pb.inc(1);
                        }
                        Err(e) => {
                            error!("Failed to canonicalize file path {}: {}. Skipping.", entry_path.display(), e);
                        }
                    }
                }
            }
        }

        pb.finish_with_message(format!("Collected {} files", files.len()));
        Ok(files)
    }

    pub fn save(&mut self) -> Result<()> {
        debug!("Attempting to save database to {}", self.db_path);

        // Make sure cache is saved
        if let Err(e) = self.cache.save() {
            error!("Failed to save cache: {}", e);
            // Decide if this should be a hard error for the main save operation
            // For now, log it and continue saving the main db file
        }

        // Optionally, rebuild HNSW index before saving if needed
        if self.hnsw_index.is_none() || self.hnsw_index.as_ref().unwrap().stats().total_nodes != self.embeddings.len() {
             debug!("Rebuilding HNSW index before saving...");
             match self.rebuild_hnsw_index() {
                  Ok(_) => debug!("HNSW index rebuilt successfully."),
                  Err(e) => {
                      error!("Failed to rebuild HNSW index: {}. Index may be outdated.", e);
                      // Continue saving without the updated index?
                      // Or return error? Let's return error for now.
                      return Err(VectorDBError::HNSWError(format!("Failed to rebuild HNSW index during save: {}", e)));
                  }
             }
        }

        // Save HNSW index if it exists
        if let Some(hnsw_index) = &self.hnsw_index {
            let hnsw_path = Path::new(&self.db_path)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
            debug!("Saving HNSW index to {}", hnsw_path.display());
            if let Err(e) = hnsw_index.save_to_file(&hnsw_path) {
                error!("Failed to save HNSW index: {}", e);
                return Err(VectorDBError::HNSWError(format!(
                    "Failed to save HNSW index to {}: {}",
                    hnsw_path.display(),
                    e
                )));
            }
        }

        // Save main DB file
        let db_file = DBFile {
            embeddings: self.embeddings.clone(),
            hnsw_config: self.hnsw_index.as_ref().map(|idx| idx.get_config()),
            feedback: Some(self.feedback.clone()),
            embedding_model_type: Some(self.embedding_model_type),
            onnx_model_path: self.onnx_model_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            onnx_tokenizer_path: self.onnx_tokenizer_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            indexed_roots: self.indexed_roots.clone(),
        };

        let contents = serde_json::to_string_pretty(&db_file)?;
        debug!("Serialized DB file size: {} bytes", contents.len());
        fs::write(&self.db_path, contents)?;

        debug!("Database saved successfully to {}", self.db_path);
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.embeddings.clear();
        self.cache.clear();
        self.hnsw_index = None; // Clear the HNSW index object
        self.feedback = FeedbackData::default();
        self.indexed_roots.clear();

        // Delete the persistent files
        let _ = fs::remove_file(&self.db_path); // Ignore error if file doesn't exist
        // Ignore result of cache clear, best effort
        let _ = self.cache.clear(); 
        let cache_path = Path::new(&self.db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("cache.json");
        let _ = fs::remove_file(&cache_path);
        let hnsw_path = Path::new(&self.db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("hnsw_index.json");
        let _ = fs::remove_file(&hnsw_path);

        self.save() // Save the now empty state (optional, but consistent)
    }

    pub fn stats(&self) -> DBStats {
        let embedding_dimension = self.create_embedding_model()
            .map(|model| model.dim())
            .unwrap_or(0);

        DBStats {
            indexed_files: self.embeddings.len(),
            embedding_dimension,
            db_path: self.db_path.clone(),
            cached_files: self.cache.len(),
            hnsw_stats: self.hnsw_index.as_ref().map(|index| index.stats()),
            embedding_model_type: self.embedding_model_type.clone(),
        }
    }

    pub fn onnx_model_path(&self) -> Option<&PathBuf> {
        self.onnx_model_path.as_ref()
    }

    pub fn onnx_tokenizer_path(&self) -> Option<&PathBuf> {
        self.onnx_tokenizer_path.as_ref()
    }

    pub fn hnsw_index(&self) -> Option<&HNSWIndex> {
        if let Some(index) = &self.hnsw_index {
            debug!( "HNSW index accessed: {} nodes, {} layers", index.stats().total_nodes, index.stats().layers );
            Some(index)
        } else {
            debug!("HNSW index requested but not available");
            None
        }
    }

    pub fn get_supported_file_types() -> Vec<String> {
        vec![
            "rs".to_string(), "rb".to_string(), "go".to_string(), "js".to_string(), "ts".to_string(),
            "md".to_string(), "yaml".to_string(), "yml".to_string(), "toml".to_string(), "xml".to_string(),
        ]
    }

    // Internal function for parallel indexing
    // index_directory_parallel now receives already canonicalized paths
    fn index_directory_parallel(
        &mut self,
        files: Vec<PathBuf>, // These paths are already canonicalized
        model: Arc<EmbeddingModel>,
        batch_size: usize,
    ) -> Result<()> {
        let total_files = files.len() as u64;
        if total_files == 0 {
            return Ok(());
        }

        let progress_bar = ProgressBar::new(total_files);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({percent}%)")?
                .progress_chars("#>- ")
        );

        // Channel includes Option<u64> for hash
        let (files_to_embed_sender, files_to_embed_receiver) = mpsc::channel::<(PathBuf, String, Option<u64>)>();

        // Shared state for results from the embedder thread
        let new_embeddings_arc = Arc::new(Mutex::new(HashMap::<String, Vec<f32>>::new()));
        let updated_cache_arc = Arc::new(Mutex::new(self.cache.clone())); // Clone cache for embed thread to update

        let model_type_clone = self.embedding_model_type.clone();

        // --- Embedder Thread --- 
        let embed_thread_handle = std::thread::spawn({
            let model_arc = model.clone();
            let receiver = files_to_embed_receiver;
            let embeddings_map_write_ref = new_embeddings_arc.clone();
            let cache_write_ref = updated_cache_arc.clone();
            let _model_type_clone_for_cache = model_type_clone.clone();

            move || -> Result<()> {
                // Store paths, strings, and optional hashes
                let mut batch_paths_hashes = Vec::with_capacity(batch_size);
                let mut batch_texts = Vec::with_capacity(batch_size);

                // Receive path, string, and optional hash
                for (canonical_path_buf, canonical_path_str, hash_opt) in receiver {
                    match fs::read_to_string(&canonical_path_buf) {
                        Ok(content) => {
                            batch_paths_hashes.push((canonical_path_buf, canonical_path_str, hash_opt));
                            batch_texts.push(content);

                            if batch_texts.len() >= batch_size {
                                let texts_to_embed: Vec<&str> = batch_texts.iter().map(|s| s.as_str()).collect();
                                let embeddings_result = model_arc.embed_batch(&texts_to_embed);

                                match embeddings_result {
                                    Ok(embeddings) => {
                                        let mut embeddings_map_guard = embeddings_map_write_ref.lock().unwrap();
                                        let mut cache_guard = cache_write_ref.lock().unwrap();
                                        // Iterate through paths_hashes
                                        for ((path_buf_processed, path_str_processed, received_hash_opt), embedding) in batch_paths_hashes.drain(..).zip(embeddings) {
                                            // Get hash: use received hash or calculate if None
                                            let hash_to_insert = match received_hash_opt {
                                                Some(h) => h,
                                                None => match EmbeddingCache::get_file_hash(&path_buf_processed) {
                                                    Ok(h) => h,
                                                    Err(e) => {
                                                        error!("Failed to get hash for {}: {}. Cannot update cache.", path_str_processed, e);
                                                        continue; // Skip cache update if hash fails
                                                    }
                                                }
                                            };
                                            // Use insert_with_hash
                                            if let Err(e) = cache_guard.insert_with_hash(path_str_processed.clone(), embedding.clone(), hash_to_insert) {
                                                error!("Failed to insert into cache for {}: {}", path_str_processed, e);
                                            }
                                            embeddings_map_guard.insert(path_str_processed, embedding);
                                        }
                                    }
                                    Err(e) => {
                                        error!("Batch embedding failed: {}. Skipping batch.", e);
                                        batch_paths_hashes.clear(); // Clear paths for the failed batch
                                    }
                                }
                                batch_texts.clear();
                            }
                        }
                        Err(e) => {
                            error!("Failed to read file {} during embedding: {}. Skipping.", canonical_path_str, e);
                        }
                    }
                }

                // Process remaining batch
                if !batch_texts.is_empty() {
                     let texts_to_embed: Vec<&str> = batch_texts.iter().map(|s| s.as_str()).collect();
                     match model_arc.embed_batch(&texts_to_embed) {
                         Ok(embeddings) => {
                            let mut embeddings_map_guard = embeddings_map_write_ref.lock().unwrap();
                            let mut cache_guard = cache_write_ref.lock().unwrap();
                            // Iterate through paths_hashes
                             for ((path_buf_processed, path_str_processed, received_hash_opt), embedding) in batch_paths_hashes.drain(..).zip(embeddings) {
                                 // Get hash: use received hash or calculate if None
                                 let hash_to_insert = match received_hash_opt {
                                     Some(h) => h,
                                     None => match EmbeddingCache::get_file_hash(&path_buf_processed) {
                                        Ok(h) => h,
                                        Err(e) => {
                                            error!("Failed to get hash for {}: {}. Cannot update cache.", path_str_processed, e);
                                            continue; // Skip cache update
                                        }
                                    }
                                 };
                                 // Use insert_with_hash
                                 if let Err(e) = cache_guard.insert_with_hash(path_str_processed.clone(), embedding.clone(), hash_to_insert) {
                                     error!("Failed to insert into cache for {}: {}", path_str_processed, e);
                                 }
                                 embeddings_map_guard.insert(path_str_processed, embedding);
                             }
                         }
                         Err(e) => {
                             error!("Final batch embedding failed: {}. Skipping batch.", e);
                         }
                     }
                 }
                Ok(())
            }
        });

        // --- Cache Checking --- 
        let original_cache = self.cache.clone(); 
        files.par_iter().for_each(|canonical_path_buf| {
            let canonical_path_str = canonical_path_buf.to_string_lossy().into_owned();
            let cache_result = original_cache.check_cache_and_get_hash(&canonical_path_str, &canonical_path_buf);

            match cache_result {
                Ok(CacheCheckResult::Hit(_embedding)) => { 
                    debug!("Cache hit for {}", canonical_path_str);
                    progress_bar.inc(1);
                }
                Ok(CacheCheckResult::Miss(hash_opt)) => { // Pass hash_opt 
                    debug!("Cache miss/invalidated for {}. Needs embedding.", canonical_path_str);
                    // Send path, string, and optional hash
                    files_to_embed_sender.send((canonical_path_buf.clone(), canonical_path_str, hash_opt))
                        .expect("Failed to send path to embedding thread");
                }
                Err(e) => {
                    error!("Failed cache check/hash for {}: {}. Assuming needs embedding.", canonical_path_str, e);
                    // Send path, string, and None hash
                    files_to_embed_sender.send((canonical_path_buf.clone(), canonical_path_str, None))
                        .expect("Failed to send path (cache error)");
                    progress_bar.inc(1); 
                }
            }
        });

        // Signal embedder thread no more files are coming
        drop(files_to_embed_sender);

        // --- Wait and Merge Results --- 
        let embed_result = embed_thread_handle.join().expect("Embedding thread panicked");
        progress_bar.finish_with_message("File processing complete");

        if let Err(e) = embed_result {
            return Err(VectorDBError::EmbeddingError(format!("Error during embedding generation: {}", e)));
        }

        // Update the main state with results
        let new_embeddings = Arc::try_unwrap(new_embeddings_arc)
            .expect("Failed to unwrap new_embeddings Arc")
            .into_inner()
            .expect("Failed to get new_embeddings from Mutex");
        debug!("Merging {} new/updated embeddings", new_embeddings.len());
        self.embeddings.extend(new_embeddings); // Add new/updated embeddings

        // Update the cache state from the one modified by the embedder thread
        self.cache = Arc::try_unwrap(updated_cache_arc)
            .expect("Failed to unwrap updated_cache Arc")
            .into_inner()
            .expect("Failed to get updated_cache from Mutex");

        // Note: HNSW index rebuild happens in self.save() which is called by index_directory

        Ok(())
    }

    fn rebuild_hnsw_index(&mut self) -> Result<()> {
        if self.embeddings.is_empty() {
            debug!("No embeddings found, skipping HNSW index rebuild.");
            self.hnsw_index = None;
            return Ok(());
        }

        debug!("Rebuilding HNSW index with {} vectors...", self.embeddings.len());
        let start = Instant::now();

        // Determine dimension from the first embedding or model type
        let dimension = self.embeddings.values().next().map_or_else(
            || self.embedding_model_type.default_dimension(),
            |v| v.len(),
        );

        if dimension == 0 {
            return Err(VectorDBError::HNSWError("Cannot build HNSW index with dimension 0".to_string()));
        }

        let config = HNSWConfig::new(dimension);
        let mut hnsw_index = HNSWIndex::new(config);

        let pb = ProgressBar::new(self.embeddings.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} Building HNSW index: [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)")?
                .progress_chars("#>- ")
        );

        // Create a stable order for insertion based on file paths
        let mut sorted_paths: Vec<String> = self.embeddings.keys().cloned().collect();
        sorted_paths.sort();

        for path_str in sorted_paths {
            if let Some(embedding) = self.embeddings.get(&path_str) {
                if let Err(e) = hnsw_index.insert(embedding.clone()) {
                    error!("Failed to insert vector for {} into HNSW index: {}", path_str, e);
                    // Decide whether to continue or fail hard
                    return Err(VectorDBError::HNSWError(format!(
                        "Failed to insert vector for {} into HNSW index: {}",
                        path_str, e
                    )));
                }
                pb.inc(1);
            } else {
                 warn!("Path {} found in sorted keys but not in embeddings map during HNSW build", path_str);
            }
        }

        pb.finish_with_message("HNSW index build complete");
        let duration = start.elapsed();
        debug!("HNSW index rebuild took {:.2} seconds", duration.as_secs_f32());

        self.hnsw_index = Some(hnsw_index);
        Ok(())
    }

    // Helper to get file path associated with an HNSW node ID
    pub fn get_file_path(&self, node_id: usize) -> Option<String> {
        // Assuming node IDs correspond to the insertion order, which we now enforce by sorting keys
        let mut sorted_paths: Vec<String> = self.embeddings.keys().cloned().collect();
        sorted_paths.sort();
        sorted_paths.get(node_id).cloned()
    }

    // Add getter for cache
    pub fn cache(&self) -> &EmbeddingCache {
        &self.cache
    }

    // Add getter for embedding model type
    pub fn embedding_model_type(&self) -> EmbeddingModelType {
        self.embedding_model_type
    }

    // Add method to add a root
    pub fn add_indexed_root(&mut self, path_str: String) {
        self.indexed_roots.insert(path_str);
    }

    // Add getter for indexed roots
    pub fn indexed_roots(&self) -> &HashSet<String> {
        &self.indexed_roots
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
    
    use tempfile::tempdir;

    #[test]
    fn test_vectordb() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db").to_string_lossy().to_string();
        let _db = VectorDB::new(db_path)?;
        // Basic test - more tests needed for specific functionality
        Ok(())
    }
}