use crate::{
    error::{Result, SagittaError},
    qdrant_client_trait::QdrantClientTrait,
    config::{self, AppConfig},
    constants::*,
    vocabulary::VocabularyManager,
    sync_progress::{SyncProgressReporter, SyncProgress, SyncStage, NoOpProgressReporter},
    qdrant_ops::{self, upsert_batch, delete_collection_by_name},
    app_config_to_embedding_config,
    syntax, // Import our syntax parsing module
};
use qdrant_client::{
    qdrant::{PointStruct, Vector, NamedVectors},
    Payload,
};
use std::{
    collections::{HashSet, HashMap},
    path::PathBuf,
    sync::{Arc, atomic::{AtomicUsize, Ordering}},
    time::Instant,
};
use walkdir::WalkDir;
use uuid::Uuid;
use tokio::sync::Semaphore;
use anyhow::anyhow;
use async_trait::async_trait;

// Import from sagitta-embed for the new decoupled processing architecture
use sagitta_embed::{
    DefaultFileProcessor, EmbeddingPool, FileProcessor, EmbeddingProcessor,
    ProcessingConfig, ProcessedChunk, EmbeddedChunk, ChunkMetadata,
};
use sagitta_embed::processor::{
    ProgressReporter as EmbedProgressReporter, ProcessingProgress, ProcessingStage
};

/// Bridge between sagitta-embed progress reporting and our sync progress reporting
/// This version handles file processing stages
struct FileProcessingProgressBridge {
    reporter: Arc<dyn SyncProgressReporter>,
    total_files: usize,
}

#[async_trait::async_trait]
impl EmbedProgressReporter for FileProcessingProgressBridge {
    async fn report(&self, progress: ProcessingProgress) {
        let stage = match progress.stage {
            ProcessingStage::Starting => SyncStage::CollectFiles {
                total_files: progress.total_files,
                message: "Starting file processing...".to_string(),
            },
            ProcessingStage::ProcessingFiles => SyncStage::IndexFile {
                current_file: progress.current_file,
                total_files: progress.total_files,
                current_file_num: progress.files_completed,
                files_per_second: progress.files_per_second,
                message: progress.message,
            },
            ProcessingStage::GeneratingEmbeddings => {
                // File processing is done, don't report embedding stage here
                return;
            },
            ProcessingStage::Completed => SyncStage::IndexFile {
                current_file: None,
                total_files: progress.total_files,
                current_file_num: progress.files_completed,
                files_per_second: progress.files_per_second,
                message: Some("File processing completed".to_string()),
            },
            ProcessingStage::Error { message } => SyncStage::Error { message },
        };

        self.reporter.report(SyncProgress { stage }).await;
    }
}

/// Bridge for embedding generation progress
/// This treats chunks as "files" for display purposes
struct EmbeddingProgressBridge {
    reporter: Arc<dyn SyncProgressReporter>,
    total_chunks: usize,
}

#[async_trait::async_trait]
impl EmbedProgressReporter for EmbeddingProgressBridge {
    async fn report(&self, progress: ProcessingProgress) {
        let stage = match progress.stage {
            ProcessingStage::Starting => SyncStage::IndexFile {
                current_file: None,
                total_files: progress.total_files,
                current_file_num: 0,
                files_per_second: None,
                message: Some("Starting embedding generation...".to_string()),
            },
            ProcessingStage::GeneratingEmbeddings => SyncStage::IndexFile {
                current_file: None,
                total_files: progress.total_files,
                current_file_num: progress.files_completed,
                files_per_second: progress.files_per_second,
                message: Some(format!("Generating embeddings ({}/{} chunks)", progress.files_completed, progress.total_files)),
            },
            ProcessingStage::Completed => SyncStage::IndexFile {
                current_file: None,
                total_files: progress.total_files,
                current_file_num: progress.files_completed,
                files_per_second: progress.files_per_second,
                message: Some("Embedding generation completed".to_string()),
            },
            ProcessingStage::ProcessingFiles | ProcessingStage::Error { .. } => {
                // These stages are handled by the file processing bridge
                return;
            },
        };

        self.reporter.report(SyncProgress { stage }).await;
    }
}

/// Indexes files from specified paths into a Qdrant collection using the decoupled processing architecture.
///
/// # Arguments
/// * `paths` - Vector of paths (files or directories) to index.
/// * `file_extensions` - Optional set of lowercase file extensions (without '.') to include.
/// * `collection_name` - The Qdrant collection name.
/// * `client` - An Arc-wrapped Qdrant client instance.
/// * `embedding_pool` - Reference to the initialized EmbeddingPool.
/// * `progress_reporter` - Optional progress reporter for reporting.
/// * `config` - The application configuration.
///
/// # Returns
/// * `Result<(usize, usize)>` - (indexed files, indexed chunks/points)
pub async fn index_paths<
    C: QdrantClientTrait + Send + Sync + 'static
