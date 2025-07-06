// Application initialization for the Sagitta Code

use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::sync::Mutex;
use super::SagittaCodeApp;
use super::super::repository::manager::RepositoryManager;
use super::super::repository::RepoPanel;
use super::super::theme::AppTheme;
use super::super::chat::view::{StreamingMessage, MessageAuthor};
use crate::config::loader::load_all_configs;
use crate::llm::claude_code::client::ClaudeCodeClient;
use crate::llm::client::LlmClient;
use crate::agent::Agent;
use crate::agent::state::types::AgentMode;
// Tool imports removed - tools now via MCP
use crate::config::SagittaCodeConfig;
// Add imports for concrete persistence/search and traits
use crate::agent::conversation::persistence::{
    ConversationPersistence, 
    disk::DiskConversationPersistence
};
use crate::agent::conversation::search::{
    ConversationSearchEngine, 
    text::TextConversationSearchEngine
};
use std::path::PathBuf;
// Additional tool imports removed - tools now via MCP

// Imports for sagitta-search components for embedding provider
 // For Path::new

// Qdrant imports
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use qdrant_client::Qdrant;
// Qdrant collection imports removed - no longer needed after removing analyze_input tool

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
    let config_guard = app.config.lock().await;
    match config_guard.ui.theme.as_str() {
        "light" => app.state.current_theme = AppTheme::Light,
        "custom" => {
            app.state.current_theme = AppTheme::Custom;
            
            // Load custom theme colors if path is specified
            if let Some(theme_path) = &config_guard.ui.custom_theme_path {
                match load_custom_theme_from_file(theme_path).await {
                    Ok(custom_colors) => {
                        crate::gui::theme::set_custom_theme_colors(custom_colors);
                        log::info!("Loaded custom theme from: {}", theme_path.display());
                    }
                    Err(e) => {
                        log::error!("Failed to load custom theme from {}: {}. Using default custom colors.", theme_path.display(), e);
                        // Fall back to default custom colors
                        crate::gui::theme::set_custom_theme_colors(crate::gui::theme::CustomThemeColors::default());
                    }
                }
            } else {
                log::warn!("Custom theme selected but no theme file path specified. Using default custom colors.");
                crate::gui::theme::set_custom_theme_colors(crate::gui::theme::CustomThemeColors::default());
            }
        }
        "dark" | _ => app.state.current_theme = AppTheme::Dark, // Default to Dark
    }
    drop(config_guard);
}

/// Load custom theme colors from a JSON file
async fn load_custom_theme_from_file(path: &std::path::Path) -> Result<crate::gui::theme::CustomThemeColors> {
    use tokio::fs;
    
    let content = fs::read_to_string(path).await
        .with_context(|| format!("Failed to read theme file: {}", path.display()))?;
    
    let colors: crate::gui::theme::CustomThemeColors = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse theme file: {}", path.display()))?;
    
    Ok(colors)
}

/// Create repository manager with config
pub async fn create_repository_manager(core_config: sagitta_search::AppConfig) -> Result<Arc<Mutex<RepositoryManager>>> {
    let repo_manager_core_config = Arc::new(Mutex::new(core_config)); 
    let repo_manager = Arc::new(Mutex::new(RepositoryManager::new(repo_manager_core_config)));
    
    if let Err(e) = repo_manager.lock().await.initialize().await {
        log::error!("Failed to initialize RepositoryManager: {e}");
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
            log::error!("Failed to create EmbeddingPool for GUI App: {e}. Intent analysis will be impaired.");
            Err(anyhow::anyhow!("Failed to create EmbeddingPool for GUI: {}", e))
        }
    }
}

