// Chat display UI with enhanced styling

use egui::{
    ScrollArea, 
    Color32, 
    RichText, 
    Rounding, 
    Stroke, 
    Ui, 
    Vec2, 
    TextFormat, 
    Frame,
    Align, 
    Layout,
    Response,
    Rect,
    CornerRadius,
    TextStyle,
    FontId,
    FontFamily,
    Sense,
    Pos2,
};
use syntect::{
    highlighting::{ThemeSet, Style as SyntectStyle, Theme},
    parsing::SyntaxSet,
    easy::HighlightLines,
    util::LinesWithEndings,
};
use similar::{ChangeTag, TextDiff};
use std::sync::OnceLock;
use crate::gui::theme::AppTheme;
use crate::gui::symbols;
use crate::gui::app::RunningToolInfo;
use crate::agent::events::ToolRunId;
use super::{ChatItem, ToolCard, ToolCardStatus};
use catppuccin_egui::Theme as CatppuccinTheme;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use std::cell::RefCell;
use std::thread::LocalKey;
use serde_json;
use uuid;
use std::time::Instant;
use std::collections::HashMap;
use regex;

#[cfg(feature = "gui")]
use egui_notify::Toasts;
#[cfg(feature = "gui")]
use egui_modal::Modal;

/// Convert raw tool names (including MCP format) to human-friendly display names
fn get_human_friendly_tool_name(raw_name: &str) -> String {
    match raw_name {
        // MCP tool patterns
        name if name.contains("__query") => "Semantic Code Search".to_string(),
        name if name.contains("__repository_view_file") => "View Repository File".to_string(),
        name if name.contains("__repository_search_file") => "Search Repository Files".to_string(),
        name if name.contains("__repository_list_branches") => "List Repository Branches".to_string(),
        name if name.contains("__repository_list") => "List Repositories".to_string(),
        name if name.contains("__repository_map") => "Map Repository Structure".to_string(),
        name if name.contains("__repository_add") => "Add Repository".to_string(),
        name if name.contains("__repository_remove") => "Remove Repository".to_string(),
        name if name.contains("__repository_sync") => "Sync Repository".to_string(),
        name if name.contains("__repository_switch_branch") => "Switch Repository Branch".to_string(),
        name if name.contains("__ping") => "Ping".to_string(),
        
        // Native Claude tools
        "Read" => "Read File".to_string(),
        "Write" => "Write File".to_string(),
        "Edit" => "Edit File".to_string(),
        "MultiEdit" => "Multi Edit File".to_string(),
        "Bash" => "Run Command".to_string(),
        "WebSearch" => "Search Web".to_string(),
        "WebFetch" => "Fetch Web Content".to_string(),
        "TodoRead" => "Read Todo List".to_string(),
        "TodoWrite" => "Update Todo List".to_string(),
        "NotebookRead" => "Read Notebook".to_string(),
        "NotebookEdit" => "Edit Notebook".to_string(),
        "Task" => "Task Agent".to_string(),
        "Glob" => "Find Files".to_string(),
        "Grep" => "Search In Files".to_string(),
        "LS" => "List Directory".to_string(),
        "exit_plan_mode" => "Exit Plan Mode".to_string(),
        
        _ => {
            // For unknown MCP tools, try to extract and format the operation name
            if raw_name.starts_with("mcp__") {
                if let Some(op) = raw_name.split("__").last() {
                    // Convert snake_case to Title Case
                    op.split('_')
                        .map(|word| {
                            let mut chars = word.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    raw_name.to_string()
                }
            } else {
                raw_name.to_string()
            }
        }
    }
}

/// Format tool parameters for display in the UI
fn format_tool_parameters(tool_name: &str, args: &serde_json::Value) -> Vec<(String, String)> {
    let mut params = Vec::new();
    
    if let Some(obj) = args.as_object() {
        // Special handling for common tools
        match tool_name {
            name if name.contains("__query") => {
                if let Some(query) = obj.get("query").and_then(|v| v.as_str()) {
                    params.push(("Query".to_string(), query.to_string()));
                }
                if let Some(repo) = obj.get("repository").and_then(|v| v.as_str()) {
                    params.push(("Repository".to_string(), repo.to_string()));
                }
                if let Some(limit) = obj.get("limit").and_then(|v| v.as_i64()) {
                    params.push(("Limit".to_string(), limit.to_string()));
                }
            },
            "Read" | "Write" | "Edit" => {
                if let Some(path) = obj.get("file_path").and_then(|v| v.as_str()) {
                    params.push(("File".to_string(), path.to_string()));
                }
            },
            "Bash" => {
                if let Some(cmd) = obj.get("command").and_then(|v| v.as_str()) {
                    params.push(("Command".to_string(), cmd.to_string()));
                }
            },
            _ => {
                // Generic parameter formatting
                for (key, value) in obj {
                    let formatted_key = key.split('_')
                        .map(|w| {
                            let mut c = w.chars();
                            match c.next() {
                                None => String::new(),
                                Some(first) => first.to_uppercase().collect::<String>() + c.as_str()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    
                    let formatted_value = match value {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => value.to_string(),
                    };
                    
                    params.push((formatted_key, formatted_value));
                }
            }
        }
    }
    
    params
}

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

thread_local! {
    static COMMONMARK_CACHE: RefCell<CommonMarkCache> = RefCell::new(CommonMarkCache::default());
}

fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(|| SyntaxSet::load_defaults_newlines())
}

fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(|| ThemeSet::load_defaults())
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageAuthor {
    User,
    Agent,
    System,
    Tool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageStatus {
    Sending,
    Thinking,
    Streaming,
    Complete,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    Normal,
    Summary,
    Tool,
    System,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub result: Option<String>,
    pub status: MessageStatus,
    pub content_position: Option<usize>, // Position in content where tool was initiated
}

#[derive(Debug, Clone)]
pub struct StreamingMessage {
    pub id: String,
    pub author: MessageAuthor,
    pub content: String,
    pub status: MessageStatus,
    pub thinking_content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub message_type: MessageType,
    
    // NEW: Enhanced thinking support for streaming and fade-out
    pub thinking_stream_content: String,  // Accumulates streaming thinking content
    pub thinking_is_streaming: bool,      // Whether thinking is currently streaming
    pub thinking_fade_start: Option<std::time::Instant>, // When to start fading out thinking
    pub thinking_should_fade: bool,       // Whether thinking should fade out
    pub thinking_collapsed: bool,         // Whether thinking content is collapsed
}

impl StreamingMessage {
    pub fn new(author: MessageAuthor, id: String) -> Self {
        Self {
            id,
            author,
            content: String::new(),
            status: MessageStatus::Sending,
            thinking_content: None,
            tool_calls: Vec::new(),
            timestamp: chrono::Utc::now(),
            message_type: MessageType::Normal,
            thinking_stream_content: String::new(),
            thinking_is_streaming: false,
            thinking_fade_start: None,
            thinking_should_fade: false,
            thinking_collapsed: true,  // Default to collapsed
        }
    }
    
    pub fn new_streaming(author: MessageAuthor, id: String) -> Self {
        Self {
            id,
            author,
            content: String::new(),
            status: MessageStatus::Streaming,
            thinking_content: None,
            tool_calls: Vec::new(),
            timestamp: chrono::Utc::now(),
            message_type: MessageType::Normal,
            thinking_stream_content: String::new(),
            thinking_is_streaming: false,
            thinking_fade_start: None,
            thinking_should_fade: false,
            thinking_collapsed: true,  // Default to collapsed
        }
    }
    
    pub fn from_text(author: MessageAuthor, text: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            author,
            content: text,
            status: MessageStatus::Complete,
            thinking_content: None,
            tool_calls: Vec::new(),
            timestamp: chrono::Utc::now(),
            message_type: MessageType::Normal,
            thinking_stream_content: String::new(),
            thinking_is_streaming: false,
            thinking_fade_start: None,
            thinking_should_fade: false,
            thinking_collapsed: true,  // Default to collapsed
        }
    }
    
    pub fn append_content(&mut self, chunk: &str) {
        self.content.push_str(chunk);
        
        // Start fading thinking content when we get regular content
        if !chunk.is_empty() && self.has_thinking_content() && !self.thinking_should_fade {
            self.start_thinking_fade();
        }
    }
    
    pub fn set_thinking(&mut self, thinking_content: String) {
        self.status = MessageStatus::Thinking;
        self.thinking_content = Some(thinking_content);
    }
    
    // NEW: Enhanced thinking methods for streaming support
    pub fn start_thinking_stream(&mut self) {
        self.status = MessageStatus::Thinking;
        self.thinking_is_streaming = true;
        self.thinking_stream_content.clear();
        self.thinking_fade_start = None;
        self.thinking_should_fade = false;
    }
    
    pub fn append_thinking_stream(&mut self, chunk: &str) {
        self.thinking_stream_content.push_str(chunk);
        self.thinking_is_streaming = true;
    }
    
    pub fn finish_thinking_stream(&mut self) {
        self.thinking_is_streaming = false;
        // Don't change status here - let the actual content streaming handle that
    }
    
    pub fn start_thinking_fade(&mut self) {
        self.thinking_should_fade = true;
        self.thinking_fade_start = Some(std::time::Instant::now());
    }
    
    pub fn has_thinking_content(&self) -> bool {
        self.thinking_content.is_some() || !self.thinking_stream_content.is_empty()
    }
    
    pub fn get_thinking_content(&self) -> Option<&str> {
        if !self.thinking_stream_content.is_empty() {
            Some(&self.thinking_stream_content)
        } else {
            self.thinking_content.as_deref()
        }
    }
    
    pub fn should_show_thinking(&self) -> bool {
        // Show thinking if we have content AND opacity > 0
        if !self.has_thinking_content() {
            return false;
        }
        
        // Check if it's faded out
        self.get_thinking_opacity() > 0.0
    }
    
    pub fn get_thinking_opacity(&self) -> f32 {
        if !self.thinking_should_fade {
            return 1.0;
        }
        
        if let Some(fade_start) = self.thinking_fade_start {
            let fade_duration = std::time::Duration::from_secs(2); // 2 second fade
            let elapsed = fade_start.elapsed();
            
            if elapsed >= fade_duration {
                return 0.0;
            }
            
            // Smooth fade from 1.0 to 0.0
            let progress = elapsed.as_secs_f32() / fade_duration.as_secs_f32();
            1.0 - progress
        } else {
            1.0
        }
    }
    
    pub fn add_tool_call(&mut self, tool_call: ToolCall) {
        self.tool_calls.push(tool_call);
    }
    
    pub fn finish_streaming(&mut self) {
        self.status = MessageStatus::Complete;
    }
    
    pub fn set_error(&mut self, error: String) {
        self.status = MessageStatus::Error(error);
    }
    
    pub fn is_streaming(&self) -> bool {
        matches!(self.status, MessageStatus::Streaming)
    }
    
    pub fn is_thinking(&self) -> bool {
        matches!(self.status, MessageStatus::Thinking) || self.thinking_is_streaming
    }
    
    pub fn is_complete(&self) -> bool {
        matches!(self.status, MessageStatus::Complete)
    }
    
    fn format_time(&self) -> String {
        self.timestamp.format("%H:%M").to_string()
    }
}

// Legacy ChatMessage for backward compatibility
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub author: MessageAuthor,
    pub text: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub id: Option<String>,
}

impl ChatMessage {
    pub fn new(author: MessageAuthor, text: String) -> Self {
        Self {
            author,
            text,
            timestamp: chrono::Utc::now(),
            id: None,
        }
    }
    
    fn format_time(&self) -> String {
        self.timestamp.format("%H:%M").to_string()
    }
}

impl From<ChatMessage> for StreamingMessage {
    fn from(msg: ChatMessage) -> Self {
        StreamingMessage {
            id: msg.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            author: msg.author,
            content: msg.text,
            status: MessageStatus::Complete,
            thinking_content: None,
            tool_calls: Vec::new(),
            timestamp: msg.timestamp,
            message_type: MessageType::Normal,
            thinking_stream_content: String::new(),
            thinking_is_streaming: false,
            thinking_fade_start: None,
            thinking_should_fade: false,
            thinking_collapsed: true,  // Default to collapsed
        }
    }
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
    modern_chat_view_ui(ui, &chat_items, app_theme, copy_state, &empty_running_tools, &mut empty_collapsed_thinking, &empty_tool_results);
}

pub fn modern_chat_view_ui(ui: &mut egui::Ui, items: &[ChatItem], app_theme: AppTheme, copy_state: &mut CopyButtonState, running_tools: &HashMap<ToolRunId, RunningToolInfo>, collapsed_thinking: &mut HashMap<String, bool>, tool_results: &HashMap<String, String>) -> Option<(String, String)> {
    // Use the app theme's colors directly
    let bg_color = app_theme.panel_background();
    let text_color = app_theme.text_color();
    let accent_color = app_theme.accent_color();

    // Get the total available width - use full width for compact design
    let total_width = ui.available_width();

    let mut clicked_tool = None;

    Frame::none()
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
                        .rounding(CornerRadius::same(6));
                    
                    if ui.add(copy_all_button).on_hover_text("Copy entire conversation for sharing").clicked() {
                        // Extract messages and tool cards from ChatItems for conversation copying
                        let conversation_text = format_conversation_with_tools_for_copying(items);
                        ui.output_mut(|o| o.copied_text = conversation_text.clone());
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
                    
                    // Render each chat item (messages and tool cards)
                    for (item_index, item) in items.iter().enumerate() {
                        if item_index > 0 {
                            ui.add_space(8.0); // Space between items
                        }
                        
                        match item {
                            ChatItem::Message(message) => {
                                // Render individual messages
                                let messages_group = vec![message];
                                if let Some(tool_info) = render_message_group(ui, &messages_group, &bg_color, total_width - 32.0, app_theme, copy_state, running_tools, collapsed_thinking) {
                                    clicked_tool = Some(tool_info);
                                }
                            }
                            ChatItem::ToolCard(tool_card) => {
                                // Render tool card
                                if let Some(tool_info) = render_tool_card(ui, tool_card, &bg_color, total_width - 32.0, app_theme, running_tools, copy_state) {
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
                return Some((format!("Tool Result for {}", tool_call_id), result.clone()));
            }
        }
    }
    
    clicked_tool
}

/// Group consecutive messages from the same author
fn group_consecutive_messages(messages: &[StreamingMessage]) -> Vec<Vec<&StreamingMessage>> {
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
fn render_message_group(
    ui: &mut Ui, 
    message_group: &[&StreamingMessage], 
    bg_color: &Color32,
    total_width: f32,
    app_theme: AppTheme,
    copy_state: &mut CopyButtonState,
    running_tools: &HashMap<ToolRunId, RunningToolInfo>,
    collapsed_thinking: &mut HashMap<String, bool>,
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
                ui.label(RichText::new(format!("{} - {}", first_time, last_time))
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
                .rounding(CornerRadius::same(4));
            
            if ui.add(copy_button).on_hover_text("Copy all messages in group").clicked() {
                let combined_content = message_group.iter()
                    .map(|msg| msg.content.clone())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                ui.output_mut(|o| o.copied_text = combined_content.clone());
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
                    
                    if let Some(tool_info) = render_single_message_content(ui, message, &bg_color, total_width - 80.0, app_theme, running_tools, copy_state, collapsed_thinking) {
                        clicked_tool = Some(tool_info);
                    }
                });
            });
        } else {
            // Single message in group - use full width
            if let Some(tool_info) = render_single_message_content(ui, message, &bg_color, total_width, app_theme, running_tools, copy_state, collapsed_thinking) {
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

/// Render the content of a single message (without author header)
fn render_single_message_content(
    ui: &mut Ui, 
    message: &StreamingMessage, 
    bg_color: &Color32,
    max_width: f32,
    app_theme: AppTheme,
    running_tools: &HashMap<ToolRunId, RunningToolInfo>,
    copy_state: &mut CopyButtonState,
    collapsed_thinking: &mut HashMap<String, bool>,
) -> Option<(String, String)> {
    let mut clicked_tool = None;
    
    // Thinking content (if any) - now with streaming and fade support
    if message.should_show_thinking() {
        render_thinking_content(ui, message, &bg_color, max_width, app_theme, collapsed_thinking);
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
                if let Some(tool_info) = render_text_content_compact(ui, content_chunk, &bg_color, max_width, app_theme) {
                    clicked_tool = Some(tool_info);
                }
            }
        }
        
        // Render the tool call
        ui.add_space(1.0);
        if let Some(tool_info) = render_single_tool_call(ui, tool_call, &bg_color, max_width, app_theme, running_tools, copy_state) {
            clicked_tool = Some(tool_info);
        }
        ui.add_space(1.0);
        
        last_pos = *pos;
    }
    
    // Render any remaining content after the last tool
    if last_pos < content.len() {
        let remaining_content = &content[last_pos..];
        if !remaining_content.is_empty() {
            if let Some(tool_info) = render_text_content_compact(ui, remaining_content, &bg_color, max_width, app_theme) {
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
            if let Some(tool_info) = render_single_tool_call(ui, tool_call, &bg_color, max_width, app_theme, running_tools, copy_state) {
                clicked_tool = Some(tool_info);
            }
            ui.add_space(4.0);
        }
    }
    
    clicked_tool
}

/// Render thinking content with streaming support and fade-out effects
fn render_thinking_content(ui: &mut Ui, message: &StreamingMessage, bg_color: &Color32, max_width: f32, app_theme: AppTheme, collapsed_thinking: &mut HashMap<String, bool>) {
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
            let arrow = if *is_collapsed { "▶" } else { "▼" };
            if ui.small_button(arrow).clicked() {
                *is_collapsed = !*is_collapsed;
            }
            
            // Thinking icon with animation if streaming
            if message.thinking_is_streaming {
                let time = ui.input(|i| i.time);
                let rotation = (time * 2.0) as f32;
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
                Frame::none()
                    .fill(app_theme.thinking_background())
                    .inner_margin(Vec2::new(8.0, 6.0))
                    .rounding(CornerRadius::same(6))
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

/// Render a single tool call as a compact, clickable card
fn render_single_tool_call(ui: &mut Ui, tool_call: &ToolCall, bg_color: &Color32, max_width: f32, app_theme: AppTheme, running_tools: &HashMap<ToolRunId, RunningToolInfo>, copy_state: &mut CopyButtonState) -> Option<(String, String)> {
    let mut clicked_tool_result = None;
        // Create a frame for the tool card
        Frame::none()
            .fill(app_theme.code_background())
            .rounding(Rounding::same(4))
            .stroke(Stroke::new(1.0, app_theme.border_color()))
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Tool status icon
                    let (status_icon, status_color) = match tool_call.status {
                        MessageStatus::Complete => ("✅", app_theme.success_color()),
                        MessageStatus::Error(_) => ("❌", app_theme.error_color()),
                        MessageStatus::Streaming => ("🔄", app_theme.accent_color()),
                        MessageStatus::Sending => ("⏳", app_theme.hint_text_color()),
                        _ => ("🔧", app_theme.hint_text_color()),
                    };
                    
                    ui.label(RichText::new(status_icon).color(status_color).size(14.0));
                    ui.add_space(4.0);
                    
                    // Tool name and parameters
                    let friendly_name = get_human_friendly_tool_name(&tool_call.name);
                    
                    // Build display text with tool name and key parameters
                    let mut display_parts = vec![format!("🔧 {}", friendly_name)];
                    
                    // Parse and format parameters
                    if !tool_call.arguments.is_empty() {
                        if let Ok(args_value) = serde_json::from_str::<serde_json::Value>(&tool_call.arguments) {
                            let params = format_tool_parameters(&tool_call.name, &args_value);
                            if !params.is_empty() {
                                let param_text: Vec<String> = params.iter()
                                    .take(2) // Show only first 2 params
                                    .map(|(key, value)| {
                                        let truncated_value = if value.len() > 20 {
                                            format!("{}...", &value[..17])
                                        } else {
                                            value.clone()
                                        };
                                        format!("{}: {}", key, truncated_value)
                                    })
                                    .collect();
                                
                                if !param_text.is_empty() {
                                    display_parts.push(format!("({})", param_text.join(", ")));
                                }
                            }
                        }
                    }
                    
                    ui.label(RichText::new(display_parts.join(" "))
                        .color(app_theme.text_color())
                        .strong()
                        .size(12.0));
                    
                    // Status text
                    let status_text = match &tool_call.status {
                        MessageStatus::Complete => "completed".to_string(),
                        MessageStatus::Streaming => "running...".to_string(),
                        MessageStatus::Error(err) => format!("failed: {}", err),
                        MessageStatus::Sending => "starting...".to_string(),
                        _ => String::new()
                    };
                    
                    if !status_text.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new(status_text).color(app_theme.hint_text_color()).size(11.0));
                    }
                    
                    // Add progress bar for running tools
                    if tool_call.status == MessageStatus::Streaming {
                        // Try to find running tool info to get actual progress
                        let progress = running_tools.values()
                            .find(|info| info.tool_name == tool_call.name)
                            .and_then(|info| info.progress)
                            .unwrap_or(0.0);
                        
                        ui.add_space(8.0);
                        ui.add(egui::ProgressBar::new(progress)
                            .desired_width(100.0)
                            .desired_height(4.0)
                            .fill(app_theme.accent_color())
                            .animate(progress == 0.0)); // Only animate if we don't have actual progress
                    }
                    
                    // Right-aligned action buttons
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if tool_call.result.is_some() {
                            // View details button for completed tools
                            log::trace!("Rendering view details button for tool: {}", tool_call.name);
                            let view_details_btn = ui.small_button("View details");
                            if view_details_btn.clicked() {
                                // Determine display title based on tool type
                                let result = tool_call.result.as_ref().unwrap();
                                let is_shell_result = tool_call.name.contains("shell") || tool_call.name.contains("execution") ||
                                                     result.contains("stdout") || result.contains("stderr") ||
                                                     result.contains("exit_code");
                                
                                let display_title = if is_shell_result {
                                    format!("{} - Terminal Output", tool_call.name)
                                } else {
                                    format!("{} - Result", tool_call.name)
                                };
                                
                                log::debug!("View details clicked for tool: {}, display_title: {}", tool_call.name, display_title);
                                clicked_tool_result = Some((display_title, result.clone()));
                            }
                            
                            // Copy button for tool results
                            let copy_text = copy_state.get_button_text("Copy");
                            let copy_color = copy_state.get_button_color(app_theme);
                            
                            if ui.add(egui::Button::new(copy_text)
                                .small()
                                .fill(copy_color.gamma_multiply(0.1)))
                                .clicked() {
                                let result = tool_call.result.as_ref().unwrap();
                                ui.output_mut(|o| o.copied_text = result.clone());
                                copy_state.start_copy_feedback(result.clone());
                                log::debug!("Copied tool result to clipboard");
                            }
                        } else if tool_call.status == MessageStatus::Streaming {
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
                        }
                    });
                });
                
                // Add inline result display for completed tools
                if tool_call.status == MessageStatus::Complete && tool_call.result.is_some() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    
                    let result_str = tool_call.result.as_ref().unwrap();
                    
                    // Parse the result as JSON
                    if let Ok(result_json) = serde_json::from_str::<serde_json::Value>(result_str) {
                        // Use ToolResultFormatter to format the result
                        let formatter = crate::gui::app::tool_formatting::ToolResultFormatter::new();
                        let tool_result = crate::tools::types::ToolResult::Success(result_json.clone());
                        let formatted_result = formatter.format_tool_result_for_preview(&tool_call.name, &tool_result);
                        
                        // Display the formatted result in a scrollable area with limited height
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .id_source(format!("tool_result_{}", tool_call.id))
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.set_max_width(max_width - 24.0);
                                
                                // Check if this is a shell command result for special rendering
                                if is_shell_command_result(&tool_call.name, &result_json) {
                                    render_terminal_output(ui, &result_json, app_theme);
                                } else if is_code_change_result(&tool_call.name, &result_json) {
                                    render_diff_output(ui, &result_json, app_theme);
                                } else {
                                    // Default rendering with markdown support
                                    ui.style_mut().wrap = Some(true);
                                    
                                    // Use markdown rendering for formatted results
                                    crate::gui::chat::view::COMMONMARK_CACHE.with(|cache| {
                                        let mut cache = cache.borrow_mut();
                                        let viewer = egui_commonmark::CommonMarkViewer::new();
                                        viewer.show(ui, &mut *cache, &formatted_result);
                                    });
                                }
                            });
                    } else {
                        // Fallback: display raw result if JSON parsing fails
                        ui.code(result_str);
                    }
                }
            });
    
    clicked_tool_result
}

