// crates/sagitta-search/src/search/result.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a single search result from semantic code search.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResult {
    /// Path to the file containing the matching code.
    pub file_path: PathBuf,
    /// Starting line number of the matching code chunk (0-indexed).
    pub start_line: usize,
    /// Ending line number of the matching code chunk (0-indexed).
    pub end_line: usize,
    /// Relevance score for this result (higher is more relevant).
    pub score: f32,
    /// The actual text content of the matching code chunk.
    pub text: String,
} 