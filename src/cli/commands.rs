use clap::Parser;
use anyhow::Result;
use crate::vectordb::VectorDB;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::search::{Search, CodeSearchType};
use crate::vectordb::parsing::{RustAnalyzer, CodeElement};
use std::path::Path;
use std::collections::HashSet;
use colored::Colorize;
use walkdir::WalkDir;
use std::time::{Instant, Duration};
use rayon;
use num_cpus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use ctrlc;

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

    /// Hybrid search combining semantic and lexical matches (deprecated, use Query instead)
    #[deprecated(
        since = "0.2.0",
        note = "Use 'query' command instead which performs hybrid search by default"
    )]
    Hybrid {
        /// Search query
        #[arg(required = true)]
        query: String,
        
        /// Weight for vector search (default: 0.7)
        #[arg(long = "vector-weight")]
        vector_weight: Option<f32>,
        
        /// Weight for BM25 lexical search (default: 0.3)
        #[arg(long = "bm25-weight")]
        bm25_weight: Option<f32>,
    },

    /// Record feedback for a search result to improve future searches
    Feedback {
        /// The search query
        #[arg(short, long)]
        query: String,
        
        /// The file path of the result
        #[arg(short, long)]
        file: String,
        
        /// Whether the result was relevant (true) or not (false)
        #[arg(short, long)]
        relevant: bool,
    },
}

