use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use std::path::PathBuf;
use std::io::{self, Write};
use futures_util::StreamExt;
use tokio::time::{timeout, Duration};

use sagitta_code::{
    agent::{Agent, state::types::AgentMode},
    config::{SagittaCodeConfig, load_config},
    utils::init_logger,
    llm::client::LlmClient,
    llm::openrouter::client::OpenRouterClient,
    tools::analyze_input::TOOLS_COLLECTION_NAME,
};

// Imports for sagitta-search components
use sagitta_search::config::AppConfig as SagittaAppConfig;
use sagitta_search::config::load_config as load_sagitta_config;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollection, VectorParams, VectorsConfig,
    Distance, PointStruct, UpsertPoints,
    vectors_config::Config as VectorsConfigEnum,
};
use serde_json;
use sagitta_embed::provider::EmbeddingProvider;
use qdrant_client::Payload;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the logger with more verbose output for debugging
    init_logger();
    
    println!("🤖 Sagitta Code CLI Chat");
    println!("{}", "=".repeat(50));
    println!("Interactive chat interface for testing the reasoning engine");
    println!("Type 'exit', 'quit', or Ctrl+C to exit");
    println!("Type 'help' for available commands");
    println!();

    // Load the configuration
    let config = match load_config() {
        Ok(config) => {
            println!("✓ Configuration loaded successfully");
            config
        }
        Err(e) => {
            eprintln!("⚠️  Warning: Failed to load config: {}", e);
            eprintln!("Using default configuration");
            SagittaCodeConfig::default()
        }
    };

    // Check for API key - follow the same pattern as OpenRouterClient
    let api_key_available = match config.openrouter.api_key.as_ref() {
        Some(key) if !key.is_empty() => true,
        _ => {
            // Check environment variable as fallback
            std::env::var("OPENROUTER_API_KEY").map(|key| !key.is_empty()).unwrap_or(false)
        }
    };

    if !api_key_available {
        eprintln!("❌ Error: OPENROUTER_API_KEY not available");
        eprintln!("Please set your OpenRouter API key in:");
        eprintln!("  1. Configuration file: ~/.config/sagitta/sagitta_code_config.json");
        eprintln!("  2. Environment variable: export OPENROUTER_API_KEY=your_key_here");
        std::process::exit(1);
    }

    // Initialize the agent
    let agent = match initialize_agent(config.clone()).await {
        Ok(agent) => {
            println!("✓ Agent initialized successfully");
            agent
        }
        Err(e) => {
            eprintln!("❌ Failed to initialize agent: {}", e);
            std::process::exit(1);
        }
    };

    // Set default mode
    if let Err(e) = agent.set_mode(AgentMode::ToolsWithConfirmation).await {
        eprintln!("⚠️  Warning: Failed to set agent mode: {}", e);
    }

    // Subscribe to agent events for real-time feedback
    let mut event_receiver = agent.subscribe();
    
    // Start event handler task
    let event_task = tokio::spawn(async move {
        while let Ok(event) = event_receiver.recv().await {
            match event {
                sagitta_code::agent::events::AgentEvent::LlmChunk { content, is_final } => {
                    print!("{}", content);
                    io::stdout().flush().unwrap();
                    if is_final {
                        println!();
                    }
                },
                sagitta_code::agent::events::AgentEvent::ToolCall { tool_call } => {
                    println!("\n🔧 [Tool call: {}]", tool_call.name);
                },
                sagitta_code::agent::events::AgentEvent::ToolCallComplete { tool_call_id: _, tool_name, result } => {
                    if result.is_success() {
                        println!("✅ [Tool {} completed successfully]", tool_name);
                    } else if let Some(error) = result.error_message() {
                        println!("❌ [Tool {} failed: {}]", tool_name, error);
                    }
                },
                sagitta_code::agent::events::AgentEvent::StateChanged(state) => {
                    use sagitta_code::agent::state::types::AgentState;
                    match &state {
                        AgentState::Thinking { message } => {
                            println!("🤔 [Thinking: {}]", message);
                        },
                        AgentState::Responding { is_streaming, .. } => {
                            if *is_streaming {
                                println!("💬 [Streaming response...]");
                            } else {
                                println!("💬 [Responding...]");
                            }
                        },
                        AgentState::Error { message, .. } => {
                            eprintln!("❌ [Error: {}]", message);
                        },
                        _ => {}
                    }
                },
                sagitta_code::agent::events::AgentEvent::Error(msg) => {
                    eprintln!("❌ [Error: {}]", msg);
                },
                _ => {}
            }
        }
    });

    println!("🚀 Chat interface ready! Start typing your message...");
    println!();

    // Main interactive loop
    loop {
        print!("👤 ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                // EOF reached (e.g., piped input finished)
                println!("👋 Input stream ended. Goodbye!");
                break;
            },
            Ok(_) => {},
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        }
        
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        // Handle special commands
        match input {
            "exit" | "quit" => {
                println!("👋 Goodbye!");
                break;
            },
            "help" => {
                print_help();
                continue;
            },
            "clear" => {
                if let Err(e) = agent.clear_history().await {
                    eprintln!("Error clearing history: {}", e);
                } else {
                    println!("🗑️  Conversation history cleared");
                }
                continue;
            },
            "mode auto" => {
                if let Err(e) = agent.set_mode(AgentMode::FullyAutonomous).await {
                    eprintln!("Error setting mode: {}", e);
                } else {
                    println!("🤖 Mode set to fully autonomous");
                }
                continue;
            },
            "mode confirm" => {
                if let Err(e) = agent.set_mode(AgentMode::ToolsWithConfirmation).await {
                    eprintln!("Error setting mode: {}", e);
                } else {
                    println!("🔧 Mode set to tools with confirmation");
                }
                continue;
            },
            "mode chat" => {
                if let Err(e) = agent.set_mode(AgentMode::ChatOnly).await {
                    eprintln!("Error setting mode: {}", e);
                } else {
                    println!("💬 Mode set to chat only");
                }
                continue;
            },
            _ => {}
        }
        
        println!("🤖 ");
        
        // Process the message with streaming and timeout
        let process_future = agent.process_message_stream(input);
        let timeout_duration = Duration::from_secs(300); // 5 minute timeout
        
        match timeout(timeout_duration, process_future).await {
            Ok(Ok(mut stream)) => {
                // Process the stream
                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(_chunk) => {
                            // The actual output is handled by the event receiver task
                            // which prints chunks in real-time
                        },
                        Err(e) => {
                            eprintln!("\n❌ Stream error: {}", e);
                            break;
                        }
                    }
                }
                println!(); // Ensure we end with a newline
            },
            Ok(Err(e)) => {
                eprintln!("❌ Error processing message: {}", e);
            },
            Err(_) => {
                eprintln!("⏰ Request timed out after {} seconds", timeout_duration.as_secs());
            }
        }
        
        println!(); // Extra spacing between interactions
    }

    // Clean shutdown
    event_task.abort();
    Ok(())
}

