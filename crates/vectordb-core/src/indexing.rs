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

/// Indexes files from specified paths into a Qdrant collection.
///
/// # Arguments
/// * `paths` - Vector of paths (files or directories) to index.
/// * `file_extensions` - Optional set of lowercase file extensions (without '.') to include.
/// * `collection_name` - The Qdrant collection name.
/// * `client` - An Arc-wrapped Qdrant client instance.
/// * `embedding_handler` - Reference to the initialized EmbeddingHandler.
/// * `progress` - Optional progress bar for reporting.
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
) -> Result<(usize, usize)> { // Updated return type
    log::info!(
        "Core: Starting index process for {} paths into collection \"{}\"",
        paths.len(),
        collection_name
    );

    // --- 1. Ensure Collection Exists ---
    let embedding_dim = embedding_handler.dimension()?;
    ensure_collection_exists(&*client, collection_name, embedding_dim as u64).await?; // Pass dereferenced client
    log::debug!("Core: Collection \"{}\" ensured.", collection_name);

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

    // --- 3. Process Files ---
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
    let model = match embedding_handler.create_embedding_model() {
        Ok(m) => m,
        Err(e) => {
            log::error!("Failed to create embedding model for sequential indexing: {}", e);
            return Err(e.into());
        }
    };

    for (file_idx, file_path) in files_to_process.into_iter().enumerate() {
        log::debug!("Core: Processing file {}/{}: {}", file_idx + 1, total_files, file_path.display());

        let absolute_path_str = file_path.to_string_lossy().to_string();

        // --- File Size Check (copied from index_repo_files) ---
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

        let chunks = match syntax::get_chunks(&file_path) {
            Ok(chunks) => chunks,
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

        // Generate embeddings sequentially using the single model instance
        match model.embed_batch(&chunk_contents) {
            Ok(embeddings) => {
                let file_extension = file_path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_string();

                for (i, chunk) in chunks.iter().enumerate() {
                    let mut payload = Payload::new();
                    payload.insert(FIELD_FILE_PATH, absolute_path_str.clone());
                    payload.insert(FIELD_START_LINE, chunk.start_line as i64);
                    payload.insert(FIELD_END_LINE, chunk.end_line as i64);
                    payload.insert(FIELD_LANGUAGE, chunk.language.clone());
                    payload.insert(FIELD_FILE_EXTENSION, file_extension.clone());
                    payload.insert(FIELD_ELEMENT_TYPE, chunk.element_type.to_string());
                    payload.insert(FIELD_CHUNK_CONTENT, chunk.content.clone());

                    let point = PointStruct::new(
                        Uuid::new_v4().to_string(), // Generate unique ID for each chunk
                        embeddings[i].clone(),
                        payload,
                    );
                    points_batch.push(point);

                    if points_batch.len() >= BATCH_SIZE {
                        let batch_to_upsert = std::mem::replace(&mut points_batch, Vec::with_capacity(BATCH_SIZE));
                        let current_batch_size = batch_to_upsert.len();
                        // Use tokio::spawn for upsert concurrency even in sequential file processing
                        let client_clone = client.clone();
                        let collection_name_clone = collection_name.to_string();
                        tokio::spawn(async move {
                            if let Err(e) = upsert_batch(client_clone, &collection_name_clone, batch_to_upsert).await {
                                log::error!("Background batch upsert failed: {}", e);
                                // Decide how to handle failure (e.g., metrics, retry queue)
                            }
                        });
                        points_processed_count += current_batch_size;
                    }
                }
                files_processed_count += 1;
            }
            Err(e) => {
                log::error!("Failed to generate embeddings for {}: {}", file_path.display(), e);
                // Skip file if embedding fails
            }
        }

        if let Some(pb) = progress {
            pb.inc(1);
            pb.set_message(format!("Processed {} files ", files_processed_count));
        }
    }

    // Upsert any remaining points in the last batch
    if !points_batch.is_empty() {
        let final_batch_size = points_batch.len();
         if let Err(e) = upsert_batch(client.clone(), collection_name, points_batch).await {
            log::error!("Failed to upsert final batch: {}", e);
        } else {
            points_processed_count += final_batch_size;
        }
    }

    if let Some(pb) = progress {
        pb.finish_with_message(format!("Indexed {} chunks from {} files ", points_processed_count, files_processed_count));
    }
    log::info!(
        "Core: Indexing process finished. Indexed {} points from {} files.",
        points_processed_count,
        files_processed_count
    );

    Ok((files_processed_count, points_processed_count))
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
    _config: &AppConfig, // Keep config for future use? Marked unused for now.
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

    // Removed prewarm call

    // --- 2. Process Files in Parallel (CPU Bound) ---
    if let Some(pb) = progress {
        pb.set_length(relative_paths.len() as u64);
        pb.set_position(0); // Reset position
        pb.set_message("Parsing & Embedding Files...");
    }

    // Clone Arc for passing into parallel processing
    let handler_arc_clone = embedding_handler.clone();

    // Call the parallel processing function
    let (all_points, processing_errors) = process_repo_files_parallel(
        handler_arc_clone, // Pass Arc<EmbeddingHandler>
        repo_root,
        relative_paths,
        branch_name,
        commit_hash,
        progress,
    );

    let total_points_generated = all_points.len();
    let files_processed_successfully = relative_paths.len() - processing_errors.len();
    log::info!(
        "Core: Parallel processing complete. Generated {} points from {} files ({} errors encountered).",
        total_points_generated,
        files_processed_successfully,
        processing_errors.len()
    );
    if let Some(pb) = progress {
        pb.set_position(relative_paths.len() as u64); // Ensure progress bar shows completion of phase 1
        pb.set_message("Uploading Points to Qdrant...");
    }

    // --- 3. Upload Points to Qdrant (Network Bound, Concurrent) ---
    let semaphore = Arc::new(Semaphore::new(max_concurrent_upserts));
    log::info!("Using max_concurrent_upserts: {}", max_concurrent_upserts);
    let mut upsert_tasks = Vec::new();
    let mut total_points_attempted_upsert = 0;

    // Iterate over the collected points and create concurrent upload tasks
    for points_batch in all_points.chunks(BATCH_SIZE) {
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
         pb.set_length(upsert_tasks.len() as u64); // Update progress bar for upload tasks
         pb.set_position(0);
         pb.set_message(format!("Uploading {} batches...", upsert_tasks.len()));
    }

    let mut upsert_errors: Vec<VectorDBError> = Vec::new(); // Explicitly type the error vector
    let total_tasks = upsert_tasks.len();
    for (i, task) in upsert_tasks.into_iter().enumerate() {
         match task.await {
             Ok(Ok(())) => { // Task completed successfully, batch upsert was Ok
                 // Batch succeeded
             },
             Ok(Err(e)) => { // Task completed successfully, but batch upsert failed
                 log::error!("Batch upsert task failed: {}", e);
                 upsert_errors.push(e.into()); // Convert anyhow::Error to VectorDBError
             },
             Err(join_err) => { // Tokio task failed to execute (e.g., panic)
                 log::error!("Tokio task join error during upsert: {}", join_err);
                 upsert_errors.push(VectorDBError::Other(format!("Tokio task join error: {}", join_err)));
             },
         }
         if let Some(pb) = progress {
             pb.inc(1);
             // pb.set_message(format!("Uploaded {}/{} batches", i + 1, total_tasks));
         }
    }

    // Log processing errors (from the parallel CPU phase)
    if !processing_errors.is_empty() {
        log::warn!("Encountered {} errors during file processing:", processing_errors.len());
        for e in processing_errors.iter().take(10) { // Log first 10 errors
            log::warn!("  - {}", e);
        }
        if processing_errors.len() > 10 {
            log::warn!("  ... and {} more processing errors.", processing_errors.len() - 10);
        }
    }

    // Handle upsert errors (from the network phase)
    if !upsert_errors.is_empty() {
        log::error!("Encountered {} errors during batch upserts.", upsert_errors.len());
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

// --- Start: Added process_repo_files_parallel definition ---
/// Parallel processing function for repository files.
fn process_repo_files_parallel(
    embedding_handler: Arc<EmbeddingHandler>,
    repo_root: &PathBuf,
    relative_paths: &[PathBuf],
    branch_name: &str,
    commit_hash: &str,
    progress: Option<&ProgressBar>,
) -> (Vec<PointStruct>, Vec<String>) { // Return (all points, all errors)
    let total_files = relative_paths.len();
    let files_processed_counter = Arc::new(AtomicUsize::new(0));

    let results: Vec<std::result::Result<Vec<PointStruct>, String>> = relative_paths // Use full path for Result
        .par_iter()
        .map(|relative_path| {
            let full_path = repo_root.join(relative_path);
            log::trace!("Parallel processing: {}", full_path.display());

            // --- File Size Check ---
            let metadata = match std::fs::metadata(&full_path) {
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

            let chunks = match syntax::get_chunks(&full_path) {
                Ok(c) => c,
                Err(e) => {
                    return Err(format!("Skipping file due to parsing error: {} - {}", full_path.display(), e));
                }
            };

            if chunks.is_empty() {
                log::trace!("No code chunks found in file: {}", full_path.display());
                return Ok(Vec::new()); // No error, just no points
            }

            // Get the ONNX provider directly, avoid handler's mutex
            let provider = match embedding_handler.get_onnx_provider() {
                Ok(p) => p,
                Err(e) => return Err(format!("Failed to get ONNX provider for {}: {}", full_path.display(), e)),
            };

            let mut all_embeddings = Vec::with_capacity(chunks.len());
            for chunk_batch in chunks.chunks(INTERNAL_EMBED_BATCH_SIZE) {
                let contents_batch: Vec<&str> = chunk_batch.iter().map(|c| c.content.as_str()).collect();
                // Call the provider's embed_batch directly
                match provider.embed_batch(&contents_batch) {
                    Ok(embeddings_batch) => {
                        if embeddings_batch.len() != contents_batch.len() {
                            log::warn!("Embedding batch size mismatch for {}: Expected {}, got {}. Skipping batch.", full_path.display(), contents_batch.len(), embeddings_batch.len());
                            return Err(format!("Embedding batch size mismatch for {}", full_path.display()));
                        }
                        all_embeddings.extend(embeddings_batch);
                    }
                    Err(e) => {
                        return Err(format!("Failed to generate embeddings batch for {}: {}", full_path.display(), e));
                    }
                }
            }

            if all_embeddings.len() != chunks.len() {
                log::error!("Final embedding count ({}) does not match chunk count ({}) for file: {}. Skipping file.", all_embeddings.len(), chunks.len(), full_path.display());
                return Err(format!("Embedding count mismatch after batching for {}", full_path.display()));
            }

            let file_path_str = relative_path.to_string_lossy().to_string();
            let file_extension = relative_path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_string();
            let mut points = Vec::with_capacity(chunks.len());

            for (i, chunk) in chunks.iter().enumerate() {
                let mut payload = Payload::new();
                payload.insert(FIELD_FILE_PATH, file_path_str.clone());
                payload.insert(FIELD_START_LINE, chunk.start_line as i64);
                payload.insert(FIELD_END_LINE, chunk.end_line as i64);
                payload.insert(FIELD_LANGUAGE, chunk.language.clone());
                payload.insert(FIELD_FILE_EXTENSION, file_extension.clone());
                payload.insert(FIELD_ELEMENT_TYPE, chunk.element_type.to_string());
                payload.insert(FIELD_CHUNK_CONTENT, chunk.content.clone());
                payload.insert(FIELD_BRANCH, branch_name.to_string());
                payload.insert(FIELD_COMMIT_HASH, commit_hash.to_string());

                let point = PointStruct::new(
                    Uuid::new_v4().to_string(),
                    all_embeddings[i].clone(),
                    payload,
                );
                points.push(point);
            }
            let points_result = Ok(points);

            // Update progress after processing file (success or error)
            let processed_count = files_processed_counter.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some(pb) = progress {
                pb.set_position(processed_count as u64);
                // pb.set_message(format!("Processed {}/{} files", processed_count, total_files));
            }

            points_result // Return the Result<Vec<PointStruct>, String>
        })
        .collect();

    // --- Separate successful points from errors ---
    let mut all_points = Vec::new();
    let mut processing_errors = Vec::new();
    for result in results {
        match result {
            Ok(points) => all_points.extend(points),
            Err(err_msg) => {
                log::warn!("{}", err_msg); // Log the error
                processing_errors.push(err_msg);
            }
        }
    }

    (all_points, processing_errors)
}
// --- End: Added process_repo_files_parallel definition ---
