use anyhow::{Context, Result};
use clap::Args;
use indicatif::{ProgressBar, ProgressStyle};
use qdrant_client::{
    qdrant::{
        CreateCollection, CreateCollectionBuilder, Distance, FieldType,
        PointStruct,
        VectorParamsBuilder, VectorsConfig,
    },
    Payload, Qdrant,
};
use std::{
    collections::HashSet,
    fs,
    path::{PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    config::AppConfig,
    syntax, // Import syntax module
    vectordb::{embedding, embedding_logic::EmbeddingHandler},
};

use super::commands::{
    ensure_payload_index, upsert_batch, CliArgs, CODE_SEARCH_COLLECTION, FIELD_CHUNK_CONTENT,
    FIELD_DIR_PATH, FIELD_ELEMENT_TYPE, FIELD_END_LINE, FIELD_FILE_EXTENSION, FIELD_FILE_PATH,
    FIELD_LANGUAGE, FIELD_START_LINE,
};

const BATCH_SIZE: usize = 128;

#[derive(Args, Debug)]
pub struct IndexArgs {
    /// Directories to index
    #[arg(required = true)]
    pub dirs: Vec<PathBuf>,

    /// Optional file extensions to include (e.g., ".rs", ".py"). If omitted, all files are attempted.
    #[arg(short = 't', long = "type")]
    pub file_types: Option<Vec<String>>,
}

/// Handles the `index` command, processing directories and upserting data into Qdrant.
pub async fn handle_index(
    cmd_args: &IndexArgs,
    cli_args: &CliArgs,
    config: &AppConfig,
) -> Result<()> {
    log::info!("Starting indexing process...");
    log::debug!("IndexArgs: {:?}", cmd_args);
    log::debug!("CliArgs: {:?}, Config: {:?}", cli_args, config);
    log::info!("Using Qdrant URL: {}", config.qdrant_url);

    // --- Resolve ONNX Paths ---
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


    // --- 1. Initialize Embedding Handler ---
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

    // --- 2. Initialize Qdrant Client ---
    log::info!("Connecting to Qdrant at {}", config.qdrant_url);
    let client = Arc::new(Qdrant::from_url(&config.qdrant_url).build()?);
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
                VectorParamsBuilder::new(embedding_dim as u64, Distance::Cosine).build(),
            )),
        };
        let create_request: CreateCollection = CreateCollectionBuilder::new(CODE_SEARCH_COLLECTION)
            .vectors_config(vectors_config)
            .build();
        client
            .create_collection(create_request)
            .await?;
        log::info!("Collection '{}' created.", CODE_SEARCH_COLLECTION);

        // --- 4. Create Payload Indices (only needed after creation) ---
        log::info!("Creating payload indices...");
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_FILE_PATH, FieldType::Keyword).await?;
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_DIR_PATH, FieldType::Keyword).await?;
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_FILE_EXTENSION, FieldType::Keyword).await?;
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_START_LINE, FieldType::Integer).await?;
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_END_LINE, FieldType::Integer).await?;
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_LANGUAGE, FieldType::Keyword).await?;
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_ELEMENT_TYPE, FieldType::Keyword).await?;
        ensure_payload_index(&client, CODE_SEARCH_COLLECTION, FIELD_CHUNK_CONTENT, FieldType::Text).await?;
        log::info!("Payload indices created (or already exist).");
    } else {
        log::info!(
            "Collection '{}' already exists. Skipping creation and index setup.",
            CODE_SEARCH_COLLECTION
        );
    }

    // --- 5. Pre-calculate File Types Filter ---
    let file_types_set: Option<HashSet<String>> = cmd_args
        .file_types
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

    // --- 6. File Traversal and Processing ---
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

    // --- First pass: Collect files ---
    for dir in &cmd_args.dirs {
        let canonical_root_dir = match fs::canonicalize(dir) {
            Ok(path) => path,
            Err(e) => {
                log::error!("Failed to canonicalize directory {:?}: {}. Skipping.", dir, e);
                pb.println(format!("Error: Could not process directory {:?}: {}", dir, e));
                continue;
            }
        };
        log::debug!("Scanning directory: {}", canonical_root_dir.display());

        for entry_result in WalkDir::new(&canonical_root_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry_result.path();
            if !path.is_file() {
                continue;
            }

            if let Some(ref filter_set) = file_types_set {
                let extension = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                if !filter_set.contains(&extension) {
                    log::trace!("Skipping file due to extension filter: {}", path.display());
                    continue;
                }
            }

            log::trace!("Found file to process: {}", path.display());
            files_to_process.push((path.to_path_buf(), canonical_root_dir.clone()));
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

    for (file_path, root_dir) in files_to_process {
        let root_dir_str = root_dir.to_string_lossy().to_string();
        let file_path_str = file_path.to_string_lossy().to_string();
        log::debug!("Processing file: {}", file_path_str);

        // --- 1. Get Chunks ---
        let chunks = match syntax::get_chunks(&file_path) {
            Ok(chunks) => chunks,
            Err(e) => {
                log::warn!("Failed to parse file {}: {}. Skipping.", file_path_str, e);
                pb.println(format!("Warning: Failed to parse {}, skipping.", file_path_str));
                total_files_skipped += 1;
                pb.inc(1);
                continue;
            }
        };

        if chunks.is_empty() {
            log::debug!("No chunks extracted from file: {}", file_path_str);
            // Don't count as skipped, just processed with 0 points
            total_files_processed += 1;
            pb.inc(1);
            continue;
        }

        // --- 2. Prepare Embeddings ---
        let contents: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();

        let embeddings = match model.embed_batch(&contents) {
            Ok(embeddings) => embeddings,
            Err(e) => {
                log::error!("Failed to generate embeddings for {}: {}. Skipping file.", file_path_str, e);
                pb.println(format!("Error: Failed embeddings for {}, skipping.", file_path_str));
                total_files_skipped += 1;
                pb.inc(1);
                continue;
            }
        };

        // --- 3. Create Points ---
        let file_extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        for (chunk, embedding) in chunks.iter().zip(embeddings.into_iter()) {
            let mut payload = Payload::new();
            payload.insert(FIELD_FILE_PATH, file_path_str.clone());
            payload.insert(FIELD_DIR_PATH, root_dir_str.clone());
            payload.insert(FIELD_START_LINE, chunk.start_line as i64);
            payload.insert(FIELD_END_LINE, chunk.end_line as i64);
            payload.insert(FIELD_FILE_EXTENSION, file_extension.clone());
            payload.insert(FIELD_LANGUAGE, chunk.language.clone());
            payload.insert(FIELD_ELEMENT_TYPE, chunk.element_type.clone());
            payload.insert(FIELD_CHUNK_CONTENT, chunk.content.clone());

            let point = PointStruct::new(
                Uuid::new_v4().to_string(),
                embedding, // Pass Vec<f32> directly
                payload,
            );
            points_batch.push(point);

            // --- 4. Upsert Batch if full ---
            if points_batch.len() >= BATCH_SIZE {
                let batch_to_upsert = std::mem::take(&mut points_batch);
                let upsert_count = batch_to_upsert.len();
                // Use a temporary progress bar for the batch upsert to avoid confusing main progress
                let batch_pb = ProgressBar::hidden();
                upsert_batch(
                    &client,
                    CODE_SEARCH_COLLECTION,
                    batch_to_upsert,
                    &batch_pb,
                )
                .await
                .context(format!("Failed to upsert batch during file {}", file_path_str))?;
                total_points_processed += upsert_count as u64;
                log::trace!("Upserted batch of {} points", upsert_count);
            }
        }
        total_files_processed += 1;
        pb.inc(1);
    }

    // --- 5. Upsert remaining points ---
    if !points_batch.is_empty() {
        let final_batch_count = points_batch.len();
        let final_pb = ProgressBar::hidden();
        upsert_batch(
            &client,
            CODE_SEARCH_COLLECTION,
            points_batch,
            &final_pb,
        )
        .await
        .context("Failed to upsert final batch")?;
        total_points_processed += final_batch_count as u64;
        log::trace!("Upserted final batch of {} points", final_batch_count);
    }

    pb.finish_with_message(format!(
        "Indexing complete. Processed {} files ({} skipped). Upserted {} points.",
        total_files_processed, total_files_skipped, total_points_processed
    ));
    log::info!(
        "Indexing complete. Processed {} files ({} skipped). Total points upserted: {}",
        total_files_processed, total_files_skipped, total_points_processed
    );

    Ok(())
}