// src/vectordb/indexing.rs

use super::db::{IndexedChunk, VectorDB}; // Import IndexedChunk from db module now
use crate::vectordb::cache::{CacheCheckResult, EmbeddingCache};
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::error::{Result, VectorDBError};
use crate::vectordb::hnsw::{HNSWConfig, HNSWIndex};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, error, warn};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashSet;
use std::fs::{self, canonicalize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::time::{Instant, SystemTime};
use walkdir::WalkDir;
use chrono::{Utc};
use crate::vectordb::search::chunking::{chunk_by_paragraphs, chunk_by_lines};

// --- Public Functions (called from VectorDB impl) ---

/// Indexes a directory based on file patterns.
pub(super) fn index_directory(db: &mut VectorDB, dir_path: &str, file_patterns: &[String]) -> Result<()> {
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

    let model = db.embedding_handler().create_embedding_model()?;
    let model_arc = Arc::new(model);
    let embedding_dim = model_arc.dim();

    let file_list = collect_files(db, &root_path_str, file_patterns)?;

    if file_list.is_empty() {
        println!("No files matching the patterns found in {}.", root_path_str);
        return Ok(());
    }

    let embedding_batch_size = 32;

    if let Some(existing_index) = &db.hnsw_index {
        if existing_index.get_config().dimension != embedding_dim {
            warn!(
                "Existing HNSW index dimension ({}) does not match current model dimension ({}). Discarding index.",
                existing_index.get_config().dimension, embedding_dim
            );
            db.hnsw_index = None;
            let hnsw_path = Path::new(&db.db_path).parent().unwrap_or_else(|| Path::new(".")).join("hnsw_index.json");
            let _ = fs::remove_file(&hnsw_path);
        }
    }

    db.cache.set_model_type(db.embedding_handler().embedding_model_type());

    let processed_chunks_data = index_files_parallel(db, file_list, model_arc, embedding_batch_size)?;

    if !processed_chunks_data.is_empty() {
        debug!("Rebuilding HNSW index with new and existing chunks...");
        rebuild_hnsw_index_from_state(db, embedding_dim)?;
    } else {
        debug!("No new chunks were processed, skipping HNSW rebuild.");
    }

    let timestamp = Utc::now().timestamp() as u64;
    // Call the internal update method on VectorDB
    db.update_indexed_root_timestamp_internal(root_path_str.clone(), timestamp);

    db.save()?;

    Ok(())
}

/// Removes an indexed directory and associated data.
pub(super) fn remove_directory(db: &mut VectorDB, dir_path: &str) -> Result<()> {
    let canonical_dir = canonicalize(Path::new(dir_path)).map_err(|e| {
        VectorDBError::IndexingError(format!(
            "Failed to canonicalize directory '{}': {}",
            dir_path, e
        ))
    })?;
    let canonical_dir_str = canonical_dir.to_string_lossy().to_string();

    debug!("Attempting to remove canonical directory: {}", canonical_dir_str);

    if db.indexed_roots.remove(&canonical_dir_str).is_none() {
        warn!(
            "Directory '{}' (canonical: {}) not found in indexed roots.",
            dir_path, canonical_dir_str
        );
        return Err(VectorDBError::DirectoryNotIndexed(canonical_dir_str));
    }
    debug!("Removed '{}' from indexed_roots.", canonical_dir_str);

    let initial_chunk_count = db.indexed_chunks.len();
    let path_prefix = format!("{}", canonical_dir.display());
    db.indexed_chunks.retain(|chunk| {
        !Path::new(&chunk.file_path).starts_with(&path_prefix)
    });
    let removed_chunk_count = initial_chunk_count - db.indexed_chunks.len();
    debug!(
        "Removed {} chunks associated with directory '{}'.",
        removed_chunk_count,
        canonical_dir_str
    );

    if removed_chunk_count > 0 {
        if db.hnsw_index.is_some() {
            debug!("Clearing HNSW index due to chunk removal.");
            db.hnsw_index = None;
            let hnsw_path = Path::new(&db.db_path)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
            let _ = fs::remove_file(&hnsw_path);
            debug!("Removed HNSW index file: {}", hnsw_path.display());
        }
    }

    println!(
        "Removed index entry for '{}' and {} associated data chunks.",
        canonical_dir_str,
        removed_chunk_count
    );

    Ok(())
}

// --- Internal Helper Functions ---

fn collect_files(_db: &VectorDB, canonical_dir_path: &str, file_patterns: &[String]) -> Result<Vec<PathBuf>> {
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

fn index_files_parallel(
    db: &mut VectorDB,
    files: Vec<PathBuf>,
    model: Arc<EmbeddingModel>,
    embedding_batch_size: usize,
) -> Result<Vec<IndexedChunk>> {
    let total_files = files.len() as u64;
    if total_files == 0 {
        return Ok(Vec::new());
    }

    let files_to_reindex: HashSet<String> = files.iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let initial_chunk_count = db.indexed_chunks.len();
    db.indexed_chunks.retain(|chunk| !files_to_reindex.contains(&chunk.file_path));
    debug!("Removed {} existing chunks for {} files being re-indexed.",
           initial_chunk_count - db.indexed_chunks.len(), files_to_reindex.len());

    let progress_bar = ProgressBar::new(total_files);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({percent}%) - Chunks: {msg}")?
            .progress_chars("#>- ")
    );
    progress_bar.set_message("0");

    let (files_to_process_sender, files_to_process_receiver) = mpsc::channel::<(PathBuf, String, Option<u64>)>();

    let processed_chunks_arc = Arc::new(Mutex::new(Vec::<IndexedChunk>::new()));
    let updated_cache_arc = Arc::new(Mutex::new(db.cache.clone()));
    let processed_chunk_count_this_run = Arc::new(Mutex::new(0_usize));

    let processor_thread_handle = std::thread::spawn({
        let model_arc = model.clone();
        let receiver = files_to_process_receiver;
        let chunks_write_ref = processed_chunks_arc.clone();
        let cache_write_ref = updated_cache_arc.clone();
        let chunk_count_ref = processed_chunk_count_this_run.clone();
        let pb_clone = progress_bar.clone();

        move || -> Result<()> {
            let mut chunk_batch_meta = Vec::with_capacity(embedding_batch_size);
            let mut chunk_batch_texts: Vec<String> = Vec::with_capacity(embedding_batch_size);

            const CODE_EXTENSIONS: [&str; 6] = ["js", "ts", "py", "go", "rs", "rb"];
            const CODE_CHUNK_SIZE: usize = 20;
            const CODE_OVERLAP: usize = 5;

            while let Ok((canonical_path_buf, canonical_path_str, file_hash_opt)) = receiver.recv() {
                debug!("Processing file: {}", canonical_path_str);
                match fs::read_to_string(&canonical_path_buf) {
                    Ok(content) => {
                        let file_extension = canonical_path_buf
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .unwrap_or("")
                            .to_lowercase();

                        let file_chunks =
                            if CODE_EXTENSIONS.contains(&file_extension.as_str()) {
                                debug!(
                                    "Using line chunking (size={}, overlap={}) for: {}",
                                    CODE_CHUNK_SIZE, CODE_OVERLAP, canonical_path_str
                                );
                                chunk_by_lines(&content, CODE_CHUNK_SIZE, CODE_OVERLAP)
                            } else {
                                debug!("Using paragraph chunking for: {}", canonical_path_str);
                                chunk_by_paragraphs(&content)
                            };

                        if file_chunks.is_empty() {
                            debug!("Skipping empty file or file with no text: {}", canonical_path_str);
                            pb_clone.inc(1);
                            continue;
                        }

                        // Get last modified time
                        let last_modified_time = match fs::metadata(&canonical_path_buf) {
                            Ok(meta) => meta.modified().unwrap_or_else(|_| SystemTime::now()),
                            Err(_) => SystemTime::now(), // Fallback
                        };

                        let mut file_processed_chunks = Vec::<IndexedChunk>::new();

                        for chunk_info in file_chunks.into_iter() {
                            chunk_batch_meta.push((chunk_info.clone(), canonical_path_str.clone()));
                            chunk_batch_texts.push(chunk_info.text);

                            if chunk_batch_texts.len() >= embedding_batch_size {
                                let text_refs: Vec<&str> = chunk_batch_texts.iter().map(|s| s.as_str()).collect();
                                match model_arc.embed_batch(&text_refs) {
                                    Ok(embeddings) => {
                                        for (i, embedding) in embeddings.into_iter().enumerate() {
                                            let (info, path) = chunk_batch_meta[i].clone();
                                            file_processed_chunks.push(IndexedChunk {
                                                id: 0, // Placeholder ID
                                                file_path: path,
                                                start_line: info.start_line,
                                                end_line: info.end_line,
                                                text: info.text,
                                                embedding: embedding,
                                                last_modified: last_modified_time, // Use file mod time
                                                metadata: None, // Add metadata field
                                            });
                                        }
                                    }
                                    Err(e) => {
                                        error!("Chunk batch embedding failed: {}. Skipping batch.", e);
                                    }
                                }
                                chunk_batch_meta.clear();
                                chunk_batch_texts.clear();
                            }
                        }

                        if !chunk_batch_texts.is_empty() {
                            let text_refs: Vec<&str> = chunk_batch_texts.iter().map(|s| s.as_str()).collect();
                            match model_arc.embed_batch(&text_refs) {
                                Ok(embeddings) => {
                                    for (i, embedding) in embeddings.into_iter().enumerate() {
                                        let (info, path) = chunk_batch_meta[i].clone();
                                        file_processed_chunks.push(IndexedChunk {
                                            id: 0, // Placeholder ID
                                            file_path: path,
                                            start_line: info.start_line,
                                            end_line: info.end_line,
                                            text: info.text,
                                            embedding: embedding,
                                            last_modified: last_modified_time, // Use file mod time
                                            metadata: None, // Add metadata field
                                        });
                                    }
                                }
                                Err(e) => {
                                    error!("Final chunk batch embedding failed: {}. Skipping batch.", e);
                                }
                            }
                        }

                        if !file_processed_chunks.is_empty() {
                            let num_added = file_processed_chunks.len();
                            let mut processed_chunks_guard = chunks_write_ref.lock().unwrap();
                            processed_chunks_guard.extend(file_processed_chunks);

                            let mut chunk_count_guard = chunk_count_ref.lock().unwrap();
                            *chunk_count_guard += num_added;
                            pb_clone.set_message(format!("{}", *chunk_count_guard));
                        }

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
                pb_clone.inc(1);
            }

            Ok(())
        }
    });

    // Pass db.cache which has the correct model type set already
    let original_cache = db.cache.clone();
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
            }
        }
    });

    drop(files_to_process_sender);

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
    db.indexed_chunks.extend(processed_chunks_data.clone());

    db.cache = Arc::try_unwrap(updated_cache_arc)
        .expect("Failed to unwrap updated_cache Arc")
        .into_inner()
        .expect("Failed to get updated_cache from Mutex");

    Ok(processed_chunks_data)
}

