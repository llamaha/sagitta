use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct GraphNode {
    pub commit_id: String,
    pub x: f32,
    pub y: f32,
    pub lane: usize,
}

#[derive(Debug, Clone, Default)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub from_lane: usize,
    pub to_lane: usize,
}

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
    pub selected_commit: Option<String>,
    pub hovered_commit: Option<String>,
    pub graph_nodes: Vec<GraphNode>,
    pub graph_edges: Vec<GraphEdge>,
}