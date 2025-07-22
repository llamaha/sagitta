// Message rendering functions

use egui::{Ui, Color32, Vec2, Frame, Layout, Align, RichText, Stroke, CornerRadius};
use std::collections::HashMap;
use crate::gui::theme::AppTheme;
use crate::gui::chat::types::{StreamingMessage, MessageAuthor, MessageStatus, CopyButtonState, ToolCall};
use crate::gui::chat::{ChatItem, ToolCard};
use crate::gui::app::RunningToolInfo;
use crate::agent::events::ToolRunId;
use crate::gui::symbols;

// Import functions temporarily from view.rs - these will be moved to other modules
use crate::gui::chat::view::{render_single_tool_call, render_text_content_compact};

/// Group consecutive messages from the same author
pub fn group_consecutive_messages(messages: &[StreamingMessage]) -> Vec<Vec<&StreamingMessage>> {
    let mut groups = Vec::new();
    let mut current_group = Vec::new();
    let mut last_author: Option<&MessageAuthor> = None;
    
    for message in messages {
        if let Some(prev_author) = last_author {
            if &message.author != prev_author {
                // Author changed, start new group
                if !current_group.is_empty() {
                    groups.push(current_group);
                    current_group = Vec::new();
                }
            }
        }
        
        current_group.push(message);
        last_author = Some(&message.author);
    }
    
    // Add the last group if it's not empty
    if !current_group.is_empty() {
        groups.push(current_group);
    }
    
    groups
}

/// Render a group of messages from the same author
pub fn render_message_group(
    ui: &mut Ui, 
    message_group: &[&StreamingMessage], 
    bg_color: &Color32,
    total_width: f32,
    app_theme: AppTheme,
    copy_state: &mut CopyButtonState,
    running_tools: &HashMap<ToolRunId, RunningToolInfo>,
    collapsed_thinking: &mut HashMap<String, bool>,
    tool_cards_collapsed: bool,
    tool_card_individual_states: &mut HashMap<String, bool>,
    use_simplified_tool_rendering: bool,
) -> Option<(String, String)> {
    if message_group.is_empty() {
        return None;
    }
    
    let first_message = message_group[0];
    let mut clicked_tool = None;
    
    // Author colors
    let author_color = match first_message.author {
        MessageAuthor::User => app_theme.user_color(),
        MessageAuthor::Agent => app_theme.agent_color(),
        MessageAuthor::System => app_theme.system_color(),
        MessageAuthor::Tool => app_theme.tool_color(),
    };
    
    // Author name
    let author_name = match first_message.author {
        MessageAuthor::User => "You",
        MessageAuthor::Agent => "Sagitta Code",
        MessageAuthor::System => "System",
        MessageAuthor::Tool => "Tool",
    };
    
    // Render author header only once for the group
    ui.horizontal(|ui| {
        // Author badge - compact and colorful
        ui.add(egui::Label::new(
            RichText::new(author_name)
                .strong()
                .color(author_color)
                .size(13.0) // Slightly larger for group header
        ));
        
        // Show time range for the group
        let first_time = first_message.format_time();
        // Always show timestamp, even for summary/finalization messages
        if message_group.len() > 1 {
            let last_time = message_group.last().unwrap().format_time();
            if first_time != last_time {
                ui.label(RichText::new(format!("{first_time} - {last_time}"))
                    .small()
                    .color(app_theme.timestamp_color())
                    .size(10.0));
            } else {
                ui.label(RichText::new(first_time)
                    .small()
                    .color(app_theme.timestamp_color())
                    .size(10.0));
            }
        } else {
            ui.label(RichText::new(first_time)
                .small()
                .color(app_theme.timestamp_color())
                .size(10.0));
        }
        
        // Show group status (if any message is streaming/thinking)
        let group_status = get_group_status(message_group, app_theme);
        if let Some((icon, color)) = group_status {
            ui.label(RichText::new(icon).color(color).size(12.0));
        }
        
        // Copy all messages button for the group
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let button_text = if copy_state.is_copying {
                "✔"
            } else {
                "📋"
            };
            let button_color = copy_state.get_button_color(app_theme);
            
            let copy_button = egui::Button::new(button_text)
                .fill(button_color)
                .stroke(Stroke::new(1.0, app_theme.border_color()))
                .corner_radius(CornerRadius::same(4));
            
            if ui.add(copy_button).on_hover_text("Copy all messages in group").clicked() {
                let combined_content = message_group.iter()
                    .map(|msg| msg.content.clone())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                ui.ctx().copy_text(combined_content.clone());
                copy_state.start_copy_feedback(combined_content);
                ui.ctx().request_repaint(); // Request repaint for animation
            }
        });
    });
    
    ui.add_space(4.0);
    
    // Render each message in the group with minimal headers
    for (msg_index, message) in message_group.iter().enumerate() {
        if msg_index > 0 {
            ui.add_space(1.0); // Reduced space between messages in same group
        }
        // Always show timestamp for all message types
        if message_group.len() > 1 {
            ui.horizontal(|ui| {
                // Timestamp on the left (only for multi-message groups)
                ui.label(RichText::new(message.format_time())
                    .small()
                    .color(app_theme.timestamp_color())
                    .size(9.0)
                    .monospace());
                
                ui.add_space(8.0);
                
                // Message content in a vertical layout
                ui.vertical(|ui| {
                    ui.set_max_width(total_width - 80.0); // Leave space for timestamp
                    
                    if let Some(tool_info) = render_single_message_content(ui, message, bg_color, total_width - 80.0, app_theme, running_tools, copy_state, collapsed_thinking, tool_cards_collapsed, tool_card_individual_states, use_simplified_tool_rendering) {
                        clicked_tool = Some(tool_info);
                    }
                });
            });
        } else {
            // Single message in group - use full width
            if let Some(tool_info) = render_single_message_content(ui, message, bg_color, total_width, app_theme, running_tools, copy_state, collapsed_thinking, tool_cards_collapsed, tool_card_individual_states, use_simplified_tool_rendering) {
                clicked_tool = Some(tool_info);
            }
        }
    }
    
    clicked_tool
}

