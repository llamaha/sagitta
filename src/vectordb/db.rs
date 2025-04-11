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
use chrono::{Utc};
use crate::vectordb::search::chunking::{chunk_by_paragraphs};

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IndexedChunk {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    // Embedding vector is stored in HNSW, not duplicated here by default
    pub embedding: Vec<f32>,
}

#[derive(Serialize, Deserialize, Debug)]
struct DBFile {
    indexed_chunks: Vec<IndexedChunk>,
    hnsw_config: Option<HNSWConfig>,
    feedback: Option<FeedbackData>,
    embedding_model_type: Option<EmbeddingModelType>,
    onnx_model_path: Option<String>,
    onnx_tokenizer_path: Option<String>,
    #[serde(default)]
    indexed_roots: HashMap<String, u64>,
}

pub struct VectorDB {
    pub indexed_chunks: Vec<IndexedChunk>,
    db_path: String,
    pub cache: EmbeddingCache,
    pub hnsw_index: Option<HNSWIndex>,
    feedback: FeedbackData,
    pub embedding_model_type: EmbeddingModelType,
    onnx_model_path: Option<PathBuf>,
    onnx_tokenizer_path: Option<PathBuf>,
    indexed_roots: HashMap<String, u64>,
}

impl Clone for VectorDB {
    fn clone(&self) -> Self {
        Self {
            indexed_chunks: self.indexed_chunks.clone(),
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
            indexed_chunks,
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
                        "Database parsed successfully: {} indexed chunks, {} indexed roots",
                        db_file.indexed_chunks.len(),
                        db_file.indexed_roots.len()
                    );
                    (
                        db_file.indexed_chunks,
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
                        Vec::new(),
                        Some(HNSWConfig::default()),
                        FeedbackData::default(),
                        EmbeddingModelType::Onnx,
                        None,
                        None,
                        HashMap::new(),
                    )
                }
            }
        } else {
            debug!("Database file doesn't exist, creating new database");
            (
                Vec::new(),
                Some(HNSWConfig::default()),
                FeedbackData::default(),
                EmbeddingModelType::Onnx,
                None,
                None,
                HashMap::new(),
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
            indexed_chunks,
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
        let embedding_dim = model_arc.dim(); // Get dimension from model

        let file_list = self.collect_files(&root_path_str, file_patterns)?;

        if file_list.is_empty() {
            println!("No files matching the patterns found in {}.", root_path_str);
            return Ok(());
        }

        // Determine embedding batch size (e.g., from config or default)
        // TODO: Make this configurable
        let embedding_batch_size = 32; 

        // Check HNSW index compatibility *before* indexing
        if let Some(existing_index) = &self.hnsw_index {
            if existing_index.get_config().dimension != embedding_dim {
                warn!(
                    "Existing HNSW index dimension ({}) does not match current model dimension ({}). Discarding index.",
                    existing_index.get_config().dimension, embedding_dim
                );
                self.hnsw_index = None;
                // Optionally, delete the physical index file here
                let hnsw_path = Path::new(&self.db_path).parent().unwrap_or_else(|| Path::new(".")).join("hnsw_index.json");
                let _ = fs::remove_file(&hnsw_path);
            }
        }

        let processed_chunks_data = self.index_files_parallel(file_list, model_arc, embedding_batch_size)?;
        
        // Rebuild HNSW index using *all* current chunks
        if !processed_chunks_data.is_empty() {
            debug!("Rebuilding HNSW index with new and existing chunks...");
            self.rebuild_hnsw_index_from_state(embedding_dim)?; 
        } else {
            debug!("No new chunks were processed, skipping HNSW rebuild.");
        }
        
        // Record the timestamp for the indexed root directory
        let timestamp = Utc::now().timestamp() as u64;
        if let Some(root_dir) = Path::new(dir_path).canonicalize().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
             self.update_indexed_root_timestamp(root_dir.to_string_lossy().to_string(), timestamp);
        } else {
            warn!("Could not determine parent directory for {} to update timestamp.", dir_path);
        }

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
        debug!("Saving VectorDB to {}", self.db_path);
        let start = Instant::now();

        // --- Rebuild HNSW Index before saving ---
        // Rebuilding HNSW is now tied to indexing, not saving.
        // If an index exists, save it. If not, that's fine.
        if let Some(hnsw_index) = &self.hnsw_index {
            let hnsw_path = Path::new(&self.db_path)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
            debug!("Saving HNSW index to {}", hnsw_path.display());
            if let Err(e) = hnsw_index.save_to_file(&hnsw_path) {
                error!("Failed to save HNSW index: {}", e);
                // Don't return error, allow db.json and cache to save
                eprintln!("Warning: Failed to save HNSW index: {}", e);
            } else {
                debug!("HNSW index saved successfully.");
            }
        } else {
            debug!("No HNSW index found, skipping save.");
        }

        let db_file = DBFile {
            indexed_chunks: self.indexed_chunks.clone(),
            hnsw_config: self.hnsw_index.as_ref().map(|idx| idx.get_config().clone()),
            feedback: Some(self.feedback.clone()),
            embedding_model_type: Some(self.embedding_model_type.clone()),
            onnx_model_path: self.onnx_model_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            onnx_tokenizer_path: self.onnx_tokenizer_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            indexed_roots: self.indexed_roots.clone(),
        };

        let contents = serde_json::to_string_pretty(&db_file)?;
        fs::write(&self.db_path, contents)?;
        debug!("Saved database file successfully to {}", self.db_path);

        // debug!("Saving cache to {}", self.cache.cache_path); // Removed log using private field
        self.cache.save()?;
        debug!("Saved cache successfully.");

        debug!("VectorDB saved in {:.2?}", start.elapsed());
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        debug!("Clearing VectorDB data");
        self.indexed_chunks.clear();
        self.hnsw_index = None;
        self.feedback = FeedbackData::default();
        self.indexed_roots.clear();

        // Also clear the physical files
        let _ = fs::remove_file(&self.db_path);
        let hnsw_path = Path::new(&self.db_path)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
        let _ = fs::remove_file(hnsw_path);
        self.cache.clear()?; // Clear cache content and file

        debug!("VectorDB cleared");
        Ok(())
    }

    pub fn stats(&self) -> DBStats {
        // Calculate unique files from indexed_chunks
        let unique_files = self.indexed_chunks.iter()
            .map(|chunk| &chunk.file_path)
            .collect::<HashSet<_>>()
            .len();

        DBStats {
            indexed_chunks: self.indexed_chunks.len(),
            unique_files,
            embedding_dimension: self.hnsw_index.as_ref()
                .map_or(self.embedding_model_type.default_dimension(), |idx| idx.get_config().dimension),
            db_path: self.db_path.clone(),
            cached_files: self.cache.len(),
            hnsw_stats: self.hnsw_index.as_ref().map(|idx| idx.stats()),
            embedding_model_type: self.embedding_model_type,
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

    // Renamed from index_directory_parallel to clarify it processes files
    fn index_files_parallel(
        &mut self,
        files: Vec<PathBuf>, // These paths are already canonicalized
        model: Arc<EmbeddingModel>,
        embedding_batch_size: usize,
    ) -> Result<Vec<IndexedChunk>> { // Return processed chunk data
        let total_files = files.len() as u64;
        if total_files == 0 {
            return Ok(Vec::new());
        }

        // Remove existing chunks originating from the files being indexed
        let files_to_reindex: HashSet<String> = files.iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let initial_chunk_count = self.indexed_chunks.len();
        self.indexed_chunks.retain(|chunk| !files_to_reindex.contains(&chunk.file_path));
        debug!("Removed {} existing chunks for {} files being re-indexed.", 
               initial_chunk_count - self.indexed_chunks.len(), files_to_reindex.len());

        let progress_bar = ProgressBar::new(total_files);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({percent}%) - Chunks: {msg}")?
                .progress_chars("#>- ")
        );
        progress_bar.set_message("0"); // Initial chunk count for this run

        let (files_to_process_sender, files_to_process_receiver) = mpsc::channel::<(PathBuf, String, Option<u64>)>();

        // Shared state for results from the processor thread
        let processed_chunks_arc = Arc::new(Mutex::new(Vec::<IndexedChunk>::new()));
        let updated_cache_arc = Arc::new(Mutex::new(self.cache.clone())); 
        let processed_chunk_count_this_run = Arc::new(Mutex::new(0_usize));

        // --- Processor Thread (Embeds Chunks) --- 
        let processor_thread_handle = std::thread::spawn({
            let model_arc = model.clone();
            let receiver = files_to_process_receiver;
            let chunks_write_ref = processed_chunks_arc.clone();
            let cache_write_ref = updated_cache_arc.clone();
            let chunk_count_ref = processed_chunk_count_this_run.clone();
            let pb_clone = progress_bar.clone(); 

            move || -> Result<()> {
                // Store metadata and owned text strings for the batch
                let mut chunk_batch_meta = Vec::with_capacity(embedding_batch_size);
                let mut chunk_batch_texts: Vec<String> = Vec::with_capacity(embedding_batch_size); // Store owned Strings

                for (canonical_path_buf, canonical_path_str, file_hash_opt) in receiver {
                    match fs::read_to_string(&canonical_path_buf) {
                        Ok(content) => {
                            let file_chunks = chunk_by_paragraphs(&content);
                            
                            if file_chunks.is_empty() {
                                // Handle empty files (as before)
                                debug!("Skipping empty file or file with no text: {}", canonical_path_str);
                                if let Some(hash_to_insert) = file_hash_opt.or_else(|| EmbeddingCache::get_file_hash(&canonical_path_buf).ok()) {
                                     if let Err(e) = cache_write_ref.lock().unwrap().insert_file_hash(canonical_path_str.clone(), hash_to_insert) {
                                         error!("Failed to update cache for skipped file {}: {}", canonical_path_str, e);
                                     }
                                } else {
                                     error!("Could not get hash for skipped file {}. Cache not updated.", canonical_path_str);
                                }
                                pb_clone.inc(1); 
                                continue;
                            }

                            // Chunk the content directly
                            let file_chunks = chunk_by_paragraphs(&content);

                            let mut file_processed_chunks = Vec::<IndexedChunk>::new();

                            for chunk_info in file_chunks.into_iter() {
                                // Store metadata
                                chunk_batch_meta.push((chunk_info.clone(), canonical_path_str.clone())); 
                                // Store owned text string for batching
                                chunk_batch_texts.push(chunk_info.text);

                                if chunk_batch_texts.len() >= embedding_batch_size {
                                    // Convert Vec<String> to Vec<&str> for embed_batch
                                    let text_refs: Vec<&str> = chunk_batch_texts.iter().map(|s| s.as_str()).collect();
                                    match model_arc.embed_batch(&text_refs) {
                                        Ok(embeddings) => {
                                            for (i, embedding) in embeddings.into_iter().enumerate() {
                                                 let (info, path) = chunk_batch_meta[i].clone(); 
                                                 file_processed_chunks.push(IndexedChunk {
                                                     file_path: path,
                                                     start_line: info.start_line,
                                                     end_line: info.end_line,
                                                     text: info.text, // Text is already owned in info
                                                     embedding: embedding,
                                                 });
                                            }
                                        }
                                        Err(e) => {
                                            error!("Chunk batch embedding failed: {}. Skipping batch.", e);
                                        }
                                    }
                                    chunk_batch_meta.clear();
                                    chunk_batch_texts.clear(); // Clear owned strings
                                }
                            }

                            // Process remaining batch for the file
                            if !chunk_batch_texts.is_empty() {
                                let text_refs: Vec<&str> = chunk_batch_texts.iter().map(|s| s.as_str()).collect();
                                match model_arc.embed_batch(&text_refs) {
                                    Ok(embeddings) => {
                                        for (i, embedding) in embeddings.into_iter().enumerate() {
                                            let (info, path) = chunk_batch_meta[i].clone();
                                             file_processed_chunks.push(IndexedChunk {
                                                 file_path: path,
                                                 start_line: info.start_line,
                                                 end_line: info.end_line,
                                                 text: info.text, // Text is already owned in info
                                                 embedding: embedding,
                                             });
                                        }
                                    }
                                    Err(e) => {
                                        error!("Final chunk batch embedding failed: {}. Skipping batch.", e);
                                    }
                                }
                                // Clear meta and texts after processing
                                chunk_batch_meta.clear();
                                chunk_batch_texts.clear(); 
                            }

                            // Add successfully processed chunks for this file to the main shared vec
                            if !file_processed_chunks.is_empty() {
                                let num_added = file_processed_chunks.len(); // Count before moving
                                let mut processed_chunks_guard = chunks_write_ref.lock().unwrap();
                                processed_chunks_guard.extend(file_processed_chunks); // Extend with Vec<IndexedChunk>
                                
                                let mut chunk_count_guard = chunk_count_ref.lock().unwrap();
                                *chunk_count_guard += num_added;
                                pb_clone.set_message(format!("{}", *chunk_count_guard)); 
                            }
                            
                            // Update file cache
                            if let Some(hash_to_insert) = file_hash_opt.or_else(|| EmbeddingCache::get_file_hash(&canonical_path_buf).ok()) {
                                if let Err(e) = cache_write_ref.lock().unwrap().insert_file_hash(canonical_path_str.clone(), hash_to_insert) {
                                    error!("Failed to update cache for {}: {}", canonical_path_str, e);
                                }
                            } else {
                                error!("Could not get hash for {}. Cache not updated.", canonical_path_str);
                            }
                        }
                        Err(e) => {
                            error!("Failed to read file {} during processing: {}. Skipping.", canonical_path_str, e);
                        }
                    }
                    pb_clone.inc(1); // Increment file progress bar
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
                Ok(CacheCheckResult::Hit) => {
                    debug!("Cache hit for file {}. Skipping chunk processing.", canonical_path_str);
                    progress_bar.inc(1);
                }
                Ok(CacheCheckResult::Miss(hash_opt)) => {
                    debug!("Cache miss/invalidated for file {}. Needs processing.", canonical_path_str);
                    files_to_process_sender.send((canonical_path_buf.clone(), canonical_path_str, hash_opt))
                        .expect("Failed to send file path to processing thread");
                }
                Err(e) => {
                    error!("Failed cache check/hash for file {}: {}. Assuming needs processing.", canonical_path_str, e);
                    files_to_process_sender.send((canonical_path_buf.clone(), canonical_path_str, None))
                        .expect("Failed to send file path (cache error)");
                     // Let processor thread inc progress after trying to read
                }
            }
        });

        drop(files_to_process_sender);

        // --- Wait and Merge Results --- 
        let process_result = processor_thread_handle.join().expect("Processing thread panicked");
        progress_bar.finish_with_message(format!("File processing complete. New chunks: {}", *processed_chunk_count_this_run.lock().unwrap()));

        if let Err(e) = process_result {
            return Err(VectorDBError::EmbeddingError(format!("Error during chunk processing: {}", e)));
        }

        let processed_chunks_data = Arc::try_unwrap(processed_chunks_arc)
            .expect("Failed to unwrap processed_chunks Arc")
            .into_inner()
            .expect("Failed to get processed_chunks from Mutex");
        
        debug!("Adding {} new indexed chunks to main state", processed_chunks_data.len());
        self.indexed_chunks.extend(processed_chunks_data.clone()); // Clone needed if returning below
        
        self.cache = Arc::try_unwrap(updated_cache_arc)
            .expect("Failed to unwrap updated_cache Arc")
            .into_inner()
            .expect("Failed to get updated_cache from Mutex");
        
        Ok(processed_chunks_data) // Return the processed data for HNSW build
    }

    // Rebuilds HNSW index using the current `self.indexed_chunks`
    fn rebuild_hnsw_index_from_state(&mut self, dimension: usize) -> Result<()> {
        if self.indexed_chunks.is_empty() {
            debug!("No indexed chunks found, skipping HNSW index rebuild.");
            self.hnsw_index = None;
            return Ok(());
        }

        debug!("Rebuilding HNSW index with {} vectors...", self.indexed_chunks.len());
        let start = Instant::now();

        // Dimension is now passed in
        if dimension == 0 {
            return Err(VectorDBError::HNSWError("Cannot build HNSW index with dimension 0".to_string()));
        }

        let config = HNSWConfig::new(dimension);
        let mut hnsw_index = HNSWIndex::new(config);

        let pb = ProgressBar::new(self.indexed_chunks.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} Building HNSW index: [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)")?
                .progress_chars("#>- ")
        );

        // Need embeddings for all chunks in self.indexed_chunks.
        // This requires re-embedding everything, which is inefficient.
        // We MUST store embeddings alongside IndexedChunk or retrieve them.
        // TEMPORARY: Simulate retrieval (THIS WILL PANIC IF EMBEDDINGS ARE NOT STORED)
        // TODO: Fix this by storing embeddings or having a proper retrieval mechanism.
        // let model = self.create_embedding_model()?; // No longer needed
        
        // Iterate through stored chunks and use their embeddings
        for (i, chunk) in self.indexed_chunks.iter().enumerate() {
            // Simulate getting the embedding (REPLACE THIS)
            // let embedding = self.get_embedding_for_chunk(i)?; // Hypothetical method
            // let embedding = model.embed(&chunk.text)?; // Inefficiently re-embed
            
            // Directly use the stored embedding
            let embedding = &chunk.embedding; // Borrow the embedding

            if embedding.len() != dimension {
                 error!("Fatal error: Chunk {} ({}:{}) has embedding dimension {} but index expects {}. Aborting build.", 
                       i, chunk.file_path, chunk.start_line, embedding.len(), dimension);
                 return Err(VectorDBError::HNSWError(format!(
                     "Dimension mismatch for vector {} during HNSW rebuild.", i
                 )));
            }

            if let Err(e) = hnsw_index.insert(embedding.clone()) { // Clone embedding for insertion
                error!("Fatal error inserting vector for chunk {} ({}:{}) into HNSW index: {}. Aborting build.", 
                       i, chunk.file_path, chunk.start_line, e);
                return Err(VectorDBError::HNSWError(format!(
                    "Failed to insert vector {} into HNSW index during rebuild: {}", i, e
                )));
            }
            pb.inc(1);
        }

        pb.finish_with_message("HNSW index build complete");
        let duration = start.elapsed();
        debug!("HNSW index rebuild took {:.2} seconds", duration.as_secs_f32());

        self.hnsw_index = Some(hnsw_index);
        Ok(())
    }

    // Helper to get file path associated with an HNSW node ID
    pub fn get_file_path(&self, node_id: usize) -> Option<String> {
        // The HNSW node ID now corresponds directly to the index in indexed_chunks
        self.indexed_chunks.get(node_id).map(|chunk| chunk.file_path.clone())
    }

    // Add getter for cache
    pub fn cache(&self) -> &EmbeddingCache {
        &self.cache
    }

    // Add getter for embedding model type
    pub fn embedding_model_type(&self) -> EmbeddingModelType {
        self.embedding_model_type
    }

    // Replace add_indexed_root with update_indexed_root_timestamp
    pub fn update_indexed_root_timestamp(&mut self, path_str: String, timestamp: u64) {
        self.indexed_roots.insert(path_str, timestamp);
    }

    // Update getter for indexed roots
    pub fn indexed_roots(&self) -> &HashMap<String, u64> {
        &self.indexed_roots
    }
}

pub struct DBStats {
    pub indexed_chunks: usize,
    pub unique_files: usize,
    pub embedding_dimension: usize,
    pub db_path: String,
    pub cached_files: usize,
    pub hnsw_stats: Option<HNSWStats>,
    pub embedding_model_type: EmbeddingModelType,
}

impl Clone for DBStats {
    fn clone(&self) -> Self {
        Self {
            indexed_chunks: self.indexed_chunks,
            unique_files: self.unique_files,
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