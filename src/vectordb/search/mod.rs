// Declare the modules within the search directory
pub mod bm25;
pub mod chunking;
pub mod hybrid;
pub mod query_analysis;
pub mod result; // Make result public so SearchResult can be used outside
pub mod snippet; // Make snippet public
mod vector;

// Re-export the necessary public items
pub use result::SearchResult;

use crate::vectordb::db::VectorDB;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::error::Result; // Import VectorDBError here
use crate::vectordb::snippet_extractor::SnippetExtractor;
use bm25::{build_bm25_index, BM25Index}; // Keep this import
use log::{debug, warn};
 // Keep HashMap if needed by tests or Search struct
// Remove HashSet if not needed

// --- Removed Structs --- 
// Remove the duplicated struct definitions from here
// struct BM25DocumentData { ... }
// struct BM25Index { ... }
// struct QueryAnalysis { ... }
// enum QueryType { ... }
// --- End of Removed Structs ---

/// Main struct for performing searches.
pub struct Search {
    pub db: VectorDB,
    model: EmbeddingModel,
    snippet_extractor: SnippetExtractor,
    bm25_index: Option<BM25Index>, // This should now correctly refer to bm25::BM25Index
}

impl Search {
    /// Creates a new Search instance, building the BM25 index.
    pub fn new(db: VectorDB, model: EmbeddingModel) -> Self {
        let bm25_index = match build_bm25_index(&db) { // Uses bm25::build_bm25_index
            Ok(index) => {
                debug!(
                    "Successfully built BM25 index: {} docs, avg length {:.2}",
                    index.total_docs,
                    index.avg_doc_length
                );
                Some(index) // index is bm25::BM25Index
            }
            Err(e) => {
                warn!("Failed to build BM25 index: {}. BM25 scoring will be disabled.", e);
                None
            }
        };

        Self {
            db,
            model,
            snippet_extractor: SnippetExtractor::new(),
            bm25_index, // Stores Option<bm25::BM25Index>
        }
    }

    /// Standard search using vector similarity with a limit on the number of results.
    /// Delegates to the `vector::search_with_limit` function.
    pub fn search_with_limit(
        &mut self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>> {
        vector::search_with_limit(
            &self.db,
            &mut self.model,
            &mut self.snippet_extractor, // Pass as mutable
            query,
            max_results,
        )
    }

    /// Hybrid search combining vector similarity and BM25 lexical matching with a limit.
    /// Delegates to the `hybrid::hybrid_search_with_limit` function.
    pub fn hybrid_search_with_limit(
        &mut self,
        query: &str,
        vector_weight: Option<f32>,
        bm25_weight: Option<f32>,
        max_results: usize,
    ) -> Result<Vec<SearchResult>> {
        let file_paths = self.get_file_paths();

        hybrid::hybrid_search_with_limit(
            &self.db,
            &mut self.model,
            &mut self.snippet_extractor,
            &self.bm25_index,
            file_paths,
            query,
            vector_weight,
            bm25_weight,
            max_results,
        )
    }

    /// Helper function to get all file paths from the database embeddings.
    fn get_file_paths(&self) -> Vec<String> {
        self.db.embeddings.keys().cloned().collect()
    }
}

// --- Tests --- 
#[cfg(test)]
mod tests {
    // Keep imports as they are, they should work with the new structure
    use super::*; 
    use crate::vectordb::db::VectorDB;
    use tempfile::tempdir;
    use std::fs;
    use std::path::Path;
    use log::warn; // Ensure warn is imported for setup_test_env

    // Helper function to set up a test environment with indexed files
    fn setup_test_env() -> (tempfile::TempDir, VectorDB) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db.json");
        let db_path_str = db_path.to_str().unwrap().to_string();

        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        let mut db = VectorDB::new(db_path_str.clone()).unwrap();

        // Attempt to set default ONNX paths
        let default_model_path = Path::new("onnx/all-minilm-l12-v2.onnx");
        let default_tokenizer_path = Path::new("onnx/minilm_tokenizer.json");
        if default_model_path.exists() && default_tokenizer_path.exists() {
             if let Err(e) = db.set_onnx_paths(Some(default_model_path.to_path_buf()), Some(default_tokenizer_path.to_path_buf())) {
                warn!("Setup_test_env: Failed to set default ONNX paths: {}", e);
             }
        }

