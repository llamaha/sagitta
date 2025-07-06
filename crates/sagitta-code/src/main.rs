// mod agent; // Removed, should be used via lib.rs
// mod llm; // Removed
// mod tools; // Removed
// mod config; // Removed
// mod utils; // Removed
// mod gui; // This is conditionally compiled, might be okay or also via lib.rs

use anyhow::{anyhow, Context, Result};
use std::sync::Arc;

use sagitta_code::{
    agent::Agent,
    config::{SagittaCodeConfig, load_config},
    utils::init_logger,
    // If gui items are needed here from lib, add them e.g. gui::SagittaCodeApp
};

// When compiled with the "gui" feature, this will import and use the GUI modules
#[cfg(feature = "gui")]
mod gui_app {
    use super::*;
    use std::sync::Arc;
    use eframe::{egui, CreationContext};
    use tokio::sync::Mutex;
    // Ensure gui items are correctly pathed if lib.rs also declares pub mod gui;
    // If sagitta_code::gui is the canonical path from lib.rs, these direct uses are fine.
    use sagitta_code::gui::repository::manager::RepositoryManager;
    use sagitta_code::gui::app::SagittaCodeApp; // Already using sagitta_code::gui path
    use sagitta_code::config::SagittaCodeConfig;
    use sagitta_search::config::AppConfig as SagittaAppConfig;
    use sagitta_search::config::load_config as load_sagitta_config;
    
    struct GuiApp {
        app: SagittaCodeApp, // This now correctly refers to sagitta_code::gui::app::SagittaCodeApp
        update_sender: tokio::sync::mpsc::Sender<()>,
        update_receiver: Option<tokio::sync::mpsc::Receiver<()>>,
    }
    
    impl GuiApp {
        // Removed unused GuiApp::new method
    }
    
    impl eframe::App for GuiApp {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            // Process keyboard shortcuts at the application level if needed
            
            // Render the application UI
            self.app.render(ctx);
            
            // Request a repaint for animations (if needed)
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(0.1));
        }
    }
    
    pub async fn run(sagitta_code_config: SagittaCodeConfig) -> Result<()> {
        log::info!("Starting Sagitta Code GUI");
        
        // Load sagitta-search AppConfig
        let sagitta_config_path_val = sagitta_code_config.sagitta_config_path(); 
        let sagitta_app_config = match load_sagitta_config(Some(&sagitta_config_path_val)) {
            Ok(config) => config,
            Err(e) => {
                log::warn!("Failed to load sagitta-search config from {}: {}. Using default.", sagitta_config_path_val.display(), e);
                SagittaAppConfig::default() 
            }
        };
        
        // Create repository manager
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new(Arc::new(Mutex::new(sagitta_app_config.clone())))));
        
        // Initialize the repository manager with proper client and embedding handler
        let mut repo_manager_guard = repo_manager.lock().await;
        if let Err(e) = repo_manager_guard.initialize().await {
            log::warn!("Failed to fully initialize repository manager: {e}. Some features may be limited.");
        }
        drop(repo_manager_guard); // Release the lock
        
        let repo_manager_clone = Arc::clone(&repo_manager);
        
        // Create the SagittaCodeApp first to be able to initialize it properly
        let mut app_instance = sagitta_code::gui::app::SagittaCodeApp::new(
            repo_manager_clone.clone(), 
            sagitta_code_config, // Pass owned SagittaCodeConfig
            sagitta_app_config // Pass owned AppConfig
        );
        
        // Initialize the app asynchronously before launching eframe
        if let Err(e) = app_instance.initialize().await {
            log::error!("Failed to initialize SagittaCodeApp: {e}");
            // Consider showing an error message to the user
        }
        
        // Now create the actual GUI app with the pre-initialized SagittaCodeApp
        let app_creator = move |cc: &CreationContext| -> Result<Box<dyn eframe::App>, Box<dyn std::error::Error + Send + Sync>> {
            let (update_sender, update_receiver) = tokio::sync::mpsc::channel(10);
            
            // Set up the app style
            let visuals = egui::Visuals::dark();
            cc.egui_ctx.set_visuals(visuals);
            
            // Configure fonts for better emoji support
            sagitta_code::gui::fonts::apply_font_config(&cc.egui_ctx);
            
            let mut gui_app = GuiApp {
                app: app_instance, // Use the pre-initialized app
                update_sender,
                update_receiver: Some(update_receiver),
            };
            
            // Set up background task for updates if needed
            let mut update_receiver = gui_app.update_receiver.take().unwrap();
            
            let handle = tokio::runtime::Handle::current();
            handle.spawn(async move {
                // Listen for update requests
                while update_receiver.recv().await.is_some() {
                    // Handle update requests here if needed
                }
            });
            
            Ok(Box::new(gui_app))
        };
        
        // Launch the eframe application
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
            ..Default::default()
        };
        
        eframe::run_native(
            "Sagitta Code",
            options,
            Box::new(app_creator)
        ).map_err(|e| anyhow!("eframe error: {}", e))?;
        
        Ok(())
    }
}

