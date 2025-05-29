// mod agent; // Removed, should be used via lib.rs
// mod llm; // Removed
// mod tools; // Removed
// mod config; // Removed
// mod utils; // Removed
// mod gui; // This is conditionally compiled, might be okay or also via lib.rs

use anyhow::Result;
use std::sync::Arc;

use sagitta_code::{
    agent::Agent,
    config::{FredAgentConfig, load_config},
    utils::init_logger,
    // If gui items are needed here from lib, add them e.g. gui::FredAgentApp
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
    use sagitta_code::gui::app::FredAgentApp; // Already using sagitta_code::gui path
    use sagitta_code::gui::fonts;
    use sagitta_code::config::FredAgentConfig;
    use sagitta_search::config::AppConfig as SagittaAppConfig;
    use sagitta_search::config::load_config as load_sagitta_config;
    
    struct GuiApp {
        app: FredAgentApp, // This now correctly refers to sagitta_code::gui::app::FredAgentApp
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
    
    pub async fn run(fred_config: FredAgentConfig) -> Result<()> {
        log::info!("Starting Fred Agent GUI");
        
        // Load sagitta-search AppConfig
        let sagitta_config_path_val = fred_config.sagitta_config_path(); 
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
            log::warn!("Failed to fully initialize repository manager: {}. Some features may be limited.", e);
        }
        drop(repo_manager_guard); // Release the lock
        
        let repo_manager_clone = Arc::clone(&repo_manager);
        
        // Create the FredAgentApp first to be able to initialize it properly
        let mut app_instance = sagitta_code::gui::app::FredAgentApp::new(
            repo_manager_clone.clone(), 
            fred_config, // Pass owned FredAgentConfig
            sagitta_app_config // Pass owned AppConfig
        );
        
        // Initialize the app asynchronously before launching eframe
        if let Err(e) = app_instance.initialize().await {
            log::error!("Failed to initialize FredAgentApp: {}", e);
            // Consider showing an error message to the user
        }
        
        // Now create the actual GUI app with the pre-initialized FredAgentApp
        let app_creator = move |cc: &CreationContext| -> Result<Box<dyn eframe::App>, Box<dyn std::error::Error + Send + Sync>> {
            let (update_sender, update_receiver) = tokio::sync::mpsc::channel(10);
            
            // Set up the app style
            let mut visuals = egui::Visuals::dark();
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
                while let Some(_) = update_receiver.recv().await {
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
            "Fred Agent",
            options,
            Box::new(app_creator)
        ).map_err(|e| anyhow::anyhow!("eframe error: {}", e))?;
        
        Ok(())
    }
}

// CLI version of the app for when GUI is not available or not wanted
mod cli_app {
    use std::io::{self, Write};
    use super::*;
    use sagitta_code::agent::state::types::{AgentState, AgentMode};
    use futures_util::StreamExt;
    use std::path::Path; // For Path::new

    // Imports for sagitta-search components
    use sagitta_search::config::AppConfig as SagittaAppConfig;
    use sagitta_search::config::load_config as load_sagitta_config;
    use sagitta_search::embedding::provider::onnx::{OnnxEmbeddingModel, ThreadSafeOnnxProvider};
    use sagitta_search::qdrant_client_trait::QdrantClientTrait;
    use qdrant_client::Qdrant;
    use qdrant_client::qdrant::{
        CreateCollection, VectorParams, VectorsConfig,
        Distance, PointStruct, UpsertPoints,
        vectors_config::Config as VectorsConfigEnum,
    };
    use sagitta_code::tools::analyze_input::TOOLS_COLLECTION_NAME;
    use serde_json;
    use sagitta_search::embedding::provider::EmbeddingProvider;
    use qdrant_client::Payload;
    use sagitta_code::llm::client::LlmClient; // Corrected path
    use sagitta_code::llm::gemini::client::GeminiClient; // Corrected path

    pub async fn run(config: FredAgentConfig) -> Result<()> {
        log::info!("Starting Fred Agent CLI");
        
        // Load sagitta-search AppConfig (assuming core_config.toml is handled by this)
        // The FredAgentConfig might need a field for the path to sagitta_search's config,
        // or a shared config loading mechanism.
        // For now, try to load it using a default path or a path from FredAgentConfig if available.
        let sagitta_config_path_val = config.sagitta_config_path(); // PathBuf from FredAgentConfig
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
        let embedding_provider = {
            let model_path_str = sagitta_app_config.onnx_model_path.as_deref()
                .ok_or_else(|| anyhow::anyhow!("ONNX model path not set in sagitta_search config"))?;
            let tokenizer_path_str = sagitta_app_config.onnx_tokenizer_path.as_deref()
                .ok_or_else(|| anyhow::anyhow!("ONNX tokenizer path not set in sagitta_search config"))?;

            let onnx_model = OnnxEmbeddingModel::new(
                Path::new(model_path_str),
                Path::new(tokenizer_path_str),
            ).map_err(|e| anyhow::anyhow!("Failed to create OnnxEmbeddingModel: {}", e))?;
            
            Arc::new(ThreadSafeOnnxProvider::new(onnx_model))
        };
        
        // Initialize Qdrant Client
        let qdrant_client_result = Qdrant::from_url(&sagitta_app_config.qdrant_url).build();
        let qdrant_client: Arc<dyn QdrantClientTrait> = match qdrant_client_result {
            Ok(client) => Arc::new(client),
            Err(e) => {
                log::error!("Failed to connect to Qdrant at {}: {}. Semantic tool analysis will be disabled.", sagitta_app_config.qdrant_url, e);
                // Create a mock/dummy client or handle error appropriately for graceful degradation
                // For now, let's panic or return an error, as it's critical for the new AnalyzeInputTool
                return Err(anyhow::anyhow!("Failed to initialize Qdrant client: {}", e));
            }
        };
        
        // Create and register tools before creating the agent
        let tool_registry = Arc::new(sagitta_code::tools::registry::ToolRegistry::new());
        
        // Register AnalyzeInputTool first, passing the qdrant_client
        tool_registry.register(Arc::new(sagitta_code::tools::analyze_input::AnalyzeInputTool::new(tool_registry.clone(), embedding_provider.clone(), qdrant_client.clone()))).await.unwrap_or_else(|e| {
            eprintln!("Warning: Failed to register AnalyzeInputTool: {}", e);
        });

        // Register shell execution and test execution tools
        let default_working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        tool_registry.register(Arc::new(sagitta_code::tools::shell_execution::ShellExecutionTool::new(default_working_dir.clone()))).await.unwrap_or_else(|e| {
            eprintln!("Warning: Failed to register shell execution tool: {}", e);
        });
        tool_registry.register(Arc::new(sagitta_code::tools::test_execution::TestExecutionTool::new(default_working_dir))).await.unwrap_or_else(|e| {
            eprintln!("Warning: Failed to register test execution tool: {}", e);
        });
        
        // --- Populate Qdrant tool collection (run once or ensure exists) ---
        // This logic is NOW MOVED to after all tools are registered.
        let vector_size = embedding_provider.dimension() as u64;
        match qdrant_client.collection_exists(TOOLS_COLLECTION_NAME.to_string()).await {
            Ok(exists) => {
                if !exists {
                    log::info!("CLI: Creating Qdrant tool collection: {}", TOOLS_COLLECTION_NAME);
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
                        log::error!("CLI: Failed to create Qdrant tool collection \'{}\': {}", TOOLS_COLLECTION_NAME, e);
                    }
                } else {
                    log::info!("CLI: Qdrant tool collection \'{}\' already exists.", TOOLS_COLLECTION_NAME);
                }
            }
            Err(e) => {
                log::error!("CLI: Failed to check Qdrant tool collection '{}': {}. Tool analysis might fail.", TOOLS_COLLECTION_NAME, e);
            }
        }
        
        let all_tool_defs = tool_registry.get_definitions().await;
        let mut points_to_upsert = Vec::new();
        for (idx, tool_def) in all_tool_defs.iter().enumerate() {
            let tool_desc_text = format!("{}: {}", tool_def.name, tool_def.description);
            match embedding_provider.embed_batch(&[&tool_desc_text]) {
                Ok(mut embeddings) => {
                    if let Some(embedding) = embeddings.pop() {
                        let mut payload_map: std::collections::HashMap<String, qdrant_client::qdrant::Value> = std::collections::HashMap::new();
                        payload_map.insert("tool_name".to_string(), tool_def.name.clone().into());
                        payload_map.insert("description".to_string(), tool_def.description.clone().into());
                        let params_json_str = serde_json::to_string(&tool_def.parameters).unwrap_or_else(|_| "{}".to_string());
                        payload_map.insert("parameter_schema".to_string(), params_json_str.into());
                        
                        points_to_upsert.push(PointStruct::new(
                            qdrant_client::qdrant::PointId::from(idx as u64), // Explicit PointId conversion for u64
                            qdrant_client::qdrant::NamedVectors::default()
                                .add_vector("dense", embedding), 
                            qdrant_client::Payload::from(payload_map) // Explicit Payload conversion
                        ));
                    }
                }
                Err(e) => log::warn!("CLI: Failed to generate embedding for tool '{}' during Qdrant population: {}", tool_def.name, e),
            }
        }
        if !points_to_upsert.is_empty() {
            let upsert_request = UpsertPoints {
                collection_name: TOOLS_COLLECTION_NAME.to_string(),
                wait: Some(true),
                points: points_to_upsert,
                ordering: None,
                shard_key_selector: None,
            };
            if let Err(e) = qdrant_client.upsert_points(upsert_request).await {
                log::error!("CLI: Failed to upsert tool definitions to Qdrant: {}", e);
            }
        }
        // --- End Qdrant tool collection population ---
        
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
                .map_err(|e| anyhow::anyhow!("Failed to create disk conversation persistence: {}", e))?
        );
        
        let search_engine: Box<dyn sagitta_code::agent::conversation::search::ConversationSearchEngine> = Box::new(
            sagitta_code::agent::conversation::search::text::TextConversationSearchEngine::new()
        );
        
        // Create the LLM client for the CLI app
        let llm_client_cli: Arc<dyn LlmClient> = Arc::new(
            GeminiClient::new(&config)
                .map_err(|e| anyhow::anyhow!("Failed to create GeminiClient for CLI: {}", e))?
        );

        let agent = match Agent::new(
            config.clone(), 
            tool_registry.clone(), 
            embedding_provider.clone(),
            persistence,
            search_engine,
            llm_client_cli.clone() // Pass the created LLM client
        ).await {
            Ok(agent) => agent,
            Err(e) => {
                eprintln!("Failed to create agent: {}", e);
                return Err(anyhow::anyhow!("Failed to create agent: {}", e));
            }
        };
        
        // Set to autonomous mode by default for CLI
        agent.set_mode(AgentMode::ToolsWithConfirmation).await?;
        
        // Subscribe to agent events
        let mut event_receiver = agent.subscribe();
        
        // Start a task to handle events
        let event_task = tokio::spawn(async move {
            while let Ok(event) = event_receiver.recv().await {
                match event {
                    sagitta_code::agent::events::AgentEvent::LlmMessage(msg) => {
                        // We'll handle printing full messages elsewhere
                    },
                    sagitta_code::agent::events::AgentEvent::LlmChunk { content, is_final } => {
                        print!("{}", content);
                        io::stdout().flush().unwrap();
                        if is_final {
                            println!();
                        }
                    },
                    sagitta_code::agent::events::AgentEvent::ToolCall { tool_call } => {
                        println!("\n[Tool call: {}]", tool_call.name);
                    },
                    sagitta_code::agent::events::AgentEvent::ToolCallComplete { tool_call_id: _, tool_name, result } => {
                        if result.is_success() {
                            println!("[Tool {} completed successfully]", tool_name);
                        } else if let Some(error) = result.error_message() {
                            println!("[Tool {} failed: {}]", tool_name, error);
                        }
                    },
                    sagitta_code::agent::events::AgentEvent::StateChanged(state) => {
                        if let AgentState::Error { message, details: _ } = &state {
                            eprintln!("Error: {}", message);
                        }
                    },
                    sagitta_code::agent::events::AgentEvent::Error(msg) => {
                        eprintln!("Error: {}", msg);
                    },
                    _ => {}
                }
            }
        });
        
        // Main CLI loop
        println!("Fred Agent CLI");
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
                    while let Some(_) = stream.next().await {}
                },
                Err(e) => {
                    eprintln!("Error: {}", e);
                }
            }
        }
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the logger
    init_logger();
    
    // Load the configuration
    let config = match load_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Warning: Failed to load config: {}", e);
            eprintln!("Using default configuration");
            FredAgentConfig::default()
        }
    };
    
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
