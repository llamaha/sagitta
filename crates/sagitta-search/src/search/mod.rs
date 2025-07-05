// crates/sagitta-search/src/search/mod.rs
//! Core module for handling search operations (semantic, potentially others later).

pub mod result;

pub use result::SearchResult;

// --- Add imports needed for search_semantic --- 
use std::sync::Arc;
use std::path::PathBuf;
use log::{debug, warn};
use qdrant_client::qdrant::{SearchPointsBuilder, Filter};
use crate::config::AppConfig;
use crate::qdrant_client_trait::QdrantClientTrait;
use crate::{EmbeddingPool, app_config_to_embedding_config};
use crate::repo_helpers::get_branch_aware_collection_name;
use crate::error::SagittaError;
use crate::constants::{FIELD_FILE_PATH, FIELD_START_LINE, FIELD_END_LINE, FIELD_CHUNK_CONTENT};
use sagitta_embed::EmbeddingProcessor;
// --- End imports --- 

/// Performs semantic search against a specific repository collection in Qdrant.
pub async fn search_semantic<C>(
    query: &str,
    limit: usize,
    filter: Option<Filter>,
    repo_name: &str,
    branch_name: &str,
    config: &Arc<AppConfig>, // Use Arc<AppConfig> for consistency
    client: Arc<C>,
) -> Result<Vec<SearchResult>, SagittaError>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    debug!("Performing semantic search query=\"{query}\" repo=\"{repo_name}\" branch=\"{branch_name}\" limit={limit} filter={filter:?}");

    // 1. Get Query Embedding using EmbeddingPool
    let embedding_config = app_config_to_embedding_config(config);
    let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config)
        .map_err(|e| SagittaError::EmbeddingError(e.to_string()))?;
    
    // Create a dummy ProcessedChunk for the query
    use sagitta_embed::processor::{ProcessedChunk, ChunkMetadata};
    let query_chunk = ProcessedChunk {
        content: query.to_string(),
        metadata: ChunkMetadata {
            file_path: PathBuf::from("query"),
            start_line: 0,
            end_line: 0,
            language: "text".to_string(),
            file_extension: "txt".to_string(),
            element_type: "query".to_string(),
            context: None,
        },
        id: "query".to_string(),
    };
    
    let embedded_chunks = embedding_pool.process_chunks(vec![query_chunk]).await
        .map_err(|e| SagittaError::EmbeddingError(e.to_string()))?;
    
    let query_embedding = embedded_chunks.into_iter().next()
        .ok_or_else(|| SagittaError::EmbeddingError("Embedding pool returned no embedding for query".to_string()))?
        .embedding;

    debug!("Generated query embedding of dimension {}", query_embedding.len());

    // 2. Determine Collection Name using branch-aware naming
    let collection_name = get_branch_aware_collection_name(repo_name, branch_name, config);
    debug!("Searching collection: {collection_name}");

    // 3. Build Qdrant Search Request
    let mut builder = SearchPointsBuilder::new(collection_name, query_embedding, limit as u64);
    if let Some(f) = filter {
        builder = builder.filter(f); // Conditionally apply filter
    }
    let search_request = builder.with_payload(true).build(); // Build after potential filter

    // 4. Execute Search
    let search_response = client
        .search_points(search_request) // Pass ownership
        .await?;
    
    debug!("Received {} search results from Qdrant", search_response.result.len());

    // 5. Map Results
    let mut results: Vec<SearchResult> = Vec::new();
    for scored_point in search_response.result {
        let payload = scored_point.payload;
        let file_path_opt = payload.get(FIELD_FILE_PATH).and_then(|v| v.as_str().map(PathBuf::from));
        let start_line_opt = payload.get(FIELD_START_LINE).and_then(|v| v.as_integer().map(|i| i as usize));
        let end_line_opt = payload.get(FIELD_END_LINE).and_then(|v| v.as_integer().map(|i| i as usize));
        let text_opt = payload.get(FIELD_CHUNK_CONTENT).and_then(|v| v.as_str().map(String::from));

        match (file_path_opt, start_line_opt, end_line_opt, text_opt) {
            (Some(file_path), Some(start_line), Some(end_line), Some(text)) => {
                results.push(SearchResult {
                    file_path,
                    start_line,
                    end_line,
                    score: scored_point.score,
                    text,
                });
            }
            _ => {
                warn!("Search result point id={:?} missing required payload fields ({}, {}, {}, {}). Skipping.", 
                      scored_point.id, FIELD_FILE_PATH, FIELD_START_LINE, FIELD_END_LINE, FIELD_CHUNK_CONTENT);
            }
        }
    }

    debug!("Mapped {} valid search results", results.len());
    Ok(results)
}

// search_semantic function was implemented here. 

/// Search module providing vector store traits and search utilities.
///
/// This module contains the central vector store trait that can be used
/// by GUI components and analytics without pulling in all of sagitta-search.

pub mod vector_store;

pub use vector_store::{VectorStore, VectorStoreError, UpsertResult, CollectionConfig, DistanceMetric, VectorPoint, SearchQuery, CollectionInfo, CollectionStatus, ScrollResult};
// Re-export SearchResult with a different name to avoid conflict
pub use vector_store::SearchResult as VectorSearchResult; 