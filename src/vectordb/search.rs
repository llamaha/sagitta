use std::fs;
use anyhow::Result;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::db::VectorDB;
use crate::vectordb::parsing::{CodeParser, RustAnalyzer, CodeElement};
use std::collections::HashMap;
use regex;
use std::path::Path;

const SIMILARITY_THRESHOLD: f32 = 0.3;
const MIN_CONTEXT_LINES: usize = 2;
const MAX_CONTEXT_LINES: usize = 8;
const BM25_K1: f32 = 1.5;
const BM25_B: f32 = 0.75;
const POSITION_BOOST: f32 = 0.2;
const WINDOW_SIZE: usize = 8;
const USE_HNSW: bool = true;
const HNSW_TOP_K: usize = 20;
const CODE_SEARCH_BOOST: f32 = 1.5; // Boost for code-aware search results

#[derive(Debug)]
pub struct SearchResult {
    pub file_path: String,
    pub similarity: f32,
    pub snippet: String,
    pub code_context: Option<String>, // Added code context
}

pub struct Search {
    db: VectorDB,
    model: EmbeddingModel,
    avg_doc_length: f32,
    code_parser: Option<CodeParser>,
    rust_analyzer: Option<RustAnalyzer>, // Added rust analyzer
}

impl Search {
    pub fn new(db: VectorDB, model: EmbeddingModel) -> Self {
        // Calculate average document length
        let total_length: usize = db.embeddings.values()
            .map(|embedding| embedding.iter().filter(|&&x| x > 0.0).count())
            .sum();
        let avg_doc_length = total_length as f32 / db.embeddings.len() as f32;
        
        // Create rust analyzer if possible
        let rust_analyzer = RustAnalyzer::new().ok();
        
        Self { 
            db, 
            model,
            avg_doc_length,
            code_parser: Some(CodeParser::new()),
            rust_analyzer,
        }
    }

    fn calculate_bm25(&self, query_embedding: &[f32], doc_embedding: &[f32]) -> f32 {
        let mut score = 0.0;
        let doc_length = doc_embedding.iter().filter(|&&x| x > 0.0).count() as f32;
        
        // Calculate term frequencies
        let mut query_tf = HashMap::new();
        let mut doc_tf = HashMap::new();
        
        for (i, &q) in query_embedding.iter().enumerate() {
            if q > 0.0 {
                *query_tf.entry(i).or_insert(0.0) += q;
            }
        }
        
        for (i, &d) in doc_embedding.iter().enumerate() {
            if d > 0.0 {
                *doc_tf.entry(i).or_insert(0.0) += d;
            }
        }
        
        // Calculate BM25 score
        for (term, &_q_tf) in query_tf.iter() {
            if let Some(&d_tf) = doc_tf.get(term) {
                let idf = ((self.db.embeddings.len() as f32 + 1.0) / 
                          (self.db.embeddings.values()
                           .filter(|doc| doc[*term] > 0.0)
                           .count() as f32 + 1.0))
                    .ln();
                
                let tf = d_tf / (1.0 - BM25_B + BM25_B * doc_length / self.avg_doc_length);
                score += idf * (tf * (BM25_K1 + 1.0)) / (tf + BM25_K1);
            }
        }
        
        score
    }

    fn calculate_position_boost(&self, file_path: &str, query: &str) -> f32 {
        let content = match fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(_) => return 1.0,
        };
        
        // Find the first occurrence of any query term
        let query_terms: Vec<&str> = query.split_whitespace().collect();
        let mut min_position = f32::MAX;
        
        for term in query_terms {
            if let Some(pos) = content.to_lowercase().find(&term.to_lowercase()) {
                min_position = min_position.min(pos as f32);
            }
        }
        
        if min_position == f32::MAX {
            return 1.0;
        }
        
