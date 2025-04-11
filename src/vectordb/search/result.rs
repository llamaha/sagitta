/// Represents a single search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub similarity: f32,
} 