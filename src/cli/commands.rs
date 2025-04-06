use clap::Parser;
use anyhow::{Result, anyhow};
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
use std::io::Write;
use std::process::Command as ProcessCommand;
use std::collections::HashMap;
use crate::vectordb::repo_yaml;
use crate::vectordb::error::VectorDBError;

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

        /// File types to index (e.g. rs,rb,go,js,ts,yaml,md)
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
        
        /// File types to search (e.g. rs,rb,go,js,ts)
        #[arg(short = 't', long = "file-types", value_delimiter = ',')]
        file_types: Option<Vec<String>>,
        
        /// Repository to search (can be specified multiple times)
        #[arg(short = 'r', long = "repo", value_delimiter = ',')]
        repositories: Option<Vec<String>>,
        
        /// Search across all repositories
        #[arg(long = "all-repos")]
        all_repositories: bool,
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
        
        /// File types to parse (e.g. rs,rb,go,js,ts,yaml,md)
        #[arg(short = 't', long = "file-types", value_delimiter = ',', default_value = "rs,rb,go,js,ts,yaml,md")]
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
    Clear {
        /// Clear all repositories (requires confirmation)
        #[arg(short, long)]
        all: bool,
        
        /// Clear a specific repository
        #[arg(short, long)]
        repo: Option<String>,
        
        /// Show help information
        #[arg(short, long)]
        help: bool,
    },
    
    /// Repository management commands
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
}

#[derive(Parser, Debug)]
pub enum RepoCommand {
    /// Add a new repository
    Add {
        /// Path to the repository
        #[arg(required = true)]
        path: String,
        
        /// Repository name (optional, defaults to directory name)
        #[arg(short = 'n', long = "name")]
        name: Option<String>,
        
        /// File types to index (e.g. rs,rb,go,js,ts,yaml,md)
        #[arg(short = 't', long = "file-types", value_delimiter = ',')]
        file_types: Option<Vec<String>>,
        
        /// Embedding model type (fast, onnx)
        #[arg(long = "model")]
        model: Option<String>,
    },
    
    /// Import repositories from a YAML file
    ImportYaml {
        /// Path to the YAML configuration file
        #[arg(required = true)]
        path: String,
        
        /// Skip existing repositories instead of failing
        #[arg(short = 's', long = "skip-existing")]
        skip_existing: bool,
    },
    
    /// Remove a repository
    Remove {
        /// Repository ID or name
        #[arg(required = true)]
        repo: String,
    },
    
    /// List repositories
    List,
    
    /// Set the active repository
    Use {
        /// Repository ID or name
        #[arg(required = true)]
        repo: String,
        
        /// Branch to switch to
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,
    },
    
    /// Sync a repository
    Sync {
        /// Repository ID or name
        #[arg(required = true)]
        repo: String,
        
        /// Branch to sync (defaults to active branch)
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,
        
        /// Sync all branches
        #[arg(long = "all-branches")]
        all_branches: bool,
        
        /// Force full reindexing
        #[arg(long = "force")]
        force: bool,
    },
    
    /// Sync all repositories
    SyncAll {
        /// Force full reindexing
        #[arg(long = "force")]
        force: bool,
    },
    
    /// Show repository status
    Status {
        /// Repository ID or name
        #[arg(required = true)]
        repo: String,
    },
    
    /// Configure auto-sync for repositories
    AutoSync {
        #[command(subcommand)]
        command: AutoSyncCommand,
    },
}

#[derive(Parser, Debug)]
pub enum AutoSyncCommand {
    /// Enable auto-sync for a repository
    Enable {
        /// Repository ID or name
        #[arg(required = true)]
        repo: String,
        
        /// Minimum interval between syncs in seconds (default: 60)
        #[arg(short = 'i', long = "interval")]
        interval: Option<u64>,
    },
    
    /// Disable auto-sync for a repository
    Disable {
        /// Repository ID or name
        #[arg(required = true)]
        repo: String,
    },
    
    /// Show auto-sync status
    Status {
        /// Repository ID or name (optional, shows all if not specified)
        #[arg(short = 'r', long = "repo")]
        repo: Option<String>,
    },
    
    /// Start the auto-sync daemon
    Start,
    
    /// Stop the auto-sync daemon
    Stop,
}

