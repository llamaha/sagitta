// use crate::vectordb::embedding::EmbeddingModelType;
use crate::vectordb::error::VectorDBError;
// use crate::vectordb::search::Search; // Removed
use crate::vectordb::VectorDB;
// use crate::vectordb::cache::CacheCheckResult; // Removed
use anyhow::{anyhow, Result};
use clap::Parser;
use log::{debug, error, warn};
use num_cpus;
use rayon;
use std::path::{Path, PathBuf};
use std::time::Instant;
// use crate::vectordb::search::result::SearchResult; // Removed
// use crate::vectordb::search::{chunking, snippet}; // Removed
// use std::collections::HashMap; // Removed
use std::fs;
// use std::collections::HashSet; // Removed
// use crate::vectordb::utils::cosine_distance; // Removed
// use walkdir::WalkDir; // Removed
// use chrono::{DateTime, Utc, TimeZone, Local}; // Removed DateTime, TimeZone, Local
use chrono::{Utc, Local, TimeZone}; // Add back Local and TimeZone

// Default weights for hybrid search
const HYBRID_VECTOR_WEIGHT: f32 = 0.7;
const HYBRID_BM25_WEIGHT: f32 = 0.3;

// Global flag for handling interrupts
pub static mut INTERRUPT_RECEIVED: bool = false;

#[derive(Parser, Debug)]
pub enum Command {
    /// Index files in one or more directories
    Index {
        /// Directories to index (provide one or more paths)
        #[arg(required = true)]
        dirs: Vec<String>,

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

        /// File types to search (e.g. rs,rb,go,js,ts)
        #[arg(short = 't', long = "file-types", value_delimiter = ',')]
        file_types: Option<Vec<String>>,
    },

    /// Show database statistics
    Stats,

    /// Clear the database
    Clear {},

    /// List the unique top-level directories found in the index
    List,
}

