// Application state management for the Fred Agent

use crate::agent::state::types::{AgentMode, AgentState, ConversationStatus};
use crate::agent::message::types::{ToolCall, AgentMessage};
use super::super::theme::AppTheme;
use egui_notify::Toasts;
use uuid::Uuid;
use std::collections::VecDeque;

/// Application state management
pub struct AppState {
    // Chat state
    pub chat_input_buffer: String,
    pub current_response_id: Option<String>,
    pub chat_on_submit: bool,
    pub is_waiting_for_response: bool,
    
    // UI state
    pub current_theme: AppTheme,
    pub show_hotkeys_modal: bool,
    pub clicked_tool_info: Option<(String, String)>, // (tool_name, tool_args)
    pub toasts: Toasts,
    
    // Agent operational state flags for UI
    pub current_agent_state: AgentState,
    pub is_thinking: bool,
    pub is_responding: bool,
    pub is_streaming_response: bool,
    pub is_executing_tool: bool,

    // Temporary thinking indicator
    pub thinking_message: Option<String>,
    pub thinking_start_time: Option<std::time::Instant>,
    
    // Agent mode change tracking
    pub pending_agent_mode_change: Option<AgentMode>,
    pub current_agent_mode: AgentMode,
    
    // Conversation data cache and communication
    pub current_conversation_id: Option<Uuid>,
    pub current_conversation_title: Option<String>,
    pub conversation_list: Vec<crate::agent::conversation::types::ConversationSummary>,
    pub conversation_data_loading: bool,
    pub last_conversation_refresh: Option<std::time::Instant>,
    pub tool_results: std::collections::HashMap<String, String>,
    pub messages: Vec<AgentMessage>,
    pub pending_tool_calls: VecDeque<ToolCall>,
    pub active_tool_call_message_id: Option<Uuid>,
    
    // Loop control features
    pub is_in_loop: bool,
    pub loop_break_requested: bool,
    pub loop_inject_message: Option<String>,
    pub loop_inject_buffer: String,
    pub show_loop_inject_input: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            // Chat state
            chat_input_buffer: String::new(),
            current_response_id: None,
            chat_on_submit: false,
            is_waiting_for_response: false,
            
            // UI state
            current_theme: AppTheme::default(),
            show_hotkeys_modal: false,
            clicked_tool_info: None,
            toasts: Toasts::default(),
            
            // Agent operational state flags for UI
            current_agent_state: AgentState::default(),
            is_thinking: false,
            is_responding: false,
            is_streaming_response: false,
            is_executing_tool: false,

            // Temporary thinking indicator
            thinking_message: None,
            thinking_start_time: None,
            
            // Agent mode change tracking
            pending_agent_mode_change: None,
            current_agent_mode: AgentMode::FullyAutonomous,
            
            // Conversation data cache and communication
            current_conversation_id: None,
            current_conversation_title: None,
            conversation_list: Vec::new(),
            conversation_data_loading: false,
            last_conversation_refresh: None,
            tool_results: std::collections::HashMap::new(),
            messages: Vec::new(),
            pending_tool_calls: VecDeque::new(),
            active_tool_call_message_id: None,
            
            // Loop control features
            is_in_loop: false,
            loop_break_requested: false,
            loop_inject_message: None,
            loop_inject_buffer: String::new(),
            show_loop_inject_input: false,
        }
    }
} 