use std::fs;
use anyhow::Result;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::db::VectorDB;
use std::collections::HashMap;
use regex;

const SIMILARITY_THRESHOLD: f32 = 0.3;
const MIN_CONTEXT_LINES: usize = 2;
const MAX_CONTEXT_LINES: usize = 8;
const BM25_K1: f32 = 1.5;
const BM25_B: f32 = 0.75;
const POSITION_BOOST: f32 = 0.2;
const WINDOW_SIZE: usize = 8;
const USE_HNSW: bool = true;
const HNSW_TOP_K: usize = 20;

#[derive(Debug)]
pub struct SearchResult {
    pub file_path: String,
    pub similarity: f32,
    pub snippet: String,
}

pub struct Search {
    db: VectorDB,
    model: EmbeddingModel,
    avg_doc_length: f32,
}

impl Search {
    pub fn new(db: VectorDB, model: EmbeddingModel) -> Self {
        // Calculate average document length
        let total_length: usize = db.embeddings.values()
            .map(|embedding| embedding.iter().filter(|&&x| x > 0.0).count())
            .sum();
        let avg_doc_length = total_length as f32 / db.embeddings.len() as f32;
        
        Self { 
            db, 
            model,
            avg_doc_length,
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
                      context_length < MAX_CONTEXT_LINES &&
                      (lines[context_start - 1].contains("{") || 
                       lines[context_start - 1].trim().is_empty() ||
                       lines[context_start - 1].starts_with("    ") ||
                       lines[context_start - 1].contains("impl") ||
                       lines[context_start - 1].contains("struct") ||
                       lines[context_start - 1].contains("fn")) {
                    context_start -= 1;
                    context_length += 1;
                }
                
                // Look forwards for context
                let mut context_end = window_end;
                while context_end < lines.len() && 
                      context_length < MAX_CONTEXT_LINES &&
                      (lines[context_end - 1].contains("}") ||
                       lines[context_end - 1].trim().is_empty() ||
                       lines[context_end - 1].contains("{") ||
                       lines[context_end - 1].contains("fn")) {
                    context_end += 1;
                    context_length += 1;
                }
                
                // Ensure we include the entire function body
                if has_fn {
                    let mut brace_count: i32 = 0;
                    for line in &lines[context_start..context_end] {
                        brace_count = brace_count.saturating_add(line.chars().filter(|&c| c == '{').count() as i32);
                        brace_count = brace_count.saturating_sub(line.chars().filter(|&c| c == '}').count() as i32);
                    }
                    
                    // If braces are unbalanced, try to extend the context
                    while brace_count > 0 && context_end < lines.len() && context_length < MAX_CONTEXT_LINES {
                        let line = lines[context_end];
                        brace_count = brace_count.saturating_sub(line.chars().filter(|&c| c == '}').count() as i32);
                        brace_count = brace_count.saturating_add(line.chars().filter(|&c| c == '{').count() as i32);
                        context_end += 1;
                        context_length += 1;
                    }
                    
                    // If we still have unbalanced braces, keep extending until we find the closing brace
                    while brace_count > 0 && context_end < lines.len() {
                        let line = lines[context_end];
                        brace_count = brace_count.saturating_sub(line.chars().filter(|&c| c == '}').count() as i32);
                        brace_count = brace_count.saturating_add(line.chars().filter(|&c| c == '{').count() as i32);
                        context_end += 1;
                        context_length += 1;
                    }
                }
                
                // Ensure we don't go out of bounds
                context_end = context_end.min(lines.len());
                context_length = context_end - context_start;
                
                // Additional scoring for function definitions
                if has_fn {
                    let fn_text = &lines[i..context_end].join("\n");
                    if fn_text.contains("fn ") && fn_text.contains("{") && fn_text.contains("}") {
                        score *= 1.5;  // Bonus for complete function definitions
                    }
                }
                
                // Check if this region contains the exact function we're looking for
                let region_text = &lines[context_start..context_end].join("\n");
                if query_terms.iter().all(|term| region_text.contains(term)) {
                    score *= 2.0;  // Significant bonus for exact matches
                }
                
                regions.push((context_start, score, context_length));
            }
        }
        
