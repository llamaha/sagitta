// Declare the modules within the search directory
// pub mod bm25; // Removed unused module
pub mod chunking;
// pub mod hybrid; // Removed unused module
pub mod query_analysis;
pub mod result; // Make result public so SearchResult can be used outside
// pub mod snippet; // Removed unused module
pub mod vector; // Make public

// Re-export the necessary public items
pub use result::SearchResult;

use crate::vectordb::db::VectorDB;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::error::Result;
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
}

impl Search {
    /// Creates a new Search instance.
    pub fn new(db: VectorDB) -> Result<Self> {
        // Use the embedding handler to create the model
        let model = db.embedding_handler().create_embedding_model()?;
        Ok(Self {
            db,
            model,
        })
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
    use super::Search; // Import Search from the parent module
    use crate::vectordb::db::VectorDB;
    use crate::VectorDBConfig;
    use tempfile::tempdir;
    use std::fs;
    use std::path::PathBuf;
    use log::warn; // Ensure warn is imported for setup_test_env

    // Helper to setup a test environment
    // Returns None if required ONNX files are missing
    fn setup_test_env() -> Option<(tempfile::TempDir, PathBuf, VectorDB)> {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("search_test_db.json");
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");

        // Return None if files are missing
        if !model_path.exists() || !tokenizer_path.exists() {
            warn!("Default ONNX model/tokenizer not found in ./onnx/. Skipping search test.");
            return None; 
        }
        
        let config = VectorDBConfig {
            db_path: db_path.to_str().unwrap().to_string(),
            onnx_model_path: model_path,
            onnx_tokenizer_path: tokenizer_path,
        };

        // Use expect here as failure after file check is unexpected
        let mut db = VectorDB::new(config).expect("Failed to create VectorDB for test after checking files");

        // Create dummy files for indexing
        let dir_path = temp_dir.path().join("test_files");
        fs::create_dir(&dir_path).unwrap();
        let file1_path = dir_path.join("file_alpha.txt");
        fs::write(&file1_path, "Alpha Bravo Charlie").unwrap();
        let file2_path = dir_path.join("file_rust.rs");
        fs::write(&file2_path, "fn main() { println!(\"bravo\"); }").unwrap();
        
        // Index the directory
        // Use expect here as failure after file check is unexpected
        db.index_directory(dir_path.to_str().unwrap(), &[])
            .expect("Failed to index test directory in setup_test_env");

        Some((temp_dir, dir_path, db))
    }

    #[test]
    fn test_search_struct_new() {
        let setup = setup_test_env();
        if setup.is_none() {
            println!("Skipping test_search_struct_new due to missing ONNX files.");
            return;
        }
        let (_temp_dir, _dir_path, db) = setup.unwrap();

        let search_instance = Search::new(db.clone()); // Clone db for the test
        assert!(search_instance.is_ok());
        let search = search_instance.unwrap();
        // Check if the model was created (basic check) - Need EmbeddingModelType
        use crate::vectordb::embedding::EmbeddingModelType;
        assert_eq!(search.model.model_type(), EmbeddingModelType::Onnx);
    }

    #[test]
    fn test_list_file_types() {
        let setup = setup_test_env();
        if setup.is_none() {
            println!("Skipping test_list_file_types due to missing ONNX files.");
            return;
        }
        let (_temp_dir, _dir_path, db) = setup.unwrap();
        let search = Search::new(db).expect("Failed to create Search instance");

        let mut file_types = search.list_file_types();
        file_types.sort(); // Sort for consistent assertion

        assert_eq!(file_types, vec!["rs", "txt"]);
    }

    #[test]
    fn test_list_indexed_dirs() {
        let setup = setup_test_env();
        if setup.is_none() {
            println!("Skipping test_list_indexed_dirs due to missing ONNX files.");
            return;
        }
        let (_temp_dir, dir_path, db) = setup.unwrap(); // Need dir_path
        let search = Search::new(db).expect("Failed to create Search instance");

        let mut indexed_dirs = search.list_indexed_dirs();
        indexed_dirs.sort(); // Sort for consistent assertion

        // Expecting the canonicalized path of the indexed directory
        let expected_dir = fs::canonicalize(&dir_path).unwrap().to_string_lossy().into_owned();

        assert_eq!(indexed_dirs.len(), 1);
        assert_eq!(indexed_dirs[0], expected_dir);
    }

    #[test]
    fn test_vector_db_search() {
        // Skip test if setup failed (ONNX files missing)
        let setup_result = setup_test_env();
        if setup_result.is_none() {
            println!("Skipping test_vector_db_search due to missing ONNX files.");
            return;
        }
        let (_temp_dir, _dir_path, db) = setup_result.unwrap(); 

        // --- Test cases --- 
        let query_alpha = "alpha implementation details";
        let results_alpha = match db.search(query_alpha, 3, None) { // Use db.search directly
            Ok(results) => results,
            Err(e) => panic!("Search for alpha failed: {}", e),
        };
        assert!(!results_alpha.is_empty(), "Should find results for alpha");
        // Add more assertions based on expected results

        let query_bravo = "bravo";
        let results_bravo = db.search(query_bravo, 1, None).expect("Search for bravo failed"); // Limit 1
        assert_eq!(results_bravo.len(), 1, "Should find exactly one result for bravo");
        assert!(results_bravo[0].file_path.contains("file_rust.rs"));

        let query_rust = "rust code example";
        let results_rust_only = db.search(query_rust, 5, Some(vec!["rs".to_string()])).expect("Search for rust with filter failed");
        assert!(!results_rust_only.is_empty(), "Should find rust results with filter");
        for result in results_rust_only {
            assert!(result.file_path.ends_with(".rs"), "Filtered result should be a .rs file");
        }

        let query_no_match = "nonexistent topic xyz";
        let results_no_match = db.search(query_no_match, 5, None).expect("Search for no match failed");
        assert!(results_no_match.is_empty(), "Should find no results for nonexistent topic");
    }
} 