use anyhow::{Context, Result};
use clap::Args;
use indicatif::{ProgressBar, ProgressStyle};
use qdrant_client::{
    Qdrant,
    Payload,
    qdrant::{
        CreateCollectionBuilder, Distance, FieldType, PointStruct,
        VectorParamsBuilder, VectorsConfig,
        CreateCollection,
    },
};
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::vectordb::{
    embedding,
    embedding_logic::EmbeddingHandler,
    search::chunking::chunk_by_lines,
};
// use crate::chunking::chunk_text;
// Temp: Assuming chunking is available later
// fn chunk_text(_content: &str, _max_len: usize, _overlap: usize) -> Vec<crate::vectordb::search::chunking::ChunkInfo> { vec![] } // Placeholder - Renamed Chunk to ChunkInfo
use super::commands::{CliArgs, ensure_payload_index, upsert_batch, CODE_SEARCH_COLLECTION, FIELD_FILE_PATH, FIELD_DIR_PATH, FIELD_START_LINE, FIELD_END_LINE, FIELD_FILE_EXTENSION};

// Import necessary items for path resolution
use crate::config::AppConfig;

const BATCH_SIZE: usize = 128;

#[derive(Args, Debug)]
pub struct IndexArgs {
    /// Directories to index
    #[arg(required = true)]
    pub dirs: Vec<PathBuf>,

    /// Optional file extensions to include (e.g., ".rs", ".py")
    #[arg(short = 't', long = "type")]
    pub file_types: Option<Vec<String>>,

    /// Max length for text chunking
    #[arg(long, default_value_t = 1536)]
    pub chunk_max_length: usize,

    /// Overlap size for text chunking
    #[arg(long, default_value_t = 256)]
    pub chunk_overlap: usize,
}