async fn initialize_agent(config: SagittaCodeConfig) -> Result<Agent> {
    println!("⚙️  Initializing agent components...");
    
    // Load sagitta-search AppConfig
    let sagitta_config_path_val = config.sagitta_config_path();
    let sagitta_app_config = match load_sagitta_config(Some(&sagitta_config_path_val)) {
        Ok(cfg) => {
            println!("✓ Sagitta-search config loaded");
            cfg
        },
        Err(e) => {
            println!("⚠️  Failed to load sagitta-search config from {}: {}. Using default.", 
                    sagitta_config_path_val.display(), e);
            SagittaAppConfig::default()
        }
    };

    // Initialize Embedding Provider
    println!("🔧 Setting up embedding provider...");
    let embedding_config = sagitta_search::app_config_to_embedding_config(&sagitta_app_config);
    let embedding_pool = sagitta_search::EmbeddingPool::with_configured_sessions(embedding_config)
        .context("Failed to create embedding pool")?;
    let embedding_provider = Arc::new(sagitta_search::EmbeddingPoolAdapter::new(Arc::new(embedding_pool)));
    
    // Initialize Qdrant Client
    println!("🔧 Connecting to Qdrant...");
    let qdrant_client_result = Qdrant::from_url(&sagitta_app_config.qdrant_url).build();
    let qdrant_client: Arc<dyn QdrantClientTrait> = match qdrant_client_result {
        Ok(client) => {
            println!("✓ Connected to Qdrant at {}", sagitta_app_config.qdrant_url);
            Arc::new(client)
        },
        Err(e) => {
            return Err(anyhow!("Failed to connect to Qdrant at {}: {}. Tool analysis will be disabled.", 
                              sagitta_app_config.qdrant_url, e));
        }
    };
    
    // Create and register tools
    println!("🔧 Setting up tools...");
    let tool_registry = Arc::new(sagitta_code::tools::registry::ToolRegistry::new());
    
    // Create the LLM client for tools
    let llm_client: Arc<dyn LlmClient> = Arc::new(
        OpenRouterClient::new(&config)
            .map_err(|e| anyhow!("Failed to create OpenRouterClient: {}", e))?
    );
    
    // Register AnalyzeInputTool
    tool_registry.register(Arc::new(
        sagitta_code::tools::analyze_input::AnalyzeInputTool::new(
            tool_registry.clone(), 
            embedding_provider.clone(), 
            qdrant_client.clone()
        )
    )).await.context("Failed to register AnalyzeInputTool")?;

    // Register shell execution tools
    let default_working_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    
    tool_registry.register(Arc::new(sagitta_code::tools::shell_execution::ShellExecutionTool::new(
        default_working_dir.clone()
    ))).await.context("Failed to register shell execution tool")?;

    // Note: Project creation and test execution functionality is now available through shell_execution tool
    // Examples:
    // - Project creation: Use shell_execution with commands like "cargo init my-project", "npm init", "python -m venv myenv"  
    // - Test execution: Use shell_execution with commands like "cargo test", "npm test", "pytest", "go test"
    
    // Setup Qdrant tool collection
    println!("🔧 Setting up Qdrant tool collection...");
    setup_qdrant_tool_collection(&tool_registry, &qdrant_client, &embedding_provider).await?;
    
    // Create persistence and search engine
    println!("🔧 Setting up conversation persistence...");
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
            .context("Failed to create disk conversation persistence")?
    );
    
    let search_engine: Box<dyn sagitta_code::agent::conversation::search::ConversationSearchEngine> = Box::new(
        sagitta_code::agent::conversation::search::text::TextConversationSearchEngine::new()
    );
    
    // Create the agent
    println!("🔧 Creating agent...");
    let agent = Agent::new(
        config.clone(), 
        tool_registry.clone(), 
        embedding_provider.clone(),
        persistence,
        search_engine,
        llm_client.clone()
    ).await.context("Failed to create agent")?;
    
    println!("✅ Agent initialization complete!");
    Ok(agent)
}

