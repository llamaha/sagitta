use egui::{Ui, Pos2, Rect, Sense, Color32, Stroke, Vec2};
use crate::gui::theme::AppTheme;
use super::types::{GitHistoryState, CommitNode};

const NODE_RADIUS: f32 = 6.0;
const LANE_WIDTH: f32 = 30.0;
const ROW_HEIGHT: f32 = 40.0;
const PADDING: f32 = 10.0;

pub fn render_commit_graph(ui: &mut Ui, state: &mut GitHistoryState, theme: AppTheme) {
    // Calculate graph layout if not already done
    if state.graph_nodes.is_empty() && !state.commits.is_empty() {
        calculate_graph_layout(state);
    }

    let graph_width = calculate_graph_width(state);
    let graph_height = state.commits.len() as f32 * ROW_HEIGHT + 2.0 * PADDING;
    
    let (response, painter) = ui.allocate_painter(
        Vec2::new(graph_width, graph_height),
        Sense::click()
    );
    
    let rect = response.rect;
    
    // Check if clicked on empty space to deselect
    if response.clicked() {
        let click_pos = response.interact_pointer_pos();
        if let Some(pos) = click_pos {
            let mut clicked_on_node = false;
            
            // Check if click was on any node
            for (i, _) in state.commits.iter().enumerate() {
                let node = &state.graph_nodes[i];
                let node_pos = Pos2::new(
                    rect.left() + PADDING + node.x,
                    rect.top() + PADDING + node.y
                );
                let node_rect = Rect::from_center_size(node_pos, Vec2::splat(NODE_RADIUS * 2.0));
                if node_rect.contains(pos) {
                    clicked_on_node = true;
                    break;
                }
            }
            
            // If clicked on empty space, deselect
            if !clicked_on_node {
                state.selected_commit = None;
            }
        }
    }
    
    // Draw edges first (behind nodes)
    draw_edges(&painter, rect, state, theme);
    
    // Draw nodes and labels
    for (i, commit) in state.commits.iter().enumerate() {
        let node = &state.graph_nodes[i];
        let pos = Pos2::new(
            rect.left() + PADDING + node.x,
            rect.top() + PADDING + node.y
        );
        
        // Check if node is hovered
        let node_rect = Rect::from_center_size(pos, Vec2::splat(NODE_RADIUS * 2.0));
        let is_hovered = ui.input(|i| i.pointer.hover_pos())
            .is_some_and(|cursor_pos| node_rect.contains(cursor_pos));
        
        if is_hovered {
            state.hovered_commit = Some(commit.id.clone());
        } else if state.hovered_commit.as_ref() == Some(&commit.id) {
            state.hovered_commit = None;
        }
        
        // Check for click - toggle selection if clicking on already selected commit
        if is_hovered && response.clicked() {
            if state.selected_commit.as_ref() == Some(&commit.id) {
                state.selected_commit = None;
            } else {
                state.selected_commit = Some(commit.id.clone());
            }
        }
        
        // Draw node
        let is_selected = state.selected_commit.as_ref() == Some(&commit.id);
        draw_node(&painter, pos, is_selected, is_hovered, theme);
        
        // Draw commit info
        draw_commit_label(&painter, pos, commit, theme);
        
        // Draw branch labels
        if !commit.branch_refs.is_empty() {
            draw_branch_labels(&painter, pos, &commit.branch_refs, theme);
        }
    }
    
    // Show tooltip on hover
    if let Some(hovered_id) = &state.hovered_commit {
        if let Some(commit) = state.commits.iter().find(|c| c.id == *hovered_id) {
            egui::containers::popup::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new("commit_tooltip"), |ui| {
                ui.label(format!("Commit: {}", commit.short_id));
                ui.label(format!("Author: {}", commit.author));
                ui.label(format!("Date: {}", commit.timestamp.format("%Y-%m-%d %H:%M")));
                ui.separator();
                ui.label(&commit.message);
            });
        }
    }
}

pub fn calculate_graph_layout(state: &mut GitHistoryState) {
    state.graph_nodes.clear();
    
    for (i, commit) in state.commits.iter().enumerate() {
        // For a simple linear layout, all commits are in lane 0
        let lane = 0;
        
        // Create node
        let node = CommitNode {
            commit: commit.clone(),
            x: lane as f32 * LANE_WIDTH,
            y: i as f32 * ROW_HEIGHT,
            lane,
        };
        
        state.graph_nodes.push(node);
    }
}

pub fn find_available_lane(
    lane_tracker: &mut Vec<Option<String>>,
    commit_id: &str,
    parents: &[String]
) -> usize {
    // Try to reuse parent's lane
    for (lane, occupied_by) in lane_tracker.iter().enumerate() {
        if let Some(id) = occupied_by {
            if parents.contains(id) {
                lane_tracker[lane] = Some(commit_id.to_string());
                return lane;
            }
        }
    }
    
    // Find first available lane
    for (lane, occupied_by) in lane_tracker.iter_mut().enumerate() {
        if occupied_by.is_none() {
            *occupied_by = Some(commit_id.to_string());
            return lane;
        }
    }
    
    // Add new lane
    lane_tracker.push(Some(commit_id.to_string()));
    lane_tracker.len() - 1
}

fn calculate_graph_width(state: &GitHistoryState) -> f32 {
    let max_lane = state.graph_nodes
        .iter()
        .map(|n| n.lane)
        .max()
        .unwrap_or(0);
    
    (max_lane + 1) as f32 * LANE_WIDTH + 2.0 * PADDING + 300.0 // Extra space for labels
}

