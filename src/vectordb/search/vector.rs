use crate::vectordb::db::VectorDB;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::error::Result;
use crate::vectordb::snippet_extractor::SnippetExtractor;
use super::result::SearchResult;
use super::snippet::get_snippet; // Use the fallback snippet function from the snippet module
use crate::vectordb::utils::cosine_distance; // Ensure this import is present
use log::{debug, warn};
use std::collections::HashMap;

// Constants for Vector Search
pub(crate) const SIMILARITY_THRESHOLD: f32 = 0.25;
pub(crate) const SPECIALIZED_SEARCH_THRESHOLD: f32 = 0.25;
pub(crate) const HNSW_TOP_K: usize = 20;

/// Standard search using vector similarity with a limit on the number of results
pub(crate) fn search_with_limit(
    db: &VectorDB, // Pass db as reference
    model: &mut EmbeddingModel, // Pass model as mutable reference
    snippet_extractor: &mut SnippetExtractor, // Pass snippet_extractor as mutable reference
    query: &str,
    max_results: usize,
) -> Result<Vec<SearchResult>> {
    debug!("Performing vector search for query: {}", query);

    // Validate query
    if query.trim().is_empty() {
        debug!("Empty query detected, returning empty results");
        return Ok(Vec::new());
    }

    // Convert the query to an embedding
    debug!("Converting query to embedding vector");
    let query_embedding = model.embed(query)?;
    debug!("Generated embedding of dimension {}", query_embedding.len());

    // Use HNSW index for faster search if available
    let results: Vec<SearchResult> = if let Some(hnsw_index) = db.hnsw_index() {
        debug!("Using HNSW index for search (faster)");

        // Use more efficient HNSW search - need to use search_parallel since it doesn't require mutable reference
        // Set ef to HNSW_TOP_K * 2 for better recall
        let nearest =
            hnsw_index.search_parallel(&query_embedding, HNSW_TOP_K, HNSW_TOP_K * 2)?;
        debug!("HNSW search returned {} nearest neighbors", nearest.len());

        // Convert the node IDs to file paths AND convert distance to similarity
        let mut file_results = Vec::new();
        for (node_id, distance) in nearest { // Renamed similarity -> distance
            if let Some(file_path) = db.get_file_path(node_id) {
                let similarity = 1.0 - distance; // Calculate similarity
                file_results.push((file_path, similarity)); // Store owned String
            }
        }

        // --- Add uniqueness check here ---
        let mut unique_file_results_map: HashMap<String, f32> = HashMap::new();
        for (file_path, similarity) in file_results {
            // Keep the entry with the highest similarity if duplicates occur
            unique_file_results_map.entry(file_path)
                .and_modify(|existing_sim| *existing_sim = existing_sim.max(similarity)) // Keep max similarity
                .or_insert(similarity);
        }
        // --- End uniqueness check ---

        // Convert to SearchResult objects from the unique map
        unique_file_results_map
            .into_iter()
            .map(|(file_path, similarity)| SearchResult { // Now uses correct similarity
                file_path,
                similarity,
                snippet: String::new(), // Snippet generated later
            })
            .collect()
    } else {
        debug!("Using brute force search (slower)");

        // Fall back to brute force search
        let mut results: Vec<_> = db
            .embeddings
            .iter()
            .map(|(path, embedding)| {
                // Use the imported free function
                let distance = cosine_distance(embedding, &query_embedding);
                let similarity = 1.0 - distance;
                (path.clone(), similarity)
            })
            .collect();

        debug!("Brute force search returned {} results", results.len());

        // Sort by similarity (highest first)
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top K results (initial candidate pool)
        let top_k = results.len().min(HNSW_TOP_K);
        if results.len() > top_k {
            debug!("Limiting brute force results to top {}", top_k);
            results.truncate(top_k);
        }

        // Convert to SearchResult objects
        results
            .into_iter()
            .map(|(file_path, similarity)| SearchResult {
                file_path,
                similarity,
                snippet: String::new(), // Snippet generated later
            })
            .collect()
    };

    // --- Remove temporary debug ---
    debug!( // Restore original debug log
        "Raw results before filtering (len={}): {:?}",
        results.len(),
        results.iter().map(|r| (&r.file_path, r.similarity)).collect::<Vec<_>>()
    );
    // --- End log cleanup ---

    // Filter by similarity threshold
    let results_count = results.len();

    // Choose threshold based on query characteristics
    let query_lower = query.to_lowercase();
    let has_specialized_terms = ["ssh", "api", "http", "jwt", "cli", "gui", "tls", "ssl"]
        .iter()
        .any(|term| query_lower.contains(term));

    let threshold = if has_specialized_terms {
        // Use lower threshold for specialized queries
        SPECIALIZED_SEARCH_THRESHOLD
    } else {
        SIMILARITY_THRESHOLD
    };

    debug!("Filtering results with threshold {}", threshold);

    let filtered_results: Vec<_> = results
        .into_iter()
        .filter(|r| r.similarity >= threshold)
        .collect();

    debug!(
        "Filtered {} results below threshold, {} remaining",
        results_count - filtered_results.len(),
        filtered_results.len()
    );

    // Generate snippets for each result using the improved snippet extractor
    debug!("Generating snippets for {} results", filtered_results.len());
    let mut final_results = Vec::new();
    for mut result in filtered_results {
        // Use the improved snippet extractor
        match snippet_extractor.extract_snippet(&result.file_path, query) {
            Ok(snippet_context) => {
                debug!("Generated snippet for {}", result.file_path);
                result.snippet = snippet_context.snippet_text;
            }
            Err(e) => {
                warn!("Failed to generate snippet for {}: {}", result.file_path, e);

                // Fall back to original snippet generation method
                match get_snippet(&result.file_path, query) { // Call the fallback from snippet module
                    Ok(snippet) => {
                        result.snippet = snippet;
                    }
                    Err(e) => {
                        result.snippet = format!("Failed to read file: {}", e);
                    }
                }
            }
        }
        final_results.push(result);
    }

    // Apply final ranking
    debug!("Sorting final results by similarity");
    final_results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));

    // Apply result diversity (currently commented out)
    // let diverse_results = self.apply_mmr(final_results, 0.7, max_results);
    let diverse_results = final_results;

    // Always strictly limit to max_results, no exceptions
    let limited_results = if diverse_results.len() > max_results {
        diverse_results[0..max_results].to_vec()
    } else {
        diverse_results
    };

    debug!(
        "Vector search complete, returning {} results (limit was {})",
        limited_results.len(),
        max_results
    );

    // Add final length check before returning
    debug!("Final check: limited_results length = {}", limited_results.len());

    Ok(limited_results)
} 