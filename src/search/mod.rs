// crates/vectordb-core/src/search/mod.rs
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
use crate::embedding::EmbeddingHandler;
use crate::repo_helpers::get_collection_name;
use crate::error::VectorDBError;
use crate::constants::{FIELD_FILE_PATH, FIELD_START_LINE, FIELD_END_LINE, FIELD_CHUNK_CONTENT};
// --- End imports --- 

/// Performs semantic search against a specific repository collection in Qdrant.
pub async fn search_semantic<C>(
    query: &str,
    limit: usize,
    filter: Option<Filter>,
    tenant_id: &str,
    repo_name: &str,
    config: &Arc<AppConfig>, // Use Arc<AppConfig> for consistency
    client: Arc<C>,
) -> Result<Vec<SearchResult>, VectorDBError>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    debug!("Performing semantic search query=\"{}\" repo=\"{}\" limit={} filter={:?}", query, repo_name, limit, filter);

    // 1. Get Query Embedding
    let embedding_handler = EmbeddingHandler::new(&*config)
        .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))?;
    let embeddings = embedding_handler.embed(&[query])
        .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))?;
    let query_embedding = embeddings.into_iter().next()
        .ok_or_else(|| VectorDBError::EmbeddingError("Embedding handler returned no embedding for query".to_string()))?;

    debug!("Generated query embedding of dimension {}", query_embedding.len());

    // 2. Determine Collection Name
    let collection_name = get_collection_name(tenant_id, repo_name, &**config);
    debug!("Searching collection: {}", collection_name);

    // 3. Build Qdrant Search Request
    let mut builder = SearchPointsBuilder::new(collection_name, query_embedding, limit as u64);
    if let Some(f) = filter {
        builder = builder.filter(f); // Conditionally apply filter
    }
    let search_request = builder.with_payload(true).build(); // Build after potential filter

    // 4. Execute Search
    let search_response = client
        .search_points(search_request) // Pass ownership
        .await
        .map_err(VectorDBError::from)?;
    
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