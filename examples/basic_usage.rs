// examples/basic_usage.rs

/* // Comment out the entire example for now
use anyhow::Result;
use std::path::PathBuf;
use vectordb_lib::{VectorDB, VectorDBConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup configuration (replace with your actual paths)
    let config = VectorDBConfig {
        db_path: "./my_vectordb.json".to_string(), // Path to store database state
        onnx_model_path: PathBuf::from("onnx/all-minilm-l12-v2.onnx"),
        onnx_tokenizer_path: PathBuf::from("onnx/minilm_tokenizer.json"),
    };

    // Initialize the database
    let mut db = VectorDB::new(config)?;

    // --- Indexing ---
    let dir_to_index = "./src"; // Directory you want to index
    let file_types_to_index = vec!["rs".to_string()]; // Only index Rust files

    println!("Indexing directory: {}", dir_to_index);
    db.index_directory(dir_to_index, &file_types_to_index)?;
    println!("Indexing complete.");

    // --- Searching ---
    let query = "function to handle errors";
    let limit = 5; // Number of results to return

    println!("\nSearching for: '{}'", query);
    let results = db.search(query, limit, None)?; // No file type filter for search

    if results.is_empty() {
        println!("No results found.");
    } else {
        println!("Found {} results:", results.len());
        for (i, result) in results.iter().enumerate() {
            println!(
                " {}. {} (Lines {}-{}) Score: {:.4}",
                i + 1,
                result.file_path,
                result.start_line,
                result.end_line,
                result.score
            );
            println!("   ```\n{}
   ```", result.text);
        }
    }

    Ok(())
}
*/

fn main() {
    println!("Example commented out pending Qdrant migration.");
} 