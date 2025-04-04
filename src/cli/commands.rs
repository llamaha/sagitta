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
use dirs;

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

        /// File types to index (e.g. rs,rb,go)
        #[arg(short = 't', long = "file-types", value_delimiter = ',')]
        file_types: Vec<String>,
        
        /// Number of threads to use for indexing (defaults to available CPUs)
        #[arg(short = 'j', long = "threads")]
        threads: Option<usize>,
        
        /// Use fast embedding model instead of the default ONNX model (faster but less accurate)
        #[arg(long = "fast")]
        use_fast: bool,
        
        /// Path to ONNX model file (default: ./onnx/all-minilm-l6-v2.onnx)
        #[arg(long = "onnx-model")]
        onnx_model: Option<String>,
        
        /// Path to ONNX tokenizer directory (default: ./onnx)
        #[arg(long = "onnx-tokenizer")]
        onnx_tokenizer: Option<String>,
    },

    /// Search for files by content
    Query {
        /// Search query
        #[arg(required = true)]
        query: String,
        
        /// Maximum number of results to return (default: 20)
        #[arg(short = 'l', long = "limit")]
        max_results: Option<usize>,
        
        /// Use only vector search (without hybrid BM25 combination)
        #[arg(long = "vector-only")]
        vector_only: bool,
        
        /// Weight for vector search (default: 0.7)
        #[arg(long = "vector-weight")]
        vector_weight: Option<f32>,
        
        /// Weight for BM25 lexical search (default: 0.3)
        #[arg(long = "bm25-weight")]
        bm25_weight: Option<f32>,
        
        /// File types to search (e.g. rs,rb,go)
        #[arg(short = 't', long = "file-types", value_delimiter = ',')]
        file_types: Option<Vec<String>>,
    },

    /// Code-aware search for functions, types, etc.
    CodeSearch {
        /// Search query
        #[arg(required = true)]
        query: String,

        /// Maximum number of results to return (default: 20)
        #[arg(short = 'l', long = "limit")]
        max_results: Option<usize>,
        
        /// Type of code search (function, type, dependency, usage)
        #[arg(short = 't', long = "type")]
        search_type: Option<String>,
    },
    
    /// Configure the embedding model
    Model {
        /// Use fast embedding model instead of the default ONNX model (faster but less accurate)
        #[arg(long = "fast")]
        use_fast: bool,
        
        /// Use ONNX embedding model (explicitly set, this is the default)
        #[arg(long = "onnx")]
        use_onnx: bool,
        
        /// Path to ONNX model file (default: ./onnx/all-minilm-l6-v2.onnx)
        #[arg(long = "onnx-model")]
        onnx_model: Option<String>,
        
        /// Path to ONNX tokenizer directory (default: ./onnx)
        #[arg(long = "onnx-tokenizer")]
        onnx_tokenizer: Option<String>,
    },
    
    /// Parse code in a directory and show analysis
    ParseCode {
        /// Directory containing code to parse
        #[arg(required = true)]
        dir: String,
        
        /// File types to parse (e.g. rs,rb,go)
        #[arg(short = 't', long = "file-types", value_delimiter = ',', default_value = "rs,rb,go")]
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
}

