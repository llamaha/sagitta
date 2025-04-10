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
use crate::vectordb::search::result::SearchResult;
use crate::vectordb::search::{chunking, snippet};
use std::collections::HashMap;
use std::fs;
use std::collections::HashSet;
use crate::vectordb::utils::cosine_distance;

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

        /// Use faster, keyword-based snippet extraction instead of semantic chunking.
        #[arg(long = "fast-snippets")]
        fast_snippets: bool,
    },

    /// Show database statistics
    Stats,

    /// Clear the database
    Clear {},
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
            fast_snippets,
        } => {
            debug!("Executing Query command: \"{}\"", query);
            let limit = max_results.unwrap_or(20);
            debug!("Using max_results limit: {}", limit);
            println!("Limiting results to a maximum of {}", limit);

            let model = db.create_embedding_model()?;
            let mut search = Search::new(db.clone(), model.clone());

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

            // Initial Search
            debug!("Performing initial search...");
            let mut initial_results: Vec<SearchResult> = if vector_only {
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
                    initial_results.retain(|result| {
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

            if initial_results.is_empty() {
                debug!("No results found for query: \"{}\"", query);
                println!("No results found.");
                return Ok(());
            }

            println!("\nSearch results for: {}\n", query);

            // --- Conditional Snippet Logic ---
            if fast_snippets {
                // --- Fast Snippet Path (Old Logic) ---
                debug!("Using fast snippet extraction.");
                for (i, result) in initial_results.iter().enumerate() {
                    println!(
                        "{}. {} (score: {:.2})",
                        i + 1,
                        result.file_path,
                        result.similarity
                    );
                    match snippet::get_snippet(&result.file_path, &query) {
                        Ok(snippet_text) => {
                            println!("{}", snippet_text);
                        }
                        Err(e) => {
                            error!("Failed to get snippet for {}: {}", result.file_path, e);
                            println!("  Error getting snippet: {}", e);
                        }
                    }
                    println!();
                }
            } else {
                // --- Semantic Snippet Path (New Logic) ---
                debug!("Using semantic snippet extraction.");

                let query_embedding = model.embed(&query)?;
                let mut all_chunks: Vec<(String, chunking::ChunkInfo, Vec<f32>)> = Vec::new();
                let mut file_contents: HashMap<String, String> = HashMap::new();

                // 1. Read content and chunk top N files
                debug!("Reading and chunking top {} files...", initial_results.len());
                for result in &initial_results {
                    if file_contents.contains_key(&result.file_path) {
                        continue;
                    }
                    match fs::read_to_string(&result.file_path) {
                        Ok(content) => {
                            let chunks = chunking::chunk_by_paragraphs(&content);
                            file_contents.insert(result.file_path.clone(), content);
                            for chunk in chunks {
                                all_chunks.push((result.file_path.clone(), chunk, Vec::new()));
                            }
                        }
                        Err(e) => {
                            error!("Failed to read file {} for semantic snippets: {}", result.file_path, e);
                        }
                    }
                }

                if all_chunks.is_empty() {
                    println!("Could not read or chunk any of the top results to generate semantic snippets.");
                    return Ok(());
                }

                // 2. Embed all chunks
                debug!("Embedding {} chunks...", all_chunks.len());
                let chunk_texts: Vec<&str> = all_chunks.iter().map(|(_, chunk, _)| chunk.text.as_str()).collect();
                // --- DEBUG: Log first few chunk texts ---
                for (idx, text) in chunk_texts.iter().take(5).enumerate() {
                    debug!("Chunk Text [{}]: {:?}", idx, text.chars().take(100).collect::<String>());
                }
                // --- END DEBUG ---
                let chunk_embeddings = model.embed_batch(&chunk_texts)?;

                // --- DEBUG: Log first few chunk embeddings (partial) ---
                for (idx, embedding) in chunk_embeddings.iter().take(5).enumerate() {
                    debug!("Chunk Embedding [{}]: dim={}, first 5 vals=[{:?}...]",
                          idx, embedding.len(), &embedding[..5.min(embedding.len())]);
                }
                // --- END DEBUG ---

                // Assign embeddings back to chunks
                for (i, embedding) in chunk_embeddings.into_iter().enumerate() {
                    if let Some(entry) = all_chunks.get_mut(i) {
                        entry.2 = embedding;
                    }
                }

                // 3. Calculate scores for chunks
                debug!("Calculating semantic scores for chunks...");
                let mut scored_chunks: Vec<(String, chunking::ChunkInfo, f32)> = Vec::new();
                let mut debug_scores_logged = 0; // Counter for debug logging
                for (file_path, chunk_info, chunk_embedding) in all_chunks {
                    if chunk_embedding.is_empty() { continue; }
                    let similarity = 1.0 - cosine_distance(&query_embedding, &chunk_embedding);
                    // --- DEBUG: Log first few similarity scores ---
                    if debug_scores_logged < 5 {
                        debug!("Chunk Score [{} @ L{}]: {:?}",
                              file_path.split('/').last().unwrap_or("?"), chunk_info.start_line, similarity);
                        debug_scores_logged += 1;
                    }
                    // --- END DEBUG ---
                    scored_chunks.push((file_path, chunk_info, similarity));
                }

                // 4. Find the best chunk for each of the *original* top N files
                debug!("Finding best semantic snippet for each top file...");
                let mut final_results: Vec<(String, f32, chunking::ChunkInfo)> = Vec::new();
                let mut processed_files = HashSet::new();

                for initial_result in &initial_results {
                    if !processed_files.insert(initial_result.file_path.clone()) {
                        continue;
                    }

                    let best_chunk_for_file = scored_chunks.iter()
                        .filter(|(fp, _, _)| *fp == initial_result.file_path)
                        .max_by(|(_, _, score_a), (_, _, score_b)| score_a.partial_cmp(score_b).unwrap_or(std::cmp::Ordering::Equal));

                    // --- DEBUG: Log best chunk found for first file ---
                    if processed_files.len() == 1 { // Only log for the very first file processed
                        if let Some((_fp, _info, score)) = best_chunk_for_file {
                            debug!("Best chunk for {}: Score={:?}", initial_result.file_path, score);
                        } else {
                            debug!("Best chunk for {}: None found", initial_result.file_path);
                        }
                    }
                    // --- END DEBUG ---

                    if let Some((_, chunk_info, chunk_score)) = best_chunk_for_file {
                        final_results.push((initial_result.file_path.clone(), *chunk_score, chunk_info.clone()));
                    } else {
                        debug!("No valid semantic chunk found for {}", initial_result.file_path);
                    }
                }

                // Sort final results by chunk score (highest similarity first)
                final_results.sort_by(|(_, score_a, _), (_, score_b, _)| score_b.partial_cmp(score_a).unwrap_or(std::cmp::Ordering::Equal));

                // 5. Display results
                for (i, (file_path, chunk_score, chunk_info)) in final_results.iter().take(limit).enumerate() {
                    println!(
                        "{}. {} (semantic score: {:.4})",
                        i + 1,
                        file_path,
                        chunk_score
                    );
                    println!("[Line {}] {}\n", chunk_info.start_line, chunk_info.text);
                    println!();
                }
            }
            // --- End Conditional Snippet Logic ---
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
        Command::Clear {} => {
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
