mod modal;
mod types;
mod graph;

pub use modal::GitHistoryModal;
pub use types::{CommitInfo, GitHistoryState, GraphNode, GraphEdge};
pub use graph::{calculate_graph_layout, find_available_lane};

#[cfg(test)]
mod tests;