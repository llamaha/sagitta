use anyhow::Result;
use clap::Parser;
use futures_util::StreamExt;
use std::sync::Arc;
use std::collections::HashMap;

use sagitta_code::providers::{ProviderFactory, ProviderType, ProviderConfig};
use sagitta_code::providers::claude_code::mcp_integration::McpIntegration;
use sagitta_code::llm::client::{Message, MessagePart, Role, LlmClient, ToolDefinition, ToolCall};
use sagitta_code::agent::AgentBuilder;
use sagitta_code::tools::registry::ToolRegistry;
use sagitta_code::utils::errors::SagittaCodeError;
use uuid::Uuid;
use serde_json::json;
use tokio::sync::broadcast;

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

    /// Enable interactive mode for tool chaining tests
    #[arg(short, long)]
    interactive: bool,

    /// Use Agent instead of direct LLM client (better for tool chaining)
    #[arg(short = 'g', long)]
    use_agent: bool,
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
    log::info!("Mode: {}", if args.use_agent { "Agent" } else { "Direct LLM" });

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
    let client = provider.create_client(&provider_config, mcp_integration.clone())?;
    
    log::info!("Created OpenAI-compatible LLM client");

    if args.use_agent {
        // Use the Agent for better tool handling
        run_with_agent(client, args.prompt, args.interactive).await?;
    } else {
        // Use direct LLM client (original behavior) 
        run_direct_llm(client, args.prompt, args.interactive).await?;
    }
    
    Ok(())
}

async fn run_with_agent(
    client: Arc<dyn LlmClient>,
    initial_prompt: String,
    interactive: bool,
) -> Result<()> {
    log::info!("=== Running with Agent (better tool handling) ===");
    
    // Create tool registry
    let tool_registry = Arc::new(ToolRegistry::new());
    
    // Create event channel
    let (tx, mut rx) = broadcast::channel(1000);
    
    // Build the agent
    let agent = AgentBuilder::new(client)
        .with_tool_registry(tool_registry)
        .build();
    
    // Subscribe to agent events
    agent.subscribe_to_events(tx);
    
    // Handle events in background
    let event_handle = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                sagitta_code::agent::events::AgentEvent::LlmChunk { chunk } => {
                    match &chunk.part {
                        MessagePart::Text { text } => print!("{}", text),
                        MessagePart::ToolCall { name, .. } => {
                            log::info!("\n🔧 Tool call: {}", name);
                        }
                        MessagePart::Thought { text } => {
                            log::debug!("💭 Thinking: {}", text);
                        }
                        _ => {}
                    }
                }
                sagitta_code::agent::events::AgentEvent::ToolCallExecuting { tool_call } => {
                    log::info!("⚙️  Executing tool: {} with args: {}", 
                        tool_call.name, 
                        tool_call.arguments
                    );
                }
                sagitta_code::agent::events::AgentEvent::ToolCallComplete { tool_name, result, .. } => {
                    match result {
                        sagitta_code::agent::events::ToolResult::Success { output } => {
                            log::info!("✅ Tool {} completed successfully", tool_name);
                            log::debug!("   Result: {}", output);
                        }
                        sagitta_code::agent::events::ToolResult::Error { error } => {
                            log::error!("❌ Tool {} failed: {}", tool_name, error);
                        }
                    }
                }
                sagitta_code::agent::events::AgentEvent::StateChanged(state) => {
                    log::debug!("🔄 Agent state: {:?}", state);
                }
                _ => {}
            }
        }
    });
    
    // Process initial message
    log::info!("\nUser: {}", initial_prompt);
    log::info!("Assistant:");
    
    let mut stream = agent.process_message_stream(&initial_prompt).await?;
    
    // Process stream
    tokio::pin!(stream);
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                if chunk.is_final {
                    log::debug!("\n✅ Stream complete: {:?}", chunk.finish_reason);
                    break;
                }
            }
            Err(e) => {
                log::error!("Stream error: {}", e);
                return Err(anyhow::anyhow!("Stream error: {}", e));
            }
        }
    }
    
    println!(); // New line after response
    
    if interactive {
        // Interactive mode for testing tool chaining
        loop {
            log::info!("\nEnter next prompt (or 'quit' to exit):");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim();
            
            if input == "quit" || input == "exit" {
                break;
            }
            
            log::info!("\nUser: {}", input);
            log::info!("Assistant:");
            
            let mut stream = agent.process_message_stream(input).await?;
            
            tokio::pin!(stream);
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if chunk.is_final {
                            log::debug!("\n✅ Stream complete: {:?}", chunk.finish_reason);
                            break;
                        }
                    }
                    Err(e) => {
                        log::error!("Stream error: {}", e);
                        break;
                    }
                }
            }
            
            println!(); // New line after response
        }
    }
    
    // Cancel event handler
    event_handle.abort();
    
    Ok(())
}

