use crate::{
    embedding::EmbeddingHandler,
    error::{Result, VectorDBError},
    embedding::provider::EmbeddingProvider,
    qdrant_client_trait::QdrantClientTrait,
    syntax::{self},
    constants::{ // Import necessary constants
        FIELD_FILE_PATH, FIELD_START_LINE, FIELD_END_LINE, FIELD_LANGUAGE, 
        FIELD_FILE_EXTENSION, FIELD_ELEMENT_TYPE, FIELD_CHUNK_CONTENT, 
        FIELD_BRANCH, FIELD_COMMIT_HASH, BATCH_SIZE, MAX_FILE_SIZE_BYTES, INTERNAL_EMBED_BATCH_SIZE
    },
    config::AppConfig, // Import AppConfig
};
use qdrant_client::{
    qdrant::PointStruct,
    Payload, // Corrected import location
};
use std::{
    collections::{HashSet},
    path::{PathBuf},
    sync::Arc, // Removed unused AtomicUsize, Ordering
    // cell::RefCell, // Added RefCell
    // thread_local, // Added thread_local
    time::Instant, // Added for timing
};
use walkdir::WalkDir;
use indicatif::ProgressBar; // Keep for progress reporting type hint
use uuid::Uuid; // Add Uuid import
use tokio::sync::Semaphore; // Import Semaphore
// use futures::future::try_join_all; // Removed unused
use crate::qdrant_ops::upsert_batch;
use rayon::prelude::*; // Added Rayon
use crate::syntax::parser::CodeChunk; // Use CodeChunk from parser
use crate::tokenizer::{self, TokenKind, TokenizerConfig}; // Import tokenizer module
use crate::vocabulary::VocabularyManager; // Import vocabulary manager
use qdrant_client::qdrant::{Vector, NamedVectors};
use std::collections::{HashMap};
use crate::config; // Import config module
use anyhow::anyhow; // Added for context in ensure_collection_exists
use crate::qdrant_ops; // Ensure qdrant_ops module is accessible for delete_collection_by_name
use crate::qdrant_ops::delete_collection_by_name;

// Add chunk size optimization constants
const MIN_CHUNK_SIZE: usize = 100;  // Merge chunks smaller than this
const MAX_CHUNK_SIZE: usize = 50_000;  // Split chunks larger than this
const TARGET_CHUNK_SIZE: usize = 1000;  // Aim for this size when splitting/merging
const MAX_BATCH_CONTENT_SIZE: usize = 1_000_000;  // 1MB of text per batch

/// Indexes files from specified paths into a Qdrant collection.
///
/// # Arguments
/// * `paths` - Vector of paths (files or directories) to index.
/// * `file_extensions` - Optional set of lowercase file extensions (without '.') to include.
/// * `collection_name` - The Qdrant collection name.
/// * `client` - An Arc-wrapped Qdrant client instance.
/// * `embedding_handler` - Reference to the initialized EmbeddingHandler.
/// * `progress` - Optional progress bar for reporting.
/// * `config` - The application configuration (needed for thread-local handlers).
///
/// # Returns
/// * `Result<(usize, usize)>` - (indexed files, indexed chunks/points)
pub async fn index_paths<
    C: QdrantClientTrait + Send + Sync + 'static // Make generic over trait