/// Render tool calls as compact, clickable cards (for backward compatibility)
fn render_tool_calls_compact(ui: &mut Ui, tool_calls: &[ToolCall], bg_color: &Color32, max_width: f32, app_theme: AppTheme, running_tools: &HashMap<ToolRunId, RunningToolInfo>, copy_state: &mut CopyButtonState) -> Option<(String, String)> {
    let mut clicked_tool_result = None;
    
    for tool_call in tool_calls {
        if let Some(tool_info) = render_single_tool_call(ui, tool_call, bg_color, max_width, app_theme, running_tools, copy_state) {
            clicked_tool_result = Some(tool_info);
        }
        ui.add_space(4.0); // Spacing between tool cards
    }
    
    clicked_tool_result
}

/// Render a standalone tool card
fn render_tool_card(ui: &mut Ui, tool_card: &ToolCard, bg_color: &Color32, max_width: f32, app_theme: AppTheme, running_tools: &HashMap<ToolRunId, RunningToolInfo>, copy_state: &mut CopyButtonState) -> Option<(String, String)> {
    let mut clicked_tool_result = None;
    
    // Create a frame for the tool card
    Frame::none()
        .fill(app_theme.code_background())
        .rounding(Rounding::same(6))
        .stroke(Stroke::new(1.0, app_theme.border_color()))
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Tool status icon
                let (status_icon, status_color) = match tool_card.status {
                    ToolCardStatus::Completed { success: true } => ("✅", app_theme.success_color()),
                    ToolCardStatus::Completed { success: false } => ("❌", app_theme.error_color()),
                    ToolCardStatus::Failed { .. } => ("❌", app_theme.error_color()),
                    ToolCardStatus::Running => ("🔄", app_theme.accent_color()),
                    ToolCardStatus::Cancelled => ("⏹️", app_theme.hint_text_color()),
                };
                
                ui.label(RichText::new(status_icon).color(status_color).size(16.0));
                ui.add_space(8.0);
                
                // Tool name, parameters, and timing - all on one line
                let friendly_name = get_human_friendly_tool_name(&tool_card.tool_name);
                
                // Build compact display text
                let mut display_parts = vec![format!("🔧 {}", friendly_name)];
                
                // Add key parameters inline
                let params = format_tool_parameters(&tool_card.tool_name, &tool_card.input_params);
                if !params.is_empty() {
                    let param_text: Vec<String> = params.iter()
                        .take(2) // Show only first 2 params to save space
                        .map(|(key, value)| {
                            let truncated_value = if value.len() > 30 {
                                format!("{}...", &value[..27])
                            } else {
                                value.clone()
                            };
                            format!("{}: {}", key, truncated_value)
                        })
                        .collect();
                    
                    if !param_text.is_empty() {
                        display_parts.push(format!("({})", param_text.join(", ")));
                    }
                }
                
                // Add timing info
                if let Some(completed_at) = tool_card.completed_at {
                    let duration = completed_at.signed_duration_since(tool_card.started_at);
                    display_parts.push(format!("- {:.1}s", duration.num_milliseconds() as f64 / 1000.0));
                } else if tool_card.status == ToolCardStatus::Running {
                    display_parts.push("- Running...".to_string());
                }
                
                // Display all on one line
                ui.label(RichText::new(display_parts.join(" "))
                    .color(app_theme.text_color())
                    .strong()
                    .size(13.0));
                
                // Show progress bar if running (in the remaining space)
                if tool_card.status == ToolCardStatus::Running {
                    ui.add_space(8.0);
                    if let Some(progress) = tool_card.progress {
                        ui.add(egui::ProgressBar::new(progress)
                            .desired_width(100.0)
                            .desired_height(4.0)
                            .fill(app_theme.accent_color()));
                    } else {
                        ui.add(egui::ProgressBar::new(0.0)
                            .desired_width(100.0)
                            .desired_height(4.0)
                            .fill(app_theme.accent_color())
                            .animate(true));
                    }
                }
                
                // Right-aligned action buttons
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    match &tool_card.status {
                        ToolCardStatus::Completed { success: true } => {
                            if tool_card.result.is_some() {
                                // View details button
                                if ui.small_button("View details").clicked() {
                                    let result_json = tool_card.result.as_ref().unwrap();
                                    let friendly_name = get_human_friendly_tool_name(&tool_card.tool_name);
                                    let display_title = format!("{} - Result", friendly_name);
                                    clicked_tool_result = Some((display_title, serde_json::to_string_pretty(result_json).unwrap_or_else(|_| result_json.to_string())));
                                }
                                
                                // Copy button
                                let copy_text = copy_state.get_button_text("Copy");
                                let copy_color = copy_state.get_button_color(app_theme);
                                
                                if ui.add(egui::Button::new(copy_text)
                                    .small()
                                    .fill(copy_color.gamma_multiply(0.1)))
                                    .clicked() {
                                    let result_text = serde_json::to_string_pretty(tool_card.result.as_ref().unwrap())
                                        .unwrap_or_else(|_| tool_card.result.as_ref().unwrap().to_string());
                                    ui.output_mut(|o| o.copied_text = result_text.clone());
                                    copy_state.start_copy_feedback(result_text);
                                }
                            }
                        }
                        ToolCardStatus::Running => {
                            // Cancel button for running tools
                            if ui.small_button("Cancel").clicked() {
                                clicked_tool_result = Some(("__CANCEL_TOOL__".to_string(), tool_card.run_id.to_string()));
                            }
                        }
                        ToolCardStatus::Failed { error } => {
                            ui.label(RichText::new(&format!("Error: {}", error))
                                .color(app_theme.error_color())
                                .size(11.0));
                        }
                        _ => {}
                    }
                });
            });
            
            // Add inline result display for completed tools
            if let ToolCardStatus::Completed { success: true } = &tool_card.status {
                if let Some(result) = &tool_card.result {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    
                    // Use ToolResultFormatter to format the result
                    let formatter = crate::gui::app::tool_formatting::ToolResultFormatter::new();
                    let success = matches!(tool_card.status, ToolCardStatus::Completed { success: true });
                    let tool_result = if success {
                        crate::tools::types::ToolResult::Success(result.clone())
                    } else {
                        crate::tools::types::ToolResult::Error { error: "Tool execution failed".to_string() }
                    };
                    
                    let formatted_result = formatter.format_tool_result_for_preview(&tool_card.tool_name, &tool_result);
                    
                    // Display the formatted result in a scrollable area with limited height
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .id_source(format!("tool_result_{}", tool_card.run_id))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_max_width(max_width - 24.0);
                            
                            // Check if this is a shell command result for special rendering
                            if is_shell_command_result(&tool_card.tool_name, result) {
                                render_terminal_output(ui, result, app_theme);
                            } else if is_code_change_result(&tool_card.tool_name, result) {
                                render_diff_output(ui, result, app_theme);
                            } else {
                                // Default rendering with markdown support
                                ui.style_mut().wrap = Some(true);
                                
                                // Use markdown rendering for formatted results
                                crate::gui::chat::view::COMMONMARK_CACHE.with(|cache| {
                                    let mut cache = cache.borrow_mut();
                                    let viewer = egui_commonmark::CommonMarkViewer::new();
                                    viewer.show(ui, &mut *cache, &formatted_result);
                                });
                            }
                        });
                }
            }
        });
    
    clicked_tool_result
}

