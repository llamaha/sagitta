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
    log::info!("=== Running with Agent (with proper tool execution) ===");
    
    // Create tool registry
    let tool_registry = Arc::new(ToolRegistry::new());
    
    // Create event channel
    let (tx, rx) = broadcast::channel(1000);
    
    // Build the agent
    let agent = AgentBuilder::new(client)
        .with_tool_registry(tool_registry)
        .build();
    
    // Subscribe to agent events
    agent.subscribe_to_events(tx);
    
    // Create test CLI with tool execution capabilities
    let mut test_cli = TestCLI::new(agent, rx).await;
    
    // Process initial message
    log::info!("\nUser: {}", initial_prompt);
    log::info!("Assistant:");
    
    test_cli.process_message(&initial_prompt).await?;
    
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
            
            test_cli.process_message(input).await?;
        }
    }
    
    Ok(())
}

/// Test CLI that handles tool execution like the GUI
struct TestCLI {
    agent: Arc<sagitta_code::agent::core::Agent>,
    event_receiver: broadcast::Receiver<sagitta_code::agent::events::AgentEvent>,
    active_tool_calls: HashMap<String, String>, // tool_call_id -> message_id
}

impl TestCLI {
    async fn new(
        agent: Arc<sagitta_code::agent::core::Agent>,
        event_receiver: broadcast::Receiver<sagitta_code::agent::events::AgentEvent>,
    ) -> Self {
        Self {
            agent,
            event_receiver,
            active_tool_calls: HashMap::new(),
        }
    }
    
