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
    sync::{Arc, atomic::{AtomicUsize, Ordering}}, // Removed unused AtomicUsize, Ordering
    // cell::RefCell, // Added RefCell
    // thread_local, // Added thread_local
    // time::Instant, // Added for timing
};
use walkdir::WalkDir;
use indicatif::ProgressBar; // Keep for progress reporting type hint
use uuid::Uuid; // Add Uuid import
use tokio::sync::Semaphore; // Import Semaphore
// use futures::future::try_join_all; // Removed unused
use crate::qdrant_ops::upsert_batch;
use rayon::prelude::*; // Added Rayon
use crate::syntax::parser::CodeChunk; // Use CodeChunk from parser
use crate::tokenizer::{self, Token, TokenKind, TokenizerConfig}; // Import tokenizer module
use crate::vocabulary::VocabularyManager; // Import vocabulary manager
use crate::constants::*;
use qdrant_client::qdrant::{Vector, NamedVectors};
use std::collections::{HashMap};
use qdrant_client::qdrant::PointsSelector;
use crate::config; // Import config module

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
    ensure_collection_exists(&*client, collection_name, embedding_dim as u64).await?; 
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

    let mut points_batch = Vec::with_capacity(BATCH_SIZE);
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
                 if metadata.len() > MAX_FILE_SIZE_BYTES {
                     log::warn!(
                         "Skipping file larger than {} bytes: {}",
                         MAX_FILE_SIZE_BYTES,
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
    embedding_provider: Arc<dyn EmbeddingProvider + Send + Sync>, // Take Arc directly
) -> std::result::Result<(Vec<IntermediatePointData>, HashSet<String>), String> { 
    
    // --- File Size Check (as before) ---
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

    if chunks.is_empty() {
        log::trace!("No code chunks found in file: {}", full_path.display());
        return Ok((Vec::new(), HashSet::new())); // No error, just no points/tokens
    }

    // --- Dense Embeddings (as before) --- 
    let mut all_dense_embeddings = Vec::with_capacity(chunks.len());
    for chunk_batch in chunks.chunks(INTERNAL_EMBED_BATCH_SIZE) {
        let contents_batch: Vec<&str> = chunk_batch.iter().map(|c| c.content.as_str()).collect();
        match embedding_provider.embed_batch(&contents_batch) {
            Ok(embeddings_batch) => {
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
        let tokens = tokenizer::tokenize_code(&chunk.content, &tokenizer_config);
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
    embedding_handler: Arc<EmbeddingHandler>, // Changed to Arc<EmbeddingHandler>
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
    ensure_collection_exists(&*client, collection_name, embedding_dim as u64).await?; // Pass dereferenced client
    log::debug!("Core: Collection \"{}\" ensured.", collection_name);

    // --- 2. Process Files in Parallel (CPU Bound) ---
    if let Some(pb) = progress {
        pb.set_length(relative_paths.len() as u64);
        pb.set_position(0); // Reset position
        pb.set_message("Parsing & Embedding Files...");
    }

    let handler_arc_clone = embedding_handler.clone();

    // Call the parallel processing function to get intermediate data
    let (all_intermediate_data, all_token_sets, processing_errors) = process_repo_files_parallel(
        handler_arc_clone, // Pass Arc<EmbeddingHandler>
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

    // Save the updated vocabulary
    if let Err(e) = vocabulary_manager.save(&vocab_path) {
        log::error!("Failed to save updated vocabulary to {}: {}", vocab_path.display(), e);
        // Decide if this is a fatal error for indexing?
    } else {
        log::info!("Updated vocabulary saved to {}", vocab_path.display());
    }
    // --- End Vocabulary ---

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

/// Parallel processing function for repository files.
/// Returns intermediate data for point construction and sets of unique tokens per file.
fn process_repo_files_parallel(
    embedding_handler: Arc<EmbeddingHandler>,
    repo_root: &PathBuf,
    relative_paths: &[PathBuf],
    branch_name: &str,
    commit_hash: &str,
    progress: Option<&ProgressBar>,
) -> (Vec<IntermediatePointData>, Vec<HashSet<String>>, Vec<String>) { // Return (IntermediateData, TokensPerFile, Errors)
    let total_files = relative_paths.len();
    let files_processed_counter = Arc::new(AtomicUsize::new(0));

    // Get the embedding provider ONCE before the parallel loop
    let embedding_provider = match embedding_handler.get_onnx_provider() {
        Ok(p) => p,
        Err(e) => {
            log::error!("Failed to get embedding provider for parallel processing: {}", e);
            return (Vec::new(), Vec::new(), vec![format!("Failed to initialize embedding provider: {}", e)]);
        }
    };

    let results: Vec<std::result::Result<(Vec<IntermediatePointData>, HashSet<String>), String>> = relative_paths
        .par_iter()
        .map(|relative_path| {
            let full_path = repo_root.join(relative_path);
            let relative_path_str = relative_path.to_string_lossy();
            log::trace!("Parallel processing: {}", full_path.display());

            // Call the refactored function
            let result = process_single_file_for_indexing(
                &full_path,
                &relative_path_str,
                branch_name,
                commit_hash,
                embedding_provider.clone(), // Pass clone of the provider Arc
            );

            // Update progress bar regardless of outcome for this file
            let count = files_processed_counter.fetch_add(1, Ordering::Relaxed);
            if let Some(pb) = progress {
                pb.set_position((count + 1) as u64);
            }
            result // Forward the result (Ok or Err)
        })
        .collect();

    // Separate successful results (intermediate data, tokens) from errors
    let mut all_intermediate_data = Vec::new();
    let mut all_token_sets = Vec::new();
    let mut processing_errors = Vec::new();

    for result in results {
        match result {
            Ok((intermediate_list, tokens)) => {
                all_intermediate_data.extend(intermediate_list);
                all_token_sets.push(tokens);
            }
            Err(e) => {
                processing_errors.push(e);
            }
        }
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
/// TODO: Move payload index creation logic here from src/cli/commands/mod.rs
pub async fn ensure_collection_exists<
    C: QdrantClientTrait // Make generic
>(
    client: &C, // Use generic client trait ref
    collection_name: &str,
    embedding_dimension: u64,
) -> Result<()> {
    if !client.collection_exists(collection_name.to_string()).await? {
        log::info!(
            "Collection \"{}\" not found. Creating it with dimension {}.",
            collection_name,
            embedding_dimension
        );
        // Call the trait method directly with name and dimension
        client.create_collection(collection_name, embedding_dimension).await?;

        log::info!("Collection \"{}\" created successfully.", collection_name);

        // TODO: Add payload index creation here after moving ensure_payload_index
        // ensure_payload_index(client, collection_name, FIELD_LANGUAGE).await?;
        // ensure_payload_index(client, collection_name, FIELD_ELEMENT_TYPE).await?;
        // ensure_payload_index(client, collection_name, FIELD_FILE_PATH).await?; // Ensure file path is indexed

    } else {
        log::debug!("Collection \"{}\" already exists.", collection_name);
        // Optionally: Validate existing collection's dimension?
    }
    Ok(())
}

// Helper functions for filtering files (moved from src/vectordb/indexing.rs)
pub fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| s.starts_with('.'))
         .unwrap_or(false)
}

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