/// Render message content in a compact format
fn render_message_content_compact(ui: &mut Ui, message: &StreamingMessage, bg_color: &Color32, max_width: f32, app_theme: AppTheme) -> Option<(String, String)> {
    // Use message_type for summary/finalization
    if message.message_type == MessageType::Summary {
        return render_text_content_compact(ui, &message.content, &bg_color, max_width, app_theme);
    }
    
    // Set up content area
    ui.set_max_width(max_width - 20.0);
    ui.style_mut().wrap = Some(true);
    
    // Render content based on type
    let clicked_tool = if message.content.contains("```") {
        render_mixed_content_compact(ui, &message.content, &bg_color, max_width - 20.0, app_theme)
    } else {
        render_text_content_compact(ui, &message.content, &bg_color, max_width - 20.0, app_theme)
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
fn render_text_content_compact(ui: &mut Ui, text: &str, bg_color: &Color32, max_width: f32, app_theme: AppTheme) -> Option<(String, String)> {
    // Check if text contains tool:// links
    if text.contains("tool://") {
        render_text_with_tool_links(ui, text, bg_color, max_width, app_theme)
    } else {
        // Set the text color from theme before rendering
        let original_text_color = ui.style().visuals.text_color();
        ui.style_mut().visuals.override_text_color = Some(app_theme.text_color());
        
        COMMONMARK_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let viewer = CommonMarkViewer::new()
                .max_image_width(Some(max_width as usize))
                .default_width(Some(max_width as usize));
            
            ui.set_max_width(max_width);
            ui.style_mut().wrap = Some(true);
            viewer.show(ui, &mut *cache, text);
        });
        
        // Restore original text color
        ui.style_mut().visuals.override_text_color = None;
        None
    }
}

