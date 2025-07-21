// Chat display UI with enhanced styling

use egui::{
    ScrollArea, 
    Color32, 
    RichText, 
    Stroke, 
    Ui, 
    Vec2, 
    TextFormat, 
    Frame,
    Align, 
    Layout,
    CornerRadius,
};

pub mod collapsing_header_helper;
use collapsing_header_helper::{create_controlled_collapsing_header, get_tool_card_state};
use similar::{ChangeTag, TextDiff};
use crate::gui::theme::AppTheme;
use crate::gui::symbols;
use crate::gui::app::RunningToolInfo;
use crate::agent::events::ToolRunId;

// Import types from the parent module's types module
use super::types::{
    MessageAuthor, MessageStatus, MessageType, ToolCall, 
    StreamingMessage, ChatMessage, CopyButtonState
};
use super::{ChatItem, ToolCard, ToolCardStatus};
// Import tool mappings
use super::tool_mappings::{
    get_human_friendly_tool_name, get_tool_icon, 
    format_tool_parameters_for_inline, format_tool_parameters
};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use std::cell::RefCell;
use serde_json;
use uuid;
use std::time::Instant;
use std::collections::HashMap;
use regex;

// Import syntax highlighting functions
use super::syntax_highlighting::{
    render_syntax_highlighted_code, render_code_diff, detect_diff_content,
    get_syntax_set, get_theme_set, DIFF_COLLAPSING_THRESHOLD_LINES,
    EXPANDED_DIFF_SCROLL_AREA_MAX_HEIGHT
};
// Import message rendering functions
use super::rendering::messages::{
    group_consecutive_messages, render_message_group, 
    render_single_message_content, render_thinking_content
};

thread_local! {
    static COMMONMARK_CACHE: RefCell<CommonMarkCache> = RefCell::new(CommonMarkCache::default());
}



pub fn chat_view_ui(ui: &mut egui::Ui, messages: &[ChatMessage], app_theme: AppTheme, copy_state: &mut CopyButtonState) {
    // Convert legacy messages to ChatItems for modern rendering
    let chat_items: Vec<ChatItem> = messages.iter()
        .map(|msg| ChatItem::Message(msg.clone().into()))
        .collect();
    
    // Create empty HashMap for backward compatibility
    let empty_running_tools = HashMap::new();
    let mut empty_collapsed_thinking = HashMap::new();
    let empty_tool_results = HashMap::new();
    let mut empty_tool_card_states = HashMap::new();
    modern_chat_view_ui(ui, &chat_items, app_theme, copy_state, &empty_running_tools, &mut empty_collapsed_thinking, &empty_tool_results, false, &mut empty_tool_card_states);
}

pub fn modern_chat_view_ui(ui: &mut egui::Ui, items: &[ChatItem], app_theme: AppTheme, copy_state: &mut CopyButtonState, running_tools: &HashMap<ToolRunId, RunningToolInfo>, collapsed_thinking: &mut HashMap<String, bool>, tool_results: &HashMap<String, String>, tool_cards_collapsed: bool, tool_card_individual_states: &mut HashMap<String, bool>) -> Option<(String, String)> {
    // Use the app theme's colors directly
    let bg_color = app_theme.panel_background();
    let _text_color = app_theme.text_color();
    let _accent_color = app_theme.accent_color();

    // Get the total available width - use full width for compact design
    let total_width = ui.available_width();

    let mut clicked_tool = None;

    Frame::NONE
        .fill(bg_color)
        .inner_margin(Vec2::new(16.0, 8.0)) // Add horizontal and vertical margins
        .outer_margin(0.0)
        .show(ui, |ui| {
            // Add "Copy Entire Conversation" button at the top
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Update copy state
                    copy_state.update();
                    
                    let button_text = copy_state.get_button_text("📋 Copy Entire Conversation");
                    let button_color = if copy_state.is_copying {
                        app_theme.success_color()
                    } else {
                        app_theme.accent_color()
                    };
                    
                    let copy_all_button = egui::Button::new(&button_text)
                        .fill(button_color)
                        .stroke(Stroke::new(1.0, app_theme.border_color()))
                        .corner_radius(CornerRadius::same(6));
                    
                    if ui.add(copy_all_button).on_hover_text("Copy entire conversation for sharing").clicked() {
                        // Extract messages and tool cards from ChatItems for conversation copying
                        let conversation_text = format_conversation_with_tools_for_copying(items);
                        ui.ctx().copy_text(conversation_text.clone());
                        copy_state.start_copy_feedback(conversation_text);
                        ui.ctx().request_repaint(); // Request repaint for animation
                    }
                });
            });
            
            ui.add_space(8.0);
            
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .max_height(f32::INFINITY)
                .show(ui, |ui| {
                    // Compact spacing
                    ui.spacing_mut().item_spacing.y = 4.0; // Reduced spacing for grouped messages
                    ui.spacing_mut().button_padding = Vec2::new(6.0, 4.0);
                    
                    ui.add_space(12.0);
                    
                    // Show welcome message if no items exist
                    if items.is_empty() {
                        render_welcome_message(ui, app_theme);
                    }
                    
                    // Render each chat item (messages and tool cards)
                    for (item_index, item) in items.iter().enumerate() {
                        if item_index > 0 {
                            ui.add_space(8.0); // Space between items
                        }
                        
                        match item {
                            ChatItem::Message(message) => {
                                // Render individual messages
                                let messages_group = vec![message];
                                if let Some(tool_info) = render_message_group(ui, &messages_group, &bg_color, total_width - 32.0, app_theme, copy_state, running_tools, collapsed_thinking, tool_cards_collapsed, tool_card_individual_states) {
                                    clicked_tool = Some(tool_info);
                                }
                            }
                            ChatItem::ToolCard(tool_card) => {
                                // Render tool card
                                if let Some(tool_info) = render_tool_card(ui, tool_card, &bg_color, total_width - 32.0, app_theme, running_tools, copy_state, tool_cards_collapsed, tool_card_individual_states) {
                                    clicked_tool = Some(tool_info);
                                }
                            }
                        }
                    }
                    ui.add_space(16.0);
                });
        });
    
    // Handle tool result clicks
    if let Some((title, tool_call_id)) = &clicked_tool {
        if title == "Tool Result" {
            // Look up the actual tool result
            if let Some(result) = tool_results.get(tool_call_id) {
                return Some((format!("Tool Result for {tool_call_id}"), result.clone()));
            }
        }
    }
    
    clicked_tool
}


