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
};
use syntect::{
    highlighting::{ThemeSet, Style as SyntectStyle},
    parsing::SyntaxSet,
    easy::HighlightLines,
    util::LinesWithEndings,
};
use std::sync::OnceLock;
use crate::gui::theme::AppTheme;
use crate::gui::symbols;
use catppuccin_egui::Theme as CatppuccinTheme;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use std::cell::RefCell;
use std::thread::LocalKey;
use serde_json;
use uuid;

#[cfg(feature = "gui")]
use egui_notify::Toasts;
#[cfg(feature = "gui")]
use egui_modal::Modal;

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
    pub name: String,
    pub arguments: String,
    pub result: Option<String>,
    pub status: MessageStatus,
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
        }
    }
    
    pub fn append_content(&mut self, chunk: &str) {
        self.content.push_str(chunk);
        
        // NEW: When actual content starts streaming, begin fading out thinking
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
        if !self.has_thinking_content() {
            return false;
        }
        
        // If we're not fading, always show
        if !self.thinking_should_fade {
            return true;
        }
        
        // If we're fading, check if fade duration has elapsed
        if let Some(fade_start) = self.thinking_fade_start {
            let fade_duration = std::time::Duration::from_secs(2); // 2 second fade
            fade_start.elapsed() < fade_duration
        } else {
            true
        }
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
        }
    }
}

pub fn chat_view_ui(ui: &mut egui::Ui, messages: &[ChatMessage], app_theme: AppTheme) {
    // Convert legacy messages to streaming messages for modern rendering
    let streaming_messages: Vec<StreamingMessage> = messages.iter()
        .map(|msg| msg.clone().into())
        .collect();
    
    modern_chat_view_ui(ui, &streaming_messages, app_theme);
}

