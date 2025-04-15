use anyhow::{bail, Context, Result};
use clap::Args;
use indicatif::{ProgressBar, ProgressStyle};
use qdrant_client::{
    qdrant::{ CreateCollectionBuilder, Distance, FieldType, VectorParamsBuilder, PointStruct },
    Payload,
    Qdrant,
};
use std::{
    path::PathBuf, sync::Arc, time::Duration, collections::HashSet,
};
use walkdir::WalkDir;
use uuid::Uuid;

use crate::{
    config::{AppConfig},
    syntax,
    vectordb::{embedding, embedding_logic::EmbeddingHandler},
};

use super::commands::{
    upsert_batch, BATCH_SIZE, CliArgs, FIELD_CHUNK_CONTENT, FIELD_ELEMENT_TYPE,
    FIELD_END_LINE, FIELD_FILE_EXTENSION, FIELD_FILE_PATH, FIELD_LANGUAGE, FIELD_START_LINE,
    ensure_payload_index, SIMPLE_INDEX_COLLECTION,
};
use super::repo_commands::{DEFAULT_VECTOR_DIMENSION};

// const LEGACY_INDEX_COLLECTION: &str = "vectordb-code-search"; // Removed: Now defined in commands.rs

#[derive(Args, Debug)]
pub struct IndexArgs {
    /// One or more directories or files to index.
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,

    /// Optional file extensions to include (e.g., ".rs", ".py"). If omitted, all files are attempted.
    #[arg(short = 'e', long = "extension")]
    pub file_extensions: Option<Vec<String>>,
    // TODO: Add --collection argument to specify target collection? For now, use default.
}