>(
    paths: &[PathBuf],
    file_extensions: Option<HashSet<String>>,
    collection_name: &str,
    client: Arc<C>,
    embedding_pool: &EmbeddingPool,
    progress_reporter: Option<Arc<dyn SyncProgressReporter>>,
    config: &AppConfig,
) -> Result<(usize, usize)> {
    let reporter = progress_reporter.as_ref().map(|r| r.clone()).unwrap_or_else(|| Arc::new(NoOpProgressReporter));
    
    log::info!(
        "Core: Starting index process for {} paths into collection \"{}\" using decoupled processing",
        paths.len(),
        collection_name
    );

    // --- 1. Ensure Collection Exists ---
    let embedding_dim = embedding_pool.dimension();
    ensure_collection_exists(client.clone(), collection_name, embedding_dim as u64).await?; 
    log::debug!("Core: Collection \"{}\" ensured.", collection_name);

    // --- 2. Gather Files ---
    let files_to_process = gather_files(paths, file_extensions)?;
    log::info!("Core: Found {} files to process.", files_to_process.len());

    if files_to_process.is_empty() {
        log::warn!("Core: No files found matching the criteria. Indexing complete.");
        reporter.report(SyncProgress {
            stage: SyncStage::Error {
                message: format!("No files found to index"),
            }
        }).await;
        return Ok((0, 0));
    }

    // --- 3. Set up decoupled processing ---
    let embedding_config = app_config_to_embedding_config(config);
    let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
    
    log::info!("Using decoupled processing: {} CPU cores for file processing, {} embedding sessions for GPU control",
        processing_config.file_processing_concurrency,
        processing_config.max_embedding_sessions);

    // Create syntax parser function that uses our existing syntax parsing infrastructure
    let syntax_parser_fn = |file_path: &std::path::Path| -> sagitta_embed::error::Result<Vec<sagitta_embed::processor::file_processor::ParsedChunk>> {
        let chunks = syntax::get_chunks(file_path)
            .map_err(|e| sagitta_embed::error::SagittaEmbedError::file_system(format!("Syntax parsing error: {}", e)))?;
        
        let parsed_chunks = chunks.into_iter().map(|chunk| {
            sagitta_embed::processor::file_processor::ParsedChunk {
                content: chunk.content,
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                language: chunk.language,
                element_type: chunk.element_type,
            }
        }).collect();
        
        Ok(parsed_chunks)
    };

    let file_processor = DefaultFileProcessor::new(processing_config.clone())
        .with_syntax_parser(syntax_parser_fn);

    // --- 4. Process Files (CPU-intensive, parallel) ---
    let progress_bridge = Arc::new(FileProcessingProgressBridge {
        reporter: reporter.clone(),
        total_files: files_to_process.len(),
    });

    let start_time = Instant::now();
    let processed_chunks = file_processor.process_files_with_progress(&files_to_process, progress_bridge).await?;
    
    if processed_chunks.is_empty() {
        log::warn!("Core: No chunks generated from processed files. Indexing complete.");
        reporter.report(SyncProgress {
            stage: SyncStage::Error {
                message: format!("No indexable content found in {} files", files_to_process.len()),
            }
        }).await;
        return Ok((files_to_process.len(), 0));
    }

    log::info!("Processed {} files into {} chunks", files_to_process.len(), processed_chunks.len());

    // --- 5. Generate Embeddings (GPU-intensive, controlled) ---
    let progress_bridge = Arc::new(EmbeddingProgressBridge {
        reporter: reporter.clone(),
        total_chunks: processed_chunks.len(),
    });

    let embedded_chunks = embedding_pool.process_chunks_with_progress(processed_chunks, progress_bridge).await?;
    
    log::info!("Generated {} embeddings", embedded_chunks.len());

    // --- 6. Build Vocabulary and Create Points ---
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

    let mut points_batch = Vec::with_capacity(config.performance.batch_size);
    let mut points_processed_count = 0;

    for embedded_chunk in embedded_chunks {
        let chunk = &embedded_chunk.chunk;
        
        // Generate sparse vector using vocabulary
        let mut term_frequencies: HashMap<u32, u32> = HashMap::new();
        // Simple tokenization for sparse vector (this could be improved)
        let words: Vec<&str> = chunk.content.split_whitespace().collect();
        for word in words {
            let clean_word = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            if !clean_word.is_empty() {
                let token_id = vocabulary_manager.add_token(&clean_word);
                *term_frequencies.entry(token_id).or_insert(0) += 1;
            }
        }
        
        let sparse_indices: Vec<u32> = term_frequencies.keys().copied().collect();
        let sparse_values: Vec<f32> = term_frequencies.values().map(|&count| count as f32).collect();

        // Create Payload
        let mut payload = Payload::new();
        payload.insert(FIELD_FILE_PATH, chunk.metadata.file_path.to_string_lossy().to_string());
        payload.insert(FIELD_START_LINE, chunk.metadata.start_line as i64);
        payload.insert(FIELD_END_LINE, chunk.metadata.end_line as i64);
        payload.insert(FIELD_LANGUAGE, chunk.metadata.language.clone());
        payload.insert(FIELD_FILE_EXTENSION, chunk.metadata.file_extension.clone());
        payload.insert(FIELD_ELEMENT_TYPE, chunk.metadata.element_type.clone());
        payload.insert(FIELD_CHUNK_CONTENT, chunk.content.clone());

        // Create NamedVectors for both dense and sparse
        let vectors = NamedVectors::default()
            .add_vector("dense", Vector::new_dense(embedded_chunk.embedding))
            .add_vector("sparse_tf", Vector::new_sparse(sparse_indices, sparse_values));

        // Create PointStruct with NamedVectors
        let point = PointStruct::new(
            Uuid::new_v4().to_string(),
            vectors,
            payload,
        );
        points_batch.push(point);
        points_processed_count += 1;

        // --- Upsert Batch Logic ---
        if points_batch.len() >= config.performance.batch_size {
            log::debug!("Upserting batch of {} points...", points_batch.len());
            if let Err(e) = upsert_batch(client.clone(), collection_name, points_batch).await {
                log::error!("Failed to upsert batch: {}", e);
                return Err(e.into());
            }
            points_batch = Vec::with_capacity(config.performance.batch_size);
        }
    }

    // --- Upsert Final Batch ---
    if !points_batch.is_empty() {
        log::debug!("Upserting final batch of {} points...", points_batch.len());
        if let Err(e) = upsert_batch(client.clone(), collection_name, points_batch).await {
            log::error!("Failed to upsert final batch: {}", e);
            return Err(e.into());
        }
    }

    // --- Save Vocabulary --- 
    if let Err(e) = vocabulary_manager.save(&vocab_path) {
        log::error!("Failed to save vocabulary to {}: {}", vocab_path.display(), e);
    } else {
        log::info!("Vocabulary saved to {}", vocab_path.display());
    }

    reporter.report(SyncProgress {
        stage: SyncStage::Completed {
            message: format!("Indexed {} files ({} points) using decoupled processing", files_to_process.len(), points_processed_count),
        }
    }).await;

    log::info!(
        "Core: Finished indexing {} files, {} points processed into collection \"{}\" using decoupled processing",
        files_to_process.len(),
        points_processed_count,
        collection_name
    );

    Ok((files_to_process.len(), points_processed_count))
}