/// Handles the `index` command, processing directories and upserting data into Qdrant.
pub async fn handle_index(
    cmd_args: &IndexArgs,
    cli_args: &CliArgs, 
    config: &AppConfig
) -> Result<()> {
    log::info!("Starting indexing process...");
    log::debug!("IndexArgs: {:?}", cmd_args);
    log::debug!("CliArgs: {:?}", cli_args);
    log::debug!("Config: {:?}", config);
    log::info!("Using Qdrant URL: {}", config.qdrant_url);

    // --- Resolve ONNX Paths (Arg > Env > Config) --- 
    let model_env_var = std::env::var("VECTORDB_ONNX_MODEL").ok();
    let tokenizer_env_var = std::env::var("VECTORDB_ONNX_TOKENIZER_DIR").ok();

    // Check for conflicts (Arg vs Env Var)
    if cli_args.onnx_model_path_arg.is_some() && model_env_var.is_some() {
        return Err(anyhow::anyhow!(
            "Cannot provide ONNX model path via both --onnx-model argument and VECTORDB_ONNX_MODEL environment variable."
        ));
    }
    if cli_args.onnx_tokenizer_dir_arg.is_some() && tokenizer_env_var.is_some() {
        return Err(anyhow::anyhow!(
            "Cannot provide ONNX tokenizer dir via both --onnx-tokenizer-dir argument and VECTORDB_ONNX_TOKENIZER_DIR environment variable."
        ));
    }

    // Model Path Resolution
    let onnx_model_path_str = cli_args.onnx_model_path_arg.as_ref()
        .or(model_env_var.as_ref())
        .or(config.onnx_model_path.as_ref())
        .ok_or_else(|| anyhow::anyhow!("ONNX model path must be provided via --onnx-model, VECTORDB_ONNX_MODEL, or config"))?;

    // Tokenizer Path Resolution
    let onnx_tokenizer_dir_str = cli_args.onnx_tokenizer_dir_arg.as_ref()
        .or(tokenizer_env_var.as_ref())
        .or(config.onnx_tokenizer_path.as_ref())
        .ok_or_else(|| anyhow::anyhow!("ONNX tokenizer path must be provided via --onnx-tokenizer-dir, VECTORDB_ONNX_TOKENIZER_DIR, or config"))?;

    let onnx_model_path = PathBuf::from(onnx_model_path_str);
    let onnx_tokenizer_path = PathBuf::from(onnx_tokenizer_dir_str);

    // Validate paths exist (crucial check)
    if !onnx_model_path.exists() {
        return Err(anyhow::anyhow!("Resolved ONNX model path does not exist: {}", onnx_model_path.display()));
    }
    if !onnx_tokenizer_path.is_dir() {
         return Err(anyhow::anyhow!("Resolved ONNX tokenizer path is not a directory: {}", onnx_tokenizer_path.display()));
    }
    let tokenizer_file = onnx_tokenizer_path.join("tokenizer.json");
    if !tokenizer_file.exists() {
         return Err(anyhow::anyhow!("tokenizer.json not found in the ONNX tokenizer directory: {}", onnx_tokenizer_path.display()));
    }
    log::info!("Using resolved ONNX model: {}", onnx_model_path.display());
    log::info!("Using resolved ONNX tokenizer directory: {}", onnx_tokenizer_path.display());

    // --- 1. Initialize Embedding Handler (using resolved paths) ---
    log::info!("Initializing embedding handler...");
    let embedding_handler = Arc::new(
        EmbeddingHandler::new(
            embedding::EmbeddingModelType::Onnx,
            Some(onnx_model_path), // Use resolved path
            Some(onnx_tokenizer_path), // Use resolved path
        )
        .context("Failed to initialize embedding handler")?,
    );
    let embedding_dim = embedding_handler.dimension().context("Failed to get embedding dimension")?;
    log::info!("Actual embedding dimension: {}", embedding_dim);

    // --- 2. Initialize Qdrant Client (using passed URL) ---
    log::info!("Connecting to Qdrant at {}", config.qdrant_url);
    let client = Qdrant::from_url(&config.qdrant_url).build()?;
    log::info!("Qdrant client connected.");

    // --- 3. Check/Create Collection ---
    let collection_exists = client
        .collection_exists(CODE_SEARCH_COLLECTION)
        .await
        .context("Failed to check collection existence")?;

    if !collection_exists {
        log::info!(
            "Collection '{}' does not exist. Creating...",
            CODE_SEARCH_COLLECTION
        );
        let vectors_config = VectorsConfig {
            config: Some(qdrant_client::qdrant::vectors_config::Config::Params(
                VectorParamsBuilder::new(embedding_dim as u64, Distance::Cosine).build()
            ))
        };

        let create_request: CreateCollection = CreateCollectionBuilder::new(CODE_SEARCH_COLLECTION)
            .vectors_config(vectors_config)
            .build();

        client.create_collection(create_request).await?;
        log::info!("Collection '{}' created successfully.", CODE_SEARCH_COLLECTION);

        // --- 4. Create Payload Indices (only needed after creation) ---
        log::info!("Creating payload indices...");
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_FILE_PATH, FieldType::Keyword).await?;
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_DIR_PATH, FieldType::Keyword).await?;
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_FILE_EXTENSION, FieldType::Keyword).await?;
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_START_LINE, FieldType::Integer).await?;
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_END_LINE, FieldType::Integer).await?;
        log::info!("Payload indices created (or already exist).");

    } else {
        log::info!(
            "Collection '{}' already exists. Skipping creation.",
            CODE_SEARCH_COLLECTION
        );
        // Consider checking if indices exist even if collection exists, though ensure_payload_index handles it
    }

    // --- 5. Pre-calculate File Types ---
    let file_types_set: Option<HashSet<String>> = cmd_args
        .file_types
        .as_ref() // Borrow the Option's content
        .map(|ft_vec| ft_vec.iter().map(|s| s.trim_start_matches('.').to_lowercase()).collect());

    // --- 6. File Traversal and Processing ---
    log::info!("Starting file traversal and processing...");

    // Initialize progress bar (will be updated in helper)
    let pb_style = ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {processed_points}/{len} points ({per_sec}) {msg}",
    )?
    .progress_chars("#>-");
    let pb = ProgressBar::new(0); // Initial length 0, will be updated
    pb.set_style(pb_style.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message("Scanning directories...");

    let mut total_points_processed = 0;
    let client = Arc::new(client); // Share client with helper

    for dir in &cmd_args.dirs { // Iterate over a slice of the Vec
        let canonical_root_dir = match fs::canonicalize(dir) {
            Ok(path) => path,
            Err(e) => {
                log::error!("Failed to canonicalize directory {:?}: {}. Skipping.", dir, e);
                pb.println(format!("Error: Could not process directory {:?}: {}", dir, e));
                continue;
            }
        };

        let processed_count = process_directory(
            canonical_root_dir,
            &client,
            &embedding_handler,
            &file_types_set,
            cmd_args.chunk_max_length,
            cmd_args.chunk_overlap,
            &pb,
        )
        .await?; // Await the helper function

        total_points_processed += processed_count;
    }

    pb.finish_with_message(format!("Indexing complete. Processed {} points.", total_points_processed));
    log::info!("Indexing complete. Total points processed: {}", total_points_processed);

    Ok(())
}

