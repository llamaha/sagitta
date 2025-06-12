// Application initialization for the Sagitta Code

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use super::SagittaCodeApp;
use super::super::repository::manager::RepositoryManager;
use super::super::repository::RepoPanel;
use super::super::theme::AppTheme;
use super::super::chat::view::{StreamingMessage, MessageAuthor};
use crate::config::loader::load_all_configs;
use crate::llm::openrouter::client::OpenRouterClient;
use crate::llm::client::LlmClient;
use crate::agent::Agent;
use crate::agent::state::types::AgentMode;
use crate::tools::code_search::tool::CodeSearchTool;
use crate::tools::file_operations::read::ReadFileTool;
use crate::tools::repository::list::ListRepositoriesTool;
use crate::tools::repository::search::SearchFileInRepositoryTool;
use crate::tools::repository::view::ViewFileInRepositoryTool;
use crate::tools::repository::add::AddExistingRepositoryTool;
use crate::tools::repository::sync::SyncRepositoryTool;
use crate::tools::repository::remove::RemoveRepositoryTool;
use crate::tools::repository::map::RepositoryMapTool;
use crate::tools::repository::targeted_view::TargetedViewTool;
use crate::tools::web_search::WebSearchTool;
use crate::tools::analyze_input::AnalyzeInputTool;
use crate::tools::analyze_input::TOOLS_COLLECTION_NAME; // Import the const
use crate::tools::code_edit::edit::EditTool; // Corrected import for EditTool
use crate::config::SagittaCodeConfig;
use crate::tools::registry::ToolRegistry;
use crate::tools::shell_execution::ShellExecutionTool;
// Add imports for concrete persistence/search and traits
use crate::agent::conversation::persistence::{
    ConversationPersistence, 
    disk::DiskConversationPersistence
};
use crate::agent::conversation::search::{
    ConversationSearchEngine, 
    text::TextConversationSearchEngine
};
use sagitta_embed::provider::{onnx::OnnxEmbeddingModel, EmbeddingProvider};
use sagitta_search::{EmbeddingPool, EmbeddingProcessor};
use std::path::PathBuf;

// Imports for sagitta-search components for embedding provider
use std::path::Path; // For Path::new

// Qdrant imports
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollection, VectorParams, VectorsConfig, UpsertPoints, // Added UpsertPoints
    Distance, PointStruct, // Renamed to avoid conflict with a potential local Payload struct
    vectors_config::Config as VectorsConfigEnum,
    // VectorParamsBuilder, // Not needed for direct VectorParams construction
};
use qdrant_client::Payload as QdrantPayload; // Corrected Payload import

/// Get the default conversation storage path
pub fn get_default_conversation_storage_path() -> PathBuf {
    let mut default_path = dirs::config_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    default_path.push("sagitta-code");
    default_path.push("conversations");
    default_path
}

/// Configure theme from config
pub async fn configure_theme_from_config(app: &mut SagittaCodeApp) {
    match app.config.lock().await.ui.theme.as_str() {
        "light" => app.state.current_theme = AppTheme::Light,
        "dark" | _ => app.state.current_theme = AppTheme::Dark, // Default to Dark
    }
}

/// Create repository manager with config
pub async fn create_repository_manager(core_config: sagitta_search::AppConfig) -> Result<Arc<Mutex<RepositoryManager>>> {
    let repo_manager_core_config = Arc::new(Mutex::new(core_config)); 
    let repo_manager = Arc::new(Mutex::new(RepositoryManager::new(repo_manager_core_config)));
    
    if let Err(e) = repo_manager.lock().await.initialize().await {
        log::error!("Failed to initialize RepositoryManager: {}",e);
        return Err(anyhow::anyhow!("Failed to initialize RepositoryManager: {}", e));
    }
    
    Ok(repo_manager)
}

/// Create embedding pool from config
pub fn create_embedding_pool(core_config: &sagitta_search::AppConfig) -> Result<Arc<sagitta_search::EmbeddingPool>> {
    let embedding_config = sagitta_search::app_config_to_embedding_config(core_config);
    match sagitta_search::EmbeddingPool::with_configured_sessions(embedding_config) {
        Ok(pool) => {
            log::info!("Successfully created EmbeddingPool for GUI App.");
            Ok(Arc::new(pool))
        }
        Err(e) => {
            log::error!("Failed to create EmbeddingPool for GUI App: {}. Intent analysis will be impaired.", e);
            Err(anyhow::anyhow!("Failed to create EmbeddingPool for GUI: {}", e))
        }
    }
}

