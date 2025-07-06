// Main application UI - modularized version

use std::sync::Arc;
use anyhow::Result;
use egui::Context;
use tokio::sync::{Mutex, mpsc, broadcast};
use uuid;
use std::ops::{Deref, DerefMut};

use super::repository::manager::RepositoryManager;
use super::repository::RepoPanel;
use super::repository::git_controls::GitControls;
use crate::services::{SyncOrchestrator, FileWatcherService, AutoCommitter, CommitMessageGenerator};
use crate::services::file_watcher::FileWatcherConfig as ServiceFileWatcherConfig;
use super::settings::SettingsPanel;
// Removed - ConversationSidebar is re-exported from conversation module
use super::claude_md_modal::ClaudeMdModal;
use crate::agent::Agent;
use crate::agent::message::types::{AgentMessage, ToolCall};
use crate::agent::state::types::{AgentState, AgentStateInfo};
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

use crate::agent::conversation::tagging::TaggingPipeline;
use crate::llm::title::TitleGenerator;

// Import the modularized components
mod panels;
use super::conversation;
pub mod events;
pub mod tool_formatting;
pub mod state;
pub mod rendering;
mod initialization;
mod conversation_title_updater;

#[cfg(test)]
mod tests;

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
    pub git_controls: GitControls,
    pub chat_manager: Arc<StreamingChatManager>,
    pub settings_panel: SettingsPanel,
    conversation_sidebar: ConversationSidebar,
    claude_md_modal: ClaudeMdModal,
    config: Arc<Mutex<SagittaCodeConfig>>,
    app_core_config: Arc<AppConfig>,
    


    // Conversation service for cluster management
    conversation_service: Option<Arc<ConversationService>>,
    
    // Title updater for auto-generating conversation titles
    title_updater: Option<Arc<conversation_title_updater::ConversationTitleUpdater>>,
    
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
    
    // Working directory management
    working_dir_manager: Option<Arc<crate::tools::WorkingDirectoryManager>>,
    
    // Auto-sync services
    sync_orchestrator: Option<Arc<SyncOrchestrator>>,
    file_watcher: Option<Arc<FileWatcherService>>,
    auto_committer: Option<Arc<AutoCommitter>>,
}

