use anyhow::Result;

/// Represents a distinct chunk of code extracted from a file.
#[derive(Debug, Clone)] // Added Clone
pub struct CodeChunk {
    /// The textual content of the code chunk.
    pub content: String,
    /// The path to the file this chunk belongs to.
    pub file_path: String,
    /// The starting line number of the chunk (1-based).
    pub start_line: usize,
    /// The ending line number of the chunk (1-based, inclusive).
    pub end_line: usize,
    /// The detected language of the code chunk (e.g., "rust", "python").
    pub language: String,       // e.g., "rust", "python", "fallback"
    /// The type of code element this chunk represents (e.g., "function", "struct").
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