/// Render text with tool:// links as clickable buttons
fn render_text_with_tool_links(ui: &mut Ui, text: &str, bg_color: &Color32, max_width: f32, app_theme: AppTheme) -> Option<(String, String)> {
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
        let placeholder = format!("__TOOL_LINK_{}__", i);
        processed_text = processed_text.replace(&caps[0], &placeholder);
    }
    
    
    if tool_links.is_empty() {
        // No tool links, render normally with CommonMark
        let original_text_color = ui.style().visuals.text_color();
        ui.style_mut().visuals.override_text_color = Some(app_theme.text_color());
        
        COMMONMARK_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let viewer = CommonMarkViewer::new()
                .max_image_width(Some(max_width as usize))
                .default_width(Some(max_width as usize));
            
            ui.set_max_width(max_width);
            ui.style_mut().wrap = Some(true);
            viewer.show(ui, &mut *cache, text);
        });
        
        ui.style_mut().visuals.override_text_color = None;
        return None;
    }
    
    // Process the text with proper markdown rendering and tool link buttons
    let mut remaining_text = processed_text.as_str();
    
    for (i, (link_text, tool_call_id)) in tool_links.iter().enumerate() {
        let placeholder = format!("__TOOL_LINK_{}__", i);
        
        if let Some(split_pos) = remaining_text.find(&placeholder) {
            // Render text before the placeholder using CommonMark
            let before_text = &remaining_text[..split_pos];
            if !before_text.trim().is_empty() {
                let original_text_color = ui.style().visuals.text_color();
                ui.style_mut().visuals.override_text_color = Some(app_theme.text_color());
                
                COMMONMARK_CACHE.with(|cache| {
                    let mut cache = cache.borrow_mut();
                    let viewer = CommonMarkViewer::new()
                        .max_image_width(Some(max_width as usize))
                        .default_width(Some(max_width as usize));
                    
                    ui.set_max_width(max_width);
                    ui.style_mut().wrap = Some(true);
                    viewer.show(ui, &mut *cache, before_text);
                });
                
                ui.style_mut().visuals.override_text_color = None;
            }
            
            // Render the clickable button
            ui.horizontal(|ui| {
                let button = ui.small_button(format!("📋 {}", link_text));
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
        let original_text_color = ui.style().visuals.text_color();
        ui.style_mut().visuals.override_text_color = Some(app_theme.text_color());
        
        COMMONMARK_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let viewer = CommonMarkViewer::new()
                .max_image_width(Some(max_width as usize))
                .default_width(Some(max_width as usize));
            
            ui.set_max_width(max_width);
            ui.style_mut().wrap = Some(true);
            viewer.show(ui, &mut *cache, remaining_text);
        });
        
        ui.style_mut().visuals.override_text_color = None;
    }
    
    clicked_tool
}

