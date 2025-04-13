use anyhow::Result;

/// Represents a distinct chunk of code extracted from a file.
#[derive(Debug, Clone)] // Added Clone
pub struct CodeChunk {
    pub content: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,       // e.g., "rust", "python", "fallback"
    pub element_type: String, // e.g., "function", "struct", "fallback_chunk"
}

/// Trait for syntax-aware parsers.
/// Implementations of this trait will handle specific languages.
pub trait SyntaxParser {
    /// Parses the given code string and extracts relevant code chunks.
    ///
    /// # Arguments
    ///
    /// * `code` - The source code content as a string.
    /// * `file_path` - The path to the original file (used for context in `CodeChunk`).
    ///
    /// # Returns
    ///
    /// A `Result` containing a vector of `CodeChunk`s on success, or an error.
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>>;
} 