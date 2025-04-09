use crate::vectordb::db::VectorDB;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::error::Result;
use crate::vectordb::snippet_extractor::SnippetExtractor;
use super::result::SearchResult;
use super::bm25::{self, BM25Index};
use super::vector;
use log::{debug, warn};
use std::collections::HashMap;
use std::fs;

// Constants for Hybrid Search
pub(crate) const HYBRID_VECTOR_WEIGHT: f32 = 0.6;
pub(crate) const HYBRID_BM25_WEIGHT: f32 = 0.4;

/// Hybrid search combining vector similarity and BM25 lexical matching with a limit on the number of results
pub(crate) fn hybrid_search_with_limit(
    db: &VectorDB,
    model: &mut EmbeddingModel,
    snippet_extractor: &mut SnippetExtractor,
    bm25_index_opt: &Option<BM25Index>,
    _all_file_paths: Vec<String>,
    query: &str,
    vector_weight: Option<f32>,
    bm25_weight: Option<f32>,
    max_results: usize,
) -> Result<Vec<SearchResult>> {
    debug!("Performing hybrid search for query: {}", query);

    if query.trim().is_empty() {
        debug!("Empty query detected, returning empty results");
        return Ok(Vec::new());
    }

    // Remove query analysis for weight determination
    // let query_analysis = query_analysis::preprocess_query(query);
    // debug!("Query analysis: {:?}", query_analysis);

    // Get weights: Use user-provided weights or the default constants.
    let mut v_weight = vector_weight.unwrap_or(HYBRID_VECTOR_WEIGHT);
    let mut b_weight = bm25_weight.unwrap_or(HYBRID_BM25_WEIGHT);

    // Optional: Normalize weights to sum to 1.0 if both are provided
    // Useful if user provides weights like 0.8 and 0.8 - we scale them down.
    if vector_weight.is_some() || bm25_weight.is_some() { 
        let total_weight = v_weight + b_weight;
        if total_weight > 1e-6 { // Avoid division by zero
            v_weight /= total_weight;
            b_weight /= total_weight;
            debug!("Using user-provided or default weights (normalized): vector={:.2}, bm25={:.2}", v_weight, b_weight);
        } else {
             // Handle case where both weights are zero or negative (use defaults instead?)
             warn!("Sum of provided weights is zero or negative, using defaults.");
             v_weight = HYBRID_VECTOR_WEIGHT;
             b_weight = HYBRID_BM25_WEIGHT;
             debug!("Using default weights: vector={:.2}, bm25={:.2}", v_weight, b_weight);
        }
    } else {
        // If no weights provided, defaults are already normalized (0.6+0.4=1.0)
        debug!("Using default weights: vector={:.2}, bm25={:.2}", v_weight, b_weight);
    }

    // Set a larger search scope for internal queries
    let internal_limit = max_results * 3;

    // Perform vector search (semantic search part)
    debug!("Performing vector search component");
    let vector_results = vector::search_with_limit(
        db, model, snippet_extractor, query, internal_limit
    )?;
    debug!("Vector search returned {} results", vector_results.len());

    // If BM25 weight is effectively zero or index is missing, return vector results
    if b_weight < 1e-6 || bm25_index_opt.is_none() { // Use epsilon for float comparison
        if bm25_index_opt.is_none() && b_weight > 0.0 {
            warn!("BM25 weight > 0 but index is not available. Returning vector-only results.");
        }
        debug!("Returning vector-only results limited to max_results");
        let mut diverse_results = vector_results;
        if diverse_results.len() > max_results {
            diverse_results.truncate(max_results);
        }
        return Ok(diverse_results);
    }

    let bm25_index = bm25_index_opt.as_ref().unwrap();

    // Perform BM25 lexical search for top K results
    debug!("Performing BM25 top-k search component (k={})", internal_limit);
    let bm25_top_k_results = bm25::search_bm25_top_k(query, bm25_index, internal_limit)?;

    // Convert BM25 top-k results to SearchResult format
    let mut bm25_results: Vec<SearchResult> = bm25_top_k_results
        .into_iter()
        .map(|(file_path, score)| SearchResult {
            file_path,
            similarity: score,
            snippet: String::new(), // Snippet generated later
        })
        .collect();

    debug!("BM25 top-k search returned {} candidates", bm25_results.len());

    // Normalize BM25 scores (already sorted by search_bm25_top_k)
    min_max_normalize(&mut bm25_results);

    // Normalize vector scores 
    let mut normalized_vector_results = vector_results; // Reuse vec
    min_max_normalize(&mut normalized_vector_results);

    // Combine vector and BM25 results using a HashMap
    debug!("Combining vector and BM25 results using HashMap");
    let mut combined_results_map: HashMap<String, SearchResult> = HashMap::new();

    // Process normalized vector results
    for result in normalized_vector_results {
        let file_path = result.file_path.clone();
        let vector_score_norm = result.similarity;

        // Get normalized BM25 score for this file if available
        let bm25_score_norm = bm25_results // Search the normalized top-k BM25 results
            .iter()
            .find(|r| r.file_path == file_path)
            .map(|r| r.similarity)
            .unwrap_or(0.0);

        // Combine normalized scores using weighted formula
        let combined_score = v_weight * vector_score_norm + b_weight * bm25_score_norm;

        debug!(
            "Hybrid Score Debug ({}): vec_norm={:.4}, bm25_norm={:.4}, combined={:.4}",
            &file_path, vector_score_norm, bm25_score_norm, combined_score
        );

        let mut combined_result = result;
        combined_result.similarity = combined_score;
        combined_results_map.insert(file_path, combined_result);
    }

    // Add BM25-only results
    for result in bm25_results {
        let file_path = result.file_path.clone();
        if !combined_results_map.contains_key(&file_path) {
            let bm25_score_norm = result.similarity;
            let combined_score = b_weight * bm25_score_norm;

            // Remove threshold check for BM25-only results
            let mut combined_result = result.clone();
            combined_result.similarity = combined_score;

            debug!(
                "Hybrid Score Debug (BM25 Only for {}): bm25_norm={:.4}, combined={:.4}",
                &file_path, bm25_score_norm, combined_score
            );

            let inserted_path = combined_result.file_path.clone();
            combined_results_map.insert(file_path, combined_result);
            debug!(
                "Added BM25-only result: {} with score {:.2}",
                inserted_path, combined_score
            );
        }
    }

    // Convert map back to Vec
    let mut combined_results: Vec<SearchResult> = combined_results_map.into_values().collect();
    debug!("Combined results size: {}", combined_results.len());

    // Generate snippets (only if not already generated by vector search)
    debug!("Generating snippets for combined results");
    for result in &mut combined_results {
        if result.snippet.is_empty() { // Only generate if not already present (e.g., from vector search)
            match snippet_extractor.extract_snippet(&result.file_path, query) {
                Ok(snippet_context) => {
                    result.snippet = snippet_context.snippet_text;
                }
                Err(e) => {
                    warn!("Hybrid: Failed to generate snippet for {}: {}", result.file_path, e);
                    // Fall back to reading the file directly if possible
                    if let Ok(content) = fs::read_to_string(&result.file_path) {
                        let lines: Vec<&str> = content.lines().take(10).collect();
                        result.snippet = lines.join("\n");
                        if content.lines().count() > 10 {
                            result.snippet += "\n... (truncated)";
                        }
                    } else {
                        result.snippet = "Failed to generate snippet".to_string();
                    }
                }
            }
        }
    }

    // Sort by final combined score
    combined_results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));

    // Limit to max_results
    debug!("Limiting final results to {}", max_results);
    let limited_results = if combined_results.len() > max_results {
        combined_results[0..max_results].to_vec()
    } else {
        combined_results
    };

    debug!("Hybrid search complete, returning {} results", limited_results.len());
    Ok(limited_results)
}