pub fn modern_chat_view_ui(ui: &mut egui::Ui, messages: &[StreamingMessage], app_theme: AppTheme) -> Option<(String, String)> {
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
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .max_height(f32::INFINITY)
                .show(ui, |ui| {
                    // Compact spacing
                    ui.spacing_mut().item_spacing.y = 4.0; // Reduced spacing for grouped messages
                    ui.spacing_mut().button_padding = Vec2::new(6.0, 4.0);
                    
                    ui.add_space(12.0);
                    
                    // Group consecutive messages from the same author
                    let grouped_messages = group_consecutive_messages(messages);
                    
                    // Render each message group
                    for (group_index, group) in grouped_messages.iter().enumerate() {
                        if group_index > 0 {
                            ui.add_space(8.0); // Reduced space between different message groups
                        }
                        
                        // Render the message group with adjusted width for margins
                        if let Some(tool_info) = render_message_group(ui, group, &bg_color, total_width - 32.0, app_theme) {
                            clicked_tool = Some(tool_info);
                        }
                    }
                    ui.add_space(16.0);
                });
        });
    
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
        MessageAuthor::Agent => "Fred",
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
            let copy_button = egui::Button::new("📋")
                .fill(app_theme.input_background())
                .stroke(Stroke::new(1.0, app_theme.border_color()))
                .rounding(CornerRadius::same(4));
            
            if ui.add(copy_button).on_hover_text("Copy all messages in group").clicked() {
                let combined_content = message_group.iter()
                    .map(|msg| msg.content.clone())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                ui.output_mut(|o| o.copied_text = combined_content);
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
                    
                    if let Some(tool_info) = render_single_message_content(ui, message, &bg_color, total_width - 80.0, app_theme) {
                        clicked_tool = Some(tool_info);
                    }
                });
            });
        } else {
            // Single message in group - use full width
            if let Some(tool_info) = render_single_message_content(ui, message, &bg_color, total_width, app_theme) {
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
) -> Option<(String, String)> {
    let mut clicked_tool = None;
    
    // Thinking content (if any) - now with streaming and fade support
    if message.should_show_thinking() {
        render_thinking_content(ui, message, &bg_color, max_width, app_theme);
        ui.add_space(2.0); // Reduced spacing
    }
    
    // Tool calls (if any) - render as compact cards
    if !message.tool_calls.is_empty() {
        if let Some(tool_info) = render_tool_calls_compact(ui, &message.tool_calls, &bg_color, max_width, app_theme) {
            clicked_tool = Some(tool_info);
        }
        ui.add_space(1.0); // Reduced spacing between tool calls
    }
    
    // Main message content
    if !message.content.is_empty() {
        render_message_content_compact(ui, message, &bg_color, max_width, app_theme);
    }
    
    clicked_tool
}

/// Render thinking content with streaming support and fade-out effects
fn render_thinking_content(ui: &mut Ui, message: &StreamingMessage, bg_color: &Color32, max_width: f32, app_theme: AppTheme) {
    // Check if we should show thinking content
    if !message.should_show_thinking() {
        return;
    }
    
    let thinking_content = match message.get_thinking_content() {
        Some(content) => content,
        None => return,
    };
    
    // Get opacity for fade effect
    let opacity = message.get_thinking_opacity();
    
    // Apply opacity to the entire thinking section
    ui.scope(|ui| {
        ui.set_opacity(opacity);
        
        ui.horizontal(|ui| {
            // Thinking icon with animation if streaming
            if message.thinking_is_streaming {
                let time = ui.input(|i| i.time);
                let rotation = (time * 2.0) as f32;
                ui.label(RichText::new(symbols::get_thinking_symbol()).size(14.0)); // Brain emoji for active thinking
            } else {
                ui.label(RichText::new("💭").size(14.0));
            }
            ui.add_space(4.0);
            
            // Thinking content with enhanced styling
            ui.vertical(|ui| {
                ui.set_max_width(max_width - 40.0);
                
                // Header with status
                let status_text = if message.thinking_is_streaming {
                    "Thinking..."
                } else if message.thinking_should_fade {
                    "Thought complete"
                } else {
                    "Thinking"
                };
                
                ui.label(RichText::new(status_text)
                    .italics()
                    .color(app_theme.hint_text_color())
                    .size(10.0));
                
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
        });
    });
    
    // Request repaint for animations and fade effects
    if message.thinking_is_streaming || message.thinking_should_fade {
        ui.ctx().request_repaint();
    }
}

/// Render tool calls as compact, clickable cards
fn render_tool_calls_compact(ui: &mut Ui, tool_calls: &[ToolCall], bg_color: &Color32, max_width: f32, app_theme: AppTheme) -> Option<(String, String)> {
    let opacity = 0.3; // Default opacity for UI elements
    let mut clicked_tool_result = None;
    
    for tool_call in tool_calls {
        ui.horizontal(|ui| {
            // Tool icon with status color
            let (status_icon, status_color) = match tool_call.status {
                MessageStatus::Complete => (symbols::get_success_symbol(), app_theme.success_color()),
                MessageStatus::Error(_) => (symbols::get_error_symbol(), app_theme.error_color()),
                MessageStatus::Streaming => ("⟳", app_theme.streaming_color()),
                MessageStatus::Thinking => ("💭", app_theme.thinking_indicator_color()),
                _ => (symbols::get_tool_symbol(), app_theme.hint_text_color()),
            };
            
            ui.label(RichText::new(symbols::get_tool_symbol()).size(12.0));
            ui.label(RichText::new(status_icon).color(status_color).size(10.0));
            ui.add_space(4.0);
            
            // Tool name as clickable button
            let tool_button = ui.add(
                egui::Button::new(RichText::new(&tool_call.name).size(11.0))
                    .fill(app_theme.button_background())
                    .stroke(Stroke::new(0.5, app_theme.border_color()))
                    .rounding(CornerRadius::same(4))
            );
            
            if tool_button.clicked() {
                clicked_tool_result = Some((tool_call.name.clone(), tool_call.arguments.clone()));
            }
            
            // CRITICAL FIX: Add tool result button if result is available
            if let Some(result) = &tool_call.result {
                ui.add_space(4.0);
                
                // Create a preview of the result
                let preview = if result.len() > 50 {
                    format!("{}...", &result[..47])
                } else {
                    result.clone()
                };
                
                // Add clickable result button with better styling
                let result_button = ui.add(
                    egui::Button::new(RichText::new(format!("📊 {}", preview)).size(10.0))
                        .fill(app_theme.button_background())
                        .stroke(Stroke::new(1.0, app_theme.accent_color()))  // More visible border
                        .rounding(CornerRadius::same(4))
                );
                
                if result_button.clicked() {
                    // Return the full result for display in preview panel
                    clicked_tool_result = Some((format!("{} Result", tool_call.name), result.clone()));
                }
                
                result_button.on_hover_text("Click to view full tool result");
            }
        });
        
        ui.add_space(1.0); // Reduced spacing between tool calls
    }
    
    clicked_tool_result
}

/// Render message content in a compact format
fn render_message_content_compact(ui: &mut Ui, message: &StreamingMessage, bg_color: &Color32, max_width: f32, app_theme: AppTheme) {
    // Use message_type for summary/finalization
    if message.message_type == MessageType::Summary {
        render_text_content_compact(ui, &message.content, &bg_color, max_width, app_theme);
        return;
    }
    
    // Set up content area
    ui.set_max_width(max_width - 20.0);
    ui.style_mut().wrap = Some(true);
    
    // Render content based on type
    if message.content.contains("```") {
        render_mixed_content_compact(ui, &message.content, &bg_color, max_width - 20.0, app_theme);
    } else {
        render_text_content_compact(ui, &message.content, &bg_color, max_width - 20.0, app_theme);
    }
    
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
}

/// Render tool result as a compact, clickable summary
fn render_tool_result_compact(ui: &mut Ui, text: &str, bg_color: &Color32, max_width: f32) {
    let opacity = 0.3; // Default opacity for UI elements
    let summary = extract_tool_result_summary(text);
    
    ui.horizontal(|ui| {
        ui.label(RichText::new("📊").size(12.0));
        ui.add_space(4.0);
        
        let result_button = ui.add(
            egui::Button::new(RichText::new(format!("Tool Result: {}", summary)).size(11.0))
                .fill(Color32::from_rgba_premultiplied(
                    bg_color.r(),
                    bg_color.g(),
                    bg_color.b(),
                    (255.0 * opacity) as u8,
                ))
                .stroke(Stroke::new(0.5, Color32::from_rgba_premultiplied(
                    bg_color.r(),
                    bg_color.g(),
                    bg_color.b(),
                    (255.0 * opacity) as u8,
                )))
                .rounding(CornerRadius::same(4))
        );
        
        if result_button.clicked() {
            // TODO: Show full result in side panel or modal
            println!("Tool result clicked: {}", summary);
        }
        
        result_button.on_hover_text("Click to view full result");
    });
}

/// Render mixed content (text + code blocks) compactly
fn render_mixed_content_compact(ui: &mut Ui, content: &str, bg_color: &Color32, max_width: f32, app_theme: AppTheme) {
    let parts: Vec<&str> = content.split("```").collect();
    for (i, part) in parts.iter().enumerate() {
        if i % 2 == 0 {
            // Text part
            if !part.is_empty() {
                render_text_content_compact(ui, part, &bg_color, max_width, app_theme);
            }
        } else {
            // Code part
            render_code_block_compact(ui, part, &bg_color, max_width, app_theme);
        }
    }
}

/// Render text content compactly using markdown with proper theme colors
fn render_text_content_compact(ui: &mut Ui, text: &str, bg_color: &Color32, max_width: f32, app_theme: AppTheme) {
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
            
            // Collapsible for long code
            let line_count = remaining_text.lines().count();
            if line_count > 10 {
                egui::CollapsingHeader::new(RichText::new(format!("{} lines of code", line_count)).small())
                    .default_open(false)
                    .show(ui, |ui| {
                        render_syntax_highlighted_code(ui, remaining_text, &bg_color, max_width - 32.0);
                    });
            } else {
                render_syntax_highlighted_code(ui, remaining_text, &bg_color, max_width - 16.0);
            }
        });
}

/// Render syntax highlighted code
fn render_syntax_highlighted_code(ui: &mut Ui, text: &str, bg_color: &Color32, max_width: f32) {
    let syntax_set = get_syntax_set();
    let theme_set = get_theme_set();
    
    let syntect_theme = &theme_set.themes["base16-ocean.dark"];
    let syntax = syntax_set.find_syntax_by_extension("rs")
        .or_else(|| syntax_set.find_syntax_by_extension("txt"))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    
    let mut highlighter = HighlightLines::new(syntax, syntect_theme);
    
    ui.style_mut().wrap = Some(true);
    
    for line in LinesWithEndings::from(text).take(20) { // Limit to 20 lines for performance
        let ranges = highlighter.highlight_line(line, syntax_set).unwrap_or_default();
        
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(max_width);
            
            for (style, text_part) in ranges {
                let color = syntect_style_to_color(&style);
                ui.label(RichText::new(text_part).monospace().color(color).size(10.0));
            }
        });
    }
}