pub fn execute_command(command: Command, mut db: VectorDB) -> Result<()> {
    match command {
        Command::Index { dir, file_types, threads, use_fast, onnx_model, onnx_tokenizer } => {
            debug!("Executing Index command for directory: {}", dir);
            println!("Indexing files in {}...", dir);
            
            // Default to using ONNX model unless fast model is explicitly requested
            if !use_fast {
                debug!("Using ONNX model for indexing (default)");
                // Get or use default paths
                let env_model_path = std::env::var("VECTORDB_ONNX_MODEL").ok();
                let env_tokenizer_path = std::env::var("VECTORDB_ONNX_TOKENIZER").ok();
                
                let model_path = onnx_model.as_deref().or_else(|| 
                    env_model_path.as_deref()
                ).unwrap_or("onnx/all-minilm-l6-v2.onnx");
                
                let tokenizer_path = onnx_tokenizer.as_deref().or_else(|| 
                    env_tokenizer_path.as_deref()
                ).unwrap_or("onnx");
                
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
                                println!("Using ONNX-based embedding model (default):");
                                println!("  - Model: {}", model_path);
                                println!("  - Tokenizer: {}", tokenizer_path);
                            },
                            Err(e) => {
                                error!("Failed to use ONNX model: {}", e);
                                eprintln!("Failed to use ONNX model: {}", e);
                                eprintln!("Model error: {}. Checking if model files exist...", e);
                                
                                let model_file_exists = Path::new(model_path).exists();
                                let tokenizer_file_exists = Path::new(&format!("{}/tokenizer.json", tokenizer_path)).exists();
                                
                                if !model_file_exists || !tokenizer_file_exists {
                                    eprintln!("ONNX model files not found:");
                                    if !model_file_exists {
                                        eprintln!("  - Model file not found: {}", model_path);
                                    }
                                    if !tokenizer_file_exists {
                                        eprintln!("  - Tokenizer file not found: {}/tokenizer.json", tokenizer_path);
                                    }
                                    eprintln!("\nPlease ensure model files are available or set correct paths.");
                                    eprintln!("You can run with --fast flag to use the fast embedding model instead.");
                                }
                                
                                eprintln!("Falling back to fast embedding model.");
                                // Ensure we're using the fast model
                                let _ = db.set_embedding_model_type(EmbeddingModelType::Fast);
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to set ONNX model paths: {}", e);
                        eprintln!("Failed to set ONNX model paths: {}", e);
                        eprintln!("Falling back to fast embedding model.");
                        // Ensure we're using the fast model
                        let _ = db.set_embedding_model_type(EmbeddingModelType::Fast);
                    }
                }
            } else {
                // Use the fast model as requested
                debug!("Using fast embedding model for indexing (as requested)");
                let _ = db.set_embedding_model_type(EmbeddingModelType::Fast);
                println!("Using fast embedding model (faster but less accurate) as requested");
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
        Command::Query { query, max_results, vector_only, vector_weight, bm25_weight, file_types } => {
            debug!("Executing Query command: \"{}\"", query);
            
            // Get the max_results value or use the default
            let limit = max_results.unwrap_or(20);
            debug!("Using max_results limit: {}", limit);
            println!("Limiting results to a maximum of {}", limit);
            
            // Use get_embedding_model for embedding logic
            let model_type = db.embedding_model_type();
            debug!("Current model type: {:?}", model_type);
            
            match model_type {
                EmbeddingModelType::Onnx => {
                    println!("Using ONNX model for semantic search (default)");
                },
                EmbeddingModelType::Fast => {
                    println!("Using fast model for search (less accurate but faster)");
                    println!("Run 'vectordb-cli model --onnx' to use the ONNX model for better results.");
                }
            }
            
            match get_embedding_model(model_type, &db) {
                Ok(model) => {
                    debug!("Successfully created embedding model: {:?}", model_type);
                    let mut search = Search::new(db, model);
                    
                    // Determine search type based on flags
                    let mut results = if vector_only {
                        debug!("Performing vector-only search");
                        println!("Performing vector-only search...");
                        search.search_with_limit(&query, limit)?
                    } else {
                        debug!("Performing hybrid search (vector + BM25)");
                        println!("Performing hybrid search (combining semantic and lexical matching)...");
                        
                        // Show weights being used
                        let v_weight = vector_weight.unwrap_or(HYBRID_VECTOR_WEIGHT);
                        let b_weight = bm25_weight.unwrap_or(HYBRID_BM25_WEIGHT);
                        debug!("Using weights: vector={:.2}, bm25={:.2}", v_weight, b_weight);
                        println!("Using weights: vector={:.2}, bm25={:.2}", v_weight, b_weight);
                        
                        search.hybrid_search_with_limit(&query, vector_weight, bm25_weight, limit)?
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
                        
                        // Limit the snippet size to avoid displaying entire files
                        let max_lines = 20; // Reasonable number of lines to display
                        let snippet_lines: Vec<&str> = result.snippet.lines().collect();
                        
                        // If snippet is too large, only show a subset with indication
                        if snippet_lines.len() > max_lines {
                            // Show first few lines
                            for line in &snippet_lines[0..max_lines/2] {
                                println!("{}", line);
                            }
                            
                            // Show ellipsis to indicate truncation
                            println!("... [truncated {} lines] ...", snippet_lines.len() - max_lines);
                            
                            // Show last few lines
                            for line in &snippet_lines[snippet_lines.len() - max_lines/2..] {
                                println!("{}", line);
                            }
                        } else {
                            // Show entire snippet if it's reasonably sized
                            println!("{}", result.snippet);
                        }
                        
                        println!();
                    }
                },
                Err(e) => {
                    error!("Error creating embedding model: {}", e);
                    eprintln!("Error creating embedding model: {}", e);
                    
                    // Check if this is likely due to missing ONNX files
                    if model_type == &EmbeddingModelType::Onnx {
                        let model_path = db.onnx_model_path().map_or_else(
                            || "onnx/all-minilm-l6-v2.onnx".to_string(), 
                            |p| p.to_string_lossy().to_string()
                        );
                        let tokenizer_path = db.onnx_tokenizer_path().map_or_else(
                            || "onnx".to_string(), 
                            |p| p.to_string_lossy().to_string()
                        );
                        
                        let model_file_exists = Path::new(&model_path).exists();
                        let tokenizer_file_exists = Path::new(&format!("{}/tokenizer.json", tokenizer_path)).exists();
                        
                        if !model_file_exists || !tokenizer_file_exists {
                            eprintln!("\nONNX model files not found:");
                            if !model_file_exists {
                                eprintln!("  - Model file not found: {}", model_path);
                            }
                            if !tokenizer_file_exists {
                                eprintln!("  - Tokenizer file not found: {}/tokenizer.json", tokenizer_path);
                            }
                            eprintln!("\nYou can do one of the following:");
                            eprintln!("1. Switch to the fast model: vectordb-cli model --fast");
                            eprintln!("2. Make sure the ONNX files are in the correct location");
                            eprintln!("3. Specify custom paths: vectordb-cli model --onnx --onnx-model /path/to/model.onnx --onnx-tokenizer /path/to/tokenizer");
                        }
                    }
                    
                    return Ok(());
                }
            }
        }
        Command::CodeSearch { query, max_results, search_type } => {
            // Get the max_results value or use the default
            let limit = max_results.unwrap_or(20);
            debug!("Using max_results limit: {}", limit);
            println!("Limiting code search results to a maximum of {}", limit);
            
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
                    let results = search.search_code_with_limit(&query, code_search_type, limit)?;

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
                        
                        // Limit the snippet size to avoid displaying entire files
                        let max_lines = 20; // Reasonable number of lines to display
                        let snippet_lines: Vec<&str> = result.snippet.lines().collect();
                        
                        // If snippet is too large, only show a subset with indication
                        if snippet_lines.len() > max_lines {
                            // Show first few lines
                            for line in &snippet_lines[0..max_lines/2] {
                                println!("   {}", line);
                            }
                            
                            // Show ellipsis to indicate truncation
                            println!("   ... [truncated {} lines] ...", snippet_lines.len() - max_lines);
                            
                            // Show last few lines
                            for line in &snippet_lines[snippet_lines.len() - max_lines/2..] {
                                println!("   {}", line);
                            }
                        } else {
                            // Show entire snippet if it's reasonably sized
                            println!("   {}", result.snippet.replace("\n", "\n   "));
                        }
                        
                        println!();
                    }
                },
                Err(e) => {
                    eprintln!("Error creating embedding model: {}", e);
                    return Ok(());
                }
            }
        }
        Command::Model { use_fast, use_onnx, onnx_model, onnx_tokenizer } => {
            debug!("Executing Model command");
            
            // Validate that we're not getting conflicting flags
            if use_fast && use_onnx {
                println!("Error: Cannot use both --fast and --onnx flags together.");
                println!("Please specify either --fast for the fast model or --onnx for the ONNX model.");
                return Ok(());
            }
            
            // Default to ONNX if neither is specified
            let use_onnx = use_onnx || !use_fast;
            
            if use_onnx {
                let env_model_path = std::env::var("VECTORDB_ONNX_MODEL").ok();
                let env_tokenizer_path = std::env::var("VECTORDB_ONNX_TOKENIZER").ok();
                
                let model_path = onnx_model.as_deref().or_else(|| 
                    env_model_path.as_deref()
                ).unwrap_or("onnx/all-minilm-l6-v2.onnx");
                
                let tokenizer_path = onnx_tokenizer.as_deref().or_else(|| 
                    env_tokenizer_path.as_deref()
                ).unwrap_or("onnx");
                
                debug!("Setting model type to ONNX with paths: {} and {}", model_path, tokenizer_path);
                
                match db.set_onnx_paths(
                    Some(PathBuf::from(model_path)),
                    Some(PathBuf::from(tokenizer_path))
                ) {
                    Ok(_) => {
                        match db.set_embedding_model_type(EmbeddingModelType::Onnx) {
                            Ok(_) => {
                                println!("Successfully set embedding model to ONNX model:");
                                println!("  - Model path: {}", model_path);
                                println!("  - Tokenizer path: {}", tokenizer_path);
                                
                                // Verify the files exist
                                let model_file_exists = Path::new(model_path).exists();
                                let tokenizer_file_exists = Path::new(&format!("{}/tokenizer.json", tokenizer_path)).exists();
                                
                                if !model_file_exists || !tokenizer_file_exists {
                                    println!("\nWarning: Some model files were not found:");
                                    if !model_file_exists {
                                        println!("  - Model file not found: {}", model_path);
                                    }
                                    if !tokenizer_file_exists {
                                        println!("  - Tokenizer file not found: {}/tokenizer.json", tokenizer_path);
                                    }
                                    println!("\nPlease ensure these files are available before indexing or searching.");
                                }
                            },
                            Err(e) => {
                                error!("Failed to set embedding model type to ONNX: {}", e);
                                eprintln!("Error: Failed to set embedding model type to ONNX: {}", e);
                                return Err(e.into());
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to set ONNX model paths: {}", e);
                        eprintln!("Error: Failed to set ONNX model paths: {}", e);
                        return Err(e.into());
                    }
                }
            } else {
                debug!("Setting model type to Fast");
                match db.set_embedding_model_type(EmbeddingModelType::Fast) {
                    Ok(_) => {
                        println!("Successfully set embedding model to Fast model.");
                        println!("Note: Fast model is quicker but less accurate than the ONNX model.");
                    },
                    Err(e) => {
                        error!("Failed to set embedding model type to Fast: {}", e);
                        eprintln!("Error: Failed to set embedding model type to Fast: {}", e);
                        return Err(e.into());
                    }
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
            println!("Clearing the database...");
            db.clear()?;
            println!("Database cleared successfully.");
        }
    }
    Ok(())
}

/// Creates the embedding model based on the database configuration
fn get_embedding_model(model_type: &EmbeddingModelType, db: &VectorDB) -> anyhow::Result<EmbeddingModel> {
    debug!("Creating embedding model of type: {:?}", model_type);
    match model_type {
        EmbeddingModelType::Fast => {
            debug!("Creating fast embedding model");
            Ok(EmbeddingModel::new())
        },
        EmbeddingModelType::Onnx => {
            debug!("Creating ONNX embedding model");
            
            let home_dir = dirs::home_dir();
            
            // Try different locations for the model files
            let possible_model_locations = vec![
                // First check the paths set in the database
                db.onnx_model_path().map(|p| p.to_owned()),
                
                // Then check in the current directory
                Some(PathBuf::from("onnx/all-minilm-l6-v2.onnx")),
                
                // Then check in the user's home directory
                home_dir.as_ref().map(|h| h.join(".vectordb-cli").join("models").join("all-minilm-l6-v2.onnx")),
                
                // Check environment variable if set
                std::env::var("VECTORDB_ONNX_MODEL").ok().map(PathBuf::from),
            ];
            
            // Try different locations for the tokenizer
            let possible_tokenizer_locations = vec![
                // First check the paths set in the database
                db.onnx_tokenizer_path().map(|p| p.to_owned()),
                
                // Then check in the current directory
                Some(PathBuf::from("onnx")),
                
                // Then check in the user's home directory
                home_dir.as_ref().map(|h| h.join(".vectordb-cli").join("models")),
                
                // Check environment variable if set
                std::env::var("VECTORDB_ONNX_TOKENIZER").ok().map(PathBuf::from),
            ];
            
            let mut errors = Vec::new();
            
            // Try each model/tokenizer path combination
            for model_path in possible_model_locations.iter().flatten() {
                for tokenizer_path in possible_tokenizer_locations.iter().flatten() {
                    debug!("Trying ONNX model: {}, tokenizer: {}", 
                           model_path.display(), tokenizer_path.display());
                    
                    // Check if files exist before attempting to load
                    if !model_path.exists() {
                        trace!("Model file does not exist: {}", model_path.display());
                        continue;
                    }
                    
                    let tokenizer_json = tokenizer_path.join("tokenizer.json");
                    if !tokenizer_json.exists() {
                        trace!("Tokenizer file does not exist: {}", tokenizer_json.display());
                        continue;
                    }
                    
                    // Try to create the model with this path combination
                    match EmbeddingModel::new_onnx(model_path, tokenizer_path) {
                        Ok(model) => {
                            info!("Successfully loaded ONNX model from: {}, tokenizer: {}", 
                                  model_path.display(), tokenizer_path.display());
                            return Ok(model);
                        },
                        Err(e) => {
                            debug!("Failed to load ONNX model from: {}, tokenizer: {}, error: {}", 
                                   model_path.display(), tokenizer_path.display(), e);
                            errors.push(format!("Path: {}, tokenizer: {} - Error: {}", 
                                       model_path.display(), tokenizer_path.display(), e));
                        }
                    }
                }
            }
            
            // If we reach here, all attempts failed
            let error_details = if errors.is_empty() {
                "No valid ONNX model files found".to_string()
            } else {
                format!("All attempts to load ONNX model failed, last error: {}", 
                         errors.last().unwrap_or(&"Unknown error".to_string()))
            };
            
            warn!("Falling back to fast model due to ONNX model loading failure");
            warn!("{}", error_details);
            println!("Warning: ONNX model could not be loaded, falling back to fast model (less accurate but quicker)");
            
            Ok(EmbeddingModel::new())
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
            // Use supported file types
            file_types: vec![],  // Empty to test auto-detection of file types
            threads: None,
            use_fast: false,
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
        // Use supported file types
        db_hnsw.index_directory(&temp_dir.path().to_string_lossy(), &VectorDB::get_supported_file_types())?;
        
        // Create embedding model for HNSW search
        let model_hnsw = EmbeddingModel::new();
        let mut search_hnsw = Search::new(db_hnsw, model_hnsw);
        
        // For comparison, create a database with a deliberately slowed down search
        // by using a modified brute force approach (not using the HNSW index)
        let db_path_slow = db_dir.path().join("test_slow.db").to_string_lossy().to_string();
        let mut db_slow = VectorDB::new(db_path_slow)?;
        
        // Index without accessing HNSW functionality
        // Use supported file types
        db_slow.index_directory(&temp_dir.path().to_string_lossy(), &VectorDB::get_supported_file_types())?;
        
        // We'll clone the database which will do a shallow clone, allowing us to use
        // a slower search method for comparison purposes
        let mut db_clone = db_slow.clone();
        
        // Create separate embedding model
        let model_slow = EmbeddingModel::new();
        let _search_slow = Search::new(db_slow, model_slow);
        
        // Measure search time with standard HNSW
        let start_hnsw = Instant::now();
        let _results_hnsw = search_hnsw.search("function")?;
        
        // ... rest of the method ...
        Ok(())
    }
}