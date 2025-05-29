// Main application UI - modularized version

use std::sync::Arc;
use anyhow::Result;
use egui::{Context, Key};
use tokio::sync::{Mutex, mpsc, broadcast};
use uuid;
use std::ops::{Deref, DerefMut};

use super::repository::manager::RepositoryManager;
use super::repository::RepoPanel;
use super::settings::SettingsPanel;
use super::conversation::ConversationSidebar;
use crate::agent::Agent;
use crate::agent::message::types::{AgentMessage, ToolCall};
use crate::agent::state::types::{AgentState, AgentMode, AgentStateInfo};
use super::chat::StreamingChatManager;
use super::theme::AppTheme;
use crate::config::SagittaCodeConfig;
use sagitta_search::config::AppConfig;
use crate::agent::events::AgentEvent;

// Import the modularized components
mod panels;
mod events;
mod tool_formatting;
mod state;
mod rendering;
mod initialization;

// Re-export types and functions from modules
pub use panels::*;
pub use events::*;
pub use tool_formatting::*;
pub use state::*;

/// String extension trait for title case conversion
trait StringExt {
    fn to_title_case(&self) -> String;
}

impl StringExt for str {
    fn to_title_case(&self) -> String {
        self.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Main application UI
pub struct SagittaCodeApp {
    // Core components
    pub agent: Option<Arc<Agent>>,
    pub repo_panel: RepoPanel,
    pub chat_manager: Arc<StreamingChatManager>,
    pub settings_panel: SettingsPanel,
    conversation_sidebar: ConversationSidebar,
    config: Arc<SagittaCodeConfig>,
    app_core_config: Arc<AppConfig>,
    
    // State management - make public for direct access
    pub state: AppState,
    
    // Panel management
    panels: PanelManager,
    
    // Event handling
    agent_event_receiver: Option<broadcast::Receiver<AgentEvent>>,
    conversation_event_sender: Option<mpsc::UnboundedSender<ConversationEvent>>,
    conversation_event_receiver: Option<mpsc::UnboundedReceiver<ConversationEvent>>,
    app_event_sender: mpsc::UnboundedSender<AppEvent>,
    app_event_receiver: Option<mpsc::UnboundedReceiver<AppEvent>>,
    
    // Tool result formatting
    tool_formatter: ToolResultFormatter,
}

impl SagittaCodeApp {
    /// Create a new Sagitta Code App
    pub fn new(
        repo_manager: Arc<Mutex<RepositoryManager>>,
        sagitta_code_config: SagittaCodeConfig,
        app_core_config: AppConfig
    ) -> Self {
        let sagitta_code_config_arc = Arc::new(sagitta_code_config.clone());
        let app_core_config_arc = Arc::new(app_core_config.clone());

        // Create settings panel and initialize it with the current configs
        let settings_panel = SettingsPanel::new(sagitta_code_config.clone(), app_core_config.clone());

        // Create conversation event channel
        let (conversation_sender, conversation_receiver) = mpsc::unbounded_channel();
        // Create app event channel
        let (app_event_sender, app_event_receiver) = mpsc::unbounded_channel::<AppEvent>();

        // Create initial state and set theme from config
        let mut initial_state = AppState::new();
        match sagitta_code_config.ui.theme.as_str() {
            "light" => initial_state.current_theme = AppTheme::Light,
            "dark" | _ => initial_state.current_theme = AppTheme::Dark, // Default to Dark
        }

        Self {
            agent: None,
            repo_panel: RepoPanel::new(repo_manager.clone()),
            chat_manager: Arc::new(StreamingChatManager::new()),
            settings_panel,
            conversation_sidebar: ConversationSidebar::with_default_config(),
            config: sagitta_code_config_arc,
            app_core_config: app_core_config_arc,
            
            // Initialize state management with theme from config
            state: initial_state,
            
            // Initialize panel management
            panels: PanelManager::new(),
            
            // Event handling
            agent_event_receiver: None,
            conversation_event_sender: Some(conversation_sender),
            conversation_event_receiver: Some(conversation_receiver),
            app_event_sender,
            app_event_receiver: Some(app_event_receiver),
            
            // Tool result formatting
            tool_formatter: ToolResultFormatter::new(),
        }
    }

    /// Process agent events
    fn process_agent_events(&mut self) {
        events::process_agent_events(self);
    }
    
    /// Process app events
    fn process_app_events(&mut self) {
        events::process_app_events(self);
    }
    
    /// Create a chat message from an agent message
    fn make_chat_message_from_agent_message(&self, agent_msg: &AgentMessage) -> super::chat::view::ChatMessage {
        events::make_chat_message_from_agent_message(agent_msg)
    }
    
    /// Handle tool call events
    fn handle_tool_call(&mut self, tool_call: ToolCall) {
        events::handle_tool_call(self, tool_call);
    }
    
    /// Handle tool call result events
    fn handle_tool_call_result(&mut self, tool_call_id: String, tool_name: String, result: crate::tools::types::ToolResult) {
        events::handle_tool_call_result(self, tool_call_id, tool_name, result);
    }
    
    /// Handle agent state changes
    pub fn handle_state_change(&mut self, state: AgentState) {
        events::handle_state_change(self, state);
    }

    /// Render the application UI
    pub fn render(&mut self, ctx: &Context) {
        rendering::render(self, ctx);
    }

    /// Initialize application state, including loading configurations and setting up the agent
    pub async fn initialize(&mut self) -> Result<()> {
        initialization::initialize(self).await
    }

    /// Set preview panel content and make it visible
    pub fn show_preview(&mut self, title: &str, content: &str) {
        self.panels.show_preview(title, content);
    }

    /// Process conversation events from async tasks
    fn process_conversation_events(&mut self) {
        events::process_conversation_events(self);
    }
    
    /// Refresh conversation data asynchronously
    fn refresh_conversation_data(&mut self) {
        events::refresh_conversation_data(self);
    }
    
    /// Force refresh conversation data immediately
    fn force_refresh_conversation_data(&mut self) {
        events::force_refresh_conversation_data(self);
    }
    
    /// Switch to a conversation and update the chat view
    pub fn switch_to_conversation(&mut self, conversation_id: uuid::Uuid) {
        self.state.current_conversation_id = Some(conversation_id);
        self.state.messages.clear();
        self.state.chat_input_buffer.clear();
    }

    pub fn agent_state_info(&self) -> Option<Arc<tokio::sync::RwLock<AgentStateInfo>>> {
        if let Some(agent_arc) = &self.agent {
            Some(agent_arc.get_state_manager_state_info_arc())
        } else {
            None
        }
    }
}

// Implement Deref and DerefMut to allow direct access to state fields
impl Deref for SagittaCodeApp {
    type Target = AppState;
    
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for SagittaCodeApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

// Keep all the existing tests at the end of the file
// ... existing code ...