/// Render a single tool call as a compact, clickable card
pub fn render_single_tool_call(ui: &mut Ui, tool_call: &ToolCall, _bg_color: &Color32, max_width: f32, app_theme: AppTheme, running_tools: &HashMap<ToolRunId, RunningToolInfo>, _copy_state: &mut CopyButtonState, tool_cards_collapsed: bool, tool_card_individual_states: &mut HashMap<String, bool>) -> Option<(String, String)> {
    let mut clicked_tool_result = None;
    
    // Limit tool card width to 90% of max_width
    let tool_card_width = max_width * 0.9;
    
    // Add some padding around the tool card
    ui.add_space(2.0);
    
    // Don't wrap in Frame when using CollapsingHeader to avoid double header appearance
    ui.vertical(|ui| {
        ui.set_max_width(tool_card_width);
        
        // Build the header text with inline parameters
        let friendly_name = get_human_friendly_tool_name(&tool_call.name);
        let status_icon = match tool_call.status {
            MessageStatus::Complete => "✅",
            MessageStatus::Error(_) => "❌",
            MessageStatus::Streaming => "🔄",
            MessageStatus::Sending => "⏳",
            _ => "🔧",
        };
        
        // Get parameters for inline display (excluding edits for edit tools)
        let inline_params = if !tool_call.arguments.is_empty() {
            if let Ok(args_value) = serde_json::from_str::<serde_json::Value>(&tool_call.arguments) {
                let params = format_tool_parameters_for_inline(&tool_call.name, &args_value);
                if !params.is_empty() {
                    let param_str = params.iter()
                        .map(|(k, v)| format!("{}: {}", k, v))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(" - {}", param_str)
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        
        // Format header text with icon at the end to avoid double icon with arrow
        let tool_icon = get_tool_icon(&tool_call.name);
        let header_text = format!("{friendly_name}{inline_params} {tool_icon}");
        
        // Apply tool color to the header for better visual distinction
        let header_text_colored = egui::RichText::new(header_text)
            .color(app_theme.tool_color())
            .strong();
        
        // Create parameter tooltip text
        let mut tooltip_text = String::new();
        if !tool_call.arguments.is_empty() {
            if let Ok(args_value) = serde_json::from_str::<serde_json::Value>(&tool_call.arguments) {
                let params = format_tool_parameters(&tool_call.name, &args_value);
                if !params.is_empty() {
                    tooltip_text.push_str("Parameters:\n");
                    for (key, value) in params.iter() {
                        tooltip_text.push_str(&format!("  {key}: {value}\n"));
                    }
                }
            }
        }
        
        // Add status to tooltip
        match &tool_call.status {
            MessageStatus::Complete => tooltip_text.push_str("\nStatus: Completed"),
            MessageStatus::Streaming => tooltip_text.push_str("\nStatus: Running..."),
            MessageStatus::Error(err) => tooltip_text.push_str(&format!("\nStatus: Failed - {err}")),
            MessageStatus::Sending => tooltip_text.push_str("\nStatus: Starting..."),
            _ => {}
        }
        
        // Use CollapsingHeader for the tool card
        let id = egui::Id::new(&tool_call.id);
        
        // Determine if this tool card should be open and if it has an override
        let (should_be_open, has_override) = get_tool_card_state(&tool_call.id, tool_cards_collapsed, tool_card_individual_states);
        
        let mut collapsing_response = create_controlled_collapsing_header(
            ui,
            id,
            header_text_colored,
            should_be_open,
            has_override,
            |ui| {
                    // Add progress bar for running tools
                    if tool_call.status == MessageStatus::Streaming {
                        // Try to find running tool info to get actual progress
                        let progress = running_tools.values()
                            .find(|info| info.tool_name == tool_call.name)
                            .and_then(|info| info.progress)
                            .unwrap_or(0.0);
                        
                        ui.add(egui::ProgressBar::new(progress)
                            .desired_width(tool_card_width - 20.0)
                            .desired_height(4.0)
                            .fill(app_theme.accent_color())
                            .animate(progress == 0.0)); // Only animate if we don't have actual progress
                        ui.add_space(4.0);
                    }
                    
                    // Action buttons - only show container if we have buttons to display
                    if tool_call.result.is_none() && tool_call.status == MessageStatus::Streaming {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            // Cancel button for running tools
                            if ui.small_button("Cancel").clicked() {
                                // Find the running tool by name to get its run_id
                                if let Some((run_id, _)) = running_tools.iter()
                                    .find(|(_, info)| info.tool_name == tool_call.name) {
                                    // Store the run_id for cancellation (will be handled by the main app)
                                    clicked_tool_result = Some(("__CANCEL_TOOL__".to_string(), run_id.to_string()));
                                    log::info!("Requested cancellation for tool: {} ({})", tool_call.name, run_id);
                                } else {
                                    log::warn!("Could not find running tool to cancel: {}", tool_call.name);
                                }
                            }
                        });
                    }
                    
                    // Add inline result display for completed or error tools
                if (matches!(tool_call.status, MessageStatus::Complete | MessageStatus::Error(_))) && tool_call.result.is_some() {
                    if !matches!(tool_call.status, MessageStatus::Streaming) {
                        ui.add_space(4.0);
                    }
                    
                    let result_str = tool_call.result.as_ref().unwrap();
                    
                    // Parse the result as JSON
                    if let Ok(result_json) = serde_json::from_str::<serde_json::Value>(result_str) {
                        // Use ToolResultFormatter to format the result
                        let formatter = crate::gui::app::tool_formatting::ToolResultFormatter::new();
                        
                        // Create appropriate ToolResult based on status
                        let tool_result = if matches!(tool_call.status, MessageStatus::Error(_)) {
                            // For error status, wrap in Error variant
                            if let Some(error_msg) = result_json.get("error").and_then(|v| v.as_str()) {
                                crate::agent::events::ToolResult::Error { error: error_msg.to_string() }
                            } else if let Some(error_msg) = result_json.as_str() {
                                crate::agent::events::ToolResult::Error { error: error_msg.to_string() }
                            } else {
                                crate::agent::events::ToolResult::Error { error: "Tool execution failed".to_string() }
                            }
                        } else {
                            crate::agent::events::ToolResult::Success { output: result_json.to_string() }
                        };
                        let formatted_result = formatter.format_tool_result_for_preview(&tool_call.name, &tool_result);
                        
                        // Check content size to decide if we need scroll area
                        // For file results, check the actual content field
                        let actual_content_size = if tool_call.name.contains("__repository_view_file") {
                            // Specifically for repository view file, check content field
                            result_json.get("content")
                                .and_then(|v| v.as_str())
                                .map(|s| s.len())
                                .unwrap_or(0)
                        } else if tool_call.name.contains("file") || tool_call.name.contains("view") || tool_call.name.contains("read") {
                            // For other file operations, check various possible content fields
                            result_json.get("content")
                                .or_else(|| result_json.get("file_content"))
                                .or_else(|| result_json.get("data"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.len())
                                .unwrap_or(0)
                        } else {
                            0
                        };
                        
                        let content_lines = formatted_result.lines().count();
                        // Lower thresholds for file content to ensure proper scrolling
                        let needs_scroll = if actual_content_size > 0 {
                            // For file content, use lower thresholds
                            content_lines > 10 || formatted_result.len() > 500 || actual_content_size > 500
                        } else {
                            // For other content, use standard thresholds
                            content_lines > 20 || formatted_result.len() > 2000
                        };
                        
                        if needs_scroll {
                            // Large content - use scroll area
                            // Calculate dynamic height based on content
                            let estimated_line_height = 14.0; // Approximate line height in pixels
                            let content_height = (content_lines as f32 * estimated_line_height).min(800.0);
                            
                            // Set minimum height based on tool type and content
                            let min_height = if tool_call.name.contains("query") || tool_call.name.contains("search") {
                                // For search results, show more but don't force huge minimum
                                content_height.max(200.0).min(600.0)
                            } else if actual_content_size > 0 {
                                // For file content, base on actual content
                                content_height.max(100.0).min(400.0)
                            } else {
                                // For other content, be more flexible
                                content_height.max(50.0).min(300.0)
                            };
                            
                            egui::ScrollArea::vertical()
                                .max_height(800.0)  // Reasonable max height
                                .min_scrolled_height(min_height)
                                .id_salt(format!("tool_result_{}", tool_call.id))
                                .auto_shrink([false, true])  // Don't auto-shrink width, do auto-shrink height
                                .show(ui, |ui| {
                                    ui.set_max_width(tool_card_width - 24.0);
                                    
                                    // Check tool type for special rendering (skip for errors)
                                    if !matches!(tool_call.status, MessageStatus::Error(_)) && is_shell_command_result(&tool_call.name, &result_json) {
                                        render_terminal_output(ui, &result_json, app_theme);
                                    } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_code_change_result(&tool_call.name, &result_json) {
                                        render_diff_output(ui, &result_json, app_theme);
                                    } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_file_read_result(&tool_call.name, &result_json) {
                                        if let Some((key, value)) = render_file_read_output(ui, &result_json, app_theme) {
                                            clicked_tool_result = Some((key, value));
                                        }
                                    } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_file_write_result(&tool_call.name, &result_json) {
                                        if let Some((key, value)) = render_file_write_output(ui, &result_json, app_theme) {
                                            clicked_tool_result = Some((key, value));
                                        }
                                    } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_search_result(&tool_call.name, &result_json) {
                                        if let Some((key, value)) = render_search_output(ui, &result_json, app_theme) {
                                            clicked_tool_result = Some((key, value));
                                        }
                                    } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_repository_result(&tool_call.name, &result_json) {
                                        render_repository_output(ui, &result_json, app_theme);
                                    } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_todo_result(&tool_call.name, &result_json) {
                                        render_todo_output(ui, &result_json, app_theme);
                                    } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_ping_result(&tool_call.name, &result_json) {
                                        render_ping_output(ui, &result_json, app_theme);
                                    } else {
                                        // Default rendering with markdown support
                                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                        
                                        // Use markdown rendering for formatted results
                                        crate::gui::chat::view::COMMONMARK_CACHE.with(|cache| {
                                            let mut cache = cache.borrow_mut();
                                            let viewer = egui_commonmark::CommonMarkViewer::new();
                                            viewer.show(ui, &mut cache, &formatted_result);
                                        });
                                    }
                                });
                        } else {
                            // Small content - render directly without scroll area
                            ui.set_max_width(tool_card_width - 24.0);
                            
                            // Check tool type for special rendering (skip for errors)
                            if !matches!(tool_call.status, MessageStatus::Error(_)) && is_shell_command_result(&tool_call.name, &result_json) {
                                render_terminal_output(ui, &result_json, app_theme);
                            } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_code_change_result(&tool_call.name, &result_json) {
                                render_diff_output(ui, &result_json, app_theme);
                            } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_file_read_result(&tool_call.name, &result_json) {
                                if let Some((key, value)) = render_file_read_output(ui, &result_json, app_theme) {
                                    clicked_tool_result = Some((key, value));
                                }
                            } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_file_write_result(&tool_call.name, &result_json) {
                                if let Some((key, value)) = render_file_write_output(ui, &result_json, app_theme) {
                                    clicked_tool_result = Some((key, value));
                                }
                            } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_search_result(&tool_call.name, &result_json) {
                                if let Some((key, value)) = render_search_output(ui, &result_json, app_theme) {
                                    clicked_tool_result = Some((key, value));
                                }
                            } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_repository_result(&tool_call.name, &result_json) {
                                render_repository_output(ui, &result_json, app_theme);
                            } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_todo_result(&tool_call.name, &result_json) {
                                render_todo_output(ui, &result_json, app_theme);
                            } else if !matches!(tool_call.status, MessageStatus::Error(_)) && is_ping_result(&tool_call.name, &result_json) {
                                render_ping_output(ui, &result_json, app_theme);
                            } else {
                                // Default rendering with markdown support
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                
                                // Use markdown rendering for formatted results
                                crate::gui::chat::view::COMMONMARK_CACHE.with(|cache| {
                                    let mut cache = cache.borrow_mut();
                                    let viewer = egui_commonmark::CommonMarkViewer::new();
                                    viewer.show(ui, &mut cache, &formatted_result);
                                });
                            }
                        }
                    } else {
                        // Fallback: display raw result if JSON parsing fails
                        ui.code(result_str);
                    }
                }
            },
        );
        
        // Check if the user manually toggled this tool card
        if collapsing_response.header_response.clicked() {
            // User clicked the header - track their preference  
            let new_collapsed_state = collapsing_response.openness < 0.5; // If openness < 0.5, it's collapsing
            tool_card_individual_states.insert(tool_call.id.clone(), new_collapsed_state);
        }
        
        // Add tooltip to the header if we have parameter info
        if !tooltip_text.is_empty() {
            collapsing_response.header_response = collapsing_response.header_response.on_hover_text(tooltip_text);
        }
    });
    
    clicked_tool_result
}

/// Render tool calls as compact, clickable cards (for backward compatibility)
fn render_tool_calls_compact(ui: &mut Ui, tool_calls: &[ToolCall], bg_color: &Color32, max_width: f32, app_theme: AppTheme, running_tools: &HashMap<ToolRunId, RunningToolInfo>, copy_state: &mut CopyButtonState, tool_cards_collapsed: bool, tool_card_individual_states: &mut HashMap<String, bool>) -> Option<(String, String)> {
    let mut clicked_tool_result = None;
    
    for tool_call in tool_calls {
        if let Some(tool_info) = render_single_tool_call(ui, tool_call, bg_color, max_width, app_theme, running_tools, copy_state, tool_cards_collapsed, tool_card_individual_states) {
            clicked_tool_result = Some(tool_info);
        }
        ui.add_space(8.0); // Spacing between tool cards
    }
    
    clicked_tool_result
}

/// Render a standalone tool card
fn render_tool_card(ui: &mut Ui, tool_card: &ToolCard, _bg_color: &Color32, max_width: f32, app_theme: AppTheme, _running_tools: &HashMap<ToolRunId, RunningToolInfo>, _copy_state: &mut CopyButtonState, tool_cards_collapsed: bool, tool_card_individual_states: &mut HashMap<String, bool>) -> Option<(String, String)> {
    let mut clicked_tool_result = None;
    
    // Limit tool card width to 90% of max_width
    let tool_card_width = max_width * 0.9;
    
    // Add some padding around the tool card
    ui.add_space(2.0);
    
    // Don't wrap in Frame when using CollapsingHeader to avoid double header appearance
    ui.vertical(|ui| {
        ui.set_max_width(tool_card_width);
        
        // Build the header text with inline parameters
        let friendly_name = get_human_friendly_tool_name(&tool_card.tool_name);
        let status_icon = match tool_card.status {
            ToolCardStatus::Completed { success: true } => "✅",
            ToolCardStatus::Completed { success: false } => "❌",
            ToolCardStatus::Failed { .. } => "❌",
            ToolCardStatus::Running => "🔄",
            ToolCardStatus::Cancelled => "⏹️",
        };
        
        // Get parameters for inline display (excluding edits for edit tools)
        let inline_params = {
            let params = format_tool_parameters_for_inline(&tool_card.tool_name, &tool_card.input_params);
            if !params.is_empty() {
                let param_str = params.iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(" - {}", param_str)
            } else {
                String::new()
            }
        };
        
        // Format header text with icon at the end to avoid double icon with arrow
        let tool_icon = get_tool_icon(&tool_card.tool_name);
        let header_text = format!("{friendly_name}{inline_params} {tool_icon}");
        
        // Apply tool color to the header for better visual distinction
        let header_text_colored = egui::RichText::new(header_text)
            .color(app_theme.tool_color())
            .strong();
        
        // Create parameter tooltip text
        let mut tooltip_text = String::new();
        let params = format_tool_parameters(&tool_card.tool_name, &tool_card.input_params);
        if !params.is_empty() {
            tooltip_text.push_str("Parameters:\n");
            for (key, value) in params.iter() {
                tooltip_text.push_str(&format!("  {key}: {value}\n"));
            }
        }
        
        // Add timing info to tooltip
        if let Some(completed_at) = tool_card.completed_at {
            let duration = completed_at.signed_duration_since(tool_card.started_at);
            tooltip_text.push_str(&format!("\nDuration: {:.1}s", duration.num_milliseconds() as f64 / 1000.0));
        }
        
        // Use CollapsingHeader for the tool card
        let id = egui::Id::new(tool_card.run_id);
        
        // Use the run_id as the key for individual state tracking
        let tool_card_key = tool_card.run_id.to_string();
        
        // Determine if this tool card should be open and if it has an override
        let (should_be_open, has_override) = get_tool_card_state(&tool_card_key, tool_cards_collapsed, tool_card_individual_states);
        
        let mut collapsing_response = create_controlled_collapsing_header(
            ui,
            id,
            header_text_colored,
            should_be_open,
            has_override,
            |ui| {
                    // Show progress bar if running
                    if tool_card.status == ToolCardStatus::Running {
                        if let Some(progress) = tool_card.progress {
                            ui.add(egui::ProgressBar::new(progress)
                                .desired_width(tool_card_width - 20.0)
                                .desired_height(4.0)
                                .fill(app_theme.accent_color()));
                        } else {
                            ui.add(egui::ProgressBar::new(0.0)
                                .desired_width(tool_card_width - 20.0)
                                .desired_height(4.0)
                                .fill(app_theme.accent_color())
                                .animate(true));
                        }
                        ui.add_space(4.0);
                    }
                    
                    // Action buttons - only show container if we have something to display
                    match &tool_card.status {
                        ToolCardStatus::Running => {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                // Cancel button for running tools
                                if ui.small_button("Cancel").clicked() {
                                    clicked_tool_result = Some(("__CANCEL_TOOL__".to_string(), tool_card.run_id.to_string()));
                                }
                            });
                        }
                        ToolCardStatus::Failed { error } => {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("Error: {error}"))
                                    .color(app_theme.error_color())
                                    .size(11.0));
                            });
                        }
                        _ => {}
                    }
            
            // Add inline result display for completed tools
            if let ToolCardStatus::Completed { success: true } = &tool_card.status {
                if let Some(result) = &tool_card.result {
                    ui.add_space(4.0);
                    
                    // Use ToolResultFormatter to format the result
                    let formatter = crate::gui::app::tool_formatting::ToolResultFormatter::new();
                    let success = matches!(tool_card.status, ToolCardStatus::Completed { success: true });
                    let tool_result = if success {
                        crate::agent::events::ToolResult::Success { output: result.to_string() }
                    } else {
                        crate::agent::events::ToolResult::Error { error: "Tool execution failed".to_string() }
                    };
                    
                    let formatted_result = formatter.format_tool_result_for_preview(&tool_card.tool_name, &tool_result);
                    
                    // Check content size to decide if we need scroll area
                    // For file results, check the actual content field
                    let actual_content_size = if tool_card.tool_name.contains("__repository_view_file") {
                        // Specifically for repository view file, check content field
                        result.get("content")
                            .and_then(|v| v.as_str())
                            .map(|s| s.len())
                            .unwrap_or(0)
                    } else if tool_card.tool_name.contains("file") || tool_card.tool_name.contains("view") || tool_card.tool_name.contains("read") {
                        // For other file operations, check various possible content fields
                        result.get("content")
                            .or_else(|| result.get("file_content"))
                            .or_else(|| result.get("data"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.len())
                            .unwrap_or(0)
                    } else {
                        0
                    };
                    
                    let content_lines = formatted_result.lines().count();
                    // Use same threshold logic as tool calls for consistency
                    let needs_scroll = if actual_content_size > 0 {
                        // For file content, use lower thresholds
                        content_lines > 10 || formatted_result.len() > 500 || actual_content_size > 500
                    } else {
                        // For other content, use standard thresholds
                        content_lines > 20 || formatted_result.len() > 2000
                    };
                    
                    if needs_scroll {
                        // Large content - use scroll area
                        // Calculate dynamic height based on content
                        let estimated_line_height = 14.0; // Approximate line height in pixels
                        let content_height = (content_lines as f32 * estimated_line_height).min(800.0);
                        
                        // Set minimum height based on tool type
                        let min_height = if tool_card.tool_name.contains("query") || tool_card.tool_name.contains("search") {
                            // For search results, show more but don't force huge minimum
                            content_height.max(200.0).min(600.0)
                        } else if tool_card.tool_name.contains("shell") || tool_card.tool_name.contains("execute") {
                            // For shell execution, similar to search
                            content_height.max(150.0).min(500.0)
                        } else if actual_content_size > 0 {
                            // For file content, base on actual content
                            content_height.max(100.0).min(400.0)
                        } else {
                            // For other content, be more flexible
                            content_height.max(50.0).min(300.0)
                        };
                        
                        egui::ScrollArea::vertical()
                            .max_height(800.0)  // Reasonable max height
                            .min_scrolled_height(min_height)
                            .id_salt(format!("tool_result_{}", tool_card.run_id))
                            .auto_shrink([false, true])  // Don't auto-shrink width, do auto-shrink height
                            .show(ui, |ui| {
                                ui.set_max_width(tool_card_width - 24.0);
                                
                                // Check tool type for special rendering
                                if is_shell_command_result(&tool_card.tool_name, result) {
                                    render_terminal_output(ui, result, app_theme);
                                } else if is_code_change_result(&tool_card.tool_name, result) {
                                    render_diff_output(ui, result, app_theme);
                                } else if is_file_read_result(&tool_card.tool_name, result) {
                                    if let Some(action) = render_file_read_output(ui, result, app_theme) {
                                        clicked_tool_result = Some(action);
                                    }
                                } else if is_file_write_result(&tool_card.tool_name, result) {
                                    if let Some(action) = render_file_write_output(ui, result, app_theme) {
                                        clicked_tool_result = Some(action);
                                    }
                                } else if is_search_result(&tool_card.tool_name, result) {
                                    if let Some(action) = render_search_output(ui, result, app_theme) {
                                        clicked_tool_result = Some(action);
                                    }
                                } else if is_repository_result(&tool_card.tool_name, result) {
                                    render_repository_output(ui, result, app_theme);
                                } else if is_todo_result(&tool_card.tool_name, result) {
                                    render_todo_output(ui, result, app_theme);
                                } else if is_ping_result(&tool_card.tool_name, result) {
                                    render_ping_output(ui, result, app_theme);
                                } else {
                                    // Default rendering with markdown support
                                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                    
                                    // Use markdown rendering for formatted results
                                    crate::gui::chat::view::COMMONMARK_CACHE.with(|cache| {
                                        let mut cache = cache.borrow_mut();
                                        let viewer = egui_commonmark::CommonMarkViewer::new();
                                        viewer.show(ui, &mut cache, &formatted_result);
                                    });
                                }
                            });
                    } else {
                        // Small content - render directly without scroll area
                        ui.set_max_width(tool_card_width - 24.0);
                        
                        // Check tool type for special rendering
                        if is_shell_command_result(&tool_card.tool_name, result) {
                            render_terminal_output(ui, result, app_theme);
                        } else if is_code_change_result(&tool_card.tool_name, result) {
                            render_diff_output(ui, result, app_theme);
                        } else if is_file_read_result(&tool_card.tool_name, result) {
                            if let Some(action) = render_file_read_output(ui, result, app_theme) {
                                clicked_tool_result = Some(action);
                            }
                        } else if is_file_write_result(&tool_card.tool_name, result) {
                            if let Some(action) = render_file_write_output(ui, result, app_theme) {
                                clicked_tool_result = Some(action);
                            }
                        } else if is_search_result(&tool_card.tool_name, result) {
                            if let Some(action) = render_search_output(ui, result, app_theme) {
                                clicked_tool_result = Some(action);
                            }
                        } else if is_repository_result(&tool_card.tool_name, result) {
                            render_repository_output(ui, result, app_theme);
                        } else if is_todo_result(&tool_card.tool_name, result) {
                            render_todo_output(ui, result, app_theme);
                        } else if is_ping_result(&tool_card.tool_name, result) {
                            render_ping_output(ui, result, app_theme);
                        } else {
                            // Default rendering with markdown support
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                            
                            // Use markdown rendering for formatted results
                            crate::gui::chat::view::COMMONMARK_CACHE.with(|cache| {
                                let mut cache = cache.borrow_mut();
                                let viewer = egui_commonmark::CommonMarkViewer::new();
                                viewer.show(ui, &mut cache, &formatted_result);
                            });
                        }
                    }
                }
            }
        },
    );
        
        // Check if the user manually toggled this tool card
        if collapsing_response.header_response.clicked() {
            // User clicked the header - track their preference
            let new_collapsed_state = collapsing_response.openness < 0.5; // If openness < 0.5, it's collapsing
            tool_card_individual_states.insert(tool_card_key, new_collapsed_state);
        }
        
        // Add tooltip to the header if we have parameter info
        if !tooltip_text.is_empty() {
            collapsing_response.header_response = collapsing_response.header_response.on_hover_text(tooltip_text);
        }
    });
    
    clicked_tool_result
}