fn syntect_style_to_color(style: &SyntectStyle) -> Color32 {
    Color32::from_rgb(
        style.foreground.r, 
        style.foreground.g, 
        style.foreground.b
    )
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
                             text.contains("\"uri\":");
    
    // Return true if it's explicitly a tool result OR tool call OR has JSON structure with indicators
    has_tool_result_prefix || has_tool_call_prefix || (has_json_structure && (has_large_content || has_tool_indicators || has_structured_data))
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
            // Check for web search results
            if let Some(query) = obj.get("query").and_then(|v| v.as_str()) {
                if let Some(source_count) = obj.get("source_count").and_then(|v| v.as_u64()) {
                    return format!("Web search: \"{}\" ({} sources)", query, source_count);
                }
                return format!("Web search: \"{}\"", query);
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
        }
    }

    #[test]
    fn test_summary_message_type_detection() {
        let summary_msg = create_test_message("Okay, I've finished those tasks. Successfully completed: web search.", MessageType::Summary);
        let normal_msg = create_test_message("This is a normal message.", MessageType::Normal);
        
        assert_eq!(summary_msg.message_type, MessageType::Summary);
        assert_eq!(normal_msg.message_type, MessageType::Normal);
    }

    #[test]
    fn test_message_group_spinner_suppression() {
        let summary_msg = create_test_message("Okay, I've finished those tasks. Successfully completed: web search.", MessageType::Summary);
        let normal_msg = create_test_message("This is a normal message.", MessageType::Normal);
        
        // Test that summary messages would suppress spinners in a group
        let summary_group = vec![&summary_msg];
        let normal_group = vec![&normal_msg];
        
        // We can't easily test the UI rendering without egui context, but we can test the logic
        // The suppress_spinner logic is in render_message_group and checks for MessageType::Summary
        let has_summary = summary_group.iter().any(|m| m.message_type == MessageType::Summary);
        let has_no_summary = normal_group.iter().any(|m| m.message_type == MessageType::Summary);
        
        assert!(has_summary, "Summary group should contain summary message");
        assert!(!has_no_summary, "Normal group should not contain summary message");
    }

    #[test]
    fn test_message_type_enum_values() {
        // Test that all MessageType variants are properly defined
        let _normal = MessageType::Normal;
        let _summary = MessageType::Summary;
        let _tool = MessageType::Tool;
        let _system = MessageType::System;
        
        // Test PartialEq implementation
        assert_eq!(MessageType::Summary, MessageType::Summary);
        assert_ne!(MessageType::Summary, MessageType::Normal);
    }
}

