use serde::{Serialize, Deserialize};

/// Represents a single search result chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Path to the file containing the chunk.
    pub file_path: String,
    /// Starting line number (1-indexed).
    pub start_line: usize,
    /// Ending line number (1-indexed).
    pub end_line: usize,
    /// The text content of the chunk or relevant snippet.
    pub text: String,
    /// Similarity score (higher is better, typically 0.0 to 1.0).
    pub score: f32,
} 