fn rebuild_hnsw_index_from_state(db: &mut VectorDB, dimension: usize) -> Result<()> {
    debug!("Starting sequential HNSW index rebuild...");
    let start_time = Instant::now();

    if db.indexed_chunks.is_empty() {
        debug!("No chunks to index. Clearing existing index if any.");
        db.hnsw_index = None;
        return Ok(());
    }

    let config = db.hnsw_index.as_ref()
                   .map(|idx| idx.get_config().clone())
                   .filter(|cfg| cfg.dimension == dimension)
                   .unwrap_or_else(|| {
                        debug!("Creating new HNSW config for dimension {}.", dimension);
                        HNSWConfig::new(dimension)
                   });

    let mut hnsw_index = HNSWIndex::new(config);

    debug!("Building HNSW index sequentially for {} chunks...", db.indexed_chunks.len());
    let pb = ProgressBar::new(db.indexed_chunks.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} Building HNSW index: [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) ETA: {eta}")?
            .progress_chars("#>- ")
    );

    // Iterate mutably to update chunk IDs
    for chunk in db.indexed_chunks.iter_mut() {
        match hnsw_index.insert(chunk.embedding.clone()) {
            Ok(internal_id) => {
                // Assign the internal HNSW ID to the chunk's ID field
                chunk.id = internal_id;
            }
            Err(e) => {
                 error!("Fatal error inserting vector for chunk in file {} (lines {}-{}) into HNSW index: {}. Aborting build.",
                    chunk.file_path, chunk.start_line, chunk.end_line, e);
                 return Err(VectorDBError::HNSWError(format!(
                     "Failed to insert vector for chunk in {} into HNSW index during rebuild: {}", chunk.file_path, e
                )));
            }
        }
        pb.inc(1);
    }
    pb.finish_with_message("HNSW index build complete.");

    let duration = start_time.elapsed();
    debug!("Sequential HNSW index rebuild took {:.2?}", duration);

    db.hnsw_index = Some(hnsw_index);
    Ok(())
} 