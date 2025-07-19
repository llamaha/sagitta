use anyhow::Result;
use clap::Parser;
use futures_util::StreamExt;
use std::sync::Arc;

use sagitta_code::providers::{ProviderFactory, ProviderType, ProviderConfig};
use sagitta_code::providers::claude_code::mcp_integration::McpIntegration;
use sagitta_code::llm::client::{Message, MessagePart, Role, LlmClient, ToolDefinition};
use uuid::Uuid;
use serde_json::json;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The prompt to send to the model
    #[arg(short, long, default_value = "Please ping the server and tell me if it responds")]
    prompt: String,

    /// API endpoint
    #[arg(short, long, default_value = "http://localhost:1234/v1")]
    endpoint: String,

    /// Model to use
    #[arg(short, long, default_value = "default")]
    model: String,

    /// API key (if required)
    #[arg(short, long, default_value = "sk-dummy")]
    api_key: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    if args.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
            .format_timestamp_millis()
            .init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
            .format_timestamp_millis()
            .init();
    }

    log::info!("Starting OpenAI streaming test CLI");
    log::info!("Endpoint: {}", args.endpoint);
    log::info!("Model: {}", args.model);

    // Create provider config for OpenAI-compatible
    let mut provider_config = ProviderConfig::default_for_provider(ProviderType::OpenAICompatible);
    provider_config.set_option("base_url", args.endpoint.clone()).unwrap();
    if !args.api_key.is_empty() && args.api_key != "sk-dummy" {
        provider_config.set_option("api_key", args.api_key.clone()).unwrap();
    }
    if !args.model.is_empty() && args.model != "default" {
        provider_config.set_option("model", args.model.clone()).unwrap();
    }
    provider_config.set_option("timeout_seconds", 120u64).unwrap();
    provider_config.set_option("max_retries", 3u32).unwrap();

    // Create provider factory
    let factory = ProviderFactory::new();
    
    // Create the OpenAI-compatible provider
    let provider = factory.create_provider(ProviderType::OpenAICompatible)?;
    
    // Create MCP integration
    let mcp_integration = Arc::new(McpIntegration::new());
    
    // Create the LLM client
    let client = provider.create_client(&provider_config, mcp_integration)?;
    
    log::info!("Created OpenAI-compatible LLM client");

    // Create messages
    let mut messages = vec![
        Message {
            id: Uuid::new_v4(),
            role: Role::System,
            parts: vec![MessagePart::Text {
                text: "You are a helpful AI assistant with access to tools. When you use tools, you should continue after receiving the results to complete the user's request.".to_string()
            }],
            metadata: Default::default(),
        },
        Message {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![MessagePart::Text {
                text: args.prompt.clone()
            }],
            metadata: Default::default(),
        }
    ];

    // Define some tools
    let tools = vec![
        ToolDefinition {
            name: "ping".to_string(),
            description: "Checks if the server is responsive.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "repository_list".to_string(),
            description: "Lists currently configured repositories.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "semantic_code_search".to_string(),
            description: "Performs semantic search on an indexed repository.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to query" },
                    "queryText": { "type": "string", "description": "The natural language query text" },
                    "limit": { "type": "integer", "description": "Maximum number of results to return" }
                },
                "required": ["repositoryName", "queryText", "limit"]
            }),
            is_required: false,
        },
    ];

    log::info!("\n=== Starting conversation ===");
    log::info!("User: {}", args.prompt);

    // Generate stream
    let mut stream = client.generate_stream(&messages, &tools).await?;
    
    log::info!("Streaming response...");
    
    let mut chunk_count = 0;
    let mut has_tool_calls = false;
    
    // Process the stream
    tokio::pin!(stream);
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                chunk_count += 1;
                log::debug!("Chunk {} received: {:?}", chunk_count, chunk);
                
                match &chunk.part {
                    MessagePart::Text { text } => {
                        print!("{}", text);
                    }
                    MessagePart::ToolCall { name, .. } => {
                        has_tool_calls = true;
                        log::info!("\n🔧 Tool call detected: {}", name);
                    }
                    MessagePart::Thought { text } => {
                        log::debug!("[Thinking] {}", text);
                    }
                    _ => {}
                }
                
                if chunk.is_final {
                    log::info!("\n✅ Stream done. Finish reason: {:?}", chunk.finish_reason);
                    break;
                }
            }
            Err(e) => {
                log::error!("Stream error: {}", e);
                return Err(anyhow::anyhow!("Stream error: {}", e));
            }
        }
    }
    
    println!(); // New line after streaming
    
    log::info!("\n=== Conversation complete ===");
    log::info!("Total chunks received: {}", chunk_count);
    log::info!("Had tool calls: {}", has_tool_calls);
    
    // If there were tool calls, we would need to handle them and continue
    // but for this test we just want to verify the streaming works correctly
    
    Ok(())
}