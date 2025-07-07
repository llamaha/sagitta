use super::types::{CommitInfo, GitHistoryState, GraphNode, GraphEdge};
use std::collections::{HashMap, HashSet};

/// Find an available lane for a commit
pub fn find_available_lane(lane_tracker: &mut Vec<Option<String>>, commit_id: &str, parents: &[String]) -> usize {
    // Check if this commit is already assigned to a lane
    for (i, lane) in lane_tracker.iter().enumerate() {
        if lane.as_ref() == Some(&commit_id.to_string()) {
            return i;
        }
    }
    
    // First check if any parent lane can be reused
    for parent in parents {
        for (i, lane) in lane_tracker.iter().enumerate() {
            if lane.as_ref() == Some(parent) {
                lane_tracker[i] = Some(commit_id.to_string());
                return i;
            }
        }
    }
    
    // Look for an empty lane
    for (i, lane) in lane_tracker.iter().enumerate() {
        if lane.is_none() {
            lane_tracker[i] = Some(commit_id.to_string());
            return i;
        }
    }
    
    // Add a new lane
    lane_tracker.push(Some(commit_id.to_string()));
    lane_tracker.len() - 1
}

/// Calculate the graph layout for commits
pub fn calculate_graph_layout(state: &mut GitHistoryState) {
    state.graph_nodes.clear();
    state.graph_edges.clear();
    
    let mut lane_tracker: Vec<Option<String>> = Vec::new();
    let mut commit_lanes: HashMap<String, usize> = HashMap::new();
    
    // Calculate nodes
    for (idx, commit) in state.commits.iter().enumerate() {
        let lane = find_available_lane(&mut lane_tracker, &commit.id, &commit.parents);
        commit_lanes.insert(commit.id.clone(), lane);
        
        state.graph_nodes.push(GraphNode {
            commit_id: commit.id.clone(),
            x: lane as f32 * 20.0,
            y: idx as f32 * 40.0,
            lane,
        });
        
        // Create edges to parents
        for parent_id in &commit.parents {
            let parent_lane = if let Some(&existing_lane) = commit_lanes.get(parent_id) {
                existing_lane
            } else {
                // For linear history, parent should use the same lane as the child
                // Only use a different lane if the current lane is already occupied
                let parent_lane = if commit.parents.len() == 1 && lane_tracker.get(lane).and_then(|l| l.as_ref()) == Some(&commit.id) {
                    // Linear history case: parent inherits child's lane
                    lane_tracker[lane] = Some(parent_id.clone());
                    lane
                } else {
                    // Branching case: find a new lane for parent
                    find_available_lane(&mut lane_tracker, parent_id, &[])
                };
                commit_lanes.insert(parent_id.clone(), parent_lane);
                parent_lane
            };
            
            state.graph_edges.push(GraphEdge {
                from: commit.id.clone(),
                to: parent_id.clone(),
                from_lane: lane,
                to_lane: parent_lane,
            });
        }
    }
}