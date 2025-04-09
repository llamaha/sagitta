use crate::vectordb::embedding::{EmbeddingModel, EmbeddingModelType};
use crate::vectordb::error::VectorDBError;
use crate::vectordb::search::Search;
use crate::vectordb::VectorDB;
use anyhow::Result;
use clap::Parser;
use ctrlc;
use log::{debug, error};
use num_cpus;
use rayon;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

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

    /// Show database statistics
    Stats,

    /// Clear the database
    Clear {
        /// Show help information
        #[arg(short, long)]
        help: bool,
    },
}

pub fn execute_command(command: Command, mut db: VectorDB) -> Result<()> {
    match command {
        Command::Index {
            dir,
            file_types,
            threads,
            use_fast,
            onnx_model,
            onnx_tokenizer,
        } => {
            debug!("Executing Index command for directory: {}", dir);
            println!("Indexing files in {}...", dir);

            // Default to using ONNX model unless fast model is explicitly requested
            if !use_fast {
                debug!("Using ONNX model for indexing (default)");
                // Get environment variable paths (required)
                let env_model_path = std::env::var("VECTORDB_ONNX_MODEL").map_err(|_| {
                    VectorDBError::EmbeddingError(
                        "VECTORDB_ONNX_MODEL environment variable is required but not set"
                            .to_string(),
                    )
                })?;

                let env_tokenizer_path =
                    std::env::var("VECTORDB_ONNX_TOKENIZER").map_err(|_| {
                        VectorDBError::EmbeddingError(
                            "VECTORDB_ONNX_TOKENIZER environment variable is required but not set"
                                .to_string(),
                        )
                    })?;

                let model_path = onnx_model.unwrap_or(env_model_path);
                let tokenizer_path = onnx_tokenizer.unwrap_or(env_tokenizer_path);

                debug!("Using ONNX model path: {}", model_path);
                debug!("Using ONNX tokenizer path: {}", tokenizer_path);

                // Set ONNX paths
                match db.set_onnx_paths(
                    Some(PathBuf::from(model_path.clone())),
                    Some(PathBuf::from(tokenizer_path.clone())),
                ) {
                    Ok(_) => {
                        // Now set the model type to ONNX
                        match db.set_embedding_model_type(EmbeddingModelType::Onnx) {
                            Ok(_) => {
                                debug!("Successfully set embedding model type to ONNX");
                                println!("Using ONNX-based embedding model (default):");
                                println!("  - Model: {}", model_path);
                                println!("  - Tokenizer: {}", tokenizer_path);
                            }
                            Err(e) => {
                                error!("Failed to use ONNX model: {}", e);
                                eprintln!("Failed to use ONNX model: {}", e);
                                eprintln!("Model error: {}. Checking if model files exist...", e);

                                let model_file_exists = Path::new(&model_path).exists();
                                let tokenizer_file_exists =
                                    Path::new(&format!("{}/tokenizer.json", tokenizer_path))
                                        .exists();

                                if !model_file_exists || !tokenizer_file_exists {
                                    eprintln!("ONNX model files not found:");
                                    if !model_file_exists {
                                        eprintln!("  - Model file not found: {}", model_path);
                                    }
                                    if !tokenizer_file_exists {
                                        eprintln!(
                                            "  - Tokenizer file not found: {}/tokenizer.json",
                                            tokenizer_path
                                        );
                                    }
                                    eprintln!("\nPlease ensure model files are available or set correct paths.");
                                    eprintln!("You can run with --fast flag to use the fast embedding model instead.");
                                }

                                eprintln!("Falling back to fast embedding model.");
                                // Ensure we're using the fast model
                                let _ = db.set_embedding_model_type(EmbeddingModelType::Fast);
                            }
                        }
                    }
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
                unsafe {
                    INTERRUPT_RECEIVED = true;
                }
            })
            .expect("Failed to set Ctrl+C handler");

            // Set thread count if specified
            let num_cpus = num_cpus::get();
            if let Some(thread_count) = threads {
                debug!(
                    "Setting thread count to {} (of {} available CPUs)",
                    thread_count, num_cpus
                );
                println!(
                    "Using {} threads for indexing ({} CPUs available)...",
                    thread_count, num_cpus
                );
                let _ = rayon::ThreadPoolBuilder::new()
                    .num_threads(thread_count)
                    .build_global()
                    .map_err(|e| println!("Failed to set thread count: {}", e));
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
                    println!(
                        "No file types specified, using all supported types with code parsers: {}",
                        supported.join(", ")
                    );
                    supported
                }
            } else {
                // User specified file types, use those regardless of mode
                println!("Indexing file types: {}", file_types.join(", "));
                file_types
            };

            // Check for interrupt periodically during indexing
            debug!(
                "Starting directory indexing: {}, file types: {:?}",
                dir, file_types_to_use
            );
            match db.index_directory(&dir, &file_types_to_use) {
                Ok(_) => {
                    let duration = start.elapsed();
                    if unsafe { INTERRUPT_RECEIVED } {
                        debug!("Indexing was interrupted but data saved safely");
                        println!("Indexing was interrupted but data has been saved safely.");
                    } else {
                        debug!(
                            "Indexing completed successfully in {:.2} seconds",
                            duration.as_secs_f32()
                        );
                        println!(
                            "Indexing complete in {:.2} seconds!",
                            duration.as_secs_f32()
                        );
                    }
                }
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
        Command::Query {
            query,
            max_results,
            vector_only,
            vector_weight,
            bm25_weight,
            file_types,
        } => {
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
                }
                EmbeddingModelType::Fast => {
                    println!("Using fast model for search (less accurate but faster)");
                    println!(
                        "Run 'vectordb-cli model --onnx' to use the ONNX model for better results."
                    );
                }
            }

            match get_embedding_model(model_type, &db) {
                Ok(model) => {
                    debug!("Successfully created embedding model: {:?}", model_type);
                    let mut search = Search::new(db, model);

                    // Prepare file type filter
                    let file_types_to_use = if let Some(types) = file_types {
                        if types.is_empty() {
                            println!("No specific file types specified, searching all supported file types");
                            Some(VectorDB::get_supported_file_types())
                        } else {
                            println!("Filtering results by file types: {}", types.join(", "));
                            Some(types.clone())
                        }
                    } else {
                        println!("No file types filter, searching all supported file types");
                        Some(VectorDB::get_supported_file_types())
                    };

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
                        debug!(
                            "Using weights: vector={:.2}, bm25={:.2}",
                            v_weight, b_weight
                        );
                        println!(
                            "Using weights: vector={:.2}, bm25={:.2}",
                            v_weight, b_weight
                        );

                        search.hybrid_search_with_limit(
                            &query,
                            vector_weight,
                            bm25_weight,
                            limit,
                        )?
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
                    let is_method_query = query.to_lowercase().contains("method")
                        || query.to_lowercase().contains("function")
                        || query.to_lowercase().contains("fn ");

                    if is_method_query {
                        debug!(
                            "Presenting method search results, {} results found",
                            results.len()
                        );
                        println!("\nSearch results for methods: {}\n", query);
                    } else {
                        debug!(
                            "Presenting general search results, {} results found",
                            results.len()
                        );
                        println!("\nSearch results for: {}\n", query);
                    }

                    for (i, result) in results.iter().enumerate() {
                        println!(
                            "{}. {} (similarity: {:.2})",
                            i + 1,
                            result.file_path,
                            result.similarity
                        );

                        // Limit the snippet size to avoid displaying entire files
                        let max_lines = 20; // Reasonable number of lines to display
                        let snippet_lines: Vec<&str> = result.snippet.lines().collect();

                        // If snippet is too large, only show a subset with indication
                        if snippet_lines.len() > max_lines {
                            // Show first few lines
                            for line in &snippet_lines[0..max_lines / 2] {
                                println!("{}", line);
                            }

                            // Show ellipsis to indicate truncation
                            println!(
                                "... [truncated {} lines] ...",
                                snippet_lines.len() - max_lines
                            );

                            // Show last few lines
                            for line in &snippet_lines[snippet_lines.len() - max_lines / 2..] {
                                println!("{}", line);
                            }
                        } else {
                            // Show entire snippet if it's reasonably sized
                            println!("{}", result.snippet);
                        }

                        println!();
                    }
                }
                Err(e) => {
                    error!("Error creating embedding model: {}", e);
                    eprintln!("Error creating embedding model: {}", e);

                    // Check if this is likely due to missing ONNX files
                    if model_type == &EmbeddingModelType::Onnx {
                        let model_path = db.onnx_model_path().map_or_else(
                            || "onnx/all-minilm-l6-v2.onnx".to_string(),
                            |p| p.to_string_lossy().to_string(),
                        );
                        let tokenizer_path = db.onnx_tokenizer_path().map_or_else(
                            || "onnx".to_string(),
                            |p| p.to_string_lossy().to_string(),
                        );

                        let model_file_exists = Path::new(&model_path).exists();
                        let tokenizer_file_exists =
                            Path::new(&format!("{}/tokenizer.json", tokenizer_path)).exists();

                        if !model_file_exists || !tokenizer_file_exists {
                            eprintln!("\nONNX model files not found:");
                            if !model_file_exists {
                                eprintln!("  - Model file not found: {}", model_path);
                            }
                            if !tokenizer_file_exists {
                                eprintln!(
                                    "  - Tokenizer file not found: {}/tokenizer.json",
                                    tokenizer_path
                                );
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
        Command::Model {
            use_fast,
            use_onnx,
            onnx_model,
            onnx_tokenizer,
        } => {
            debug!("Executing Model command");

            // Validate that we're not getting conflicting flags
            if use_fast && use_onnx {
                println!("Error: Cannot use both --fast and --onnx flags together.");
                println!(
                    "Please specify either --fast for the fast model or --onnx for the ONNX model."
                );
                return Ok(());
            }

            // Default to ONNX if neither is specified
            let use_onnx = use_onnx || !use_fast;

            if use_onnx {
                let env_model_path = std::env::var("VECTORDB_ONNX_MODEL").map_err(|_| {
                    VectorDBError::EmbeddingError(
                        "VECTORDB_ONNX_MODEL environment variable is required but not set"
                            .to_string(),
                    )
                })?;

                let env_tokenizer_path =
                    std::env::var("VECTORDB_ONNX_TOKENIZER").map_err(|_| {
                        VectorDBError::EmbeddingError(
                            "VECTORDB_ONNX_TOKENIZER environment variable is required but not set"
                                .to_string(),
                        )
                    })?;

                let model_path = onnx_model.unwrap_or(env_model_path);
                let tokenizer_path = onnx_tokenizer.unwrap_or(env_tokenizer_path);

                debug!(
                    "Setting model type to ONNX with paths: {} and {}",
                    model_path, tokenizer_path
                );

                match db.set_onnx_paths(
                    Some(PathBuf::from(model_path.clone())),
                    Some(PathBuf::from(tokenizer_path.clone())),
                ) {
                    Ok(_) => {
                        match db.set_embedding_model_type(EmbeddingModelType::Onnx) {
                            Ok(_) => {
                                println!("Successfully set embedding model to ONNX model:");
                                println!("  - Model path: {}", model_path);
                                println!("  - Tokenizer path: {}", tokenizer_path);

                                // Verify the files exist
                                let model_file_exists = Path::new(&model_path).exists();
                                let tokenizer_file_exists =
                                    Path::new(&format!("{}/tokenizer.json", tokenizer_path))
                                        .exists();

                                if !model_file_exists || !tokenizer_file_exists {
                                    println!("\nWarning: Some model files were not found:");
                                    if !model_file_exists {
                                        println!("  - Model file not found: {}", model_path);
                                    }
                                    if !tokenizer_file_exists {
                                        println!(
                                            "  - Tokenizer file not found: {}/tokenizer.json",
                                            tokenizer_path
                                        );
                                    }
                                    println!("\nPlease ensure these files are available before indexing or searching.");
                                }
                            }
                            Err(e) => {
                                error!("Failed to set embedding model type to ONNX: {}", e);
                                eprintln!(
                                    "Error: Failed to set embedding model type to ONNX: {}",
                                    e
                                );
                                return Err(e.into());
                            }
                        }
                    }
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
                        println!(
                            "Note: Fast model is quicker but less accurate than the ONNX model."
                        );
                    }
                    Err(e) => {
                        error!("Failed to set embedding model type to Fast: {}", e);
                        eprintln!("Error: Failed to set embedding model type to Fast: {}", e);
                        return Err(e.into());
                    }
                }
            }
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
                println!(
                    "  - Model: {}",
                    db.onnx_model_path().map_or_else(
                        || "Not set".to_string(),
                        |p| p.to_string_lossy().to_string()
                    )
                );
                println!(
                    "  - Tokenizer: {}",
                    db.onnx_tokenizer_path().map_or_else(
                        || "Not set".to_string(),
                        |p| p.to_string_lossy().to_string()
                    )
                );
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
                    println!(
                        "    Layer {}: {} nodes, {:.2} avg. connections",
                        i, layer.nodes, layer.avg_connections
                    );
                }
            } else {
                println!("\nHNSW Index: Not found");
                println!("  This is unusual as HNSW is enabled by default.");
                println!("  You may want to rebuild the index with the 'index' command.");
            }
        }
        Command::Clear { help } => {
            if help {
                println!("Usage: vectordb-cli clear [OPTIONS]");
                println!("Clears the database");
                println!("Options:");
                println!("  -h, --help         Show help information");
                return Ok(());
            }

            // Show warning and ask for confirmation
            println!("WARNING: You are about to clear the database.");
            println!("This action cannot be undone.");
            print!("Continue? [y/N]: ");
            std::io::stdout().flush()?;

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            if input.trim().to_lowercase() == "y" {
                println!("Clearing database...");
                db.clear()?;
                println!("Database cleared successfully.");
            } else {
                println!("Operation cancelled.");
            }
        }
    }
    Ok(())
}

/// Creates the embedding model based on the database configuration
fn get_embedding_model(
    model_type: &EmbeddingModelType,
    _db: &VectorDB,
) -> anyhow::Result<EmbeddingModel> {
    debug!("Creating embedding model of type: {:?}", model_type);
    match model_type {
        EmbeddingModelType::Fast => {
            debug!("Creating fast embedding model");
            Ok(EmbeddingModel::new())
        }
        EmbeddingModelType::Onnx => {
            debug!("Creating ONNX embedding model");

            // Environment variables are now required for ONNX models
            let env_model_path = std::env::var("VECTORDB_ONNX_MODEL").map_err(|_| {
                VectorDBError::EmbeddingError(
                    "VECTORDB_ONNX_MODEL environment variable is required but not set".to_string(),
                )
            })?;

            let env_tokenizer_path = std::env::var("VECTORDB_ONNX_TOKENIZER").map_err(|_| {
                VectorDBError::EmbeddingError(
                    "VECTORDB_ONNX_TOKENIZER environment variable is required but not set"
                        .to_string(),
                )
            })?;

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