        // Sort regions by score and take the best one
        regions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        if let Some((start_idx, _, length)) = regions.first() {
            let mut snippet = String::new();
            let end_idx = (*start_idx + length).min(lines.len());
            
            // Add file path and line numbers as header
            snippet.push_str(&format!("// {} (lines {}-{})\n", file_path, start_idx + 1, end_idx));
            
            // Add the lines with highlighting
            for i in *start_idx..end_idx {
                let line = lines[i];
                let mut highlighted = line.to_string();
                
                // Highlight matching terms
                for term in &query_terms {
                    if let Some(idx) = highlighted.to_lowercase().find(term) {
                        let end_idx = idx + term.len();
                        let term_case = &highlighted[idx..end_idx];
                        highlighted = highlighted.replace(term_case, &format!("\x1b[1;32m{}\x1b[0m", term_case));
                    }
                }
                
                snippet.push_str(&format!("{:>4} | {}\n", i + 1, highlighted));
            }
            
            Ok(snippet)
        } else {
            // Fallback to simple context if no good regions found
            let middle = lines.len() / 2;
            let start = middle.saturating_sub(MIN_CONTEXT_LINES);
            let end = (middle + MIN_CONTEXT_LINES).min(lines.len());
            
            let mut snippet = String::new();
            for i in start..end {
                snippet.push_str(&format!("{:>4} | {}\n", i + 1, lines[i]));
            }
            
            Ok(snippet)
        }
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot_product / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectordb::db::VectorDB;
    use anyhow::Result;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_bm25_calculation() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("db.json");
        let mut db = VectorDB::new(db_path.to_str().unwrap().to_string())?;
        let model = EmbeddingModel::new()?;
        
        // Create test files with content
        let file1_path = dir.path().join("test1.txt");
        let mut file1 = File::create(&file1_path)?;
        file1.write_all(b"This is a test file containing important code snippets.")?;
        
        // Index the file
        db.index_file(&file1_path)?;
        
        let search = Search::new(db, model);
        
        // Create test vectors that simulate real embeddings
        let query = vec![0.5, 0.3, 0.8, 0.1, 0.9];  // Simulated query embedding
        let doc = vec![0.4, 0.2, 0.7, 0.2, 0.8];    // Simulated document embedding
        
        let score = search.calculate_bm25(&query, &doc);
        assert!(score > 0.0, "BM25 score should be positive for similar vectors");
        Ok(())
    }

    #[test]
    fn test_position_boost() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("db.json");
        let db = VectorDB::new(db_path.to_str().unwrap().to_string())?;
        let model = EmbeddingModel::new()?;
        let search = Search::new(db, model);
        
        // Create a test file with content
        let test_file = dir.path().join("test.txt");
        let mut file = File::create(&test_file)?;
        file.write_all(b"This is a test file. The query term appears early.")?;
        
        let boost = search.calculate_position_boost(test_file.to_str().unwrap(), "query");
        assert!(boost >= 1.0);
        assert!(boost <= 1.0 + POSITION_BOOST);
        Ok(())
    }

    #[test]
    fn test_snippet_generation() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("db.json");
        let db = VectorDB::new(db_path.to_str().unwrap().to_string())?;
        let model = EmbeddingModel::new()?;
        let search = Search::new(db, model);
        
        // Create a test file with Rust-like content
        let test_file = dir.path().join("test.rs");
        let mut file = File::create(&test_file)?;
        let content = "use std::collections::HashMap;

struct User {
    name: String,
    age: u32,
}

impl User {
    fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}

fn main() {
    let user = User::new(\"Alice\".to_string(), 30);
    println!(\"Name: {}\", user.get_name());
}";
        file.write_all(content.as_bytes())?;
        
        // Helper function to strip ANSI color codes
        fn strip_ansi(s: &str) -> String {
            let re = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
            re.replace_all(s, "").to_string()
        }
        
        // Test snippet generation with different queries
        println!("Testing 'struct User' query...");
        let snippet = search.get_snippet(test_file.to_str().unwrap(), "struct User")?;
        println!("Generated snippet:\n{}", snippet);
        let clean_snippet = strip_ansi(&snippet);
        assert!(clean_snippet.contains("struct User"), "Snippet should contain 'struct User'");
        assert!(clean_snippet.contains("name: String"), "Snippet should contain struct fields");
        
        println!("\nTesting 'fn get_name' query...");
        let snippet = search.get_snippet(test_file.to_str().unwrap(), "fn get_name")?;
        println!("Generated snippet:\n{}", snippet);
        let clean_snippet = strip_ansi(&snippet);
        assert!(clean_snippet.contains("fn get_name"), "Snippet should contain 'fn get_name'");
        assert!(clean_snippet.contains("&self.name"), "Snippet should contain function body");
        
        println!("\nTesting 'impl User' query...");
        let snippet = search.get_snippet(test_file.to_str().unwrap(), "impl User")?;
        println!("Generated snippet:\n{}", snippet);
        let clean_snippet = strip_ansi(&snippet);
        assert!(clean_snippet.contains("impl User"), "Snippet should contain 'impl User'");
        assert!(clean_snippet.contains("fn new"), "Snippet should contain nearby methods");
        
        Ok(())
    }
} 