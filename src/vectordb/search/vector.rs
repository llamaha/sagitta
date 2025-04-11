use crate::vectordb::db::VectorDB;
use crate::vectordb::embedding::EmbeddingModel;
use log::{debug, warn, error};
use std::collections::HashSet;
use anyhow::anyhow;
use std::cmp::Ordering;
use super::result::SearchResult;

/// Standard search using vector similarity with a limit on the number of results
pub(crate) fn search_with_limit(
    db: &VectorDB, // Pass db as reference
    model: &mut EmbeddingModel, // Pass model as mutable reference
    query: &str,
    max_results: usize,
) -> anyhow::Result<Vec<SearchResult>> {
    debug!("Performing vector search for query: {}", query);

    // Validate query
    if query.trim().is_empty() {
        debug!("Empty query detected, returning empty results");
        return Ok(Vec::new());
    }

    // Convert the query to an embedding
    debug!("Converting query to embedding vector");
    let query_embedding = model.embed(query).map_err(|e| anyhow!(e))?;
    debug!("Generated embedding of dimension {}", query_embedding.len());

    let ef_search = 100; // Example, make configurable?
    let hnsw_index = match db.hnsw_index() {
        Some(index) => index,
        None => {
            warn!("Attempted search but HNSW index is not built.");
            return Ok(Vec::new()); // Return empty results if no index
        }
    };

    let search_results = hnsw_index.search_parallel(&query_embedding, max_results * 5, ef_search)?;

    // Process results
    let mut final_results: Vec<SearchResult> = Vec::with_capacity(search_results.len());
    for (node_id, distance) in search_results {
        let similarity = 1.0 - distance;
        if similarity < 0.0 { continue; } // Skip highly dissimilar results

        // Retrieve chunk data using node_id
        if let Some(chunk) = db.indexed_chunks.get(node_id) {
             // Create a SearchResult 
             final_results.push(SearchResult {
                 file_path: chunk.file_path.clone(),
                 similarity, // Use the HNSW similarity
                 // Optional: Add chunk info like start/end lines if SearchResult is adapted
             });
        } else {
             error!("HNSW search returned invalid node ID: {}", node_id);
        }
    }
    
    // Sort by similarity (descending)
    final_results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(Ordering::Equal));

    // Deduplicate results by file path, keeping the one with the highest similarity
    let mut unique_results = Vec::new();
    let mut seen_files = HashSet::new();
    for result in final_results {
        if seen_files.insert(result.file_path.clone()) {
            unique_results.push(result);
        }
    }
    
    // Apply the final limit
    unique_results.truncate(max_results);
    Ok(unique_results)

    /* // Old logic using db.embeddings
    let embeddings_map = &db
        .embeddings
        .par_iter()
        .filter(|(path, _)| {
            // ... (file type filtering) ...
        })
        .map(|(path, embedding)| (path.clone(), embedding))
        .collect::<HashMap<_, _>>();

    if embeddings_map.is_empty() {
        return Ok(vec![]);
    }

    let mut results: Vec<SearchResult> = embeddings_map
        .par_iter()
        .map(|(path, embedding)| {
            let similarity = 1.0 - crate::vectordb::utils::cosine_distance(&query_embedding, embedding);
            SearchResult {
                file_path: path.clone(),
                similarity,
            }
        })
        .collect();
    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(Ordering::Equal));
    results.truncate(limit);
    Ok(results)
    */
} 