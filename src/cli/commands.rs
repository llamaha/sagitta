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
        Command::Index { dir, file_types } => {
            println!("Indexing files in {}...", dir);
            db.index_directory(&dir, &file_types)?;
            println!("Indexing complete!");
        }
        Command::Query { query } => {
            let model = EmbeddingModel::new()?;
            let search = Search::new(db, model);
            let results = search.search(&query)?;

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            println!("\nSearch results for: {}\n", query);
            for (i, result) in results.iter().enumerate() {
                println!("{}. {} (similarity: {:.2})", i + 1, result.file_path, result.similarity);
                println!("   {}", result.snippet);
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
                
                // Display layer stats
                println!("  Layer statistics:");
                for (i, layer_stat) in hnsw_stats.layer_stats.iter().enumerate() {
                    println!("    Layer {}: {} nodes", i, layer_stat.nodes);
                }
            } else {
                println!("\nHNSW Index: Not enabled");
            }
        }
        Command::Clear => {
            db.clear()?;
            println!("Database cleared!");
        }
    }
    Ok(())
} 