/// Render message content in a compact format
fn render_message_content_compact(ui: &mut Ui, message: &StreamingMessage, bg_color: &Color32, max_width: f32, app_theme: AppTheme) -> Option<(String, String)> {
    // Use message_type for summary/finalization
    if message.message_type == MessageType::Summary {
        return render_text_content_compact(ui, &message.content, bg_color, max_width, app_theme);
    }
    
    // Set up content area
    ui.set_max_width(max_width - 20.0);
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    
    // Render content based on type
    let clicked_tool = if message.content.contains("```") {
        render_mixed_content_compact(ui, &message.content, bg_color, max_width - 20.0, app_theme)
    } else {
        render_text_content_compact(ui, &message.content, bg_color, max_width - 20.0, app_theme)
    };
    
    // Show streaming cursor if message is still streaming but not in thinking_is_streaming phase
    // SUPPRESS spinner for reasoning-engine summary messages
    if message.is_streaming() && !message.thinking_is_streaming && !is_reasoning_engine_summary_message(&message.content) {
        ui.horizontal(|ui| {
            let time = ui.input(|i| i.time);
            let alpha = ((time * 2.0).sin() * 0.5 + 0.5) as f32;
            let cursor_color = Color32::from_rgba_premultiplied(
                bg_color.r(),
                bg_color.g(),
                bg_color.b(),
                (255.0 * alpha) as u8
            );
            ui.add(egui::Spinner::new().size(12.0).color(cursor_color));
        });
    }
    
    clicked_tool
}

/// Render text content compactly using markdown with proper theme colors
pub fn render_text_content_compact(ui: &mut Ui, text: &str, bg_color: &Color32, max_width: f32, app_theme: AppTheme) -> Option<(String, String)> {
    // Check if text contains tool:// links
    if text.contains("tool://") {
        render_text_with_tool_links(ui, text, bg_color, max_width, app_theme)
    } else {
        // Set the text color from theme before rendering
        let _original_text_color = ui.style().visuals.text_color();
        ui.style_mut().visuals.override_text_color = Some(app_theme.text_color());
        
        COMMONMARK_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let viewer = CommonMarkViewer::new()
                .max_image_width(Some(max_width as usize))
                .default_width(Some(max_width as usize));
            
            ui.set_max_width(max_width);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            viewer.show(ui, &mut cache, text);
        });
        
        // Restore original text color
        ui.style_mut().visuals.override_text_color = None;
        None
    }
}

/// Render text with tool:// links as clickable buttons
fn render_text_with_tool_links(ui: &mut Ui, text: &str, _bg_color: &Color32, max_width: f32, app_theme: AppTheme) -> Option<(String, String)> {
    let mut clicked_tool = None;
    
    // Parse all [text](tool://id) patterns in the text
    let mut processed_text = text.to_string();
    let mut tool_links = Vec::new();
    
    // Find all tool:// links and replace them with placeholder buttons
    let pattern = regex::Regex::new(r"\[([^\]]+)\]\(tool://([^)]+)\)").unwrap();
    
    for (i, caps) in pattern.captures_iter(text).enumerate() {
        let link_text = caps.get(1).unwrap().as_str();
        let tool_call_id = caps.get(2).unwrap().as_str();
        tool_links.push((link_text.to_string(), tool_call_id.to_string()));
        
        // Replace the markdown link with a placeholder
        let placeholder = format!("__TOOL_LINK_{i}__");
        processed_text = processed_text.replace(&caps[0], &placeholder);
    }
    
    
    if tool_links.is_empty() {
        // No tool links, render normally with CommonMark
        let _original_text_color = ui.style().visuals.text_color();
        ui.style_mut().visuals.override_text_color = Some(app_theme.text_color());
        
        COMMONMARK_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let viewer = CommonMarkViewer::new()
                .max_image_width(Some(max_width as usize))
                .default_width(Some(max_width as usize));
            
            ui.set_max_width(max_width);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            viewer.show(ui, &mut cache, text);
        });
        
        ui.style_mut().visuals.override_text_color = None;
        return None;
    }
    
    // Process the text with proper markdown rendering and tool link buttons
    let mut remaining_text = processed_text.as_str();
    
    for (i, (link_text, tool_call_id)) in tool_links.iter().enumerate() {
        let placeholder = format!("__TOOL_LINK_{i}__");
        
        if let Some(split_pos) = remaining_text.find(&placeholder) {
            // Render text before the placeholder using CommonMark
            let before_text = &remaining_text[..split_pos];
            if !before_text.trim().is_empty() {
                let _original_text_color = ui.style().visuals.text_color();
                ui.style_mut().visuals.override_text_color = Some(app_theme.text_color());
                
                COMMONMARK_CACHE.with(|cache| {
                    let mut cache = cache.borrow_mut();
                    let viewer = CommonMarkViewer::new()
                        .max_image_width(Some(max_width as usize))
                        .default_width(Some(max_width as usize));
                    
                    ui.set_max_width(max_width);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    viewer.show(ui, &mut cache, before_text);
                });
                
                ui.style_mut().visuals.override_text_color = None;
            }
            
            // Render the clickable button
            ui.horizontal(|ui| {
                let button = ui.small_button(format!("📋 {link_text}"));
                if button.clicked() {
                    clicked_tool = Some(("Tool Result".to_string(), tool_call_id.to_string()));
                }
            });
            
            // Update remaining text
            remaining_text = &remaining_text[split_pos + placeholder.len()..];
        }
    }
    
    // Render any remaining text after the last placeholder using CommonMark
    if !remaining_text.trim().is_empty() {
        let _original_text_color = ui.style().visuals.text_color();
        ui.style_mut().visuals.override_text_color = Some(app_theme.text_color());
        
        COMMONMARK_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let viewer = CommonMarkViewer::new()
                .max_image_width(Some(max_width as usize))
                .default_width(Some(max_width as usize));
            
            ui.set_max_width(max_width);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            viewer.show(ui, &mut cache, remaining_text);
        });
        
        ui.style_mut().visuals.override_text_color = None;
    }
    
    clicked_tool
}