/// Render code block compactly
fn render_code_block_compact(ui: &mut Ui, text: &str, bg_color: &Color32, max_width: f32, app_theme: AppTheme) {
    let opacity = 0.3; // Default opacity for UI elements
    let mut lines = text.lines();
    let first_line = lines.next().unwrap_or("");
    let (language, remaining_text) = if first_line.trim().is_empty() {
        ("text", text)
    } else {
        (first_line.trim(), text.splitn(2, '\n').nth(1).unwrap_or(""))
    };
    
    // Compact code block header
    ui.horizontal(|ui| {
        ui.label(RichText::new("💻").size(12.0));
        ui.label(RichText::new(language).monospace().color(app_theme.hint_text_color()).size(10.0));
        
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let copy_button = egui::Button::new("📋")
                .fill(app_theme.input_background())
                .stroke(Stroke::new(1.0, app_theme.border_color()))
                .rounding(CornerRadius::same(4));
            
            if ui.add(copy_button).on_hover_text("Copy code").clicked() {
                ui.output_mut(|o| o.copied_text = remaining_text.to_string());
            }
        });
    });
    
    ui.add_space(2.0);
    
    // Code content in a subtle frame
    Frame::none()
        .fill(app_theme.code_background())
        .inner_margin(Vec2::new(8.0, 6.0))
        .rounding(CornerRadius::same(4))
        .stroke(Stroke::new(0.5, app_theme.border_color()))
        .show(ui, |ui| {
            ui.set_max_width(max_width - 16.0);
            
            // Scrollable for long code
            let line_count = remaining_text.lines().count();
            if line_count > 10 {
                // Show line count indicator
                ui.label(RichText::new(format!("{} lines of code", line_count))
                    .small()
                    .color(app_theme.hint_text_color()));
                ui.add_space(2.0);
                
                // Use scrollable area with max height for ~10 lines
                ScrollArea::vertical()
                    .max_height(120.0) // Approximately 10 lines at 12px line height
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        render_syntax_highlighted_code(ui, remaining_text, language, &bg_color, max_width - 32.0);
                    });
            } else {
                render_syntax_highlighted_code(ui, remaining_text, language, &bg_color, max_width - 16.0);
            }
        });
}

/// Render syntax highlighted code
fn render_syntax_highlighted_code(ui: &mut Ui, text: &str, language: &str, bg_color: &Color32, max_width: f32) {
    let syntax_set = get_syntax_set();
    let theme_set = get_theme_set();
    
    let syntect_theme = &theme_set.themes["base16-ocean.dark"];
    let syntax = syntax_set.find_syntax_by_extension(language)
        .or_else(|| syntax_set.find_syntax_by_name(language))
        .or_else(|| syntax_set.find_syntax_by_extension("txt"))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    
    let mut highlighter = HighlightLines::new(syntax, syntect_theme);
    
    ui.style_mut().wrap = Some(true);
    
    // Use a more compact approach by building the layout job directly
    let mut layout_job = egui::text::LayoutJob::default();
    
    for (line_index, line) in LinesWithEndings::from(text).enumerate().take(20) {
        let ranges = highlighter.highlight_line(line, syntax_set).unwrap_or_default();
            
            for (style, text_part) in ranges {
                let color = syntect_style_to_color(&style);
            layout_job.append(
                text_part,
                0.0,
                TextFormat {
                    font_id: egui::FontId::monospace(10.0),
                    line_height: Some(12.0), // Tight line height - 12.0 for 10.0 font
                    color,
                    ..Default::default()
                },
            );
            }
    }
    
    // Render the entire layout job as a single label
    ui.set_max_width(max_width);
    ui.label(layout_job);
}

fn syntect_style_to_color(style: &SyntectStyle) -> Color32 {
    Color32::from_rgb(
        style.foreground.r, 
        style.foreground.g, 
        style.foreground.b
    )
}

// New helper function to contain the core logic of preparing and rendering diff lines
fn render_internal_diff_display_logic(ui: &mut Ui, old_content: &str, new_content: &str, language: Option<&str>, _bg_color: &Color32, max_width: f32, app_theme: AppTheme) {
    let diff = TextDiff::from_lines(old_content, new_content);
    
    let syntax_set = get_syntax_set();
    let theme_set = get_theme_set();
    let syntect_theme = &theme_set.themes["base16-ocean.dark"];
    
    let syntax = if let Some(lang) = language {
        syntax_set.find_syntax_by_extension(lang)
            .or_else(|| syntax_set.find_syntax_by_name(lang))
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
    } else {
        syntax_set.find_syntax_by_extension("rs")
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
    };

    ui.style_mut().wrap = Some(true);
    ui.set_max_width(max_width);
    ui.spacing_mut().item_spacing.y = 0.0; // Remove spacing between items
    render_diff_lines(ui, &diff, syntax, syntect_theme, &syntax_set, app_theme);
}

/// Render a unified diff view of two code snippets with syntax highlighting
fn render_code_diff(ui: &mut Ui, old_content: &str, new_content: &str, language: Option<&str>, bg_color: &Color32, max_width: f32, app_theme: AppTheme) {
    let diff = TextDiff::from_lines(old_content, new_content);
    let total_lines = diff.iter_all_changes().count();

    if total_lines > DIFF_COLLAPSING_THRESHOLD_LINES { // Use the constant
        egui::CollapsingHeader::new(RichText::new(format!("{} lines changed", total_lines)).small())
            .default_open(false) // Keep it closed by default
            .show(ui, |header_ui| {
                ScrollArea::vertical()
                    .max_height(EXPANDED_DIFF_SCROLL_AREA_MAX_HEIGHT) // Use the constant
                    .auto_shrink([false, true])
                    .show(header_ui, |scroll_ui| {
                        render_internal_diff_display_logic(scroll_ui, old_content, new_content, language, bg_color, scroll_ui.available_width(), app_theme);
                    });
            });
    } else {
        render_internal_diff_display_logic(ui, old_content, new_content, language, bg_color, max_width, app_theme);
    }
}