>(
    paths: &[PathBuf],
    file_extensions: Option<HashSet<String>>,
    collection_name: &str,
    client: Arc<C>, // Use generic client trait
    embedding_handler: &EmbeddingHandler,
    progress: Option<&ProgressBar>, // Pass progress bar reference
    config: &AppConfig, // Add AppConfig reference
) -> Result<(usize, usize)> { // Updated return type
    log::info!(
        "Core: Starting index process for {} paths into collection \"{}\"",
        paths.len(),
        collection_name
    );

    // --- 1. Ensure Collection Exists ---
    let embedding_dim = embedding_handler.dimension()?;
    // Collection creation now handles both dense and sparse implicitly via the trait impl
    ensure_collection_exists(client.clone(), collection_name, embedding_dim as u64).await?; 
    log::debug!("Core: Collection \"{}\" ensured.", collection_name);

    // --- Vocabulary Manager --- 
    // Use helper function to get the correct path
    let vocab_path = config::get_vocabulary_path(config, collection_name)?;
    let mut vocabulary_manager = if vocab_path.exists() {
        match VocabularyManager::load(&vocab_path) {
            Ok(vm) => {
                log::info!("Loaded existing vocabulary from {}", vocab_path.display());
                vm
            }
            Err(e) => {
                log::warn!("Failed to load vocabulary from {}: {}. Creating new.", vocab_path.display(), e);
                VocabularyManager::new()
            }
        }
    } else {
        log::info!("No vocabulary found at {}. Creating new.", vocab_path.display());
        VocabularyManager::new()
    };
    // --- End Vocabulary Manager --- 

    // --- 2. Gather Files ---
    let files_to_process = gather_files(paths, file_extensions)?;
    log::info!("Core: Found {} files to process.", files_to_process.len());

    if files_to_process.is_empty() {
        log::warn!("Core: No files found matching the criteria. Indexing complete.");
        if let Some(pb) = progress {
            pb.finish_with_message("No files found to index ");
        }
        return Ok((0, 0)); // Return zero counts
    }

    // --- 3. Process Files (Sequential Loop) ---
    if let Some(pb) = progress {
        pb.set_length(files_to_process.len() as u64);
        pb.set_position(0); // Reset position
        pb.set_message("Processing files...");
    }

    let mut points_batch = Vec::with_capacity(config.performance.batch_size);
    let mut files_processed_count = 0;
    let mut points_processed_count = 0;
    let total_files = files_to_process.len();

    // Create a single embedding model instance for sequential processing
    let dense_model = match embedding_handler.create_embedding_model() {
        Ok(m) => m,
        Err(e) => {
            log::error!("Failed to create embedding model for sequential indexing: {}", e);
            return Err(e.into());
        }
    };

    for (file_idx, file_path) in files_to_process.into_iter().enumerate() {
        log::debug!("Core: Processing file {}/{}: {}", file_idx + 1, total_files, file_path.display());

        let absolute_path_str = file_path.to_string_lossy().to_string();

        // --- File Size Check (existing code) --- 
        match std::fs::metadata(&file_path) {
             Ok(metadata) => {
                 if metadata.len() > config.performance.max_file_size_bytes {
                     log::warn!(
                         "Skipping file larger than {} bytes: {}",
                         config.performance.max_file_size_bytes,
                         file_path.display()
                     );
                     if let Some(pb) = progress { pb.inc(1); }
                     continue;
                 }
             }
             Err(e) => {
                 log::warn!(
                     "Failed to get metadata for file {}: {}. Skipping.",
                     file_path.display(), e
                 );
                 if let Some(pb) = progress { pb.inc(1); }
                 continue;
             }
         }
         // --- End File Size Check ---

        let io_start = Instant::now();
        let chunks: Vec<CodeChunk> = match syntax::get_chunks(&file_path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Skipping file due to parsing error: {} - {}", file_path.display(), e);
                if let Some(pb) = progress {
                    pb.inc(1); // Increment progress even if skipped
                }
                continue; // Skip this file
            }
        };

        let io_elapsed = io_start.elapsed();
        log::info!("[PROFILE] File I/O + chunking for {}: {:?}", file_path.display(), io_elapsed);
        log::info!("[PROFILE] Created {} chunks from file {} (avg chunk size: {} chars)", 
            chunks.len(),
            file_path.display(),
            if chunks.is_empty() { 0 } else { chunks.iter().map(|c| c.content.len()).sum::<usize>() / chunks.len() });

        if chunks.is_empty() {
            log::debug!("No code chunks found in file: {}", file_path.display());
            if let Some(pb) = progress {
                pb.inc(1);
            }
            continue;
        }

        let chunk_contents: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();

        // Generate dense embeddings sequentially using the single model instance
        match dense_model.embed_batch(&chunk_contents) {
            Ok(dense_embeddings) => {
                let file_extension = file_path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_string();

                for (i, chunk) in chunks.iter().enumerate() {
                    // --- Generate Sparse Vector (TF) --- 
                    let tokenizer_config = TokenizerConfig::default();
                    let tokens = tokenizer::tokenize_code(&chunk.content, &tokenizer_config);
                    let mut term_frequencies: HashMap<u32, u32> = HashMap::new();
                    for token in tokens {
                        // Filter out unwanted tokens (e.g., whitespace, maybe comments later)
                        if token.kind != TokenKind::Whitespace && token.kind != TokenKind::Unknown {
                             // TODO: Add option to filter comments
                             // TODO: Add option for case normalization (lowercase)
                             let token_text = token.text; // Use directly for now
                             let token_id = vocabulary_manager.add_token(&token_text);
                             *term_frequencies.entry(token_id).or_insert(0) += 1;
                        }
                    }
                    let sparse_indices: Vec<u32> = term_frequencies.keys().copied().collect();
                    let sparse_values: Vec<f32> = term_frequencies.values().map(|&count| count as f32).collect();
                    // --- End Generate Sparse Vector ---

                    // Create Payload
                    let mut payload = Payload::new();
                    payload.insert(FIELD_FILE_PATH, absolute_path_str.clone());
                    payload.insert(FIELD_START_LINE, chunk.start_line as i64);
                    payload.insert(FIELD_END_LINE, chunk.end_line as i64);
                    payload.insert(FIELD_LANGUAGE, chunk.language.clone());
                    payload.insert(FIELD_FILE_EXTENSION, file_extension.clone());
                    payload.insert(FIELD_ELEMENT_TYPE, chunk.element_type.to_string());
                    payload.insert(FIELD_CHUNK_CONTENT, chunk.content.clone());

                    // Create NamedVectors for both dense and sparse
                    let vectors = NamedVectors::default()
                        .add_vector("dense", Vector::new_dense(dense_embeddings[i].clone()))
                        .add_vector("sparse_tf", Vector::new_sparse(sparse_indices, sparse_values));

                    // Create PointStruct with NamedVectors
                    let point = PointStruct::new(
                        Uuid::new_v4().to_string(), // Generate unique ID for each chunk
                        vectors,
                        payload,
                    );
                    points_batch.push(point);
                    points_processed_count += 1;
                }
            }
            Err(e) => {
                log::warn!("Skipping file due to embedding error: {} - {}", file_path.display(), e);
                if let Some(pb) = progress {
                    pb.inc(1);
                }
                continue;
            }
        }

        files_processed_count += 1;

        // --- Upsert Batch Logic (existing code) ---
        if points_batch.len() >= BATCH_SIZE {
            log::debug!("Upserting batch of {} points...", points_batch.len());
            if let Err(e) = upsert_batch(client.clone(), collection_name, points_batch).await {
                 log::error!("Failed to upsert batch: {}", e); // Log error but continue?
                 // Decide on error handling: skip file, stop indexing?
            }
            points_batch = Vec::with_capacity(BATCH_SIZE); // Clear the batch
        }
        // --- End Upsert Batch Logic ---

        if let Some(pb) = progress {
            pb.inc(1);
        }
    }

    // --- Upsert Final Batch (existing code) ---
    if !points_batch.is_empty() {
        log::debug!("Upserting final batch of {} points...", points_batch.len());
        if let Err(e) = upsert_batch(client.clone(), collection_name, points_batch).await {
             log::error!("Failed to upsert final batch: {}", e);
        }
    }
    // --- End Upsert Final Batch ---

    // --- Save Vocabulary --- 
    if let Err(e) = vocabulary_manager.save(&vocab_path) {
        log::error!("Failed to save vocabulary to {}: {}", vocab_path.display(), e);
    } else {
        log::info!("Vocabulary saved to {}", vocab_path.display());
    }
    // --- End Save Vocabulary ---

    if let Some(pb) = progress {
        pb.finish_with_message(format!(
            "Indexed {} files ({} points)",
            files_processed_count,
            points_processed_count
        ));
    }

    log::info!(
        "Core: Finished indexing {} files, {} points processed into collection \"{}\"",
        files_processed_count,
        points_processed_count,
        collection_name
    );

    Ok((files_processed_count, points_processed_count))
}

/// Intermediate data structure holding info needed to build a PointStruct later.
struct IntermediatePointData {
    dense_vector: Vec<f32>,
    term_frequencies: HashMap<String, u32>, // Map of token text -> count
    payload_map: HashMap<String, qdrant_client::qdrant::Value>,
}