// --- Score Normalization --- 
// Replace normalize_score_distribution with min_max_normalize
pub(crate) fn min_max_normalize(results: &mut Vec<SearchResult>) {
    if results.is_empty() {
        return; // Nothing to normalize
    }

    let mut min_score = f32::INFINITY;
    let mut max_score = f32::NEG_INFINITY;

    // Find min and max scores in the current list
    for result in results.iter() {
        min_score = min_score.min(result.similarity);
        max_score = max_score.max(result.similarity);
    }

    let score_range = max_score - min_score;

    // Handle cases where range is zero or very small to avoid division by zero or instability
    if score_range < 1e-6 { // Use a small epsilon for comparison
        // All scores are (almost) identical. Assign a neutral value like 0.5 or leave as is.
        // Assigning 0.5 might be reasonable if these identical scores should contribute neutrally.
        // Let's assign 0.5 as a default for identical scores.
        debug!("Score range is zero or near-zero. Assigning 0.5 to all results.");
        for result in results.iter_mut() {
            result.similarity = 0.5; 
        }
        return;
    }

    // Apply min-max normalization: (score - min) / range
    for result in results.iter_mut() {
        result.similarity = (result.similarity - min_score) / score_range;
        // Ensure score is within [0.0, 1.0] due to potential floating point inaccuracies
        result.similarity = result.similarity.max(0.0).min(1.0);
    }
} 