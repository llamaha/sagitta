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
            
            if brace_count <= 0 && i > end_line {
                actual_end_line = i + 1;
                break;
            }
        }
        
        // Calculate the method size and limit if too large
        let method_size = actual_end_line - start_line;
        let max_method_size = 30; // Maximum size for showing entire method
        
        let context_lines = if method_size > max_method_size {
            // For large methods, show smaller context
            2
        } else {
            // For smaller methods, use default context
            DEFAULT_CONTEXT_LINES.min(lines.len())
        };
        
        // Add context around the method
        let start = start_line.saturating_sub(context_lines);
        
        // Handle large methods appropriately
        let mut snippet = String::new();
        
        if method_size > max_method_size {
            // For large methods, show the signature plus beginning and end
            let signature_end = start_line + 5.min(method_size); // Show signature + few lines
            
            // Show method signature and beginning
            for i in start..signature_end {
                snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
            }
            
            // Add truncation indicator
            snippet.push_str("... [method body truncated] ...\n");
            
            // Show end of method
            let end_section_start = actual_end_line.saturating_sub(5);
            for i in end_section_start..actual_end_line {
                if i < lines.len() {
                    snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
                }
            }
            
            // Show additional context after the method
            let end = (actual_end_line + context_lines).min(lines.len());
            for i in actual_end_line..end {
                if i < lines.len() {
                    snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
                }
            }
        } else {
            // For smaller methods, show the entire method with context
            let end = (actual_end_line + context_lines).min(lines.len());
            
            // Add truncation indicator if needed
            if start > 0 {
                snippet.push_str("... (truncated above)\n");
            }
            
            // Show the full method and context
            for i in start..end {
                snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
            }
            
            // Add truncation indicator if needed
            if end < lines.len() {
                snippet.push_str("... (truncated below)\n");
            }
        }
        
        Ok(SnippetContext {
            snippet_text: snippet,
            start_line: start + 1,
            end_line: actual_end_line,
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
            
            if brace_count <= 0 && i > end_line {
                actual_end_line = i + 1;
                break;
            }
        }
        
        // Calculate the type size and limit if too large
        let type_size = actual_end_line - start_line;
        let max_type_size = 35; // Maximum size for showing entire type definition
        
        let context_lines = if type_size > max_type_size {
            // For large types, show smaller context
            2
        } else {
            // For smaller types, use default context
            DEFAULT_CONTEXT_LINES.min(lines.len())
        };
        
        // Add context around the type definition
        let start = start_line.saturating_sub(context_lines);
        
        // Handle large type definitions appropriately
        let mut snippet = String::new();
        
        if type_size > max_type_size {
            // For large types, show the declaration plus beginning and end
            let declaration_end = start_line + 8.min(type_size); // Show declaration + few lines
            
            // Show type declaration and beginning
            for i in start..declaration_end {
                snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
            }
            
            // Add truncation indicator
            snippet.push_str("... [type body truncated] ...\n");
            
            // Show end of type definition
            let end_section_start = actual_end_line.saturating_sub(5);
            for i in end_section_start..actual_end_line {
                if i < lines.len() {
                    snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
                }
            }
            
            // Show additional context after the type
            let end = (actual_end_line + context_lines).min(lines.len());
            for i in actual_end_line..end {
                if i < lines.len() {
                    snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
                }
            }
        } else {
            // For smaller types, show the entire definition with context
            let end = (actual_end_line + context_lines).min(lines.len());
            
            // Add truncation indicator if needed
            if start > 0 {
                snippet.push_str("... (truncated above)\n");
            }
            
            // Show the full type and context
            for i in start..end {
                snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
            }
            
            // Add truncation indicator if needed
            if end < lines.len() {
                snippet.push_str("... (truncated below)\n");
            }
        }
        
        Ok(SnippetContext {
            snippet_text: snippet,
            start_line: start + 1,
            end_line: actual_end_line,
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
        
        // Find all matching lines with their scores
        let mut scored_lines: Vec<(usize, usize)> = Vec::new(); // (line_index, score)
        
        for (i, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();
            let mut score = 0;
            
            // Score based on query terms
            for term in query_terms {
                if line_lower.contains(term) {
                    score += 1;
                }
            }
            
            // Boost lines with specific code indicators
            if line_lower.contains("fn ") || line_lower.contains("def ") || 
               line_lower.contains("class ") || line_lower.contains("struct ") ||
               line_lower.contains("trait ") || line_lower.contains("module ") ||
               line_lower.contains("impl ") || line_lower.contains("pub ") {
                score += 2; // Higher boost for important code elements
            }
            
            // Only consider lines with positive scores
            if score > 0 {
                scored_lines.push((i, score));
            }
        }
        
        // Sort by score in descending order
        scored_lines.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Find the best section with multiple matched lines if possible
        let absolute_max_lines = 35; // Maximum snippet size
        let context_lines = DEFAULT_CONTEXT_LINES.min(lines.len());
        
        // If we have multiple matches, try to find the best cluster
        if scored_lines.len() > 1 {
            // Group nearby matches to find the most relevant section of code
            let mut best_section_start = 0;
            let mut best_section_end = 0;
            let mut best_section_score = 0;
            
            // Try each high-scoring line as a potential center of a section
            for &(line_idx, line_score) in scored_lines.iter().take(5) { // Consider top 5 matches
                let section_start = line_idx.saturating_sub(context_lines);
                let section_end = (line_idx + context_lines + 1).min(lines.len());
                
                // Count how many other matched lines fall within this section
                let section_score = scored_lines.iter()
                    .filter(|&&(idx, _)| idx >= section_start && idx < section_end)
                    .map(|&(_, score)| score)
                    .sum::<usize>() + line_score; // Include the center line's score
                
                if section_score > best_section_score {
                    best_section_score = section_score;
                    best_section_start = section_start;
                    best_section_end = section_end;
                }
            }
            
            // Use the best section if we found one
            if best_section_score > 0 {
                // Hard limit on length
                let section_length = best_section_end - best_section_start;
                let max_lines = absolute_max_lines.min(section_length);
                
                if section_length > max_lines {
                    // If section is too large, center it around the highest scoring line in the section
                    let best_line_in_section = scored_lines.iter()
                        .filter(|&&(idx, _)| idx >= best_section_start && idx < best_section_end)
                        .map(|&(idx, _)| idx)
                        .next()
                        .unwrap_or(best_section_start + section_length / 2);
                    
                    let half_max = max_lines / 2;
                    best_section_start = best_line_in_section.saturating_sub(half_max);
                    best_section_end = (best_line_in_section + half_max).min(lines.len());
                }
                
                // Extract the snippet
                let mut snippet = String::new();
                for i in best_section_start..best_section_end {
                    snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
                }
                
                // Add truncation indicator if appropriate
                if best_section_start > 0 {
                    snippet.insert_str(0, "... (truncated above)\n");
                }
                if best_section_end < lines.len() {
                    snippet.push_str("... (truncated below)\n");
                }
                
                return Ok(SnippetContext {
                    snippet_text: snippet,
                    start_line: best_section_start + 1,
                    end_line: best_section_end,
                    file_path: file_path.to_string(),
                    relevant_method: None,
                    relevant_type: None,
                    is_definition: false,
                    is_usage: true,
                });
            }
        }
        
        // Fallback: If we don't have good clusters, use the best single line
        let best_line = if !scored_lines.is_empty() {
            scored_lines[0].0 // Use the highest scoring line
        } else {
            0 // Default to first line if no matches
        };
        
        // Extract context around the best line
        let start = best_line.saturating_sub(context_lines);
        let end = (best_line + context_lines + 1).min(lines.len());
        
        // Hard limit on snippet size
        let max_lines = absolute_max_lines.min(lines.len());
        let actual_end = end.min(start + max_lines);
        
        // Extract the snippet with line numbers
        let mut snippet = String::new();
        for i in start..actual_end {
            snippet.push_str(&format!("{}: {}\n", i + 1, lines[i]));
        }
        
        // Add truncation indicators
        if start > 0 {
            snippet.insert_str(0, "... (truncated above)\n");
        }
        if actual_end < lines.len() {
            snippet.push_str("... (truncated below)\n");
        }
        
        Ok(SnippetContext {
            snippet_text: snippet,
            start_line: start + 1,
            end_line: actual_end,
            file_path: file_path.to_string(),
            relevant_method: None,
            relevant_type: None,
            is_definition: false,
            is_usage: !scored_lines.is_empty(),
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