use crate::vectordb::db::VectorDB;
use crate::vectordb::error::{Result, VectorDBError};
use log::{debug, warn};
use std::collections::{HashMap, HashSet};
use std::fs;

// Constants for BM25
pub(crate) const BM25_K1: f32 = 1.5;
pub(crate) const BM25_B: f32 = 0.75;

/// Holds term frequencies and document length for BM25.
#[derive(Debug, Clone)]
pub(crate) struct BM25DocumentData {
    pub(crate) term_freqs: HashMap<String, usize>,
    pub(crate) length: usize,
}

/// Holds the precomputed BM25 index data.
#[derive(Debug, Clone)]
pub(crate) struct BM25Index {
    pub(crate) doc_data: HashMap<String, BM25DocumentData>, // file_path -> {term_freqs, length}
    pub(crate) idf: HashMap<String, f32>,                   // term -> idf_score
    pub(crate) avg_doc_length: f32,
    pub(crate) total_docs: usize,
}

// --- BM25 Index Building Logic ---
pub(crate) fn build_bm25_index(db: &VectorDB) -> Result<BM25Index> {
    debug!("Building BM25 index...");
    let mut doc_data = HashMap::new();
    let mut doc_freqs = HashMap::new(); // term -> count of docs containing term
    let mut total_length = 0;
    // Get paths from embeddings map, assuming these are the docs to index
    let file_paths: Vec<String> = db.embeddings.keys().cloned().collect();
    let total_docs = file_paths.len();

    if total_docs == 0 {
        debug!("No documents found, returning empty BM25 index.");
        return Ok(BM25Index {
            doc_data,
            idf: HashMap::new(),
            avg_doc_length: 0.0,
            total_docs: 0,
        });
    }

    for file_path in &file_paths {
        match fs::read_to_string(file_path) {
            Ok(content) => {
                // Simple tokenization: lowercase, split by whitespace
                let tokens: Vec<String> = content
                    .to_lowercase()
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();

                let doc_len = tokens.len();
                total_length += doc_len;

                let mut term_freqs = HashMap::new();
                let mut unique_terms = HashSet::new();

                for token in tokens {
                    *term_freqs.entry(token.clone()).or_insert(0) += 1;
                    unique_terms.insert(token);
                }

                // Update document frequencies (for IDF)
                for term in unique_terms {
                    *doc_freqs.entry(term).or_insert(0) += 1;
                }

                doc_data.insert(
                    file_path.clone(),
                    BM25DocumentData {
                        term_freqs,
                        length: doc_len,
                    },
                );
            }
            Err(e) => {
                // Log error but continue building index with available files
                warn!("Failed to read file {} for BM25 indexing: {}. Skipping.", file_path, e);
            }
        }
    }

    // Calculate IDF scores
    let mut idf = HashMap::new();
    let num_docs_f32 = total_docs as f32;
    for (term, freq) in doc_freqs {
        // IDF formula: log( (N - n + 0.5) / (n + 0.5) + 1 )
        // N = total number of documents
        // n = number of documents containing the term
        let idf_score = ((num_docs_f32 - freq as f32 + 0.5) / (freq as f32 + 0.5) + 1.0).ln();
        idf.insert(term, idf_score);
    }

    let avg_doc_length = if total_docs > 0 {
        total_length as f32 / total_docs as f32
    } else {
        0.0
    };

    debug!("BM25 index build complete. Docs: {}, Avg Len: {:.2}, Terms: {}",
           total_docs, avg_doc_length, idf.len());

    Ok(BM25Index {
        doc_data,
        idf,
        avg_doc_length,
        total_docs,
    })
}

// --- BM25 Score Calculation Logic (Internal Helper) ---
// Renamed to `calculate_single_doc_bm25_score` and kept internal
fn calculate_single_doc_bm25_score(
    query_tokens: &HashSet<String>, // Use HashSet for faster lookups
    file_path: &str,
    bm25_index: &BM25Index,
) -> Result<f32> {
    // Get pre-calculated data for the document
    let doc_info = bm25_index.doc_data.get(file_path).ok_or_else(|| {
        // Log error but return 0 score, as the search function will handle skipping
        warn!("BM25 data not found during scoring for document: {}", file_path);
        VectorDBError::SearchError(format!(
            "BM25 data inconsistency for document: {}",
            file_path
        ))
    })?;

    let doc_len = doc_info.length as f32;
    let avg_dl = bm25_index.avg_doc_length;

    let mut score: f32 = 0.0;

    // Iterate through query terms present in this document's term_freqs
    // This is faster than iterating through all query tokens again
    for (term, tf_val) in &doc_info.term_freqs {
        if query_tokens.contains(term) { // Check if this doc term is in the query
            if let Some(idf_score) = bm25_index.idf.get(term) {
                let tf = *tf_val as f32;
                let numerator = tf * (BM25_K1 + 1.0);
                let denominator = tf + BM25_K1 * (1.0 - BM25_B + BM25_B * (doc_len / avg_dl));
                if denominator > 0.0 { // Avoid division by zero
                   score += idf_score * (numerator / denominator);
                } 
            }
            // If term is not in IDF map, it means it wasn't in any indexed doc, score contribution is 0.
        }
    }

    // Return the calculated score, ensuring it's non-negative
    Ok(score.max(0.0))
}

// --- New BM25 Top-K Search Function ---
pub(crate) fn search_bm25_top_k(
    query: &str,
    bm25_index: &BM25Index,
    k: usize,
) -> Result<Vec<(String, f32)>> {
    if bm25_index.total_docs == 0 || bm25_index.avg_doc_length <= 0.0 { // Avoid division by zero if avg_doc_length is 0
        debug!("BM25 index is empty or invalid, returning empty results.");
        return Ok(Vec::new());
    }

    // Tokenize the query and store unique terms in a HashSet for quick lookups
    let query_tokens_set: HashSet<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    if query_tokens_set.is_empty() {
        debug!("Empty query after tokenization, returning empty BM25 results.");
        return Ok(Vec::new());
    }

    let mut results: Vec<(String, f32)> = Vec::with_capacity(bm25_index.total_docs);

    // Iterate through all documents in the index
    for (file_path, doc_info) in &bm25_index.doc_data {
        // --- Optimization: Check for term overlap before scoring --- 
        let has_overlap = query_tokens_set.iter().any(|term| doc_info.term_freqs.contains_key(term));
        
        if !has_overlap {
            // debug!("Skipping BM25 scoring for {} due to no term overlap", file_path);
            continue; // Skip scoring if no query terms are in this document
        }
        // --- End Optimization --- 

        // Calculate score only if there's overlap
        match calculate_single_doc_bm25_score(&query_tokens_set, file_path, bm25_index) {
            Ok(score) => {
                if score > 0.0 {
                    results.push((file_path.clone(), score));
                }
            }
            Err(e) => {
                // Log error but continue searching other documents
                warn!("Error calculating BM25 score for {}: {}. Skipping file.", file_path, e);
            }
        }
    }

    // Sort results by score (descending)
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Truncate to top k
    results.truncate(k);

    debug!("BM25 top-k search found {} results (k={})", results.len(), k);
    Ok(results)
} 