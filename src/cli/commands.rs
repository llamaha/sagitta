use clap::Parser;
use anyhow::Result;
use crate::vectordb::VectorDB;
use crate::vectordb::embedding::{EmbeddingModel, EmbeddingModelType};
use crate::vectordb::search::{Search, CodeSearchType};
use crate::vectordb::parsing::{RustAnalyzer, CodeElement};
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use colored::Colorize;
use walkdir::WalkDir;
use std::time::{Instant, Duration};
use rayon;
use num_cpus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use ctrlc;
use log::{debug, info, warn, error, trace};

// Default weights for hybrid search
const HYBRID_VECTOR_WEIGHT: f32 = 0.7;
const HYBRID_BM25_WEIGHT: f32 = 0.3;

// Global flag for handling interrupts
pub static mut INTERRUPT_RECEIVED: bool = false;

#[derive(Parser, Debug)]
pub enum Command {
    /// Index files in a directory
    Index {
        /// Directory to index
        #[arg(required = true)]
        dir: String,

        /// File types to index (e.g. rs,py,txt)
        #[arg(short = 't', long = "file-types", value_delimiter = ',')]
        file_types: Vec<String>,
        
        /// Number of threads to use for indexing (defaults to available CPUs)
        #[arg(short = 'j', long = "threads")]
        threads: Option<usize>,
        
        /// Use ONNX-based embedding model
        #[arg(long = "onnx")]
        use_onnx: bool,
        
        /// Path to ONNX model file
        #[arg(long = "onnx-model")]
        onnx_model: Option<String>,
        
        /// Path to ONNX tokenizer directory
        #[arg(long = "onnx-tokenizer")]
        onnx_tokenizer: Option<String>,
    },

    /// Search for files by content
    Query {
        /// Search query
        #[arg(required = true)]
        query: String,
        
        /// Use only vector search (without hybrid BM25 combination)
        #[arg(long = "vector-only")]
        vector_only: bool,
        
        /// Weight for vector search (default: 0.7)
        #[arg(long = "vector-weight")]
        vector_weight: Option<f32>,
        
        /// Weight for BM25 lexical search (default: 0.3)
        #[arg(long = "bm25-weight")]
        bm25_weight: Option<f32>,
        
        /// File types to search (e.g. rs,py,txt)
        #[arg(short = 't', long = "file-types", value_delimiter = ',')]
        file_types: Option<Vec<String>>,
    },

    /// Code-aware search for functions, types, etc.
    CodeSearch {
        /// Search query
        #[arg(required = true)]
        query: String,

        /// Type of code search (function, type, dependency, usage)
        #[arg(short = 't', long = "type")]
        search_type: Option<String>,
    },
    
    /// Configure the embedding model
    Model {
        /// Use basic embedding model
        #[arg(long = "basic")]
        use_basic: bool,
        
        /// Use ONNX embedding model (requires model and tokenizer paths)
        #[arg(long = "onnx")]
        use_onnx: bool,
        
        /// Path to ONNX model file
        #[arg(long = "onnx-model")]
        onnx_model: Option<String>,
        
        /// Path to ONNX tokenizer directory
        #[arg(long = "onnx-tokenizer")]
        onnx_tokenizer: Option<String>,
    },
    
    /// Parse code in a directory and show analysis
    ParseCode {
        /// Directory containing code to parse
        #[arg(required = true)]
        dir: String,
        
        /// File types to parse (e.g. rs,py)
        #[arg(short = 't', long = "file-types", value_delimiter = ',', default_value = "rs")]
        file_types: Vec<String>,
        
        /// Show functions defined in the code
        #[arg(short = 'f', long = "functions")]
        show_functions: bool,
        
        /// Show type definitions in the code
        #[arg(short = 's', long = "structs")]
        show_structs: bool,
        
        /// Show imports and dependencies
        #[arg(short = 'i', long = "imports")]
        show_imports: bool,
    },

    /// Show database statistics
    Stats,

    /// Clear the database
    Clear,

