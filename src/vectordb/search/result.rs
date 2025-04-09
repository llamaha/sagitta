/// Represents a single search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub similarity: f32,
    pub snippet: String,
    /// Optional repository name, used for grouping/boosting.
    pub repository: Option<String>,
} 