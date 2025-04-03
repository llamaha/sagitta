use std::path::{Path, PathBuf};
use std::fs;
use anyhow::Result;
use log::{debug, info, warn, error};
use regex::Regex;
use super::code_structure::{CodeStructureAnalyzer, CodeContext, MethodInfo, TypeInfo};

const DEFAULT_CONTEXT_LINES: usize = 5;
const MAX_CONTEXT_LINES: usize = 15;
const MIN_CONTEXT_LINES: usize = 2;

/// Structure to hold context information for a code snippet
#[derive(Debug, Clone)]
pub struct SnippetContext {
    pub snippet_text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub file_path: String,
    pub relevant_method: Option<MethodInfo>,
    pub relevant_type: Option<TypeInfo>,
    pub is_definition: bool,
    pub is_usage: bool,
}

/// Improved snippet extractor with awareness of code structure
pub struct SnippetExtractor {
    code_analyzer: CodeStructureAnalyzer,
}

impl SnippetExtractor {
    pub fn new() -> Self {
        Self {
            code_analyzer: CodeStructureAnalyzer::new(),
        }
    }
    
    /// Extract a relevant snippet from a file based on the query
    pub fn extract_snippet(&mut self, file_path: &str, query: &str) -> Result<SnippetContext> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(anyhow::anyhow!("File does not exist: {}", file_path));
        }
        
        // Read file content
        let content = fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();
        
        // Try to find the most relevant code section
        let query_terms: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        
        // Get code structure information
        let context_result = self.code_analyzer.analyze_file(file_path);
        
        if let Ok(code_context) = context_result {
            // Try to find matching methods or types first
            // Get the method and type matches without holding the borrow
            let method_match = Self::find_matching_method(code_context, &query_terms);
            let type_match = Self::find_matching_type(code_context, &query_terms);
            
            // If we found a structure match, use it for the snippet
            if let Some(method) = method_match {
                return self.extract_method_snippet(&lines, &method, file_path);
            }
            
            if let Some(type_info) = type_match {
                return self.extract_type_snippet(&lines, &type_info, file_path);
            }
        }
        
        // Fall back to content-based matching
        self.extract_content_based_snippet(&content, file_path, &query_terms)
    }
    
    // Move the method matching logic to a separate function that doesn't use self
    fn find_matching_method(context: &CodeContext, query_terms: &[String]) -> Option<MethodInfo> {
        let mut best_match = None;
        let mut best_score = 0;
        
        for method in &context.methods {
            let mut score = 0;
            let method_name = method.name.to_lowercase();
            
            for term in query_terms {
                if method_name.contains(term) {
                    score += 2;
                }
                
                if method.signature.to_lowercase().contains(term) {
                    score += 1;
                }
            }
            
            // Prioritize public methods
            if method.is_public {
                score += 1;
            }
            
            if score > best_score {
                best_score = score;
                best_match = Some(method.clone());
            }
        }
        
        best_match
    }
    
    // Move the type matching logic to a separate function that doesn't use self
    fn find_matching_type(context: &CodeContext, query_terms: &[String]) -> Option<TypeInfo> {
        let mut best_match = None;
        let mut best_score = 0;
        
        for type_info in &context.types {
            let mut score = 0;
            let type_name = type_info.name.to_lowercase();
            
            for term in query_terms {
                if type_name.contains(term) {
                    score += 2;
                }
            }
            
            if score > best_score {
                best_score = score;
                best_match = Some(type_info.clone());
            }
        }
        
        best_match
    }
    
    /// Extract a snippet around a method
    fn extract_method_snippet(&self, lines: &[&str], method: &MethodInfo, file_path: &str) -> Result<SnippetContext> {
        let (start_line, end_line) = method.span;
        
        // Find the method boundaries - try to find end of method
        let mut actual_end_line = end_line;
        let mut brace_count = 0;
        
        // Simple approach - count braces or look for end keyword
        for (i, line) in lines.iter().enumerate().skip(end_line.saturating_sub(1)) {
            if i >= lines.len() {
                break;
            }
            
            // For Ruby, look for 'end' keyword
            if line.trim() == "end" {
                actual_end_line = i + 1;
                break;
            }
            
            // For C-like languages, count braces
            for c in line.chars() {
                if c == '{' {
                    brace_count += 1;
                } else if c == '}' {
                    brace_count -= 1;
                    if brace_count <= 0 {
                        actual_end_line = i + 1;
                        break;
                    }
                }
            }
            
            actual_end_line = i + 1;
        }
        
        // Expand the context around the method
        let context_lines = DEFAULT_CONTEXT_LINES.min(lines.len());
        let start = start_line.saturating_sub(context_lines);
        let end = (actual_end_line + context_lines).min(lines.len());
        
        // Extract the snippet with line numbers
        let mut snippet = String::new();
        for i in start..end {
            snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
        }
        
        Ok(SnippetContext {
            snippet_text: snippet,
            start_line: start + 1,
            end_line: end,
            file_path: file_path.to_string(),
            relevant_method: Some(method.clone()),
            relevant_type: None,
            is_definition: true,
            is_usage: false,
        })
    }
    
    /// Extract a snippet around a type definition
    fn extract_type_snippet(&self, lines: &[&str], type_info: &TypeInfo, file_path: &str) -> Result<SnippetContext> {
        let (start_line, end_line) = type_info.span;
        
        // Determine the end of the type definition
        let mut actual_end_line = end_line;
        let mut brace_count = 0;
        
        // Similar approach to methods
        for (i, line) in lines.iter().enumerate().skip(end_line.saturating_sub(1)) {
            if i >= lines.len() {
                break;
            }
            
            if line.trim() == "end" {
                actual_end_line = i + 1;
                break;
            }
            
            for c in line.chars() {
                if c == '{' {
                    brace_count += 1;
                } else if c == '}' {
                    brace_count -= 1;
                    if brace_count <= 0 {
                        actual_end_line = i + 1;
                        break;
                    }
                }
            }
            
            actual_end_line = i + 1;
        }
        
        // Limit the size of large type definitions
        if actual_end_line - start_line > MAX_CONTEXT_LINES * 2 {
            actual_end_line = start_line + MAX_CONTEXT_LINES;
        }
        
        // Add context around the type definition
        let context_lines = DEFAULT_CONTEXT_LINES.min(lines.len());
        let start = start_line.saturating_sub(context_lines);
        let end = (actual_end_line + 2).min(lines.len()); // Show a bit of the type body
        
        // Extract the snippet with line numbers
        let mut snippet = String::new();
        for i in start..end {
            snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
        }
        
        if actual_end_line < lines.len() && end < lines.len() {
            snippet.push_str("... (truncated)\n");
        }
        
        Ok(SnippetContext {
            snippet_text: snippet,
            start_line: start + 1,
            end_line: end,
            file_path: file_path.to_string(),
            relevant_method: None,
            relevant_type: Some(type_info.clone()),
            is_definition: true,
            is_usage: false,
        })
    }
    
    /// Extract a snippet based on content matching
    fn extract_content_based_snippet(&self, content: &str, file_path: &str, query_terms: &[String]) -> Result<SnippetContext> {
        let lines: Vec<&str> = content.lines().collect();
        
        // Find the best matching line
        let mut best_line = 0;
        let mut best_score = 0;
        
        for (i, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();
            let mut score = 0;
            
            for term in query_terms {
                if line_lower.contains(term) {
                    score += 1;
                }
            }
            
            // Boost lines with specific code indicators
            if line_lower.contains("fn ") || line_lower.contains("def ") || 
               line_lower.contains("class ") || line_lower.contains("struct ") ||
               line_lower.contains("trait ") || line_lower.contains("module ") {
                score += 1;
            }
            
            if score > best_score {
                best_score = score;
                best_line = i;
            }
        }
        
        // If no match found, just return the beginning of the file
        if best_score == 0 {
            best_line = 0;
        }
        
        // Extract context around the best line
        let context_lines = DEFAULT_CONTEXT_LINES.min(lines.len());
        let start = best_line.saturating_sub(context_lines);
        let end = (best_line + context_lines + 1).min(lines.len());
        
        // Extract the snippet with line numbers
        let mut snippet = String::new();
        for i in start..end {
            snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
        }
        
        Ok(SnippetContext {
            snippet_text: snippet,
            start_line: start + 1,
            end_line: end,
            file_path: file_path.to_string(),
            relevant_method: None,
            relevant_type: None,
            is_definition: false,
            is_usage: best_score > 0,
        })
    }
    
    /// Highlight query terms in the snippet
    pub fn highlight_snippet(&self, snippet: &str, query_terms: &[String]) -> String {
        let mut result = snippet.to_string();
        
        // Use regex to find and highlight terms
        for term in query_terms {
            if term.len() <= 2 {
                continue; // Skip very short terms
            }
            
            let pattern = format!(r"(?i)\b{}\b", regex::escape(term));
            if let Ok(regex) = Regex::new(&pattern) {
                result = regex.replace_all(&result, |caps: &regex::Captures| {
                    format!("**{}**", &caps[0])
                }).to_string();
            }
        }
        
        result
    }
    
    /// Extract a more focused snippet for a specific method or function
    pub fn extract_method_usage_snippet(&mut self, file_path: &str, method_name: &str) -> Result<SnippetContext> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(anyhow::anyhow!("File does not exist: {}", file_path));
        }
        
        // Read file content
        let content = fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();
        
        // Create a pattern to find method usage
        let patterns = [
            format!(r"\b{}\s*\(", method_name),         // function call
            format!(r"\.\s*{}\s*\(", method_name),      // method call
            format!(r"\bself\.\s*{}\s*\(", method_name), // self method call
        ];
        
        let mut best_line = 0;
        let mut found_match = false;
        
        // Try to find method usage
        for pattern in &patterns {
            if let Ok(regex) = Regex::new(pattern) {
                for (i, line) in lines.iter().enumerate() {
                    if regex.is_match(line) {
                        best_line = i;
                        found_match = true;
                        break;
                    }
                }
            }
            
            if found_match {
                break;
            }
        }
        
        // If not found, try a simple match
        if !found_match {
            for (i, line) in lines.iter().enumerate() {
                if line.contains(method_name) {
                    best_line = i;
                    found_match = true;
                    break;
                }
            }
        }
        
        // Extract context around the best line
        let context_lines = DEFAULT_CONTEXT_LINES.min(lines.len());
        let start = best_line.saturating_sub(context_lines);
        let end = (best_line + context_lines + 1).min(lines.len());
        
        // Extract the snippet with line numbers
        let mut snippet = String::new();
        for i in start..end {
            snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
        }
        
        Ok(SnippetContext {
            snippet_text: snippet,
            start_line: start + 1,
            end_line: end,
            file_path: file_path.to_string(),
            relevant_method: None,
            relevant_type: None,
            is_definition: false,
            is_usage: true,
        })
    }
    
    /// Clear the analyzer cache
    pub fn clear_cache(&mut self) {
        self.code_analyzer.clear_cache();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_highlight_snippet() {
        let snippet = "This is a test function that does something important.";
        let query_terms = vec!["function".to_string(), "important".to_string()];
        
        let extractor = SnippetExtractor::new();
        let highlighted = extractor.highlight_snippet(snippet, &query_terms);
        
        assert!(highlighted.contains("**function**"));
        assert!(highlighted.contains("**important**"));
    }
} 