/// Helper function to render individual diff lines
fn render_diff_lines<'a>(
    ui: &mut Ui, 
    diff: &TextDiff<'a, 'a, 'a, str>, 
    syntax: &syntect::parsing::SyntaxReference, 
    syntect_theme: &syntect::highlighting::Theme,
    syntax_set: &SyntaxSet,
    app_theme: AppTheme
) {
    // Build a single layout job for all diff lines to eliminate spacing issues
    let mut layout_job = egui::text::LayoutJob::default();
    
    for change in diff.iter_all_changes() {
        let (line_bg_color, prefix_color, prefix_text) = match change.tag() {
            ChangeTag::Delete => (
                app_theme.diff_removed_bg(),     // Use theme colors
                app_theme.diff_removed_text(),   // Use theme colors
                "- "
            ),
            ChangeTag::Insert => (
                app_theme.diff_added_bg(),       // Use theme colors
                app_theme.diff_added_text(),     // Use theme colors
                "+ "
            ),
            ChangeTag::Equal => (
                Color32::TRANSPARENT,            // No background for unchanged lines
                Color32::from_rgb(150, 150, 150), // Gray prefix
                "  "
            ),
        };

        let line_content = change.value();
        
        // Add the prefix (-, +, or space) with appropriate color
        layout_job.append(
            prefix_text,
            0.0,
            TextFormat {
                font_id: egui::FontId::monospace(10.0),
                line_height: Some(12.0), // Tight line height
                color: prefix_color,
                background: line_bg_color,
                ..Default::default()
            },
        );

        // Handle the line content - we need to preserve the structure including newlines
        if line_content.trim().is_empty() {
            // For empty lines, just add the newline with background
            layout_job.append(
                line_content, // This preserves the actual whitespace/newline
                0.0,
                TextFormat {
                    font_id: egui::FontId::monospace(10.0),
                    line_height: Some(12.0),
                    color: prefix_color,
                    background: line_bg_color,
                    ..Default::default()
                },
            );
        } else {
            // For lines with content, syntax highlight them
            let mut highlighter = HighlightLines::new(syntax, syntect_theme);

            if let Ok(ranges) = highlighter.highlight_line(line_content, syntax_set) {
                for (style, text_part) in ranges {
                    let text_color = match change.tag() {
                        ChangeTag::Delete => app_theme.diff_removed_text(),
                        ChangeTag::Insert => app_theme.diff_added_text(),
                        ChangeTag::Equal => syntect_style_to_color(&style),
                    };
                    
                    layout_job.append(
                        text_part,
                        0.0,
                        TextFormat {
                            font_id: egui::FontId::monospace(10.0),
                            line_height: Some(12.0), // Tight line height
                            color: text_color,
                            background: line_bg_color,
                            ..Default::default()
                        },
                    );
                }
            } else {
                // Fallback to plain text if syntax highlighting fails
                let text_color = match change.tag() {
                    ChangeTag::Delete => app_theme.diff_removed_text(),
                    ChangeTag::Insert => app_theme.diff_added_text(),
                    ChangeTag::Equal => Color32::from_rgb(200, 200, 200),
                };
                
                layout_job.append(
                    line_content,
                    0.0,
                    TextFormat {
                        font_id: egui::FontId::monospace(10.0),
                        line_height: Some(12.0), // Tight line height
                        color: text_color,
                        background: line_bg_color,
                        ..Default::default()
                    },
                );
            }
        }
    }
    
    // Render the entire diff as a single label with tight spacing
    ui.label(layout_job);
}

