// Application state management for the Sagitta Code

use crate::agent::state::types::{AgentMode, AgentState};
use crate::agent::message::types::{ToolCall, AgentMessage};
use crate::agent::events::ToolRunId;
use crate::gui::conversation::sidebar::SidebarAction;
use crate::gui::chat::view::CopyButtonState;
use super::super::theme::AppTheme;
use crate::providers::types::ProviderType;
use egui_notify::Toasts;
use uuid::Uuid;
use std::collections::{HashMap, VecDeque};

/// Information about a currently running tool
#[derive(Debug, Clone)]
pub struct RunningToolInfo {
    pub tool_name: String,
    pub progress: Option<f32>,
    pub message_id: String,
    pub start_time: std::time::Instant,
}

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
    pub show_tools_modal: bool,
    pub show_json_modal: bool,
    pub json_modal_content: Option<String>,
    pub show_file_content_modal: bool,
    pub file_content_modal_data: Option<(String, String)>, // (file_path, content)
    pub show_new_conversation_confirmation: bool,
    pub clicked_tool_info: Option<(String, String)>, // (tool_name, tool_args)
    pub toasts: Toasts,
    pub copy_button_state: CopyButtonState,
    
    // Repository context state
    pub current_repository_context: Option<String>,
    pub available_repositories: Vec<String>,
    pub pending_repository_context_change: Option<String>,
    pub pending_git_repository_path: Option<std::path::PathBuf>,
    
    // Provider context state
    pub current_provider: ProviderType,
    pub available_providers: Vec<ProviderType>,
    pub pending_provider_change: Option<ProviderType>,
    pub pending_agent_replacement: Option<std::sync::Arc<crate::agent::core::Agent>>,
    pub show_provider_quick_switch: bool,
    pub show_provider_setup_dialog: bool,
    
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
    
    // Running tool tracking
    pub running_tools: HashMap<ToolRunId, RunningToolInfo>,
    pub tool_call_to_run_id: HashMap<String, ToolRunId>, // Maps tool_call_id to run_id
    pub active_tool_calls: HashMap<String, String>, // Maps tool_call_id to message_id
    pub completed_tool_results: HashMap<String, (String, crate::agent::events::ToolResult)>, // Maps tool_call_id to (tool_name, result)
    
    // Thinking content state
    pub collapsed_thinking: HashMap<String, bool>, // message_id -> collapsed state
    
    // Token usage tracking
    pub current_token_usage: Option<crate::llm::client::TokenUsage>,
    
    // Sidebar state
    pub sidebar_action: Option<SidebarAction>,
    pub editing_conversation_id: Option<Uuid>,
    

    
    // Loop control features
    pub is_in_loop: bool,
    pub loop_break_requested: bool,
    pub loop_inject_message: Option<String>,
    pub loop_inject_buffer: String,
    pub show_loop_inject_input: bool,
    
    // Stop/Cancel request
    pub stop_requested: bool,
    
    // Input focus management
    pub should_focus_input: bool,
    
    // Tool card collapse state
    pub tool_cards_collapsed: bool,
    pub tool_card_individual_states: HashMap<String, bool>, // tool_call_id -> collapsed state
    
    // Undo/redo for chat input
    pub input_undo_stack: Vec<String>,
    pub input_redo_stack: Vec<String>,
    pub last_input_snapshot: String,
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
            show_tools_modal: false,
            show_json_modal: false,
            json_modal_content: None,
            show_file_content_modal: false,
            file_content_modal_data: None,
            show_new_conversation_confirmation: false,
            clicked_tool_info: None,
            toasts: Toasts::default(),
            copy_button_state: CopyButtonState::default(),
            
            // Repository context state
            current_repository_context: None,
            available_repositories: Vec::new(),
            pending_repository_context_change: None,
            pending_git_repository_path: None,
            
            // Provider context state
            current_provider: ProviderType::default(),
            available_providers: vec![ProviderType::ClaudeCode, ProviderType::OpenAICompatible],
            pending_provider_change: None,
            pending_agent_replacement: None,
            show_provider_quick_switch: false,
            show_provider_setup_dialog: false,
            
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
            messages: Vec::new(), // Start with empty messages to ensure welcome screen shows
            pending_tool_calls: VecDeque::new(),
            active_tool_call_message_id: None,
            
            // Running tool tracking
            running_tools: HashMap::new(),
            tool_call_to_run_id: HashMap::new(),
            active_tool_calls: HashMap::new(),
            completed_tool_results: HashMap::new(),
            
            // Thinking content state
            collapsed_thinking: HashMap::new(),
            
            // Token usage tracking
            current_token_usage: None,
            
            // Sidebar state
            sidebar_action: None,
            editing_conversation_id: None,
            

            
            // Loop control features
            is_in_loop: false,
            loop_break_requested: false,
            loop_inject_message: None,
            loop_inject_buffer: String::new(),
            show_loop_inject_input: false,
            
            // Stop/Cancel request
            stop_requested: false,
            
            // Input focus management
            should_focus_input: true, // Focus input on startup
            
            // Tool card collapse state
            tool_cards_collapsed: false, // Start with tool cards expanded
            tool_card_individual_states: HashMap::new(),
            
            // Undo/redo for chat input
            input_undo_stack: Vec::new(),
            input_redo_stack: Vec::new(),
            last_input_snapshot: String::new(),
        }
    }

    /// Clear chat input buffer
    pub fn clear_chat_input(&mut self) {
        self.chat_input_buffer.clear();
        self.chat_on_submit = false;
    }
    
    /// Get display name for a provider type
    pub fn provider_display_name(provider_type: ProviderType) -> &'static str {
        match provider_type {
            ProviderType::ClaudeCode => "Claude Code",

            ProviderType::OpenAICompatible => "OpenAI Compatible",
            ProviderType::ClaudeCodeRouter => "Claude Code Router",
            ProviderType::MistralRs => "Mistral.rs",
        }
    }

    /// Set agent thinking state
    pub fn set_thinking(&mut self, thinking: bool, message: Option<String>) {
        self.is_thinking = thinking;
        self.thinking_message = message;
        if thinking {
            self.thinking_start_time = Some(std::time::Instant::now());
        } else {
            self.thinking_start_time = None;
        }
    }

    /// Update agent operational state
    pub fn update_agent_operational_state(&mut self, responding: bool, streaming: bool, executing_tool: bool) {
        self.is_responding = responding;
        self.is_streaming_response = streaming;
        self.is_executing_tool = executing_tool;
        self.is_waiting_for_response = responding || streaming || executing_tool;
    }

    /// Set conversation loading state
    pub fn set_conversation_loading(&mut self, loading: bool) {
        self.conversation_data_loading = loading;
        if !loading {
            self.last_conversation_refresh = Some(std::time::Instant::now());
        }
    }

    /// Add a tool result
    pub fn add_tool_result(&mut self, tool_id: String, result: String) {
        self.tool_results.insert(tool_id, result);
    }

    /// Clear all tool results
    pub fn clear_tool_results(&mut self) {
        self.tool_results.clear();
    }

    /// Set loop state
    pub fn set_loop_state(&mut self, in_loop: bool) {
        self.is_in_loop = in_loop;
        if !in_loop {
            self.loop_break_requested = false;
            self.loop_inject_message = None;
            self.loop_inject_buffer.clear();
            self.show_loop_inject_input = false;
        }
    }

    /// Set current repository context
    pub fn set_repository_context(&mut self, repo_name: Option<String>) {
        self.current_repository_context = repo_name;
        self.pending_repository_context_change = None;
    }

    /// Update available repositories list
    pub fn update_available_repositories(&mut self, repositories: Vec<String>) {
        log::info!("Updating available repositories: {repositories:?}");
        self.available_repositories = repositories;
        log::info!("Available repositories updated. Current list: {:?}", self.available_repositories);
    }

    /// Request repository context change
    pub fn request_repository_context_change(&mut self, repo_name: String) {
        self.pending_repository_context_change = Some(repo_name);
    }

    /// Get current repository context display name
    pub fn get_repository_context_display(&self) -> String {
        match &self.current_repository_context {
            Some(repo) => format!("📁 {repo}"),
            None => "📁 No Repository".to_string(),
        }
    }

    /// Request loop break
    pub fn request_loop_break(&mut self) {
        self.loop_break_requested = true;
    }

    /// Toggle hotkeys modal
    pub fn toggle_hotkeys_modal(&mut self) {
        self.show_hotkeys_modal = !self.show_hotkeys_modal;
    }

    /// Check if any async operation is in progress
    pub fn is_busy(&self) -> bool {
        self.is_waiting_for_response || self.conversation_data_loading || self.is_in_loop
    }

    /// Get thinking duration if currently thinking
    pub fn thinking_duration(&self) -> Option<std::time::Duration> {
        if self.is_thinking {
            self.thinking_start_time.map(|start| start.elapsed())
        } else {
            None
        }
    }

    // Terminal functionality removed




    /// Switch to a conversation and update the chat view
    pub fn switch_to_conversation(&mut self, conversation_id: Uuid) {
        self.current_conversation_id = Some(conversation_id);
        self.messages.clear();
        // Additional logic for switching conversations can be added here
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::thread;

    #[test]
    fn test_app_state_initialization() {
        let state = AppState::new();
        
        // Test initial chat state
        assert!(state.chat_input_buffer.is_empty());
        assert!(state.current_response_id.is_none());
        assert!(!state.chat_on_submit);
        assert!(!state.is_waiting_for_response);
        
        // Test initial UI state
        assert_eq!(state.current_theme, AppTheme::default());
        assert!(!state.show_hotkeys_modal);
        assert!(state.clicked_tool_info.is_none());
        
        // Test initial agent state
        assert!(!state.is_thinking);
        assert!(!state.is_responding);
        assert!(!state.is_streaming_response);
        assert!(!state.is_executing_tool);
        assert!(state.thinking_message.is_none());
        assert!(state.thinking_start_time.is_none());
        
        // Test initial conversation state
        assert!(state.current_conversation_id.is_none());
        assert!(state.current_conversation_title.is_none());
        assert!(state.conversation_list.is_empty());
        assert!(!state.conversation_data_loading);
        assert!(state.last_conversation_refresh.is_none());
        assert!(state.tool_results.is_empty());
        assert!(state.messages.is_empty());
        assert!(state.pending_tool_calls.is_empty());
        assert!(state.active_tool_call_message_id.is_none());
        
        // Test initial running tools state
        assert!(state.running_tools.is_empty());
        
        // Test initial sidebar state
        assert!(state.sidebar_action.is_none());
        assert!(state.editing_conversation_id.is_none());
        
        // Test initial loop state
        assert!(!state.is_in_loop);
        assert!(!state.loop_break_requested);
        assert!(state.loop_inject_message.is_none());
        assert!(state.loop_inject_buffer.is_empty());
        assert!(!state.show_loop_inject_input);
        
        // Terminal state fields have been removed
        // These tests are no longer applicable


    }

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        let new_state = AppState::new();
        
        assert_eq!(state.chat_input_buffer, new_state.chat_input_buffer);
        assert_eq!(state.current_theme, new_state.current_theme);
        assert_eq!(state.current_agent_mode, new_state.current_agent_mode);
    }

    #[test]
    fn test_clear_chat_input() {
        let mut state = AppState::new();
        state.chat_input_buffer = "Hello world".to_string();
        state.chat_on_submit = true;
        
        state.clear_chat_input();
        
        assert!(state.chat_input_buffer.is_empty());
        assert!(!state.chat_on_submit);
    }

    #[test]
    fn test_set_thinking() {
        let mut state = AppState::new();
        
        // Start thinking
        state.set_thinking(true, Some("Processing your request...".to_string()));
        assert!(state.is_thinking);
        assert_eq!(state.thinking_message, Some("Processing your request...".to_string()));
        assert!(state.thinking_start_time.is_some());
        
        // Stop thinking
        state.set_thinking(false, None);
        assert!(!state.is_thinking);
        assert!(state.thinking_message.is_none());
        assert!(state.thinking_start_time.is_none());
    }

    #[test]
    fn test_thinking_duration() {
        let mut state = AppState::new();
        
        // Not thinking - should return None
        assert!(state.thinking_duration().is_none());
        
        // Start thinking
        state.set_thinking(true, Some("Thinking...".to_string()));
        thread::sleep(Duration::from_millis(10));
        
        let duration = state.thinking_duration();
        assert!(duration.is_some());
        assert!(duration.unwrap() >= Duration::from_millis(10));
        
        // Stop thinking
        state.set_thinking(false, None);
        assert!(state.thinking_duration().is_none());
    }

    #[test]
    fn test_update_agent_operational_state() {
        let mut state = AppState::new();
        
        // Test all false
        state.update_agent_operational_state(false, false, false);
        assert!(!state.is_responding);
        assert!(!state.is_streaming_response);
        assert!(!state.is_executing_tool);
        assert!(!state.is_waiting_for_response);
        
        // Test responding
        state.update_agent_operational_state(true, false, false);
        assert!(state.is_responding);
        assert!(!state.is_streaming_response);
        assert!(!state.is_executing_tool);
        assert!(state.is_waiting_for_response);
        
        // Test streaming
        state.update_agent_operational_state(false, true, false);
        assert!(!state.is_responding);
        assert!(state.is_streaming_response);
        assert!(!state.is_executing_tool);
        assert!(state.is_waiting_for_response);
        
        // Test executing tool
        state.update_agent_operational_state(false, false, true);
        assert!(!state.is_responding);
        assert!(!state.is_streaming_response);
        assert!(state.is_executing_tool);
        assert!(state.is_waiting_for_response);
        
        // Test all active
        state.update_agent_operational_state(true, true, true);
        assert!(state.is_responding);
        assert!(state.is_streaming_response);
        assert!(state.is_executing_tool);
        assert!(state.is_waiting_for_response);
    }

    #[test]
    fn test_conversation_loading_state() {
        let mut state = AppState::new();
        
        // Start loading
        state.set_conversation_loading(true);
        assert!(state.conversation_data_loading);
        assert!(state.last_conversation_refresh.is_none());
        
        // Stop loading
        state.set_conversation_loading(false);
        assert!(!state.conversation_data_loading);
        assert!(state.last_conversation_refresh.is_some());
    }

    #[test]
    fn test_tool_results_management() {
        let mut state = AppState::new();
        
        // Add tool results
        state.add_tool_result("tool1".to_string(), "result1".to_string());
        state.add_tool_result("tool2".to_string(), "result2".to_string());
        
        assert_eq!(state.tool_results.len(), 2);
        assert_eq!(state.tool_results.get("tool1"), Some(&"result1".to_string()));
        assert_eq!(state.tool_results.get("tool2"), Some(&"result2".to_string()));
        
        // Clear all results
        state.clear_tool_results();
        assert!(state.tool_results.is_empty());
    }

    #[test]
    fn test_loop_state_management() {
        let mut state = AppState::new();
        
        // Start loop
        state.set_loop_state(true);
        assert!(state.is_in_loop);
        
        // Add some loop-related state
        state.loop_break_requested = true;
        state.loop_inject_message = Some("Inject this".to_string());
        state.loop_inject_buffer = "Buffer content".to_string();
        state.show_loop_inject_input = true;
        
        // End loop - should clear all related state
        state.set_loop_state(false);
        assert!(!state.is_in_loop);
        assert!(!state.loop_break_requested);
        assert!(state.loop_inject_message.is_none());
        assert!(state.loop_inject_buffer.is_empty());
        assert!(!state.show_loop_inject_input);
    }

    #[test]
    fn test_request_loop_break() {
        let mut state = AppState::new();
        
        assert!(!state.loop_break_requested);
        state.request_loop_break();
        assert!(state.loop_break_requested);
    }

    #[test]
    fn test_toggle_hotkeys_modal() {
        let mut state = AppState::new();
        
        assert!(!state.show_hotkeys_modal);
        state.toggle_hotkeys_modal();
        assert!(state.show_hotkeys_modal);
        state.toggle_hotkeys_modal();
        assert!(!state.show_hotkeys_modal);
    }

    #[test]
    fn test_is_busy() {
        let mut state = AppState::new();
        
        // Initially not busy
        assert!(!state.is_busy());
        
        // Waiting for response makes it busy
        state.is_waiting_for_response = true;
        assert!(state.is_busy());
        state.is_waiting_for_response = false;
        
        // Loading conversation makes it busy
        state.conversation_data_loading = true;
        assert!(state.is_busy());
        state.conversation_data_loading = false;
        
        // In loop makes it busy
        state.is_in_loop = true;
        assert!(state.is_busy());
        state.is_in_loop = false;
        
        assert!(!state.is_busy());
    }

    #[test]
    fn test_agent_mode_transitions() {
        let mut state = AppState::new();
        
        // Test initial mode
        assert_eq!(state.current_agent_mode, AgentMode::FullyAutonomous);
        assert!(state.pending_agent_mode_change.is_none());
        
        // Test pending mode change
        state.pending_agent_mode_change = Some(AgentMode::ChatOnly);
        assert_eq!(state.pending_agent_mode_change, Some(AgentMode::ChatOnly));
        
        // Test mode change application
        state.current_agent_mode = state.pending_agent_mode_change.take().unwrap();
        assert_eq!(state.current_agent_mode, AgentMode::ChatOnly);
        assert!(state.pending_agent_mode_change.is_none());
    }

    #[test]
    fn test_conversation_state() {
        let mut state = AppState::new();
        let conversation_id = Uuid::new_v4();
        
        // Test setting conversation
        state.current_conversation_id = Some(conversation_id);
        state.current_conversation_title = Some("Test Conversation".to_string());
        
        assert_eq!(state.current_conversation_id, Some(conversation_id));
        assert_eq!(state.current_conversation_title, Some("Test Conversation".to_string()));
        
        // Test clearing conversation
        state.current_conversation_id = None;
        state.current_conversation_title = None;
        
        assert!(state.current_conversation_id.is_none());
        assert!(state.current_conversation_title.is_none());
    }

    #[test]
    fn test_tool_call_state() {
        let mut state = AppState::new();
        let message_id = Uuid::new_v4();
        
        // Test tool call message tracking
        state.active_tool_call_message_id = Some(message_id);
        assert_eq!(state.active_tool_call_message_id, Some(message_id));
        
        // Test pending tool calls
        assert!(state.pending_tool_calls.is_empty());
        // Note: We can't easily create ToolCall instances without more dependencies
        // so we just test the basic structure
    }

    #[test]
    fn test_clicked_tool_info() {
        let mut state = AppState::new();
        
        // Test setting tool info
        state.clicked_tool_info = Some(("web_search".to_string(), "query: rust".to_string()));
        
        let (tool_name, tool_args) = state.clicked_tool_info.as_ref().unwrap();
        assert_eq!(tool_name, "web_search");
        assert_eq!(tool_args, "query: rust");
        
        // Test clearing tool info
        state.clicked_tool_info = None;
        assert!(state.clicked_tool_info.is_none());
    }

    #[test]
    fn test_theme_management() {
        let mut state = AppState::new();
        
        // Test initial theme
        assert_eq!(state.current_theme, AppTheme::default());
        
        // Test theme changes
        state.current_theme = AppTheme::Light;
        assert_eq!(state.current_theme, AppTheme::Light);
        
        state.current_theme = AppTheme::Dark;
        assert_eq!(state.current_theme, AppTheme::Dark);
        
        state.current_theme = AppTheme::Custom;
        assert_eq!(state.current_theme, AppTheme::Custom);
    }


    #[test]
    fn test_switch_to_conversation() {
        let mut state = AppState::new();
        let conversation_id = Uuid::new_v4();
        
        // Add some messages to test clearing
        state.messages.push(AgentMessage::user("test message"));
        
        // Switch to conversation
        state.switch_to_conversation(conversation_id);
        
        // Verify conversation was switched and messages cleared
        assert_eq!(state.current_conversation_id, Some(conversation_id));
        assert!(state.messages.is_empty());
    }

    #[test]
    fn test_repository_context_management() {
        let mut state = AppState::new();
        
        // Initially no repository context
        assert_eq!(state.current_repository_context, None);
        assert_eq!(state.pending_repository_context_change, None);
        
        // Set repository context
        state.set_repository_context(Some("test-repo".to_string()));
        assert_eq!(state.current_repository_context, Some("test-repo".to_string()));
        assert_eq!(state.pending_repository_context_change, None);
        
        // Clear repository context
        state.set_repository_context(None);
        assert_eq!(state.current_repository_context, None);
    }

    #[test]
    fn test_request_repository_context_change() {
        let mut state = AppState::new();
        
        // Request a repository change
        state.request_repository_context_change("new-repo".to_string());
        assert_eq!(state.pending_repository_context_change, Some("new-repo".to_string()));
        
        // Current context should not change yet
        assert_eq!(state.current_repository_context, None);
    }

    #[test]
    fn test_update_available_repositories() {
        let mut state = AppState::new();
        
        // Initially empty
        assert!(state.available_repositories.is_empty());
        
        // Update with repositories
        let repos = vec!["repo1".to_string(), "repo2".to_string(), "repo3".to_string()];
        state.update_available_repositories(repos.clone());
        
        assert_eq!(state.available_repositories, repos);
    }

    #[test]
    fn test_get_repository_context_display() {
        let mut state = AppState::new();
        
        // No repository
        assert_eq!(state.get_repository_context_display(), "📁 No Repository");
        
        // With repository
        state.set_repository_context(Some("my-project".to_string()));
        assert_eq!(state.get_repository_context_display(), "📁 my-project");
    }

    #[test]
    #[ignore = "TerminalConfig and BufferConfig have been removed"]
    fn test_terminal_configuration_prevents_flickering() {
        // Test that terminal is configured with increased line limits to prevent flickering
        let state = AppState::new();
        
        // Terminal widget fields have been removed
        // These assertions are no longer applicable
        
        // The test below is disabled because TerminalConfig and BufferConfig have been removed
        /*
        // Test the configuration we would use
        let test_config = TerminalConfig::default()
            .with_max_lines(50000).unwrap()
            .with_buffer_config(BufferConfig {
                max_lines: 50000,
                lines_to_keep: 40000,
                cleanup_interval_ms: 5000,
            }).unwrap();
        
        // Verify the configuration is valid
        assert!(test_config.validate().is_ok());
        
        // Verify the line limits are correct to prevent flickering at 5000 lines
        assert_eq!(test_config.buffer.max_lines, 50000);
        assert_eq!(test_config.buffer.lines_to_keep, 40000);
        assert_eq!(test_config.buffer.cleanup_interval_ms, 5000);
        
        // Ensure lines_to_keep is significantly higher than the old 5000 line flickering point
        assert!(test_config.buffer.lines_to_keep > 5000 * 5); // At least 5x the old limit
        */
    }
} 