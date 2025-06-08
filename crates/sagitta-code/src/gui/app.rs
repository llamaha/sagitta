// Main application UI - modularized version

use std::sync::Arc;
use anyhow::Result;
use egui::{Context, Key};
use tokio::sync::{Mutex, mpsc, broadcast};
use uuid;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

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
use crate::agent::conversation::service::ConversationService;
use crate::agent::conversation::clustering::ConversationClusteringManager;
use crate::agent::conversation::analytics::{ConversationAnalyticsManager, AnalyticsConfig};
use crate::agent::conversation::manager::{ConversationManager, ConversationManagerImpl};
use crate::agent::conversation::persistence::disk::DiskConversationPersistence;
use crate::agent::conversation::search::text::TextConversationSearchEngine;
use crate::project::workspace::manager::{WorkspaceManager, WorkspaceManagerImpl};
use crate::agent::conversation::tagging::{TaggingPipeline, TaggingPipelineConfig};
use crate::llm::title::TitleGenerator;

// Import the modularized components
mod panels;
use super::conversation;
pub mod events;
mod tool_formatting;
mod state;
mod rendering;
mod initialization;

// Re-export types and functions from modules
pub use panels::*;
pub use conversation::*;
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
    config: Arc<Mutex<SagittaCodeConfig>>,
    app_core_config: Arc<AppConfig>,
    
    // Workspace Manager
    pub workspace_manager: Arc<Mutex<WorkspaceManagerImpl>>,

    // Conversation service for cluster management
    conversation_service: Option<Arc<ConversationService>>,
    
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

        // Initialize Workspace Manager
        let workspace_storage_path = sagitta_code_config.workspaces.storage_path
            .clone()
            .unwrap_or_else(|| {
                // Default path if not set in config, e.g., in user's data directory
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("sagitta")
                    .join("workspaces")
            });

        let workspace_manager = Arc::new(Mutex::new(WorkspaceManagerImpl::new(workspace_storage_path)));

        Self {
            agent: None,
            repo_panel: RepoPanel::new(repo_manager.clone()),
            chat_manager: Arc::new(StreamingChatManager::new()),
            settings_panel,
            conversation_sidebar: ConversationSidebar::with_default_config(),
            config: Arc::new(Mutex::new(sagitta_code_config)),
            app_core_config: app_core_config_arc,
            workspace_manager,
            
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
            
            // Conversation service for cluster management
            conversation_service: None,
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

    /// Backwards compatibility for tests that still use handle_llm_chunk
    pub fn handle_llm_chunk(&mut self, content: String, is_final: bool, ctx: &Context) {
        // This is a temporary measure to get tests to pass.
        // The tests should be refactored to use handle_agent_event.
        use crate::agent::events::AgentEvent;
        self.handle_agent_event(AgentEvent::LlmChunk { content, is_final }, ctx);
    }

    /// Render the application UI
    pub fn render(&mut self, ctx: &Context) {
        rendering::render(self, ctx);
    }

    /// Initialize application state, including loading configurations and setting up the agent
    pub async fn initialize(&mut self) -> Result<()> {
        let mut workspace_manager = self.workspace_manager.lock().await;
        if let Err(e) = workspace_manager.load_workspaces().await {
            log::error!("Failed to load workspaces: {}", e);
            // Decide if you want to bail out or continue with an empty workspace list
        }
        self.state.workspaces = workspace_manager.list_workspaces().await?;
        drop(workspace_manager);
        initialization::initialize(self).await
    }

    /// Initialize conversation service with clustering support
    pub async fn initialize_conversation_service(&mut self) -> Result<()> {
        // Create persistence layer using the same path logic as initialization
        let config_guard = self.config.lock().await;
        let storage_path = if let Some(path) = &config_guard.conversation.storage_path {
            path.clone()
        } else {
            initialization::get_default_conversation_storage_path()
        };
        drop(config_guard);
        
        let persistence = Box::new(DiskConversationPersistence::new(storage_path.clone()).await?);
        
        // Create search engine
        let search_engine = Box::new(TextConversationSearchEngine::new());
        
        // Create conversation manager
        let conversation_manager = Arc::new(ConversationManagerImpl::new(persistence, search_engine).await?);
        
        // Try to create and add tagging pipeline and title generator
        let conversation_manager_with_features = {
            // Create a new manager instance for adding features
            let mut manager_impl = ConversationManagerImpl::new(
                Box::new(DiskConversationPersistence::new(storage_path.clone()).await?),
                Box::new(TextConversationSearchEngine::new())
            ).await?;
            
            // Try to add tagging pipeline
            if let Ok(tagging_pipeline) = self.try_create_tagging_pipeline(conversation_manager.clone()).await {
                log::info!("Tagging pipeline initialized for conversation service");
                manager_impl = manager_impl.with_tagging_pipeline(Arc::new(tagging_pipeline));
            } else {
                log::warn!("Failed to initialize tagging pipeline - auto-tagging will be disabled");
            }
            
            // Try to add title generator
            if let Ok(title_generator) = self.try_create_title_generator().await {
                log::info!("Title generator initialized for conversation service");
                manager_impl = manager_impl.with_title_generator(Arc::new(title_generator));
            } else {
                log::warn!("Failed to initialize title generator - auto-titling will be disabled");
            }
            
            Arc::new(manager_impl) as Arc<dyn ConversationManager>
        };
        
        // Try to create clustering manager (optional, requires Qdrant)
        let clustering_manager = match self.try_create_clustering_manager().await {
            Ok(manager) => Some(manager),
            Err(e) => {
                log::warn!("Failed to initialize clustering manager: {}. Clustering features will be disabled.", e);
                None
            }
        };
        
        // Create analytics manager
        let analytics_manager = ConversationAnalyticsManager::new(AnalyticsConfig::default());
        
        // Create conversation service
        let service = ConversationService::new(
            conversation_manager_with_features,
            clustering_manager,
            analytics_manager,
        );
        
        self.conversation_service = Some(Arc::new(service));
        
        // Initial refresh of conversation data
        self.refresh_conversation_clusters().await?;
        
        Ok(())
    }
    
    /// Try to create a tagging pipeline (may fail if dependencies are not available)
    async fn try_create_tagging_pipeline(&self, conversation_manager: Arc<dyn ConversationManager>) -> Result<TaggingPipeline> {
        use crate::agent::conversation::tagging::{TaggingPipeline, TaggingPipelineConfig};
        
        // Create a basic tagging pipeline with default configuration
        let config = TaggingPipelineConfig {
            auto_apply_enabled: true,
            auto_apply_threshold: 0.7,
            max_tags_per_conversation: 10,
            tag_on_creation: false, // Don't tag empty conversations
            tag_on_update: true,
            min_messages_for_tagging: 2, // Start tagging after 2 messages
            preserve_manual_tags: true,
        };
        
        // Create a basic pipeline without embedding-based suggestions for now
        let pipeline = TaggingPipeline::new(config, conversation_manager);
        
        Ok(pipeline)
    }
    
    /// Try to create clustering manager (may fail if Qdrant is not available)
    async fn try_create_clustering_manager(&self) -> Result<ConversationClusteringManager> {
        use qdrant_client::Qdrant;
        use sagitta_search::EmbeddingPool;
        
        // Try to connect to Qdrant using the configured URL
        let qdrant_url = std::env::var("QDRANT_URL").unwrap_or_else(|_| {
            self.app_core_config.qdrant_url.clone()
        });
        let qdrant_client = Arc::new(Qdrant::from_url(&qdrant_url).build()?);
        
        // Create embedding pool
        let embedding_config = sagitta_search::app_config_to_embedding_config(&self.app_core_config);
        let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config)?;
        
        // Create clustering manager
        ConversationClusteringManager::with_default_config(
            qdrant_client,
            embedding_pool,
            "conversation_clusters".to_string(),
        ).await
    }
    
    /// Refresh conversation clusters and update sidebar
    pub async fn refresh_conversation_clusters(&mut self) -> Result<()> {
        if let Some(service) = &self.conversation_service {
            // Refresh the service data
            service.refresh().await?;
            
            // Get updated clusters
            let clusters = service.get_clusters().await?;
            
            // Update sidebar with new cluster data
            self.conversation_sidebar.clusters = clusters;
            
            log::info!("Updated conversation sidebar with {} clusters", self.conversation_sidebar.clusters.len());

            // After refresh, send an event to update conversation list in AppState
            let convos = service.list_conversations().await?;
            self.app_event_sender.send(AppEvent::RefreshConversationList)?;
        }
        
        Ok(())
    }
    
    /// Get conversation service for external use
    pub fn get_conversation_service(&self) -> Option<Arc<ConversationService>> {
        self.conversation_service.clone()
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

    /// Try to create a title generator (may fail if dependencies are not available)
    async fn try_create_title_generator(&self) -> Result<TitleGenerator> {
        use crate::llm::title::{TitleGenerator, TitleGeneratorConfig};
        
        // Create a basic title generator with default configuration
        let config = TitleGeneratorConfig {
            max_title_length: 50,
            min_messages_for_generation: 2,
            use_embeddings: false, // Disable embeddings for now
            fallback_prefix: "Conversation".to_string(),
        };
        
        // Create a basic title generator without LLM for now (rule-based only)
        let generator = TitleGenerator::new(config);
        
        Ok(generator)
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