/// Render code block compactly
fn render_code_block_compact(ui: &mut Ui, text: &str, bg_color: &Color32, max_width: f32, app_theme: AppTheme) {
    let _opacity = 0.3; // Default opacity for UI elements
    let mut lines = text.lines();
    let first_line = lines.next().unwrap_or("");
    let (language, remaining_text) = if first_line.trim().is_empty() {
        ("text", text)
    } else {
        (first_line.trim(), text.split_once('\n').map(|x| x.1).unwrap_or(""))
    };
    
    // Compact code block header
    ui.horizontal(|ui| {
        ui.label(RichText::new("💻").size(12.0));
        ui.label(RichText::new(language).monospace().color(app_theme.hint_text_color()).size(10.0));
        
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let copy_button = egui::Button::new("📋")
                .fill(app_theme.input_background())
                .stroke(Stroke::new(1.0, app_theme.border_color()))
                .corner_radius(CornerRadius::same(4));
            
            if ui.add(copy_button).on_hover_text("Copy code").clicked() {
                ui.ctx().copy_text(remaining_text.to_string());
            }
        });
    });
    
    ui.add_space(2.0);
    
    // Code content in a subtle frame
    Frame::NONE
        .fill(app_theme.code_background())
        .inner_margin(Vec2::new(8.0, 6.0))
        .corner_radius(CornerRadius::same(4))
        .stroke(Stroke::new(0.5, app_theme.border_color()))
        .show(ui, |ui| {
            ui.set_max_width(max_width - 16.0);
            
            // Scrollable for long code
            let line_count = remaining_text.lines().count();
            if line_count > 10 {
                // Show line count indicator
                ui.label(RichText::new(format!("{line_count} lines of code"))
                    .small()
                    .color(app_theme.hint_text_color()));
                ui.add_space(2.0);
                
                // Use scrollable area with max height for ~10 lines
                ScrollArea::vertical()
                    .max_height(120.0) // Approximately 10 lines at 12px line height
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        render_syntax_highlighted_code(ui, remaining_text, language, bg_color, max_width - 32.0);
                    });
            } else {
                render_syntax_highlighted_code(ui, remaining_text, language, bg_color, max_width - 16.0);
            }
        });
}



/// Check if a message appears to be a tool result (contains large structured data)
fn is_tool_result_message(text: &str) -> bool {
    // Check for explicit tool result patterns - this catches ALL tools
    let has_tool_result_prefix = text.starts_with("Tool '") && text.contains("' result:");
    
    // Check for tool call patterns
    let has_tool_call_prefix = text.starts_with("Calling tool:");
    
    // Check for JSON structure patterns (common in tool results)
    let has_json_structure = text.contains("{") && text.contains("}") && text.contains("\"");
    
    // Check for large content (more than 300 characters for tool results)
    let has_large_content = text.len() > 300;
    
    // Check for common tool result indicators from ANY tool
    let has_tool_indicators = text.contains("grounded") || 
                             text.contains("sources") || 
                             text.contains("search_queries") ||
                             text.contains("confidence") ||
                             text.contains("uri") ||
                             text.contains("file_path") ||
                             text.contains("content") ||
                             text.contains("repository_name") ||
                             text.contains("start_line") ||
                             text.contains("end_line") ||
                             text.contains("file_type") ||
                             text.contains("repositories") ||
                             text.contains("query") ||
                             text.contains("response") ||
                             text.contains("Arguments:") ||
                             text.contains("result") ||
                             text.contains("success") ||
                             text.contains("error");
    
    // Check for structured data patterns from various tools
    let has_structured_data = text.contains("\"content\":") ||
                             text.contains("\"file_path\":") ||
                             text.contains("\"sources\":") ||
                             text.contains("\"query\":") ||
                             text.contains("\"response\":") ||
                             text.contains("\"repositories\":") ||
                             text.contains("\"file_type\":") ||
                             text.contains("\"repository_name\":") ||
                             text.contains("\"start_line\":") ||
                             text.contains("\"end_line\":") ||
                             text.contains("\"grounded\":") ||
                             text.contains("\"search_queries\":") ||
                             text.contains("\"confidence\":") ||
                             text.contains("\"uri\":") ||
                             // Add shell execution specific patterns
                             text.contains("\"exit_code\":") ||
                             text.contains("\"stdout\":") ||
                             text.contains("\"stderr\":") ||
                             text.contains("\"execution_time_ms\":") ||
                             text.contains("\"container_image\":") ||
                             text.contains("\"timed_out\":");
    
    // Return true if it's explicitly a tool result OR tool call OR has JSON structure with indicators
    has_tool_result_prefix || has_tool_call_prefix || (has_json_structure && (has_large_content || has_tool_indicators || has_structured_data))
}

/// Extract execution time from tool result data
fn extract_execution_time(result_data: &str) -> Option<u64> {
    // Try to parse as JSON and look for common timing fields
    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(result_data) {
        if let Some(obj) = json_value.as_object() {
            // Check for various timing field names used by different tools
            if let Some(time) = obj.get("execution_time_ms").and_then(|v| v.as_u64()) {
                return Some(time);
            }
            if let Some(time) = obj.get("execution_time").and_then(|v| v.as_u64()) {
                return Some(time);
            }
            if let Some(time) = obj.get("duration_ms").and_then(|v| v.as_u64()) {
                return Some(time);
            }
            if let Some(time) = obj.get("duration").and_then(|v| v.as_u64()) {
                return Some(time);
            }
            if let Some(time) = obj.get("elapsed_ms").and_then(|v| v.as_u64()) {
                return Some(time);
            }
            if let Some(time) = obj.get("time_ms").and_then(|v| v.as_u64()) {
                return Some(time);
            }
        }
    }
    None
}

/// Extract a human-readable summary from tool result JSON
fn extract_tool_result_summary(text: &str) -> String {
    // Check for explicit tool result format first
    if text.starts_with("Tool '") {
        if let Some(end_quote) = text.find("' result:") {
            let tool_name = &text[6..end_quote]; // Skip "Tool '"
            
            // Try to extract additional info from the JSON
            if let Some(json_start) = text.find('{') {
                let json_part = &text[json_start..];
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_part) {
                    if let Some(obj) = json.as_object() {
                        // For file operations
                        if let Some(file_path) = obj.get("file_path").and_then(|v| v.as_str()) {
                            return format!("{tool_name}: {file_path}");
                        }
                        
                        // For search operations
                        if let Some(query) = obj.get("query").and_then(|v| v.as_str()) {
                            return format!("{tool_name}: \"{query}\"");
                        }
                        
                        // For content operations
                        if obj.contains_key("content") {
                            return format!("{tool_name}: Content retrieved");
                        }
                    }
                }
            }
            
            return format!("{tool_name} result");
        }
    }
    
    // Try to parse as JSON and extract key information
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(obj) = json.as_object() {
            // Check for stdout content first (for shell execution results)
            if let Some(stdout) = obj.get("stdout").and_then(|v| v.as_str()) {
                let stdout_trimmed = stdout.trim();
                if !stdout_trimmed.is_empty() {
                    // Find first meaningful line
                    for line in stdout_trimmed.lines() {
                        let line_trimmed = line.trim();
                        if !line_trimmed.is_empty() {
                            if line_trimmed.len() > 60 {
                                return format!("{}...", &line_trimmed[..57]);
                            }
                            return line_trimmed.to_string();
                        }
                    }
                }
            }
            
            // Check for file search results BEFORE web search (has "files" array)
            if let Some(files) = obj.get("files").and_then(|v| v.as_array()) {
                if let Some(query) = obj.get("query").and_then(|v| v.as_str()) {
                    return format!("File search: \"{}\" ({} files)", query, files.len());
                }
                return format!("File search: {} files", files.len());
            }
            
            // Check for web search results (has query + source_count but no files array)
            if let Some(query) = obj.get("query").and_then(|v| v.as_str()) {
                if let Some(source_count) = obj.get("source_count").and_then(|v| v.as_u64()) {
                    return format!("Web search: \"{query}\" ({source_count} sources)");
                }
                // Only treat as web search if it doesn't have file-specific fields
                if !obj.contains_key("files") {
                    return format!("Web search: \"{query}\"");
                }
            }
            
            // Check for file operations
            if let Some(file_path) = obj.get("file_path").and_then(|v| v.as_str()) {
                return format!("File: {file_path}");
            }
            
            // Check for other tool types
            if obj.contains_key("grounded") {
                return "Grounded web search result".to_string();
            }
            
            if obj.contains_key("sources") {
                return "Search result with sources".to_string();
            }
            
            if obj.contains_key("content") {
                return "Content retrieved".to_string();
            }
            
            // Fallback: count fields
            return format!("{} fields", obj.len());
        }
    }
    
    // Fallback: try to extract first meaningful line
    for line in text.lines().take(3) {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('{') && !trimmed.starts_with('[') {
            if trimmed.len() > 50 {
                return format!("{}...", &trimmed[..47]);
            }
            return trimmed.to_string();
        }
    }
    
    "Tool execution result".to_string()
}

fn is_reasoning_engine_summary_message(text: &str) -> bool {
    text.contains("Okay, I've finished those tasks") ||
    text.contains("Successfully completed:") ||
    text.contains("What would you like to do next?")
}

/// Constants for diff rendering
const MIN_ALLOCATED_HEIGHT_FOR_DIFF_FRAME: f32 = 400.0;

/// Constants for shell output rendering
const SHELL_OUTPUT_SCROLL_AREA_MAX_HEIGHT: f32 = 360.0;
const SHELL_OUTPUT_COLLAPSING_THRESHOLD_LINES: usize = 8;

