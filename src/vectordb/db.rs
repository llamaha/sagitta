use crate::vectordb::cache::{CacheCheckResult, EmbeddingCache};
use crate::vectordb::embedding::{EmbeddingModel, EmbeddingModelType};
use crate::vectordb::error::{Result, VectorDBError};
use crate::vectordb::hnsw::{HNSWConfig, HNSWIndex, HNSWStats};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, error, info, warn};
use num_cpus;
use rayon::iter::ParallelIterator;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

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
        debug!("Creating VectorDB with database path: {}", db_path);

        let (
            embeddings,
            hnsw_config,
            feedback,
            embedding_model_type,
            onnx_model_path,
            onnx_tokenizer_path,
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
                        "Database parsed successfully: {} embeddings",
                        db_file.embeddings.len()
                    );
                    (
                        db_file.embeddings,
                        db_file.hnsw_config,
                        db_file.feedback.unwrap_or_default(),
                        loaded_model_type,
                        db_file.onnx_model_path.map(PathBuf::from),
                        db_file.onnx_tokenizer_path.map(PathBuf::from),
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
        let dir_path = Path::new(dir_path);
        if !dir_path.exists() || !dir_path.is_dir() {
            return Err(VectorDBError::DirectoryNotFound(dir_path.to_string_lossy().to_string()));
        }
        debug!("Starting directory scan for files to index in {}", dir_path.display());
        let files: Vec<PathBuf> = WalkDir::new(dir_path)
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.file_type().is_file()
                        && match entry.path().extension() {
                            Some(ext) => file_patterns.is_empty() || file_patterns.contains(&ext.to_string_lossy().to_string()),
                            None => file_patterns.is_empty(),
                        }
                })
                .map(|entry| entry.path().to_path_buf())
                .collect();

        let file_count = files.len();
        if file_count == 0 {
            println!("No matching files found in the directory.");
            return Ok(());
        }
        println!("Found {} files to index.", file_count);
        let model = Arc::new(self.create_embedding_model()?);
        let batch_size = if file_count < 1000 { 100 } else if file_count < 10000 { 500 } else { 1000 };
        debug!("Using batch size of {} for {} files", batch_size, file_count);
        self.index_directory_parallel(files, model, batch_size)?;
        Ok(())
    }

    pub fn save(&mut self) -> Result<()> {
        debug!("Saving VectorDB to {}", self.db_path);
        let path = Path::new(&self.db_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| VectorDBError::DirectoryCreationError {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        let db_file = DBFile {
            embeddings: self.embeddings.clone(),
            hnsw_config: self.hnsw_index.as_ref().map(|idx| idx.get_config()),
            feedback: Some(self.feedback.clone()),
            embedding_model_type: Some(self.embedding_model_type.clone()),
            onnx_model_path: self.onnx_model_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            onnx_tokenizer_path: self.onnx_tokenizer_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        };
        let json = serde_json::to_string_pretty(&db_file)?;
        fs::write(&self.db_path, &json).map_err(|e| VectorDBError::FileWriteError {
            path: path.to_path_buf(),
            source: e,
        })?;
        if let Some(index) = &self.hnsw_index {
            let hnsw_path = Path::new(&self.db_path)
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
            if let Some(parent) = hnsw_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            debug!("Saving HNSW index to {}", hnsw_path.display());
            if let Err(e) = index.save_to_file(&hnsw_path) {
                error!("Failed to save HNSW index: {}", e);
                eprintln!("Warning: Failed to save HNSW index: {}", e);
            }
        }
        self.cache.save()?;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        debug!("Clearing VectorDB database and index");
        self.embeddings.clear();
        self.hnsw_index = None; // Remove the index instance entirely
        self.cache.clear()?;
        self.feedback = FeedbackData::default();
        // Save the cleared state (empty embeddings, no index)
        self.save()?;
        debug!("VectorDB cleared successfully");
        Ok(())
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

    fn index_directory_parallel( &mut self, files: Vec<PathBuf>, model: Arc<EmbeddingModel>, batch_size: usize ) -> Result<()> {
        let num_threads = num_cpus::get();
        let file_count = files.len();
        info!("Starting parallel indexing for {} files using {} threads.", file_count, num_threads);

        // --- HNSW Index Validation and Creation ---
        let target_dim = model.dim();
        debug!("Target embedding dimension for this indexing operation: {}", target_dim);

        match &mut self.hnsw_index {
            Some(index) => {
                let current_dim = index.get_config().dimension;
                if current_dim == target_dim {
                    debug!("Existing HNSW index found with matching dimension {}. Reusing index.", current_dim);
                } else {
                    warn!(
                        "Existing HNSW index dimension ({}) does not match model dimension ({}). Clearing embeddings and creating a new index.",
                        current_dim, target_dim
                    );
                    // Dimension mismatch, need to clear incompatible embeddings and create a new index
                    self.embeddings.clear(); // Clear embeddings as they belong to the old index
                    let new_config = HNSWConfig::new(target_dim);
                    *index = HNSWIndex::new(new_config); // Replace the index
                    // Cache should also be cleared or handled carefully, but clearing embeddings is the minimum
                    // For simplicity, let's rely on the cache check logic below to re-embed if needed.
                }
            }
            None => {
                debug!("No HNSW index found. Creating a new index with dimension {}.", target_dim);
                let new_config = HNSWConfig::new(target_dim);
                self.hnsw_index = Some(HNSWIndex::new(new_config));
            }
        }
        // --- End HNSW Index Validation ---

        let progress = ProgressBar::new(file_count as u64);
        progress.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files indexed ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-"));
        let cache_arc = Arc::new(Mutex::new(self.cache.clone()));
        let mut files_to_embed: Vec<(PathBuf, Option<u64>)> = Vec::with_capacity(file_count);
        let mut files_from_cache = 0;
        let mut cache_errors = 0;
        debug!("Checking cache for {} files...", file_count);
        {
            let mut cache_guard = cache_arc.lock().unwrap();
            for file_path in &files {
                let path_str = file_path.to_string_lossy().to_string();
                match cache_guard.check_cache_and_get_hash(&path_str, file_path) {
                    Ok(CacheCheckResult::Hit(embedding)) => {
                        drop(cache_guard);
                        self.embeddings.insert(path_str, embedding.clone());
                        if let Some(index) = &mut self.hnsw_index {
                            if let Err(e) = index.insert(embedding) {
                                error!("Failed to insert cached embedding into HNSW index for {}: {}", file_path.display(), e);
                            }
                        }
                        files_from_cache += 1;
                        progress.inc(1);
                        cache_guard = cache_arc.lock().unwrap();
                    }
                    Ok(CacheCheckResult::Miss(hash_opt)) => { files_to_embed.push((file_path.clone(), hash_opt)); }
                    Err(e) => {
                        error!("Cache check/hash error for {}: {}. Queuing for embedding.", file_path.display(), e);
                        cache_errors += 1;
                        files_to_embed.push((file_path.clone(), None));
                    }
                }
            }
        }
        if cache_errors > 0 { progress.println(format!("Warning: Encountered {} cache read/hash errors.", cache_errors)); }
        let files_to_embed_count = files_to_embed.len();
        debug!("Found {} files in cache. {} files need embedding.", files_from_cache, files_to_embed_count);
        if files_to_embed_count == 0 {
            progress.finish_with_message(format!("Processed {} files (all from cache).", files_from_cache));
            if let Err(e) = self.save() { error!("Failed to save database after cache check: {}", e); }
            return Ok(());
        }
        let model_clone = Arc::clone(&model);
        let effective_batch_size = std::cmp::min(batch_size, 128);
        let file_chunks: Vec<Vec<(PathBuf, Option<u64>)>> = files_to_embed.chunks(effective_batch_size).map(|chunk| chunk.to_vec()).collect();
        let chunk_count = file_chunks.len();
        info!("Processing {} files needing embedding in {} chunks (batch size ~{}).", files_to_embed_count, chunk_count, effective_batch_size);
        progress.println(format!("Scanning and preparing {} files for embedding...", files_to_embed_count));
        let pool = rayon::ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();
        let (tx, rx): ( Sender<Vec<(PathBuf, Option<u64>, Result<Vec<f32>>)>>, Receiver<_>, ) = mpsc::channel();
        let progress_clone = progress.clone();
        let processed = Arc::new(AtomicUsize::new(0));
        let processed_clone = processed.clone();
        let should_continue = Arc::new(AtomicBool::new(true));
        let should_continue_clone = should_continue.clone();
        std::thread::spawn(move || {
            let mut last_count = 0;
            let start = std::time::Instant::now();
            progress_clone.set_message("Starting file processing...");
            while should_continue_clone.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let current = processed_clone.load(Ordering::SeqCst);
                if current > last_count {
                    let elapsed = start.elapsed().as_secs_f32();
                    let rate = if elapsed > 0.1 { current as f32 / elapsed } else { 0.0 };
                    progress_clone.set_message(format!("Processing files ({:.1} files/sec)", rate));
                    progress_clone.set_position(current as u64);
                    last_count = current;
                } else if current > 0 { progress_clone.set_message("Processing (waiting for results)..."); }
                 else { progress_clone.set_message("Preparing for embedding..."); }
            }
        });
        pool.install(move || {
            file_chunks.into_par_iter().for_each_with(tx, |tx_clone, chunk_data| {
                    let mut texts_to_embed: Vec<String> = Vec::with_capacity(chunk_data.len());
                let mut paths_and_hashes: Vec<(PathBuf, Option<u64>)> = Vec::with_capacity(chunk_data.len());
                let mut results_for_chunk: Vec<(PathBuf, Option<u64>, Result<Vec<f32>>)> = Vec::with_capacity(chunk_data.len());
                    for (path, hash_opt) in chunk_data {
                        match std::fs::read_to_string(&path) {
                        Ok(content) => { texts_to_embed.push(content); paths_and_hashes.push((path, hash_opt)); }
                        Err(e) => { results_for_chunk.push(( path, hash_opt, Err(VectorDBError::IOError(e)), )); }
                    }
                }
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
                                results_for_chunk.push(( path, hash_opt, Err(VectorDBError::EmbeddingError(error_message.clone())), ));
                            }
                        }
                    }
                }
                    if !results_for_chunk.is_empty() {
                        processed.fetch_add(results_for_chunk.len(), Ordering::SeqCst);
                    if tx_clone.send(results_for_chunk).is_err() { warn!("Failed to send chunk results to main thread."); }
                    }
                });
        });
        should_continue.store(false, Ordering::SeqCst);
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
                        if let Some(hash) = hash_opt {
                            if let Err(e) = cache_guard.insert_with_hash( file_path_str.clone(), embedding.clone(), hash, ) {
                                error!("Failed to insert into cache for {}: {}", file_path.display(), e);
                            }
                        } else {
                            match EmbeddingCache::get_file_hash(&file_path) {
                                Ok(new_hash) => {
                                    if let Err(e) = cache_guard.insert_with_hash( file_path_str.clone(), embedding.clone(), new_hash, ) {
                                        error!("Failed to insert into cache (retry hash) for {}: {}", file_path.display(), e);
                                    }
                                }
                                Err(e) => { error!("Failed again to get hash for cache insertion for {}: {}", file_path.display(), e); }
                            }
                        }
                        self.embeddings.insert(file_path_str.clone(), embedding.clone());
                        drop(cache_guard);
                        if let Some(index) = &mut self.hnsw_index {
                            if let Err(e) = index.insert(embedding) {
                                error!("Failed to insert into HNSW index for {}: {}", file_path.display(), e);
                                progress.println(format!("Warning: Failed to add {} to HNSW index: {}", file_path_str, e));
                            }
                        }
                        cache_guard = cache_arc.lock().unwrap();
                        successful_embeddings += 1;
                        save_counter += 1;
                    }
                    Err(error) => { progress.println(format!("Error indexing {}: {}", file_path.display(), error)); }
                }
                progress.inc(1);
            }
            drop(cache_guard);
            if save_counter >= effective_batch_size {
                if let Err(e) = self.save() {
                    error!("Failed to save database during batch processing: {}", e);
                    progress.println(format!("Warning: Failed to save database: {}", e));
                } else { debug!("Saved database after processing {} new files", save_counter); }
                save_counter = 0;
            }
            let now = std::time::Instant::now();
            if now.duration_since(last_report_time).as_secs() >= 5 {
                last_report_time = now;
                let elapsed_secs = now.duration_since(start_time).as_secs_f32();
                let rate = if elapsed_secs > 0.1 { processed_new_files as f32 / elapsed_secs } else { 0.0 };
                progress.set_message(format!("Storing embeddings ({:.1} files/sec)", rate));
            }
        }
        if save_counter > 0 {
            if let Err(e) = self.save() {
                error!("Failed to save database at end of indexing: {}", e);
                progress.println(format!("Warning: Failed to save database: {}", e));
            } else { debug!("Final save completed."); }
            }
        let elapsed_total = start_time.elapsed().as_secs_f32();
        let rate_final = if elapsed_total > 0.1 { successful_embeddings as f32 / elapsed_total } else { 0.0 };
        let total_files_in_db = files_from_cache + successful_embeddings;
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
        Ok(())
    }

    // Restore get_file_path
    pub fn get_file_path(&self, node_id: usize) -> Option<&String> {
        self.embeddings.keys().nth(node_id)
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