fn draw_edges(
    painter: &egui::Painter,
    rect: Rect,
    state: &GitHistoryState,
    theme: AppTheme
) {
    let stroke = Stroke::new(2.0, theme.hint_text_color());
    
    for (i, commit) in state.commits.iter().enumerate() {
        let node = &state.graph_nodes[i];
        let from_pos = Pos2::new(
            rect.left() + PADDING + node.x,
            rect.top() + PADDING + node.y
        );
        
        // Draw edges to parents
        for parent_id in &commit.parents {
            if let Some(parent_idx) = state.commit_map.get(parent_id) {
                if *parent_idx < state.graph_nodes.len() {
                    let parent_node = &state.graph_nodes[*parent_idx];
                    let to_pos = Pos2::new(
                        rect.left() + PADDING + parent_node.x,
                        rect.top() + PADDING + parent_node.y
                    );
                    
                    // Draw curved line if lanes differ
                    if node.lane != parent_node.lane {
                        draw_curved_edge(painter, from_pos, to_pos, stroke);
                    } else {
                        painter.line_segment([from_pos, to_pos], stroke);
                    }
                }
            }
        }
    }
}

fn draw_curved_edge(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let control1 = Pos2::new(from.x, from.y + (to.y - from.y) * 0.3);
    let control2 = Pos2::new(to.x, from.y + (to.y - from.y) * 0.7);
    
    // Simple bezier approximation with line segments
    let steps = 10;
    let mut points = vec![];
    
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let inv_t = 1.0 - t;
        
        let x = inv_t.powi(3) * from.x 
            + 3.0 * inv_t.powi(2) * t * control1.x
            + 3.0 * inv_t * t.powi(2) * control2.x
            + t.powi(3) * to.x;
            
        let y = inv_t.powi(3) * from.y 
            + 3.0 * inv_t.powi(2) * t * control1.y
            + 3.0 * inv_t * t.powi(2) * control2.y
            + t.powi(3) * to.y;
            
        points.push(Pos2::new(x, y));
    }
    
    for i in 0..points.len() - 1 {
        painter.line_segment([points[i], points[i + 1]], stroke);
    }
}

fn draw_node(
    painter: &egui::Painter,
    pos: Pos2,
    is_selected: bool,
    is_hovered: bool,
    theme: AppTheme
) {
    let color = if is_selected {
        theme.accent_color()
    } else if is_hovered {
        theme.button_background()
    } else {
        theme.button_background()
    };
    
    painter.circle_filled(pos, NODE_RADIUS, color);
    
    // Draw border
    let border_color = if is_selected {
        theme.accent_color()
    } else {
        theme.hint_text_color()
    };
    painter.circle_stroke(pos, NODE_RADIUS, Stroke::new(2.0, border_color));
}

fn draw_commit_label(
    painter: &egui::Painter,
    pos: Pos2,
    commit: &super::types::CommitInfo,
    theme: AppTheme
) {
    let text_pos = pos + Vec2::new(NODE_RADIUS + 10.0, -8.0);
    
    // Short ID
    painter.text(
        text_pos,
        egui::Align2::LEFT_CENTER,
        &commit.short_id,
        egui::FontId::monospace(12.0),
        theme.accent_color(),
    );
    
    // Message (truncated)
    let message = if commit.message.chars().count() > 50 {
        let truncated: String = commit.message.chars().take(47).collect();
        format!("{}...", truncated)
    } else {
        commit.message.clone()
    };
    
    painter.text(
        text_pos + Vec2::new(60.0, 0.0),
        egui::Align2::LEFT_CENTER,
        message,
        egui::FontId::default(),
        theme.text_color(),
    );
    
    // Author and time
    let time_str = commit.timestamp.format("%m/%d %H:%M").to_string();
    let author_str = if commit.author.chars().count() > 15 {
        let truncated: String = commit.author.chars().take(12).collect();
        format!("{}...", truncated)
    } else {
        commit.author.clone()
    };
    
    painter.text(
        text_pos + Vec2::new(0.0, 16.0),
        egui::Align2::LEFT_CENTER,
        format!("{author_str} • {time_str}"),
        egui::FontId::proportional(11.0),
        theme.hint_text_color(),
    );
}

fn draw_branch_labels(
    painter: &egui::Painter,
    pos: Pos2,
    branches: &[String],
    theme: AppTheme
) {
    let mut x_offset = 300.0;
    
    for branch in branches {
        let text_pos = pos + Vec2::new(x_offset, 0.0);
        
        // Draw branch badge background
        let text_size = painter.text(
            text_pos,
            egui::Align2::LEFT_CENTER,
            branch,
            egui::FontId::proportional(11.0),
            Color32::TRANSPARENT, // Measure only
        );
        
        let padding = 4.0;
        let badge_rect = Rect::from_min_size(
            text_pos - Vec2::new(padding, text_size.height() / 2.0 + padding),
            text_size.size() + Vec2::new(padding * 2.0, padding * 2.0)
        );
        
        painter.rect_filled(
            badge_rect,
            3.0,
            theme.button_background()
        );
        
        // Draw branch text
        painter.text(
            text_pos,
            egui::Align2::LEFT_CENTER,
            branch,
            egui::FontId::proportional(11.0),
            theme.accent_color(),
        );
        
        x_offset += text_size.width() + padding * 3.0;
    }
}