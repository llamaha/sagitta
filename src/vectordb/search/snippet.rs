use crate::vectordb::error::{Result, VectorDBError};
use std::fs;
use std::path::Path;

// Constants for snippet context
const SNIPPET_LINES_ABOVE: usize = 5;
const SNIPPET_LINES_BELOW: usize = 25;

/// Fallback function to get snippet from file matching the query
pub(crate) fn get_snippet(file_path: &str, query: &str) -> Result<String> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(VectorDBError::FileNotFound(file_path.to_string()));
    }

    let content = fs::read_to_string(path).map_err(|e| VectorDBError::FileReadError {
        path: path.to_path_buf(),
        source: e,
    })?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return Ok("(Empty file)".to_string());
    }

    // Try to find the best matching section
    let query_lowercase = query.to_lowercase();
    let query_terms: Vec<&str> = query_lowercase.split_whitespace().collect();
    let mut best_score = 0;
    let mut best_line: usize = 0;

    for (i, line) in lines.iter().enumerate() {
        let line_lower = line.to_lowercase();
        let mut score = 0;

        for term in &query_terms {
            if line_lower.contains(term) {
                score += 1;
            }
        }

        if score > best_score {
            best_score = score;
            best_line = i;
        }
    }

    // If no match found, show the beginning of the file
    if best_score == 0 {
        best_line = 0;
    }

    // Extract context around the matching line
    let start = best_line.saturating_sub(SNIPPET_LINES_ABOVE);
    let end = (best_line + SNIPPET_LINES_BELOW + 1).min(lines.len());

    // Create the snippet
    let mut snippet = String::new();
    for i in start..end {
        let line_num = i + 1; // 1-indexed line numbers
        let line_text = lines[i];
        snippet.push_str(&format!("{}: {}\n", line_num, line_text));
    }

    // Add indicators if we truncated the file
    if start > 0 {
        snippet.insert_str(0, "... (truncated)\n");
    }

    if end < lines.len() {
        snippet.push_str("... (truncated)\n");
    }

    Ok(snippet)
} 