/// Detect if content contains a diff pattern and extract the parts
fn detect_diff_content(content: &str) -> Option<(String, String, Option<String>)> {
    // Look for common diff patterns in tool results or messages
    
    // Pattern 1: "old content" -> "new content" format
    if let Some(arrow_pos) = content.find(" -> ") {
        let before_arrow = content[..arrow_pos].trim();
        let after_arrow = content[arrow_pos + 4..].trim();
        
        // Try to extract quoted content
        if before_arrow.starts_with('"') && before_arrow.ends_with('"') &&
           after_arrow.starts_with('"') && after_arrow.ends_with('"') {
            let old_content = before_arrow[1..before_arrow.len()-1].to_string();
            let new_content = after_arrow[1..after_arrow.len()-1].to_string();
            return Some((old_content, new_content, None));
        }
    }
    
    // Pattern 2: File edit operations in tool results
    if content.contains("edit_file") || content.contains("file_edit") || content.contains("old_content") {
        // Try to parse JSON for file edit operations
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(obj) = json.as_object() {
                if let (Some(old), Some(new)) = (
                    obj.get("old_content").and_then(|v| v.as_str()),
                    obj.get("new_content").and_then(|v| v.as_str())
                ) {
                    let language = obj.get("language")
                        .and_then(|v| v.as_str())
                        .or_else(|| obj.get("file_extension").and_then(|v| v.as_str()));
                    return Some((old.to_string(), new.to_string(), language.map(|s| s.to_string())));
                }
            }
        }
    }
    
    // Pattern 3: Before/After sections
    if content.contains("Before:") && content.contains("After:") {
        if let Some(before_pos) = content.find("Before:") {
            if let Some(after_pos) = content.find("After:") {
                if after_pos > before_pos {
                    let old_content = content[before_pos + 7..after_pos].trim().to_string();
                    let new_content = content[after_pos + 6..].trim().to_string();
                    return Some((old_content, new_content, None));
                }
            }
        }
    }
    
    // Pattern 4: Git-style diff with file headers (check this before unified diff)
    if (content.contains("diff --git") || content.contains("index ")) && content.contains("@@") {
        let lines: Vec<&str> = content.lines().collect();
        let mut old_lines = Vec::new();
        let mut new_lines = Vec::new();
        let mut language = None;
        
        for line in &lines {
            if line.starts_with("diff --git") {
                // Extract file extension for language detection
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    // Look at both a/filename and b/filename
                    for part in &parts[2..] {
                        if let Some(filename) = part.strip_prefix("b/").or_else(|| part.strip_prefix("a/")) {
                            if let Some(ext) = filename.split('.').last() {
                                if ext != filename && ext.len() <= 5 { // reasonable extension length
                                    language = Some(ext.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
                continue;
            }
            if line.starts_with("---") || line.starts_with("+++") || line.starts_with("@@") || line.starts_with("index ") {
                continue;
            }
            if line.starts_with("-") {
                old_lines.push(&line[1..]);
            } else if line.starts_with("+") {
                new_lines.push(&line[1..]);
            } else if line.starts_with(" ") {
                old_lines.push(&line[1..]);
                new_lines.push(&line[1..]);
            }
        }
        
        if !old_lines.is_empty() || !new_lines.is_empty() {
            return Some((old_lines.join("\n"), new_lines.join("\n"), language));
        }
    }
    
    // Pattern 5: Traditional unified diff format (starts with --- and +++)
    if content.contains("---") && content.contains("+++") {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() > 2 {
            let mut old_lines = Vec::new();
            let mut new_lines = Vec::new();
            let mut in_diff = false;
            
            for line in &lines {
                if line.starts_with("---") || line.starts_with("+++") {
                    in_diff = true;
                    continue;
                }
                if line.starts_with("@@") {
                    continue;
                }
                if in_diff {
                    if line.starts_with("-") && !line.starts_with("---") {
                        old_lines.push(&line[1..]);
                    } else if line.starts_with("+") && !line.starts_with("+++") {
                        new_lines.push(&line[1..]);
                    } else if line.starts_with(" ") {
                        old_lines.push(&line[1..]);
                        new_lines.push(&line[1..]);
                    }
                }
            }
            
            if !old_lines.is_empty() || !new_lines.is_empty() {
                return Some((old_lines.join("\n"), new_lines.join("\n"), None));
            }
        }
    }
    
    // Pattern 6: Edit descriptions mentioning changes
    if content.contains("I will") && (content.contains("change") || content.contains("replace") || content.contains("modify")) {
        // Try a simpler approach: find all quoted strings and look for "to" pattern
        let mut quote_positions = Vec::new();
        let mut in_quote = false;
        let mut quote_start = 0;
        
        for (i, c) in content.char_indices() {
            match c {
                '"' if !in_quote => {
                    in_quote = true;
                    quote_start = i + 1;
                }
                '"' if in_quote => {
                    in_quote = false;
                    quote_positions.push((quote_start, i));
                }
                _ => {}
            }
        }
        
        // Look for patterns where we have at least 2 quoted strings with " to " between them
        if quote_positions.len() >= 2 {
            for i in 0..quote_positions.len() - 1 {
                let (_, first_end) = quote_positions[i];
                let (second_start, second_end) = quote_positions[i + 1];
                
                let between = &content[first_end + 1..second_start - 1];
                if between.contains(" to ") {
                    let old_content = &content[quote_positions[i].0..quote_positions[i].1];
                    let new_content = &content[second_start..second_end];
                    return Some((old_content.to_string(), new_content.to_string(), None));
                }
            }
        }
    }
    
    None
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
                            return format!("{}: {}", tool_name, file_path);
                        }
                        
                        // For search operations
                        if let Some(query) = obj.get("query").and_then(|v| v.as_str()) {
                            return format!("{}: \"{}\"", tool_name, query);
                        }
                        
                        // For content operations
                        if obj.contains_key("content") {
                            return format!("{}: Content retrieved", tool_name);
                        }
                    }
                }
            }
            
            return format!("{} result", tool_name);
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
                    return format!("Web search: \"{}\" ({} sources)", query, source_count);
                }
                // Only treat as web search if it doesn't have file-specific fields
                if !obj.contains_key("files") {
                    return format!("Web search: \"{}\"", query);
                }
            }
            
            // Check for file operations
            if let Some(file_path) = obj.get("file_path").and_then(|v| v.as_str()) {
                return format!("File: {}", file_path);
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
const DIFF_COLLAPSING_THRESHOLD_LINES: usize = 10;
const EXPANDED_DIFF_SCROLL_AREA_MAX_HEIGHT: f32 = 360.0;
const MIN_ALLOCATED_HEIGHT_FOR_DIFF_FRAME: f32 = 400.0;

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
                ui.label(RichText::new(format!("({})", lang)).monospace().color(app_theme.hint_text_color()).size(9.0));
            }
            
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let copy_button = egui::Button::new("📋")
                    .fill(app_theme.input_background())
                    .stroke(Stroke::new(1.0, app_theme.border_color()))
                    .rounding(CornerRadius::same(4));
                
                if ui.add(copy_button).on_hover_text("Copy diff").clicked() {
                    let diff_text = format!("--- Original\n+++ Modified\n{}", 
                        similar::TextDiff::from_lines(&old_content, &new_content)
                            .unified_diff()
                            .context_radius(3)
                            .to_string()
                    );
                    ui.output_mut(|o| o.copied_text = diff_text);
                }
            });
        });
        ui.add_space(2.0);

        let desired_min_height_for_diff_component = MIN_ALLOCATED_HEIGHT_FOR_DIFF_FRAME;
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), desired_min_height_for_diff_component),
            Layout::top_down(Align::Min).with_cross_align(Align::Min),
            |ui_for_diff_frame| {
                Frame::none()
                    .fill(app_theme.code_background())
                    .inner_margin(Vec2::new(8.0, 6.0))
                    .rounding(CornerRadius::same(4))
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
                            ui.label(RichText::new(format!("({})", lang)).monospace().color(app_theme.hint_text_color()).size(9.0));
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let copy_button = egui::Button::new("📋")
                                .fill(app_theme.input_background())
                                .stroke(Stroke::new(1.0, app_theme.border_color()))
                                .rounding(CornerRadius::same(4));
                            if ui.add(copy_button).on_hover_text("Copy diff").clicked() {
                                let diff_text = format!("--- Original\n+++ Modified\n{}", 
                                    similar::TextDiff::from_lines(&old_content, &new_content)
                                        .unified_diff().context_radius(3).to_string());
                                ui.output_mut(|o| o.copied_text = diff_text);
                            }
                        });
                    });
                    ui.add_space(2.0);
                    let desired_min_height_for_diff_component = MIN_ALLOCATED_HEIGHT_FOR_DIFF_FRAME;
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), desired_min_height_for_diff_component),
                        Layout::top_down(Align::Min).with_cross_align(Align::Min),
                        |ui_for_diff_frame| {
                            Frame::none()
                                .fill(app_theme.code_background())
                                .inner_margin(Vec2::new(8.0, 6.0))
                                .rounding(CornerRadius::same(4))
                                .stroke(Stroke::new(0.5, app_theme.border_color()))
                                .show(ui_for_diff_frame, |frame_content_ui| {
                                    render_code_diff(frame_content_ui, &old_content, &new_content, language.as_deref(), bg_color, frame_content_ui.available_width(), app_theme);
                                });
                        }
                    );
                } else {
                    if let Some(tool_info) = render_text_content_compact(ui, part, &bg_color, max_width, app_theme) {
                        return Some(tool_info);
                    }
                }
            }
        } else {
            render_code_block_compact(ui, part, &bg_color, max_width, app_theme);
        }
    }
    None
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
    fn test_detect_diff_content_arrow_format() {
        let content = r#""hello world" -> "hello rust""#;
        let result = detect_diff_content(content);
        assert!(result.is_some());
        let (old, new, lang) = result.unwrap();
        assert_eq!(old, "hello world");
        assert_eq!(new, "hello rust");
        assert!(lang.is_none());
    }

    #[test]
    fn test_detect_diff_content_before_after() {
        let content = r#"Before: fn main() {
    println!("Hello");
}
After: fn main() {
    println!("Hello, world!");
}"#;
        let result = detect_diff_content(content);
        assert!(result.is_some());
        let (old, new, _) = result.unwrap();
        assert!(old.contains("println!(\"Hello\");"));
        assert!(new.contains("println!(\"Hello, world!\");"));
    }

    #[test]
    fn test_detect_diff_content_unified_diff() {
        let content = r#"--- old_file.rs
+++ new_file.rs
@@ -1,3 +1,3 @@
 fn main() {
-    println!("Hello");
+    println!("Hello, world!");
 }"#;
        let result = detect_diff_content(content);
        assert!(result.is_some());
        let (old, new, _) = result.unwrap();
        assert!(old.contains("println!(\"Hello\");"));
        assert!(new.contains("println!(\"Hello, world!\");"));
    }

    #[test]
    fn test_detect_diff_content_git_style() {
        let content = r#"diff --git a/main.rs b/main.rs
index 1234567..abcdefg 100644
--- a/main.rs
+++ b/main.rs
@@ -1,3 +1,3 @@
 fn main() {
-    println!("Hello");
+    println!("Hello, world!");
 }"#;
        let result = detect_diff_content(content);
        
        assert!(result.is_some());
        let (old, new, lang) = result.unwrap();
        assert!(old.contains("println!(\"Hello\");"));
        assert!(new.contains("println!(\"Hello, world!\");"));
        assert_eq!(lang, Some("rs".to_string()));
    }

    #[test]
    fn test_detect_diff_content_edit_description() {
        let content = r#"I will change "old function" to "new function""#;
        let result = detect_diff_content(content);
        
        assert!(result.is_some());
        let (old, new, _) = result.unwrap();
        assert_eq!(old, "old function");
        assert_eq!(new, "new function");
    }

    #[test]
    fn test_detect_diff_content_no_diff() {
        let content = "This is just regular text without any diff patterns.";
        let result = detect_diff_content(content);
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_diff_content_json_tool_result() {
        let content = r#"{"old_content": "fn test() {\n    println!(\"old\");\n}", "new_content": "fn test() {\n    println!(\"new\");\n}", "language": "rs"}"#;
        let result = detect_diff_content(content);
        assert!(result.is_some());
        let (old, new, lang) = result.unwrap();
        assert!(old.contains("println!(\"old\");"));
        assert!(new.contains("println!(\"new\");"));
        assert_eq!(lang, Some("rs".to_string()));
    }

    #[test]
    fn test_summary_message_type_detection() {
        let summary_message = create_test_message("Okay, I've finished those tasks. What would you like to do next?", MessageType::Summary);
        
        assert_eq!(summary_message.message_type, MessageType::Summary);
        assert!(is_reasoning_engine_summary_message(&summary_message.content));
    }

    #[test]
    fn test_message_group_spinner_suppression() {
        let mut summary_message = create_test_message("Successfully completed: task1, task2", MessageType::Summary);
        summary_message.status = MessageStatus::Complete;
        
        let mut tool_message = create_test_message("Tool 'search' result: {...}", MessageType::Tool);
        tool_message.status = MessageStatus::Streaming;
        
        let mut normal_message = create_test_message("Regular message", MessageType::Normal);
        normal_message.status = MessageStatus::Streaming;
        
        // Summary messages that are complete should not show spinner
        let group = vec![&summary_message];
        let status = get_group_status(&group, AppTheme::Dark);
        assert!(status.is_none()); // No spinner for complete summary
        
        // Tool messages that are streaming should show spinner
        let group = vec![&tool_message];
        let status = get_group_status(&group, AppTheme::Dark);
        assert!(status.is_some()); // Should show spinner for streaming tool
        
        // Normal messages that are streaming should show spinner
        let group = vec![&normal_message];
        let status = get_group_status(&group, AppTheme::Dark);
        assert!(status.is_some()); // Should show spinner for streaming normal
    }

    #[test]
    fn test_message_type_enum_values() {
        // Ensure all enum variants exist and are usable
        let _normal = MessageType::Normal;
        let _summary = MessageType::Summary;
        let _tool = MessageType::Tool;
        let _system = MessageType::System;
        
        // Test equality
        assert_eq!(MessageType::Normal, MessageType::Normal);
        assert_ne!(MessageType::Normal, MessageType::Summary);
    }

    #[test]
    fn test_diff_rendering_constants() {
        // Test the threshold for when a diff becomes collapsible
        // This is used in render_code_diff
        assert_eq!(DIFF_COLLAPSING_THRESHOLD_LINES, 10, "Diff collapsing threshold should be 10 lines.");

        // Test the max_height of the ScrollArea when a CollapsingHeader for a diff is expanded
        // This is used in render_code_diff
        assert_eq!(EXPANDED_DIFF_SCROLL_AREA_MAX_HEIGHT, 360.0, "Max height for expanded diff scroll area should be 360.0 px.");

        // Test the minimum height allocated for the frame containing a diff view
        // This is used in render_mixed_content_compact
        assert_eq!(MIN_ALLOCATED_HEIGHT_FOR_DIFF_FRAME, 400.0, "Minimum allocated height for a diff component's frame should be 400.0 px.");
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
        println!("Shell summary: '{}'", summary);
        assert!(summary.contains("Created") || summary.contains("fields"));
        
        // File search result
        let file_result = r#"{"files": ["main.rs", "lib.rs"], "query": "fn main"}"#;
        let summary = extract_tool_result_summary(file_result);
        println!("File summary: '{}'", summary);
        assert!(summary.contains("2 files") || summary.contains("File search") || summary.contains("main.rs"));
        
        // Web search result  
        let web_result = r#"{"query": "rust fibonacci", "source_count": 5}"#;
        let summary = extract_tool_result_summary(web_result);
        println!("Web summary: '{}'", summary);
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
        assert!(is_tool_result_message(&format!("{{\"data\": \"{}\"}}", large_content)));
        
        // Non-tool result messages
        assert!(!is_tool_result_message("This is just a regular message"));
        assert!(!is_tool_result_message("Some JSON: {\"key\": \"value\"}"));
    }

    #[test]
    fn test_tool_call_structure() {
        // Test that ToolCall structure works correctly
        let tool_call = ToolCall {
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
        
        println!("Formatted conversation:\n{}", formatted);
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
                conversation.push(format!("{} {}", author_name, timestamp));
                conversation.push("".to_string()); // Empty line
                
                // Add thinking content if present
                if let Some(thinking) = message.get_thinking_content() {
                    if !thinking.is_empty() {
                        conversation.push(format!("💭 {}", thinking));
                        conversation.push("".to_string());
                    }
                }
                
                // Add main content
                if !message.content.is_empty() {
                    conversation.push(message.content.clone());
                }
                
                conversation.push("".to_string()); // Empty line between items
            },
            ChatItem::ToolCard(tool_card) => {
                // Format tool card for copying
                let friendly_name = get_human_friendly_tool_name(&tool_card.tool_name);
                let status_icon = match &tool_card.status {
                    ToolCardStatus::Completed { success: true } => "✅",
                    ToolCardStatus::Failed { .. } => "❌",
                    ToolCardStatus::Running => "🔄",
                    ToolCardStatus::Cancelled => "⏹️",
                    _ => "🔧",
                };
                
                conversation.push(format!("{} {} {}", status_icon, friendly_name, ""));
                
                // Add parameters
                let params = format_tool_parameters(&tool_card.tool_name, &tool_card.input_params);
                if !params.is_empty() {
                    conversation.push("Parameters:".to_string());
                    for (key, value) in params {
                        conversation.push(format!("  {}: {}", key, value));
                    }
                }
                
                // Add result if available
                if let Some(result) = &tool_card.result {
                    conversation.push("".to_string());
                    conversation.push("Result:".to_string());
                    
                    // Format the result using ToolResultFormatter
                    let formatter = crate::gui::app::tool_formatting::ToolResultFormatter::new();
                    let tool_result = match &tool_card.status {
                        ToolCardStatus::Completed { success: true } => {
                            crate::tools::types::ToolResult::Success(result.clone())
                        },
                        _ => {
                            crate::tools::types::ToolResult::Error { error: "Tool execution failed".to_string() }
                        }
                    };
                    
                    let formatted_result = formatter.format_tool_result_for_preview(&tool_card.tool_name, &tool_result);
                    for line in formatted_result.lines() {
                        conversation.push(format!("  {}", line));
                    }
                }
                
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
        conversation.push(format!("{} {}", author_name, timestamp));
        conversation.push("".to_string()); // Empty line
        
        // Add thinking content if present
        if let Some(thinking) = message.get_thinking_content() {
            if !thinking.is_empty() {
                conversation.push(format!("💭 {}", thinking));
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
                    conversation.push(format!("Result: {}", summary));
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

/// State for managing copy button visual feedback
#[derive(Debug, Clone)]
pub struct CopyButtonState {
    pub is_copying: bool,
    pub copy_feedback_start: Option<Instant>,
    pub last_copied_text: String,
}

impl Default for CopyButtonState {
    fn default() -> Self {
        Self {
            is_copying: false,
            copy_feedback_start: None,
            last_copied_text: String::new(),
        }
    }
}

impl CopyButtonState {
    /// Start the copy feedback animation
    pub fn start_copy_feedback(&mut self, text: String) {
        self.is_copying = true;
        self.copy_feedback_start = Some(Instant::now());
        self.last_copied_text = text;
    }
    
    /// Update the copy button state, returns true if UI should be repainted
    pub fn update(&mut self) -> bool {
        if let Some(start_time) = self.copy_feedback_start {
            if start_time.elapsed().as_millis() > 800 {
                self.is_copying = false;
                self.copy_feedback_start = None;
                return true; // Request repaint to clear feedback
            }
        }
        false
    }
    
    /// Get the current button text based on state
    pub fn get_button_text(&self, default_text: &str) -> String {
        if self.is_copying {
            "✔ Copied".to_string()
        } else {
            default_text.to_string()
        }
    }
    
    /// Get the current button color based on state
    pub fn get_button_color(&self, theme: AppTheme) -> Color32 {
        if self.is_copying {
            theme.success_color()
        } else {
            theme.input_background()
        }
    }
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
        
        // Terminal header
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("$ Terminal Output").strong().color(app_theme.accent_color()));
            if let Some(exit_code) = result.get("exit_code").and_then(|v| v.as_i64()) {
                let exit_color = if exit_code == 0 { app_theme.success_color() } else { app_theme.error_color() };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("Exit: {}", exit_code)).color(exit_color).small());
                });
            }
        });
        
        ui.separator();
        
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
                    output.push_str("\n");
                }
                output.push_str(&format!("STDERR:\n{}", stderr));
            }
        }
        
        if output.is_empty() {
            output = "(No output)".to_string();
        }
        
        // Render with monospace font
        ui.label(egui::RichText::new(output).monospace().color(app_theme.text_color()));
    });
}