async fn setup_qdrant_tool_collection(
    tool_registry: &Arc<sagitta_code::tools::registry::ToolRegistry>,
    qdrant_client: &Arc<dyn QdrantClientTrait>,
    embedding_provider: &Arc<sagitta_search::EmbeddingPoolAdapter>,
) -> Result<()> {
    let vector_size = embedding_provider.dimension() as u64;
    
    // Check if collection exists
    match qdrant_client.collection_exists(TOOLS_COLLECTION_NAME.to_string()).await {
        Ok(exists) => {
            if !exists {
                println!("📦 Creating Qdrant tool collection: {}", TOOLS_COLLECTION_NAME);
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
                qdrant_client.create_collection_detailed(create_collection_request).await
                    .context("Failed to create Qdrant tool collection")?;
            } else {
                println!("✓ Qdrant tool collection '{}' already exists.", TOOLS_COLLECTION_NAME);
            }
        }
        Err(e) => {
            return Err(anyhow!("Failed to check Qdrant tool collection '{}': {}", TOOLS_COLLECTION_NAME, e));
        }
    }
    
    // Populate tool definitions
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
                        qdrant_client::qdrant::PointId::from(idx as u64),
                        qdrant_client::qdrant::NamedVectors::default()
                            .add_vector("dense", embedding), 
                        qdrant_client::Payload::from(payload_map)
                    ));
                }
            }
            Err(e) => {
                println!("⚠️  Failed to generate embedding for tool '{}': {}", tool_def.name, e);
            }
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
        qdrant_client.upsert_points(upsert_request).await
            .context("Failed to upsert tool definitions to Qdrant")?;
        println!("✓ Tool definitions populated in Qdrant");
    }
    
    Ok(())
}

fn print_help() {
    println!("📚 Available Commands:");
    println!("  help              - Show this help message");
    println!("  exit, quit        - Exit the chat");
    println!("  clear             - Clear conversation history");
    println!("  mode auto         - Set to fully autonomous mode");
    println!("  mode confirm      - Set to tools with confirmation mode");
    println!("  mode chat         - Set to chat-only mode");
    println!();
    println!("💡 Tips:");
    println!("  - The agent can use various tools for development tasks");
    println!("  - Real-time streaming shows thoughts and tool executions");
    println!("  - Use Ctrl+C for immediate exit if needed");
    println!("  - Tool confirmations appear in 'confirm' mode");
    println!();
} 