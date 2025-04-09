use crate::vectordb::db::VectorDB;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::error::Result;
use crate::vectordb::snippet_extractor::SnippetExtractor;
use super::result::SearchResult;
use super::query_analysis::{self};
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
    db: &VectorDB, // Pass as ref
    model: &mut EmbeddingModel, // Pass as mut ref
    snippet_extractor: &mut SnippetExtractor, // Pass as mut ref
    bm25_index_opt: &Option<BM25Index>, // Pass as ref
    all_file_paths: Vec<String>, // <<< Change parameter type
    query: &str,
    vector_weight: Option<f32>,
    bm25_weight: Option<f32>,
    max_results: usize,
) -> Result<Vec<SearchResult>> {
    debug!("Performing hybrid search for query: {}", query);

    // If the query is empty, return empty results
    if query.trim().is_empty() {
        debug!("Empty query detected, returning empty results");
        return Ok(Vec::new());
    }

    // Analyze query for optimal weights
    let query_analysis = query_analysis::preprocess_query(query);
    debug!("Query analysis: {:?}", query_analysis);

    // Get weights (user-provided or determined automatically)
    let (v_weight, b_weight) = if vector_weight.is_some() || bm25_weight.is_some() {
        // Use user-provided weights if available
        let v = vector_weight.unwrap_or(HYBRID_VECTOR_WEIGHT);
        let b = bm25_weight.unwrap_or(HYBRID_BM25_WEIGHT);
        debug!(
            "Using user-provided weights: vector={:.2}, bm25={:.2}",
            v, b
        );
        (v, b)
    } else {
        // Otherwise determine automatically based on query analysis
        let (v, b) = query_analysis::determine_optimal_weights(
            query,
            &query_analysis,
            HYBRID_VECTOR_WEIGHT,
            HYBRID_BM25_WEIGHT,
        );
        debug!(
            "Using automatically determined weights: vector={:.2}, bm25={:.2}",
            v, b
        );
        (v, b)
    };

    // Set a larger search scope for internal queries
    let internal_limit = max_results * 3;

    // Perform vector search (semantic search part)
    debug!("Performing vector search component");
    // Call the vector search function from the vector module
    let vector_results = vector::search_with_limit(
        db, model, snippet_extractor, query, internal_limit
    )?;
    debug!("Vector search returned {} results: {:?}", vector_results.len(), vector_results.iter().map(|r| &r.file_path).collect::<Vec<_>>());

    // If we're only using vector search or BM25 index is unavailable, return vector results
    if b_weight <= 0.0 || bm25_index_opt.is_none() {
        if bm25_index_opt.is_none() && b_weight > 0.0 {
            warn!("BM25 weight > 0 but index is not available. Returning vector-only results.");
        }
        debug!("Returning vector-only results limited to max_results");
        let mut diverse_results = vector_results;
        // Apply limit here before returning
        if diverse_results.len() > max_results {
            diverse_results.truncate(max_results);
        }
        debug!("Final check (vector only): limited_results length = {}", diverse_results.len());
        return Ok(diverse_results);
    }

    let bm25_index = bm25_index_opt.as_ref().unwrap(); // Safe unwrap due to check above

    // Perform BM25 lexical search
    debug!("Performing BM25 lexical search component");
    let mut bm25_results = Vec::new();

    // Calculate BM25 scores for each file in the database
    let mut valid_files = 0;
    let total_files = all_file_paths.len();

    for file_path in all_file_paths {
        // Try to calculate BM25 score using the function from the bm25 module
        let score = match bm25::calculate_bm25_score(query, &file_path, bm25_index) {
            Ok(score) => score,
            Err(e) => {
                debug!("Failed to calculate BM25 score for {}: {}", file_path, e);
                continue;
            }
        };

        if score > 0.0 {
            valid_files += 1;
            bm25_results.push(SearchResult {
                file_path: file_path.to_string(),
                similarity: score,
                snippet: String::new(), // Snippet generated later
                repository: None, // Repository info added later if applicable
            });
        }
    }

    debug!(
        "BM25 search processed {}/{} files, found {} with matching content",
        valid_files,
        total_files,
        bm25_results.len()
    );

    // Sort BM25 results by score in descending order
    bm25_results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
    debug!("Sorted BM25 results (top {}): {:?}", bm25_results.len(), bm25_results.iter().map(|r| &r.file_path).collect::<Vec<_>>());

    // Keep only top results from BM25
    let top_k = internal_limit;
    if bm25_results.len() > top_k {
        debug!("Trimming BM25 results to top {}", top_k);
        bm25_results.truncate(top_k);
    }

    // Normalize BM25 scores before combining
    normalize_score_distribution(&mut bm25_results);

    // Normalize vector scores before combining
    let mut normalized_vector_results = vector_results; // Reuse the vector results vec
    normalize_score_distribution(&mut normalized_vector_results);

    // Combine vector and BM25 results using a HashMap to ensure uniqueness
    debug!("Combining vector and BM25 results using HashMap");
    let mut combined_results_map: HashMap<String, SearchResult> = HashMap::new();

    // Process normalized vector results
    for result in normalized_vector_results {
        let file_path = result.file_path.clone();
        let vector_score_norm = result.similarity;

        // Get normalized BM25 score for this file if available
        let bm25_score_norm = bm25_results
            .iter()
            .find(|r| r.file_path == file_path)
            .map(|r| r.similarity)
            .unwrap_or(0.0);

        // Combine normalized scores using weighted formula
        let combined_score = v_weight * vector_score_norm + b_weight * bm25_score_norm;

        // -- DEBUG LOGGING --
        // Optional: Log raw scores too if needed for comparison
        debug!(
            "Hybrid Score Debug ({}): vec_norm={:.4}, bm25_norm={:.4}, combined={:.4}",
            &file_path,
            vector_score_norm,
            bm25_score_norm,
            combined_score
        );
        // -- DEBUG LOGGING END --

        // Update the result similarity and insert/update in map
        let mut combined_result = result; // Take ownership
        combined_result.similarity = combined_score;
        combined_results_map.insert(file_path, combined_result);
    }

    // Add any BM25 results not already included
    for result in bm25_results {
        let file_path = result.file_path.clone();
        if !combined_results_map.contains_key(&file_path) {
            let bm25_score_norm = result.similarity;
            let combined_score = b_weight * bm25_score_norm;

            // Apply a threshold for BM25-only results
            let threshold = 0.1; // Lower threshold for normalized scores
            if combined_score >= threshold {
                let mut combined_result = result.clone(); // Clone result before modifying and inserting to fix borrow error
                combined_result.similarity = combined_score;

                // -- DEBUG LOGGING --
                debug!(
                    "Hybrid Score Debug (BM25 Only for {}): bm25_norm={:.4}, combined={:.4}",
                    &file_path,
                    bm25_score_norm,
                    combined_score
                );
                // -- DEBUG LOGGING END --

                let inserted_path = combined_result.file_path.clone(); // Clone path for logging
                combined_results_map.insert(file_path, combined_result);
                debug!(
                    "Added BM25-only result: {} with score {:.2} (threshold: {:.2})",
                    inserted_path, // Use cloned path for logging
                    combined_score, threshold
                );
            }
        }
    }

    // Convert map back to Vec
    let mut combined_results: Vec<SearchResult> = combined_results_map.into_values().collect();
    debug!("Combined results from map (len={}): {:?}", combined_results.len(), combined_results.iter().map(|r| &r.file_path).collect::<Vec<_>>());

    // Generate snippets for all results
    debug!(
        "Generating snippets for {} combined results",
        combined_results.len()
    );
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

    // Sort by combined score
    combined_results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));

    // Apply final diversity and limit the results (MMR commented out)
    debug!("Applying diversity and limiting results");
    let diverse_results = combined_results; // self.apply_mmr(combined_results, 0.6, max_results);

    // Strictly limit to max_results
    let limited_results = if diverse_results.len() > max_results {
        diverse_results[0..max_results].to_vec()
    } else {
        diverse_results
    };

    debug!(
        "Hybrid search complete, returning {} results (limit was {})",
        limited_results.len(),
        max_results
    );

    // Add final length check before returning
    debug!("Final check: limited_results length = {}", limited_results.len());

    Ok(limited_results)
}