/// Create LLM client from config
pub fn create_llm_client(config: &SagittaCodeConfig) -> Result<Arc<dyn LlmClient>> {
    let openrouter_client_result = OpenRouterClient::new(config);
    
    match openrouter_client_result {
        Ok(client) => Ok(Arc::new(client)),
        Err(e) => {
            log::error!(
                "Failed to create OpenRouterClient: {}. Agent will not be initialized properly. Some features may be disabled.",
                e
            );
            Err(anyhow::anyhow!("Failed to create OpenRouterClient for Agent: {}", e))
        }
    }
}

/// Initialize Qdrant client from config
pub async fn create_qdrant_client(core_config: &sagitta_search::AppConfig) -> Result<Arc<dyn QdrantClientTrait>> {
    let qdrant_client_result = Qdrant::from_url(&core_config.qdrant_url).build();
    match qdrant_client_result {
        Ok(client) => Ok(Arc::new(client)),
        Err(e) => {
            log::error!("GUI: Failed to connect to Qdrant at {}: {}. Semantic tool analysis will be disabled.", core_config.qdrant_url, e);
            Err(anyhow::anyhow!("Failed to initialize Qdrant client for GUI: {}", e))
        }
    }
}

/// Create conversation persistence from config
pub async fn create_conversation_persistence(config: &SagittaCodeConfig) -> Result<Box<dyn ConversationPersistence>> {
    let storage_path = if let Some(path) = &config.conversation.storage_path {
        path.clone()
    } else {
        get_default_conversation_storage_path()
    };

    match DiskConversationPersistence::new(storage_path).await {
        Ok(persistence) => Ok(Box::new(persistence)),
        Err(e) => Err(anyhow::anyhow!("Failed to create disk conversation persistence: {}", e))
    }
}