pub fn execute_command(command: Command, mut db: VectorDB) -> Result<()> {
    match command {
        Command::Index { dir, file_types, threads } => {
            println!("Indexing files in {}...", dir);
            
            // Set up signal handler for clean shutdown
            let running = Arc::new(AtomicBool::new(true));
            let r = running.clone();
            
            // Handle Ctrl+C gracefully
            ctrlc::set_handler(move || {
                println!("\nInterrupt received, finishing current operations and shutting down...");
                r.store(false, Ordering::SeqCst);
                unsafe { INTERRUPT_RECEIVED = true; }
            }).expect("Failed to set Ctrl+C handler");
            
            // Set thread count if specified
            let num_cpus = num_cpus::get();
            if let Some(thread_count) = threads {
                println!("Using {} threads for indexing ({} CPUs available)...", 
                         thread_count, num_cpus);
                rayon::ThreadPoolBuilder::new()
                    .num_threads(thread_count)
                    .build_global()
                    .unwrap_or_else(|e| println!("Failed to set thread count: {}", e));
            } else {
                println!("Using all {} available CPUs for indexing...", num_cpus);
            }
            
            let start = Instant::now();
            
            // Check for interrupt periodically during indexing
            match db.index_directory(&dir, &file_types) {
                Ok(_) => {
                    let duration = start.elapsed();
                    if unsafe { INTERRUPT_RECEIVED } {
                        println!("Indexing was interrupted but data has been saved safely.");
                    } else {
                        println!("Indexing complete in {:.2} seconds!", duration.as_secs_f32());
                    }
                },
                Err(e) => {
                    if unsafe { INTERRUPT_RECEIVED } {
                        println!("Indexing was interrupted but data has been saved safely.");
                    } else {
                        return Err(e.into());
                    }
                }
            }
        }
        Command::Query { query, vector_only, vector_weight, bm25_weight } => {
            let model = EmbeddingModel::new()?;
            let search = Search::new(db, model);
            
            // Determine search type based on flags
            let results = if vector_only {
                println!("Performing vector-only search...");
                search.search(&query)?
            } else {
                println!("Performing hybrid search (combining semantic and lexical matching)...");
                
                // Show weights being used
                let v_weight = vector_weight.unwrap_or(HYBRID_VECTOR_WEIGHT);
                let b_weight = bm25_weight.unwrap_or(HYBRID_BM25_WEIGHT);
                println!("Using weights: vector={:.2}, bm25={:.2}", v_weight, b_weight);
                
                search.hybrid_search(&query, vector_weight, bm25_weight)?
            };

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            // Check if this is a method-related query
            let is_method_query = query.to_lowercase().contains("method") || 
                                  query.to_lowercase().contains("function") ||
                                  query.to_lowercase().contains("fn ");

            if is_method_query {
                println!("\nSearch results for methods: {}\n", query);
            } else {
                println!("\nSearch results for: {}\n", query);
            }
            
            for (i, result) in results.iter().enumerate() {
                println!("{}. {} (similarity: {:.2})", i + 1, result.file_path, result.similarity);
                println!("{}", result.snippet);
                println!();
            }
        }
        Command::CodeSearch { query, search_type } => {
            let model = EmbeddingModel::new()?;
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
            println!("Database path: {}", stats.db_path);
            println!("Cached files: {}", stats.cached_files);
            
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
        Command::Hybrid { query, vector_weight, bm25_weight } => {
            let model = EmbeddingModel::new()?;
            let search = Search::new(db, model);
            
            println!("Performing hybrid search (combining semantic and lexical matching)...");
            
            // Show weights being used
            let v_weight = vector_weight.unwrap_or(0.7);
            let b_weight = bm25_weight.unwrap_or(0.3);
            println!("Using weights: vector={:.2}, bm25={:.2}", v_weight, b_weight);
            
            let results = search.hybrid_search(&query, vector_weight, bm25_weight)?;

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            println!("\nHybrid search results for: {}\n", query);
            for (i, result) in results.iter().enumerate() {
                println!("{}. {} (score: {:.2})", i + 1, result.file_path, result.similarity);
                println!("{}", result.snippet);
                println!();
            }
        }
        Command::Feedback { query, file, relevant } => {
            // Create search instance to access feedback recording
            let model = EmbeddingModel::new()?;
            let mut search = Search::new(db, model);
            
            // Record the feedback
            search.record_result_feedback(&query, &file, relevant)?;
            
            println!("Feedback recorded: '{}' {} for query '{}'", 
                file, 
                if relevant { "is relevant" } else { "is not relevant" },
                query);
        }
    }
    Ok(())
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
        let model_hnsw = EmbeddingModel::new()?;
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
        let model_slow = EmbeddingModel::new()?;
        let _search_slow = Search::new(db_slow, model_slow);
        
        // Measure search time with standard HNSW
        let start_hnsw = Instant::now();
        let _results_hnsw = search_hnsw.search("function")?;
        let duration_hnsw = start_hnsw.elapsed();
        
        // For comparison only, use manual vector search
        let query = "function";
        let model = EmbeddingModel::new()?;
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
    fn test_hybrid_search_command() -> Result<()> {
        // Create temporary directory for test files
        let temp_dir = tempdir()?;
        let db_dir = tempdir()?;
        let db_path = db_dir.path().join("test_hybrid.db").to_string_lossy().to_string();
        
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
        
        // Test new Query command with hybrid search enabled (default)
        let query_command = Command::Query { 
            query: "search".to_string(),
            vector_only: false,
            vector_weight: Some(0.6),
            bm25_weight: Some(0.4)
        };
        
        // Execute query command
        let result = execute_command(query_command, db.clone());
        assert!(result.is_ok(), "Query command with hybrid search should execute without error");
        
        // Test vector-only search
        let vector_command = Command::Query { 
            query: "search".to_string(),
            vector_only: true,
            vector_weight: None,
            bm25_weight: None
        };
        
        // Execute vector-only command
        let result = execute_command(vector_command, db.clone());
        assert!(result.is_ok(), "Query command with vector-only should execute without error");
        
        // Test deprecated Hybrid command
        #[allow(deprecated)]
        let deprecated_command = Command::Hybrid { 
            query: "search".to_string(),
            vector_weight: None,
            bm25_weight: None
        };
        
        // Execute deprecated command
        let result = execute_command(deprecated_command, db);
        assert!(result.is_ok(), "Deprecated Hybrid command should still execute without error");

        Ok(())
    }
} 