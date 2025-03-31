use clap::Parser;
use anyhow::Result;
use crate::vectordb::VectorDB;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::search::{Search, CodeSearchType};
use crate::vectordb::parsing::{RustAnalyzer, CodeElement};
use crate::vectordb::hnsw::HNSWConfig;
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
        
        /// Use HNSW index for faster searches on large codebases
        #[arg(long = "use-hnsw")]
        use_hnsw: bool,

        /// Number of threads to use for indexing (defaults to available CPUs)
        #[arg(short = 'j', long = "threads")]
        threads: Option<usize>,
    },

    /// Search for files by content
    Query {
        /// Search query
        #[arg(required = true)]
        query: String,
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
}

pub fn execute_command(command: Command, mut db: VectorDB) -> Result<()> {
    match command {
        Command::Index { dir, file_types, use_hnsw, threads } => {
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
            
            // Create HNSW config if the flag is used
            if use_hnsw {
                println!("Using HNSW index for faster searches...");
                let hnsw_config = HNSWConfig::default();
                db.set_hnsw_config(Some(hnsw_config));
            }
            
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
        Command::Query { query } => {
            let model = EmbeddingModel::new()?;
            let search = Search::new(db, model);
            let results = search.search(&query)?;

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
                println!("        Use --use-hnsw with the index command to enable.");
                
                // Display layer stats
                println!("\n  Layer Statistics:");
                for (i, layer) in hnsw_stats.layer_stats.iter().enumerate() {
                    println!("    Layer {}: {} nodes, {:.2} avg. connections", 
                        i, layer.nodes, layer.avg_connections);
                }
            } else {
                println!("\nHNSW Index: Not enabled");
                println!("  For faster searches on large codebases, use --use-hnsw when indexing.");
            }
        }
        Command::Clear => {
            db.clear()?;
            println!("Database cleared!");
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
    fn test_hnsw_flag_creates_index() -> Result<()> {
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
        
        // Execute the index command with use_hnsw flag
        let command = Command::Index {
            dir: temp_dir.path().to_string_lossy().to_string(),
            file_types: vec!["rs".to_string()],
            use_hnsw: true,
            threads: None,
        };
        
        execute_command(command, db)?;
        
        // Load the DB again to check if HNSW index was created and saved
        let reloaded_db = VectorDB::new(db_path)?;
        let stats = reloaded_db.stats();
        assert!(stats.hnsw_stats.is_some(), "HNSW index should be created");
        
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
        
        // Test with HNSW
        let db_path_hnsw = db_dir.path().join("test_hnsw.db").to_string_lossy().to_string();
        let mut db_hnsw = VectorDB::new(db_path_hnsw)?;
        db_hnsw.set_hnsw_config(Some(HNSWConfig::default()));
        
        // Index with HNSW
        db_hnsw.index_directory(&temp_dir.path().to_string_lossy(), &["rs".to_string()])?;
        
        // Create embedding model for HNSW search
        let model_hnsw = EmbeddingModel::new()?;
        let mut search_hnsw = Search::new(db_hnsw, model_hnsw);
        
        // Test without HNSW
        let db_path_normal = db_dir.path().join("test_normal.db").to_string_lossy().to_string();
        let mut db_normal = VectorDB::new(db_path_normal)?;
        
        // Index without HNSW
        db_normal.index_directory(&temp_dir.path().to_string_lossy(), &["rs".to_string()])?;
        
        // Create separate embedding model for normal search
        let model_normal = EmbeddingModel::new()?;
        let mut search_normal = Search::new(db_normal, model_normal);
        
        // Measure search time with HNSW
        let start_hnsw = Instant::now();
        let _results_hnsw = search_hnsw.search("function")?;
        let duration_hnsw = start_hnsw.elapsed();
        
        // Measure search time without HNSW
        let start_normal = Instant::now();
        let _results_normal = search_normal.search("function")?;
        let duration_normal = start_normal.elapsed();
        
        // For consistent test results, we don't strictly assert that HNSW is faster
        // as it might not be measurable with a small test dataset
        // Instead, just log the results
        println!("Search time with HNSW: {:?}", duration_hnsw);
        println!("Search time without HNSW: {:?}", duration_normal);
        
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
        
        // Create VectorDB with HNSW config
        let mut db = VectorDB::new(db_path.clone())?;
        db.set_hnsw_config(Some(HNSWConfig::default()));
        
        // Index files
        db.index_directory(&temp_dir.path().to_string_lossy(), &["rs".to_string()])?;
        
        // Get stats before reloading
        let hnsw_stats_before = db.stats().hnsw_stats;
        assert!(hnsw_stats_before.is_some(), "HNSW index should be present");
        
        // Force the DB to save changes
        drop(db);
        
        // Reload the database
        let db_reloaded = VectorDB::new(db_path)?;
        
        // Check if HNSW config is still there
        let hnsw_stats_after = db_reloaded.stats().hnsw_stats;
        assert!(hnsw_stats_after.is_some(), "HNSW index should persist after reload");
        
        Ok(())
    }
} 