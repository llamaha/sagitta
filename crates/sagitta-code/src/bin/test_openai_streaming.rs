use anyhow::Result;
use clap::Parser;
use futures_util::StreamExt;
use std::sync::Arc;
use sagitta_code::providers::{ProviderFactory, ProviderType, ProviderConfig};
use sagitta_code::providers::claude_code::mcp_integration::McpIntegration;
use sagitta_code::llm::client::{Message, MessagePart, Role, LlmClient, ToolDefinition};
use sagitta_code::agent::message::types::ToolCall;
use uuid::Uuid;
use serde_json::json;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The prompt to send to the model
    #[arg(short, long, default_value = "Please read the file /home/adam/repos/sagitta/README.md and tell me what the project is about")]
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
        run_with_agent(Arc::from(client), args.prompt, args.interactive).await?;
    } else {
        // Use direct LLM client (original behavior) 
        run_direct_llm(Arc::from(client), args.prompt, args.interactive).await?;
    }
    
    Ok(())
}

async fn run_with_agent(
    client: Arc<dyn LlmClient>,
    initial_prompt: String,
    interactive: bool,
) -> Result<()> {
    log::info!("=== Running with Agent (with proper tool execution) ===");
    log::info!("Note: Using simplified approach focused on tool execution testing");
    
    // For now, use the direct LLM approach with proper tool execution
    // TODO: Implement full Agent integration once we get tool execution working
    run_direct_llm_with_tool_execution(client, initial_prompt, interactive).await
}

async fn run_direct_llm_with_tool_execution(
    client: Arc<dyn LlmClient>,
    initial_prompt: String,
    interactive: bool,
) -> Result<()> {
    log::info!("=== Running with direct LLM client + proper tool execution ===");
    
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

    // Define some tools including read_file to test streaming issue
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
            name: "read_file".to_string(),
            description: "Reads a specific range of lines from a file. You MUST specify both start_line and end_line (1-based line numbers). Maximum 400 lines per request. Example: to read first 100 lines use start_line=1, end_line=100.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": { 
                        "type": "string", 
                        "description": "The absolute path to the file to read" 
                    },
                    "start_line": { 
                        "type": "integer", 
                        "description": "REQUIRED: Line number to start reading from (1-based, inclusive). Example: 1 for first line" 
                    },
                    "end_line": { 
                        "type": "integer", 
                        "description": "REQUIRED: Line number to stop reading at (1-based, inclusive). Maximum range is 400 lines. Example: 100 to read up to line 100" 
                    }
                },
                "required": ["file_path", "start_line", "end_line"]
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
            name: "shell_execute".to_string(),
            description: "Executes shell commands with cross-platform support.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to execute" },
                    "working_directory": { "type": "string", "description": "Optional: Leave empty to use current repository. Can be: repository name (e.g. 'sagitta'), relative path (e.g. 'src/'), or absolute path. Usually not needed - just use paths in your command like 'ls src/'" },
                    "timeout_ms": { "type": "integer", "description": "Optional timeout in milliseconds" }
                },
                "required": ["command"]
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
                        MessagePart::ToolCall { tool_call_id, name, parameters } => {
                            log::info!("\n🔧 Tool call detected: {}", name);
                            tool_calls.push(ToolCall {
                                id: tool_call_id.clone(),
                                name: name.clone(),
                                arguments: parameters.clone(),
                                result: None,
                                successful: false,
                                execution_time: None,
                            });
                        }
                        MessagePart::Thought { text } => {
                            log::debug!("[Thinking] {}", text);
                        }
                        _ => {}
                    }
                    
                    // Note: StreamChunk doesn't have message_id field, using UUID per chunk
                    current_message_id = Uuid::new_v4();
                    
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
                tool_call_id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                parameters: tool_call.arguments.clone(),
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
                log::info!("🚀 Executing tool: {} with args: {}", tool_call.name, tool_call.arguments);
                
                // CRITICAL FIX: Execute tools via MCP like the GUI does
                let result_json = match execute_mcp_tool(&tool_call.name, tool_call.arguments).await {
                    Ok(result) => {
                        log::info!("✅ Tool {} executed successfully", tool_call.name);
                        log::debug!("   Result: {}", serde_json::to_string_pretty(&result).unwrap_or_default());
                        result
                    },
                    Err(e) => {
                        log::error!("❌ Tool {} execution failed: {}", tool_call.name, e);
                        json!({ "error": e.to_string() })
                    }
                };
                
                // Add tool result message to conversation history (CRITICAL FIX)
                messages.push(Message {
                    id: Uuid::new_v4(),
                    role: Role::Function,
                    parts: vec![MessagePart::ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        name: tool_call.name.clone(),
                        result: result_json.clone(),
                    }],
                    metadata: Default::default(),
                });
                
                log::info!("📝 Added tool result to conversation history");
                log::debug!("Tool result: {}", serde_json::to_string_pretty(&result_json)?);
            }
            
            log::info!("\n=== All tools executed, continuing conversation ===");
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
    log::info!("Total messages in history: {}", messages.len());
    
    Ok(())
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
            name: "read_file".to_string(),
            description: "Reads a specific range of lines from a file. You MUST specify both start_line and end_line (1-based line numbers). Maximum 400 lines per request. Example: to read first 100 lines use start_line=1, end_line=100.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": { 
                        "type": "string", 
                        "description": "The absolute path to the file to read" 
                    },
                    "start_line": { 
                        "type": "integer", 
                        "description": "REQUIRED: Line number to start reading from (1-based, inclusive). Example: 1 for first line" 
                    },
                    "end_line": { 
                        "type": "integer", 
                        "description": "REQUIRED: Line number to stop reading at (1-based, inclusive). Maximum range is 400 lines. Example: 100 to read up to line 100" 
                    }
                },
                "required": ["file_path", "start_line", "end_line"]
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
                        MessagePart::ToolCall { tool_call_id, name, parameters } => {
                            log::info!("\n🔧 Tool call detected: {}", name);
                            tool_calls.push(ToolCall {
                                id: tool_call_id.clone(),
                                name: name.clone(),
                                arguments: parameters.clone(),
                                result: None,
                                successful: false,
                                execution_time: None,
                            });
                        }
                        MessagePart::Thought { text } => {
                            log::debug!("[Thinking] {}", text);
                        }
                        _ => {}
                    }
                    
                    // Note: StreamChunk doesn't have message_id field, using UUID per chunk
                    current_message_id = Uuid::new_v4();
                    
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
                tool_call_id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                parameters: tool_call.arguments.clone(),
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
                        let args = &tool_call.arguments;
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
                    role: Role::Function,
                    parts: vec![MessagePart::ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        name: tool_call.name.clone(),
                        result: result.clone(),
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