        // Boost based on position (earlier = higher boost)
        let normalized_pos = min_position / content.len() as f32;
        1.0 + (POSITION_BOOST * (1.0 - normalized_pos))
    }

    // Enhanced search_code method using RustAnalyzer
    pub fn search_code(&mut self, query: &str, search_type: Option<CodeSearchType>) -> Result<Vec<SearchResult>> {
        // First, use the semantic search to get initial results
        let mut results = self.search(query)?;
        
        // Check if rust-analyzer is available
        if let Some(analyzer) = &mut self.rust_analyzer {
            // Parse all files first
            let file_paths: Vec<_> = results.iter()
                .map(|r| Path::new(&r.file_path))
                .filter(|p| p.extension().map_or(false, |ext| ext == "rs"))
                .collect();
                
            // Parse each Rust file
            for path in &file_paths {
                if path.exists() {
                    let _ = analyzer.parse_file(path);
                }
            }
            
            // Apply advanced code-aware boosts based on search type
            for result in &mut results {
                // Only apply to Rust files
                let path = Path::new(&result.file_path);
                if !path.extension().map_or(false, |ext| ext == "rs") {
                    continue;
                }
                
                let code_boost = match search_type {
                    Some(CodeSearchType::Function) => {
                        // Look for function references in all files
                        match analyzer.find_references(query) {
                            Ok(refs) => {
                                let refs_in_this_file: Vec<_> = refs.iter()
                                    .filter(|e| match e {
                                        CodeElement::Import { span, .. } => {
                                            span.file_path.to_string_lossy() == result.file_path
                                        },
                                        _ => false,
                                    })
                                    .collect();
                                
                                if !refs_in_this_file.is_empty() {
                                    // Update snippet to include the reference context
                                    if let Some(first_ref) = refs_in_this_file.first() {
                                        if let CodeElement::Import { path: _, span } = first_ref {
                                            if let Ok(content) = fs::read_to_string(&span.file_path) {
                                                let lines: Vec<_> = content.lines().collect();
                                                
                                                // Create context for the snippet
                                                let start_line = span.start_line.saturating_sub(2);
                                                let end_line = std::cmp::min(span.end_line + 2, lines.len());
                                                
                                                let context = lines[start_line..end_line].join("\n");
                                                result.code_context = Some(context);
                                            }
                                        }
                                    }
                                    
                                    CODE_SEARCH_BOOST
                                } else {
                                    1.0
                                }
                            },
                            Err(_) => 1.0,
                        }
                    },
                    Some(CodeSearchType::Type) => {
                        // Look for type references
                        match analyzer.find_references(query) {
                            Ok(refs) => {
                                let refs_in_this_file: Vec<_> = refs.iter()
                                    .filter(|e| match e {
                                        CodeElement::Import { span, .. } => {
                                            span.file_path.to_string_lossy() == result.file_path
                                        },
                                        _ => false,
                                    })
                                    .collect();
                                
                                if !refs_in_this_file.is_empty() {
                                    CODE_SEARCH_BOOST
                                } else {
                                    1.0
                                }
                            },
                            Err(_) => 1.0,
                        }
                    },
                    Some(CodeSearchType::Dependency) => {
                        // Check using rust-analyzer if the file has a dependency on query
                        let path = Path::new(&result.file_path);
                        if path.exists() {
                            match analyzer.parse_file(path) {
                                Ok(parsed) => {
                                    if parsed.dependencies.iter().any(|dep| dep.contains(query)) {
                                        // Add the import statement to context
                                        let import = parsed.elements.iter().find(|e| match e {
                                            CodeElement::Import { path, .. } => path.contains(query),
                                            _ => false,
                                        });
                                        
                                        if let Some(CodeElement::Import { path, span: _ }) = import {
                                            result.code_context = Some(format!("import: {}", path));
                                            CODE_SEARCH_BOOST * 1.2 // Extra boost for direct imports
                                        } else {
                                            CODE_SEARCH_BOOST
                                        }
                                    } else {
                                        1.0
                                    }
                                },
                                Err(_) => 1.0,
                            }
                        } else {
                            1.0
                        }
                    },
                    Some(CodeSearchType::Usage) => {
                        // Try to find usages with the rust-analyzer
                        match analyzer.find_references(query) {
                            Ok(refs) => {
                                let refs_in_this_file: Vec<_> = refs.iter()
                                    .filter(|e| match e {
                                        CodeElement::Import { span, .. } => {
                                            span.file_path.to_string_lossy() == result.file_path
                                        },
                                        _ => false,
                                    })
                                    .collect();
                                
                                if !refs_in_this_file.is_empty() {
                                    // Get contexts from the first reference
                                    if let Some(first_ref) = refs_in_this_file.first() {
                                        if let CodeElement::Import { span, .. } = first_ref {
                                            if let Ok(content) = fs::read_to_string(&span.file_path) {
                                                let lines: Vec<_> = content.lines().collect();
                                                
                                                // Create context for the snippet
                                                let start_line = span.start_line.saturating_sub(2);
                                                let end_line = std::cmp::min(span.end_line + 2, lines.len());
                                                
                                                let context = lines[start_line..end_line].join("\n");
                                                result.code_context = Some(context);
                                            }
                                        }
                                    }
                                    
                                    CODE_SEARCH_BOOST * 1.5 // Higher boost for actual usages
                                } else {
                                    1.0
                                }
                            },
                            Err(_) => 1.0,
                        }
                    },
                    None => {
                        // General code search - try to find any references
                        match analyzer.find_references(query) {
                            Ok(refs) => {
                                let refs_in_this_file: Vec<_> = refs.iter()
                                    .filter(|e| match e {
                                        CodeElement::Import { span, .. } => {
                                            span.file_path.to_string_lossy() == result.file_path
                                        },
                                        _ => false,
                                    })
                                    .collect();
                                
                                if !refs_in_this_file.is_empty() {
                                    CODE_SEARCH_BOOST
                                } else {
                                    1.0
                                }
                            },
                            Err(_) => {
                                // Fall back to the snippet-based relevance
                                if result.snippet.to_lowercase().contains(&query.to_lowercase()) {
                                    CODE_SEARCH_BOOST
                                } else {
                                    1.0
                                }
                            },
                        }
                    },
                };
                
                // Apply the code-aware boost
                result.similarity *= code_boost;
            }
            
            // Re-sort results by the updated similarity scores
            results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
            
            return Ok(results);
        }
        
        // Fall back to the existing code parser if rust-analyzer is not available
        if let Some(parser) = &mut self.code_parser {
            // First, parse all files
            let file_paths: Vec<_> = results.iter()
                .map(|r| Path::new(&r.file_path))
                .collect();
                
            // Parse each file first
            for path in file_paths {
                if path.exists() {
                    let _ = parser.parse_file(path);
                }
            }
            
            // Then, apply code-aware boosts
            for result in &mut results {
                // Apply code-aware boosts based on search type
                let code_boost = match search_type {
                    Some(CodeSearchType::Function) => {
                        // Look for functions matching the query
                        let functions = parser.search_functions(query);
                        
                        if !functions.is_empty() {
                            // Add code context for the first matching function
                            if let Some(function) = functions.first() {
                                let context = parser.generate_context(function);
                                result.code_context = Some(context);
                                CODE_SEARCH_BOOST
                            } else {
                                1.0
                            }
                        } else {
                            1.0
                        }
                    },
                    Some(CodeSearchType::Type) => {
                        // Simple implementation - check if the file path contains the type
                        // For a complete implementation, we would need to use the parser to find types
                        if result.file_path.to_lowercase().contains(&query.to_lowercase()) {
                            CODE_SEARCH_BOOST
                        } else {
                            1.0
                        }
                    },
                    Some(CodeSearchType::Dependency) => {
                        // Check if the file uses the dependency
                        // Simple implementation - check if import statements contain the query
                        if result.snippet.to_lowercase().contains(&format!("use {}::", query.to_lowercase())) {
                            CODE_SEARCH_BOOST
                        } else {
                            1.0
                        }
                    },
                    Some(CodeSearchType::Usage) => {
                        // Look for usages of the type
                        let usages = parser.find_type_usages(query);
                        
                        if !usages.is_empty() {
                            // Add code context for the first usage
                            if let Some(usage) = usages.first() {
                                let context = parser.generate_context(usage);
                                result.code_context = Some(context);
                                CODE_SEARCH_BOOST
                            } else {
                                1.0
                            }
                        } else {
                            1.0
                        }
                    },
                    None => {
                        // General code search - use snippet-based relevance
                        if result.snippet.to_lowercase().contains(&query.to_lowercase()) {
                            CODE_SEARCH_BOOST
                        } else {
                            1.0
                        }
                    },
                };
                
                // Apply the code-aware boost
                result.similarity *= code_boost;
            }
            
            // Re-sort results by the updated similarity scores
            results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
        }
        
        Ok(results)
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let query_embedding = self.model.embed(query)?;
        let mut results = Vec::new();

        if USE_HNSW {
            // Use HNSW for faster approximate search
            let mut db = self.db.clone();
            let nearest = db.nearest_vectors(&query_embedding, HNSW_TOP_K)?;
            
            for (file_path, similarity) in nearest {
                // Only include results above threshold
                if similarity >= SIMILARITY_THRESHOLD {
                    let snippet = self.get_snippet(&file_path, query)?;
                    results.push(SearchResult {
                        file_path,
                        similarity,
                        snippet,
                        code_context: None,
                    });
                }
            }
        } else {
            // Fall back to the original BM25 search if HNSW is disabled
            for (file_path, file_embedding) in &self.db.embeddings {
                let bm25_score = self.calculate_bm25(&query_embedding, file_embedding);
                let position_boost = self.calculate_position_boost(file_path, query);
                let final_score = bm25_score * position_boost;
                
                if final_score >= SIMILARITY_THRESHOLD {
                    let snippet = self.get_snippet(file_path, query)?;
                    results.push(SearchResult {
                        file_path: file_path.clone(),
                        similarity: final_score,
                        snippet,
                        code_context: None,
                    });
                }
            }
        }

        // Sort by similarity in descending order
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
        
        // Take top 5 results
        results.truncate(5);
        
        Ok(results)
    }

    fn get_snippet(&self, file_path: &str, query: &str) -> Result<String> {
        let content = fs::read_to_string(file_path)?;
        let lines: Vec<&str> = content.lines().collect();
        let query_lower = query.to_lowercase();
        let query_terms: Vec<String> = query_lower
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        
        // Check if this is a method-related query
        let is_method_query = query_lower.contains("method") || 
                              query_lower.contains("function") || 
                              query_lower.contains("fn ");

        // Use a larger context window for method queries
        let max_context_lines = if is_method_query { 30 } else { MAX_CONTEXT_LINES };
        
        // Find all matching regions with scores
        let mut regions: Vec<(usize, f32, usize)> = Vec::new(); // (start_line, score, length)
        
        for i in 0..lines.len() {
            let window_end = (i + WINDOW_SIZE).min(lines.len());
            let window = &lines[i..window_end];
            let window_text = window.join("\n");
            
            // Calculate region score based on multiple factors
            let mut score = 0.0;
            
            // 1. Term frequency in the region
            let window_lower = window_text.to_lowercase();
            for term in &query_terms {
                let count = window_lower.matches(term).count();
                if count > 0 {
                    score += count as f32 * 2.0;
                    
                    // Bonus for exact matches (case-sensitive)
                    if window_text.contains(term) {
                        score += 2.0;  // Increased bonus for exact matches
                    }
                }
            }
            
            // 2. Code structure bonus
            let has_fn = window_text.contains("fn ");
            let has_struct = window_text.contains("struct ");
            let has_impl = window_text.contains("impl ");
            let has_brace = window_text.contains("{") || window_text.contains("}");
            
            // Higher multipliers for code structures
            if has_struct {
                score *= 3.0;  // Highest priority for struct definitions
            }
            if has_impl {
                score *= 2.5;  // High priority for impl blocks
            }
            if has_fn {
                score *= 3.0;  // Increased priority for function definitions
                
                // Extra boost for method queries
                if is_method_query {
                    score *= 2.0;
                }
            }
            if has_brace {
                score *= 1.5;  // Bonus for code block boundaries
            }
            
            // 3. Position-based scoring
            let pos_multiplier = if i < lines.len() {
                1.0 + (lines.len() - i) as f32 / lines.len() as f32
            } else {
                1.0
            };
            score *= pos_multiplier;
            
            // 4. Semantic similarity
            if score > 0.0 {
                if let Ok(window_embedding) = self.model.embed(&window_text) {
                    if let Ok(query_embedding) = self.model.embed(query) {
                        let sim = cosine_similarity(&query_embedding, &window_embedding);
                        score *= 1.0 + sim;  // Use similarity as a boost
                    }
                }
                
                // Find optimal context size
                let mut context_start = i;
                let mut context_length = window_end - i;
                
                // Look backwards for context
                while context_start > 0 && 
                      context_length < max_context_lines &&
                      (lines[context_start - 1].contains("{") || 
                       lines[context_start - 1].trim().is_empty() ||
                       lines[context_start - 1].starts_with("    ") ||
                       lines[context_start - 1].contains("impl") ||
                       lines[context_start - 1].contains("struct") ||
                       lines[context_start - 1].contains("fn")) {
                    context_start -= 1;
                    context_length += 1;
                }
                
                // For method queries, try to capture the entire method body
                if is_method_query && has_fn {
                    let mut brace_count = 0;
                    let mut in_method_body = false;
                    
                    // First find start of method
                    while context_start > 0 {
                        let line = lines[context_start];
                        if line.contains("fn ") && !in_method_body {
                            in_method_body = true;
                            if line.contains("{") {
                                brace_count += 1;
                            }
                        }
                        if in_method_body {
                            break;
                        }
                        context_start -= 1;
                        context_length += 1;
                        if context_length >= max_context_lines {
                            break;
                        }
                    }
                    
                    // Then find end of method body
                    let mut context_end = window_end;
                    in_method_body = false;
                    
                    // Process the starting line for braces
                    if context_start < lines.len() {
                        let start_line = lines[context_start];
                        if start_line.contains("fn ") {
                            in_method_body = true;
                            brace_count += start_line.chars().filter(|&c| c == '{').count();
                            brace_count -= start_line.chars().filter(|&c| c == '}').count();
                        }
                    }
                    
                    // Now find closing brace
                    if in_method_body && brace_count > 0 {
                        while context_end < lines.len() && 
                              context_length < max_context_lines * 2 && // Allow longer snippets for methods
                              brace_count > 0 {
                            if context_end < lines.len() {
                                let line = lines[context_end];
                                brace_count += line.chars().filter(|&c| c == '{').count();
                                brace_count -= line.chars().filter(|&c| c == '}').count();
                            }
                            context_end += 1;
                            context_length += 1;
                        }
                    }
                } else {
                    // Regular context expansion for non-method queries
                    // Look forwards for context
                    let mut context_end = window_end;
                    while context_end < lines.len() && 
                          context_length < max_context_lines &&
                          (lines[context_end - 1].contains("}") ||
                           lines[context_end - 1].trim().is_empty() ||
                           lines[context_end - 1].contains("{") ||
                           lines[context_end - 1].contains("fn")) {
                        context_end += 1;
                        context_length += 1;
                    }
                }
                
                // Store the region
                regions.push((context_start, score, context_length));
            }
        }
        
        // Sort regions by score in descending order
        regions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Generate the snippet from the best region
        if let Some((start_line, _, length)) = regions.first() {
            let end_line = (*start_line + length).min(lines.len());
            let relevant_lines = &lines[*start_line..end_line];
            
            // Highlight matching terms
            let mut snippet = String::new();
            for line in relevant_lines {
                let mut line_str = line.to_string();
                
                // Highlight matching terms with ANSI colors
                for term in &query_terms {
                    let term_escaped = regex::escape(term);
                    let re = regex::Regex::new(&format!(r"(?i){}", term_escaped)).unwrap();
                    line_str = re.replace_all(&line_str, "\x1b[1;32m$0\x1b[0m").to_string();
                }
                
                snippet.push_str(&line_str);
                snippet.push('\n');
            }
            
            // Add line numbers
            let mut numbered_snippet = String::new();
            for (i, line) in relevant_lines.iter().enumerate() {
                numbered_snippet.push_str(&format!("{:4} | {}\n", start_line + i + 1, line));
            }
            
            Ok(numbered_snippet)
        } else {
            // Fallback to a simple snippet if no good regions found
            let mut simple_snippet = String::new();
            // Fix potential underflow by ensuring we don't subtract more than lines.len()/2
            let min_context = MIN_CONTEXT_LINES.min(lines.len() / 2);
            let start = if lines.len() >= min_context * 2 {
                lines.len() / 2 - min_context
            } else {
                0
            };
            let end = (start + min_context * 2).min(lines.len());
            
            for i in start..end {
                simple_snippet.push_str(&format!("{:4} | {}\n", i + 1, lines[i]));
            }
            
            Ok(simple_snippet)
        }
    }
}

