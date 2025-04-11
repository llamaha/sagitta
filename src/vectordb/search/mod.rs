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
// use crate::vectordb::error::Result; // Import VectorDBError here // Removed
use crate::vectordb::snippet_extractor::SnippetExtractor;
use log::{warn};
use std::collections::HashSet;
use std::fs; // Re-add fs import
use std::path::Path;

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
}

impl Search {
    /// Creates a new Search instance.
    pub fn new(db: VectorDB, model: EmbeddingModel) -> Self {
        Self {
            db,
            model,
            snippet_extractor: SnippetExtractor::new(),
        }
    }

    /// Lists unique file types present in the database.
    pub fn list_file_types(&self) -> Vec<String> {
        let mut extensions = HashSet::new();
        // Iterate through indexed_chunks to get file paths
        for chunk in &self.db.indexed_chunks {
            if let Some(ext) = Path::new(&chunk.file_path).extension().and_then(|e| e.to_str()) {
                extensions.insert(ext.to_lowercase());
            }
        }
        extensions.into_iter().collect()
    }

    /// Lists unique top-level directories present in the database.
    pub fn list_indexed_dirs(&self) -> Vec<String> {
        let mut top_dirs = HashSet::new();
        for chunk in &self.db.indexed_chunks {
            if let Ok(abs_path) = fs::canonicalize(&chunk.file_path) {
                 if let Some(parent) = abs_path.parent() {
                     // Find the first ancestor directory that exists in the indexed_roots map
                     let mut current = parent;
                     loop {
                         if self.db.indexed_roots().contains_key(current.to_string_lossy().as_ref()) {
                             top_dirs.insert(current.to_string_lossy().into_owned());
                             break;
                         }
                         if let Some(p) = current.parent() {
                             current = p;
                         } else {
                             break; // Reached root without finding indexed root
                         }
                     }
                 }
            } else {
                 warn!("Could not canonicalize path {} during list_indexed_dirs", chunk.file_path);
            }
        }
        // Alternative: Directly return keys from db.indexed_roots() if that's desired?
        // return self.db.indexed_roots().keys().cloned().collect();
        top_dirs.into_iter().collect()
    }

    /// Retrieves all file paths from the database (used by hybrid search).
    fn get_all_file_paths(&self) -> Vec<String> {
        // Use indexed_chunks to get unique file paths
        self.db.indexed_chunks
            .iter()
            .map(|chunk| chunk.file_path.clone())
            .collect::<HashSet<_>>() // Collect into HashSet for uniqueness
            .into_iter()
            .collect() // Convert back to Vec
        // self.db.embeddings.keys().cloned().collect() // Old line
    }

    /// Standard search using vector similarity with a limit on the number of results.
    pub fn search_with_limit(
        &mut self,
        query: &str,
        max_results: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        vector::search_with_limit(
            &self.db,
            &mut self.model,
            query,
            max_results,
        )
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

        assert!(!results_alpha.is_empty(), "Should find results for 'alpha problem'");
        assert!(results_alpha[0].file_path.contains("_alpha.txt"), "Top result should be alpha");
        assert!(results_alpha.len() >= 1);

        let query_bravo = "bravo subject data processing";
        let results_bravo = search.search_with_limit(query_bravo, 1).unwrap();
        println!("Query: '{}', Results: {:?}", query_bravo, results_bravo.iter().map(|r| (&r.file_path, r.similarity)).collect::<Vec<_>>());
        assert_eq!(results_bravo.len(), 1, "Should find 1 result for 'bravo subject'");
        assert!(results_bravo[0].file_path.contains("file2_bravo.txt"));
    }
} 