    async fn process_message(&mut self, input: &str) -> Result<()> {
        // Start the agent processing
        let stream = self.agent.process_message_stream(input).await?;
        
        // Handle stream and events concurrently
        tokio::pin!(stream);
        loop {
            tokio::select! {
                // Process stream chunks
                chunk_result = stream.next() => {
                    match chunk_result {
                        Some(Ok(chunk)) => {
                            if chunk.is_final {
                                log::debug!("\n✅ Stream complete: {:?}", chunk.finish_reason);
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            log::error!("Stream error: {}", e);
                            return Err(anyhow::anyhow!("Stream error: {}", e));
                        }
                        None => {
                            log::debug!("Stream ended");
                            break;
                        }
                    }
                }
                
                // Handle agent events
                event_result = self.event_receiver.recv() => {
                    match event_result {
                        Ok(event) => {
                            if let Err(e) = self.handle_agent_event(event).await {
                                log::error!("Error handling agent event: {}", e);
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            log::debug!("Event channel closed");
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            log::warn!("Skipped {} events due to lag", skipped);
                        }
                    }
                }
            }
        }
        
        println!(); // New line after response
        Ok(())
    }
    
    async fn handle_agent_event(&mut self, event: sagitta_code::agent::events::AgentEvent) -> Result<()> {
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
            
            sagitta_code::agent::events::AgentEvent::ToolCall { tool_call, message_id } => {
                log::info!("🛠️  Tool call received: {} (id: {})", tool_call.name, tool_call.id);
                self.execute_tool(tool_call, message_id).await?;
            }
            
            sagitta_code::agent::events::AgentEvent::ToolCallComplete { tool_call_id, tool_name, result } => {
                log::info!("✅ Tool call complete: {} (id: {})", tool_name, tool_call_id);
                self.handle_tool_completion(tool_call_id, tool_name, result).await?;
            }
            
            sagitta_code::agent::events::AgentEvent::StateChanged(state) => {
                log::debug!("🔄 Agent state: {:?}", state);
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    async fn execute_tool(
        &mut self,
        tool_call: sagitta_code::agent::message::types::ToolCall,
        message_id: String,
    ) -> Result<()> {
        // Store active tool call
        self.active_tool_calls.insert(tool_call.id.clone(), message_id);
        
        let tool_call_id = tool_call.id.clone();
        let tool_name = tool_call.name.clone();
        let agent = self.agent.clone();
        
        // Execute tool in background using same logic as GUI
        tokio::spawn(async move {
            log::info!("🚀 Executing tool {} through MCP", tool_name);
            
            let (success, result_json) = match execute_mcp_tool(&tool_name, tool_call.arguments).await {
                Ok(result) => {
                    log::info!("✅ Tool {} executed successfully", tool_name);
                    log::debug!("   Result: {}", serde_json::to_string_pretty(&result).unwrap_or_default());
                    (true, result)
                },
                Err(e) => {
                    log::error!("❌ Tool {} execution failed: {}", tool_name, e);
                    (false, serde_json::json!({
                        "error": e.to_string()
                    }))
                }
            };
            
            // Add tool result to conversation history (CRITICAL FIX)
            if let Err(e) = agent.add_tool_result_to_history(&tool_call_id, &tool_name, &result_json).await {
                log::error!("Failed to add tool result to history: {}", e);
            } else {
                log::info!("📝 Added tool result to conversation history");
            }
            
            // TODO: Trigger continuation if this was the last tool
            // For now, we'll rely on the agent to continue automatically
        });
        
        Ok(())
    }
    
    async fn handle_tool_completion(
        &mut self,
        tool_call_id: String,
        tool_name: String,
        result: sagitta_code::agent::events::ToolResult,
    ) -> Result<()> {
        // Remove from active tools
        self.active_tool_calls.remove(&tool_call_id);
        
        match result {
            sagitta_code::agent::events::ToolResult::Success { output } => {
                log::info!("✅ Tool {} completed successfully", tool_name);
                log::debug!("   Result: {}", output);
            }
            sagitta_code::agent::events::ToolResult::Error { error } => {
                log::error!("❌ Tool {} failed: {}", tool_name, error);
            }
        }
        
        // If all tools are complete, trigger continuation
        if self.active_tool_calls.is_empty() {
            log::info!("🔄 All tools complete, triggering continuation...");
            
            // Trigger continuation with empty message like the GUI does
            let agent = self.agent.clone();
            tokio::spawn(async move {
                match agent.process_message_stream("").await {
                    Ok(mut stream) => {
                        log::info!("📡 Processing continuation stream...");
                        
                        // Process the continuation stream
                        while let Some(chunk_result) = stream.next().await {
                            match chunk_result {
                                Ok(chunk) => {
                                    // Handle continuation chunks
                                    match &chunk.part {
                                        MessagePart::Text { text } => print!("{}", text),
                                        MessagePart::ToolCall { name, .. } => {
                                            log::info!("\n🔧 Continuation tool call: {}", name);
                                        }
                                        _ => {}
                                    }
                                    
                                    if chunk.is_final {
                                        log::debug!("\n✅ Continuation complete: {:?}", chunk.finish_reason);
                                        break;
                                    }
                                }
                                Err(e) => {
                                    log::error!("Continuation stream error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to create continuation stream: {}", e);
                    }
                }
            });
        }
        
        Ok(())
    }
}

/// Execute MCP tool (copied from GUI events.rs)
async fn execute_mcp_tool(tool_name: &str, arguments: serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    use sagitta_mcp::handlers::tool::handle_tools_call;
    use sagitta_mcp::mcp::types::CallToolParams;
    use sagitta_search::config::load_config;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use qdrant_client::Qdrant;
    
    log::debug!("Executing tool {} with arguments: {}", tool_name, serde_json::to_string_pretty(&arguments)?);
    
    // Load the sagitta search config
    let config = load_config(None).map_err(|e| format!("Failed to load config: {}", e))?;
    let config = Arc::new(RwLock::new(config));
    
    // Create Qdrant client
    let qdrant_url = {
        let cfg = config.read().await;
        cfg.qdrant_url.clone()
    };
    let qdrant_client = Qdrant::from_url(&qdrant_url).build()
        .map_err(|e| format!("Failed to create Qdrant client: {}", e))?;
    let qdrant_client = Arc::new(qdrant_client);
    
    // Create the tool call params
    let params = CallToolParams {
        name: tool_name.to_string(),
        arguments,
    };
    
    // Execute the tool directly using the MCP handler
    match handle_tools_call(params, config, qdrant_client).await {
        Ok(Some(result)) => {
            log::debug!("Tool {} executed successfully, raw result: {}", tool_name, 
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| "unparseable".to_string()));
            
            // Extract the actual content from the MCP result structure
            // MCP returns: { "content": [{"text": "...", "type": "text"}], "isError": false }
            if let Some(content_array) = result.get("content").and_then(|v| v.as_array()) {
                if let Some(first_content) = content_array.first() {
                    if let Some(text) = first_content.get("text").and_then(|v| v.as_str()) {
                        // Try to parse the text as JSON, otherwise return it as a string
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                            log::debug!("Parsed tool result as JSON");
                            Ok(parsed)
                        } else {
                            log::debug!("Returning tool result as text");
                            Ok(serde_json::json!({ "result": text }))
                        }
                    } else {
                        // No text field, return the whole content block
                        Ok(first_content.clone())
                    }
                } else {
                    // Empty content array
                    Ok(serde_json::json!({}))
                }
            } else {
                // No content field, return the whole result
                log::warn!("MCP result doesn't have expected 'content' field, returning raw result");
                Ok(result)
            }
        }
        Ok(None) => {
            log::debug!("Tool {} executed successfully with no result", tool_name);
            Ok(serde_json::json!({}))
        }
        Err(e) => {
            log::error!("Tool {} execution failed: {:?}", tool_name, e);
            Err(format!("Tool execution failed: {:?}", e).into())
        }
    }
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