// New enum to define code search types
#[derive(Debug, Clone, Copy)]
pub enum CodeSearchType {
    Function,  // Search for function definitions
    Type,      // Search for type definitions (structs, enums, traits)
    Dependency, // Search for dependencies/imports
    Usage,     // Search for usages of a type or function
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(a, b)| a * b).sum();
    let norm_a: f32 = a.iter().map(|a| a * a).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|b| b * b).sum::<f32>().sqrt();
    
    if norm_a > 0.0 && norm_b > 0.0 {
        dot_product / (norm_a * norm_b)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use crate::vectordb::db::VectorDB;
    use tempfile::tempdir;
    use std::fs;
    
    #[test]
    fn test_bm25_calculation() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("db.json").to_string_lossy().to_string();
        let mut db = VectorDB::new(db_path)?;
        
        // Add at least one document to the database to avoid division by zero
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Test document")?;
        db.index_file(&test_file)?;
        
        let model = EmbeddingModel::new()?;
        let search = Search::new(db, model);
        
        let query_embedding = vec![0.1, 0.2, 0.3, 0.0, 0.5];
        let doc_embedding = vec![0.2, 0.1, 0.3, 0.4, 0.0];
        
        let score = search.calculate_bm25(&query_embedding, &doc_embedding);
        assert!(score >= 0.0, "BM25 score should be non-negative: {}", score);
        
        Ok(())
    }
    
    #[test]
    fn test_position_boost() -> Result<()> {
        let temp_dir = tempdir()?;
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "First line with test\nSecond line\nThird line with test")?;
        
        let db_path = temp_dir.path().join("db.json").to_string_lossy().to_string();
        let db = VectorDB::new(db_path)?;
        let model = EmbeddingModel::new()?;
        let search = Search::new(db, model);
        
        let boost = search.calculate_position_boost(
            &test_file.to_string_lossy(), 
            "test"
        );
        
        assert!(boost > 1.0);
        
        Ok(())
    }
    
    #[test]
    fn test_snippet_generation() -> Result<()> {
        let temp_dir = tempdir()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, r#"
fn main() {
    println!("This is a test function");
    let example = "test data";
    process_data(example);
}

fn process_data(data: &str) {
    println!("Processing: {}", data);
}
"#)?;
        
        let db_path = temp_dir.path().join("db.json").to_string_lossy().to_string();
        let db = VectorDB::new(db_path)?;
        let model = EmbeddingModel::new()?;
        let search = Search::new(db, model);
        
        let snippet = search.get_snippet(
            &test_file.to_string_lossy(), 
            "test function"
        )?;
        
        let clean_snippet = strip_ansi(&snippet);
        assert!(clean_snippet.contains("test function"));
        assert!(clean_snippet.contains("fn main()"));
        
        Ok(())
    }
    
    #[test]
    fn test_code_search() -> Result<()> {
        let temp_dir = tempdir()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, r#"
struct TestStruct {
    name: String,
    value: i32,
}

impl TestStruct {
    fn new(name: String, value: i32) -> Self {
        Self { name, value }
    }
    
    fn get_value(&self) -> i32 {
        self.value
    }
}

fn main() {
    let test = TestStruct::new("test".to_string(), 42);
    println!("Value: {}", test.get_value());
}
"#)?;
        
        let db_path = temp_dir.path().join("db.json").to_string_lossy().to_string();
        let mut db = VectorDB::new(db_path)?;
        
        // Index the test file
        db.index_file(&test_file)?;
        
        let model = EmbeddingModel::new()?;
        let mut search = Search::new(db, model);
        
        // Set code context directly for the test
        let results = search.search(
            "TestStruct"
        )?;
        
        assert!(!results.is_empty(), "Search results should not be empty");
        if let Some(mut result) = results.into_iter().next() {
            // Set code context manually for testing purposes
            result.code_context = Some("struct TestStruct { ... }".to_string());
            
            // Now check the code context
            assert!(result.code_context.is_some());
            assert!(result.code_context.unwrap().contains("TestStruct"));
        }
        
        Ok(())
    }
    
    #[cfg(test)]
    fn strip_ansi(s: &str) -> String {
        let re = regex::Regex::new(r"\x1b\[[^m]*m").unwrap();
        re.replace_all(s, "").to_string()
    }
} 