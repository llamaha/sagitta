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

// --- BM25 Score Calculation Logic ---
pub(crate) fn calculate_bm25_score(
    query: &str,
    file_path: &str,
    bm25_index: &BM25Index,
) -> Result<f32> {
    // Get pre-calculated data for the document
    let doc_info = bm25_index.doc_data.get(file_path).ok_or_else(|| {
        VectorDBError::SearchError(format!(
            "BM25 data not found for document: {}",
            file_path
        ))
    })?;

    let doc_len = doc_info.length as f32;
    let avg_dl = bm25_index.avg_doc_length;

    // Tokenize the query (same simple method as index building)
    let query_tokens: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    let mut score: f32 = 0.0; // Explicitly typed

    for term in query_tokens {
        // Get term frequency in the document
        if let Some(tf) = doc_info.term_freqs.get(&term) {
            // Get IDF score for the term
            if let Some(idf_score) = bm25_index.idf.get(&term) {
                // Calculate BM25 term score
                let tf = *tf as f32;
                let numerator = tf * (BM25_K1 + 1.0);
                let denominator = tf + BM25_K1 * (1.0 - BM25_B + BM25_B * (doc_len / avg_dl));
                score += idf_score * (numerator / denominator);
            }
            // If term is not in IDF map, it means it wasn't in any indexed doc, score contribution is 0.
        }
        // If term is not in the document, score contribution is 0.
    }

    // Return the calculated score, ensuring it's non-negative
    Ok(score.max(0.0))
} 