/// Handles the `index` command (legacy mode), processing specified paths into a default collection.
pub async fn handle_index(
    cmd_args: &IndexArgs,
    cli_args: &CliArgs,
    config: AppConfig, // Keep config for ONNX paths
    client: Arc<Qdrant>,
) -> Result<()> {
    log::info!("Starting legacy indexing process...");

    // --- 1. Use Dedicated Collection Name --- 
    let collection_name = SIMPLE_INDEX_COLLECTION;
    log::info!("Indexing into default collection: '{}'", collection_name);

    // Ensure the legacy collection exists and has basic indices
    ensure_simple_collection_exists(&client, collection_name).await?;

    // --- 2. Validate Input Paths --- 
    for path in &cmd_args.paths {
        if !path.exists() {
             bail!("Input path does not exist: {}", path.display());
        }
    }
    log::info!("Processing input paths: {:?}", cmd_args.paths);

    // --- 3. Initialize Embedding Handler (Ensure ONNX paths are resolved) --- 
    let model_env_var = std::env::var("VECTORDB_ONNX_MODEL").ok();
    let tokenizer_env_var = std::env::var("VECTORDB_ONNX_TOKENIZER_DIR").ok();

    if cli_args.onnx_model_path_arg.is_some() && model_env_var.is_some() {
        return Err(anyhow::anyhow!("Cannot provide ONNX model path via both --onnx-model argument and VECTORDB_ONNX_MODEL environment variable."));
    }
    if cli_args.onnx_tokenizer_dir_arg.is_some() && tokenizer_env_var.is_some() {
        return Err(anyhow::anyhow!("Cannot provide ONNX tokenizer dir via both --onnx-tokenizer-dir argument and VECTORDB_ONNX_TOKENIZER_DIR environment variable."));
    }

    let onnx_model_path_str = cli_args.onnx_model_path_arg.as_ref()
        .or(model_env_var.as_ref())
        .or(config.onnx_model_path.as_ref())
        .ok_or_else(|| anyhow::anyhow!("ONNX model path must be provided via --onnx-model, VECTORDB_ONNX_MODEL, or config"))?;
    let onnx_tokenizer_dir_str = cli_args.onnx_tokenizer_dir_arg.as_ref()
        .or(tokenizer_env_var.as_ref())
        .or(config.onnx_tokenizer_path.as_ref())
        .ok_or_else(|| anyhow::anyhow!("ONNX tokenizer path must be provided via --onnx-tokenizer-dir, VECTORDB_ONNX_TOKENIZER_DIR, or config"))?;

    let onnx_model_path = PathBuf::from(onnx_model_path_str);
    let onnx_tokenizer_path = PathBuf::from(onnx_tokenizer_dir_str);

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

    log::info!("Initializing embedding handler...");
    let embedding_handler = Arc::new(
        EmbeddingHandler::new(
            embedding::EmbeddingModelType::Onnx,
            Some(onnx_model_path),
            Some(onnx_tokenizer_path),
        )
        .context("Failed to initialize embedding handler")?,
    );
    let embedding_dim = embedding_handler
        .dimension()
        .context("Failed to get embedding dimension")?;
    log::info!("Embedding dimension: {}", embedding_dim);

    // Ensure the collection *exists* before proceeding (could have been deleted manually)
    if !client.collection_exists(collection_name.to_string()).await? {
        bail!("Collection '{}' not found. Please run 'repo add' again or check Qdrant.", collection_name);
    }

    // --- 4. Pre-calculate File Types Filter ---
    let file_types_set: Option<HashSet<String>> = cmd_args
        .file_extensions
        .as_ref()
        .map(|ft_vec| {
            ft_vec
                .iter()
                .map(|s| s.trim_start_matches('.').to_lowercase())
                .collect()
        });
    if let Some(ref ft_set) = file_types_set {
        log::info!("Filtering by file extensions: {:?}", ft_set);
    }

    // --- 5. File Traversal and Processing ---
    log::info!("Starting file traversal and processing...");

    // Initialize progress bar
    let pb_style = ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({per_sec}) {msg}",
    )?
    .progress_chars("#>-");
    let pb = ProgressBar::new(0);
    pb.set_style(pb_style);
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message("Scanning directories...");

    let mut files_to_process = Vec::new();

    // --- First pass: Collect files (absolute paths) --- 
    for path_arg in &cmd_args.paths {
        if path_arg.is_file() {
            // Handle direct file path argument
            if let Some(ref filter_set) = file_types_set {
                let extension = path_arg
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                if filter_set.contains(&extension) {
                     files_to_process.push(path_arg.clone());
                } else {
                    log::trace!("Skipping file due to extension filter: {}", path_arg.display());
                }
            } else {
                 files_to_process.push(path_arg.clone());
            }
        } else if path_arg.is_dir() {
             // Handle directory path argument
             for entry_result in WalkDir::new(path_arg).into_iter().filter_map(|e| e.ok()) {
                 let absolute_path = entry_result.path();
                 if !absolute_path.is_file() {
                     continue;
                 }

                 if let Some(ref filter_set) = file_types_set {
                     let extension = absolute_path
                         .extension()
                         .and_then(|ext| ext.to_str())
                         .map(|s| s.to_lowercase())
                         .unwrap_or_default();
                     if !filter_set.contains(&extension) {
                         log::trace!("Skipping file due to extension filter: {}", absolute_path.display());
                         continue;
                     }
                 }
                 files_to_process.push(absolute_path.to_path_buf()); 
             }
        } else {
            log::warn!("Input path is neither a file nor a directory: {}. Skipping.", path_arg.display());
        }
    }
    
    pb.set_length(files_to_process.len() as u64);
    pb.set_position(0);
    pb.set_message("Processing files...");

    // --- Second pass: Process each file ---
    let mut total_points_processed = 0;
    let mut total_files_processed = 0;
    let mut total_files_skipped = 0;

    let model = embedding_handler
        .create_embedding_model()
        .context("Failed to create embedding model")?;

    let mut points_batch = Vec::with_capacity(BATCH_SIZE);

    for file_path in files_to_process { // file_path is absolute here
        let absolute_path_str = file_path.to_string_lossy().to_string(); // Use absolute path string
        log::debug!("Processing file: {}", file_path.display());

        // --- 1. Get Chunks ---
        let chunks = match syntax::get_chunks(&file_path) {
            Ok(chunks) => chunks,
            Err(e) => {
                log::warn!("Failed to parse file {}: {}. Skipping.", file_path.display(), e);
                pb.println(format!("Warning: Failed to parse {}, skipping.", file_path.display()));
                total_files_skipped += 1;
                pb.inc(1);
                continue;
            }
        };

        if chunks.is_empty() {
            log::debug!("No text chunks found in file {}. Skipping.", file_path.display());
            total_files_skipped += 1;
            pb.inc(1);
            continue;
        }

        // --- 2. Generate Embeddings (Batching within the loop) ---
        let chunk_contents: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
        let embeddings = match model.embed_batch(&chunk_contents) {
            Ok(embeddings) => embeddings,
            Err(e) => {
                log::error!(
                    "Failed to generate embeddings for {}: {}. Skipping file.",
                    file_path.display(),
                    e
                );
                pb.println(format!("Error embedding {}, skipping.", file_path.display()));
                total_files_skipped += 1;
                 pb.inc(1); // Increment progress even if skipped after parsing
                continue;
            }
        };

        // --- 3. Create PointStructs ---
        let file_extension = file_path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_string();

        for (i, chunk) in chunks.iter().enumerate() {
            let mut payload = Payload::new();
            payload.insert(FIELD_FILE_PATH, absolute_path_str.clone()); // Store absolute path 
            payload.insert(FIELD_START_LINE, chunk.start_line as i64);
            payload.insert(FIELD_END_LINE, chunk.end_line as i64);
            payload.insert(FIELD_LANGUAGE, chunk.language.to_string());
            payload.insert(FIELD_FILE_EXTENSION, file_extension.clone());
            payload.insert(FIELD_ELEMENT_TYPE, chunk.element_type.clone());
            payload.insert(FIELD_CHUNK_CONTENT, chunk.content.clone());

            let point = PointStruct::new(
                Uuid::new_v4().to_string(), // Generate new UUID for each chunk
                embeddings[i].clone(), // Use the corresponding embedding
                payload,
            );
            points_batch.push(point);

            // Upsert batch if full
            if points_batch.len() >= BATCH_SIZE {
                let batch_to_upsert = std::mem::take(&mut points_batch);
                 upsert_batch(&client, &collection_name, batch_to_upsert, &pb).await?;
                total_points_processed += BATCH_SIZE;
            }
        }
        total_files_processed += 1;
        pb.inc(1); // Increment progress after file is fully processed (or skipped)
    }

    // Upsert any remaining points
    if !points_batch.is_empty() {
        let final_batch_size = points_batch.len();
         upsert_batch(&client, &collection_name, points_batch, &pb).await?;
        total_points_processed += final_batch_size;
    }

    pb.finish_with_message("Indexing complete!");

    // --- Final Summary ---
    log::info!("Indexing finished.");
    log::info!("Total files processed: {}", total_files_processed);
    log::info!("Total files skipped: {}", total_files_skipped);
    log::info!("Total points upserted: {}", total_points_processed);

    Ok(())
}