/// Processes a single file: reads content, extracts chunks, generates dense embeddings,
/// tokenizes chunks, calculates TF, and collects data needed for PointStruct creation.
/// Returns intermediate data and a set of unique tokens found.
fn process_single_file_for_indexing(
    full_path: &PathBuf,
    relative_path_str: &str, // Use &str for efficiency
    branch_name: &str,
    commit_hash: &str,
    embedding_provider: &crate::embedding::provider::onnx::OnnxEmbeddingModel, // Take reference directly
) -> std::result::Result<(Vec<IntermediatePointData>, HashSet<String>), String> { 
    use std::time::Instant;
    // --- File Size Check (as before) ---
    let io_start = Instant::now();
    let metadata = match std::fs::metadata(full_path) {
        Ok(m) => m,
        Err(e) => {
            return Err(format!(
                "Failed to get metadata for file {}: {}. Skipping.",
                full_path.display(), e
            ));
        }
    };
    if metadata.len() > MAX_FILE_SIZE_BYTES {
        return Err(format!(
            "Skipping file larger than {} bytes: {}",
            MAX_FILE_SIZE_BYTES,
            full_path.display()
        ));
    }
    if !metadata.is_file() {
        return Err(format!("Path is not a file, skipping: {}", full_path.display()));
    }
    // --- End File Size Check ---

    let chunks: Vec<CodeChunk> = match syntax::get_chunks(full_path) {
        Ok(c) => c,
        Err(e) => {
            return Err(format!("Skipping file due to parsing error: {} - {}", full_path.display(), e));
        }
    };
    let io_elapsed = io_start.elapsed();
    log::info!("[PROFILE] File I/O + chunking for {}: {:?}", full_path.display(), io_elapsed);
    log::info!("[PROFILE] Created {} chunks from file {} (avg chunk size: {} chars)", 
        chunks.len(),
        full_path.display(),
        if chunks.is_empty() { 0 } else { chunks.iter().map(|c| c.content.len()).sum::<usize>() / chunks.len() });

    if chunks.is_empty() {
        log::trace!("No code chunks found in file: {}", full_path.display());
        return Ok((Vec::new(), HashSet::new())); // No error, just no points/tokens
    }

    // --- Dense Embeddings (as before) --- 
    let embed_start = Instant::now();
    let mut all_dense_embeddings = Vec::with_capacity(chunks.len());
    log::info!("[PROFILE] Processing {} chunks from file {} with INTERNAL_EMBED_BATCH_SIZE={}", chunks.len(), full_path.display(), INTERNAL_EMBED_BATCH_SIZE);
    
    for chunk_batch in chunks.chunks(INTERNAL_EMBED_BATCH_SIZE) {
        let contents_batch: Vec<&str> = chunk_batch.iter().map(|c| c.content.as_str()).collect();
        log::info!("[PROFILE] Processing batch of {} chunks ({}% of file)", 
            contents_batch.len(),
            (contents_batch.len() as f32 / chunks.len() as f32 * 100.0) as u32);
        
        let batch_embed_start = Instant::now();
        // More granular timing
        let trait_call_start = Instant::now();
        let embed_result = embedding_provider.embed_batch(&contents_batch);
        let trait_call_elapsed = trait_call_start.elapsed();
        log::info!("[PROFILE] Time spent in embed_batch trait call for {} items in {}: {:?}", contents_batch.len(), full_path.display(), trait_call_elapsed);
        let after_trait_call = Instant::now();
        match embed_result {
            Ok(embeddings_batch) => {
                let after_trait_elapsed = after_trait_call.elapsed();
                log::info!("[PROFILE] Time spent after embed_batch trait call (result handling) for {} items in {}: {:?}", contents_batch.len(), full_path.display(), after_trait_elapsed);
                let batch_embed_elapsed = batch_embed_start.elapsed();
                log::info!("[PROFILE] Embedding batch ({} items) for {}: {:?}", contents_batch.len(), full_path.display(), batch_embed_elapsed);
                if embeddings_batch.len() != contents_batch.len() {
                    return Err(format!("Embedding batch size mismatch for {}", full_path.display()));
                }
                all_dense_embeddings.extend(embeddings_batch);
            }
            Err(e) => {
                return Err(format!("Failed to generate dense embeddings batch for {}: {}", full_path.display(), e));
            }
        }
    }
    let embed_elapsed = embed_start.elapsed();
    log::info!("[PROFILE] Total embedding time for {}: {:?}", full_path.display(), embed_elapsed);
    if all_dense_embeddings.len() != chunks.len() {
        return Err(format!("Embedding count mismatch after batching for {}", full_path.display()));
    }
    // --- End Dense Embeddings ---

    let file_extension = full_path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_string();
    let mut intermediate_data_list = Vec::with_capacity(chunks.len());
    let mut file_tokens = HashSet::new(); // Collect unique tokens for this file
    
    // Use default tokenizer config for indexing
    let tokenizer_config = TokenizerConfig::default(); 

    for (i, chunk) in chunks.iter().enumerate() {
        // --- Tokenize & Calculate TF & Collect Tokens --- 
        let token_start = Instant::now();
        let tokens = tokenizer::tokenize_code(&chunk.content, &tokenizer_config);
        let token_elapsed = token_start.elapsed();
        log::info!("[PROFILE] Tokenization for chunk {} in {}: {:?}", i, full_path.display(), token_elapsed);
        let mut term_frequencies: HashMap<String, u32> = HashMap::new(); 
        for token in tokens {
            // Filtering is now handled by tokenize_code based on config
            // TODO: Confirm if any additional filtering is needed here?
            let token_text = token.text; // Text is already potentially lowercased by tokenizer
            *term_frequencies.entry(token_text.clone()).or_insert(0) += 1;
            file_tokens.insert(token_text); // Add to file's unique token set
        }
        // --- End Tokenize ---

        // Create Payload Map
        let mut payload_map = HashMap::new();
        payload_map.insert(FIELD_FILE_PATH.to_string(), qdrant_client::qdrant::Value::from(relative_path_str.to_string()));
        payload_map.insert(FIELD_START_LINE.to_string(), qdrant_client::qdrant::Value::from(chunk.start_line as i64));
        payload_map.insert(FIELD_END_LINE.to_string(), qdrant_client::qdrant::Value::from(chunk.end_line as i64));
        payload_map.insert(FIELD_LANGUAGE.to_string(), qdrant_client::qdrant::Value::from(chunk.language.clone()));
        payload_map.insert(FIELD_FILE_EXTENSION.to_string(), qdrant_client::qdrant::Value::from(file_extension.clone()));
        payload_map.insert(FIELD_ELEMENT_TYPE.to_string(), qdrant_client::qdrant::Value::from(chunk.element_type.to_string()));
        payload_map.insert(FIELD_CHUNK_CONTENT.to_string(), qdrant_client::qdrant::Value::from(chunk.content.clone()));
        payload_map.insert(FIELD_BRANCH.to_string(), qdrant_client::qdrant::Value::from(branch_name.to_string())); 
        payload_map.insert(FIELD_COMMIT_HASH.to_string(), qdrant_client::qdrant::Value::from(commit_hash.to_string()));

        let intermediate_data = IntermediatePointData {
            dense_vector: all_dense_embeddings[i].clone(),
            term_frequencies,
            payload_map,
        };
        intermediate_data_list.push(intermediate_data);
    }

    Ok((intermediate_data_list, file_tokens))
}

/// Indexes specific files within a repository context into a Qdrant collection.
///
/// This function is optimized for repository indexing by:
/// - Accepting repo root and relative paths directly.
/// - Adding branch and commit hash to the payload.
/// - Using Rayon for parallel CPU-bound processing (parsing, embedding).
/// - Using a semaphore to control concurrent Qdrant upsert operations.
///
/// # Arguments
/// * `config` - The application configuration (needed for thread-local handlers).
/// * `repo_root` - The absolute path to the repository root.
/// * `relative_paths` - Slice of relative paths within the repo to index.
/// * `collection_name` - The Qdrant collection name.
/// * `branch_name` - The current branch name.
/// * `commit_hash` - The current commit hash.
/// * `client` - An Arc-wrapped Qdrant client instance.
/// * `embedding_handler` - Reference to the main EmbeddingHandler (used for dimension check).
/// * `progress` - Optional progress bar for reporting.
/// * `max_concurrent_upserts` - Maximum number of concurrent Qdrant upsert operations.
///
/// # Returns
/// * `Result<usize>` - Total number of points successfully processed and attempted to upsert.
pub async fn index_repo_files<
    C: QdrantClientTrait + Send + Sync + 'static // Make generic over trait
