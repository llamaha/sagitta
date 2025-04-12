// use crate::vectordb::embedding::EmbeddingModelType;
use crate::VectorDBError;
// use crate::vectordb::search::Search; // Removed
// use crate::vectordb::cache::CacheCheckResult; // Removed
use anyhow::{anyhow, Result};
use clap::Parser;
use log::{debug, error, warn};
use num_cpus;
use rayon;
// use std::path::PathBuf; // Remove unused import
use std::time::Instant;
// use crate::vectordb::search::result::SearchResult; // Removed
// use crate::vectordb::search::{chunking, snippet}; // Removed
// use std::collections::HashMap; // Removed
use std::fs;
// use std::collections::HashSet; // Removed
// use crate::vectordb::utils::cosine_distance; // Removed
// use walkdir::WalkDir; // Removed
// use chrono::{DateTime, Utc, TimeZone, Local}; // Removed DateTime, TimeZone, Local
use chrono::{Utc, TimeZone};

// Global flag for handling interrupts
pub static mut INTERRUPT_RECEIVED: bool = false;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
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
    },

    /// Search across indexed text chunks using semantic similarity
    Query {
        /// Search query string
        #[arg(required = true)]
        query: String,

        /// Maximum number of relevant chunks to return (default: 20)
        #[arg(short = 'l', long = "limit")]
        max_results: Option<usize>,

        /// Only show chunks from files with these extensions (e.g. rs,md,py)
        #[arg(short = 't', long = "file-types", value_delimiter = ',')]
        file_types: Option<Vec<String>>,
    },

    /// Show database statistics
    Stats,

    /// Clear the database
    Clear {},

    /// List the unique top-level directories found in the index
    List,

    /// Remove an indexed directory and its associated data
    Remove {
        /// Directory path to remove from the index
        #[arg(required = true)]
        dir: String,
    },
}

pub fn execute_command(command: Command, mut db: crate::vectordb::VectorDB) -> Result<()> {
    let start_time = Instant::now();
    let result = match command {
        Command::Index {
            dirs,
            file_types,
            threads,
        } => {
            debug!("Executing Index command for directories: {:?}", dirs);
            println!("Indexing files in {:?}...", dirs);

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
                let supported = crate::vectordb::VectorDB::get_supported_file_types();
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
                            let _now_ts = Utc::now().timestamp() as u64;
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
            debug!("Executing Query command with query: '{}', limit: {:?}, types: {:?}", 
                   query, max_results, file_types);
            let start_time = Instant::now();
            let limit = max_results.unwrap_or(20);

            // --- Call the library search function and handle Result --- 
            match db.search(&query, limit, file_types) {
                 Ok(search_results) => {
                     // --- Display Results --- 
                     let duration = start_time.elapsed();
                     debug!(
                         "Query completed in {:.2} seconds, found {} results",
                         duration.as_secs_f32(),
                         search_results.len()
                     );

                     if search_results.is_empty() {
                         println!("No relevant chunks found for query '{}'.", query);
                     } else {
                         println!(
                             "Found {} relevant chunks ({:.2} seconds):",
                             search_results.len(),
                             duration.as_secs_f32()
                         );
                         println!("---");
                         for (i, chunk) in search_results.iter().enumerate() {
                              let display_path = match fs::canonicalize(&chunk.file_path) {
                                  Ok(p) => p.to_string_lossy().into_owned(),
                                  Err(_) => chunk.file_path.clone(), 
                              };
                             println!(
                                 "{}. {} (Lines {}-{}) (score: {:.4})",
                                 i + 1,
                                 display_path,
                                 chunk.start_line,
                                 chunk.end_line,
                                 chunk.score
                             );
                             for line in chunk.text.lines() {
                                  println!("  {}", line);
                             }
                             println!("---");
                         }
                     }
                     // // println!("{}", crate::cli::formatters::format_search_results(&search_results)); // TODO: Uncomment when formatters exist
                     Ok(()) // Return Ok from this arm of the outer match
                 },
                 Err(e) => {
                     // Error handling logic remains the same, ends with return Err(...)
                     error!("Search failed: {}", e);
                     match e {
                         VectorDBError::IndexNotFound => {
                             eprintln!("Error: Search index is not built or is empty. Please run the 'index' command first.");
                             return Err(e.into());
                         }
                         VectorDBError::DimensionMismatch { expected, found } => {
                             eprintln!(
                                 "Error: Query embedding dimension ({}) does not match index dimension ({}).",
                                 found, expected
                             );
                             eprintln!("The index might be corrupted or built with a different model.");
                             return Err(e.into());
                         }
                         VectorDBError::EmbeddingError(msg) => {
                              eprintln!("Error generating embedding for query: {}", msg);
                              // Don't return early here, let the outer Err handle it?
                              // Or return specific error?
                              // Let's return the original error for now.
                               return Err(VectorDBError::EmbeddingError(msg).into());
                         }
                         _ => {
                             eprintln!("An unexpected error occurred during search: {}", e);
                             return Err(e.into());
                         }
                     }
                 }
             }
        }
        Command::Stats => {
            debug!("Executing Stats command");
            let stats = db.stats();
            // println!("{}", crate::cli::formatters::format_stats(&stats)); // TODO: Uncomment when formatters exist
            // Print basic stats for now
            println!("Database Statistics:");
            println!("  DB Path: {}", stats.db_path);
            println!("  Model Type: {}", stats.embedding_model_type);
            // ... (print other stats fields manually) ...
            Ok(())
        }
        Command::Clear {} => {
            println!("Clearing database...");
            db.clear()?;
            println!("Database cleared.");
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
                // Convert UNIX timestamp to DateTime<Utc> and then format locally
                let dt = match Utc.timestamp_opt(timestamp as i64, 0) {
                    chrono::LocalResult::Single(dt) => dt, // Keep as UTC
                    _ => { // Handle potential invalid timestamp
                        warn!("Invalid timestamp ({}) found for directory {}", timestamp, root_path_str);
                        println!("  - {} (Invalid Timestamp)", root_path_str);
                        continue;
                    }
                };
                // Format the timestamp (implicitly using local timezone via `println!`)
                let formatted_time = dt.format("%Y-%m-%d %H:%M:%S").to_string();
                println!("  - {} ({})", root_path_str, formatted_time);
            }
            // println!("{}", crate::cli::formatters::format_roots(indexed_roots_map)); // TODO: Uncomment when formatters exist
            // Print basic roots for now
            if indexed_roots_map.is_empty() {
                println!("No indexed directories found.");
            } else {
                println!("Indexed Directories:");
                for (_root_path_str, _timestamp) in indexed_roots_map {
                    // ... (print root path and time) ...
                }
            }
            Ok(())
        }
        Command::Remove { dir } => {
            debug!("Executing Remove command for directory: {}", dir);
            match db.remove_directory(&dir) {
                Ok(_) => Ok(()), // Return Ok
                Err(e) => Err(e.into()), // Return Err
            }
        }
    }; // End of match command assignment

    // Handle the result from the match arms that don't return early
    if result.is_ok() {
        let duration = start_time.elapsed();
        debug!("Command executed in {:.2?}", duration);
    }
    result // Return the final result
}