async fn run_direct_llm(
    client: Arc<dyn LlmClient>, 
    initial_prompt: String,
    interactive: bool,
) -> Result<()> {
    log::info!("=== Running with direct LLM client ===");
    
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
                text: initial_prompt.clone()
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
    log::info!("User: {}", initial_prompt);

    // Run conversation loop
    loop {
        // Generate stream
        let mut stream = client.generate_stream(&messages, &tools).await?;
        
        log::info!("Streaming response...");
        
        let mut chunk_count = 0;
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut response_text = String::new();
        let mut current_message_id = Uuid::new_v4();
        
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
                            response_text.push_str(text);
                        }
                        MessagePart::ToolCall { id, name, arguments } => {
                            log::info!("\n🔧 Tool call detected: {}", name);
                            tool_calls.push(ToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                arguments: arguments.clone(),
                            });
                        }
                        MessagePart::Thought { text } => {
                            log::debug!("[Thinking] {}", text);
                        }
                        _ => {}
                    }
                    
                    if let Some(id) = chunk.message_id {
                        current_message_id = id;
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
        
        // Add assistant response to messages
        let mut assistant_parts = vec![];
        if !response_text.is_empty() {
            assistant_parts.push(MessagePart::Text { text: response_text });
        }
        for tool_call in &tool_calls {
            assistant_parts.push(MessagePart::ToolCall {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            });
        }
        
        messages.push(Message {
            id: current_message_id,
            role: Role::Assistant,
            parts: assistant_parts,
            metadata: Default::default(),
        });
        
        // If there were tool calls, execute them and continue
        if !tool_calls.is_empty() {
            log::info!("\n=== Executing {} tool calls ===", tool_calls.len());
            
            for tool_call in tool_calls {
                log::info!("Executing tool: {} with args: {}", tool_call.name, tool_call.arguments);
                
                // Simulate tool execution (in real scenario, would use MCP)
                let result = match tool_call.name.as_str() {
                    "ping" => json!({ "status": "ok", "message": "Server is responsive" }),
                    "repository_list" => json!({ 
                        "repositories": [
                            { "name": "sagitta", "status": "ready" },
                            { "name": "test-repo", "status": "ready" }
                        ]
                    }),
                    "semantic_code_search" => {
                        let args: serde_json::Value = serde_json::from_str(&tool_call.arguments)?;
                        json!({
                            "results": [
                                {
                                    "file": "example.py",
                                    "line": 42,
                                    "preview": "# Usage example for the API",
                                    "score": 0.95
                                }
                            ],
                            "query": args["queryText"],
                            "repository": args["repositoryName"]
                        })
                    }
                    _ => json!({ "error": "Unknown tool" })
                };
                
                // Add tool result message
                messages.push(Message {
                    id: Uuid::new_v4(),
                    role: Role::ToolResult,
                    parts: vec![MessagePart::ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        result: result.clone(),
                        is_error: result.get("error").is_some(),
                    }],
                    metadata: Default::default(),
                });
                
                log::info!("Tool result: {}", serde_json::to_string_pretty(&result)?);
            }
            
            log::info!("\n=== Continuing after tool execution ===");
            // Continue the loop to process tool results
            continue;
        }
        
        // No tool calls, check if we should continue interactively
        if interactive {
            log::info!("\nEnter next prompt (or 'quit' to exit):");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim();
            
            if input == "quit" || input == "exit" {
                break;
            }
            
            messages.push(Message {
                id: Uuid::new_v4(),
                role: Role::User,
                parts: vec![MessagePart::Text { text: input.to_string() }],
                metadata: Default::default(),
            });
            
            log::info!("\nUser: {}", input);
        } else {
            break;
        }
    }
    
    log::info!("\n=== Conversation complete ===");
    log::info!("Total messages: {}", messages.len());
    
    Ok(())
}