>(
    config: &AppConfig, // Pass full config needed for Vocab path etc.
    repo_root: &PathBuf,
    relative_paths: &[PathBuf],
    collection_name: &str,
    branch_name: &str,
    commit_hash: &str,
    client: Arc<C>, // Use generic client trait
    embedding_handler: Arc<EmbeddingHandler>, // Kept for dimension, but not used for parallel embedding
    progress: Option<&ProgressBar>,
    max_concurrent_upserts: usize,
) -> Result<usize> {
    log::info!(
        "Core: Starting parallel repo index process for {} files into collection \"{}\" (branch: {}, commit: {})",
        relative_paths.len(),
        collection_name,
        branch_name,
        &commit_hash[..8] // Log abbreviated commit hash
    );

    if relative_paths.is_empty() {
        log::warn!("Core: No relative paths provided for repo indexing.");
        if let Some(pb) = progress {
            pb.finish_with_message("No files provided to index");
        }
        return Ok(0);
    }

    // --- 1. Ensure Collection Exists ---
    let embedding_dim = embedding_handler.dimension()?; // Use the passed handler
    ensure_collection_exists(client.clone(), collection_name, embedding_dim as u64).await?; // Pass dereferenced client
    log::debug!("Core: Collection \"{}\" ensured.", collection_name);

    // --- 2. Process Files in Parallel (CPU Bound) ---
    if let Some(pb) = progress {
        pb.set_length(relative_paths.len() as u64);
        pb.set_position(0); // Reset position
        pb.set_message("Parsing & Embedding Files...");
    }

    // Get ONNX model and tokenizer paths from the handler
    let onnx_model_path = embedding_handler.onnx_model_path().ok_or_else(|| {
        VectorDBError::EmbeddingError("ONNX model path not set in handler.".to_string())
    })?;
    let onnx_tokenizer_path = embedding_handler.onnx_tokenizer_path().ok_or_else(|| {
        VectorDBError::EmbeddingError("ONNX tokenizer path not set in handler.".to_string())
    })?;

    // Call the parallel processing function to get intermediate data
    let (all_intermediate_data, all_token_sets, processing_errors) = process_repo_files_parallel(
        onnx_model_path,
        onnx_tokenizer_path,
        repo_root,
        relative_paths,
        branch_name,
        commit_hash,
        progress,
    );

    let total_points_generated = all_intermediate_data.len();
    let files_processed_successfully = relative_paths.len() - processing_errors.len();
    log::info!(
        "Core: Parallel processing complete. Generated intermediate data for {} points from {} files ({} errors encountered).",
        total_points_generated,
        files_processed_successfully,
        processing_errors.len()
    );
    if let Some(pb) = progress {
        pb.set_position(relative_paths.len() as u64); 
        pb.set_message("Building Vocabulary...");
    }

    // --- 3. Build/Update Vocabulary (Sequential) ---
    // Use helper function to get the correct path
    let vocab_path = config::get_vocabulary_path(config, collection_name)?;
    let mut vocabulary_manager = if vocab_path.exists() {
        match VocabularyManager::load(&vocab_path) {
            Ok(vm) => {
                log::info!("Loaded existing vocabulary ({} terms) from {}", vm.len(), vocab_path.display());
                vm
            }
            Err(e) => {
                log::warn!("Failed to load vocabulary from {}: {}. Creating new.", vocab_path.display(), e);
                VocabularyManager::new()
            }
        }
    } else {
        log::info!("No vocabulary found at {}. Creating new.", vocab_path.display());
        VocabularyManager::new()
    };

    let initial_vocab_size = vocabulary_manager.len();
    for token_set in all_token_sets {
        for token in token_set {
            vocabulary_manager.add_token(&token); // Add tokens from all files
        }
    }
    let final_vocab_size = vocabulary_manager.len();
    log::info!("Vocabulary updated. Size: {} -> {}", initial_vocab_size, final_vocab_size);

    // Save the updated vocabulary, regardless of whether files were processed
    log::info!("Attempting to save vocabulary for collection '{}' to path: {}", collection_name, vocab_path.display());
    vocabulary_manager.save(&vocab_path).map_err(|e| {
        log::error!("FATAL: Failed to save updated vocabulary to {}: {}", vocab_path.display(), e);
        // Convert the vocabulary error into a VectorDBError
        VectorDBError::Other(format!("Failed to save vocabulary: {}", e)) 
    })?; // Use ? to return error immediately if save fails

    log::info!("Updated vocabulary saved to {}", vocab_path.display());
    // --- End Vocabulary ---

    // Check if there are any points to construct/upload before proceeding
    if all_intermediate_data.is_empty() {
        log::info!("No intermediate data generated (likely no indexable files found or processed). Skipping point construction and upload.");
        // Ensure progress bar finishes cleanly if it exists
        if let Some(pb) = progress {
            pb.finish_with_message(format!("No indexable files found in {} files checked", relative_paths.len()));
        }
        // Return Ok(0) because no points were indexed, but the process (incl. vocab save) was successful
        return Ok(0); 
    }

    // --- 4. Construct Final Points (Sequential) ---
    if let Some(pb) = progress {
        pb.set_length(all_intermediate_data.len() as u64); // Total points to construct
        pb.set_position(0);
        pb.set_message("Constructing Points for Upload...");
    }
    let mut final_points: Vec<PointStruct> = Vec::with_capacity(all_intermediate_data.len());
    for intermediate in all_intermediate_data {
        let mut sparse_indices = Vec::new();
        let mut sparse_values = Vec::new();
        for (token_text, tf_count) in intermediate.term_frequencies {
            if let Some(token_id) = vocabulary_manager.get_id(&token_text) {
                sparse_indices.push(token_id);
                sparse_values.push(tf_count as f32);
            } else {
                // This shouldn't happen if vocab was built correctly from all tokens
                log::warn!("Token '{}' found in TF map but not in final vocabulary!", token_text);
            }
        }

        let vectors = NamedVectors::default()
            .add_vector("dense", Vector::new_dense(intermediate.dense_vector))
            .add_vector("sparse_tf", Vector::new_sparse(sparse_indices, sparse_values));

        let point = PointStruct::new(
            Uuid::new_v4().to_string(), // Consider using a deterministic ID if needed
            vectors,
            intermediate.payload_map, // Pass HashMap directly, From will be called
        );
        final_points.push(point);
         if let Some(pb) = progress {
             pb.inc(1);
         }
    }
    log::info!("Constructed {} final points with dense and sparse vectors.", final_points.len());
    // --- End Construct Points ---

    // --- 5. Upload Points to Qdrant (Network Bound, Concurrent) ---
    let semaphore = Arc::new(Semaphore::new(max_concurrent_upserts));
    log::info!("Using max_concurrent_upserts: {}", max_concurrent_upserts);
    let mut upsert_tasks = Vec::new();
    let mut total_points_attempted_upsert = 0;

    // Iterate over the final points and create concurrent upload tasks
    for points_batch in final_points.chunks(BATCH_SIZE) {
        if points_batch.is_empty() {
            continue;
        }
        total_points_attempted_upsert += points_batch.len();
        let batch_to_upsert = points_batch.to_vec(); // Clone the batch for the async task
        let client_clone = client.clone();
        let collection_name_clone = collection_name.to_string();
        let semaphore_clone = semaphore.clone();

        let task = tokio::spawn(async move {
            let permit = semaphore_clone.acquire_owned().await.expect("Semaphore acquisition failed");
            let result = upsert_batch(client_clone, &collection_name_clone, batch_to_upsert).await;
            drop(permit); // Release semaphore permit
            result
        });
        upsert_tasks.push(task);
    }

    if let Some(pb) = progress {
         pb.reset(); // Reset progress bar for upload phase
         pb.set_length(upsert_tasks.len() as u64); // Update progress bar length for upload tasks
         pb.set_position(0);
         pb.set_message(format!("Uploading {} batches...", upsert_tasks.len()));
    }

    let mut upsert_errors: Vec<VectorDBError> = Vec::new(); 
    let total_tasks = upsert_tasks.len();
    for (i, task) in upsert_tasks.into_iter().enumerate() {
         match task.await {
             Ok(Ok(())) => { 
                 // Batch succeeded
             },
             Ok(Err(e)) => { 
                 log::error!("Batch upsert task failed: {}", e);
                 upsert_errors.push(e.into()); 
             },
             Err(join_err) => { 
                 log::error!("Tokio task join error during upsert: {}", join_err);
                 upsert_errors.push(VectorDBError::Other(format!("Tokio task join error: {}", join_err)));
             },
         }
         if let Some(pb) = progress {
             pb.inc(1);
             // pb.set_message(format!("Uploaded {}/{} batches", i + 1, total_tasks));
         }
    }

    // Log processing errors 
    if !processing_errors.is_empty() {
        log::warn!("Encountered {} errors during file processing:", processing_errors.len());
        for e in processing_errors.iter().take(10) { 
            log::warn!("  - {}", e);
        }
        if processing_errors.len() > 10 {
            log::warn!("  ... and {} more processing errors.", processing_errors.len() - 10);
        }
    }

    // Handle upsert errors
    if !upsert_errors.is_empty() {
        log::error!("Encountered {} errors during Qdrant upsert:", upsert_errors.len());
        for e in upsert_errors.iter().take(10) {
            log::error!("  - {}", e);
        }
        if upsert_errors.len() > 10 {
            log::error!("  ... and {} more upsert errors.", upsert_errors.len() - 10);
        }
        // Return the first upsert error encountered
        return Err(upsert_errors.remove(0).into());
    }

    if let Some(pb) = progress {
        pb.finish_with_message(format!("Upload complete for {} points", total_points_attempted_upsert));
    }

    log::info!(
        "Core: Repo indexing finished. Processed {} files, attempted upsert for {} points.",
        files_processed_successfully,
        total_points_attempted_upsert
    );
    Ok(total_points_attempted_upsert) // Return total points attempted to upsert
}