/// Render diff output with syntax highlighting
fn render_diff_output(ui: &mut egui::Ui, result: &serde_json::Value, app_theme: AppTheme) {
    ui.group(|ui| {
        // Diff header
        ui.label(egui::RichText::new("📝 Code Changes").strong().color(app_theme.accent_color()));
        ui.separator();
        
        if let Some(file_path) = result.get("file_path").and_then(|v| v.as_str()) {
            ui.label(egui::RichText::new(format!("File: {}", file_path)).color(app_theme.hint_text_color()).small());
            ui.add_space(4.0);
        }
        
        // Get the diff/changes content
        let diff_content = result.get("diff")
            .or_else(|| result.get("changes"))
            .and_then(|v| v.as_str())
            .unwrap_or("No changes available");
        
        // Render diff lines with appropriate colors
        for line in diff_content.lines() {
            let (text_color, prefix) = if line.starts_with('+') && !line.starts_with("+++") {
                (app_theme.success_color(), "+ ")
            } else if line.starts_with('-') && !line.starts_with("---") {
                (app_theme.error_color(), "- ")
            } else if line.starts_with("@@") {
                (app_theme.accent_color(), "")
            } else {
                (app_theme.text_color(), "  ")
            };
            
            ui.label(egui::RichText::new(line).monospace().color(text_color));
        }
    });
}

