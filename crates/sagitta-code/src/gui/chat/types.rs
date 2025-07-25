use std::collections::HashMap;
use std::time::Instant;
use egui::Color32;
use crate::gui::theme::AppTheme;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MessageAuthor {
    User,
    Agent,
    System,
    Tool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    
    // Enhanced thinking support for streaming and fade-out
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
    
    // Enhanced thinking methods for streaming support
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
    
    pub fn format_time(&self) -> String {
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
    pub tool_calls: Vec<ToolCall>,
}

impl ChatMessage {
    pub fn new(author: MessageAuthor, text: String) -> Self {
        Self {
            author,
            text,
            timestamp: chrono::Utc::now(),
            id: None,
            tool_calls: Vec::new(),
        }
    }
    
    pub fn format_time(&self) -> String {
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
            tool_calls: msg.tool_calls,
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

pub struct CopyButtonState {
    pub is_copying: bool,
    pub copy_feedback_start: Option<Instant>,
    pub last_copied_text: String,
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

impl Default for CopyButtonState {
    fn default() -> Self {
        Self {
            is_copying: false,
            copy_feedback_start: None,
            last_copied_text: String::new(),
        }
    }
}