/// Holds information about a chunk and its source file for batch processing
#[derive(Clone)]
struct ChunkWithMetadata {
    chunk: CodeChunk,
    file_path: PathBuf,
    relative_path: String,
    branch: String,
    commit: String,
}

/// Normalize chunks to be within size thresholds
fn normalize_chunks(chunks: Vec<CodeChunk>) -> Vec<CodeChunk> {
    let mut normalized = Vec::new();
    let mut current_chunk: Option<CodeChunk> = None;
    let mut current_size = 0;

    for chunk in chunks {
        let chunk_size = chunk.content.len();

        // If chunk is too large, split it
        if chunk_size > MAX_CHUNK_SIZE {
            if let Some(c) = current_chunk {
                normalized.push(c);
            }
            
            // Split large chunk into smaller ones
            let mut content = chunk.content;
            while !content.is_empty() {
                let split_point = content.char_indices()
                    .take_while(|(i, _)| *i < TARGET_CHUNK_SIZE)
                    .last()
                    .map(|(i, char_val)| i + char_val.len_utf8())
                    .unwrap_or(content.len());

                let split_content = content[..split_point].to_string();
                content = content[split_point..].to_string();

                normalized.push(CodeChunk {
                    content: split_content,
                    file_path: chunk.file_path.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    language: chunk.language.clone(),
                    element_type: chunk.element_type.clone(),
                });
            }
            current_chunk = None;
            current_size = 0;
        }
        // If chunk is too small, try to merge with previous
        else if chunk_size < MIN_CHUNK_SIZE {
            match &mut current_chunk {
                Some(c) if current_size + chunk_size <= TARGET_CHUNK_SIZE => {
                    // Merge with current chunk
                    c.content.push_str("\n");
                    c.content.push_str(&chunk.content);
                    c.end_line = chunk.end_line;
                    current_size += chunk_size + 1;
                }
                _ => {
                    // Push current and start new
                    if let Some(c) = current_chunk {
                        normalized.push(c);
                    }
                    current_chunk = Some(chunk);
                    current_size = chunk_size;
                }
            }
        }
        // Normal sized chunk
        else {
            if let Some(c) = current_chunk {
                normalized.push(c);
            }
            normalized.push(chunk);
            current_chunk = None;
            current_size = 0;
        }
    }

    // Don't forget the last chunk
    if let Some(c) = current_chunk {
        normalized.push(c);
    }

    normalized
}

