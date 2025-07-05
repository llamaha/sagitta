use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const DEFAULT_CONTEXT_LINES: usize = 3; // Number of context lines above/below

/// Extracts a snippet of text from a file, centered around the given line range,
/// optionally adding context lines above and below.
///
/// # Arguments
/// * `file_path` - Path to the file.
/// * `chunk_start_line` - The 1-indexed starting line of the core chunk.
/// * `chunk_end_line` - The 1-indexed ending line of the core chunk.
///
/// # Returns
/// A `Result` containing the formatted snippet string (with line numbers and truncation markers),
/// or an error if the file cannot be read or lines are invalid.
pub fn extract_snippet(file_path: &str, chunk_start_line: usize, chunk_end_line: usize) -> Result<String> {
    let path = Path::new(file_path);
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file for snippet extraction: {file_path}"))?;
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if chunk_start_line == 0 || chunk_end_line == 0 || chunk_start_line > chunk_end_line || chunk_end_line > total_lines {
        return Err(anyhow::anyhow!(
            "Invalid line range [{}, {}] for file {} with {} lines",
            chunk_start_line, chunk_end_line, file_path, total_lines
        ));
    }

    // Calculate context window (0-indexed)
    let core_start_idx = chunk_start_line - 1;
    let core_end_idx = chunk_end_line - 1;

    let context_start_idx = core_start_idx.saturating_sub(DEFAULT_CONTEXT_LINES);
    // Add 1 to core_end_idx because end is exclusive in range, then add context lines
    let context_end_idx = (core_end_idx + 1 + DEFAULT_CONTEXT_LINES).min(total_lines);

    let mut snippet = String::new();

    // Add lines with numbers (use 1-based indexing for display)
    for (i, line) in lines.iter().enumerate().take(context_end_idx).skip(context_start_idx) {
        // Maybe highlight the core chunk lines?
        let line_prefix = if i >= core_start_idx && i <= core_end_idx {
            // Indicate core chunk lines (optional)
            format!("\n{:>4} | ", i + 1) // Line number for core line
        } else {
            format!("\n{:>4} : ", i + 1) // Line number for context line
        };
        snippet.push_str(&line_prefix);
        snippet.push_str(line);
    }

    // Add truncation markers if necessary
    let mut final_snippet = String::new();
    if context_start_idx > 0 {
        final_snippet.push_str("  ...\n");
    }
    final_snippet.push_str(&snippet);
    if context_end_idx < total_lines {
        // Add newline only if snippet wasn't empty
        if !snippet.is_empty() { 
            final_snippet.push('\n'); 
        }
        final_snippet.push_str("  ...");
    }

    Ok(final_snippet.trim_end().to_string())
}

/// Represents the type of code element a snippet corresponds to.
/// TODO: Align this with the string element_types used by SyntaxParser implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementType {
    /// A standalone function.
    Function,
    /// A struct definition.
    Struct,
    /// A class definition.
    Class,
    /// A method within a class or impl.
    Method,
    /// An interface definition.
    Interface,
    /// An enum definition.
    Enum,
    /// A type alias (e.g., `type MyType = ...`).
    TypeAlias,
    /// A constant definition (`const`).
    Const,
    /// A static variable definition (`static`).
    Static,
    /// A module definition (`mod`).
    Module,
    /// A trait definition.
    Trait,
    /// An implementation block (`impl`).
    Impl,
    /// A macro definition or invocation.
    Macro,
    /// A use/import statement.
    Use,
    /// An extern crate declaration.
    ExternCrate,
    /// A test function (`#[test]`).
    Test,
    /// A chunk from a file where specific elements couldn't be parsed (fallback).
    FileChunk,
    /// An unknown or uncategorized element.
    Unknown,
}

/// Represents a code snippet extracted by the syntax parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    /// The content of the code snippet.
    pub content: String,
    /// The programming language of the snippet.
    pub language: String,
    /// The 1-based starting line number in the original file.
    pub start_line: usize,
    /// The 1-based ending line number (inclusive) in the original file.
    pub end_line: usize,
    /// The type of code element this snippet represents.
    pub element_type: ElementType,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // Helper to create a temporary file with content
    fn create_temp_file(content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_extract_basic_snippet() -> Result<()> {
        let content = "Line 1\nLine 2\nLine 3 - Core\nLine 4 - Core\nLine 5\nLine 6\nLine 7";
        let file = create_temp_file(content);
        let snippet = extract_snippet(file.path().to_str().unwrap(), 3, 4)?;

        assert!(snippet.contains(" 3 | Line 3 - Core"));
        assert!(snippet.contains(" 4 | Line 4 - Core"));
        // Check context lines (default 3)
        assert!(snippet.contains(" 1 : Line 1"));
        assert!(snippet.contains(" 2 : Line 2"));
        assert!(snippet.contains(" 5 : Line 5"));
        assert!(snippet.contains(" 6 : Line 6"));
        assert!(snippet.contains(" 7 : Line 7"));
        // Check no truncation markers needed
        assert!(!snippet.contains("..."));

        Ok(())
    }

    #[test]
    fn test_extract_snippet_with_truncation() -> Result<()> {
        let content = (1..=20).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
        let file = create_temp_file(&content);
        let file_path = file.path().to_str().unwrap().to_string();

        // Check start truncation
        let snippet_start = extract_snippet(&file_path, 1, 2)?;
        assert!(!snippet_start.starts_with("..."), "Snippet start should not start with ...");
        assert!(snippet_start.ends_with("\n  ..."), "Snippet start should end with truncation marker");

        // Check end truncation
        let snippet_end = extract_snippet(&file_path, 19, 20)?;
        assert!(snippet_end.starts_with("  ..."), "Snippet end should start with truncation marker");
        assert!(!snippet_end.ends_with("..."), "Snippet end should not have extra trailing marker");

        // Check middle (both truncations)
        let snippet_middle = extract_snippet(&file_path, 8, 10)?;
        assert!(snippet_middle.starts_with("  ..."), "Snippet middle should start with truncation marker");
        assert!(snippet_middle.ends_with("\n  ..."), "Snippet middle should end with truncation marker");

        Ok(())
    }

     #[test]
     fn test_extract_invalid_lines() -> Result<()> {
         let content = "Line 1\nLine 2";
         let file = create_temp_file(content);
         let path_str = file.path().to_str().unwrap();

         assert!(extract_snippet(path_str, 0, 1).is_err(), "Start line 0 should fail");
         assert!(extract_snippet(path_str, 1, 0).is_err(), "End line 0 should fail");
         assert!(extract_snippet(path_str, 2, 1).is_err(), "Start > End should fail");
         assert!(extract_snippet(path_str, 1, 3).is_err(), "End > Total lines should fail");
         assert!(extract_snippet(path_str, 3, 3).is_err(), "Start > Total lines should fail");

         Ok(())
     }
} 