/// Normalize the score distribution to spread out the scores more evenly
/// Uses rank-based normalization for scores clustered together, min-max otherwise.
pub(crate) fn normalize_score_distribution(results: &mut Vec<SearchResult>) {
    if results.len() <= 1 {
        return;
    }

    // Group results by repository if information is available
    let mut repo_groups: HashMap<String, Vec<usize>> = HashMap::new();
    let has_repo_info = results.iter().any(|r| r.repository.is_some());

    if has_repo_info {
        for (i, result) in results.iter().enumerate() {
            let repo_name = result.repository.as_deref().unwrap_or("unknown").to_string();
            repo_groups.entry(repo_name).or_default().push(i);
        }
    } else {
        // If no repo info, treat all results as a single group
        repo_groups.insert("all".to_string(), (0..results.len()).collect());
    }

    // Normalize within each group
    for indices in repo_groups.values() {
        if indices.len() <= 1 {
            continue;
        }

        let mut min_score = f32::INFINITY;
        let mut max_score = f32::NEG_INFINITY;
        for &idx in indices {
            min_score = min_score.min(results[idx].similarity);
            max_score = max_score.max(results[idx].similarity);
        }

        let score_range = max_score - min_score;

        // If scores are very close, use rank-based normalization
        if score_range < 0.01 { // Lower threshold for closer scores
            let mut group_results: Vec<(usize, f32)> = indices
                .iter()
                .map(|&idx| (idx, results[idx].similarity))
                .collect();
            group_results.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

            let result_count = group_results.len() as f32;
            for (i, (idx, _)) in group_results.iter().enumerate() {
                // Normalize to 0.5 - 1.0 range for better separation at the top
                let rank_score = 1.0 - (i as f32 / result_count) * 0.5;
                results[*idx].similarity = rank_score;
            }
        } else if score_range > 0.0 {
            // Apply standard min-max normalization, mapping to 0.0 - 1.0
            for &idx in indices {
                results[idx].similarity = (results[idx].similarity - min_score) / score_range;
            }
        }
        // If score_range is 0.0 (and not < 0.01), all scores are identical; leave them as is (likely 0 or 1).
    }

    // Optional: Add a small boost for exact repository matches if query contained repo name
    // (Requires passing query or analysis results here, omitted for simplicity)
} 