/// Parallel processing function for repository files.
/// Returns intermediate data for point construction and sets of unique tokens per file.
fn process_repo_files_parallel(
    onnx_model_path: &std::path::Path,
    onnx_tokenizer_path: &std::path::Path,
    repo_root: &PathBuf,
    relative_paths: &[PathBuf],
    branch_name: &str,
    commit_hash: &str,
    progress: Option<&ProgressBar>,
) -> (Vec<IntermediatePointData>, Vec<HashSet<String>>, Vec<String>) {
    use std::cell::RefCell;
    use crate::embedding::provider::onnx::OnnxEmbeddingModel;
    use std::sync::Mutex;
    thread_local! {
        static ONNX_MODEL: RefCell<Option<OnnxEmbeddingModel>> = RefCell::new(None);
    }

    let mut all_intermediate_data = Vec::new();
    let mut all_token_sets = Vec::new();
    let mut processing_errors = Vec::new();
    let current_batch: Arc<Mutex<Vec<ChunkWithMetadata>>> = Arc::new(Mutex::new(Vec::new()));
    let mut current_file_tokens = HashSet::new();

    // Process function for a batch of chunks
    let process_batch = |batch: &[ChunkWithMetadata]| -> Result<Vec<Vec<f32>>> {
        let contents: Vec<&str> = batch.iter()
            .map(|c| c.chunk.content.as_str())
            .collect();

        let embed_start = Instant::now();
        let embeddings = ONNX_MODEL.with(|cell| {
            let model = cell.borrow();
            model.as_ref().unwrap().embed_batch(&contents)
        })?;
        let embed_elapsed = embed_start.elapsed();
        log::info!("[PROFILE] Cross-file batch embedding time for {} chunks: {:?}", contents.len(), embed_elapsed);

        Ok(embeddings)
    };

    // Process files in parallel
    let results: Vec<_> = relative_paths.par_iter().map(|relative_path| {
        // Initialize ONNX model for this thread if not already initialized
        ONNX_MODEL.with(|cell| {
            let mut model_opt = cell.borrow_mut();
            if model_opt.is_none() {
                *model_opt = Some(OnnxEmbeddingModel::new(onnx_model_path, onnx_tokenizer_path)
                    .expect("Failed to create thread-local ONNX model instance"));
            }
        });

        let full_path = repo_root.join(relative_path);
        let relative_path_str = relative_path.to_string_lossy().to_string();
        
        // File I/O and chunking
        let io_start = Instant::now();
        let chunks = match syntax::get_chunks(&full_path) {
            Ok(c) => normalize_chunks(c),
            Err(e) => {
                let error_msg = format!("Failed to process {}: {}", full_path.display(), e);
                return Err(error_msg);
            }
        };
        let io_elapsed = io_start.elapsed();
        log::info!("[PROFILE] File I/O + chunking for {}: {:?}", full_path.display(), io_elapsed);
        log::info!("[PROFILE] Created {} chunks from file {} (avg chunk size: {} chars)", 
            chunks.len(),
            full_path.display(),
            if chunks.is_empty() { 0 } else { chunks.iter().map(|c| c.content.len()).sum::<usize>() / chunks.len() });

        // Add chunks to the current batch
        let mut file_tokens = HashSet::new();
        let mut file_intermediate_data = Vec::new();

        for chunk in chunks {
            let chunk_meta = ChunkWithMetadata {
                chunk,
                file_path: full_path.clone(),
                relative_path: relative_path_str.clone(),
                branch: branch_name.to_string(),
                commit: commit_hash.to_string(),
            };

            // Add to shared batch
            let mut batch = current_batch.lock().unwrap();
            let batch_content_size: usize = batch.iter().map(|c| c.chunk.content.len()).sum();
            
            if batch_content_size + chunk_meta.chunk.content.len() <= MAX_BATCH_CONTENT_SIZE 
                && batch.len() < INTERNAL_EMBED_BATCH_SIZE {
                batch.push(chunk_meta);
            } else {
                // Process current batch before adding new chunk
                let batch_to_process = std::mem::take(&mut *batch);
                drop(batch);

                if !batch_to_process.is_empty() {
                    match process_batch(&batch_to_process) {
                        Ok(embeddings) => {
                            // Process results and create intermediate data
                            for (i, chunk_meta) in batch_to_process.iter().enumerate() {
                                let file_extension = chunk_meta.file_path.extension()
                                    .and_then(|ext| ext.to_str())
                                    .unwrap_or("")
                                    .to_string();

                                // Generate sparse vectors and collect tokens
                                let tokenizer_config = TokenizerConfig::default();
                                let tokens = tokenizer::tokenize_code(&chunk_meta.chunk.content, &tokenizer_config);
                                let mut term_frequencies = HashMap::new();
                                for token in tokens {
                                    if token.kind != TokenKind::Whitespace && token.kind != TokenKind::Unknown {
                                        let token_text = token.text;
                                        file_tokens.insert(token_text.clone());
                                        *term_frequencies.entry(token_text).or_insert(0) += 1;
                                    }
                                }

                                // Create payload
                                let mut payload_map = HashMap::new();
                                payload_map.insert(FIELD_FILE_PATH.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.relative_path.clone()));
                                payload_map.insert(FIELD_START_LINE.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.chunk.start_line as i64));
                                payload_map.insert(FIELD_END_LINE.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.chunk.end_line as i64));
                                payload_map.insert(FIELD_LANGUAGE.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.chunk.language.clone()));
                                payload_map.insert(FIELD_FILE_EXTENSION.to_string(), qdrant_client::qdrant::Value::from(file_extension));
                                payload_map.insert(FIELD_ELEMENT_TYPE.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.chunk.element_type.to_string()));
                                payload_map.insert(FIELD_CHUNK_CONTENT.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.chunk.content.clone()));
                                payload_map.insert(FIELD_BRANCH.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.branch.clone()));
                                payload_map.insert(FIELD_COMMIT_HASH.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.commit.clone()));

                                file_intermediate_data.push(IntermediatePointData {
                                    dense_vector: embeddings[i].clone(),
                                    term_frequencies,
                                    payload_map,
                                });
                            }
                        }
                        Err(e) => {
                            return Err(format!("Failed to process batch: {}", e));
                        }
                    }
                }
            }
        }

        // Update progress after processing each file
        if let Some(pb) = progress {
            pb.inc(1);
        }

        Ok((file_intermediate_data, file_tokens))
    }).collect();

    // ==================================================================================
    // IMPORTANT: GPU Memory Management - DO NOT REMOVE THIS SECTION!
    // ==================================================================================
    // The following code is critical for preventing GPU Out-of-Memory (OOM) errors.
    //
    // Problem: During parallel processing, each Rayon worker thread creates its own 
    // thread-local ONNX model in GPU memory. When the main thread tries to process the
    // final batch afterward, it attempts to create yet another model instance, which can
    // exhaust available VRAM and cause a crash.
    //
    // Solution: Before the main thread processes the final batch, we explicitly force all
    // worker threads to drop their thread-local ONNX models, freeing GPU memory. This is
    // done by:
    // 1. Creating a parallel job that accesses each thread's thread-local storage
    // 2. Taking ownership of the model (via Option::take()) which drops it when the scope ends
    // 3. Adding a short sleep to allow the GPU driver to properly reclaim memory
    //
    // Alternative approaches like pooling models or restructuring the batching logic would
    // require more extensive changes to the codebase.
    //
    // WARNING: Removing this cleanup code may cause GPU OOM errors on large repositories!
    // ==================================================================================
    log::info!("Cleaning up thread-local ONNX models before processing final batch");
    (0..num_cpus::get()).into_par_iter().for_each(|_| {
        // Access thread_local to force it to this thread, then explicitly drop it
        ONNX_MODEL.with(|cell| {
            let mut model_opt = cell.borrow_mut();
            // Take the model out (if it exists) which will drop it when this scope ends
            if model_opt.is_some() {
                log::debug!("Dropping thread-local ONNX model to free GPU memory");
                let _ = model_opt.take();
            }
        });
    });

    // Small delay to allow GPU memory to be properly released
    std::thread::sleep(std::time::Duration::from_millis(200));
    log::info!("Thread-local models cleaned up, proceeding with final batch processing");

    // Process any remaining chunks in the final batch
    // Lock the mutex to get access to the batch, then clone it to release the lock ASAP.
    let final_batch_data: Vec<ChunkWithMetadata> = {
        let batch_guard = current_batch.lock().unwrap();
        batch_guard.clone()
    }; // MutexGuard is dropped here

    if !final_batch_data.is_empty() {
        log::info!("Processing final batch of {} chunks on the main thread.", final_batch_data.len());

        // Create a dedicated, local ONNX model for the main thread's final batch.
        // This happens AFTER worker threads have cleaned up their models.
        let main_thread_final_batch_model = OnnxEmbeddingModel::new(onnx_model_path, onnx_tokenizer_path)
            .map_err(|e| {
                log::error!("Failed to create main-thread ONNX model for final batch: {:?}", e);
                // Assuming VectorDBError can be created from a String or anyhow::Error
                // Adjust error creation as per your VectorDBError definition
                VectorDBError::EmbeddingError(format!("Failed to create main-thread ONNX model for final batch: {}", e))
            })
            .expect("Main thread ONNX model creation for final batch failed"); // Or handle Result if preferred

        // Temporarily place the local model into the main thread's thread_local slot
        // so that `process_batch` (which uses the thread_local) can find it.
        ONNX_MODEL.with(|cell| {
            let mut model_opt = cell.borrow_mut();
            // It should have been cleared by the parallel cleanup if the main thread participated,
            // or it was never set if the main thread didn't run a worker task.
            if model_opt.is_some() {
                log::warn!("Main thread's ONNX_MODEL was already Some before final batch processing. This is unexpected. Taking existing model.");
                let _ = model_opt.take(); // Drop any unexpected existing model
            }
            *model_opt = Some(main_thread_final_batch_model); // Move our local model in
        });

        let embeddings_result = process_batch(&final_batch_data);

        // CRUCIALLY: Take the model back out of the thread_local immediately after use.
        // The model (now in `taken_model_option`) will be dropped when this scope ends.
        let mut taken_model_option: Option<OnnxEmbeddingModel> = None;
        ONNX_MODEL.with(|cell| {
            let mut model_opt = cell.borrow_mut();
            if model_opt.is_some() {
                log::debug!("Taking back main thread's final batch ONNX model from thread_local to ensure it's dropped.");
                taken_model_option = model_opt.take();
            } else {
                log::warn!("Main thread's ONNX_MODEL was None after final batch processing. Model might have been taken elsewhere or not set.");
            }
        });
        // `taken_model_option` (containing the model used for the final batch) will drop here if it's Some.

        if let Ok(embeddings) = embeddings_result {
            for (i, chunk_meta) in final_batch_data.iter().enumerate() { // Iterate over final_batch_data
                let file_extension = chunk_meta.file_path.extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("")
                    .to_string();

                // Generate sparse vectors and collect tokens
                let tokenizer_config = TokenizerConfig::default();
                let tokens = tokenizer::tokenize_code(&chunk_meta.chunk.content, &tokenizer_config);
                let mut term_frequencies = HashMap::new();
                for token in tokens {
                    if token.kind != TokenKind::Whitespace && token.kind != TokenKind::Unknown {
                        let token_text = token.text;
                        current_file_tokens.insert(token_text.clone());
                        *term_frequencies.entry(token_text).or_insert(0) += 1;
                    }
                }

                // Create payload
                let mut payload_map = HashMap::new();
                payload_map.insert(FIELD_FILE_PATH.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.relative_path.clone()));
                payload_map.insert(FIELD_START_LINE.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.chunk.start_line as i64));
                payload_map.insert(FIELD_END_LINE.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.chunk.end_line as i64));
                payload_map.insert(FIELD_LANGUAGE.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.chunk.language.clone()));
                payload_map.insert(FIELD_FILE_EXTENSION.to_string(), qdrant_client::qdrant::Value::from(file_extension));
                payload_map.insert(FIELD_ELEMENT_TYPE.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.chunk.element_type.to_string()));
                payload_map.insert(FIELD_CHUNK_CONTENT.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.chunk.content.clone()));
                payload_map.insert(FIELD_BRANCH.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.branch.clone()));
                payload_map.insert(FIELD_COMMIT_HASH.to_string(), qdrant_client::qdrant::Value::from(chunk_meta.commit.clone()));

                all_intermediate_data.push(IntermediatePointData {
                    dense_vector: embeddings[i].clone(),
                    term_frequencies,
                    payload_map,
                });
            }
        }
    }

    // Collect results from parallel processing
    for result in results {
        match result {
            Ok((file_data, file_tokens)) => {
                all_intermediate_data.extend(file_data);
                all_token_sets.push(file_tokens);
            }
            Err(e) => {
                processing_errors.push(e);
            }
        }
    }

    // Add any remaining tokens from the last file
    if !current_file_tokens.is_empty() {
        all_token_sets.push(current_file_tokens);
    }

    (all_intermediate_data, all_token_sets, processing_errors)
}

