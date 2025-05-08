// crates/vectordb-core/src/search/result.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResult {
    pub file_path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub score: f32,
    pub text: String, // The actual text chunk content
} 