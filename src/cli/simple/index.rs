use anyhow::{anyhow, Context, Result};
use std::collections::HashSet;
use std::path::PathBuf;
use indicatif::ProgressBar;
use uuid::Uuid;
use walkdir::WalkDir;
use qdrant_client::{qdrant::PointStruct, Payload, Qdrant};

use crate::vectordb::embedding_logic::EmbeddingHandler;
use crate::cli::commands::{
    upsert_batch, BATCH_SIZE, 
    FIELD_CHUNK_CONTENT, FIELD_ELEMENT_TYPE, FIELD_END_LINE, FIELD_FILE_EXTENSION,
    FIELD_FILE_PATH, FIELD_LANGUAGE, FIELD_START_LINE
};
use crate::syntax;

/// Index files from specified paths into a Qdrant collection
///
/// # Arguments
/// * `client` - Qdrant client
/// * `collection_name` - Name of the collection to index into
/// * `embedding_handler` - Embedding handler for creating embeddings
/// * `paths` - Paths to index
/// * `extensions` - Optional extensions to filter by
/// * `progress_bar` - Progress bar for displaying progress
///
/// # Returns
/// * `Result<(usize, usize)>` - (indexed files, indexed chunks)
pub async fn index_paths(
    client: &Qdrant,
    collection_name: &str,
    embedding_handler: &EmbeddingHandler,
    paths: &[PathBuf],
    extensions: &[String],
    progress_bar: &ProgressBar,
) -> Result<(usize, usize)> {
    let mut files_to_process = Vec::new();
    let file_types_set: Option<HashSet<String>> = if extensions.is_empty() {
        None
    } else {
        Some(extensions.iter()
            .map(|s| s.trim_start_matches('.').to_lowercase())
            .collect())
    };

    for path_arg in paths {
        let absolute_path_arg = path_arg.canonicalize()
            .with_context(|| format!("Failed to get absolute path for: {}", path_arg.display()))?;
        
        if absolute_path_arg.is_file() {
            let should_process = match &file_types_set {
                Some(filter_set) => {
                    let extension = absolute_path_arg
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|s| s.to_lowercase())
                        .unwrap_or_default();
                    filter_set.contains(&extension)
                }
                None => true,
            };

            if should_process {
                files_to_process.push(absolute_path_arg);
            }
        } else if absolute_path_arg.is_dir() {
            for entry_result in WalkDir::new(&absolute_path_arg).into_iter().filter_map(|e| e.ok()) {
                let entry_path = entry_result.path();
                if !entry_path.is_file() {
                    continue;
                }

                let should_process = match &file_types_set {
                    Some(filter_set) => {
                        let extension = entry_path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .map(|s| s.to_lowercase())
                            .unwrap_or_default();
                        filter_set.contains(&extension)
                    }
                    None => true,
                };

                if should_process {
                    files_to_process.push(entry_path.to_path_buf());
                }
            }
        }
    }
    
    progress_bar.set_length(files_to_process.len() as u64);
    progress_bar.set_position(0);
    progress_bar.set_message("Processing files...");

    let mut total_points_processed: usize = 0;
    let mut total_files_processed: usize = 0;
    let total_files_to_scan = files_to_process.len();

    let model = embedding_handler
        .create_embedding_model()
        .context("Failed to create embedding model")?;

    let mut points_batch = Vec::with_capacity(BATCH_SIZE);
    let mut batch_num = 1;
    let total_batches_estimate = (total_files_to_scan / BATCH_SIZE).max(1); // Ensure at least 1

    for file_path in files_to_process {
        let absolute_path_str = file_path.to_string_lossy().to_string();
        
        let chunks = match syntax::get_chunks(&file_path) {
            Ok(chunks) => chunks,
            Err(_) => {
                progress_bar.inc(1);
                continue;
            }
        };

        if chunks.is_empty() {
            progress_bar.inc(1);
            continue;
        }

        let chunk_contents: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
        let embeddings = match model.embed_batch(&chunk_contents) {
            Ok(embeddings) => embeddings,
            Err(_) => {
                progress_bar.inc(1);
                continue;
            }
        };

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
                Uuid::new_v4().to_string(),
                embeddings[i].clone(),
                payload,
            );
            points_batch.push(point);

            if points_batch.len() >= BATCH_SIZE {
                let batch_to_upsert = std::mem::take(&mut points_batch);
                let current_batch_size = batch_to_upsert.len();
                upsert_batch(client, collection_name, batch_to_upsert, batch_num, total_batches_estimate, progress_bar).await?;
                total_points_processed += current_batch_size;
                batch_num += 1;
            }
        }
        total_files_processed += 1;
        progress_bar.inc(1);
    }

    if !points_batch.is_empty() {
        let final_batch_size = points_batch.len();
        upsert_batch(client, collection_name, points_batch, batch_num, total_batches_estimate, progress_bar).await?;
        total_points_processed += final_batch_size;
    }

    Ok((total_files_processed, total_points_processed))
} 