/// Gathers file paths from a list of starting paths, respecting ignore rules and file extensions.
/// Recursively walks directories.
pub fn gather_files(
    paths: &[PathBuf],
    file_extensions: Option<HashSet<String>>,
) -> Result<Vec<PathBuf>> {
    let mut collected_files = Vec::new();

    for path_arg in paths {
        // Ensure paths are absolute before processing
        let absolute_path_arg = if path_arg.is_absolute() {
            path_arg.clone()
        } else {
             // Attempt to make absolute relative to CWD. Handle error appropriately.
             std::env::current_dir()?.join(path_arg)
        };
        // Further canonicalization might be needed depending on usage
        // let absolute_path_arg = absolute_path_arg.canonicalize().with_context(|| format!("Failed to get absolute path for: {}", path_arg.display()))?;


        if !absolute_path_arg.exists() {
            log::warn!("Input path does not exist, skipping: {}", absolute_path_arg.display());
            continue;
        }

        if absolute_path_arg.is_file() {
            let should_process = match &file_extensions {
                Some(filter_set) => {
                    let extension = absolute_path_arg
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|s| s.to_lowercase())
                        .unwrap_or_default();
                    filter_set.contains(&extension)
                }
                None => true, // No filter means include all
            };
            if should_process {
                collected_files.push(absolute_path_arg);
            } else {
                log::debug!("Skipping file due to extension filter: {}", absolute_path_arg.display());
            }
        } else if absolute_path_arg.is_dir() {
            log::debug!("Walking directory: {}", absolute_path_arg.display());
            for entry_result in WalkDir::new(&absolute_path_arg)
                    .into_iter()
                    .filter_entry(|e| !is_hidden(e) && !is_target_dir(e)) // Add filtering here
                    .filter_map(|e| e.ok()) // Process only valid entries
            {
                let entry_path = entry_result.path();
                if !entry_path.is_file() {
                    continue;
                }
                let should_process = match &file_extensions {
                    Some(filter_set) => {
                        let extension = entry_path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .map(|s| s.to_lowercase())
                            .unwrap_or_default();
                        filter_set.contains(&extension)
                    }
                    None => true, // No filter means include all
                };
                if should_process {
                    // Store absolute path directly
                    collected_files.push(entry_path.to_path_buf());
                } else {
                     log::trace!("Skipping file in dir due to extension filter: {}", entry_path.display());
                }
            }
        }
    }
    Ok(collected_files)
}

