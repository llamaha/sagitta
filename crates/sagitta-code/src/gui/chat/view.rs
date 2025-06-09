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
    highlighting::{ThemeSet, Style as SyntectStyle, Theme},
    parsing::SyntaxSet,
    easy::HighlightLines,
    util::LinesWithEndings,
};
use similar::{ChangeTag, TextDiff};
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
    
    // Main message content first (this contains the conversation flow)
    if !message.content.is_empty() {
        render_message_content_compact(ui, message, &bg_color, max_width, app_theme);
    }
    
    // Tool calls (if any) - render inline after the main content
    if !message.tool_calls.is_empty() {
        ui.add_space(1.0); // Small spacing before tool calls
        if let Some(tool_info) = render_tool_calls_compact(ui, &message.tool_calls, &bg_color, max_width, app_theme) {
            clicked_tool = Some(tool_info);
        }
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
    let mut clicked_tool_result = None;
    
    for tool_call in tool_calls {
        // Only show the tool result as an inline preview link if there's a result
        if let Some(result) = &tool_call.result {
            if !result.trim().is_empty() {
                ui.horizontal(|ui| {
                    // Tool completion status and name
                    let (_status_icon, _status_color) = match tool_call.status {
                        MessageStatus::Complete => (symbols::get_success_symbol(), app_theme.success_color()),
                        MessageStatus::Error(_) => (symbols::get_error_symbol(), app_theme.error_color()),
                        _ => (symbols::get_tool_symbol(), app_theme.hint_text_color()),
                    };
                    
                    // Only show the wrench icon, not the status icon
                    ui.label(RichText::new(symbols::get_tool_symbol()).size(12.0));
                    ui.add_space(4.0);
                    
                    // Tool name
                    ui.label(RichText::new(&tool_call.name).color(app_theme.tool_color()).size(11.0));
                    ui.add_space(8.0);
                    
                    // Simple preview link
                    let preview_link = ui.link(RichText::new("preview").color(app_theme.accent_color()).size(11.0));
                    
                    if preview_link.clicked() {
                        // Determine display title based on tool type
                        let is_shell_result = tool_call.name.contains("shell") || tool_call.name.contains("execution") ||
                                             result.contains("stdout") || result.contains("stderr") ||
                                             result.contains("exit_code");
                        
                        let display_title = if is_shell_result {
                            format!("{} - Terminal Output", tool_call.name)
                        } else {
                            format!("{} - Result", tool_call.name)
                        };
                        
                        clicked_tool_result = Some((display_title, result.clone()));
                    }
                });
                
                ui.add_space(2.0); // Small spacing between tool results
            }
        }
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

/// Constants for diff rendering
const DIFF_COLLAPSING_THRESHOLD_LINES: usize = 10;
const EXPANDED_DIFF_SCROLL_AREA_MAX_HEIGHT: f32 = 360.0;
const MIN_ALLOCATED_HEIGHT_FOR_DIFF_FRAME: f32 = 400.0;

/// Render mixed content (text + code blocks) compactly
fn render_mixed_content_compact(ui: &mut Ui, content: &str, bg_color: &Color32, max_width: f32, app_theme: AppTheme) {
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
        return;
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
                    render_text_content_compact(ui, part, &bg_color, max_width, app_theme);
                }
            }
        } else {
            render_code_block_compact(ui, part, &bg_color, max_width, app_theme);
        }
    }
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
        // Test different types of tool results generate appropriate summaries
        
        // Shell execution result
        let shell_result = r#"{
            "exit_code": 0,
            "stdout": "     Created binary (application) `fibonacci_calculator` package\n",
            "stderr": "",
            "execution_time_ms": 156,
            "container_image": "local",
            "timed_out": false
        }"#;
        let summary = extract_tool_result_summary(shell_result);
        assert!(summary.contains("Created") || summary.contains("fields"));
        
        // File search result
        let file_result = r#"{"files": ["main.rs", "lib.rs"], "query": "fn main"}"#;
        let summary = extract_tool_result_summary(file_result);
        assert!(summary.contains("2 items") || summary.contains("fields"));
        
        // Web search result  
        let web_result = r#"{"query": "rust fibonacci", "source_count": 5}"#;
        let summary = extract_tool_result_summary(web_result);
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
}