        // Create test files
        let files_data = vec![
            ("file1_alpha.txt", "Detailed Rust code snippet regarding alpha topic, contains specific implementation details."),
            ("file2_bravo.txt", "Python script focusing on the bravo subject matter, includes data processing functions."),
            ("file3_alpha.txt", "Another Rust example for the alpha problem, showcasing a different approach to the implementation."),
        ];

        for (filename, content) in files_data {
            let file_path = temp_dir.path().join(filename);
            fs::write(&file_path, content).unwrap();
        }

        // Index the directory containing the test files
        let file_patterns = vec!["txt".to_string()];
        db.index_directory(temp_dir.path().to_str().unwrap(), &file_patterns)
            .expect("Failed to index test directory in setup_test_env");

        (temp_dir, db)
    }

    #[test_log::test]
    fn test_vector_search() {
        let default_model_path = Path::new("onnx/all-minilm-l12-v2.onnx");
        let default_tokenizer_path = Path::new("onnx/minilm_tokenizer.json");
        if !default_model_path.exists() || !default_tokenizer_path.exists() {
            println!("Skipping test_vector_search because default ONNX model files aren't available in ./onnx/");
            return;
        }

        let (_temp_dir, db) = setup_test_env();
        let model = db.create_embedding_model().expect("Failed to create ONNX model in test_vector_search");
        let mut search = Search::new(db, model);

        let query_alpha = "alpha problem implementation";
        let results_alpha = search.search_with_limit(query_alpha, 3).unwrap();
        println!("Query: '{}', Results: {:?}", query_alpha, results_alpha.iter().map(|r| (&r.file_path, r.similarity)).collect::<Vec<_>>());

        assert!(results_alpha.len() >= 2, "Should find at least 2 results for 'alpha problem' (after threshold)");
        assert!(results_alpha[0].file_path.contains("_alpha.txt"));
        assert!(results_alpha[1].file_path.contains("_alpha.txt"));

        let query_bravo = "bravo subject data processing";
        let results_bravo = search.search_with_limit(query_bravo, 1).unwrap();
        println!("Query: '{}', Results: {:?}", query_bravo, results_bravo.iter().map(|r| (&r.file_path, r.similarity)).collect::<Vec<_>>());
        assert_eq!(results_bravo.len(), 1, "Should find 1 result for 'bravo subject'");
        assert!(results_bravo[0].file_path.contains("file2_bravo.txt"));
    }

    #[test_log::test]
    fn test_hybrid_search() {
        let default_model_path = Path::new("onnx/all-minilm-l12-v2.onnx");
        let default_tokenizer_path = Path::new("onnx/minilm_tokenizer.json");
        if !default_model_path.exists() || !default_tokenizer_path.exists() {
            println!("Skipping test_hybrid_search because default ONNX model files aren't available in ./onnx/");
            return;
        }

        let (_temp_dir, db) = setup_test_env();
        let model = db.create_embedding_model().expect("Failed to create ONNX model in test_hybrid_search");
        let mut search = Search::new(db, model);

        assert!(search.bm25_index.is_some(), "BM25 index should be built");
        // Check the actual index from bm25 module
        assert!(search.bm25_index.as_ref().unwrap().total_docs > 0, "BM25 index should have docs"); 

        // Case: Vector-dominant search
        let query = "topic A";
        let results_hybrid_vec_dom = search.hybrid_search_with_limit(query, Some(1.0), Some(0.0), 2).unwrap();
        println!("Query: '{}'", query);
        println!("Hybrid Results (1.0/0.0): {:?}", results_hybrid_vec_dom.iter().map(|r| (&r.file_path, r.similarity)).collect::<Vec<_>>());

        assert_eq!(results_hybrid_vec_dom.len(), 2, "Hybrid search (vector only) should return 2 results for query '{}'", query);
        assert!(results_hybrid_vec_dom[0].file_path.contains("_alpha.txt"));
        assert!(results_hybrid_vec_dom[1].file_path.contains("_alpha.txt"));
    }
} 