// Helper function to ensure the simple collection exists
async fn ensure_simple_collection_exists(
    client: &Qdrant,
    collection_name: &str,
) -> Result<()> {
    // Similar logic to ensure_repository_collection_exists, but without repo-specific fields
    let exists = client.collection_exists(collection_name.to_string()).await?; // Pass String
    if !exists {
        log::info!("Simple index collection '{}' does not exist. Creating...", collection_name);
        // Determine embedding dimension (need EmbeddingHandler or pass dimension)
        // For simplicity, hardcode or get from a global config/default for now.
        // Ideally, the first index run defines dimension, or it's pre-configured.
        let vector_params = VectorParamsBuilder::new(DEFAULT_VECTOR_DIMENSION, Distance::Cosine).build(); // Use constant from repo_commands
        let create_request = CreateCollectionBuilder::new(collection_name)
             .vectors_config(vector_params)
             .build();
        client.create_collection(create_request).await?;
        log::info!("Simple index collection '{}' created.", collection_name);
         // Add wait loop like in ensure_repository_collection_exists
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut attempts = 0;
        loop {
            let info = client.collection_info(collection_name.to_string()).await?; // Pass String
            if info.result.map_or(false, |i| i.status == qdrant_client::qdrant::CollectionStatus::Green as i32) {
                break;
            }
            attempts += 1;
            if attempts > 50 {
                bail!("Collection '{}' did not become ready in time.", collection_name);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
         log::info!("Collection '{}' is ready.", collection_name);
    }
    // Ensure basic indices (Use public ensure_payload_index)
    ensure_payload_index(client, collection_name, FIELD_FILE_PATH, FieldType::Keyword).await?;
    ensure_payload_index(client, collection_name, FIELD_START_LINE, FieldType::Integer).await?;
    ensure_payload_index(client, collection_name, FIELD_END_LINE, FieldType::Integer).await?;
    ensure_payload_index(client, collection_name, FIELD_LANGUAGE, FieldType::Keyword).await?;

    Ok(())
}