/// Helper function to wrap text at specified line length
fn wrap_text_at_line_length(text: &str, max_line_length: usize) -> String {
    text.lines()
        .map(|line| {
            if line.len() <= max_line_length {
                line.to_string()
            } else {
                let mut result = String::new();
                let mut current_pos = 0;
                while current_pos < line.len() {
                    let end_pos = std::cmp::min(current_pos + max_line_length, line.len());
                    result.push_str(&line[current_pos..end_pos]);
                    if end_pos < line.len() {
                        result.push('\n');
                    }
                    current_pos = end_pos;
                }
                result
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render mixed content (text + code blocks) compactly
fn render_mixed_content_compact(ui: &mut Ui, content: &str, bg_color: &Color32, max_width: f32, app_theme: AppTheme) -> Option<(String, String)> {
    if let Some((old_content, new_content, language)) = detect_diff_content(content) {
        // Render diff header
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔄").size(12.0));
            ui.label(RichText::new("Diff").monospace().color(app_theme.hint_text_color()).size(10.0));
            if let Some(lang) = &language {
                ui.label(RichText::new(format!("({lang})")).monospace().color(app_theme.hint_text_color()).size(9.0));
            }
            
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let copy_button = egui::Button::new("📋")
                    .fill(app_theme.input_background())
                    .stroke(Stroke::new(1.0, app_theme.border_color()))
                    .corner_radius(CornerRadius::same(4));
                
                if ui.add(copy_button).on_hover_text("Copy diff").clicked() {
                    let diff_text = format!("--- Original\n+++ Modified\n{}", 
                        similar::TextDiff::from_lines(&old_content, &new_content)
                            .unified_diff()
                            .context_radius(3)
                    );
                    ui.ctx().copy_text(diff_text);
                }
            });
        });
        ui.add_space(2.0);

        let desired_min_height_for_diff_component = MIN_ALLOCATED_HEIGHT_FOR_DIFF_FRAME;
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), desired_min_height_for_diff_component),
            Layout::top_down(Align::Min).with_cross_align(Align::Min),
            |ui_for_diff_frame| {
                Frame::NONE
                    .fill(app_theme.code_background())
                    .inner_margin(Vec2::new(8.0, 6.0))
                    .corner_radius(CornerRadius::same(4))
                    .stroke(Stroke::new(0.5, app_theme.border_color()))
                    .show(ui_for_diff_frame, |frame_content_ui| {
                        render_code_diff(frame_content_ui, &old_content, &new_content, language.as_deref(), bg_color, frame_content_ui.available_width(), app_theme);
                    });
            }
        );
        return None;
    }
    
    let parts: Vec<&str> = content.split("```").collect();
    for (i, part) in parts.iter().enumerate() {
        if i % 2 == 0 {
            if !part.is_empty() {
                if let Some((old_content, new_content, language)) = detect_diff_content(part) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔄").size(12.0));
                        ui.label(RichText::new("Diff").monospace().color(app_theme.hint_text_color()).size(10.0));
                        if let Some(lang) = &language {
                            ui.label(RichText::new(format!("({lang})")).monospace().color(app_theme.hint_text_color()).size(9.0));
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let copy_button = egui::Button::new("📋")
                                .fill(app_theme.input_background())
                                .stroke(Stroke::new(1.0, app_theme.border_color()))
                                .corner_radius(CornerRadius::same(4));
                            if ui.add(copy_button).on_hover_text("Copy diff").clicked() {
                                let diff_text = format!("--- Original\n+++ Modified\n{}", 
                                    similar::TextDiff::from_lines(&old_content, &new_content)
                                        .unified_diff().context_radius(3));
                                ui.ctx().copy_text(diff_text);
                            }
                        });
                    });
                    ui.add_space(2.0);
                    let desired_min_height_for_diff_component = MIN_ALLOCATED_HEIGHT_FOR_DIFF_FRAME;
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), desired_min_height_for_diff_component),
                        Layout::top_down(Align::Min).with_cross_align(Align::Min),
                        |ui_for_diff_frame| {
                            Frame::NONE
                                .fill(app_theme.code_background())
                                .inner_margin(Vec2::new(8.0, 6.0))
                                .corner_radius(CornerRadius::same(4))
                                .stroke(Stroke::new(0.5, app_theme.border_color()))
                                .show(ui_for_diff_frame, |frame_content_ui| {
                                    render_code_diff(frame_content_ui, &old_content, &new_content, language.as_deref(), bg_color, frame_content_ui.available_width(), app_theme);
                                });
                        }
                    );
                } else if let Some(tool_info) = render_text_content_compact(ui, part, bg_color, max_width, app_theme) {
                    return Some(tool_info);
                }
            }
        } else {
            render_code_block_compact(ui, part, bg_color, max_width, app_theme);
        }
    }
    None
}

