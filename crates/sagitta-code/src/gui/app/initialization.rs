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
use crate::llm::gemini::client::GeminiClient;
use crate::llm::client::LlmClient;
use crate::agent::Agent;
use crate::agent::state::types::AgentMode;
use crate::tools::code_search::tool::CodeSearchTool;
use crate::tools::file_operations::read::ReadFileTool;
use crate::tools::repository::list::ListRepositoriesTool;
use crate::tools::repository::search::SearchFileInRepositoryTool;
use crate::tools::repository::view::ViewFileInRepositoryTool;
use crate::tools::repository::add::AddRepositoryTool;
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
use crate::tools::test_execution::TestExecutionTool;
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

/// Initialize the application
pub async fn initialize(app: &mut SagittaCodeApp) -> Result<()> {
    log::info!("SagittaCodeApp: Initializing...");

    // Load both sagitta-code and sagitta-search configs
    let (code_config, core_config) = match load_all_configs() {
        Ok(configs) => {
            log::info!("SagittaCodeApp: Loaded both configurations successfully");
            configs
        }
        Err(e) => {
            log::warn!("SagittaCodeApp: Could not load configurations: {}. Using defaults.", e);
            (crate::config::SagittaCodeConfig::default(), sagitta_search::config::AppConfig::default())
        }
    };

    // Store a clone for RepositoryManager
    let repo_manager_core_config = Arc::new(Mutex::new(core_config.clone())); 
    app.repo_panel = RepoPanel::new(Arc::new(Mutex::new(RepositoryManager::new(repo_manager_core_config))));
    if let Err(e) = app.repo_panel.get_repo_manager().lock().await.initialize().await {
        log::error!("Failed to initialize RepositoryManager: {}",e);
    }

    // Initialize Embedding Provider using the loaded core_config
    let embedding_handler_arc: Arc<EmbeddingPool> = {
        let embedding_config = sagitta_search::app_config_to_embedding_config(&core_config);
        match EmbeddingPool::with_configured_sessions(embedding_config) {
            Ok(pool) => {
                log::info!("Successfully created EmbeddingPool for GUI App.");
                Arc::new(pool)
            }
            Err(e) => {
                log::error!("Failed to create EmbeddingPool for GUI App: {}. Intent analysis will be impaired.", e);
                return Err(anyhow::anyhow!("Failed to create EmbeddingPool for GUI: {}", e));
            }
        }
    };

    // Create adapter for EmbeddingProvider compatibility
    let embedding_provider_adapter = Arc::new(sagitta_search::EmbeddingPoolAdapter::new(embedding_handler_arc.clone()));

    app.repo_panel.refresh_repositories(); // Initial refresh
    log::info!("SagittaCodeApp: RepoPanel initialized and refreshed.");

    // Load theme from config
    match app.config.ui.theme.as_str() {
        "light" => app.state.current_theme = AppTheme::Light,
        "dark" | _ => app.state.current_theme = AppTheme::Dark, // Default to Dark
    }
    
    // Create Gemini Client first (before agent and tool initialization)
    let gemini_client_result = GeminiClient::new(&app.config);
    
    let llm_client: Arc<dyn LlmClient> = match gemini_client_result {
        Ok(client) => Arc::new(client),
        Err(e) => {
            log::error!(
                "Failed to create GeminiClient: {}. Agent will not be initialized properly. Some features may be disabled.",
                e
            );
            let error_message = StreamingMessage::from_text(
                MessageAuthor::System,
                format!("CRITICAL: Failed to initialize LLM Client (Gemini): {}. Agent is disabled.", e),
            );
            app.chat_manager.add_complete_message(error_message);
            return Err(anyhow::anyhow!("Failed to create GeminiClient for Agent: {}", e));
        }
    };

    // Initialize Qdrant Client
    let qdrant_client_result = Qdrant::from_url(&core_config.qdrant_url).build();
    let qdrant_client: Arc<dyn QdrantClientTrait> = match qdrant_client_result {
        Ok(client) => Arc::new(client),
        Err(e) => {
            log::error!("GUI: Failed to connect to Qdrant at {}: {}. Semantic tool analysis will be disabled.", core_config.qdrant_url, e);
            app.panels.events_panel.add_event(super::SystemEventType::Error, format!("Qdrant connection failed: {}", e));
            return Err(anyhow::anyhow!("Failed to initialize Qdrant client for GUI: {}", e));
        }
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

    let repo_manager = app.repo_panel.get_repo_manager().clone();
    // Initialize ToolRegistry
    let tool_registry = Arc::new(crate::tools::registry::ToolRegistry::new());

    // Register tools first
    tool_registry.register(Arc::new(AnalyzeInputTool::new(tool_registry.clone(), embedding_provider_adapter.clone(), qdrant_client.clone()))).await?;
    tool_registry.register(Arc::new(CodeSearchTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(ReadFileTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(ListRepositoriesTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(SearchFileInRepositoryTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(ViewFileInRepositoryTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(AddRepositoryTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(SyncRepositoryTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(RemoveRepositoryTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(RepositoryMapTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(TargetedViewTool::new(repo_manager.clone()))).await?;
    tool_registry.register(Arc::new(WebSearchTool::new(llm_client.clone()))).await?;
    tool_registry.register(Arc::new(EditTool::new(repo_manager.clone()))).await?; // Added EditTool registration
    tool_registry.register(Arc::new(crate::tools::repository::SwitchBranchTool::new(repo_manager.clone()))).await?;
    let default_working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    tool_registry.register(Arc::new(crate::tools::shell_execution::ShellExecutionTool::new(default_working_dir.clone()))).await?;
    tool_registry.register(Arc::new(crate::tools::test_execution::TestExecutionTool::new(default_working_dir.clone()))).await?;

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

    // Create concrete persistence and search engine for the GUI app
    let storage_path = if let Some(path) = &app.config.conversation.storage_path {
        path.clone()
    } else {
        let mut default_path = dirs::config_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
        default_path.push("sagitta-code");
        default_path.push("conversations");
        default_path
    };

    let persistence: Box<dyn ConversationPersistence> = Box::new(
        DiskConversationPersistence::new(storage_path).await
            .map_err(|e| anyhow::anyhow!("Failed to create disk conversation persistence: {}", e))?
    );
    
    let search_engine: Box<dyn ConversationSearchEngine> = Box::new(TextConversationSearchEngine::new());

    let agent_result = Agent::new(
        app.config.as_ref().clone(), 
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
            
            // Subscribe to agent events
            let event_receiver = agent.subscribe();
            app.agent = Some(Arc::new(agent));
            app.agent_event_receiver = Some(event_receiver);
            
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
                format!("Failed to initialize agent: {}. Check your Gemini API key in settings.", err)
            );
        }
    }
    
    Ok(())
} 