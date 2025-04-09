use crate::vectordb::embedding::EmbeddingModelType;
use crate::vectordb::error::VectorDBError;
use crate::vectordb::search::Search;
use crate::vectordb::VectorDB;
use anyhow::Result;
use clap::Parser;
use log::{debug, error};
use num_cpus;
use rayon;
use std::io::Write;
use std::path::{Path, PathBuf};
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

        /// Path to ONNX model file (required if not set via env var)
        #[arg(long = "onnx-model")]
        onnx_model: Option<String>,

        /// Path to ONNX tokenizer file (required if not set via env var)
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
            onnx_model,
            onnx_tokenizer,
        } => {
            debug!("Executing Index command for directory: {}", dir);
            println!("Indexing files in {}...", dir);

            // Try to set paths from args or env vars
            let model_path_opt = onnx_model.or_else(|| std::env::var("VECTORDB_ONNX_MODEL").ok());
            let tokenizer_path_opt = onnx_tokenizer.or_else(|| std::env::var("VECTORDB_ONNX_TOKENIZER").ok());

            if let (Some(mp), Some(tp)) = (&model_path_opt, &tokenizer_path_opt) {
                match db.set_onnx_paths(Some(PathBuf::from(mp)), Some(PathBuf::from(tp))) {
                    Ok(_) => {
                        // Setting paths implies using ONNX, no need to set type explicitly
                        debug!("Successfully set ONNX model paths.");
                        println!("Using ONNX embedding model:");
                        println!("  - Model: {}", mp);
                        println!("  - Tokenizer: {}", tp);
                    }
                    Err(e) => {
                        // Error during path setting likely means validation failed (e.g., file not found)
                        error!("Failed to validate ONNX model/tokenizer paths: {}", e);
                        eprintln!("Error configuring ONNX model: {}", e);
                        eprintln!("Please ensure the specified ONNX model and tokenizer files exist and are valid.");
                        return Err(e.into()); // Return error
                    }
                }
            } else {
                // Paths not provided via args or env vars
                error!("ONNX model and tokenizer paths are required but not set.");
                eprintln!("Error: ONNX model and tokenizer paths must be provided either via --onnx-model/--onnx-tokenizer arguments or VECTORDB_ONNX_MODEL/VECTORDB_ONNX_TOKENIZER environment variables.");
                // Return an appropriate error
                return Err(VectorDBError::EmbeddingError("ONNX paths not configured".to_string()).into());
            }
            
            // Setup thread pool
            let num_threads = threads.unwrap_or_else(num_cpus::get);
            rayon::ThreadPoolBuilder::new()
                .num_threads(num_threads)
                .build_global()
                .map_err(|e| anyhow::anyhow!("Failed to build thread pool: {}", e))?;

            let start = Instant::now();

            // Determine file types to use based on input flags and fast mode
            let file_types_to_use = if file_types.is_empty() {
                // Standard mode with no specified file types, use all supported types with code parsers
                let supported = VectorDB::get_supported_file_types();
                println!(
                    "No file types specified, using all supported types with code parsers: {}",
                    supported.join(", ")
                );
                supported
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

            // Directly create the ONNX model. This will error out if paths aren't set.
            println!("Using ONNX model for semantic search.");
            let model = db.create_embedding_model()?;
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