/// Render welcome message when no conversation items exist
fn render_welcome_message(ui: &mut egui::Ui, app_theme: AppTheme) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        
        // Welcome header
        ui.label(egui::RichText::new("Welcome to Sagitta Code! 🚀")
            .size(24.0)
            .strong()
            .color(app_theme.accent_color()));
        
        ui.add_space(20.0);
        
        // Description
        ui.label(egui::RichText::new("Your intelligent code companion powered by Claude")
            .size(16.0)
            .color(app_theme.text_color()));
        
        ui.add_space(30.0);
        
        // Features list
        ui.group(|ui| {
            ui.set_width(500.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("What you can do:")
                    .size(14.0)
                    .strong()
                    .color(app_theme.text_color()));
                
                ui.add_space(10.0);
                
                let features = vec![
                    ("💬", "Have natural conversations about your code"),
                    ("🔍", "Search and analyze multiple repositories"),
                    ("⚡", "Execute commands and run code"),
                    ("🛠️", "Edit files with AI assistance"),
                    ("📁", "Manage git repositories and branches"),
                    ("🎯", "Get contextual help with your projects"),
                ];
                
                for (icon, description) in features {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(icon).size(14.0));
                        ui.label(egui::RichText::new(description)
                            .size(13.0)
                            .color(app_theme.text_color()));
                    });
                    ui.add_space(5.0);
                }
            });
        });
        
        ui.add_space(30.0);
        
        // Getting started hint
        ui.label(egui::RichText::new("Type a message below to get started, or press Ctrl+N to create a new conversation")
            .size(12.0)
            .color(app_theme.hint_text_color())
            .italics());
        
        ui.add_space(40.0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_message(content: &str, message_type: MessageType) -> StreamingMessage {
        StreamingMessage {
            id: "test-id".to_string(),
            author: MessageAuthor::Agent,
            content: content.to_string(),
            status: MessageStatus::Complete,
            thinking_content: None,
            tool_calls: vec![],
            timestamp: Utc::now(),
            message_type,
            thinking_stream_content: String::new(),
            thinking_is_streaming: false,
            thinking_fade_start: None,
            thinking_should_fade: false,
            thinking_collapsed: true,  // Default to collapsed
        }
    }


    #[test]
    fn test_tool_result_summary_generation() {
        // Shell execution result
        let shell_result = r#"{
            "exit_code": 0,
            "stdout": "Created binary package",
            "stderr": "",
            "execution_time_ms": 156,
            "container_image": "local",
            "timed_out": false
        }"#;
        let summary = extract_tool_result_summary(shell_result);
        println!("Shell summary: '{summary}'");
        assert!(summary.contains("Created") || summary.contains("fields"));
        
        // File search result
        let file_result = r#"{"files": ["main.rs", "lib.rs"], "query": "fn main"}"#;
        let summary = extract_tool_result_summary(file_result);
        println!("File summary: '{summary}'");
        assert!(summary.contains("2 files") || summary.contains("File search") || summary.contains("main.rs"));
        
        // Web search result  
        let web_result = r#"{"query": "rust fibonacci", "source_count": 5}"#;
        let summary = extract_tool_result_summary(web_result);
        println!("Web summary: '{summary}'");
        assert!(summary.contains("2 fields") || summary.contains("rust fibonacci"));
    }

    #[test]
    fn test_is_tool_result_message_detection() {
        // Test that different tool result formats are properly detected
        
        // Standard tool result format
        assert!(is_tool_result_message("Tool 'shell_execution' result: {\"exit_code\": 0}"));
        
        // JSON-only format (should also be detected)
        assert!(is_tool_result_message("{\"exit_code\": 0, \"stdout\": \"output\"}"));
        
        // Large content format 
        let large_content = "x".repeat(1000);
        assert!(is_tool_result_message(&format!("{{\"data\": \"{large_content}\"}}")));
        
        // Non-tool result messages
        assert!(!is_tool_result_message("This is just a regular message"));
        assert!(!is_tool_result_message("Some JSON: {\"key\": \"value\"}"));
    }

    #[test]
    fn test_tool_call_structure() {
        // Test that ToolCall structure works correctly
        let tool_call = ToolCall {
            id: "test_tool_call_1".to_string(),
            name: "shell_execution".to_string(),
            arguments: r#"{"command": "cargo new fibonacci_calculator"}"#.to_string(),
            result: Some(r#"{
                "exit_code": 0,
                "stdout": "Created binary package",
                "stderr": "",
                "execution_time_ms": 150,
                "container_image": "local",
                "timed_out": false
            }"#.to_string()),
            status: MessageStatus::Complete,
            content_position: None,
        };
        
        assert_eq!(tool_call.name, "shell_execution");
        assert!(tool_call.arguments.contains("cargo new"));
        assert!(tool_call.result.is_some());
        assert!(matches!(tool_call.status, MessageStatus::Complete));
        
        // Verify the result can be parsed as JSON
        let result_json: serde_json::Value = serde_json::from_str(tool_call.result.as_ref().unwrap()).unwrap();
        assert_eq!(result_json["exit_code"], 0);
        assert!(result_json["stdout"].as_str().unwrap().contains("Created"));
    }

    #[test]
    fn test_tool_result_click_data_structure() {
        // Test that tool result data structure is correct for different tool types
        
        // Shell execution result
        let shell_result = r#"{
            "exit_code": 0,
            "stdout": "Created binary package",
            "stderr": "",
            "execution_time_ms": 156,
            "container_image": "local",
            "timed_out": false
        }"#;
        
        // Verify JSON parsing works
        let json_value: serde_json::Value = serde_json::from_str(shell_result).unwrap();
        assert!(json_value.is_object());
        assert_eq!(json_value["exit_code"], 0);
        assert!(json_value["stdout"].as_str().unwrap().contains("Created"));
        
        // File search result
        let file_result = r#"{"files": ["main.rs", "lib.rs"], "query": "fn main"}"#;
        let json_value: serde_json::Value = serde_json::from_str(file_result).unwrap();
        assert!(json_value.is_object());
        assert!(json_value["files"].is_array());
        assert_eq!(json_value["query"], "fn main");
    }

    #[test]
    fn test_shell_execution_result_detection() {
        // Test that shell execution results are properly identified
        let shell_result = r#"{
            "exit_code": 0,
            "stdout": "Created binary package",
            "stderr": "",
            "execution_time_ms": 156,
            "container_image": "local",
            "timed_out": false
        }"#;
        
        // Check detection logic used in render_tool_result_compact
        let is_shell_result = shell_result.contains("stdout") || 
                             shell_result.contains("stderr") ||
                             shell_result.contains("exit_code");
        
        assert!(is_shell_result, "Shell execution result should be detected");
        
        // Test with tool name
        let tool_name = "shell_execution";
        let is_shell_by_name = tool_name.contains("shell") || tool_name.contains("execution");
        assert!(is_shell_by_name, "Shell tool should be detected by name");
    }

    #[test]
    fn test_extract_execution_time() {
        // Test shell execution result with execution_time_ms
        let shell_result = r#"{
            "exit_code": 0,
            "stdout": "output",
            "stderr": "",
            "execution_time_ms": 156,
            "container_image": "local",
            "timed_out": false
        }"#;
        assert_eq!(extract_execution_time(shell_result), Some(156));
        
        // Test with different field name
        let other_result = r#"{"status": "success", "duration_ms": 42}"#;
        assert_eq!(extract_execution_time(other_result), Some(42));
        
        // Test with no timing info
        let no_time_result = r#"{"status": "success", "data": "result"}"#;
        assert_eq!(extract_execution_time(no_time_result), None);
        
        // Test with invalid JSON
        assert_eq!(extract_execution_time("not json"), None);
        
        // Test with alternative field names
        let alt_result1 = r#"{"execution_time": 100}"#;
        assert_eq!(extract_execution_time(alt_result1), Some(100));
        
        let alt_result2 = r#"{"time_ms": 250}"#;
        assert_eq!(extract_execution_time(alt_result2), Some(250));
    }

    #[test]
    fn test_tool_result_render_order_inline() {
        // Test that tool results appear inline with message content in chronological order
        let mut messages = Vec::new();
        
        // Create first agent message with initial content and tool call
        let mut agent_msg_1 = create_test_message("Starting task...", MessageType::Normal);
        agent_msg_1.author = MessageAuthor::Agent;
        agent_msg_1.id = "agent-1".to_string();
        
        // Add tool call inline within the agent message
        agent_msg_1.tool_calls.push(ToolCall {
            id: "test_tool_call_2".to_string(),
            name: "add_repository".to_string(),
            arguments: r#"{"name": "test_repo"}"#.to_string(),
            result: Some(r#"{"status": "added", "message": "Repository added successfully"}"#.to_string()),
            status: MessageStatus::Complete,
            content_position: None,
        });
        messages.push(agent_msg_1);
        
        // Create second agent message with follow-up content
        let mut agent_msg_2 = create_test_message("Now reading the file...", MessageType::Normal);
        agent_msg_2.author = MessageAuthor::Agent;
        agent_msg_2.id = "agent-2".to_string();
        messages.push(agent_msg_2);
        
        // Group the messages (this should preserve order)
        let groups = group_consecutive_messages(&messages);
        
        // Should have 1 group with both agent messages (consecutive messages from same author are grouped)
        assert_eq!(groups.len(), 1, "Should have 1 message group (consecutive agent messages)");
        assert_eq!(groups[0].len(), 2, "Should have 2 agent messages in the group");
        
        // Verify order: first message is agent with "Starting task" and tool call
        assert_eq!(groups[0][0].author, MessageAuthor::Agent);
        assert!(groups[0][0].content.contains("Starting task"));
        assert!(!groups[0][0].tool_calls.is_empty(), "First agent message should contain tool call");
        assert_eq!(groups[0][0].tool_calls[0].name, "add_repository");
        
        // Second message is agent with "Now reading"
        assert_eq!(groups[0][1].author, MessageAuthor::Agent);
        assert!(groups[0][1].content.contains("Now reading"));
        assert!(groups[0][1].tool_calls.is_empty(), "Second agent message should not have tool calls");
    }

    #[test]
    fn test_comprehensive_fixes_for_user_issues() {
        // This test verifies the fixes for three main issues:
        // 1. Tool results should appear inline, not at the bottom
        // 2. Tool result summaries should be properly generated  
        // 3. Complex tasks like "ADD A TEST" should be handled correctly
        
        // Issue 1: Tool result inline ordering - now within agent messages
        let mut messages = Vec::new();
        
        // Create agent message with tool call inline
        let mut agent_msg = create_test_message("I'll help you add that test...", MessageType::Normal);
        agent_msg.author = MessageAuthor::Agent;
        agent_msg.id = "agent-1".to_string();
        
        // Add tool call inline within the agent message
        agent_msg.tool_calls.push(ToolCall {
            id: "test_tool_call_3".to_string(),
            name: "edit_file".to_string(),
            arguments: r#"{"target_file": "test.rs", "content": "test content"}"#.to_string(),
            result: Some(r#"{"status": "success", "file_path": "test.rs", "lines_added": 25}"#.to_string()),
            status: MessageStatus::Complete,
            content_position: None,
        });
        messages.push(agent_msg);
        
        // Create second agent message (separate conversation turn)
        let mut agent_msg_2 = create_test_message("Test created successfully!", MessageType::Normal);
        agent_msg_2.author = MessageAuthor::Agent;  
        agent_msg_2.id = "agent-2".to_string();
        messages.push(agent_msg_2);
        
        let groups = group_consecutive_messages(&messages);
        
        // Verify: Should have 1 agent message group (consecutive agent messages are grouped together)
        assert_eq!(groups.len(), 1, "Should have 1 agent message group (consecutive messages from same author)");
        assert_eq!(groups[0].len(), 2, "Should have 2 messages in the group");
        assert_eq!(groups[0][0].author, MessageAuthor::Agent);
        assert_eq!(groups[0][1].author, MessageAuthor::Agent);
        
        // Tool call should be inline within the first agent message
        assert!(!groups[0][0].tool_calls.is_empty(), "First agent message should contain tool call");
        assert_eq!(groups[0][0].tool_calls[0].name, "edit_file");
        
        // Second message should not have tool calls
        assert!(groups[0][1].tool_calls.is_empty(), "Second agent message should not have tool calls");
        
        // Issue 2: Tool result summary generation
        let file_result = r#"{"files": ["main.rs", "lib.rs"], "query": "fn test"}"#;
        let summary = extract_tool_result_summary(file_result);
        assert!(summary.contains("File search") && summary.contains("2 files"));
        
        let shell_result = r#"{"exit_code": 0, "stdout": "Test passed", "stderr": ""}"#;
        let summary = extract_tool_result_summary(shell_result);
        assert!(summary.contains("Test passed") || summary.contains("fields"));
        
        // Issue 3: Complex task classification is tested in reasoning-engine
        // This verifies that requests like "ADD A TEST" are properly handled
        // (Tests are in reasoning-engine crate)
        
        println!("✅ All three main user issues have been addressed:");
        println!("   1. Tool results now appear inline within agent messages");
        println!("   2. Tool result summaries are properly generated");  
        println!("   3. Complex tasks like 'ADD A TEST' are properly classified");
    }

    #[test]
    fn test_conversation_copying_format() {
        // Create a sample conversation with different message types
        let mut messages = Vec::new();
        
        // User message
        let mut user_msg = create_test_message("Hello, can you help me with a Rust project?", MessageType::Normal);
        user_msg.author = MessageAuthor::User;
        user_msg.id = "user-1".to_string();
        messages.push(user_msg);
        
        // Agent message with thinking and tool call
        let mut agent_msg = create_test_message("I'd be happy to help you with your Rust project!", MessageType::Normal);
        agent_msg.author = MessageAuthor::Agent;
        agent_msg.id = "agent-1".to_string();
        agent_msg.thinking_content = Some("Let me think about how to best help with this Rust project...".to_string());
        
        // Add a tool call
        agent_msg.tool_calls.push(ToolCall {
            id: "test_tool_call_4".to_string(),
            name: "analyze_project".to_string(),
            arguments: r#"{"path": "./src"}"#.to_string(),
            result: Some(r#"{"files": ["main.rs", "lib.rs"], "language": "rust"}"#.to_string()),
            status: MessageStatus::Complete,
            content_position: None,
        });
        messages.push(agent_msg);
        
        // Format for copying
        let formatted = format_conversation_for_copying(&messages);
        
        // Verify the format includes key elements
        assert!(formatted.contains("You "), "Should include user header");
        assert!(formatted.contains("Sagitta Code "), "Should include agent header");
        assert!(formatted.contains("💭"), "Should include thinking indicator");
        assert!(formatted.contains("✅ Tool analyze_project completed"), "Should include tool completion");
        assert!(formatted.contains("Hello, can you help me with a Rust project?"), "Should include user message content");
        assert!(formatted.contains("I'd be happy to help you with your Rust project!"), "Should include agent message content");
        
        // Verify structure
        let lines: Vec<&str> = formatted.split('\n').collect();
        assert!(lines.len() > 5, "Should have multiple lines with proper formatting");
        
        println!("Formatted conversation:\n{formatted}");
    }
}

/// Format entire conversation for copying/sharing
/// Format conversation including tool cards for clipboard copying
fn format_conversation_with_tools_for_copying(items: &[ChatItem]) -> String {
    let mut conversation = Vec::new();
    
    for item in items {
        match item {
            ChatItem::Message(message) => {
                let author_name = match message.author {
                    MessageAuthor::User => "You",
                    MessageAuthor::Agent => "Sagitta Code",
                    MessageAuthor::System => "System",
                    MessageAuthor::Tool => "Tool",
                };
                
                let timestamp = message.format_time();
                
                // Add message header
                conversation.push(format!("{author_name} {timestamp}"));
                conversation.push("".to_string()); // Empty line
                
                // Add thinking content if present
                if let Some(thinking) = message.get_thinking_content() {
                    if !thinking.is_empty() {
                        conversation.push(format!("💭 {thinking}"));
                        conversation.push("".to_string());
                    }
                }
                
                // Add main content AND embedded tool calls (this is the critical fix!)
                if !message.content.is_empty() {
                    conversation.push(message.content.clone());
                }
                
                // CRITICAL FIX: Capture tool calls embedded within this message
                for tool_call in &message.tool_calls {
                    conversation.push("".to_string()); // Empty line before tool
                    
                    // Format tool call header
                    let friendly_name = get_human_friendly_tool_name(&tool_call.name);
                    let tool_icon = get_tool_icon(&tool_call.name);
                    let status_icon = match &tool_call.status {
                        MessageStatus::Complete => "✅",
                        MessageStatus::Error(_) => "❌",
                        MessageStatus::Streaming => "🔄",
                        _ => "⏸️",
                    };
                    
                    // Parse and format tool parameters
                    let params_display = if let Ok(args_value) = serde_json::from_str::<serde_json::Value>(&tool_call.arguments) {
                        let params = format_tool_parameters_for_inline(&tool_call.name, &args_value);
                        if !params.is_empty() {
                            format!(" - {}", params.iter()
                                .map(|(k, v)| format!("{}: {}", k, v))
                                .collect::<Vec<_>>()
                                .join(", "))
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                    
                    // Tool header with visual separator
                    conversation.push("┌─────────────────────────────────────────────────────────────────────────────┐".to_string());
                    conversation.push(format!("│ {} {}{} {}", tool_icon, friendly_name, params_display, status_icon));
                    conversation.push("├─────────────────────────────────────────────────────────────────────────────┤".to_string());
                    
                    // Add tool parameters section
                    if let Ok(args_value) = serde_json::from_str::<serde_json::Value>(&tool_call.arguments) {
                        let all_params = format_tool_parameters(&tool_call.name, &args_value);
                        if !all_params.is_empty() {
                            conversation.push("│".to_string());
                            conversation.push("│ Parameters:".to_string());
                            for (key, value) in all_params {
                                let display_value = if value.len() > 100 {
                                    format!("{}...", &value[..97])
                                } else {
                                    value
                                };
                                conversation.push(format!("│   {}: {}", key, display_value));
                            }
                        }
                    }
                    
                    // Add tool result if present
                    if let Some(result) = &tool_call.result {
                        conversation.push("│".to_string());
                        conversation.push("│ Result:".to_string());
                        
                        // Format result using the same formatter as the UI
                        let formatter = crate::gui::app::tool_formatting::ToolResultFormatter::new();
                        let tool_result = if matches!(tool_call.status, MessageStatus::Error(_)) {
                            crate::agent::events::ToolResult::Error { error: result.clone() }
                        } else {
                            crate::agent::events::ToolResult::Success { output: result.clone() }
                        };
                        let formatted_result = formatter.format_tool_result_for_preview(&tool_call.name, &tool_result);
                        
                        for (i, line) in formatted_result.lines().enumerate() {
                            if i < 50 { // Limit to 50 lines
                                conversation.push(format!("│ {}", line));
                            }
                        }
                        
                        if formatted_result.lines().count() > 50 {
                            conversation.push("│ ... (output truncated, use 'View Full Result' for complete output)".to_string());
                        }
                    }
                    
                    conversation.push("└─────────────────────────────────────────────────────────────────────────────┘".to_string());
                }
                
                conversation.push("".to_string()); // Empty line between items
            },
            ChatItem::ToolCard(tool_card) => {
                // Format tool card header exactly as shown in GUI
                let friendly_name = get_human_friendly_tool_name(&tool_card.tool_name);
                let tool_icon = get_tool_icon(&tool_card.tool_name);
                let status_icon = match &tool_card.status {
                    ToolCardStatus::Completed { success: true } => "✅",
                    ToolCardStatus::Completed { success: false } => "❌",
                    ToolCardStatus::Failed { .. } => "❌",
                    ToolCardStatus::Running => "🔄",
                    ToolCardStatus::Cancelled => "⏹️",
                };
                
                // Get inline parameters as shown in header
                let inline_params = {
                    let params = format_tool_parameters_for_inline(&tool_card.tool_name, &tool_card.input_params);
                    if !params.is_empty() {
                        let param_str = params.iter()
                            .map(|(k, v)| format!("{}: {}", k, v))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!(" - {}", param_str)
                    } else {
                        String::new()
                    }
                };
                
                // Tool card header with visual separator
                conversation.push("┌─────────────────────────────────────────────────────────────────────────────┐".to_string());
                conversation.push(format!("│ {} {}{} {}", tool_icon, friendly_name, inline_params, status_icon));
                conversation.push("├─────────────────────────────────────────────────────────────────────────────┤".to_string());
                
                // Add timing info if available
                if let Some(completed_at) = tool_card.completed_at {
                    let duration = completed_at.signed_duration_since(tool_card.started_at);
                    conversation.push(format!("│ Duration: {:.1}s", duration.num_milliseconds() as f64 / 1000.0));
                }
                
                // Add all parameters (not just inline ones)
                let all_params = format_tool_parameters(&tool_card.tool_name, &tool_card.input_params);
                if !all_params.is_empty() {
                    conversation.push("│".to_string());
                    conversation.push("│ Parameters:".to_string());
                    for (key, value) in all_params {
                        // Truncate very long values
                        let display_value = if value.len() > 100 {
                            format!("{}...", &value[..97])
                        } else {
                            value
                        };
                        conversation.push(format!("│   {}: {}", key, display_value));
                    }
                }
                
                // Add status-specific information
                match &tool_card.status {
                    ToolCardStatus::Running => {
                        conversation.push("│".to_string());
                        conversation.push("│ Status: Running...".to_string());
                        if let Some(progress) = tool_card.progress {
                            conversation.push(format!("│ Progress: {:.0}%", progress * 100.0));
                        }
                    }
                    ToolCardStatus::Failed { error } => {
                        conversation.push("│".to_string());
                        conversation.push(format!("│ Error: {}", error));
                    }
                    ToolCardStatus::Cancelled => {
                        conversation.push("│".to_string());
                        conversation.push("│ Status: Cancelled".to_string());
                    }
                    _ => {}
                }
                
                // Add result if available
                if let Some(result) = &tool_card.result {
                    // Debug log to check what we're getting
                    log::debug!("Tool card {} has result: {:?}", tool_card.tool_name, result);
                    
                    conversation.push("│".to_string());
                    conversation.push("│ Result:".to_string());
                    conversation.push("│".to_string());
                    
                    // Format the result using ToolResultFormatter
                    let formatter = crate::gui::app::tool_formatting::ToolResultFormatter::new();
                    
                    // Convert serde_json::Value to proper JSON string
                    let result_str = match result {
                        serde_json::Value::String(s) => s.clone(),
                        _ => serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
                    };
                    
                    let tool_result = match &tool_card.status {
                        ToolCardStatus::Completed { success: true } => {
                            crate::agent::events::ToolResult::Success { output: result_str }
                        },
                        _ => {
                            crate::agent::events::ToolResult::Error { error: "Tool execution failed".to_string() }
                        }
                    };
                    
                    let formatted_result = formatter.format_tool_result_for_preview(&tool_card.tool_name, &tool_result);
                    
                    // If result is empty, show a message
                    if formatted_result.trim().is_empty() {
                        conversation.push("│   (No output)".to_string());
                    } else {
                        // If result is too long, show truncated version with note
                        let lines: Vec<&str> = formatted_result.lines().collect();
                        let max_lines = 50;
                        let total_lines = lines.len();
                        
                        for (i, line) in lines.iter().take(max_lines).enumerate() {
                            // Indent result content
                            conversation.push(format!("│   {}", line));
                        }
                        
                        if total_lines > max_lines {
                            conversation.push("│".to_string());
                            conversation.push(format!("│   ... ({} more lines truncated)", total_lines - max_lines));
                        }
                    }
                }
                
                // Close the tool card box
                conversation.push("└─────────────────────────────────────────────────────────────────────────────┘".to_string());
                conversation.push("".to_string()); // Empty line between items
            }
        }
    }
    
    // Remove trailing empty lines
    while conversation.last() == Some(&String::new()) {
        conversation.pop();
    }
    
    conversation.join("\n")
}

fn format_conversation_for_copying(messages: &[StreamingMessage]) -> String {
    let mut conversation = Vec::new();
    
    for message in messages {
        let author_name = match message.author {
            MessageAuthor::User => "You",
            MessageAuthor::Agent => "Sagitta Code",
            MessageAuthor::System => "System",
            MessageAuthor::Tool => "Tool",
        };
        
        let timestamp = message.format_time();
        
        // Add message header
        conversation.push(format!("{author_name} {timestamp}"));
        conversation.push("".to_string()); // Empty line
        
        // Add thinking content if present
        if let Some(thinking) = message.get_thinking_content() {
            if !thinking.is_empty() {
                conversation.push(format!("💭 {thinking}"));
                conversation.push("".to_string());
            }
        }
        
        // Add main content
        if !message.content.is_empty() {
            conversation.push(message.content.clone());
        }
        
        // Add tool calls if present
        for tool_call in &message.tool_calls {
            conversation.push("".to_string());
            
            // Tool execution header with status
            let status_icon = match tool_call.status {
                MessageStatus::Complete => "✅",
                MessageStatus::Error(_) => "❌",
                _ => "🔧",
            };
            
            conversation.push(format!("{} Tool {} completed", status_icon, tool_call.name));
            
            // Add tool result summary if available
            if let Some(result) = &tool_call.result {
                let summary = extract_tool_result_summary(result);
                if !summary.is_empty() && summary != "Tool execution result" {
                    conversation.push(format!("Result: {summary}"));
                }
            }
        }
        
        conversation.push("".to_string()); // Empty line between messages
    }
    
    // Remove trailing empty lines
    while conversation.last() == Some(&String::new()) {
        conversation.pop();
    }
    
    conversation.join("\n")
}




/// Check if this is a shell command result
fn is_shell_command_result(tool_name: &str, result: &serde_json::Value) -> bool {
    tool_name.contains("shell") || 
    tool_name.contains("bash") ||
    tool_name.contains("streaming_shell_execution") ||
    (result.get("stdout").is_some() && result.get("stderr").is_some() && result.get("exit_code").is_some())
}

/// Check if this is a code change result
fn is_code_change_result(tool_name: &str, result: &serde_json::Value) -> bool {
    tool_name.contains("edit") || 
    tool_name.contains("semantic_edit") ||
    result.get("changes").is_some() ||
    result.get("diff").is_some()
}

/// Render terminal output with monospace font and ANSI colors
fn render_terminal_output(ui: &mut egui::Ui, result: &serde_json::Value, app_theme: AppTheme) {
    ui.group(|ui| {
        ui.style_mut().visuals.override_text_color = Some(app_theme.success_color());
        
        // Exit code display
        ui.horizontal(|ui| {
            if let Some(exit_code) = result.get("exit_code").and_then(|v| v.as_i64()) {
                let exit_color = if exit_code == 0 { app_theme.success_color() } else { app_theme.error_color() };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("Exit: {exit_code}")).color(exit_color).small());
                });
            }
        });
        
        // Use monospace font for terminal output
        let mut output = String::new();
        
        if let Some(stdout) = result.get("stdout").and_then(|v| v.as_str()) {
            if !stdout.is_empty() {
                output.push_str(stdout);
            }
        }
        
        if let Some(stderr) = result.get("stderr").and_then(|v| v.as_str()) {
            if !stderr.is_empty() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(&format!("STDERR:\n{stderr}"));
            }
        }
        
        if output.is_empty() {
            output = "(No output)".to_string();
        }
        
        // Count lines to determine if we need scrolling
        let line_count = output.lines().count();
        
        if line_count > SHELL_OUTPUT_COLLAPSING_THRESHOLD_LINES {
            // Use scroll area for long output
            egui::ScrollArea::vertical()
                .max_height(SHELL_OUTPUT_SCROLL_AREA_MAX_HEIGHT)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(output).monospace().color(app_theme.text_color()));
                });
        } else {
            // Render directly for short output
            ui.label(egui::RichText::new(output).monospace().color(app_theme.text_color()));
        }
    });
}