pub fn execute_command(command: Command, mut db: VectorDB) -> Result<()> {
    match command {
        Command::Index {
            dirs,
            file_types,
            threads,
            onnx_model,
            onnx_tokenizer,
        } => {
            debug!("Executing Index command for directories: {:?}", dirs);
            println!("Indexing files in {:?}...", dirs);

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
            let mut overall_result = Ok(()); // Track overall success

            // Determine file types to use based on input flags
            let file_types_to_use = if file_types.is_empty() {
                let supported = VectorDB::get_supported_file_types();
                println!(
                    "No file types specified, using all supported types: {}",
                    supported.join(", ")
                );
                supported
            } else {
                println!("Indexing file types: {}", file_types.join(", "));
                file_types
            };

            // Loop through each directory provided
            for dir_path_str in dirs {
                 // Canonicalize the input path string
                 let canonical_dir = match fs::canonicalize(&dir_path_str) {
                     Ok(path) => path,
                     Err(e) => {
                         error!("Failed to find or canonicalize directory '{}': {}", dir_path_str, e);
                         eprintln!("Error: Invalid directory '{}': {}", dir_path_str, e);
                         if overall_result.is_ok() {
                             overall_result = Err(anyhow!("Invalid directory: {}", dir_path_str));
                         }
                         continue; // Skip to the next directory
                     }
                 };
                 let canonical_dir_str = canonical_dir.to_string_lossy().to_string();

                 println!("Starting indexing for: {}", canonical_dir_str);
                 debug!(
                    "Starting directory indexing: {}, file types: {:?}",
                    canonical_dir_str,
                    &file_types_to_use
                 );
                // Index the current canonical directory
                match db.index_directory(&canonical_dir_str, &file_types_to_use) {
                    Ok(_) => {
                        if unsafe { INTERRUPT_RECEIVED } {
                            debug!("Indexing interrupted for {}, data saved safely", canonical_dir_str);
                            println!("Indexing interrupted for {}, data saved safely.", canonical_dir_str);
                            // Set overall result to error if interrupted
                            overall_result = Err(anyhow!("Indexing interrupted"));
                            break; // Stop processing more directories if interrupted
                        } else {
                            debug!(
                                "Indexing for {} completed successfully",
                                canonical_dir_str
                            );
                            println!("Finished indexing for: {}", canonical_dir_str);
                            // Get current UTC timestamp and update
                            let now_ts = Utc::now().timestamp() as u64;
                            db.update_indexed_root_timestamp(canonical_dir_str, now_ts); 
                        }
                    }
                    Err(e) => {
                        if unsafe { INTERRUPT_RECEIVED } {
                            debug!("Indexing interrupted for {}, data saved safely", canonical_dir_str);
                            println!("Indexing interrupted for {}, data saved safely.", canonical_dir_str);
                             overall_result = Err(anyhow!("Indexing interrupted"));
                            break; // Stop processing more directories
                        } else {
                            error!("Indexing failed for {}: {}", canonical_dir_str, e);
                            eprintln!("Error indexing {}: {}", canonical_dir_str, e);
                            // Store the first error encountered, but continue if possible
                            if overall_result.is_ok() {
                                overall_result = Err(e.into());
                            }
                            // Do not break here, allow indexing other directories if requested
                        }
                    }
                }
            }

            // Save db once after all directories are processed (save includes HNSW rebuild)
             if overall_result.is_ok() {
                  if let Err(e) = db.save() {
                      error!("Failed to save database after indexing: {}", e);
                      overall_result = Err(e.into());
                  }
              }
            
            // Final summary message based on overall result
            let duration = start.elapsed();
            if overall_result.is_ok() {
                debug!(
                    "All indexing tasks completed successfully in {:.2} seconds",
                    duration.as_secs_f32()
                );
                println!(
                    "\nTotal indexing time: {:.2} seconds!",
                    duration.as_secs_f32()
                );
            } else if unsafe { INTERRUPT_RECEIVED } {
                // Message already printed during the loop for interruption
                println!("\nTotal time before interruption: {:.2} seconds.", duration.as_secs_f32());
            } else {
                // Report that some errors occurred
                eprintln!(
                    "\nIndexing completed with errors in {:.2} seconds. Check logs for details.",
                    duration.as_secs_f32()
                );
            }

            return overall_result; // Return the final result (Ok or the first error)
        }
        Command::Query {
            query,
            max_results,
            file_types,
        } => {
            debug!("Executing Query command with query: '{}'", query);
            let start_time = Instant::now();

            // --- Define QueryResultChunk struct ---
            #[derive(Debug)] // Added Debug derive for easier printing/logging
            struct QueryResultChunk {
                file_path: String,
                start_line: usize,
                end_line: usize,
                text: String,
                score: f32,
            }

            // Get search limit
            let limit = max_results.unwrap_or(20);

            // --- New Chunk-Based Query Logic ---

            // 1. Create Embedding Model
            let model = match db.create_embedding_model() {
                Ok(m) => m,
                Err(e) => {
                    error!("Failed to create embedding model for query: {}", e);
                    eprintln!("Error initializing embedding model: {}. Have you indexed any data?", e);
                    return Err(e.into());
                }
            };

            // 2. Generate Query Embedding
            let query_embedding = match model.embed(&query) {
                Ok(emb) => emb,
                Err(e) => {
                    error!("Failed to generate embedding for query '{}': {}", query, e);
                    eprintln!("Error generating query embedding: {}", e);
                    return Err(e.into());
                }
            };
            debug!("Query embedding generated (dim={})", query_embedding.len());

            // 3. Access HNSW Index
            let hnsw_index = match db.hnsw_index() {
                Some(index) => index,
                None => {
                    warn!("HNSW index is not available. No search results can be returned.");
                    eprintln!("Search index is not built. Please run the 'index' command first.");
                    return Ok(()); // Or return an error?
                }
            };

            // Verify index dimension matches query embedding dimension
            if hnsw_index.get_config().dimension != query_embedding.len() {
                error!(
                    "Query embedding dimension ({}) does not match HNSW index dimension ({}).",
                    query_embedding.len(),
                    hnsw_index.get_config().dimension
                );
                eprintln!(
                    "Error: Query embedding dimension ({}) does not match the index dimension ({}). \\
                     The index might be corrupted or built with a different model. Please re-index.",
                    query_embedding.len(),
                    hnsw_index.get_config().dimension
                );
                return Err(anyhow!("Index dimension mismatch"));
            }

            // 4. Perform HNSW Search
            // TODO: Make ef_search configurable?
            let ef_search = 100; // Example value
            debug!("Performing HNSW search with k={}, ef_search={}", limit, ef_search);
            let search_results = hnsw_index.search_parallel(&query_embedding, limit, ef_search)?;
            debug!("HNSW search returned {} results", search_results.len());

            // 5. Process HNSW Results
            let mut chunk_results: Vec<QueryResultChunk> = Vec::with_capacity(search_results.len());
            for (node_id, distance) in search_results {
                if let Some(chunk_data) = db.indexed_chunks.get(node_id) {
                    let similarity = 1.0 - distance;
                    // Basic filtering (optional, can add more like file_type filtering here if needed)
                    if similarity < 0.0 { continue; } // Skip results with negative similarity (highly dissimilar)

                    // Filter by file type if specified
                    if let Some(ref allowed_types) = file_types {
                        if !allowed_types.is_empty() {
                             if let Some(extension) = Path::new(&chunk_data.file_path).extension().and_then(|ext| ext.to_str()) {
                                 if !allowed_types.iter().any(|ft| ft.eq_ignore_ascii_case(extension)) {
                                     debug!("Skipping chunk from file {} due to file type filter", chunk_data.file_path);
                                     continue; // Skip chunk if file type doesn't match
                                 }
                             } else {
                                 continue; // Skip files with no extension if filtering
                             }
                        }
                    }

                    chunk_results.push(QueryResultChunk {
                        file_path: chunk_data.file_path.clone(),
                        start_line: chunk_data.start_line,
                        end_line: chunk_data.end_line,
                        text: chunk_data.text.clone(),
                        score: similarity,
                    });
                } else {
                    error!("Invalid node ID {} returned from HNSW search, data mismatch!", node_id);
                    // This indicates a potential corruption or bug
                }
            }

            // 6. Display Results
            let elapsed = start_time.elapsed();
            if chunk_results.is_empty() {
                println!("No relevant chunks found for query '{}'", query);
            } else {
                println!("Found {} relevant chunks ({:.2} seconds):", chunk_results.len(), elapsed.as_secs_f32());
                println!("---");
                for (i, result) in chunk_results.iter().enumerate() {
                    // Use canonicalize to try and resolve symlinks/relative paths
                    let display_path = match fs::canonicalize(&result.file_path) {
                         Ok(p) => p.to_string_lossy().into_owned(),
                         Err(_) => result.file_path.clone(), // Fallback to original path if canonicalization fails
                    };
                    println!(
                        "{}. {} (Lines {}-{}) (score: {:.4})",
                        i + 1,
                        display_path,
                        result.start_line,
                        result.end_line,
                        result.score
                    );
                    // Indent the chunk text slightly
                    for line in result.text.lines() {
                        println!("  {}", line);
                    }
                    println!("---");
                }
            }
            debug!("Query processing took {:.4} seconds", elapsed.as_secs_f32());

            Ok(())
        }
        Command::Stats => {
            let stats = db.stats();
            println!("Database Statistics:");
            println!("  DB Path: {}", stats.db_path);
            println!("  Model Type: {}", stats.embedding_model_type);
            println!("  Embedding Dimension: {}", stats.embedding_dimension);
            println!("  Unique Files Indexed: {}", stats.unique_files);
            println!("  Total Chunks Indexed: {}", stats.indexed_chunks);
            println!("  Cached Files (hashes): {}", stats.cached_files);
            if let Some(hnsw_stats) = stats.hnsw_stats {
                println!("  HNSW Index:");
                println!("    Total Nodes: {}", hnsw_stats.total_nodes);
                println!("    Layers: {}", hnsw_stats.layers);
                // Add more HNSW stats if desired
            } else {
                println!("  HNSW Index: Not built");
            }
            Ok(())
        }
        Command::Clear {} => {
            println!("Clearing database...");
            db.clear()?;
            println!("Database cleared successfully.");
            Ok(())
        }
        Command::List => {
            debug!("Executing List command");
            println!("Retrieving indexed directories...");

            let indexed_roots_map = db.indexed_roots();

            if indexed_roots_map.is_empty() {
                println!("  No directories have been explicitly indexed yet.");
                return Ok(());
            }

            // Convert HashMap to Vec for sorting
            let mut sorted_roots: Vec<(String, u64)> = indexed_roots_map.iter()
                                                    .map(|(k, v)| (k.clone(), *v))
                                                    .collect();
            // Sort by path (the String key)
            sorted_roots.sort_by(|a, b| a.0.cmp(&b.0));

            println!("Indexed Directories (Last Indexed):");

            // Print path and formatted timestamp
            for (root_path_str, timestamp) in sorted_roots {
                 // Convert UNIX timestamp to DateTime<Local>
                let dt = match Utc.timestamp_opt(timestamp as i64, 0) {
                    chrono::LocalResult::Single(dt) => dt.with_timezone(&Local),
                    _ => { // Handle potential invalid timestamp
                         warn!("Invalid timestamp ({}) found for directory {}", timestamp, root_path_str);
                         // Print placeholder or skip?
                         println!("  - {} (Invalid Timestamp)", root_path_str);
                         continue;
                    }
                 };
                 // Format the timestamp
                 let formatted_time = dt.format("%Y-%m-%d %H:%M:%S").to_string();
                 println!("  - {} ({})", root_path_str, formatted_time);
            }
            Ok(())
        }
    }
}