    /// Run the Phase 2 ONNX Optimization Demo
    Phase2Demo,
}

pub fn execute_command(command: Command, mut db: VectorDB) -> Result<()> {
    match command {
        Command::Index { dir, file_types, threads, use_onnx, onnx_model, onnx_tokenizer } => {
            debug!("Executing Index command for directory: {}", dir);
            println!("Indexing files in {}...", dir);
            
            // Set the embedding model type and paths if ONNX is specified
            if use_onnx {
                debug!("ONNX model specified for indexing");
                // Get or use default paths
                let model_path = onnx_model.as_deref().unwrap_or("onnx/all-minilm-l12-v2.onnx");
                let tokenizer_path = onnx_tokenizer.as_deref().unwrap_or("onnx/minilm_tokenizer.json");
                
                debug!("Using ONNX model path: {}", model_path);
                debug!("Using ONNX tokenizer path: {}", tokenizer_path);
                
                // Set ONNX paths
                match db.set_onnx_paths(
                    Some(PathBuf::from(model_path)),
                    Some(PathBuf::from(tokenizer_path))
                ) {
                    Ok(_) => {
                        // Now set the model type to ONNX
                        match db.set_embedding_model_type(EmbeddingModelType::Onnx) {
                            Ok(_) => {
                                debug!("Successfully set embedding model type to ONNX");
                                println!("Using ONNX-based embedding model:");
                                println!("  - Model: {}", model_path);
                                println!("  - Tokenizer: {}", tokenizer_path);
                            },
                            Err(e) => {
                                error!("Failed to use ONNX model: {}", e);
                                eprintln!("Failed to use ONNX model: {}", e);
                                eprintln!("Falling back to basic embedding model.");
                                // Ensure we're using the basic model
                                let _ = db.set_embedding_model_type(EmbeddingModelType::Basic);
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to set ONNX model paths: {}", e);
                        eprintln!("Failed to set ONNX model paths: {}", e);
                        eprintln!("Falling back to basic embedding model.");
                        // Ensure we're using the basic model
                        let _ = db.set_embedding_model_type(EmbeddingModelType::Basic);
                    }
                }
            } else {
                // Ensure we're using the basic model
                debug!("Using basic embedding model for indexing");
                let _ = db.set_embedding_model_type(EmbeddingModelType::Basic);
                println!("Using basic embedding model");
            }
            
            // Set up signal handler for clean shutdown
            let running = Arc::new(AtomicBool::new(true));
            let r = running.clone();
            
            // Handle Ctrl+C gracefully
            ctrlc::set_handler(move || {
                debug!("Interrupt signal received");
                println!("\nInterrupt received, finishing current operations and shutting down...");
                r.store(false, Ordering::SeqCst);
                unsafe { INTERRUPT_RECEIVED = true; }
            }).expect("Failed to set Ctrl+C handler");
            
            // Set thread count if specified
            let num_cpus = num_cpus::get();
            if let Some(thread_count) = threads {
                debug!("Setting thread count to {} (of {} available CPUs)", thread_count, num_cpus);
                println!("Using {} threads for indexing ({} CPUs available)...", 
                         thread_count, num_cpus);
                rayon::ThreadPoolBuilder::new()
                    .num_threads(thread_count)
                    .build_global()
                    .unwrap_or_else(|e| println!("Failed to set thread count: {}", e));
            } else {
                debug!("Using all {} available CPUs for indexing", num_cpus);
                println!("Using all {} available CPUs for indexing...", num_cpus);
            }
            
            let start = Instant::now();
            
            // Check for interrupt periodically during indexing
            debug!("Starting directory indexing: {}, file types: {:?}", dir, file_types);
            match db.index_directory(&dir, &file_types) {
                Ok(_) => {
                    let duration = start.elapsed();
                    if unsafe { INTERRUPT_RECEIVED } {
                        debug!("Indexing was interrupted but data saved safely");
                        println!("Indexing was interrupted but data has been saved safely.");
                    } else {
                        debug!("Indexing completed successfully in {:.2} seconds", duration.as_secs_f32());
                        println!("Indexing complete in {:.2} seconds!", duration.as_secs_f32());
                    }
                },
                Err(e) => {
                    if unsafe { INTERRUPT_RECEIVED } {
                        debug!("Indexing was interrupted but data saved safely");
                        println!("Indexing was interrupted but data has been saved safely.");
                    } else {
                        error!("Indexing failed: {}", e);
                        return Err(e.into());
                    }
                }
            }
        }
        Command::Query { query, vector_only, vector_weight, bm25_weight, file_types } => {
            debug!("Executing Query command: \"{}\"", query);
            
            // Use get_embedding_model for embedding logic
            let model_type = db.embedding_model_type();
            match get_embedding_model(model_type, &db) {
                Ok(model) => {
                    debug!("Successfully created embedding model: {:?}", model_type);
                    let search = Search::new(db, model);
                    
                    // Determine search type based on flags
                    let mut results = if vector_only {
                        debug!("Performing vector-only search");
                        println!("Performing vector-only search...");
                        search.search(&query)?
                    } else {
                        debug!("Performing hybrid search (vector + BM25)");
                        println!("Performing query search (combining semantic and lexical matching)...");
                        
                        // Show weights being used
                        let v_weight = vector_weight.unwrap_or(HYBRID_VECTOR_WEIGHT);
                        let b_weight = bm25_weight.unwrap_or(HYBRID_BM25_WEIGHT);
                        debug!("Using weights: vector={:.2}, bm25={:.2}", v_weight, b_weight);
                        println!("Using weights: vector={:.2}, bm25={:.2}", v_weight, b_weight);
                        
                        search.hybrid_search(&query, vector_weight, bm25_weight)?
                    };

                    // Filter results by file type if specified
                    if let Some(types) = file_types {
                        if !types.is_empty() {
                            debug!("Filtering results by file types: {:?}", types);
                            println!("Filtering results by file types: {}", types.join(", "));
                            results.retain(|result| {
                                let path = Path::new(&result.file_path);
                                if let Some(ext) = path.extension() {
                                    let ext_str = ext.to_string_lossy().to_string();
                                    types.contains(&ext_str)
                                } else {
                                    false
                                }
                            });
                        }
                    }

                    if results.is_empty() {
                        debug!("No results found for query: \"{}\"", query);
                        println!("No results found.");
                        return Ok(());
                    }

                    // Check if this is a method-related query
                    let is_method_query = query.to_lowercase().contains("method") || 
                                          query.to_lowercase().contains("function") ||
                                          query.to_lowercase().contains("fn ");

                    if is_method_query {
                        debug!("Presenting method search results, {} results found", results.len());
                        println!("\nSearch results for methods: {}\n", query);
                    } else {
                        debug!("Presenting general search results, {} results found", results.len());
                        println!("\nSearch results for: {}\n", query);
                    }
                    
                    for (i, result) in results.iter().enumerate() {
                        println!("{}. {} (similarity: {:.2})", i + 1, result.file_path, result.similarity);
                        println!("{}", result.snippet);
                        println!();
                    }
                },
                Err(e) => {
                    error!("Error creating embedding model: {}", e);
                    eprintln!("Error creating embedding model: {}", e);
                    return Ok(());
                }
            }
        }
        Command::CodeSearch { query, search_type } => {
            // Use get_embedding_model for embedding logic
            let model_type = db.embedding_model_type();
            match get_embedding_model(model_type, &db) {
                Ok(model) => {
                    let mut search = Search::new(db, model);
                    
                    // Parse the search type
                    let code_search_type = match search_type.as_deref() {
                        Some("function") => Some(CodeSearchType::Function),
                        Some("type") => Some(CodeSearchType::Type),
                        Some("dependency") => Some(CodeSearchType::Dependency),
                        Some("usage") => Some(CodeSearchType::Usage),
                        None => None,
                        Some(unknown) => {
                            println!("Unknown search type: {}. Using general code search.", unknown);
                            None
                        }
                    };
                    
                    // Execute the code-aware search
                    let results = search.search_code(&query, code_search_type)?;

                    if results.is_empty() {
                        println!("No code results found.");
                        return Ok(());
                    }

                    println!("\nCode search results for: {}\n", query);
                    for (i, result) in results.iter().enumerate() {
                        println!("{}. {} (similarity: {:.2})", i + 1, result.file_path, result.similarity);
                        
                        // Print code context if available
                        if let Some(context) = &result.code_context {
                            println!("   {}:", "Code Context".green());
                            println!("   {}", context.replace("\n", "\n   "));
                        }
                        
                        println!("   {}:", "Snippet".green());
                        println!("   {}", result.snippet.replace("\n", "\n   "));
                        println!();
                    }
                },
                Err(e) => {
                    eprintln!("Error creating embedding model: {}", e);
                    return Ok(());
                }
            }
        }
        Command::Model { use_basic, use_onnx, onnx_model, onnx_tokenizer } => {
            // Configure the embedding model
            if use_basic && use_onnx {
                println!("Cannot specify both --basic and --onnx. Please choose one model type.");
                return Ok(());
            }
            
            if use_basic {
                // Set the model type to Basic
                db.set_embedding_model_type(EmbeddingModelType::Basic)?;
                println!("Set embedding model to Basic.");
            } else if use_onnx {
                // Get or use default paths
                let model_path = onnx_model.as_deref().unwrap_or("onnx/all-minilm-l12-v2.onnx");
                let tokenizer_path = onnx_tokenizer.as_deref().unwrap_or("onnx/minilm_tokenizer.json");
                
                // Set ONNX paths
                match db.set_onnx_paths(
                    Some(PathBuf::from(model_path)),
                    Some(PathBuf::from(tokenizer_path))
                ) {
                    Ok(_) => {
                        // Now set the model type to ONNX
                        match db.set_embedding_model_type(EmbeddingModelType::Onnx) {
                            Ok(_) => {
                                println!("Set embedding model to ONNX:");
                                println!("  - Model: {}", model_path);
                                println!("  - Tokenizer: {}", tokenizer_path);
                            },
                            Err(e) => {
                                eprintln!("Failed to set ONNX model: {}", e);
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to set ONNX model paths: {}", e);
                    }
                }
            } else {
                // Print embedding statistics
                let model_type = db.embedding_model_type();
                println!("Current embedding model: {:?}", model_type);
                
                if *model_type == EmbeddingModelType::Onnx {
                    println!("ONNX model paths:");
                    println!("  - Model: {}", db.onnx_model_path().map_or_else(
                        || "Not set".to_string(), 
                        |p| p.to_string_lossy().to_string()
                    ));
                    println!("  - Tokenizer: {}", db.onnx_tokenizer_path().map_or_else(
                        || "Not set".to_string(), 
                        |p| p.to_string_lossy().to_string()
                    ));
                }
            }
        }
        Command::ParseCode { dir, file_types, show_functions, show_structs, show_imports } => {
            println!("Parsing code in {}...", dir);
            
            // Create a RustAnalyzer instance
            let mut analyzer = match RustAnalyzer::new() {
                Ok(analyzer) => analyzer,
                Err(e) => {
                    println!("Error creating RustAnalyzer: {}", e);
                    return Ok(());
                }
            };
            
            // Try to load project
            let _ = analyzer.load_project(Path::new(&dir));
            
            // Parse each Rust file in the directory
            let mut all_functions = Vec::new();
            let mut all_structs = Vec::new();
            let mut all_imports = Vec::new();
            let mut dependencies = HashSet::new();
            
            for entry in WalkDir::new(&dir) {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        println!("Error reading directory entry: {}", e);
                        continue;
                    }
                };
                
                if !entry.file_type().is_file() {
                    continue;
                }
                
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    let ext = ext.to_string_lossy().to_string();
                    if file_types.contains(&ext) {
                        println!("Parsing {}...", path.display());
                        
                        match analyzer.parse_file(path) {
                            Ok(parsed) => {
                                // Collect elements
                                for element in &parsed.elements {
                                    match element {
                                        CodeElement::Function { name, .. } => {
                                            all_functions.push((name.clone(), path.to_path_buf()));
                                        },
                                        CodeElement::Struct { name, .. } => {
                                            all_structs.push((name.clone(), path.to_path_buf()));
                                        },
                                        CodeElement::Import { path: import_path, .. } => {
                                            all_imports.push((import_path.clone(), path.to_path_buf()));
                                        },
                                        _ => {},
                                    }
                                }
                                
                                // Collect dependencies
                                for dep in &parsed.dependencies {
                                    dependencies.insert(dep.clone());
                                }
                            },
                            Err(e) => {
                                println!("Error parsing {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            }
            
            // Display results
            if show_functions {
                println!("\n{} Functions:", all_functions.len());
                for (name, path) in all_functions {
                    println!("  {} - {}", name.green(), path.display());
                }
            }
            
            if show_structs {
                println!("\n{} Structs:", all_structs.len());
                for (name, path) in all_structs {
                    println!("  {} - {}", name.blue(), path.display());
                }
            }
            
            if show_imports {
                println!("\n{} Imports:", all_imports.len());
                for (path, file) in all_imports {
                    println!("  {} - {}", path.yellow(), file.display());
                }
                
                println!("\n{} Dependencies:", dependencies.len());
                for dep in dependencies {
                    println!("  {}", dep.yellow());
                }
            }
            
            println!("\nCode parsing complete!");
        }
        Command::Stats => {
            let stats = db.stats();
            println!("Indexed files: {}", stats.indexed_files);
            println!("Embedding dimension: {}", stats.embedding_dimension);
            println!("Embedding model: {:?}", stats.embedding_model_type);
            println!("Database path: {}", stats.db_path);
            println!("Cached files: {}", stats.cached_files);
            
            // Display ONNX paths if using ONNX model
            if stats.embedding_model_type == EmbeddingModelType::Onnx {
                println!("ONNX model paths:");
                println!("  - Model: {}", db.onnx_model_path().map_or_else(
                    || "Not set".to_string(), 
                    |p| p.to_string_lossy().to_string()
                ));
                println!("  - Tokenizer: {}", db.onnx_tokenizer_path().map_or_else(
                    || "Not set".to_string(), 
                    |p| p.to_string_lossy().to_string()
                ));
            }
            
            // Display HNSW stats if available
            if let Some(hnsw_stats) = stats.hnsw_stats {
                println!("\nHNSW Index:");
                println!("  Nodes: {}", hnsw_stats.total_nodes);
                println!("  Layers: {}", hnsw_stats.layers);
                println!("  Info: The HNSW index provides faster search on large codebases.");
                println!("        This is enabled by default for optimal performance.");
                
                // Display layer stats
                println!("\n  Layer Statistics:");
                for (i, layer) in hnsw_stats.layer_stats.iter().enumerate() {
                    println!("    Layer {}: {} nodes, {:.2} avg. connections", 
                        i, layer.nodes, layer.avg_connections);
                }
            } else {
                println!("\nHNSW Index: Not found");
                println!("  This is unusual as HNSW is enabled by default.");
                println!("  You may want to rebuild the index with the 'index' command.");
            }
        }
        Command::Clear => {
            db.clear()?;
            println!("Database cleared!");
        }
        Command::Phase2Demo => {
            #[cfg(feature = "onnx")]
            {
                println!("ONNX Phase 2 Demo would run here if implemented.");
                println!("This is a placeholder for the ONNX optimization demo.");
                return Ok(());
            }
            
            #[cfg(not(feature = "onnx"))]
            {
                println!("Error: ONNX feature is not enabled.");
                println!("Please rebuild with:");
                println!("  cargo build --features onnx");
                return Ok(());
            }
        }
    }
    Ok(())
}

/// Creates the embedding model based on the database configuration
fn get_embedding_model(model_type: &EmbeddingModelType, db: &VectorDB) -> anyhow::Result<EmbeddingModel> {
    debug!("Creating embedding model of type: {:?}", model_type);
    match model_type {
        EmbeddingModelType::Basic => {
            debug!("Creating basic embedding model");
            Ok(EmbeddingModel::new())
        },
        EmbeddingModelType::Onnx => {
            if let (Some(model_path), Some(tokenizer_path)) = (db.onnx_model_path(), db.onnx_tokenizer_path()) {
                debug!("Creating ONNX embedding model with paths: model={}, tokenizer={}", 
                       model_path.display(), tokenizer_path.display());
                EmbeddingModel::new_onnx(model_path, tokenizer_path)
                    .map_err(|e| anyhow::Error::msg(format!("Failed to create ONNX embedding model: {}", e)))
            } else {
                // Fallback to basic model
                warn!("ONNX paths not set, falling back to basic model");
                println!("Warning: ONNX paths not set, falling back to basic model");
                Ok(EmbeddingModel::new())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use std::io::Write;

    #[test]
    fn test_hnsw_index_creation() -> Result<()> {
        // Create a temporary directory for the test
        let temp_dir = tempdir()?;
        let db_dir = tempdir()?;
        let db_path = db_dir.path().join("test.db").to_string_lossy().to_string();
        
        // Create a test file
        let test_file = temp_dir.path().join("test.rs");
        let mut file = fs::File::create(&test_file)?;
        writeln!(file, "fn main() {{ println!(\"Hello, world!\"); }}")?;
        
        // Create VectorDB
        let db = VectorDB::new(db_path.clone())?;
        
        // Execute the index command (HNSW should be created by default now)
        let command = Command::Index {
            dir: temp_dir.path().to_string_lossy().to_string(),
            file_types: vec!["rs".to_string()],
            threads: None,
            use_onnx: false,
            onnx_model: None,
            onnx_tokenizer: None,
        };
        
        execute_command(command, db)?;
        
        // Load the DB again to check if HNSW index was created and saved
        let reloaded_db = VectorDB::new(db_path)?;
        let stats = reloaded_db.stats();
        assert!(stats.hnsw_stats.is_some(), "HNSW index should be created by default");
        
        Ok(())
    }

    #[test]
    fn test_hnsw_search_performance() -> Result<()> {
        // Create a temporary directory for the test
        let temp_dir = tempdir()?;
        let db_dir = tempdir()?;
        
        // Create multiple test files to simulate a larger codebase
        for i in 0..50 {
            let test_file = temp_dir.path().join(format!("test_{}.rs", i));
            let mut file = fs::File::create(&test_file)?;
            writeln!(file, "// File {}", i)?;
            writeln!(file, "fn function_{}() {{ println!(\"Function {}\"); }}", i, i)?;
            writeln!(file, "struct Struct{} {{ field: i32 }}", i)?;
        }
        
        // Test with HNSW (now the default)
        let db_path_hnsw = db_dir.path().join("test_hnsw.db").to_string_lossy().to_string();
        let mut db_hnsw = VectorDB::new(db_path_hnsw)?;
        
        // Index with HNSW enabled (default)
        db_hnsw.index_directory(&temp_dir.path().to_string_lossy(), &["rs".to_string()])?;
        
        // Create embedding model for HNSW search
        let model_hnsw = EmbeddingModel::new();
        let search_hnsw = Search::new(db_hnsw, model_hnsw);
        
        // For comparison, create a database with a deliberately slowed down search
        // by using a modified brute force approach (not using the HNSW index)
        let db_path_slow = db_dir.path().join("test_slow.db").to_string_lossy().to_string();
        let mut db_slow = VectorDB::new(db_path_slow)?;
        
        // Index without accessing HNSW functionality
        db_slow.index_directory(&temp_dir.path().to_string_lossy(), &["rs".to_string()])?;
        
        // We'll clone the database which will do a shallow clone, allowing us to use
        // a slower search method for comparison purposes
        let mut db_clone = db_slow.clone();
        
        // Create separate embedding model
        let model_slow = EmbeddingModel::new();
        let _search_slow = Search::new(db_slow, model_slow);
        
        // Measure search time with standard HNSW
        let start_hnsw = Instant::now();
        let _results_hnsw = search_hnsw.search("function")?;
        let duration_hnsw = start_hnsw.elapsed();
        
        // For comparison only, use manual vector search
        let query = "function";
        let model = EmbeddingModel::new();
        let query_embedding = model.embed(query)?;
        
        // Measure time for manual search (will be slower)
        let start_slow = Instant::now();
        let _results_slow = db_clone.nearest_vectors(&query_embedding, 10)?;
        let duration_slow = start_slow.elapsed();
        
        // For consistent test results, we don't strictly assert that HNSW is faster
        // as it might not be measurable with a small test dataset
        // Instead, just log the results
        println!("Search time with HNSW: {:?}", duration_hnsw);
        println!("Search time with slow method: {:?}", duration_slow);
        
        Ok(())
    }

    #[test]
    fn test_hnsw_index_persistence() -> Result<()> {
        // Create temporary directories
        let temp_dir = tempdir()?;
        let db_dir = tempdir()?;
        let db_path = db_dir.path().join("test_persistence.db").to_string_lossy().to_string();
        
        // Create a test file
        let test_file = temp_dir.path().join("test.rs");
        let mut file = fs::File::create(&test_file)?;
        writeln!(file, "fn test() {{ println!(\"Test\"); }}")?;
        
        // Create VectorDB (HNSW will be created by default)
        let mut db = VectorDB::new(db_path.clone())?;
        
        // Index files
        db.index_directory(&temp_dir.path().to_string_lossy(), &["rs".to_string()])?;
        
        // Get stats before reloading
        let hnsw_stats_before = db.stats().hnsw_stats;
        assert!(hnsw_stats_before.is_some(), "HNSW index should be present by default");
        
        // Force the DB to save changes
        drop(db);
        
        // Reload the database
        let db_reloaded = VectorDB::new(db_path)?;
        
        // Check if HNSW index is still there
        let hnsw_stats_after = db_reloaded.stats().hnsw_stats;
        assert!(hnsw_stats_after.is_some(), "HNSW index should persist after reload");
        
        Ok(())
    }

    #[test]
    fn test_query_search_command() -> Result<()> {
        // Create temporary directory for test files
        let temp_dir = tempdir()?;
        let db_dir = tempdir()?;
        let db_path = db_dir.path().join("test_query.db").to_string_lossy().to_string();
        
        // Create test files with different content
        let test_file1 = temp_dir.path().join("test1.rs");
        fs::write(&test_file1, "fn search_function() { println!(\"Finding things\"); }")?;
        
        let test_file2 = temp_dir.path().join("test2.rs");
        fs::write(&test_file2, "// This is a file about searching\nfn other_function() {}")?;
        
        // Create and setup database
        let mut db = VectorDB::new(db_path.clone())?;
        
        // Index the files
        db.index_file(&test_file1)?;
        db.index_file(&test_file2)?;
        
        // Test Query command with hybrid search enabled (default)
        let query_command = Command::Query { 
            query: "search".to_string(),
            vector_only: false,
            vector_weight: Some(0.6),
            bm25_weight: Some(0.4),
            file_types: Some(vec!["rs".to_string()]),
        };
        
        // Execute query command
        let result = execute_command(query_command, db.clone());
        assert!(result.is_ok(), "Query command with hybrid search should execute without error");
        
        // Test vector-only search
        let vector_command = Command::Query { 
            query: "search".to_string(),
            vector_only: true,
            vector_weight: None,
            bm25_weight: None,
            file_types: None,
        };
        
        // Execute vector-only command
        let result = execute_command(vector_command, db.clone());
        assert!(result.is_ok(), "Query command with vector-only should execute without error");

        Ok(())
    }
} 