/// Ensures a Qdrant collection exists with the specified dimension.
/// If it exists with a different dimension, it will be deleted and recreated.
/// TODO: Move payload index creation logic here from src/cli/commands/mod.rs
pub async fn ensure_collection_exists<
    C: QdrantClientTrait + Send + Sync + 'static // Add Send + Sync + 'static for Arc<C>
>(
    client: Arc<C>, // Changed to Arc<C> to allow calling delete_collection_by_name
    collection_name: &str,
    embedding_dimension: u64,
) -> Result<()> { // Result is anyhow::Result from the context of vectordb_core::indexing
    if client.collection_exists(collection_name.to_string()).await? {
        log::debug!("Collection '{}' already exists. Verifying dimension...", collection_name);
        let collection_info = client.get_collection_info(collection_name.to_string()).await?;
        
        let mut current_dimension: Option<u64> = None;
        if let Some(config) = collection_info.config {
            if let Some(params) = config.params {
                if let Some(vectors_config) = params.vectors_config {
                    if let Some(config) = vectors_config.config {
                        match config {
                            qdrant_client::qdrant::vectors_config::Config::ParamsMap(params_map) => {
                                if let Some(dense_params) = params_map.map.get("dense") {
                                    current_dimension = Some(dense_params.size);
                                }
                            }
                            _ => {
                                log::warn!("Collection '{}' has unexpected vector config type", collection_name);
                            }
                        }
                    }
                }
            }
        }

        if let Some(dim) = current_dimension {
            if dim != embedding_dimension {
                log::warn!(
                    "Collection '{}' exists but has dimension {} instead of expected {}. Recreating...",
                    collection_name,
                    dim,
                    embedding_dimension
                );
                delete_collection_by_name(client.clone(), collection_name).await?;
            } else {
                log::debug!("Collection '{}' exists with correct dimension {}.", collection_name, dim);
                return Ok(());
            }
        } else {
            log::warn!(
                "Collection '{}' exists but could not determine its dimension. Recreating...",
                collection_name
            );
            delete_collection_by_name(client.clone(), collection_name).await?;
        }
    }

    log::info!("Creating collection '{}' with dimension {}...", collection_name, embedding_dimension);
    match client.create_collection(collection_name, embedding_dimension).await {
        Ok(true) => {
            log::info!("Collection '{}' created/recreated successfully.", collection_name);
            Ok(())
        }
        Ok(false) => {
            log::error!("Qdrant reported false for collection creation/recreation of '{}', though no direct error was returned.", collection_name);
            Err(VectorDBError::QdrantOperationError(format!("Qdrant reported false for collection creation/recreation of '{}'", collection_name)))
        }
        Err(e) => {
            log::error!("Failed to create/recreate collection '{}': {:?}", collection_name, e);
            Err(e)
        }
    }
    // TODO: Payload index creation logic should be here, called after collection is confirmed to exist with correct schema.
}

// Helper functions for filtering files (moved from src/vectordb/indexing.rs)
/// Checks if a directory entry is hidden (starts with a dot). 
pub fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| s.starts_with('.'))
         .unwrap_or(false)
}

/// Checks if a directory entry is the `target` directory.
pub fn is_target_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_name() == "target" && entry.file_type().is_dir()
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use walkdir::DirEntry; // Keep this use
    use std::fs::{self, File};
    use tempfile::tempdir;
    
    // Helper function to create a DirEntry from a path (specific to these tests)
    fn create_dir_entry(path: &PathBuf, is_dir: bool) -> DirEntry {
        walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|r| r.ok()) // Use explicit closure
            .find(|e| e.path() == path && e.file_type().is_dir() == is_dir)
            .expect("Failed to create DirEntry")
    }
    
    #[test]
    fn test_is_hidden() {
        let temp_dir = tempdir().unwrap();
        
        // Create a hidden directory
        let hidden_dir_path = temp_dir.path().join(".hidden_dir");
        fs::create_dir(&hidden_dir_path).unwrap();
        
        // Create a regular directory
        let regular_dir_path = temp_dir.path().join("regular_dir");
        fs::create_dir(&regular_dir_path).unwrap();
        
        // Create a hidden file
        let hidden_file_path = temp_dir.path().join(".hidden_file");
        File::create(&hidden_file_path).unwrap();
        
        // Create a regular file
        let regular_file_path = temp_dir.path().join("regular_file");
        File::create(&regular_file_path).unwrap();
        
        // Test hidden directory
        let hidden_dir_entry = create_dir_entry(&hidden_dir_path, true);
        assert!(is_hidden(&hidden_dir_entry));
        
        // Test regular directory
        let regular_dir_entry = create_dir_entry(&regular_dir_path, true);
        assert!(!is_hidden(&regular_dir_entry));
        
        // Test hidden file
        let hidden_file_entry = create_dir_entry(&hidden_file_path, false);
        assert!(is_hidden(&hidden_file_entry));
        
        // Test regular file
        let regular_file_entry = create_dir_entry(&regular_file_path, false);
        assert!(!is_hidden(&regular_file_entry));
    }
    
    #[test]
    fn test_is_target_dir() {
        let temp_dir = tempdir().unwrap();
        
        // Create a target directory
        let target_dir_path = temp_dir.path().join("target");
        fs::create_dir(&target_dir_path).unwrap();
        
        // Create a regular directory
        let regular_dir_path = temp_dir.path().join("regular_dir");
        fs::create_dir(&regular_dir_path).unwrap();
        
        // Create a file named "target"
        let target_file_path = temp_dir.path().join("target_file");
        File::create(&target_file_path).unwrap();
        
        // Test target directory
        let target_dir_entry = create_dir_entry(&target_dir_path, true);
        assert!(is_target_dir(&target_dir_entry));
        
        // Test regular directory
        let regular_dir_entry = create_dir_entry(&regular_dir_path, true);
        assert!(!is_target_dir(&regular_dir_entry));
        
        
        // Test file named "target" (should return false since it's not a directory)
        let target_file_entry = create_dir_entry(&target_file_path, false);
        assert!(!is_target_dir(&target_file_entry));
    }
}

