// examples/basic_usage.rs

use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;
use vectordb_lib::{VectorDB, VectorDBConfig};

fn main() -> Result<()> {
    println!("--- Running vectordb_lib basic usage example ---");

    // 1. Set up paths (using placeholders for models)
    //    - Create a temporary directory for the database and sample code.
    let temp_dir = tempdir()?;
    let db_path = temp_dir.path().join("example_db.json").to_string_lossy().to_string();
    let code_dir = temp_dir.path().join("sample_code");
    fs::create_dir(&code_dir)?;

    //    - Define placeholder paths for ONNX model and tokenizer.
    //      NOTE: Real applications need valid paths to actual model files.
    let placeholder_model_path = PathBuf::from("path/to/your/model.onnx");
    let placeholder_tokenizer_path = PathBuf::from("path/to/your/tokenizer.json");

    println!("Database path: {}", db_path);
    println!("Sample code directory: {}", code_dir.display());

    // 2. Create a sample code file
    let sample_file_path = code_dir.join("main.rs");
    let sample_code = r#"
        fn main() {
            println!("Hello, world!");
        }

        // A function to add two numbers
        fn add(a: i32, b: i32) -> i32 {
            a + b
        }
    "#;
    fs::write(&sample_file_path, sample_code)?;
    println!("Created sample file: {}", sample_file_path.display());

    // 3. Configure and create VectorDB
    //    NOTE: This step will likely succeed even with placeholder paths,
    //          but indexing/searching will fail if the paths are invalid.
    let config = VectorDBConfig {
        db_path,
        onnx_model_path: placeholder_model_path.clone(), // Use clone if needed later
        onnx_tokenizer_path: placeholder_tokenizer_path.clone(),
    };

    println!("Creating VectorDB instance...");
    let mut db = match VectorDB::new(config) {
        Ok(db_instance) => {
            println!("VectorDB created successfully.");
            db_instance
        }
        Err(e) => {
            eprintln!(
                "Error creating VectorDB (This is expected if model/tokenizer paths are placeholders): {}",
                e
            );
            println!("Exiting example early as DB creation failed.");
            // Early exit if DB creation fails (e.g., model path check)
            // Depending on implementation, path checks might be deferred, so we proceed cautiously
            return Ok(()); // Still return Ok for the example run itself
        }
    };

    // 4. Index the sample code directory
    println!("Indexing directory: {}...", code_dir.display());
    match db.index_directory(code_dir.to_str().unwrap(), &[]) {
        Ok(_) => println!("Directory indexing finished (may have skipped due to invalid model)."),
        Err(e) => {
            eprintln!("Error during indexing (expected if model is invalid): {}", e);
            // Don't exit yet, try searching anyway to show the API call
        }
    }

    // 5. Perform a search
    let query = "function to add numbers";
    println!("Performing search for: '{}'...", query);
    match db.search(query, 5, None) {
        Ok(results) => {
            println!("Search completed. Found {} results:", results.len());
            if results.is_empty() {
                println!("(No results found - this is expected if indexing failed or models are placeholders)");
            } else {
                for (i, result) in results.iter().enumerate() {
                    println!("  {}. Score: {:.4}", i + 1, result.score);
                    println!("     File: {}", result.file_path);
                    println!("     Lines: {} - {}", result.start_line, result.end_line);
                    println!("     Text Snippet:\n{}", result.text.trim());
                    println!("     ----------");
                }
            }
        }
        Err(e) => {
            eprintln!("Error during search (expected if model/index is invalid): {}", e);
        }
    }

    // Cleanup happens automatically when temp_dir goes out of scope
    println!("--- Example finished ---");
    Ok(())
} 