use anyhow::Result;
// Removed unused import
// use regex::Regex;
use std::fs;
use std::path::Path;
// use super::code_structure::{CodeStructureAnalyzer, CodeContext, MethodInfo, TypeInfo};

const DEFAULT_CONTEXT_LINES: usize = 5;

/// Structure to hold context information for a code snippet
#[derive(Debug, Clone)]
pub struct SnippetContext {
    pub snippet_text: String,
    // Removed unused fields
    // pub start_line: usize,
    // pub end_line: usize,
    // pub file_path: String,
    // pub is_definition: bool,
    // pub is_usage: bool,
}

/// Simple snippet extractor based on content matching
pub struct SnippetExtractor {}

impl SnippetExtractor {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Extract a relevant snippet from a file based on the query
    // Note: This now only uses content-based extraction
    pub fn extract_snippet(&mut self, file_path: &str, query: &str) -> Result<SnippetContext> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(anyhow::anyhow!("File does not exist: {}", file_path));
        }
        
        // Read file content
        let content = fs::read_to_string(path)?;
        
        // Find the most relevant code section using query terms
        let query_terms: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        
        // Use content-based matching directly
        self.extract_content_based_snippet(&content, file_path, &query_terms)
    }
    
    // Fallback snippet extraction based on query term location
    fn extract_content_based_snippet(&self, content: &str, _file_path: &str, query_terms: &[String]) -> Result<SnippetContext> {
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return Ok(SnippetContext {
                snippet_text: "".to_string(),
                // Fields removed
            });
        }

        // Find the line with the highest score based on query terms
        let mut best_line_index = 0;
        let mut max_score = 0.0;

        for (i, line) in lines.iter().enumerate() {
            let score = Self::calculate_line_score(line, query_terms);
            if score > max_score {
                max_score = score;
                best_line_index = i;
            }
        }

        // Calculate context window around the best line
        let start_context = best_line_index.saturating_sub(DEFAULT_CONTEXT_LINES);
        let end_context = (best_line_index + DEFAULT_CONTEXT_LINES + 1).min(lines.len());

        let snippet_start_line = start_context;
        let snippet_end_line = end_context;

        // Build the snippet text
        let mut snippet = String::new();
        if snippet_start_line > 0 {
            snippet.push_str("... (truncated above)\n");
        }
        for i in snippet_start_line..snippet_end_line {
            snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
        }
        if snippet_end_line < lines.len() {
            snippet.push_str("... (truncated below)\n");
        }

        Ok(SnippetContext {
            snippet_text: snippet,
            // Fields removed
        })
    }

    /// Calculate a relevance score for a line based on query terms
    fn calculate_line_score(line: &str, query_terms: &[String]) -> f32 {
        let line_lower = line.to_lowercase();
        let mut score = 0.0;

        for term in query_terms {
            if line_lower.contains(term) {
                score += 1.0;
                // Bonus for exact word match
                if line_lower.split_whitespace().any(|word| word == term.as_str()) {
                    score += 1.0;
                }
            }
        }

        // Normalize score by line length (prefer shorter lines with matches)
        if !line.is_empty() {
            score / (line.len() as f32).sqrt()
        } else {
            0.0
        }
    }
    
    // Removed unused method highlight_snippet

    // Removed structure-aware methods: extract_method_snippet, extract_type_snippet,
    // find_matching_method, find_matching_type, extract_method_usage_snippet, clear_cache
}

#[cfg(test)]
mod tests {
    use super::*;

    // Removed test test_highlight_snippet
    
    // Helper to create a temporary file with content
    fn create_temp_file(content: &str) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_extract_content_based_snippet() -> Result<()> {
        let content = "Line 1\nLine 2: Important keyword\nLine 3\nLine 4: Another important thing\nLine 5";
        let file = create_temp_file(content);
        let mut extractor = SnippetExtractor::new();
        
        // Test with a specific keyword
        let snippet_context = extractor.extract_snippet(file.path().to_str().unwrap(), "keyword")?;
        assert!(snippet_context.snippet_text.contains("Line 2: Important keyword"));
        // Removed assertions using removed fields
        // assert!(snippet_context.start_line <= 2 && snippet_context.end_line >= 2);
        println!("Snippet for 'keyword':\n{}", snippet_context.snippet_text);

        // Test with another keyword
        let snippet_context_2 = extractor.extract_snippet(file.path().to_str().unwrap(), "thing")?;
        assert!(snippet_context_2.snippet_text.contains("Line 4: Another important thing"));
        // Removed assertions using removed fields
        // assert!(snippet_context_2.start_line <= 4 && snippet_context_2.end_line >= 4);
        println!("Snippet for 'thing':\n{}", snippet_context_2.snippet_text);

        Ok(())
    }

    #[test]
    fn test_extract_from_empty_file() -> Result<()> {
        let content = "";
        let file = create_temp_file(content);
        let mut extractor = SnippetExtractor::new();
        let snippet_context = extractor.extract_snippet(file.path().to_str().unwrap(), "anything")?;
        assert!(snippet_context.snippet_text.is_empty());
        // Removed assertions using removed fields
        // assert_eq!(snippet_context.start_line, 1);
        // assert_eq!(snippet_context.end_line, 1);
        Ok(())
    }
}