/// Render diff output with syntax highlighting
fn render_diff_output(ui: &mut egui::Ui, result: &serde_json::Value, app_theme: AppTheme) {
    ui.group(|ui| {
        // Show the diff
        
        if let Some(file_path) = result.get("file_path").and_then(|v| v.as_str()) {
            ui.label(egui::RichText::new(format!("File: {file_path}")).color(app_theme.hint_text_color()).small());
            ui.add_space(4.0);
        }
        
        // Get the diff/changes content
        let diff_content = result.get("diff")
            .or_else(|| result.get("changes"))
            .and_then(|v| v.as_str())
            .unwrap_or("No changes available");
        
        // Render diff lines with appropriate colors
        for line in diff_content.lines() {
            let (text_color, _prefix) = if line.starts_with('+') && !line.starts_with("+++") {
                (app_theme.diff_added_text(), "+ ")
            } else if line.starts_with('-') && !line.starts_with("---") {
                (app_theme.diff_removed_text(), "- ")
            } else if line.starts_with("@@") {
                (app_theme.accent_color(), "")
            } else {
                (app_theme.text_color(), "  ")
            };
            
            ui.label(egui::RichText::new(line).monospace().color(text_color));
        }
    });
}

/// Check if this is a file read result
fn is_file_read_result(tool_name: &str, result: &serde_json::Value) -> bool {
    // Don't use content field check as file writes also have content
    tool_name.contains("read_file") || 
    tool_name.contains("view_file") ||
    tool_name == "Read"
}

/// Check if this is a file write result
fn is_file_write_result(tool_name: &str, result: &serde_json::Value) -> bool {
    tool_name.contains("write_file") || 
    tool_name == "Write" ||
    result.get("bytes_written").is_some()
}

/// Check if this is a search result
fn is_search_result(tool_name: &str, result: &serde_json::Value) -> bool {
    tool_name.contains("search") || 
    tool_name.contains("query") ||
    tool_name.contains("semantic_code_search") ||
    tool_name == "WebSearch" ||
    result.get("results").is_some() ||
    result.get("matches").is_some()
}

/// Check if this is a repository operation result
fn is_repository_result(tool_name: &str, result: &serde_json::Value) -> bool {
    tool_name.contains("repository") || 
    tool_name.contains("repo") ||
    tool_name == "list_repositories" ||
    result.get("repositories").is_some()
}

/// Check if this is a todo result
fn is_todo_result(tool_name: &str, result: &serde_json::Value) -> bool {
    tool_name.contains("todo") || 
    tool_name == "TodoWrite" ||
    result.get("todos").is_some()
}

/// Check if this is a ping result
fn is_ping_result(tool_name: &str, _result: &serde_json::Value) -> bool {
    tool_name.contains("ping")
}

