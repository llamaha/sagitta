use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub id: String,
    pub short_id: String,
    pub message: String,
    pub author: String,
    pub email: String,
    pub timestamp: DateTime<Utc>,
    pub parents: Vec<String>,
    pub branch_refs: Vec<String>,
}

#[derive(Debug, Default)]
pub struct GitHistoryState {
    pub commits: Vec<CommitInfo>,
    pub commit_map: HashMap<String, usize>,
    pub show_all_branches: bool,
    pub max_commits: usize,
}