/// Get the overall status for a message group
fn get_group_status(message_group: &[&StreamingMessage], app_theme: AppTheme) -> Option<(&'static str, egui::Color32)> {
    // Check if any message is streaming
    if message_group.iter().any(|msg| msg.is_streaming()) {
        return Some(("⟳", app_theme.streaming_color()));
    }
    
    // Check if any message is thinking
    if message_group.iter().any(|msg| msg.is_thinking()) {
        return Some(("💭", app_theme.thinking_indicator_color()));
    }
    
    // Check if any message has errors
    if message_group.iter().any(|msg| matches!(msg.status, MessageStatus::Error(_))) {
        return Some((symbols::get_error_symbol(), app_theme.error_color()));
    }
    
    // All complete
    None
}

/// Render a single message's content
pub fn render_single_message_content(
    ui: &mut Ui, 
    message: &StreamingMessage, 
    bg_color: &Color32,
    max_width: f32,
    app_theme: AppTheme,
    running_tools: &HashMap<ToolRunId, RunningToolInfo>,
    copy_state: &mut CopyButtonState,
    collapsed_thinking: &mut HashMap<String, bool>,
    tool_cards_collapsed: bool,
    tool_card_individual_states: &mut HashMap<String, bool>,
    use_simplified_tool_rendering: bool,
) -> Option<(String, String)> {
    let mut clicked_tool = None;
    
    // Thinking content (if any) - now with streaming and fade support
    if message.should_show_thinking() {
        render_thinking_content(ui, message, bg_color, max_width, app_theme, collapsed_thinking);
        ui.add_space(2.0); // Reduced spacing
    }
    
    // Render content and tool calls in chronological order
    let mut sorted_tools: Vec<(usize, &ToolCall)> = message.tool_calls.iter()
        .filter_map(|tc| tc.content_position.map(|pos| (pos, tc)))
        .collect();
    sorted_tools.sort_by_key(|&(pos, _)| pos);
    
    let mut last_pos = 0;
    let content = &message.content;
    
    // Render content and tool calls interleaved
    for (pos, tool_call) in &sorted_tools {
        // Render content before this tool call
        if *pos > last_pos && last_pos < content.len() {
            let content_chunk = &content[last_pos..*pos.min(&content.len())];
            if !content_chunk.is_empty() {
                if let Some(tool_info) = render_text_content_compact(ui, content_chunk, bg_color, max_width, app_theme) {
                    clicked_tool = Some(tool_info);
                }
            }
        }
        
        // Render the tool call
        ui.add_space(1.0);
        if let Some(tool_info) = render_single_tool_call(ui, tool_call, bg_color, max_width, app_theme, running_tools, copy_state, tool_cards_collapsed, tool_card_individual_states, use_simplified_tool_rendering) {
            clicked_tool = Some(tool_info);
        }
        ui.add_space(1.0);
        
        last_pos = *pos;
    }
    
    // Render any remaining content after the last tool
    if last_pos < content.len() {
        let remaining_content = &content[last_pos..];
        if !remaining_content.is_empty() {
            if let Some(tool_info) = render_text_content_compact(ui, remaining_content, bg_color, max_width, app_theme) {
                clicked_tool = Some(tool_info);
            }
        }
    }
    
    // Render tool calls without position info at the end
    let unpositioned_tools: Vec<&ToolCall> = message.tool_calls.iter()
        .filter(|tc| tc.content_position.is_none())
        .collect();
    
    if !unpositioned_tools.is_empty() {
        ui.add_space(1.0);
        for tool_call in unpositioned_tools {
            if let Some(tool_info) = render_single_tool_call(ui, tool_call, bg_color, max_width, app_theme, running_tools, copy_state, tool_cards_collapsed, tool_card_individual_states, use_simplified_tool_rendering) {
                clicked_tool = Some(tool_info);
            }
            ui.add_space(4.0);
        }
    }
    
    clicked_tool
}