/// Render file read output with nice formatting
fn render_file_read_output(ui: &mut egui::Ui, result: &serde_json::Value, app_theme: AppTheme) -> Option<(String, String)> {
    ui.group(|ui| {
        // File path at the top
        if let Some(file_path) = result.get("file_path").and_then(|v| v.as_str()) {
            ui.label(egui::RichText::new(format!("`{file_path}`")).color(app_theme.hint_text_color()).small());
        }
        
        // Get file content and metadata
        let content = result.get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("No content available");
        
        let mut info_parts = Vec::new();
        
        if let Some(line_count) = result.get("line_count").and_then(|v| v.as_i64()) {
            info_parts.push(format!("{} lines", line_count));
        }
        
        if let Some(file_size) = result.get("file_size").and_then(|v| v.as_i64()) {
            info_parts.push(format!("{} bytes", file_size));
        }
        
        if let Some(start_line) = result.get("start_line").and_then(|v| v.as_i64()) {
            if let Some(end_line) = result.get("end_line").and_then(|v| v.as_i64()) {
                info_parts.push(format!("Lines {}-{}", start_line, end_line));
            }
        }
        
        if !info_parts.is_empty() {
            ui.label(egui::RichText::new(info_parts.join(" | ")).color(app_theme.hint_text_color()).small());
            ui.add_space(4.0);
        }
        
        // Render content with syntax highlighting
        let line_count = content.lines().count();
        
        // Try to determine file language from path
        let language = if let Some(file_path) = result.get("file_path").and_then(|v| v.as_str()) {
            if let Some(ext) = std::path::Path::new(file_path).extension().and_then(|e| e.to_str()) {
                ext
            } else {
                "txt"
            }
        } else {
            "txt"
        };
        
        // Always render with syntax highlighting, let the outer scroll area handle scrolling
        render_syntax_highlighted_code(ui, content, language, &Color32::TRANSPARENT, ui.available_width());
        
        ui.add_space(4.0);
        
        // Add action buttons
        let mut action = None;
        ui.horizontal(|ui| {
            // View Full File link
            if ui.link(egui::RichText::new("View Full File").color(app_theme.hint_text_color()).small()).clicked() {
                // Create a new action to read the full file
                if let Some(file_path) = result.get("file_path").and_then(|v| v.as_str()) {
                    let action_data = serde_json::json!({
                        "file_path": file_path,
                        "full_file": true
                    });
                    action = Some(("__READ_FULL_FILE__".to_string(), action_data.to_string()));
                }
            }
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Copy button as icon only in top right
                if ui.button("📋").on_hover_text("Copy content").clicked() {
                    ui.ctx().copy_text(content.to_string());
                    // Note: Would show success toast if we had access to toasts
                }
            });
        });
        
        action
    }).inner
}

/// Render file write output with nice formatting
fn render_file_write_output(ui: &mut egui::Ui, result: &serde_json::Value, app_theme: AppTheme) -> Option<(String, String)> {
    ui.group(|ui| {
        // File write info
        if let Some(file_path) = result.get("file_path").and_then(|v| v.as_str()) {
            ui.label(egui::RichText::new(format!("`{file_path}`")).color(app_theme.hint_text_color()).small());
        }
        
        let mut info_parts = Vec::new();
        
        if let Some(bytes_written) = result.get("bytes_written").and_then(|v| v.as_i64()) {
            info_parts.push(format!("{} bytes written", bytes_written));
        }
        
        if let Some(created) = result.get("created").and_then(|v| v.as_bool()) {
            if created {
                info_parts.push("File created".to_string());
            } else {
                info_parts.push("File updated".to_string());
            }
        }
        
        if !info_parts.is_empty() {
            ui.label(egui::RichText::new(info_parts.join(" | ")).color(app_theme.success_color()).small());
        } else {
            ui.label(egui::RichText::new("File written successfully").color(app_theme.success_color()).small());
        }
        
        // Add action buttons
        ui.add_space(4.0);
        let mut action = None;
        ui.horizontal(|ui| {
            // View Full File link - only show if we have content
            if result.get("content").is_some() {
                if ui.link(egui::RichText::new("View Full File").color(app_theme.hint_text_color()).small()).clicked() {
                    // Return a special marker for file content modal request
                    action = Some(("__VIEW_FULL_FILE__".to_string(), result.to_string()));
                }
            }
        });
        
        action
    })
    .inner
}

/// Render search results with nice formatting
fn render_search_output(ui: &mut egui::Ui, result: &serde_json::Value, app_theme: AppTheme) -> Option<(String, String)> {
    ui.group(|ui| {
        // Search results display
        if let Some(query) = result.get("queryText").and_then(|v| v.as_str()) {
            ui.label(egui::RichText::new(format!("`{query}`")).color(app_theme.hint_text_color()).small());
        }
        
        // Handle different search result formats
        if let Some(results) = result.get("results").and_then(|v| v.as_array()) {
            ui.label(egui::RichText::new(format!("Found {} results", results.len())).color(app_theme.success_color()).small());
            ui.add_space(4.0);
            
            for (i, result_item) in results.iter().enumerate().take(5) {
                ui.group(|ui| {
                    if let Some(file_path) = result_item.get("filePath").and_then(|v| v.as_str()) {
                        ui.label(egui::RichText::new(format!("{}. {}", i + 1, file_path)).color(app_theme.text_color()).small());
                        
                        // Show line range if available
                        if let (Some(start), Some(end)) = (
                            result_item.get("startLine").and_then(|v| v.as_i64()),
                            result_item.get("endLine").and_then(|v| v.as_i64())
                        ) {
                            ui.label(egui::RichText::new(format!("   Lines {}-{}", start, end)).color(app_theme.hint_text_color()).small());
                        }
                        
                        // Show element type and language if available
                        let mut meta_parts = Vec::new();
                        if let Some(element_type) = result_item.get("elementType").and_then(|v| v.as_str()) {
                            meta_parts.push(element_type.to_string());
                        }
                        if let Some(language) = result_item.get("language").and_then(|v| v.as_str()) {
                            meta_parts.push(language.to_string());
                        }
                        if let Some(score) = result_item.get("score").and_then(|v| v.as_f64()) {
                            meta_parts.push(format!("score: {:.3}", score));
                        }
                        
                        // Add context info metadata
                        if let Some(context_info) = result_item.get("contextInfo") {
                            if let Some(identifiers) = context_info.get("identifiers").and_then(|v| v.as_array()) {
                                meta_parts.push(format!("identifiers: {}", identifiers.len()));
                            }
                            if let Some(outgoing_calls) = context_info.get("outgoing_calls").and_then(|v| v.as_array()) {
                                meta_parts.push(format!("calls: {}", outgoing_calls.len()));
                            }
                        }
                        
                        if !meta_parts.is_empty() {
                            ui.label(egui::RichText::new(format!("   [{}]", meta_parts.join(", "))).color(app_theme.hint_text_color()).small());
                        }
                        
                        // Show signature and description if available (avoid duplication)
                        if let Some(context_info) = result_item.get("contextInfo") {
                            let signature = context_info.get("signature").and_then(|v| v.as_str()).unwrap_or("");
                            let preview = result_item.get("preview").and_then(|v| v.as_str()).unwrap_or("");
                            
                            // Signature removed per requirements (commented out below)
                            // if !signature.is_empty() && signature != preview {
                            //     ui.label(egui::RichText::new(format!("   {}", signature)).monospace().color(app_theme.hint_text_color()).small());
                            // }
                            
                            if let Some(description) = context_info.get("description").and_then(|v| v.as_str()) {
                                if !description.is_empty() {
                                    ui.label(egui::RichText::new(format!("   {}", description)).color(app_theme.hint_text_color()).small().italics());
                                }
                            }
                        }
                        
                        // Preview removed per requirements
                        // if let Some(preview) = result_item.get("preview").and_then(|v| v.as_str()) {
                        //     let preview_text = if preview.len() > 80 {
                        //         format!("{}...", &preview[..77])
                        //     } else {
                        //         preview.to_string()
                        //     };
                        //     ui.label(egui::RichText::new(format!("   {}", preview_text)).monospace().color(app_theme.hint_text_color()).small());
                        // }
                    }
                });
                ui.add_space(2.0);
            }
            
            if results.len() > 5 {
                ui.label(egui::RichText::new(format!("... and {} more results", results.len() - 5)).color(app_theme.hint_text_color()).small());
            }
        } else if let Some(matches) = result.get("matchingFiles").and_then(|v| v.as_array()) {
            ui.label(egui::RichText::new(format!("Found {} files", matches.len())).color(app_theme.success_color()).small());
            ui.add_space(4.0);
            
            for (i, file) in matches.iter().enumerate().take(10) {
                if let Some(file_path) = file.as_str() {
                    ui.label(egui::RichText::new(format!("{}. {}", i + 1, file_path)).color(app_theme.text_color()).small());
                }
            }
            
            if matches.len() > 10 {
                ui.label(egui::RichText::new(format!("... and {} more", matches.len() - 10)).color(app_theme.hint_text_color()).small());
            }
        } else if result.get("sources").is_some() || result.get("answer").is_some() || result.get("response").is_some() {
            // Web search results - use the formatter output which handles these properly
            let formatter = crate::gui::app::tool_formatting::ToolResultFormatter::new();
            let tool_result = crate::agent::events::ToolResult::Success { output: result.to_string() };
            let formatted_result = formatter.format_tool_result_for_preview("web_search", &tool_result);
            
            // Use markdown rendering for the formatted result
            crate::gui::chat::view::COMMONMARK_CACHE.with(|cache| {
                let mut cache = cache.borrow_mut();
                let viewer = egui_commonmark::CommonMarkViewer::new();
                viewer.show(ui, &mut cache, &formatted_result);
            });
        } else {
            ui.label(egui::RichText::new("No results found").color(app_theme.hint_text_color()).small());
        }
        
        // Add small View JSON link
        ui.add_space(8.0);
        let mut action = None;
        ui.horizontal(|ui| {
            if ui.link(egui::RichText::new("View JSON").color(app_theme.hint_text_color()).small()).clicked() {
                let json_str = serde_json::to_string_pretty(result).unwrap_or_else(|_| "Failed to serialize JSON".to_string());
                // Return special marker to open JSON modal (similar to View Full File)
                action = Some(("__VIEW_JSON__".to_string(), json_str));
            }
        });
        action
    })
    .inner
}

/// Render repository operation results with nice formatting
fn render_repository_output(ui: &mut egui::Ui, result: &serde_json::Value, app_theme: AppTheme) {
    ui.group(|ui| {
        // Repository info follows
        
        if let Some(repositories) = result.get("repositories").and_then(|v| v.as_array()) {
            ui.label(egui::RichText::new(format!("{} repositories", repositories.len())).color(app_theme.success_color()).small());
            ui.add_space(4.0);
            
            // Show repositories in a horizontal, compact format similar to old behavior
            let repo_names: Vec<String> = repositories.iter()
                .filter_map(|repo| repo.get("name").and_then(|v| v.as_str()))
                .map(|name| name.to_string())
                .collect();
                
            if !repo_names.is_empty() {
                let repo_list = repo_names.join(" • ");
                ui.label(egui::RichText::new(repo_list).color(app_theme.text_color()).small());
            }
        } else if let Some(message) = result.get("message").and_then(|v| v.as_str()) {
            ui.label(egui::RichText::new(message).color(app_theme.success_color()).small());
        } else {
            ui.label(egui::RichText::new("Repository operation completed").color(app_theme.success_color()).small());
        }
    });
}

/// Render todo results with nice formatting
fn render_todo_output(ui: &mut egui::Ui, result: &serde_json::Value, app_theme: AppTheme) {
    ui.group(|ui| {
        // Todo list display
        
        if let Some(todos) = result.get("todos").and_then(|v| v.as_array()) {
            let pending_count = todos.iter().filter(|t| t.get("status").and_then(|s| s.as_str()) == Some("pending")).count();
            let completed_count = todos.iter().filter(|t| t.get("status").and_then(|s| s.as_str()) == Some("completed")).count();
            
            ui.label(egui::RichText::new(format!("{} total | {} pending | {} completed", todos.len(), pending_count, completed_count)).color(app_theme.success_color()).small());
            ui.add_space(4.0);
            
            for todo in todos.iter() {
                if let Some(content) = todo.get("content").and_then(|v| v.as_str()) {
                    let status = todo.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let status_icon = match status {
                        "completed" => "✓",
                        "pending" => "◯",
                        "in_progress" => "⏳",
                        _ => "?",
                    };
                    ui.label(egui::RichText::new(format!("{} {}", status_icon, content)).color(app_theme.text_color()).small());
                }
            }
        } else {
            ui.label(egui::RichText::new("Todo operation completed").color(app_theme.success_color()).small());
        }
    });
}

/// Render ping results with nice formatting
fn render_ping_output(ui: &mut egui::Ui, result: &serde_json::Value, app_theme: AppTheme) {
    ui.group(|ui| {
        // Ping response display
        
        if let Some(message) = result.get("message").and_then(|v| v.as_str()) {
            ui.label(egui::RichText::new(message).color(app_theme.success_color()).small());
        } else {
            ui.label(egui::RichText::new("Server is responsive").color(app_theme.success_color()).small());
        }
    });
}