// CLI version of the app for when GUI is not available or not wanted
mod cli_app {
    use std::io::{self, Write};
    use super::*;
    use sagitta_code::agent::state::types::{AgentState, AgentMode};
    use futures_util::StreamExt;
    
    // Imports for sagitta-search components
    use sagitta_search::config::AppConfig as SagittaAppConfig;
    use sagitta_search::config::load_config as load_sagitta_config;
    use sagitta_search::qdrant_client_trait::QdrantClientTrait;
    use qdrant_client::Qdrant;
    // Qdrant collection imports removed - no longer needed after removing analyze_input tool
    use sagitta_code::llm::client::LlmClient; // Corrected path
    use sagitta_code::llm::claude_code::client::ClaudeCodeClient;

    pub async fn run(config: SagittaCodeConfig) -> Result<()> {
        log::info!("Starting Sagitta Code CLI");
        
        // Load sagitta-search AppConfig (assuming core_config.toml is handled by this)
        // The SagittaCodeConfig might need a field for the path to sagitta_search's config,
        // or a shared config loading mechanism.
        // For now, try to load it using a default path or a path from SagittaCodeConfig if available.
        let sagitta_config_path_val = config.sagitta_config_path(); // PathBuf from SagittaCodeConfig
        let sagitta_app_config = match load_sagitta_config(Some(&sagitta_config_path_val)) {
            Ok(cfg) => cfg,
            Err(e) => {
                log::warn!(
                    "Failed to load sagitta-search config from {}: {}. Using default.", 
                    sagitta_config_path_val.display(), e
                );
                SagittaAppConfig::default() // Ensure SagittaAppConfig has a sensible default or handle error
            }
        };

        // Initialize Embedding Provider
        let embedding_config = sagitta_search::app_config_to_embedding_config(&sagitta_app_config);
        let embedding_pool = sagitta_search::EmbeddingPool::with_configured_sessions(embedding_config)
            .context("Failed to create embedding pool")?;
        let embedding_provider = Arc::new(sagitta_search::EmbeddingPoolAdapter::new(Arc::new(embedding_pool)));
        
        // Initialize Qdrant Client
        let qdrant_client_result = Qdrant::from_url(&sagitta_app_config.qdrant_url).build();
        let qdrant_client: Arc<dyn QdrantClientTrait> = match qdrant_client_result {
            Ok(client) => Arc::new(client),
            Err(e) => {
                log::error!("Failed to connect to Qdrant at {}: {}. Semantic tool analysis will be disabled.", sagitta_app_config.qdrant_url, e);
                // Create a mock/dummy client or handle error appropriately for graceful degradation
                // For now, let's panic or return an error, as it's critical for the new AnalyzeInputTool
                return Err(anyhow!("Failed to initialize Qdrant client: {}", e));
            }
        };
        
        // Create and register tools before creating the agent
        let tool_registry = Arc::new(sagitta_code::tools::registry::ToolRegistry::new());
        
        // Create the Claude Code LLM client (we'll wrap it in Arc later)
        let mut claude_client = ClaudeCodeClient::new(&config)
            .map_err(|e| anyhow!("Failed to create ClaudeCodeClient for CLI: {}", e))?;
        

        // Tools are now provided via MCP, not registered internally
        
        // Note: Qdrant tool collection setup removed - was only used by analyze_input tool which is no longer needed
        
        // Initialize MCP integration with the tool registry
        log::info!("CLI: Initializing MCP integration for Claude");
        if let Err(e) = claude_client.initialize_mcp(None).await {
            log::warn!("Failed to initialize MCP integration: {e}. Tool calls may not work.");
        }
        
        // Now wrap the client in Arc for use with the agent
        let llm_client_cli: Arc<dyn LlmClient> = Arc::new(claude_client);
        
        // Create concrete persistence and search engine for the CLI app
        let storage_path = if let Some(path) = &config.conversation.storage_path {
            path.clone()
        } else {
            let mut default_path = dirs::config_dir()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
            default_path.push("sagitta-code");
            default_path.push("conversations");
            default_path
        };

        let persistence: Box<dyn sagitta_code::agent::conversation::persistence::ConversationPersistence> = Box::new(
            sagitta_code::agent::conversation::persistence::disk::DiskConversationPersistence::new(storage_path).await
                .map_err(|e| anyhow!("Failed to create disk conversation persistence: {}", e))?
        );
        
        let search_engine: Box<dyn sagitta_code::agent::conversation::search::ConversationSearchEngine> = Box::new(
            sagitta_code::agent::conversation::search::text::TextConversationSearchEngine::new()
        );
        
        let agent = match Agent::new(
            config.clone(), 
            Some(tool_registry.clone()), 
            embedding_provider.clone(),
            persistence,
            search_engine,
            llm_client_cli.clone() // Pass the created LLM client
        ).await {
            Ok(agent) => agent,
            Err(e) => {
                eprintln!("Failed to create agent: {e}");
                return Err(anyhow!("Failed to create agent: {e}"));
            }
        };
        
        
        // Set to autonomous mode by default for CLI
        agent.set_mode(AgentMode::ToolsWithConfirmation).await?;
        
        // Subscribe to agent events
        let mut event_receiver = agent.subscribe();
        
        // Start a task to handle events
        let _event_task = tokio::spawn(async move {
            while let Ok(event) = event_receiver.recv().await {
                match event {
                    sagitta_code::agent::events::AgentEvent::LlmMessage(_msg) => {
                        // We'll handle printing full messages elsewhere
                    },
                    sagitta_code::agent::events::AgentEvent::LlmChunk { content, is_final, is_thinking: _ } => {
                        print!("{content}");
                        io::stdout().flush().unwrap();
                        if is_final {
                            println!();
                        }
                    },
                    sagitta_code::agent::events::AgentEvent::ToolCall { tool_call } => {
                        println!("\n[Tool call: {}]", tool_call.name);
                    },
                    sagitta_code::agent::events::AgentEvent::ToolCallComplete { tool_call_id: _, tool_name, result } => {
                        match &result {
                            sagitta_code::ToolResult::Success { .. } => {
                                println!("[Tool {tool_name} completed successfully]");
                            }
                            sagitta_code::ToolResult::Error { error } => {
                                println!("[Tool {tool_name} failed: {error}]");
                            }
                        }
                    },
                    sagitta_code::agent::events::AgentEvent::StateChanged(AgentState::Error { message, details: _ }) => {
                        eprintln!("Error: {message}");
                    },
                    sagitta_code::agent::events::AgentEvent::StateChanged(_) => {},
                    sagitta_code::agent::events::AgentEvent::Error(msg) => {
                        eprintln!("Error: {msg}");
                    },
                    _ => {}
                }
            }
        });
        
        // Main CLI loop
        println!("Sagitta Code CLI");
        println!("Type 'exit' or 'quit' to exit");
        println!("Type 'mode auto' for fully autonomous mode");
        println!("Type 'mode confirm' for tools with confirmation");
        println!("Type 'mode chat' for chat-only mode");
        println!();
        
        loop {
            print!("> ");
            io::stdout().flush().unwrap();
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();
            
            if input.is_empty() {
                continue;
            }
            
            if input == "exit" || input == "quit" {
                break;
            }
            
            if input == "mode auto" {
                agent.set_mode(AgentMode::FullyAutonomous).await?;
                println!("Mode set to fully autonomous");
                continue;
            }
            
            if input == "mode confirm" {
                agent.set_mode(AgentMode::ToolsWithConfirmation).await?;
                println!("Mode set to tools with confirmation");
                continue;
            }
            
            if input == "mode chat" {
                agent.set_mode(AgentMode::ChatOnly).await?;
                println!("Mode set to chat only");
                continue;
            }
            
            if input == "clear" {
                agent.clear_history().await?;
                println!("Conversation history cleared");
                continue;
            }
            
            // Process the message with streaming
            match agent.process_message_stream(input).await {
                Ok(mut stream) => {
                    // We'll handle the actual output through the event receiver
                    while stream.next().await.is_some() {}
                },
                Err(e) => {
                    eprintln!("Error: {e}");
                }
            }
        }
        
        Ok(())
    }
}

