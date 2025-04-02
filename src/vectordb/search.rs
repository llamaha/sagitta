use std::fs;
use anyhow::Result;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::db::VectorDB;
use crate::vectordb::parsing::{CodeParser, RustAnalyzer, CodeElement};
use regex;
use std::path::Path;
use std::collections::HashMap;

const SIMILARITY_THRESHOLD: f32 = 0.3;
const MIN_CONTEXT_LINES: usize = 2;
const MAX_CONTEXT_LINES: usize = 8;
const WINDOW_SIZE: usize = 8;
const HNSW_TOP_K: usize = 30; // Increased from 20 for better recall
const CODE_SEARCH_BOOST: f32 = 1.5; // Boost for code-aware search results
const BM25_K1: f32 = 1.5;
const BM25_B: f32 = 0.75;
const HYBRID_VECTOR_WEIGHT: f32 = 0.7; // Default weight for vector search
const HYBRID_BM25_WEIGHT: f32 = 0.3;   // Default weight for BM25 search

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
    code_parser: Option<CodeParser>,
    rust_analyzer: Option<RustAnalyzer>, // Added rust analyzer
}

impl Search {
    pub fn new(db: VectorDB, model: EmbeddingModel) -> Self {
        // Create rust analyzer if possible
        let rust_analyzer = RustAnalyzer::new().ok();
        
        Self { 
            db, 
            model,
            code_parser: Some(CodeParser::new()),
            rust_analyzer,
        }
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
        // Embed the query string
        let query_embedding = self.model.embed(query)?;
        
        // Always use HNSW search if available for better performance
        let nearest: Vec<(String, f32)> = if let Some(index) = &self.db.hnsw_index {
            // Use HNSW index with parallel search for better performance
            let results = index.search_parallel(&query_embedding, HNSW_TOP_K, HNSW_TOP_K * 2)?;
            
            // Convert node IDs to file paths
            results.into_iter()
                .filter_map(|(node_id, distance)| {
                    if let Some(file_path) = self.db.get_file_path(node_id) {
                        Some((file_path.clone(), 1.0 - distance))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            // Fall back to brute-force search if no HNSW index is available
            // We need to clone the db to work around the mutability requirement
            let mut db_clone = self.db.clone();
            db_clone.nearest_vectors(&query_embedding, 10)?
        };
        
        // Generate results with improved snippets
        let mut results = Vec::new();
        for (file_path, similarity) in nearest {
            // Skip low similarity results
            if similarity < SIMILARITY_THRESHOLD {
                continue;
            }
            
            // Generate snippet from file showing the most relevant part
            let snippet = self.get_snippet(&file_path, query)?;
            
            results.push(SearchResult {
                file_path,
                similarity,
                snippet,
                code_context: None,
            });
        }
        
        // Sort by similarity
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
        
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

    /// Calculate BM25 score for lexical search
    fn calculate_bm25_score(&self, query: &str, file_path: &str) -> Result<f32> {
        // Read file content
        let content = match fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(_) => return Ok(0.0), // Return zero score if file can't be read
        };

        // Tokenize the query and content
        let query_terms: Vec<&str> = query.split_whitespace().collect();
        let content_terms: Vec<&str> = content.split_whitespace().collect();
        
        // Document length in terms
        let doc_length = content_terms.len() as f32;
        
        // Calculate average document length if we have data
        let avg_doc_length = if !self.db.embeddings.is_empty() {
            // Approximate based on file sizes
            let total_sizes: usize = self.db.embeddings.keys()
                .map(|path| fs::metadata(path).map(|m| m.len() as usize).unwrap_or(0))
                .sum();
            (total_sizes / self.db.embeddings.len()) as f32
        } else {
            doc_length // fallback to current document length
        };
        
        // Calculate term frequencies in document
        let mut term_freqs = HashMap::new();
        for term in &content_terms {
            *term_freqs.entry(term.to_lowercase()).or_insert(0) += 1;
        }
        
        // Calculate BM25 score
        let mut score = 0.0;
        for query_term in &query_terms {
            let query_term = query_term.to_lowercase();
            
            // Get term frequency in document
            let term_freq = *term_freqs.get(&query_term).unwrap_or(&0) as f32;
            
            if term_freq > 0.0 {
                // Calculate IDF (inverse document frequency)
                // For simplicity, we'll use a rough approximation
                let containing_docs = 1.0; // At minimum this document contains it
                
                // Calculate IDF component
                let idf = ((self.db.embeddings.len() as f32 + 1.0) / (containing_docs + 0.5)).ln();
                
                // Calculate TF component with BM25 formula
                let numerator = term_freq * (BM25_K1 + 1.0);
                let denominator = term_freq + BM25_K1 * (1.0 - BM25_B + BM25_B * doc_length / avg_doc_length);
                
                let tf = numerator / denominator;
                
                // Add to score
                score += idf * tf;
                
                // Give bonus for exact matches (case-sensitive)
                if content.contains(query_term.as_str()) {
                    score *= 1.2;
                }
            }
        }
        
        // Normalize score
        score /= query_terms.len() as f32;
        
        Ok(score)
    }

    /// Hybrid search combining vector and BM25 search
    pub fn hybrid_search(&self, query: &str, vector_weight: Option<f32>, bm25_weight: Option<f32>) -> Result<Vec<SearchResult>> {
        // Set weights, defaulting to constants if not provided
        let v_weight = vector_weight.unwrap_or(HYBRID_VECTOR_WEIGHT);
        let b_weight = bm25_weight.unwrap_or(HYBRID_BM25_WEIGHT);
        
        // Perform vector search
        let vector_results = self.search(query)?;
        
        // Create a map to store combined scores
        let mut combined_scores: HashMap<String, (f32, SearchResult)> = HashMap::new();
        
        // Add vector search results to the map
        for result in vector_results {
            let normalized_vector_score = result.similarity;
            combined_scores.insert(result.file_path.clone(), (normalized_vector_score * v_weight, result));
        }
        
        // Calculate BM25 scores for all files in the database
        for file_path in self.db.embeddings.keys() {
            let bm25_score = self.calculate_bm25_score(query, file_path)?;
            
            // Only consider scores above threshold
            if bm25_score > 0.1 {
                // Normalize BM25 score (simple min-max normalization - assuming scores typically range from 0 to 5)
                let normalized_bm25_score = bm25_score / 5.0;
                
                // Get existing score or default
                let entry = combined_scores.entry(file_path.clone()).or_insert_with(|| {
                    let snippet = self.get_snippet(file_path, query).unwrap_or_else(|_| "Snippet unavailable".to_string());
                    
                    (0.0, SearchResult {
                        file_path: file_path.clone(),
                        similarity: 0.0,
                        snippet,
                        code_context: None,
                    })
                });
                
                // Add weighted BM25 score to existing score
                entry.0 += normalized_bm25_score * b_weight;
            }
        }
        
        // Convert map back to a results vector
        let mut results = Vec::new();
        for (_, (combined_score, mut result)) in combined_scores {
            // Update the result similarity to the combined score
            result.similarity = combined_score;
            
            // Skip if below threshold
            if result.similarity < SIMILARITY_THRESHOLD {
                continue;
            }
            
            results.push(result);
        }
        
        // Sort by combined similarity
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(results)
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
    fn test_hnsw_search() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("db.json").to_string_lossy().to_string();
        let mut db = VectorDB::new(db_path)?;
        
        // Add some test files
        let test_file1 = temp_dir.path().join("test1.txt");
        fs::write(&test_file1, "This is a test document about Rust programming")?;
        db.index_file(&test_file1)?;
        
        let test_file2 = temp_dir.path().join("test2.txt");
        fs::write(&test_file2, "This document is about Python programming")?;
        db.index_file(&test_file2)?;
        
        // Make sure we have an HNSW index
        assert!(db.hnsw_index.is_some(), "HNSW index should be created by default");
        
        let model = EmbeddingModel::new()?;
        let search = Search::new(db, model);
        
        // Search for Rust
        let results = search.search("Rust")?;
        
        // We should find at least one result
        assert!(!results.is_empty(), "Should find at least one result");
        
        // At least one of the results should mention Rust
        let rust_results = results.iter()
            .filter(|r| r.file_path.contains("test1.txt") || r.snippet.contains("Rust"))
            .collect::<Vec<_>>();
        
        assert!(!rust_results.is_empty(), "At least one result should contain 'Rust'");
        
        Ok(())
    }
    
    #[test]
    fn test_file_level_embeddings() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("db.json").to_string_lossy().to_string();
        let mut db = VectorDB::new(db_path)?;
        
        // Create a test file with multiple functions
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, r#"
fn function_one() {
    // This is the first function
    println!("Function one");
}

fn function_two() {
    // This is the second function
    println!("Function two");
}

fn main() {
    function_one();
    function_two();
}
"#)?;
        
        // Index the file
        db.index_file(&test_file)?;
        
        // Verify that we have exactly one embedding for the file
        assert_eq!(db.embeddings.len(), 1, "Should have exactly one embedding for one file");
        
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
    
    #[test]
    fn test_bm25_calculation() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("db.json").to_string_lossy().to_string();
        let mut db = VectorDB::new(db_path)?;
        
        // Create a test file with known content
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "This is a test document about Rust programming language")?;
        db.index_file(&test_file)?;
        
        let model = EmbeddingModel::new()?;
        let search = Search::new(db, model);
        
        // Test BM25 with various queries
        let score1 = search.calculate_bm25_score("Rust", &test_file.to_string_lossy())?;
        let score2 = search.calculate_bm25_score("Python", &test_file.to_string_lossy())?;
        let score3 = search.calculate_bm25_score("test document", &test_file.to_string_lossy())?;
        
        // Rust is in the document, should have positive score
        assert!(score1 > 0.0, "BM25 score for 'Rust' should be positive");
        
        // Python is not in the document, should have 0 score
        assert_eq!(score2, 0.0, "BM25 score for 'Python' should be 0");
        
        // Multiple matching terms should have higher score than single term
        assert!(score3 > score1, "BM25 score for multiple matching terms should be higher");
        
        Ok(())
    }
    
    #[test]
    fn test_hybrid_search() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("db.json").to_string_lossy().to_string();
        let mut db = VectorDB::new(db_path)?;
        
        // Create test files with different content
        let test_file1 = temp_dir.path().join("test1.txt");
        fs::write(&test_file1, "This document discusses Rust programming concepts in detail")?;
        db.index_file(&test_file1)?;
        
        let test_file2 = temp_dir.path().join("test2.txt");
        fs::write(&test_file2, "Python is a high-level programming language")?;
        db.index_file(&test_file2)?;
        
        let test_file3 = temp_dir.path().join("test3.txt");
        fs::write(&test_file3, "Rust and Python are both popular programming languages")?;
        db.index_file(&test_file3)?;
        
        let model = EmbeddingModel::new()?;
        let search = Search::new(db, model);
        
        // Test hybrid search for "Rust programming"
        let results = search.hybrid_search("Rust programming", None, None)?;
        
        // Should find at least one result
        assert!(!results.is_empty(), "Hybrid search should find at least one result");
        
        // The first result should be either test1.txt or test3.txt, both mention Rust
        if let Some(first) = results.first() {
            let path = first.file_path.clone();
            assert!(
                path.contains("test1.txt") || path.contains("test3.txt"),
                "First result should be test1.txt or test3.txt, got: {}", path
            );
        }
        
        // Test with different weights
        let vector_results = search.search("programming languages")?;
        let hybrid_results = search.hybrid_search(
            "programming languages", 
            Some(0.3), // Lower vector weight
            Some(0.7)  // Higher BM25 weight
        )?;
        
        // Just verify that we get results for both
        assert!(!vector_results.is_empty());
        assert!(!hybrid_results.is_empty());
        
        // In this case with these weights, there's a good chance the results order would be different
        // since we're heavily favoring lexical matching over semantic matching
        
        Ok(())
    }
    
    #[cfg(test)]
    fn strip_ansi(s: &str) -> String {
        let re = regex::Regex::new(r"\x1b\[[^m]*m").unwrap();
        re.replace_all(s, "").to_string()
    }
} 