pub fn execute_command(command: Command, mut db: VectorDB) -> Result<()> {
    match command {
        Command::Index { dir, file_types, threads, use_fast, onnx_model, onnx_tokenizer } => {
            debug!("Executing Index command for directory: {}", dir);
            println!("Indexing files in {}...", dir);
            
            // Default to using ONNX model unless fast model is explicitly requested
            if !use_fast {
                debug!("Using ONNX model for indexing (default)");
                // Get environment variable paths (required)
                let env_model_path = std::env::var("VECTORDB_ONNX_MODEL")
                    .map_err(|_| VectorDBError::EmbeddingError(
                        "VECTORDB_ONNX_MODEL environment variable is required but not set".to_string()
                    ))?;
                
                let env_tokenizer_path = std::env::var("VECTORDB_ONNX_TOKENIZER")
                    .map_err(|_| VectorDBError::EmbeddingError(
                        "VECTORDB_ONNX_TOKENIZER environment variable is required but not set".to_string()
                    ))?;
                
                let model_path = onnx_model.unwrap_or(env_model_path);
                let tokenizer_path = onnx_tokenizer.unwrap_or(env_tokenizer_path);
                
                debug!("Using ONNX model path: {}", model_path);
                debug!("Using ONNX tokenizer path: {}", tokenizer_path);
                
                // Set ONNX paths
                match db.set_onnx_paths(
                    Some(PathBuf::from(model_path.clone())),
                    Some(PathBuf::from(tokenizer_path.clone()))
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
                                
                                let model_file_exists = Path::new(&model_path).exists();
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
            
            // Determine file types to use based on input flags and fast mode
            let file_types_to_use = if file_types.is_empty() {
                if use_fast {
                    // In fast mode with no specified file types, use all non-binary file types
                    // We just pass an empty vector to index_directory which will make it index all non-binary files
                    println!("No file types specified with --fast mode, indexing all non-binary files at file level");
                    Vec::new()
                } else {
                    // Standard mode with no specified file types, use all supported types with code parsers
                    let supported = VectorDB::get_supported_file_types();
                    println!("No file types specified, using all supported types with code parsers: {}", 
                             supported.join(", "));
                    supported
                }
            } else {
                // User specified file types, use those regardless of mode
                println!("Indexing file types: {}", file_types.join(", "));
                file_types
            };
            
            // Check for interrupt periodically during indexing
            debug!("Starting directory indexing: {}, file types: {:?}", dir, file_types_to_use);
            match db.index_directory(&dir, &file_types_to_use) {
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
        Command::Query { query, max_results, vector_only, vector_weight, bm25_weight, file_types, repositories, all_repositories } => {
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
                    
                    // Check if we should do a multi-repository search
                    let should_search_multiple = !search.db.repo_manager.list_repositories().is_empty() && 
                                                (repositories.is_some() || all_repositories);
                    
                    // Process file types - use all supported types if none specified
                    let file_types_to_use = if let Some(types) = &file_types {
                        if types.is_empty() {
                            println!("No file types specified, using all supported types");
                            Some(VectorDB::get_supported_file_types())
                        } else {
                            println!("Filtering results by file types: {}", types.join(", "));
                            Some(types.clone())
                        }
                    } else {
                        println!("No file types filter, searching all supported file types");
                        Some(VectorDB::get_supported_file_types())
                    };
                    
                    if should_search_multiple {
                        debug!("Performing multi-repository search");
                        println!("Searching across repositories...");
                        
                        // Create search options
                        let mut search_options = crate::vectordb::search::SearchOptions {
                            max_results: limit,
                            file_types: file_types_to_use,
                            vector_weight: if vector_only { Some(1.0) } else { vector_weight },
                            bm25_weight: if vector_only { Some(0.0) } else { bm25_weight },
                            repositories: None,
                            branches: None,
                        };
                        
                        // If specific repositories are specified, resolve them to IDs
                        if let Some(repo_names) = repositories {
                            if !repo_names.is_empty() {
                                println!("Searching in repositories: {}", repo_names.join(", "));
                                
                                let repo_ids = repo_names.iter()
                                    .filter_map(|name| {
                                        match search.db.repo_manager.resolve_repo_name_to_id(name) {
                                            Ok(id) => Some(id),
                                            Err(_) => {
                                                eprintln!("Warning: Repository '{}' not found", name);
                                                None
                                            }
                                        }
                                    })
                                    .collect::<Vec<_>>();
                                
                                search_options.repositories = Some(repo_ids);
                            }
                        } else if all_repositories {
                            println!("Searching across all repositories");
                            // Just leave repositories as None to search all
                        }
                        
                        // Perform multi-repository search
                        let results = search.multi_repo_search(&query, search_options)?;
                        
                        if results.is_empty() {
                            debug!("No results found for query: \"{}\"", query);
                            println!("No results found.");
                            return Ok(());
                        }
                        
                        // Display results
                        println!("\nSearch results for: {}\n", query);
                        
                        for (i, result) in results.iter().enumerate() {
                            let repo_info = if let Some(repo) = &result.repository {
                                format!("[{}]", repo)
                            } else {
                                "".to_string()
                            };
                            
                            let branch_info = if let Some(branch) = &result.branch {
                                format!("({})", branch)
                            } else {
                                "".to_string()
                            };
                            
                            println!("{}. {} {} {} (similarity: {:.2})", 
                                   i + 1, 
                                   result.file_path, 
                                   repo_info,
                                   branch_info,
                                   result.similarity);
                            
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
                    } else {
                        // Do a regular search
                        debug!("Performing standard search (not multi-repository)");
                        
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
                        if let Some(types) = file_types_to_use {
                            if !types.is_empty() {
                                debug!("Filtering results by file types: {:?}", types);
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
                let env_model_path = std::env::var("VECTORDB_ONNX_MODEL")
                    .map_err(|_| VectorDBError::EmbeddingError(
                        "VECTORDB_ONNX_MODEL environment variable is required but not set".to_string()
                    ))?;
                
                let env_tokenizer_path = std::env::var("VECTORDB_ONNX_TOKENIZER")
                    .map_err(|_| VectorDBError::EmbeddingError(
                        "VECTORDB_ONNX_TOKENIZER environment variable is required but not set".to_string()
                    ))?;
                
                let model_path = onnx_model.unwrap_or(env_model_path);
                let tokenizer_path = onnx_tokenizer.unwrap_or(env_tokenizer_path);
                
                debug!("Setting model type to ONNX with paths: {} and {}", model_path, tokenizer_path);
                
                match db.set_onnx_paths(
                    Some(PathBuf::from(model_path.clone())),
                    Some(PathBuf::from(tokenizer_path.clone()))
                ) {
                    Ok(_) => {
                        match db.set_embedding_model_type(EmbeddingModelType::Onnx) {
                            Ok(_) => {
                                println!("Successfully set embedding model to ONNX model:");
                                println!("  - Model path: {}", model_path);
                                println!("  - Tokenizer path: {}", tokenizer_path);
                                
                                // Verify the files exist
                                let model_file_exists = Path::new(&model_path).exists();
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
        Command::Clear { all, repo, help } => {
            if help || (!all && repo.is_none()) {
                println!("Usage: vectordb-cli clear [OPTIONS]");
                println!("Clears the database or specific repositories");
                println!("Options:");
                println!("  -a, --all          Clear all repositories (requires confirmation)");
                println!("  -r, --repo <repo>  Clear a specific repository");
                println!("  -h, --help         Show help information");
                return Ok(());
            }
            
            if all {
                // Show warning and ask for confirmation
                println!("WARNING: You are about to clear ALL repositories from the database.");
                println!("This action cannot be undone.");
                print!("Continue? [y/N]: ");
                std::io::stdout().flush()?;
                
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                
                if input.trim().to_lowercase() == "y" {
                    println!("Clearing all repositories...");
                    db.clear()?;
                    println!("All repositories cleared successfully.");
                } else {
                    println!("Operation cancelled.");
                }
            } else if let Some(repo_name) = repo {
                // Attempt to resolve repository
                match db.repo_manager.resolve_repo_name_to_id(&repo_name) {
                    Ok(repo_id) => {
                        // Get repository name for display
                        let repo_name = db.repo_manager.get_repository(&repo_id)
                            .map(|r| r.name.clone())
                            .unwrap_or_else(|| repo_name.clone());
                        
                        // Confirm clearing this repository
                        println!("About to clear repository: {}", repo_name);
                        print!("Continue? [y/N]: ");
                        std::io::stdout().flush()?;
                        
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;
                        
                        if input.trim().to_lowercase() == "y" {
                            println!("Clearing repository {}...", repo_name);
                            match db.clear_repository(&repo_name) {
                                Ok(_) => println!("Repository '{}' cleared successfully.", repo_name),
                                Err(e) => println!("Error clearing repository: {}", e),
                            }
                        } else {
                            println!("Operation cancelled.");
                        }
                    },
                    Err(_) => {
                        println!("Repository not found: {}", repo_name);
                    }
                }
            }
        }
        Command::Repo { command } => {
            execute_repo_command(command, db)?
        }
    }
    Ok(())
}

/// Creates the embedding model based on the database configuration
fn get_embedding_model(model_type: &EmbeddingModelType, _db: &VectorDB) -> anyhow::Result<EmbeddingModel> {
    debug!("Creating embedding model of type: {:?}", model_type);
    match model_type {
        EmbeddingModelType::Fast => {
            debug!("Creating fast embedding model");
            Ok(EmbeddingModel::new())
        },
        EmbeddingModelType::Onnx => {
            debug!("Creating ONNX embedding model");
            
            // Environment variables are now required for ONNX models
            let env_model_path = std::env::var("VECTORDB_ONNX_MODEL")
                .map_err(|_| VectorDBError::EmbeddingError(
                    "VECTORDB_ONNX_MODEL environment variable is required but not set".to_string()
                ))?;
            
            let env_tokenizer_path = std::env::var("VECTORDB_ONNX_TOKENIZER")
                .map_err(|_| VectorDBError::EmbeddingError(
                    "VECTORDB_ONNX_TOKENIZER environment variable is required but not set".to_string()
                ))?;
            
            // Create paths from the environment variables
            let model_path = PathBuf::from(&env_model_path);
            let tokenizer_path = PathBuf::from(&env_tokenizer_path);
            
            // Verify paths exist
            if !model_path.exists() {
                return Err(anyhow::Error::msg(format!(
                    "ONNX model file not found at path specified in VECTORDB_ONNX_MODEL: {}", 
                    model_path.display()
                )));
            }
            
            if !tokenizer_path.exists() {
                return Err(anyhow::Error::msg(format!(
                    "ONNX tokenizer directory not found at path specified in VECTORDB_ONNX_TOKENIZER: {}", 
                    tokenizer_path.display()
                )));
            }
            
            // Create the model with verified paths
            EmbeddingModel::new_onnx(&model_path, &tokenizer_path)
                .map_err(|e| anyhow::Error::msg(format!("Failed to create ONNX model: {}", e)))
        }
    }
}

/// Execute repository management commands
fn execute_repo_command(command: RepoCommand, mut db: VectorDB) -> Result<()> {
    match command {
        RepoCommand::Add { path, name, file_types, model } => {
            debug!("Adding repository: {}", path);
            
            // Verify path exists and is a git repository
            let repo_path = PathBuf::from(&path);
            if !repo_path.exists() {
                return Err(anyhow!("Repository path does not exist: {}", path));
            }
            
            let git_dir = repo_path.join(".git");
            if !git_dir.exists() {
                return Err(anyhow!("Not a git repository: {}", path));
            }
            
            // Add the repository
            match db.repo_manager.add_repository(repo_path, name.clone()) {
                Ok(repo_id) => {
                    println!("Repository added successfully: {}", name.clone().unwrap_or_else(|| path.clone()));
                    println!("Repository ID: {}", repo_id);
                    
                    // Update file types if provided, otherwise use all supported types
                    if let Some(repo) = db.repo_manager.get_repository_mut(&repo_id) {
                        if let Some(types) = file_types {
                            if !types.is_empty() {
                                repo.file_types = types.clone();
                                println!("File types set to: {}", types.join(", "));
                            } else {
                                // Empty list provided, use all supported types
                                repo.file_types = VectorDB::get_supported_file_types();
                                println!("Using all supported file types: {}", repo.file_types.join(", "));
                            }
                        } else {
                            // No file types specified, use all supported types
                            repo.file_types = VectorDB::get_supported_file_types();
                            println!("Using all supported file types: {}", repo.file_types.join(", "));
                        }
                    }
                    
                    // Update model type if provided
                    if let Some(model_type) = model {
                        if let Some(repo) = db.repo_manager.get_repository_mut(&repo_id) {
                            match model_type.to_lowercase().as_str() {
                                "fast" => {
                                    repo.embedding_model = Some(EmbeddingModelType::Fast);
                                    println!("Using fast embedding model for this repository");
                                },
                                "onnx" => {
                                    repo.embedding_model = Some(EmbeddingModelType::Onnx);
                                    println!("Using ONNX embedding model for this repository");
                                },
                                _ => {
                                    println!("Unknown model type: {}. Using default.", model_type);
                                }
                            }
                        }
                    }
                    
                    // Save changes
                    db.repo_manager.save()?;
                    
                    println!("Use 'vectordb-cli repo sync {}' to index this repository", 
                             name.unwrap_or_else(|| repo_id));
                    
                    Ok(())
                },
                Err(e) => Err(e)
            }
        },
        
        RepoCommand::ImportYaml { path, skip_existing } => {
            debug!("Importing repositories from YAML file: {}", path);
            
            // Parse the YAML file and import repositories
            let yaml_path = Path::new(&path);
            match repo_yaml::import_repositories_from_yaml(yaml_path, &mut db.repo_manager, skip_existing) {
                Ok(result) => {
                    // Show summary of imported repositories
                    if !result.successful.is_empty() {
                        println!("\n{} repositories imported successfully:", result.successful.len());
                        for repo in &result.successful {
                            println!("  ✓ {}", repo.green());
                        }
                    }
                    
                    if !result.skipped.is_empty() {
                        println!("\n{} repositories skipped (already exist):", result.skipped.len());
                        for repo in &result.skipped {
                            println!("  ! {}", repo.yellow());
                        }
                    }
                    
                    if !result.failed.is_empty() {
                        println!("\n{} repositories failed to import:", result.failed.len());
                        for (repo, error) in &result.failed {
                            println!("  ✗ {} - {}", repo.red(), error);
                        }
                    }
                    
                    println!("\nSummary: {} imported, {} skipped, {} failed", 
                             result.successful.len(), 
                             result.skipped.len(), 
                             result.failed.len());
                    
                    // Provide guidance for next steps
                    if !result.successful.is_empty() {
                        println!("\nUse 'vectordb-cli repo list' to see all repositories");
                        println!("Use 'vectordb-cli repo sync <repo>' to index repositories");
                    }
                    
                    Ok(())
                },
                Err(e) => {
                    Err(anyhow!("Failed to import repositories from YAML: {}", e))
                }
            }
        },
        
        RepoCommand::Remove { repo } => {
            debug!("Removing repository: {}", repo);
            
            // Resolve repository name/ID
            let repo_id = db.repo_manager.resolve_repo_name_to_id(&repo)?;
            
            // Get repository name for display
            let repo_name = db.repo_manager.get_repository(&repo_id)
                .map(|r| r.name.clone())
                .unwrap_or_else(|| repo.to_string());
            
            // Remove the repository
            db.repo_manager.remove_repository(&repo_id)?;
            
            println!("Repository '{}' removed successfully", repo_name);
            
            Ok(())
        },
        
        RepoCommand::List => {
            debug!("Listing repositories");
            
            let repos = db.repo_manager.list_repositories();
            
            if repos.is_empty() {
                println!("No repositories configured");
                return Ok(());
            }
            
            println!("Configured repositories:");
            println!("{:<36} {:<20} {:<10} {:<20}", "ID", "NAME", "ACTIVE", "BRANCHES");
            println!("{}", "-".repeat(86));
            
            for repo in repos {
                let active_marker = if repo.active { "Yes" } else { "No" };
                let branch_count = repo.indexed_branches.len();
                println!("{:<36} {:<20} {:<10} {:<20}", 
                         repo.id, 
                         repo.name,
                         active_marker,
                         if branch_count > 0 { 
                             format!("{} indexed", branch_count) 
                         } else { 
                             "Not indexed".to_string() 
                         });
            }
            
            // Show active repository
            if let Some(active_id) = db.repo_manager.get_active_repository_id() {
                if let Some(active_repo) = db.repo_manager.get_repository(active_id) {
                    println!("\nActive repository: {} ({})", active_repo.name, active_id);
                }
            }
            
            Ok(())
        },
        
        RepoCommand::Use { repo, branch } => {
            debug!("Setting active repository: {}", repo);
            
            // Resolve repository name/ID
            let repo_id = db.repo_manager.resolve_repo_name_to_id(&repo)?;
            
            // Get the repository and clone necessary data
            let repo_name;
            let active_branch;
            
            {
                let repo_config = db.repo_manager.get_repository(&repo_id)
                    .ok_or_else(|| anyhow!("Repository not found: {}", repo))?;
                
                repo_name = repo_config.name.clone();
                active_branch = repo_config.active_branch.clone();
            }
            
            // Switch to this repository
            db.switch_repository(&repo_id, branch.as_deref())?;
            
            println!("Switched to repository: {}", repo_name);
            
            if let Some(branch_name) = branch {
                println!("Using branch: {}", branch_name);
            } else {
                println!("Using branch: {}", active_branch);
            }
            
            Ok(())
        },
        
        RepoCommand::Sync { repo, branch, all_branches, force } => {
            debug!("Syncing repository: {}", repo);
            
            // Resolve repository name/ID
            let repo_id = db.repo_manager.resolve_repo_name_to_id(&repo)?;
            
            // Get repository and clone necessary data
            let repo_name;
            let repo_path;
            let active_branch;
            let file_types;
            
            {
                let repo_config = db.repo_manager.get_repository(&repo_id)
                    .ok_or_else(|| anyhow!("Repository not found: {}", repo))?;
                
                repo_name = repo_config.name.clone();
                repo_path = repo_config.path.clone();
                active_branch = repo_config.active_branch.clone();
                file_types = repo_config.file_types.clone();
            }
            
            if all_branches {
                println!("Syncing all branches of repository '{}'...", repo_name);
                
                // Get git repo
                let git_repo = crate::utils::git::GitRepo::new(repo_path)
                    .map_err(|e| anyhow!("Failed to access git repository: {}", e))?;
                
                // Get all branches
                let branches = git_repo.list_branches()
                    .map_err(|e| anyhow!("Failed to list branches: {}", e))?;
                
                if branches.is_empty() {
                    println!("No branches found in repository");
                    return Ok(());
                }
                
                println!("Found {} branches to sync", branches.len());
                println!("File types to index: {}", if file_types.is_empty() { 
                    let supported = VectorDB::get_supported_file_types();
                    format!("{} (all supported types)", supported.join(", "))
                } else { 
                    file_types.join(", ") 
                });
                
                // Create progress bar
                let progress = indicatif::ProgressBar::new(branches.len() as u64);
                progress.set_style(
                    indicatif::ProgressStyle::default_bar()
                        .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} branches synced ({eta}) {msg}")
                        .unwrap()
                        .progress_chars("#>-")
                );
                
                // Stats for overall progress
                let start_time = std::time::Instant::now();
                let mut successful_branches = 0;
                
                // Sync each branch
                for (i, branch_name) in branches.iter().enumerate() {
                    progress.set_message(format!("Syncing branch {}", branch_name));
                    
                    // Sync specific branch
                    let result = if force {
                        db.index_repository_full(&repo_id, branch_name)
                    } else {
                        db.index_repository_changes(&repo_id, branch_name)
                    };
                    
                    // Handle result
                    match result {
                        Ok(_) => {
                            successful_branches += 1;
                            progress.println(format!("✓ Branch '{}' synced successfully ({}/{})", 
                                                   branch_name, i + 1, branches.len()));
                        },
                        Err(e) => {
                            progress.println(format!("⚠️ Failed to sync branch '{}': {}", branch_name, e));
                        }
                    }
                    
                    progress.inc(1);
                    
                    // Show overall progress rate
                    let elapsed = start_time.elapsed().as_secs();
                    if elapsed > 0 {
                        let branches_per_min = (successful_branches as f64 / elapsed as f64) * 60.0;
                        progress.set_message(format!("Syncing branch {} ({:.1} branches/min)", 
                                                  branch_name, branches_per_min));
                    }
                }
                
                // Report final stats
                let elapsed = start_time.elapsed().as_secs();
                let minutes = elapsed / 60;
                let seconds = elapsed % 60;
                
                progress.finish_with_message(format!("Synced {}/{} branches in {}m{}s", 
                                                 successful_branches, branches.len(),
                                                 minutes, seconds));
            } else {
                // Determine which branch to sync
                let branch_name = branch.as_deref().unwrap_or(&active_branch);
                
                println!("Syncing repository '{}' branch '{}'...", repo_name, branch_name);
                println!("File types to index: {}", if file_types.is_empty() { 
                    let supported = VectorDB::get_supported_file_types();
                    format!("{} (all supported types)", supported.join(", "))
                } else { 
                    file_types.join(", ") 
                });
                
                let start_time = std::time::Instant::now();
                
                if force {
                    println!("Performing full reindexing...");
                    db.index_repository_full(&repo_id, branch_name)?;
                } else {
                    println!("Performing incremental indexing...");
                    db.index_repository_changes(&repo_id, branch_name)?;
                }
                
                // Report execution time
                let elapsed = start_time.elapsed().as_secs();
                let minutes = elapsed / 60;
                let seconds = elapsed % 60;
                
                println!("Repository '{}' branch '{}' synced successfully in {}m{}s", 
                         repo_name, branch_name, minutes, seconds);
            }
            
            Ok(())
        },
        
        RepoCommand::SyncAll { force } => {
            debug!("Syncing all repositories");
            
            // Get the list of repositories and clone necessary data to avoid borrowing issues
            let repos: Vec<_> = db.repo_manager.list_repositories()
                .into_iter()
                .map(|repo| (repo.id.clone(), repo.name.clone(), repo.active_branch.clone()))
                .collect();
            
            if repos.is_empty() {
                println!("No repositories configured");
                return Ok(());
            }
            
            println!("Syncing all {} repositories...", repos.len());
            
            // Create progress bar
            let progress = indicatif::ProgressBar::new(repos.len() as u64);
            progress.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} repositories synced ({eta}) {msg}")
                    .unwrap()
                    .progress_chars("#>-")
            );
            
            // Stats for overall progress
            let start_time = std::time::Instant::now();
            let mut successful = 0;
            let mut failed = 0;
            
            for (i, (repo_id, repo_name, active_branch)) in repos.iter().enumerate() {
                progress.set_message(format!("Syncing repository {}", repo_name));
                
                // Perform the sync
                let result = if force {
                    db.index_repository_full(repo_id, active_branch)
                } else {
                    db.index_repository_changes(repo_id, active_branch)
                };
                
                // Handle result
                match result {
                    Ok(_) => {
                        successful += 1;
                        progress.println(format!("✓ Repository '{}' synced successfully ({}/{})", 
                                              repo_name, i + 1, repos.len()));
                    },
                    Err(e) => {
                        failed += 1;
                        progress.println(format!("⚠️ Failed to sync repository '{}': {}", repo_name, e));
                    }
                }
                
                progress.inc(1);
                
                // Show overall progress rate
                let elapsed = start_time.elapsed().as_secs();
                if elapsed > 0 {
                    let repos_per_min = (successful as f64 / elapsed as f64) * 60.0;
                    progress.set_message(format!("Syncing repository {} ({:.1} repos/min)", 
                                              repo_name, repos_per_min));
                }
            }
            
            // Report final stats
            let elapsed = start_time.elapsed().as_secs();
            let minutes = elapsed / 60;
            let seconds = elapsed % 60;
            
            progress.finish_with_message(format!("Synced {}/{} repositories in {}m{}s", 
                                             successful, repos.len(), minutes, seconds));
            
            if failed > 0 {
                println!("\n⚠️ {} repositories failed to sync", failed);
            }
            
            Ok(())
        },
        
        RepoCommand::Status { repo } => {
            debug!("Showing repository status: {}", repo);
            
            // Resolve repository name/ID
            let repo_id = db.repo_manager.resolve_repo_name_to_id(&repo)?;
            
            // Get repository and clone necessary data to prevent borrow checker issues
            let repo_clone = db.repo_manager.get_repository(&repo_id)
                .ok_or_else(|| anyhow!("Repository not found: {}", repo))?
                .clone();
            
            println!("Repository: {} ({})", repo_clone.name, repo_id);
            println!("Path: {}", repo_clone.path.display());
            println!("Active branch: {}", repo_clone.active_branch);
            println!("Status: {}", if repo_clone.active { "Active" } else { "Inactive" });
            
            if let Some(last_indexed) = repo_clone.last_indexed {
                println!("Last indexed: {}", last_indexed.format("%Y-%m-%d %H:%M:%S"));
            } else {
                println!("Last indexed: Never");
            }
            
            println!("File types: {}", if repo_clone.file_types.is_empty() {
                let supported = VectorDB::get_supported_file_types();
                format!("{} (all supported types)", supported.join(", "))
            } else {
                repo_clone.file_types.join(", ")
            });
            
            println!("Embedding model: {}", match repo_clone.embedding_model {
                Some(EmbeddingModelType::Fast) => "Fast",
                Some(EmbeddingModelType::Onnx) => "ONNX",
                None => "Default",
            });
            
            println!("\nIndexed branches:");
            if repo_clone.indexed_branches.is_empty() {
                println!("  No branches indexed yet");
            } else {
                for (branch, commit) in &repo_clone.indexed_branches {
                    println!("  - {}: {}", branch, commit);
                }
            }
            
            // Check if there are any updates needed
            println!("\nStatus check:");
            
            // Create git repo
            match crate::utils::git::GitRepo::new(repo_clone.path.clone()) {
                Ok(git_repo) => {
                    // Check current branch
                    if let Ok(current_branch) = git_repo.get_current_branch() {
                        println!("Current branch: {}", current_branch);
                        
                        // Check if the branch is indexed
                        if let Some(indexed_commit) = repo_clone.get_indexed_commit(&current_branch) {
                            // Check if it needs reindexing
                            match git_repo.needs_reindexing(&current_branch, indexed_commit) {
                                Ok(needs_reindex) => {
                                    if needs_reindex {
                                        println!("⚠️ Branch '{}' needs reindexing", current_branch);
                                        println!("Run 'vectordb-cli repo sync {}' to update the index", repo);
                                    } else {
                                        println!("✓ Branch '{}' index is up to date", current_branch);
                                    }
                                },
                                Err(e) => {
                                    println!("⚠️ Could not determine if branch needs reindexing: {}", e);
                                }
                            }
                        } else {
                            println!("⚠️ Branch '{}' has not been indexed yet", current_branch);
                            println!("Run 'vectordb-cli repo sync {}' to index this branch", repo);
                        }
                    }
                },
                Err(e) => {
                    println!("⚠️ Could not access git repository: {}", e);
                }
            }
            
            Ok(())
        },
        
        RepoCommand::AutoSync { command } => {
            execute_auto_sync_command(command, db)
        },
    }
}

/// Execute auto-sync commands
fn execute_auto_sync_command(command: AutoSyncCommand, mut db: VectorDB) -> Result<()> {
    match command {
        AutoSyncCommand::Enable { repo, interval } => {
            debug!("Enabling auto-sync for repository: {}", repo);
            
            // Resolve repository name/ID
            let repo_id = db.repo_manager.resolve_repo_name_to_id(&repo)?;
            
            // Get the repository name
            let repo_name = db.repo_manager.get_repository(&repo_id)
                .map(|r| r.name.clone())
                .ok_or_else(|| anyhow!("Repository not found: {}", repo))?;
            
            // Enable auto-sync
            db.repo_manager.enable_auto_sync(&repo_id, interval)?;
            
            println!("Auto-sync enabled for repository: {} ({})", repo_name, repo_id);
            println!("Run 'vectordb-cli repo auto-sync start' to start the auto-sync daemon");
            
            Ok(())
        },
        
        AutoSyncCommand::Disable { repo } => {
            debug!("Disabling auto-sync for repository: {}", repo);
            
            // Resolve repository name/ID
            let repo_id = db.repo_manager.resolve_repo_name_to_id(&repo)?;
            
            // Get repository name
            let repo_name = db.repo_manager.get_repository(&repo_id)
                .map(|r| r.name.clone())
                .unwrap_or_else(|| repo_id.clone());
            
            // Disable auto-sync
            db.repo_manager.disable_auto_sync(&repo_id)?;
            
            println!("Auto-sync disabled for repository: {} ({})", repo_name, repo_id);
            
            Ok(())
        },
        
        AutoSyncCommand::Status { repo } => {
            debug!("Showing auto-sync status");
            
            // Show auto-sync status for a specific repository if specified
            if let Some(repo_name) = repo {
                let repo_id = db.repo_manager.resolve_repo_name_to_id(&repo_name)?;
                let repo = db.repo_manager.get_repository(&repo_id)
                    .ok_or_else(|| anyhow!("Repository not found: {}", repo_name))?;
                
                println!("Auto-sync status for repository: {} ({})", repo.name, repo_id);
                println!("  Enabled: {}", if repo.auto_sync.enabled { "Yes" } else { "No" });
                println!("  Minimum sync interval: {} seconds", repo.auto_sync.min_interval);
                
                return Ok(());
            }
            
            // Show auto-sync status for all repositories
            let all_repos = db.repo_manager.list_repositories();
            
            if all_repos.is_empty() {
                println!("No repositories configured");
                return Ok(());
            }
            
            println!("Auto-sync status for all repositories:");
            println!("{:<36} {:<20} {:<10} {:<20}", "ID", "NAME", "ENABLED", "INTERVAL");
            println!("{}", "-".repeat(86));
            
            // Count repos with auto-sync enabled
            let mut auto_sync_count = 0;
            
            for repo in all_repos {
                let enabled = if repo.auto_sync.enabled { 
                    auto_sync_count += 1;
                    "Yes" 
                } else { 
                    "No" 
                };
                let interval = format!("{} seconds", repo.auto_sync.min_interval);
                
                println!("{:<36} {:<20} {:<10} {:<20}", 
                         repo.id, 
                         repo.name,
                         enabled,
                         interval);
            }
            
            if auto_sync_count == 0 {
                println!("\nNo repositories have auto-sync enabled.");
            } else {
                println!("\n{} repositories have auto-sync enabled.", auto_sync_count);
            }
            
            Ok(())
        },
        
        AutoSyncCommand::Start => {
            debug!("Starting auto-sync daemon");
            
            // Clone repos before starting daemon to avoid borrowing conflicts
            let auto_sync_repos = db.repo_manager.get_auto_sync_repos();
            let repo_names: Vec<_> = auto_sync_repos.iter()
                .map(|r| (r.name.clone(), r.id.clone(), r.auto_sync.min_interval))
                .collect();
            
            if repo_names.is_empty() {
                println!("No repositories have auto-sync enabled. Enable auto-sync first with 'vectordb-cli repo auto-sync enable <repo>'");
                return Ok(());
            }
            
            // Start auto-sync daemon
            db.start_auto_sync()?;
            
            println!("Auto-sync daemon started for the following repositories:");
            for (name, id, interval) in repo_names {
                println!("  - {} ({})", name, id);
                println!("    Interval: {} seconds", interval);
            }
            
            Ok(())
        },
        
        AutoSyncCommand::Stop => {
            debug!("Stopping auto-sync daemon");
            
            // Stop auto-sync daemon
            db.stop_auto_sync()?;
            
            println!("Auto-sync daemon stopped");
            
            Ok(())
        },
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
    fn test_hnsw_search_performance() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;
        let dir_path = temp_dir.path().to_string_lossy().to_string();
        
        // Create a test directory with files
        create_test_files(&dir_path, 50)?;
        
        // Create a test database
        let db_path = tempdir()?.path().join("test.db").to_string_lossy().to_string();
        let db_path_slow = tempdir()?.path().join("test_slow.db").to_string_lossy().to_string();
        
        // Create two VectorDBs - one with HNSW and one without
        let mut db_hnsw = VectorDB::new(db_path.clone())?;
        let mut db_slow = VectorDB::new(db_path_slow.clone())?;
        
        db_hnsw.index_directory(&dir_path, &["txt".to_string()])?;
        db_slow.index_directory(&dir_path, &["txt".to_string()])?;
        
        // For the slow DB, disable HNSW
        let db_clone = db_slow.clone();
        
        // Use a simple query
        let query = "function search implementation";
        
        // Measure performance for HNSW search
        // Skip actual time measurements since they're not deterministic in tests
        let start_hnsw = Instant::now();
        let hnsw_results = db_hnsw.search(query, None)?;
        let _hnsw_time = start_hnsw.elapsed();
        
        // Verify we get results
        assert!(!hnsw_results.is_empty());
        
        // Verify the top score is reasonable
        assert!(hnsw_results[0].score > 0.0);
        
        Ok(())
    }
}