// MCP server mode
mod mcp_app {
    use super::*;
    use sagitta_code::llm::claude_code::mcp_integration::run_internal_mcp_server;
    use sagitta_search::config::AppConfig as SagittaAppConfig;
    use sagitta_search::config::load_config as load_sagitta_config;
    
    pub async fn run(sagitta_code_config: SagittaCodeConfig, is_internal: bool) -> Result<()> {
        if is_internal {
            log::info!("Starting Sagitta Code Internal MCP Server");
            
            // For internal mode, just run the sagitta-mcp Server directly
            // No need for ToolRegistry or any of that complexity
            let _tool_registry = Arc::new(sagitta_code::tools::registry::ToolRegistry::new()); // Still needed by function signature
            
            // Run the internal MCP server (which now uses sagitta-mcp Server)
            run_internal_mcp_server(None).await
        } else {
            log::info!("Starting Sagitta Code MCP Server");
            
            // Load sagitta-search AppConfig
            let sagitta_config_path_val = sagitta_code_config.sagitta_config_path();
            let sagitta_app_config = match load_sagitta_config(Some(&sagitta_config_path_val)) {
                Ok(config) => config,
                Err(e) => {
                    log::warn!("Failed to load sagitta-search config from {}: {}. Using default.", sagitta_config_path_val.display(), e);
                    SagittaAppConfig::default()
                }
            };
            
            // Create and run sagitta-mcp server directly
            let server = sagitta_mcp::server::Server::new(sagitta_app_config).await
                .context("Failed to create MCP server")?;
            
            server.run().await
                .context("MCP server failed")?;
            
            Ok(())
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Check for MCP mode from environment or command line args
    let args: Vec<String> = std::env::args().collect();
    let is_mcp_mode = args.contains(&"--mcp".to_string()) || 
                      std::env::var("SAGITTA_MCP_MODE").is_ok();
    let is_mcp_internal = args.contains(&"--mcp-internal".to_string());
    
    // Initialize the logger - but for MCP modes, ensure logs don't go to stdout
    if is_mcp_mode || is_mcp_internal {
        // For MCP server mode, we need to redirect logs away from stdout
        // since stdout is used for the JSON-RPC protocol
        std::env::set_var("RUST_LOG_TARGET", "stderr");
    }
    init_logger();
    
    // Load the configuration
    let config = match load_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Warning: Failed to load config: {e}");
            eprintln!("Using default configuration");
            SagittaCodeConfig::default()
        }
    };
    
    // Run in MCP mode if requested
    if is_mcp_mode || is_mcp_internal {
        return mcp_app::run(config, is_mcp_internal).await;
    }
    
    // Run the appropriate app version based on features
    #[cfg(feature = "gui")]
    {
        gui_app::run(config).await
    }
    
    #[cfg(not(feature = "gui"))]
    {
        cli_app::run(config).await
    }
}