/// Render thinking content with streaming support and fade-out effects
pub fn render_thinking_content(ui: &mut Ui, message: &StreamingMessage, _bg_color: &Color32, max_width: f32, app_theme: AppTheme, collapsed_thinking: &mut HashMap<String, bool>) {
    // Check if we should show thinking content
    if !message.should_show_thinking() {
        return;
    }
    
    let thinking_content = match message.get_thinking_content() {
        Some(content) => content,
        None => return,
    };
    
    // Get or initialize collapsed state for this message
    // Start expanded by default so users can see thinking as it streams
    let is_collapsed = collapsed_thinking.entry(message.id.clone()).or_insert(false);
    
    // No opacity/fade effect - always show at full opacity
    ui.scope(|ui| {
        
        // Collapsible header
        ui.horizontal(|ui| {
            // Collapse/expand button
            let arrow = if *is_collapsed { 
                crate::gui::symbols::navigation::TRIANGLE_RIGHT
            } else { 
                crate::gui::symbols::navigation::TRIANGLE_DOWN
            };
            if ui.small_button(arrow).clicked() {
                *is_collapsed = !*is_collapsed;
            }
            
            // Thinking icon with animation if streaming
            if message.thinking_is_streaming {
                let time = ui.input(|i| i.time);
                let _rotation = (time * 2.0) as f32;
                ui.label(RichText::new(symbols::get_thinking_symbol()).size(14.0)); // Brain emoji for active thinking
            } else {
                ui.label(RichText::new("💭").size(14.0));
            }
            
            // Header with status
            let status_text = if message.thinking_is_streaming {
                "Thinking..."
            } else {
                "Thinking"
            };
            
            ui.label(RichText::new(status_text)
                .italics()
                .color(app_theme.hint_text_color())
                .size(11.0));
            
            // Show preview when collapsed
            if *is_collapsed {
                ui.add_space(8.0);
                let preview = if thinking_content.len() > 50 {
                    format!("{}...", &thinking_content[..50])
                } else {
                    thinking_content.to_string()
                };
                ui.label(RichText::new(preview)
                    .italics()
                    .color(app_theme.hint_text_color())
                    .size(10.0));
            }
        });
        
        // Show full content if not collapsed
        if !*is_collapsed {
            ui.indent(message.id.clone(), |ui| {
                ui.set_max_width(max_width - 40.0);
                
                // Thinking content in a subtle frame
                Frame::NONE
                    .fill(app_theme.thinking_background())
                    .inner_margin(Vec2::new(8.0, 6.0))
                    .corner_radius(CornerRadius::same(6))
                    .stroke(Stroke::new(0.5, app_theme.border_color()))
                    .show(ui, |ui| {
                        ui.set_max_width(max_width - 56.0);
                        
                        // Render thinking content with typewriter effect if streaming
                        if message.thinking_is_streaming && !thinking_content.is_empty() {
                            // Show thinking content without spinner (main spinner will be shown elsewhere)
                            ui.label(RichText::new(thinking_content)
                                .color(app_theme.thinking_text_color())
                                .size(11.0)
                                .italics());
                        } else {
                            ui.label(RichText::new(thinking_content)
                                .color(app_theme.thinking_text_color())
                                .size(11.0)
                                .italics());
                        }
                    });
            });
        }
    });
    
    // Request repaint for animations and fade effects
    if message.thinking_is_streaming || message.thinking_should_fade {
        ui.ctx().request_repaint();
    }
}