/// Indexes specific files within a repository context using the decoupled processing architecture.
///
/// # Arguments
/// * `config` - The application configuration.
/// * `repo_root` - The absolute path to the repository root.
/// * `relative_paths` - Slice of relative paths within the repo to index.
/// * `collection_name` - The Qdrant collection name.
/// * `branch_name` - The current branch name.
/// * `commit_hash` - The current commit hash.
/// * `client` - An Arc-wrapped Qdrant client instance.
/// * `embedding_pool` - Reference to the initialized EmbeddingPool.
/// * `progress_reporter` - Optional progress reporter for reporting.
/// * `max_concurrent_upserts` - Maximum number of concurrent Qdrant upsert operations.
///
/// # Returns
/// * `Result<usize>` - Total number of points successfully processed and attempted to upsert.
pub async fn index_repo_files<
    C: QdrantClientTrait + Send + Sync + 'static
>(
    config: &AppConfig,
    repo_root: &PathBuf,
    relative_paths: &[PathBuf],
    collection_name: &str,
    branch_name: &str,
    commit_hash: &str,
    client: Arc<C>,
    embedding_pool: Arc<EmbeddingPool>,
    progress_reporter: Option<Arc<dyn SyncProgressReporter>>, 
    max_concurrent_upserts: usize,
) -> Result<usize> {
    let reporter = progress_reporter.as_ref().map(|r| r.clone()).unwrap_or_else(|| Arc::new(NoOpProgressReporter));
    
    log::info!(
        "Core: Starting repo index process for {} files into collection \"{}\" using decoupled processing (branch: {}, commit: {})",
        relative_paths.len(),
        collection_name,
        branch_name,
        &commit_hash[..8]
    );

    if relative_paths.is_empty() {
        log::warn!("Core: No relative paths provided for repo indexing.");
        reporter.report(SyncProgress {
            stage: SyncStage::Error {
                message: format!("No files provided to index"),
            }
        }).await;
        return Ok(0);
    }

    // --- 1. Ensure Collection Exists ---
    let embedding_dim = embedding_pool.dimension();
    ensure_collection_exists(client.clone(), collection_name, embedding_dim as u64).await?;
    log::debug!("Core: Collection \"{}\" ensured.", collection_name);

    // --- 2. Set up decoupled processing ---
    let embedding_config = app_config_to_embedding_config(config);
    let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
    
    log::info!("Using decoupled processing: {} CPU cores for file processing, {} embedding sessions for GPU control",
        processing_config.file_processing_concurrency,
        processing_config.max_embedding_sessions);

    // Create syntax parser function that uses our existing syntax parsing infrastructure
    let syntax_parser_fn = |file_path: &std::path::Path| -> sagitta_embed::error::Result<Vec<sagitta_embed::processor::file_processor::ParsedChunk>> {
        let chunks = syntax::get_chunks(file_path)
            .map_err(|e| sagitta_embed::error::SagittaEmbedError::file_system(format!("Syntax parsing error: {}", e)))?;
        
        let parsed_chunks = chunks.into_iter().map(|chunk| {
            sagitta_embed::processor::file_processor::ParsedChunk {
                content: chunk.content,
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                language: chunk.language,
                element_type: chunk.element_type,
            }
        }).collect();
        
        Ok(parsed_chunks)
    };

    let file_processor = DefaultFileProcessor::new(processing_config.clone())
        .with_syntax_parser(syntax_parser_fn);
    let embedding_pool_ref = &*embedding_pool;

    // Convert relative paths to absolute paths
    let absolute_paths: Vec<PathBuf> = relative_paths.iter()
        .map(|relative_path| repo_root.join(relative_path))
        .collect();

    // --- 4. Process Files (CPU-intensive, parallel) ---
    let progress_bridge = Arc::new(FileProcessingProgressBridge {
        reporter: reporter.clone(),
        total_files: relative_paths.len(),
    });

    let processed_chunks = file_processor.process_files_with_progress(&absolute_paths, progress_bridge).await?;
    
    if processed_chunks.is_empty() {
        log::warn!("Core: No chunks generated from processed repo files.");
        return Ok(0);
    }

    log::info!("Processed {} repo files into {} chunks", relative_paths.len(), processed_chunks.len());

    // --- 5. Generate Embeddings (GPU-intensive, controlled) ---
    let progress_bridge = Arc::new(EmbeddingProgressBridge {
        reporter: reporter.clone(),
        total_chunks: processed_chunks.len(),
    });

    let embedded_chunks = embedding_pool_ref.process_chunks_with_progress(processed_chunks, progress_bridge).await?;
    
    log::info!("Generated {} embeddings for repo files", embedded_chunks.len());

    // --- 6. Build Vocabulary and Create Points ---
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
    let mut final_points: Vec<PointStruct> = Vec::with_capacity(embedded_chunks.len());

    for embedded_chunk in embedded_chunks {
        let chunk = &embedded_chunk.chunk;
        
        // Convert absolute path back to relative path for storage
        let relative_path = chunk.metadata.file_path.strip_prefix(repo_root)
            .unwrap_or(&chunk.metadata.file_path)
            .to_string_lossy()
            .to_string();

        // Generate sparse vector using vocabulary
        let mut term_frequencies: HashMap<u32, u32> = HashMap::new();
        // Simple tokenization for sparse vector
        let words: Vec<&str> = chunk.content.split_whitespace().collect();
        for word in words {
            let clean_word = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            if !clean_word.is_empty() {
                let token_id = vocabulary_manager.add_token(&clean_word);
                *term_frequencies.entry(token_id).or_insert(0) += 1;
            }
        }
        
        let sparse_indices: Vec<u32> = term_frequencies.keys().copied().collect();
        let sparse_values: Vec<f32> = term_frequencies.values().map(|&count| count as f32).collect();

        // Create Payload with repository context
        let mut payload = Payload::new();
        payload.insert(FIELD_FILE_PATH, relative_path);
        payload.insert(FIELD_START_LINE, chunk.metadata.start_line as i64);
        payload.insert(FIELD_END_LINE, chunk.metadata.end_line as i64);
        payload.insert(FIELD_LANGUAGE, chunk.metadata.language.clone());
        payload.insert(FIELD_FILE_EXTENSION, chunk.metadata.file_extension.clone());
        payload.insert(FIELD_ELEMENT_TYPE, chunk.metadata.element_type.clone());
        payload.insert(FIELD_CHUNK_CONTENT, chunk.content.clone());
        payload.insert(FIELD_BRANCH, branch_name.to_string());
        payload.insert(FIELD_COMMIT_HASH, commit_hash.to_string());

        // Create NamedVectors for both dense and sparse
        let vectors = NamedVectors::default()
            .add_vector("dense", Vector::new_dense(embedded_chunk.embedding))
            .add_vector("sparse_tf", Vector::new_sparse(sparse_indices, sparse_values));

        let point = PointStruct::new(
            Uuid::new_v4().to_string(),
            vectors,
            payload,
        );
        final_points.push(point);
    }

    let final_vocab_size = vocabulary_manager.len();
    log::info!("Vocabulary updated. Size: {} -> {}", initial_vocab_size, final_vocab_size);

    // Save vocabulary
    vocabulary_manager.save(&vocab_path).map_err(|e| {
        log::error!("FATAL: Failed to save updated vocabulary to {}: {}", vocab_path.display(), e);
        SagittaError::Other(format!("Failed to save vocabulary: {}", e)) 
    })?;

    log::info!("Updated vocabulary saved to {}", vocab_path.display());

    // --- 6. Upload Points to Qdrant (Network Bound, Concurrent) ---
    let semaphore = Arc::new(Semaphore::new(max_concurrent_upserts));
    log::info!("Using max_concurrent_upserts: {}", max_concurrent_upserts);
    let mut upsert_tasks = Vec::new();
    let total_points_attempted_upsert = final_points.len();

    for points_batch in final_points.chunks(config.performance.batch_size) {
        if points_batch.is_empty() {
            continue;
        }
        let batch_to_upsert = points_batch.to_vec();
        let client_clone = client.clone();
        let collection_name_clone = collection_name.to_string();
        let semaphore_clone = semaphore.clone();

        let task = tokio::spawn(async move {
            let permit = semaphore_clone.acquire_owned().await.expect("Semaphore acquisition failed");
            let result = upsert_batch(client_clone, &collection_name_clone, batch_to_upsert).await;
            drop(permit);
            result
        });
        upsert_tasks.push(task);
    }

    let mut upsert_errors: Vec<SagittaError> = Vec::new(); 
    for task in upsert_tasks.into_iter() {
         match task.await {
             Ok(Ok(())) => { 
                 // Batch succeeded
             },
             Ok(Err(e)) => { 
                 let error_msg = format!("Qdrant batch upsert failed: {}", e);
                 log::error!("Batch upsert task failed: {}", e);
                 upsert_errors.push(e.into()); 
                 reporter.report(SyncProgress { stage: SyncStage::Error { message: error_msg }}).await;
             },
             Err(join_err) => { 
                 log::error!("Tokio task join error during upsert: {}", join_err);
                 upsert_errors.push(SagittaError::Other(format!("Tokio task join error: {}", join_err)));
                 reporter.report(SyncProgress { stage: SyncStage::Error { message: format!("Tokio task join error during upsert: {}", join_err) }}).await;
             },
         }
    }

    if !upsert_errors.is_empty() {
        log::error!("Encountered {} errors during Qdrant upsert:", upsert_errors.len());
        for e in upsert_errors.iter().take(10) {
            log::error!("  - {}", e);
        }
        if upsert_errors.len() > 10 {
            log::error!("  ... and {} more upsert errors.", upsert_errors.len() - 10);
        }
        return Err(upsert_errors.remove(0).into());
    }

    log::info!(
        "Core: Repo indexing finished using decoupled processing. Processed {} files, uploaded {} points.",
        relative_paths.len(),
        total_points_attempted_upsert
    );

    Ok(total_points_attempted_upsert)
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
pub async fn ensure_collection_exists<
    C: QdrantClientTrait + Send + Sync + 'static
>(
    client: Arc<C>,
    collection_name: &str,
    embedding_dimension: u64,
) -> Result<()> {
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
            Err(SagittaError::QdrantOperationError(format!("Qdrant reported false for collection creation/recreation of '{}'", collection_name)))
        }
        Err(e) => {
            log::error!("Failed to create/recreate collection '{}': {:?}", collection_name, e);
            Err(e)
        }
    }
}

// Helper functions for filtering files
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
    use walkdir::DirEntry;
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