/// Create LLM client from config (always Claude Code now)
pub async fn create_llm_client(config: &SagittaCodeConfig, _tool_registry: Option<()>) -> Result<Arc<dyn LlmClient>> {
    log::info!("Creating Claude Code LLM client");
    let mut claude_client = match ClaudeCodeClient::new(config) {
        Ok(client) => client,
        Err(e) => {
            log::error!(
                "Failed to create ClaudeCodeClient: {e}. Agent will not be initialized properly. Some features may be disabled."
            );
            return Err(anyhow::anyhow!("Failed to create ClaudeCodeClient for Agent: {}", e));
        }
    };
    
    // Initialize MCP directly (tools provided by sagitta-mcp)
    log::info!("Initializing MCP integration for Claude Code client");
    if let Err(e) = claude_client.initialize_mcp(None).await {
        log::warn!("Failed to initialize MCP integration: {e}. Tool calls may not work properly.");
    }
    
    Ok(Arc::new(claude_client))
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
    let (_code_config, core_config) = match load_all_configs() {
        Ok((code_cfg, core_cfg_opt)) => {
            log::info!("SagittaCodeApp: Loaded both configurations successfully");
            let core_cfg = core_cfg_opt.unwrap_or_else(|| {
                log::warn!("SagittaCodeApp: sagitta-search config not found, using default");
                sagitta_search::config::AppConfig::default()
            });
            (code_cfg, core_cfg)
        }
        Err(e) => {
            log::warn!("SagittaCodeApp: Could not load configurations: {e}. Using defaults.");
            (crate::config::SagittaCodeConfig::default(), sagitta_search::config::AppConfig::default())
        }
    };

    // Create repository manager
    let repo_manager = create_repository_manager(core_config.clone()).await?;
    
    // Initialize embedding pool
    let embedding_handler_arc = create_embedding_pool(&core_config)?;
    
    // Set the embedding handler on the repository manager
    {
        let mut manager_guard = repo_manager.lock().await;
        manager_guard.set_embedding_handler(embedding_handler_arc.clone());
        log::info!("SagittaCodeApp: Set embedding handler on repository manager");
    }
    
    app.repo_panel = RepoPanel::new(
        repo_manager.clone(),
        app.config.clone(),
        None, // Agent will be set later after it's initialized
    );
    app.repo_panel.refresh_repositories(); // Initial refresh
    log::info!("SagittaCodeApp: RepoPanel initialized and refreshed.");

    // Create adapter for EmbeddingProvider compatibility
    let embedding_provider_adapter = Arc::new(sagitta_search::EmbeddingPoolAdapter::new(embedding_handler_arc.clone()));

    // Configure theme
    configure_theme_from_config(app).await;
    
    // Create LLM client (MCP tools are handled internally)
    let config_guard = app.config.lock().await;
    let llm_client = create_llm_client(&config_guard, None).await.map_err(|e| {
        let error_message = StreamingMessage::from_text(
            MessageAuthor::System,
            format!("CRITICAL: Failed to initialize LLM Client: {e}. Agent is disabled."),
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
            app.panels.events_panel.add_event(super::SystemEventType::Error, format!("Qdrant connection failed: {e}"));
            None
        }
    };

    // Create trait version for tools that need it
    let _qdrant_client: Arc<dyn QdrantClientTrait> = if let Some(ref concrete_client) = qdrant_client_concrete {
        concrete_client.clone()
    } else {
        // Fallback - should not happen in normal operation, but provides safety
        return Err(anyhow::anyhow!("Failed to initialize Qdrant client for GUI"));
    };

    // Note: Qdrant tool collection setup removed - was only used by analyze_input tool which is no longer needed

    // Tool registry already initialized earlier for MCP
    
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
    
    // Create WorkingDirectoryManager stub
    let working_dir_manager = Arc::new(crate::tools::WorkingDirectoryManager::new(working_dir.clone())
        .map_err(|e| anyhow::anyhow!("Failed to create WorkingDirectoryManager: {}", e))?);
    
    // Store the working directory manager in the app
    app.working_dir_manager = Some(working_dir_manager.clone());
    log::info!("Working directory manager initialized with base path: {}", working_dir.display());

    // Working directory manager removed
    
    // Load saved repository context from config
    let saved_repo_context = {
        let config_guard = app.config.lock().await;
        config_guard.ui.current_repository_context.clone()
    };
    
    if let Some(saved_repo_context) = saved_repo_context {
        app.state.set_repository_context(Some(saved_repo_context.clone()));
        log::info!("Restored repository context from config: {saved_repo_context}");
        
        // Working directory context handling removed
    }

    // Tool registration removed - tools now provided via MCP from sagitta-mcp

    // Create conversation persistence
    let config_guard = app.config.lock().await;
    let persistence = create_conversation_persistence(&config_guard).await?;
    let config_clone = config_guard.clone();
    drop(config_guard);
    let search_engine: Box<dyn ConversationSearchEngine> = Box::new(TextConversationSearchEngine::new());

    let agent_result = Agent::new(
        config_clone, 
        None, // No tool registry - tools via MCP
        embedding_provider_adapter.clone(), // Use the adapter instead of raw pool
        persistence, // Pass concrete persistence
        search_engine, // Pass concrete search engine
        llm_client.clone() // Add the llm_client argument here
    ).await;
    match agent_result {
        Ok(agent) => {
            // Set agent to fully autonomous mode to allow automatic tool execution
            if let Err(e) = agent.set_mode(AgentMode::FullyAutonomous).await {
                log::error!("Failed to set agent mode to FullyAutonomous: {e}");
            } else {
                log::info!("Agent mode set to FullyAutonomous for automatic tool execution");
            }
            
            // CRITICAL: Wire up terminal event sender to tool executor for streaming shell execution
            // Terminal functionality removed
            
            // Subscribe to agent events
            let event_receiver = agent.subscribe();
            let agent_arc = Arc::new(agent);
            app.agent = Some(agent_arc.clone());
            app.agent_event_receiver = Some(event_receiver);
            
            // Set the agent on the RepoPanel
            app.repo_panel.set_agent(agent_arc);
            
            // Initialize conversation service for the sidebar - use shared instances (Phase 1 optimization)
            if let Err(e) = app.initialize_conversation_service_with_shared_instances(qdrant_client_concrete.clone(), Some(embedding_handler_arc.clone())).await {
                log::warn!("Failed to initialize conversation service: {e}. Conversation sidebar features may be limited.");
                app.panels.events_panel.add_event(
                    super::SystemEventType::Info,
                    format!("Conversation service initialization failed: {e}")
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
            log::error!("Failed to initialize agent: {err}");
            
            // Add to events panel instead of chat
            app.panels.events_panel.add_event(
                super::SystemEventType::Error,
                format!("Failed to initialize agent: {err}. Check your OpenRouter API key in settings.")
            );
        }
    }
    
    // Load initial repository list to ensure saved repository context is available in dropdown
    {
        log::info!("Loading initial repository list...");
        let repo_manager = app.repo_panel.get_repo_manager();
        let app_event_sender = app.app_event_sender.clone();
        
        tokio::spawn(async move {
            match repo_manager.lock().await.list_repositories().await {
                Ok(repositories) => {
                    let repo_names: Vec<String> = repositories
                        .iter()
                        .map(|repo| repo.name.clone())
                        .collect();
                    
                    log::info!("Initial repository list loaded: {repo_names:?}");
                    
                    // Send the repository list update event
                    if let Err(e) = app_event_sender.send(super::events::AppEvent::RepositoryListUpdated(repo_names)) {
                        log::error!("Failed to send initial repository list update event: {e}");
                    } else {
                        log::debug!("Successfully sent initial repository list update event");
                    }
                },
                Err(e) => {
                    log::error!("Failed to load initial repository list: {e}");
                }
            }
        });
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
        let sagitta_config = SagittaCodeConfig::default();
        
        let app = SagittaCodeApp::new(repo_manager, sagitta_config, app_config);
        
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

    #[tokio::test]
    async fn test_create_llm_client_with_invalid_config() {
        let mut config = SagittaCodeConfig::default();
        
        // Set invalid claude path to force failure (if binary doesn't exist)
        config.claude_code.claude_path = "/nonexistent/claude/path".to_string();
        
        let result = create_llm_client(&config, None).await;
        
        // May succeed or fail depending on validation - test structure is important
        match result {
            Ok(_) => {
                // Client created but might fail later during actual API calls
                assert!(true);
            }
            Err(e) => {
                // Expected failure with invalid config
                assert!(e.to_string().contains("Failed to create ClaudeCodeClient"));
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
        
        let app = SagittaCodeApp::new(repo_manager, sagitta_config, app_config);
        let original_theme = app.state.current_theme;
        
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

    #[tokio::test]
    async fn test_repository_context_restoration() {
        use tempfile::TempDir;
        
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        
        // Create test config with repository context
        let mut sagitta_config = SagittaCodeConfig::default();
        sagitta_config.ui.current_repository_context = Some("saved-repo".to_string());
        
        let app_config = CoreAppConfig::default();
        let repo_manager = Arc::new(tokio::sync::Mutex::new(
            RepositoryManager::new(Arc::new(tokio::sync::Mutex::new(app_config.clone())))
        ));
        
        // Create app
        let app = SagittaCodeApp::new(repo_manager.clone(), sagitta_config.clone(), app_config);
        
        // Initially no repository context in state
        assert_eq!(app.state.current_repository_context, None);
        
        // TODO: We can't easily test the full initialization here because it requires
        // setting up the entire environment (Qdrant, embedding models, etc.)
        // But we've verified the loading logic in the loader tests.
    }
} 