/// Helper function to process files within a single directory.
/// Returns the number of points processed in this directory.
async fn process_directory(
    root_dir: PathBuf,
    client: &Arc<Qdrant>,
    embedding_handler: &Arc<EmbeddingHandler>,
    file_types_set: &Option<HashSet<String>>,
    chunk_max_length: usize,
    chunk_overlap: usize,
    pb: &ProgressBar,
) -> Result<u64> {
    let root_dir_str = root_dir.to_string_lossy().to_string();
    log::debug!("Processing directory: {:?}", root_dir);

    // Create the embedding model once for this directory
    let model = embedding_handler.create_embedding_model()
        .context("Failed to create embedding model for directory")?;

    let mut points_batch = Vec::with_capacity(BATCH_SIZE);
    let mut directory_points_processed = 0;

    // First pass to count files and chunks for progress bar accuracy
    let mut estimated_total_chunks = 0;
    for entry_result in WalkDir::new(&root_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry_result.path();
        if !path.is_file() {
            continue;
        }
        if let Some(ref extensions) = file_types_set {
            let file_extension = path.extension().and_then(|ext| ext.to_str()).map(|s| s.to_lowercase()).unwrap_or_default();
             if file_extension.is_empty() || !extensions.contains(&file_extension) {
                 continue;
             }
        }
        // Quick estimate: read file size, assume avg chunk size? More accurate: read and chunk.
        // For simplicity now, let's just count files and update length later.
         match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                // Use real chunking function here for estimation
                let chunks = chunk_by_lines(&content, chunk_max_length, chunk_overlap);
                estimated_total_chunks += chunks.len() as u64;
            }
            Err(e) => {
                log::warn!("Failed to read file for chunk estimation {:?}: {}. Skipping estimate.", path, e);
            }
         }
    }
    pb.set_length(estimated_total_chunks);
    pb.set_position(0); // Reset position
    pb.set_message(format!("Processing {}...", root_dir.file_name().unwrap_or_default().to_string_lossy()));


    // Second pass: actual processing
    for entry_result in WalkDir::new(&root_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry_result.path();
        if !path.is_file() {
            continue;
        }

        let file_path_str = path.to_string_lossy().to_string();
        let file_extension = path.extension().and_then(|ext| ext.to_str()).map(|s| s.to_lowercase()).unwrap_or_default();

        // Filter by file type if specified
        if let Some(ref extensions) = file_types_set {
             if file_extension.is_empty() || !extensions.contains(&file_extension) {
                 continue;
             }
        }

        pb.set_message(format!("Reading {}", path.file_name().unwrap_or_default().to_string_lossy()));

         match tokio::fs::read_to_string(path).await {
             Ok(content) => {
                 pb.set_message(format!("Chunking {}", path.file_name().unwrap_or_default().to_string_lossy()));
                 // Use real chunking function
                 let chunks = chunk_by_lines(&content, chunk_max_length, chunk_overlap);
                 log::trace!("File {:?} split into {} chunks", path, chunks.len());

                pb.set_message(format!("Embedding {}", path.file_name().unwrap_or_default().to_string_lossy()));
                for chunk in chunks {
                     // Generate embedding using the created model
                     match model.embed(&chunk.text) {
                         Ok(embedding) => {
                             let point_id = Uuid::new_v4().to_string();
                             let mut payload_map = std::collections::HashMap::<String, qdrant_client::qdrant::Value>::new();
                             payload_map.insert(FIELD_FILE_PATH.to_string(), file_path_str.clone().into());
                             payload_map.insert(FIELD_DIR_PATH.to_string(), root_dir_str.clone().into());
                             payload_map.insert(FIELD_START_LINE.to_string(), (chunk.start_line as i64).into());
                             payload_map.insert(FIELD_END_LINE.to_string(), (chunk.end_line as i64).into());
                             payload_map.insert(FIELD_FILE_EXTENSION.to_string(), file_extension.clone().into());

                             // Conversion from HashMap to Payload using `From` is infallible.
                             let payload = Payload::from(payload_map);
                             points_batch.push(PointStruct::new(
                                 point_id,
                                 embedding,
                                 payload,
                             ));

                             directory_points_processed += 1;
    
                             // Upsert if batch is full
                             if points_batch.len() >= BATCH_SIZE {
                                 pb.set_message(format!("Upserting batch ({})", points_batch.len()));
                                 let batch_to_upsert = std::mem::replace(&mut points_batch, Vec::with_capacity(BATCH_SIZE));
                                 if let Err(e) = upsert_batch(client.as_ref(), CODE_SEARCH_COLLECTION, batch_to_upsert, pb).await {
                                     pb.println(format!("Error upserting batch for {}: {}", path.display(), e));
                                     log::error!("Error upserting batch for {}: {}", path.display(), e);
                                 }
                                 pb.inc(BATCH_SIZE as u64); // Increment progress after successful or failed upsert
                             }
                         }
                         Err(e) => {
                             pb.println(format!("Error generating embedding for chunk in {}: {}. Skipping chunk.", file_path_str, e));
                             log::error!("Error generating embedding for chunk in {}: {}. Skipping chunk.", file_path_str, e);
                         }
                     }
                }
             }
             Err(e) => {
                 pb.println(format!("Error reading file {}: {}. Skipping file.", path.display(), e));
                 log::error!("Error reading file {}: {}. Skipping file.", path.display(), e);
             }
         }
    }

    // Upsert any remaining points in the batch
    if !points_batch.is_empty() {
        pb.inc(points_batch.len() as u64); // Increment progress for the final batch
        pb.set_message(format!("Upserting final batch ({})", points_batch.len()));
        if let Err(e) = upsert_batch(client.as_ref(), CODE_SEARCH_COLLECTION, points_batch, pb).await {
            pb.println(format!("Error upserting final batch for {}: {}", root_dir.display(), e));
            log::error!("Error upserting final batch for {}: {}", root_dir.display(), e);
        }
    }

    Ok(directory_points_processed)
} 