impl SagittaCodeApp {
    /// Create a new Sagitta Code App
    pub fn new(
        repo_manager: Arc<Mutex<RepositoryManager>>,
        sagitta_code_config: SagittaCodeConfig,
        app_core_config: AppConfig
    ) -> Self {
        let sagitta_code_config_arc = Arc::new(Mutex::new(sagitta_code_config.clone()));
        let app_core_config_arc = Arc::new(app_core_config.clone());

        // Create settings panel
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



        // Create panel manager
        let mut panels = PanelManager::new();
        
        // Set the current model from config
        panels.set_current_model(sagitta_code_config.claude_code.model.clone());

        Self {
            agent: None,
            repo_panel: RepoPanel::new(
                repo_manager.clone(),
                Arc::new(Mutex::new(sagitta_code_config.clone())),
                None, // Agent will be set later during initialization
            ),
            git_controls: GitControls::new(repo_manager.clone()),
            chat_manager: Arc::new(StreamingChatManager::new()),
            settings_panel,
            conversation_sidebar: ConversationSidebar::with_default_config(),
            claude_md_modal: ClaudeMdModal::new(sagitta_code_config_arc.clone()),
            config: sagitta_code_config_arc.clone(),
            app_core_config: app_core_config_arc,
            
            // Initialize state management with theme from config
            state: initial_state,
            
            // Initialize panel management with model manager
            panels,
            
            // Event handling
            agent_event_receiver: None,
            conversation_event_sender: Some(conversation_sender),
            conversation_event_receiver: Some(conversation_receiver),
            app_event_sender,
            app_event_receiver: Some(app_event_receiver),
            
            // Tool result formatting
            tool_formatter: ToolResultFormatter::new(),
            working_dir_manager: None, // Will be set during initialization
            
            // Conversation service for cluster management
            conversation_service: None,
            
            // Title updater - will be initialized later with conversation service
            title_updater: None,
            
            // Auto-sync services - will be initialized later
            sync_orchestrator: None,
            file_watcher: None,
            auto_committer: None,
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
        self.handle_agent_event(AgentEvent::LlmChunk { content, is_final, is_thinking: false }, ctx);
    }

    /// Render the application UI
    pub fn render(&mut self, ctx: &Context) {
        rendering::render(self, ctx);
    }

    /// Initialize application state, including loading configurations and setting up the agent
    pub async fn initialize(&mut self) -> Result<()> {
        initialization::initialize(self).await?;
        
        // Initialize auto-sync services after main initialization
        self.initialize_auto_sync_services().await?;
        
        Ok(())
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
        let conversation_manager = ConversationManagerImpl::new(persistence, search_engine).await?;
        let conversation_manager_arc = Arc::new(conversation_manager) as Arc<dyn ConversationManager>;
        
        // Skip advanced features for Phase 1 to focus on the main performance gain
        log::info!("Phase 1 optimization: Skipping tagging pipeline and title generator to focus on eliminating duplicate manager creation");
        
        // Try to create clustering manager (optional, requires Qdrant) - this will be optimized in later phases
        let clustering_manager = match self.try_create_clustering_manager().await {
            Ok(manager) => Some(manager),
            Err(e) => {
                log::warn!("Failed to initialize clustering manager: {e}. Clustering features will be disabled.");
                None
            }
        };
        
        // Create analytics manager
        let analytics_manager = ConversationAnalyticsManager::new(AnalyticsConfig::default());
        
        // Create conversation service
        let service = ConversationService::new(
            conversation_manager_arc,
            clustering_manager,
            analytics_manager,
        );
        
        let service_arc = Arc::new(service);
        self.conversation_service = Some(service_arc.clone());
        
        // Initialize title updater with the conversation service
        // Create fast model provider if enabled
        let fast_model_provider = {
            let config_guard = self.config.lock().await;
            if config_guard.conversation.enable_fast_model {
                let mut provider = crate::llm::fast_model::FastModelProvider::new(config_guard.clone());
                match provider.initialize().await {
                    Ok(_) => {
                        log::info!("Fast model provider initialized for title updater");
                        Some(Arc::new(provider) as Arc<dyn crate::llm::fast_model::FastModelOperations>)
                    }
                    Err(e) => {
                        log::warn!("Failed to initialize fast model provider: {e}");
                        None
                    }
                }
            } else {
                None
            }
        };
        
        let mut title_updater = conversation_title_updater::ConversationTitleUpdater::new(
            service_arc,
            None, // No LLM client needed, we use fast model
        );
        
        if let Some(provider) = fast_model_provider {
            title_updater = title_updater.with_fast_model_provider(provider);
        }
        
        self.title_updater = Some(Arc::new(title_updater));
        
        // Initial refresh of conversation data
        self.refresh_conversation_clusters().await?;
        
        Ok(())
    }

    /// Initialize conversation service with shared instances from initialization (Phase 1 optimization)
    pub async fn initialize_conversation_service_with_shared_instances(
        &mut self,
        _shared_qdrant_client: Option<Arc<qdrant_client::Qdrant>>,
        _shared_embedding_pool: Option<Arc<sagitta_search::EmbeddingPool>>,
    ) -> Result<()> {
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
        
        // Create conversation manager (single creation - Phase 1 optimization)
        let conversation_manager = ConversationManagerImpl::new(persistence, search_engine).await?;
        let conversation_manager_arc = Arc::new(conversation_manager) as Arc<dyn ConversationManager>;
        
        // Skip advanced features for Phase 1 to eliminate complexity and focus on the main performance gain
        log::info!("Phase 1 optimization: Skipping tagging pipeline and title generator to focus on eliminating duplicate manager creation");
        
        // For Phase 1, we'll skip the shared clustering optimization and use the existing method
        // The main gain is from eliminating the duplicate ConversationManagerImpl creation
        // Future phases will optimize clustering with shared instances
        let clustering_manager = match self.try_create_clustering_manager().await {
            Ok(manager) => Some(manager),
            Err(e) => {
                log::warn!("Failed to initialize clustering manager: {e}. Clustering features will be disabled.");
                None
            }
        };
        
        // Create analytics manager
        let analytics_manager = ConversationAnalyticsManager::new(AnalyticsConfig::default());
        
        // Create conversation service
        let service = ConversationService::new(
            conversation_manager_arc,
            clustering_manager,
            analytics_manager,
        );
        
        let service_arc = Arc::new(service);
        self.conversation_service = Some(service_arc.clone());
        
        // Initialize title updater with the conversation service
        // Create fast model provider if enabled
        let fast_model_provider = {
            let config_guard = self.config.lock().await;
            if config_guard.conversation.enable_fast_model {
                let mut provider = crate::llm::fast_model::FastModelProvider::new(config_guard.clone());
                match provider.initialize().await {
                    Ok(_) => {
                        log::info!("Fast model provider initialized for title updater");
                        Some(Arc::new(provider) as Arc<dyn crate::llm::fast_model::FastModelOperations>)
                    }
                    Err(e) => {
                        log::warn!("Failed to initialize fast model provider: {e}");
                        None
                    }
                }
            } else {
                None
            }
        };
        
        let mut title_updater = conversation_title_updater::ConversationTitleUpdater::new(
            service_arc,
            None, // No LLM client needed, we use fast model
        );
        
        if let Some(provider) = fast_model_provider {
            title_updater = title_updater.with_fast_model_provider(provider);
        }
        
        self.title_updater = Some(Arc::new(title_updater));
        
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
        use crate::agent::conversation::clustering::ClusteringConfig;
        
        // Try to connect to Qdrant using the configured URL
        let qdrant_url = std::env::var("QDRANT_URL").unwrap_or_else(|_| {
            self.app_core_config.qdrant_url.clone()
        });
        let qdrant_client = Arc::new(Qdrant::from_url(&qdrant_url).build()?);
        
        // Create embedding pool
        let embedding_config = sagitta_search::app_config_to_embedding_config(&self.app_core_config);
        let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config)?;
        
        // Phase 3: Create optimized clustering configuration
        let clustering_config = ClusteringConfig {
            similarity_threshold: 0.7,
            max_cluster_size: 20,
            min_cluster_size: 2,
            use_temporal_proximity: true,
            max_temporal_distance_hours: 24 * 7, // 1 week
            smart_clustering_threshold: 10, // Phase 3: Only cluster if >=10 conversations
            enable_embedding_cache: true,   // Phase 3: Enable embedding caching
            use_local_similarity: true,     // Phase 3: Use local similarity computation
            async_clustering: true,         // Phase 3: Enable async clustering
            embedding_cache_size: 100,      // Phase 3: Cache size for embeddings
        };
        
        // Create clustering manager with Phase 3 optimizations
        ConversationClusteringManager::new(
            qdrant_client,
            embedding_pool,
            "conversation_clusters".to_string(),
            clustering_config,
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
            let _convos = service.list_conversations().await?;
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
        self.agent.as_ref().map(|agent_arc| agent_arc.get_state_manager_state_info_arc())
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

    /// Handle repository list update event
    pub fn handle_repository_list_update(&mut self, repo_list: Vec<String>) {
        log::debug!("Updating available repositories list with {} repositories", repo_list.len());
        self.state.available_repositories = repo_list;
    }

    /// Open the CLAUDE.md modal
    pub fn open_claude_md_modal(&mut self) {
        // Open synchronously using the sync method
        self.claude_md_modal.open_sync();
    }
    
    /// Trigger a manual refresh of the repository list
    pub fn trigger_repository_list_refresh(&mut self) {
        log::debug!("Triggering manual repository list refresh");
        
        let repo_manager = self.repo_panel.get_repo_manager();
        let app_event_sender = self.app_event_sender.clone();
        
        tokio::spawn(async move {
            log::debug!("Starting manual repository list refresh task");
            match repo_manager.lock().await.list_repositories().await {
                Ok(repositories) => {
                    let repo_names: Vec<String> = repositories
                        .iter()
                        .map(|repo| repo.name.clone())
                        .collect();
                    
                    log::info!("Manual refresh completed: {repo_names:?}");
                    
                    // Send the repository list update event
                    if let Err(e) = app_event_sender.send(crate::gui::app::events::AppEvent::RepositoryListUpdated(repo_names)) {
                        log::error!("Failed to send repository list update event: {e}");
                    } else {
                        log::debug!("Successfully sent repository list update event from manual refresh");
                    }
                },
                Err(e) => {
                    log::error!("Failed to manually refresh repository list: {e}");
                }
            }
        });
    }
    
    /// Initialize auto-sync services
    pub async fn initialize_auto_sync_services(&mut self) -> Result<()> {
        let config_guard = self.config.lock().await;
        
        if !config_guard.auto_sync.enabled {
            log::info!("Auto-sync is disabled in configuration");
            return Ok(());
        }
        
        log::info!("Initializing auto-sync services...");
        
        // Get repository manager
        let repo_manager = self.repo_panel.get_repo_manager();
        
        // Create sync orchestrator
        let mut sync_orchestrator = SyncOrchestrator::new(
            config_guard.auto_sync.clone(),
            repo_manager.clone(),
        );
        sync_orchestrator.set_app_event_sender(self.app_event_sender.clone());
        let sync_orchestrator = Arc::new(sync_orchestrator);
        self.sync_orchestrator = Some(sync_orchestrator.clone());
        self.git_controls.set_sync_orchestrator(sync_orchestrator.clone());
        self.repo_panel.set_sync_orchestrator(sync_orchestrator.clone());
        
        // Start git controls command handler
        let command_rx = self.git_controls.start_command_handler();
        let mut git_controls_clone = GitControls::new(repo_manager.clone());
        git_controls_clone.set_sync_orchestrator(sync_orchestrator.clone());
        
        tokio::spawn(async move {
            git_controls_clone.handle_commands(command_rx).await;
        });
        
        // Only initialize file watcher and auto-committer if their respective features are enabled
        if config_guard.auto_sync.file_watcher.enabled && config_guard.auto_sync.auto_commit.enabled {
            // Create commit message generator
            let mut fast_model_provider = crate::llm::fast_model::FastModelProvider::new(config_guard.clone());
            if let Err(e) = fast_model_provider.initialize().await {
                log::warn!("Failed to initialize fast model provider for auto-commit: {e}");
            }
            let commit_generator = CommitMessageGenerator::new(
                fast_model_provider,
                config_guard.auto_sync.auto_commit.clone(),
            );
            
            // Create auto-committer
            let mut auto_committer = AutoCommitter::new(
                config_guard.auto_sync.auto_commit.clone(),
                commit_generator,
            );
            let commit_rx = auto_committer.start();
            let auto_committer = Arc::new(auto_committer);
            self.auto_committer = Some(auto_committer.clone());
            
            // Create file watcher with converted config
            let service_file_watcher_config = ServiceFileWatcherConfig {
                enabled: config_guard.auto_sync.file_watcher.enabled,
                debounce_ms: config_guard.auto_sync.file_watcher.debounce_ms,
                exclude_patterns: config_guard.auto_sync.file_watcher.exclude_patterns.clone(),
                max_buffer_size: 1000, // Default value
            };
            let mut file_watcher = FileWatcherService::new(service_file_watcher_config);
            let change_rx = file_watcher.start().await?;
            let file_watcher = Arc::new(file_watcher);
            self.file_watcher = Some(file_watcher.clone());
            
            // Set the file watcher on the sync orchestrator
            sync_orchestrator.set_file_watcher(file_watcher.clone()).await;
            
            // Start auto-commit handler
            let auto_committer_handler = auto_committer.clone();
            tokio::spawn(async move {
                auto_committer_handler.handle_file_changes(change_rx).await;
            });
            
            // Start sync handler for commits
            let sync_orchestrator_handler = sync_orchestrator.clone();
            let mut commit_rx = commit_rx;
            tokio::spawn(async move {
                while let Some(commit_result) = commit_rx.recv().await {
                    if let Err(e) = sync_orchestrator_handler.handle_commit_result(commit_result).await {
                        log::error!("Failed to handle commit result: {}", e);
                    }
                }
            });
            
            log::info!("Auto-sync services initialized and started");
            
            // Add existing repositories to file watcher and sync them in the background
            let repo_manager_clone = repo_manager.clone();
            let sync_orchestrator_clone = sync_orchestrator.clone();
            tokio::spawn(async move {
                log::info!("Starting background repository initialization");
                
                let repo_manager_guard = repo_manager_clone.lock().await;
                if let Ok(repositories) = repo_manager_guard.list_repositories().await {
                    log::info!("Adding {} existing repositories to file watcher", repositories.len());
                    
                    for repo in repositories {
                        let repo_path = std::path::PathBuf::from(&repo.local_path);
                        if let Err(e) = sync_orchestrator_clone.add_repository(&repo_path).await {
                            log::error!("Failed to add existing repository {} to file watcher: {}", repo.name, e);
                        } else {
                            log::info!("Added existing repository {} to file watcher and sync queue", repo.name);
                        }
                    }
                    
                    // Also ensure all out-of-sync repositories get synced
                    if let Err(e) = sync_orchestrator_clone.sync_out_of_sync_repositories().await {
                        log::error!("Failed to queue out-of-sync repositories: {}", e);
                    }
                }
                
                log::info!("Background repository initialization complete");
            });
        } else {
            log::info!("File watcher and/or auto-commit disabled, skipping initialization");
        }
        
        Ok(())
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