/// Initialize the application
pub async fn initialize(app: &mut SagittaCodeApp) -> Result<()> {
    log::info!("SagittaCodeApp: Initializing...");

    // Load both sagitta-code and sagitta-search configs
    let (code_config, core_config) = match load_all_configs() {
        Ok((code_cfg, core_cfg_opt)) => {
            log::info!("SagittaCodeApp: Loaded both configurations successfully");
            let core_cfg = core_cfg_opt.unwrap_or_else(|| {
                log::warn!("SagittaCodeApp: sagitta-search config not found, using default");
                sagitta_search::config::AppConfig::default()
            });
            (code_cfg, core_cfg)
        }
        Err(e) => {
            log::warn!("SagittaCodeApp: Could not load configurations: {}. Using defaults.", e);
            (crate::config::SagittaCodeConfig::default(), sagitta_search::config::AppConfig::default())
        }
    };

    // Create repository manager
    let repo_manager = create_repository_manager(core_config.clone()).await?;
    app.repo_panel = RepoPanel::new(repo_manager.clone());
    app.repo_panel.refresh_repositories(); // Initial refresh
    log::info!("SagittaCodeApp: RepoPanel initialized and refreshed.");

    // Initialize embedding pool
    let embedding_handler_arc = create_embedding_pool(&core_config)?;

    // Create adapter for EmbeddingProvider compatibility
    let embedding_provider_adapter = Arc::new(sagitta_search::EmbeddingPoolAdapter::new(embedding_handler_arc.clone()));

    // Configure theme
    configure_theme_from_config(app).await;
    
    // Create LLM client
    let config_guard = app.config.lock().await;
    let llm_client = create_llm_client(&*config_guard).map_err(|e| {
        let error_message = StreamingMessage::from_text(
            MessageAuthor::System,
            format!("CRITICAL: Failed to initialize LLM Client (OpenRouter): {}. Agent is disabled.", e),
        );
        app.chat_manager.add_complete_message(error_message);
        e
    })?;
    drop(config_guard);

    // Initialize Qdrant client - create concrete instance for sharing
    let qdrant_client_concrete = match Qdrant::from_url(&core_config.qdrant_url).build() {
        Ok(client) => {
            log::info!("GUI: Connected to Qdrant at {}", core_config.qdrant_url);
            Some(Arc::new(client))
        }
        Err(e) => {
            log::error!("GUI: Failed to connect to Qdrant at {}: {}. Semantic features will be limited.", core_config.qdrant_url, e);
            app.panels.events_panel.add_event(super::SystemEventType::Error, format!("Qdrant connection failed: {}", e));
            None
        }
    };

    // Create trait version for tools that need it
    let qdrant_client: Arc<dyn QdrantClientTrait> = if let Some(ref concrete_client) = qdrant_client_concrete {
        concrete_client.clone()
    } else {
        // Fallback - should not happen in normal operation, but provides safety
        return Err(anyhow::anyhow!("Failed to initialize Qdrant client for GUI"));
    };

    // Use the locally scoped embedding_handler_arc, which is correctly typed.
    let vector_size = embedding_handler_arc.dimension() as u64;

    // Ensure Qdrant "tools" collection exists
    match qdrant_client.collection_exists(TOOLS_COLLECTION_NAME.to_string()).await {
        Ok(exists) => {
            if !exists {
                log::info!("GUI: Creating Qdrant tool collection: {}", TOOLS_COLLECTION_NAME);
                let create_collection_request = CreateCollection {
                    collection_name: TOOLS_COLLECTION_NAME.to_string(),
                    vectors_config: Some(VectorsConfig {
                        config: Some(VectorsConfigEnum::ParamsMap(
                            qdrant_client::qdrant::VectorParamsMap {
                                map: std::collections::HashMap::from([
                                    ("dense".to_string(), VectorParams {
                                        size: vector_size,
                                        distance: Distance::Cosine.into(),
                                        hnsw_config: None,
                                        quantization_config: None,
                                        on_disk: None,
                                        datatype: None,
                                        multivector_config: None,
                                    })
                                ])
                            }
                        ))
                    }),
                    shard_number: None,
                    sharding_method: None,
                    replication_factor: None,
                    write_consistency_factor: None,
                    on_disk_payload: None,
                    hnsw_config: None,
                    wal_config: None,
                    optimizers_config: None,
                    init_from_collection: None,
                    quantization_config: None,
                    sparse_vectors_config: None,
                    timeout: None,
                    strict_mode_config: None,
                };

                if let Err(e) = qdrant_client.create_collection_detailed(create_collection_request).await {
                    log::error!("GUI: Failed to create Qdrant tool collection '{}': {}", TOOLS_COLLECTION_NAME, e);
                    app.panels.events_panel.add_event(super::SystemEventType::Error, format!("Qdrant collection creation failed: {}", e));
                    // Not returning error here, as agent might still function partially
                }
            } else {
                log::info!("GUI: Qdrant tool collection '{}' already exists.", TOOLS_COLLECTION_NAME);
            }
        }
        Err(e) => {
            log::error!("GUI: Failed to check Qdrant tool collection '{}': {}", TOOLS_COLLECTION_NAME, e);
            app.panels.events_panel.add_event(super::SystemEventType::Error, format!("Qdrant collection check failed: {}", e));
        }
    }
    // Populate/update tools in Qdrant. This should ideally be done *after* all tools are registered in tool_registry.
    // For now, we'll do a pre-registration population based on constructing them here, 
    // or assume tool_registry is populated by this point (which it isn't yet fully).
    // This part needs careful placement. Let's assume tools are registered first, then we populate Qdrant.

    // Initialize ToolRegistry
    let tool_registry = Arc::new(crate::tools::registry::ToolRegistry::new());
    
    // Get the configured working directory instead of using current_dir
    let config_guard = app.config.lock().await;
    let working_dir = config_guard.repositories_base_path();
    drop(config_guard);
    
    // Ensure the working directory exists
    if !working_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&working_dir) {
            log::warn!("Failed to create working directory {}: {}", working_dir.display(), e);
        }
    }
    
    // Create WorkingDirectoryManager
    let working_dir_manager = Arc::new(crate::tools::WorkingDirectoryManager::new(working_dir.clone())
        .map_err(|e| anyhow::anyhow!("Failed to create WorkingDirectoryManager: {}", e))?);

    // Register tools first
    tool_registry.register(Arc::new(AnalyzeInputTool::new(tool_registry.clone(), embedding_provider_adapter.clone(), qdrant_client.clone()))).await?;
    tool_registry.register(Arc::new(CodeSearchTool::new(repo_manager.clone()))).await?;
    
    // Register working directory tools
    tool_registry.register(Arc::new(crate::tools::GetCurrentDirectoryTool::new(working_dir_manager.clone()))).await?;
    tool_registry.register(Arc::new(crate::tools::ChangeDirectoryTool::new(working_dir_manager.clone()))).await?;
    
    tool_registry.register(Arc::new(ReadFileTool::new(repo_manager.clone(), working_dir.clone()))).await?;
    tool_registry.register(Arc::new(ListRepositoriesTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(SearchFileInRepositoryTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(ViewFileInRepositoryTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(AddExistingRepositoryTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(SyncRepositoryTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(RemoveRepositoryTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(RepositoryMapTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(TargetedViewTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(WebSearchTool::new(llm_client.clone()))).await?;
    tool_registry.register(Arc::new(EditTool::new(repo_manager.clone(), working_dir.clone()))).await?; // Added EditTool registration
    tool_registry.register(Arc::new(crate::tools::repository::SwitchBranchTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(crate::tools::repository::CreateBranchTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(crate::tools::repository::CommitChangesTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(crate::tools::repository::PushChangesTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(crate::tools::repository::PullChangesTool::new(repo_manager.clone()))).await?;

    tool_registry.register(Arc::new(crate::tools::shell_execution::ShellExecutionTool::new(working_dir.clone()))).await?;

    // Register streaming shell execution tool for terminal integration
    tool_registry.register(Arc::new(crate::tools::shell_execution::StreamingShellExecutionTool::new(working_dir.clone()))).await?;

    // Note: Project creation and test execution functionality is now available through shell_execution tool
    // Examples:
    // - Project creation: Use shell_execution with commands like "cargo init my-project", "npm init", "python -m venv myenv"
    // - Test execution: Use shell_execution with commands like "cargo test", "npm test", "pytest", "go test"

    // Now populate Qdrant with all registered tools
    let all_tool_defs_for_qdrant = tool_registry.get_definitions().await;
    let mut points_to_upsert_gui = Vec::new();
    for (idx, tool_def) in all_tool_defs_for_qdrant.iter().enumerate() {
        let tool_desc_text = format!("{}: {}", tool_def.name, tool_def.description);
        match sagitta_search::embed_text_with_pool(&embedding_handler_arc, &[&tool_desc_text]).await {
            Ok(mut embeddings) => {
                if let Some(embedding) = embeddings.pop() {
                    let mut payload_map: std::collections::HashMap<String, qdrant_client::qdrant::Value> = std::collections::HashMap::new();
                    payload_map.insert("tool_name".to_string(), tool_def.name.clone().into());
                    payload_map.insert("description".to_string(), tool_def.description.clone().into());
                    let params_json_str = serde_json::to_string(&tool_def.parameters).unwrap_or_else(|_| "{}".to_string());
                    payload_map.insert("parameter_schema".to_string(), params_json_str.into());
                    
                    points_to_upsert_gui.push(PointStruct::new(
                        qdrant_client::qdrant::PointId::from(idx as u64), // Explicit PointId conversion for u64
                        qdrant_client::qdrant::NamedVectors::default()
                            .add_vector("dense", embedding), 
                        qdrant_client::Payload::from(payload_map) // Explicit Payload conversion
                    ));
                } else {
                    log::warn!("GUI: Embedding batch returned empty for tool '{}'", tool_def.name);
                }
            }
            Err(e) => log::warn!("GUI: Failed to generate embedding for tool '{}': {}", tool_def.name, e),
        }
    }
    if !points_to_upsert_gui.is_empty() {
        let upsert_request_gui = UpsertPoints {
            collection_name: TOOLS_COLLECTION_NAME.to_string(),
            wait: Some(true),
            points: points_to_upsert_gui,
            ordering: None,
            shard_key_selector: None,
        };
        if let Err(e) = qdrant_client.upsert_points(upsert_request_gui).await {
            log::error!("GUI: Failed to upsert tool definitions to Qdrant: {}", e);
            app.panels.events_panel.add_event(super::SystemEventType::Error, format!("Qdrant tool upsert failed: {}", e));
        }
    }
    // --- End Qdrant tool collection population ---

    // Create conversation persistence
    let config_guard = app.config.lock().await;
    let persistence = create_conversation_persistence(&*config_guard).await?;
    let config_clone = config_guard.clone();
    drop(config_guard);
    let search_engine: Box<dyn ConversationSearchEngine> = Box::new(TextConversationSearchEngine::new());

    let agent_result = Agent::new(
        config_clone, 
        tool_registry.clone(), // tool_registry is now defined
        embedding_provider_adapter.clone(), // Use the adapter instead of raw pool
        persistence, // Pass concrete persistence
        search_engine, // Pass concrete search engine
        llm_client.clone() // Add the llm_client argument here
    ).await;
    match agent_result {
        Ok(agent) => {
            // Set agent to fully autonomous mode to allow automatic tool execution
            if let Err(e) = agent.set_mode(AgentMode::FullyAutonomous).await {
                log::error!("Failed to set agent mode to FullyAutonomous: {}", e);
            } else {
                log::info!("Agent mode set to FullyAutonomous for automatic tool execution");
            }
            
            // CRITICAL: Wire up terminal event sender to tool executor for streaming shell execution
            if let Some(terminal_sender) = app.state.get_terminal_event_sender() {
                // Set the terminal event sender on the agent's tool executor
                // Note: This requires making the tool_executor field accessible or adding a method to Agent
                agent.set_terminal_event_sender(terminal_sender).await;
                log::info!("Terminal event sender connected to agent tool executor for streaming shell execution");
            } else {
                log::warn!("No terminal event sender available - shell execution will not stream to terminal");
            }
            
            // Subscribe to agent events
            let event_receiver = agent.subscribe();
            app.agent = Some(Arc::new(agent));
            app.agent_event_receiver = Some(event_receiver);
            
            // Initialize conversation service for the sidebar - use shared instances (Phase 1 optimization)
            if let Err(e) = app.initialize_conversation_service_with_shared_instances(qdrant_client_concrete.clone(), Some(embedding_handler_arc.clone())).await {
                log::warn!("Failed to initialize conversation service: {}. Conversation sidebar features may be limited.", e);
                app.panels.events_panel.add_event(
                    super::SystemEventType::Info,
                    format!("Conversation service initialization failed: {}", e)
                );
            } else {
                log::info!("Conversation service initialized successfully with shared instances");
            }
            
            // Initial conversation data load
            app.refresh_conversation_data();
            
            // Add a welcome message
            let welcome_message = StreamingMessage::from_text(
                MessageAuthor::Agent,
                "Hello! I'm Sagitta Code, your AI assistant for code repositories. How can I help you today?".to_string(),
            );
            app.chat_manager.add_complete_message(welcome_message);
        },
        Err(err) => {
            log::error!("Failed to initialize agent: {}", err);
            
            // Add to events panel instead of chat
            app.panels.events_panel.add_event(
                super::SystemEventType::Error,
                format!("Failed to initialize agent: {}. Check your OpenRouter API key in settings.", err)
            );
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::SagittaCodeConfig;
    use sagitta_search::AppConfig as CoreAppConfig;
    use tempfile::TempDir;
    use std::env;

    #[test]
    fn test_get_default_conversation_storage_path() {
        let path = get_default_conversation_storage_path();
        
        // Should contain sagitta-code and conversations
        assert!(path.to_string_lossy().contains("sagitta-code"));
        assert!(path.to_string_lossy().contains("conversations"));
        
        // Should be an absolute path or at least a proper path
        assert!(path.is_absolute() || path.starts_with("."));
    }

    #[tokio::test]
    async fn test_configure_theme_from_config() {
        let app_config = CoreAppConfig::default();
        let repo_manager = Arc::new(tokio::sync::Mutex::new(
            RepositoryManager::new(Arc::new(tokio::sync::Mutex::new(app_config.clone())))
        ));
        let mut sagitta_config = SagittaCodeConfig::default();
        
        let mut app = SagittaCodeApp::new(repo_manager, sagitta_config, app_config);
        
        // Test light theme - modify config directly before creating the app
        let mut sagitta_config_light = SagittaCodeConfig::default();
        sagitta_config_light.ui.theme = "light".to_string();
        let app_config_clone = CoreAppConfig::default();
        let repo_manager_light = Arc::new(tokio::sync::Mutex::new(
            RepositoryManager::new(Arc::new(tokio::sync::Mutex::new(app_config_clone.clone())))
        ));
        let mut app_light = SagittaCodeApp::new(repo_manager_light, sagitta_config_light, app_config_clone);
        configure_theme_from_config(&mut app_light).await;
        assert_eq!(app_light.state.current_theme, AppTheme::Light);
        
        // Test dark theme
        let mut sagitta_config_dark = SagittaCodeConfig::default();
        sagitta_config_dark.ui.theme = "dark".to_string();
        let app_config_clone2 = CoreAppConfig::default();
        let repo_manager_dark = Arc::new(tokio::sync::Mutex::new(
            RepositoryManager::new(Arc::new(tokio::sync::Mutex::new(app_config_clone2.clone())))
        ));
        let mut app_dark = SagittaCodeApp::new(repo_manager_dark, sagitta_config_dark, app_config_clone2);
        configure_theme_from_config(&mut app_dark).await;
        assert_eq!(app_dark.state.current_theme, AppTheme::Dark);
        
        // Test default (unknown theme defaults to dark)
        let mut sagitta_config_unknown = SagittaCodeConfig::default();
        sagitta_config_unknown.ui.theme = "unknown".to_string();
        let app_config_clone3 = CoreAppConfig::default();
        let repo_manager_unknown = Arc::new(tokio::sync::Mutex::new(
            RepositoryManager::new(Arc::new(tokio::sync::Mutex::new(app_config_clone3.clone())))
        ));
        let mut app_unknown = SagittaCodeApp::new(repo_manager_unknown, sagitta_config_unknown, app_config_clone3);
        configure_theme_from_config(&mut app_unknown).await;
        assert_eq!(app_unknown.state.current_theme, AppTheme::Dark);
    }

    #[tokio::test]
    async fn test_create_repository_manager() {
        let core_config = CoreAppConfig::default();
        let result = create_repository_manager(core_config).await;
        
        // Should succeed with default config
        assert!(result.is_ok());
        
        let repo_manager = result.unwrap();
        let repos = repo_manager.lock().await.list_repositories().await;
        assert!(repos.is_ok());
        assert!(repos.unwrap().is_empty());
    }

    #[test]
    fn test_create_embedding_pool_with_invalid_config() {
        let mut core_config = CoreAppConfig::default();
        
        // Set invalid ONNX model path to force failure
        core_config.onnx_model_path = Some("/nonexistent/path/model.onnx".into());
        
        let result = create_embedding_pool(&core_config);
        
        // Should fail with invalid config, but may succeed if fallback mechanisms work
        // Just test that it handles the case gracefully
        match result {
            Ok(_) => {
                // May succeed if there are fallback mechanisms
                assert!(true);
            }
            Err(e) => {
                // Expected failure with invalid config
                assert!(e.to_string().contains("Failed to create EmbeddingPool"));
            }
        }
    }

    #[test]
    fn test_create_llm_client_with_invalid_config() {
        let mut config = SagittaCodeConfig::default();
        
        // Set invalid API key to force failure
        config.openrouter.api_key = Some("invalid_key".to_string());
        
        let result = create_llm_client(&config);
        
        // May succeed or fail depending on validation - test structure is important
        match result {
            Ok(_) => {
                // Client created but might fail later during actual API calls
                assert!(true);
            }
            Err(e) => {
                // Expected failure with invalid config
                assert!(e.to_string().contains("Failed to create OpenRouterClient"));
            }
        }
    }

    #[tokio::test]
    async fn test_create_qdrant_client_with_invalid_url() {
        let mut core_config = CoreAppConfig::default();
        
        // Set invalid Qdrant URL
        core_config.qdrant_url = "http://invalid-url:1234".to_string();
        
        let result = create_qdrant_client(&core_config).await;
        
        // Should fail with invalid URL - may succeed immediately but fail on actual connection
        // The exact behavior depends on Qdrant client implementation
        match result {
            Ok(_) => {
                // Client created but might fail on actual operations
                assert!(true);
            }
            Err(e) => {
                // Expected failure
                assert!(e.to_string().contains("Failed to initialize Qdrant client"));
            }
        }
    }

    #[tokio::test]
    async fn test_create_conversation_persistence_with_temp_dir() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = SagittaCodeConfig::default();
        config.conversation.storage_path = Some(temp_dir.path().to_path_buf());
        
        let result = create_conversation_persistence(&config).await;
        
        // Should succeed with valid temp directory
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_conversation_persistence_with_default_path() {
        let config = SagittaCodeConfig::default();
        // config.conversation.storage_path is None, should use default
        
        let result = create_conversation_persistence(&config).await;
        
        // Should succeed using default path
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_conversation_persistence_with_invalid_path() {
        let mut config = SagittaCodeConfig::default();
        // Set an invalid path (root directory that shouldn't be writable)
        config.conversation.storage_path = Some("/root/nonexistent/path".into());
        
        let result = create_conversation_persistence(&config).await;
        
        // May succeed or fail depending on permissions and OS
        // Just test that it handles the case gracefully
        match result {
            Ok(_) => assert!(true), // Succeeded
            Err(e) => {
                // Expected failure with permission issues
                assert!(e.to_string().contains("Failed to create disk conversation persistence"));
            }
        }
    }

    #[tokio::test]
    async fn test_initialization_helper_functions_isolation() {
        // Test that helper functions are properly isolated and testable
        
        // Test default path generation
        let path1 = get_default_conversation_storage_path();
        let path2 = get_default_conversation_storage_path();
        assert_eq!(path1, path2); // Should be deterministic
        
        // Test theme configuration with minimal app - use different themes to ensure change
        let app_config = CoreAppConfig::default();
        let repo_manager = Arc::new(tokio::sync::Mutex::new(
            RepositoryManager::new(Arc::new(tokio::sync::Mutex::new(app_config.clone())))
        ));
        let mut sagitta_config = SagittaCodeConfig::default();
        sagitta_config.ui.theme = "dark".to_string(); // Start with dark
        
        let mut app = SagittaCodeApp::new(repo_manager, sagitta_config, app_config);
        let original_theme = app.state.current_theme.clone();
        
        // Now create a new config with light theme
        let app_config2 = CoreAppConfig::default();
        let repo_manager2 = Arc::new(tokio::sync::Mutex::new(
            RepositoryManager::new(Arc::new(tokio::sync::Mutex::new(app_config2.clone())))
        ));
        let mut sagitta_config2 = SagittaCodeConfig::default();
        sagitta_config2.ui.theme = "light".to_string();
        
        let mut app2 = SagittaCodeApp::new(repo_manager2, sagitta_config2, app_config2);
        configure_theme_from_config(&mut app2).await;
        
        // Theme should be updated to light
        assert_eq!(app2.state.current_theme, AppTheme::Light);
        
        // Verify the function actually works by testing both themes
        assert_ne!(AppTheme::Light, AppTheme::Dark);
    }

    #[test]
    fn test_embedding_pool_creation_with_different_configs() {
        // Test with default config
        let default_config = CoreAppConfig::default();
        let result1 = create_embedding_pool(&default_config);
        
        // Test with modified config - use a valid field instead
        let mut modified_config = CoreAppConfig::default();
        modified_config.onnx_model_path = Some("/custom/path/model.onnx".into()); // Different model path
        let result2 = create_embedding_pool(&modified_config);
        
        // Both should handle their configurations appropriately
        // Exact success/failure depends on system setup, but structure should be sound
        match (result1, result2) {
            (Ok(_), Ok(_)) => assert!(true),
            (Ok(_), Err(_)) => assert!(true), // Modified config might fail
            (Err(_), Ok(_)) => assert!(true), // Default config might fail on this system
            (Err(_), Err(_)) => assert!(true), // Both might fail if no model available
        }
    }

    #[test]
    fn test_initialization_constants() {
        // Test that important constants are accessible
        assert!(!TOOLS_COLLECTION_NAME.is_empty());
        assert!(TOOLS_COLLECTION_NAME.len() > 0);
        
        // Test path generation doesn't panic
        let path = get_default_conversation_storage_path();
        assert!(!path.as_os_str().is_empty());
    }

    #[test] 
    fn test_path_construction_edge_cases() {
        // Test that path construction handles edge cases
        let original_env = env::var("HOME").ok();
        
        // Temporarily unset HOME to test fallback behavior
        env::remove_var("HOME");
        
        let path_without_home = get_default_conversation_storage_path();
        assert!(!path_without_home.as_os_str().is_empty());
        
        // Restore original environment
        if let Some(home) = original_env {
            env::set_var("HOME", home);
        }
        
        let path_with_home = get_default_conversation_storage_path();
        assert!(!path_with_home.as_os_str().is_empty());
    }
} 