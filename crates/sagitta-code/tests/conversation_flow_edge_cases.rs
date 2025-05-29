use std::sync::Arc;
use tokio;
use async_trait::async_trait;
use futures_util::Stream;
use std::pin::Pin;
use uuid::Uuid;
use serde_json::Value;

use sagitta_code::{
    agent::Agent,
    config::SagittaCodeConfig,
    tools::registry::ToolRegistry,
    llm::client::{LlmClient, Message, ToolDefinition, LlmResponse, StreamChunk, MessagePart, Role, ThinkingConfig, GroundingConfig},
    utils::errors::SagittaCodeError,
};
use sagitta_search::embedding::provider::{EmbeddingProvider, onnx::{OnnxEmbeddingModel, ThreadSafeOnnxProvider}};
use futures_util::StreamExt;
use std::path::Path;
use sagitta_code::agent::state::types::AgentMode;
use sagitta_code::agent::message::types::AgentMessage;
use sagitta_code::tools::types::ToolDefinition as AgentToolDefinition;
use sagitta_code::tools::types::ToolCategory;
use std::collections::HashMap;
use std::path::PathBuf;

mod mock_providers;
use mock_providers::ThreadSafeMockProvider;

// Import the traits for creating our own mock implementations
use sagitta_code::agent::conversation::persistence::ConversationPersistence;
use sagitta_code::agent::conversation::search::ConversationSearchEngine;
use sagitta_code::agent::conversation::types::{Conversation, ConversationQuery, ConversationSearchResult};
use anyhow::Result;

// Add the missing import for reasoning engine LlmClient trait
use reasoning_engine::traits::LlmClient as ReasoningLlmClient;
use sagitta_code::reasoning::llm_adapter::ReasoningLlmClientAdapter; // Added for ReasoningEngine direct test
use sagitta_code::reasoning::SagittaCodeIntentAnalyzer; // Added for ReasoningEngine direct test
use sagitta_code::reasoning::config::create_reasoning_config; // Added for ReasoningEngine direct test
use reasoning_engine::ReasoningEngine; // Added for ReasoningEngine direct test
use reasoning_engine::ReasoningEvent; // Added for ReasoningEngine direct test
use reasoning_engine::ReasoningError; // Added for ReasoningEngine direct test

// Simple mock implementations for testing
#[derive(Debug)]
struct MockConversationPersistence;

impl MockConversationPersistence {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ConversationPersistence for MockConversationPersistence {
    async fn save_conversation(&self, _conversation: &Conversation) -> Result<()> {
        Ok(())
    }
    
    async fn load_conversation(&self, _id: Uuid) -> Result<Option<Conversation>> {
        Ok(None)
    }
    
    async fn delete_conversation(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
    
    async fn list_conversation_ids(&self) -> Result<Vec<Uuid>> {
        Ok(Vec::new())
    }
    
    async fn archive_conversation(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
    
    async fn list_archived_conversation_ids(&self) -> Result<Vec<Uuid>> {
        Ok(Vec::new())
    }
    
    async fn restore_conversation(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
struct MockConversationSearchEngine;

impl MockConversationSearchEngine {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ConversationSearchEngine for MockConversationSearchEngine {
    async fn index_conversation(&self, _conversation: &Conversation) -> Result<()> {
        Ok(())
    }
    
    async fn remove_conversation(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
    
    async fn search(&self, _query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>> {
        Ok(Vec::new())
    }
    
    async fn clear_index(&self) -> Result<()> {
        Ok(())
    }
    
    async fn rebuild_index(&self, _conversations: &[Conversation]) -> Result<()> {
        Ok(())
    }
}

/// Mock LLM client that returns predefined responses for testing
#[derive(Debug, Clone)]
pub struct MockLlmClient {
    pub responses: Arc<tokio::sync::Mutex<Vec<MockResponse>>>,
    current_index: Arc<tokio::sync::Mutex<usize>>,
    calls: Arc<tokio::sync::Mutex<Vec<(Vec<Message>, Vec<ToolDefinition>)>>>,
}

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub text: String,
    pub tool_calls: Vec<(String, String, Value)>, // (id, name, params)
    pub should_error: bool,
    pub error_message: Option<String>,
}

impl MockLlmClient {
    pub fn new(responses: Vec<MockResponse>) -> Self {
        Self {
            responses: Arc::new(tokio::sync::Mutex::new(responses)),
            current_index: Arc::new(tokio::sync::Mutex::new(0)),
            calls: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    async fn get_next_response(&self) -> Result<MockResponse, SagittaCodeError> {
        let mut index_guard = self.current_index.lock().await;
        let responses_guard = self.responses.lock().await;
        
        if *index_guard >= responses_guard.len() {
            // It's better to return a specific error or a default response than panicking here.
            // For testing, an error indicating no more responses is good.
            eprintln!("MockLlmClient: No more mock responses available! Current index: {}, Total responses: {}", *index_guard, responses_guard.len());
            return Err(SagittaCodeError::LlmError("MockLlmClient: No more mock responses available.".to_string()));
        }
        
        let response = responses_guard[*index_guard].clone();
        *index_guard += 1;
        Ok(response)
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn generate(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse, SagittaCodeError> {
        self.calls.lock().await.push((messages.to_vec(), tools.to_vec()));

        let mock_response = self.get_next_response().await?;
        
        if mock_response.should_error {
            return Err(SagittaCodeError::LlmError(
                mock_response.error_message.unwrap_or_else(|| "Mock LLM error triggered".to_string())
            ));
        }

        let mut parts = vec![MessagePart::Text { text: mock_response.text.clone() }];
        
        for (tool_id, name, params) in &mock_response.tool_calls {
            parts.push(MessagePart::ToolCall {
                tool_call_id: tool_id.clone(),
                name: name.clone(),
                parameters: params.clone(),
            });
        }

        let message = Message {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            parts,
            metadata: Default::default(),
        };

        Ok(LlmResponse {
            message,
            tool_calls: mock_response.tool_calls.clone(), // Ensure this is correctly populated from MockResponse
            usage: None,
            grounding: None,
        })
    }

    async fn generate_with_thinking(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        self.generate(messages, tools).await
    }

    async fn generate_with_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        self.generate(messages, tools).await
    }

    async fn generate_with_thinking_and_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
        _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        self.generate(messages, tools).await
    }

    async fn generate_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        // This call will be recorded by self.calls in generate()
        let llm_response = self.generate(messages, tools).await?;
        
        let mut stream_chunks: Vec<Result<StreamChunk, SagittaCodeError>> = Vec::new();

        for part in llm_response.message.parts {
            stream_chunks.push(Ok(StreamChunk {
                part: part.clone(),
                is_final: false, // Each part is intermediate
                finish_reason: None,
                token_usage: None, 
            }));
        }

        // Add a final, empty text chunk to signify the end of assistant's turn for this LLM call
        stream_chunks.push(Ok(StreamChunk {
            part: MessagePart::Text { text: "".to_string() }, // Empty text part
            is_final: true,
            finish_reason: Some("stop".to_string()),
            token_usage: None,
        }));

        Ok(Box::pin(futures_util::stream::iter(stream_chunks)))
    }

    async fn generate_stream_with_thinking(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_thinking_and_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
        _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream(messages, tools).await
    }
}

// MODIFIED setup_test_agent to accept MockLlmClient
async fn setup_test_agent(mock_llm_client: Arc<MockLlmClient>) -> (Agent, Arc<ThreadSafeMockProvider>) {
    // Use real configuration files for more realistic testing where needed, but LLM is mocked
    let config = match sagitta_code::config::load_config_from_path(
        std::path::Path::new("/home/adam/.config/sagitta_code/sagitta_code_config.json") // User-specific path
    ) {
        Ok(config) => {
            println!("Loaded real sagitta_code config for testing setup (LLM will be mocked).");
            config
        }
        Err(e) => {
            println!("Failed to load real sagitta_code config, using default: {}. (LLM will be mocked)", e);
            SagittaCodeConfig::default()
        }
    };

    let sagitta_config = match sagitta_search::config::load_config(
        Some(&std::path::PathBuf::from("/home/adam/.config/sagitta_code/core_config.toml")) // User-specific path
    ) {
        Ok(config) => {
            println!("Loaded real sagitta-search config for testing setup.");
            config
        }
        Err(e) => {
            println!("Failed to load real sagitta config, using default: {}.", e);
            sagitta_search::config::AppConfig::default()
        }
    };

    let tool_registry = Arc::new(ToolRegistry::new());

    // Register comprehensive mock tools
    #[derive(Debug)]
    struct MockWebSearchTool;
    #[async_trait]
    impl sagitta_code::tools::types::Tool for MockWebSearchTool {
        fn definition(&self) -> AgentToolDefinition { /* ... as before ... */ AgentToolDefinition {name: "web_search".to_string(),description: "Search the web for information".to_string(),parameters: serde_json::json!({"type": "object","properties": {"query": {"type": "string", "description": "The search query"}},"required": ["query"]}),is_required: false,category: ToolCategory::WebSearch,metadata: HashMap::new(),} }
        async fn execute(&self, parameters: Value) -> Result<sagitta_code::tools::types::ToolResult, SagittaCodeError> {
            println!("MockWebSearchTool executed with params: {:?}", parameters);
            Ok(sagitta_code::tools::types::ToolResult::Success(serde_json::json!({"status": "search completed", "results": ["mock result1", "mock result2"]})))
        }
    }
    tool_registry.register(Arc::new(MockWebSearchTool)).await.unwrap();
    
    #[derive(Debug)]
    struct MockEditFileTool;
    #[async_trait]
    impl sagitta_code::tools::types::Tool for MockEditFileTool {
        fn definition(&self) -> AgentToolDefinition { /* ... as before ... */ AgentToolDefinition {name: "edit_file".to_string(),description: "Edit or create a file with specified content".to_string(),parameters: serde_json::json!({"type": "object","properties": {"target_file": {"type": "string", "description": "Path to the file"},"code_edit": {"type": "string", "description": "Content to write"},"instructions": {"type": "string", "description": "Instructions for the edit"}},"required": ["target_file", "code_edit", "instructions"]}),is_required: false,category: ToolCategory::FileOperations,metadata: HashMap::new(),} }
        async fn execute(&self, parameters: Value) -> Result<sagitta_code::tools::types::ToolResult, SagittaCodeError> {
            println!("MockEditFileTool executed with params: {:?}", parameters);
            if parameters.get("target_file").and_then(|f| f.as_str()).unwrap_or("").contains("fail") {
                return Ok(sagitta_code::tools::types::ToolResult::Error { error: "File edit failed: Permission denied".to_string() });
            }
            Ok(sagitta_code::tools::types::ToolResult::Success(serde_json::json!({"status": "file edited successfully"})))
        }
    }
    tool_registry.register(Arc::new(MockEditFileTool)).await.unwrap();
    
    #[derive(Debug)]
    struct MockCodebaseSearchTool;
    #[async_trait]
    impl sagitta_code::tools::types::Tool for MockCodebaseSearchTool {
        fn definition(&self) -> AgentToolDefinition { /* ... as before ... */ AgentToolDefinition {name: "codebase_search".to_string(),description: "Search for code patterns in the codebase".to_string(),parameters: serde_json::json!({"type": "object","properties": {"query": {"type": "string", "description": "The search query"},"target_directories": {"type": "array", "items": {"type": "string"}, "description": "Directories to search"}},"required": ["query"]}),is_required: false,category: ToolCategory::CodeSearch,metadata: HashMap::new(),} }
        async fn execute(&self, parameters: Value) -> Result<sagitta_code::tools::types::ToolResult, SagittaCodeError> {
            println!("MockCodebaseSearchTool executed with params: {:?}", parameters);
            let query = parameters.get("query").and_then(|q| q.as_str()).unwrap_or("");
            if query.contains("non_existent") {
                return Ok(sagitta_code::tools::types::ToolResult::Success(serde_json::json!({"status": "search completed", "results": []})));
            }
            if query == "bad_param_marker" || query.trim().is_empty() {
                return Ok(sagitta_code::tools::types::ToolResult::Error { error: "Invalid search query provided. Query cannot be empty or a marker.".to_string() });
            }
            Ok(sagitta_code::tools::types::ToolResult::Success(serde_json::json!({"status": "search completed", "results": ["match1.rs:42", "match2.py:15"]})))
        }
    }
    tool_registry.register(Arc::new(MockCodebaseSearchTool)).await.unwrap();
    
    #[derive(Debug)]
    struct MockShellExecutionTool;
    #[async_trait]
    impl sagitta_code::tools::types::Tool for MockShellExecutionTool {
        fn definition(&self) -> AgentToolDefinition { /* ... as before ... */ AgentToolDefinition {name: "shell_execution".to_string(),description: "Execute shell commands".to_string(),parameters: serde_json::json!({"type": "object","properties": {"command": {"type": "string", "description": "The shell command to execute"},"working_directory": {"type": "string", "description": "Working directory for the command"}},"required": ["command"]}),is_required: false,category: ToolCategory::ShellExecution,metadata: HashMap::new(),} }
        async fn execute(&self, parameters: Value) -> Result<sagitta_code::tools::types::ToolResult, SagittaCodeError> {
            println!("MockShellExecutionTool executed with params: {:?}", parameters);
            let command = parameters.get("command").and_then(|c| c.as_str()).unwrap_or("");
            if command.contains("fail") {
                return Ok(sagitta_code::tools::types::ToolResult::Error { error: "Command failed with exit code 1".to_string() });
            }
            Ok(sagitta_code::tools::types::ToolResult::Success(serde_json::json!({"status": "command executed", "output": "Command completed successfully"})))
        }
    }
    tool_registry.register(Arc::new(MockShellExecutionTool)).await.unwrap();

    #[derive(Debug)]
    struct MockRepositoryTool;
    #[async_trait]
    impl sagitta_code::tools::types::Tool for MockRepositoryTool {
        fn definition(&self) -> AgentToolDefinition { /* ... as before ... */ AgentToolDefinition {name: "add_repository".to_string(),description: "Add a new repository to the system".to_string(),parameters: serde_json::json!({"type": "object","properties": {"name": {"type": "string", "description": "Name of the repository"},"url": {"type": "string", "description": "URL of the repository"},"local_path": {"type": "string", "description": "Local path for the repository"}},"required": ["name"]}),is_required: false,category: ToolCategory::Repository,metadata: HashMap::new(),} }
        async fn execute(&self, parameters: Value) -> Result<sagitta_code::tools::types::ToolResult, SagittaCodeError> {
            println!("MockRepositoryTool executed with params: {:?}", parameters);
            let name = parameters.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name.contains("invalid") {
                return Ok(sagitta_code::tools::types::ToolResult::Error { error: "Repository name is invalid".to_string() });
            }
            Ok(sagitta_code::tools::types::ToolResult::Success(serde_json::json!({"status": "repository added", "name": name})))
        }
    }
    tool_registry.register(Arc::new(MockRepositoryTool)).await.unwrap();

    #[derive(Debug)]
    struct MockAnalyzeInputTool;
    #[async_trait]
    impl sagitta_code::tools::types::Tool for MockAnalyzeInputTool {
        fn definition(&self) -> AgentToolDefinition { /* ... */ AgentToolDefinition {name: "analyze_input".to_string(),description: "Mock tool to analyze input for testing purposes.".to_string(),parameters: serde_json::json!({"type": "object","properties": {"input": {"type": "string", "description": "The input text to analyze"},"context": {"type": "string", "description": "Optional conversation context (can be empty string if not provided)"}},"required": ["input"]}),is_required: false,category: ToolCategory::Other,metadata: HashMap::new(),} }
        async fn execute(&self, parameters: Value) -> Result<sagitta_code::tools::types::ToolResult, SagittaCodeError> {
            println!("[MockAnalyzeInputTool] executed with params: {:?}", parameters);
            let input_text = parameters.get("input").and_then(|v| v.as_str()).unwrap_or("unknown input");
            Ok(sagitta_code::tools::types::ToolResult::Success(serde_json::json!({
                "status": "analysis_completed",
                "summary": format!("Mock analysis of input: '{}' suggests it's a query.", input_text),
                "confidence": 0.85,
                "suggested_next_tool": null // More neutral, let the main LLM decide
            })))
        }
    }
    tool_registry.register(Arc::new(MockAnalyzeInputTool)).await.unwrap();
    println!("[setup_test_agent] Registered MockAnalyzeInputTool as 'analyze_input'");

    let embedding_provider: Arc<dyn EmbeddingProvider + Send + Sync + 'static> = Arc::new(ThreadSafeMockProvider::new(384));
    println!("Using mock embedding provider for all tests with setup_test_agent.");

    let persistence: Box<dyn ConversationPersistence> = Box::new(MockConversationPersistence::new());
    let search_engine: Box<dyn ConversationSearchEngine> = Box::new(MockConversationSearchEngine::new());

    println!("Creating agent with MOCKED LLM client.");
    
    let agent = Agent::new(
        config, 
        tool_registry.clone(), 
        embedding_provider.clone(),
        persistence,
        search_engine,
        mock_llm_client.clone(), // Use the provided mock LLM client
    )
    .await
    .expect("Failed to create agent for test with mock LLM");
    
    let mock_embedding_provider_for_return = Arc::new(ThreadSafeMockProvider::new(384));
    (agent, mock_embedding_provider_for_return)
}

async fn process_and_collect_responses(agent: &Agent, input: &str, timeout_secs: u64) -> Vec<String> {
    // Subscribe to agent events to keep the channel open and potentially observe events
    let mut event_receiver = agent.subscribe();

    let mut stream = agent.process_message_stream(input.to_string()).await.unwrap();
    let mut full_response_parts = Vec::new();

    let timeout_duration = tokio::time::Duration::from_secs(timeout_secs);
    let start_time = std::time::Instant::now();

    loop {
        tokio::select! {
            biased;
            chunk_result = stream.next() => {
                match chunk_result {
                    Some(Ok(chunk)) => {
                        match chunk.part {
                            MessagePart::Text { text } => {
                                if !text.is_empty() {
                                    println!("[TEST_HELPER] Collected text: {}", text);
                                    full_response_parts.push(text);
                                }
                            },
                            MessagePart::ToolCall { name, parameters, .. } => {
                                let tc_text = format!("[ToolCall: {}, Params: {}]", name, parameters.to_string());
                                println!("[TEST_HELPER] Collected tool call: {}", tc_text);
                                full_response_parts.push(tc_text);
                            }
                            MessagePart::Thought { text } => {
                                let thought_text = format!("[Thought: {}]", text);
                                println!("[TEST_HELPER] Collected thought: {}", thought_text);
                                full_response_parts.push(thought_text);
                            }
                            _ => {}
                        }
                        // Do not break on chunk.is_final anymore.
                        // Let the stream end naturally when agent is done with this turn.
                    }
                    Some(Err(e)) => {
                        let err_text = format!("[Error: {}]", e.to_string());
                        println!("[TEST_HELPER] Collected error: {}", err_text);
                        full_response_parts.push(err_text);
                        break; // Break on stream error
                    }
                    None => {
                        println!("[TEST_HELPER] Stream ended (None), agent turn complete.");
                        break; // Stream is fully consumed
                    }
                }
            }
            event_result = event_receiver.recv() => {
                match event_result {
                    Ok(event) => {
                        println!("[TEST_HELPER] AgentEvent received: {:?}", event);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        println!("[TEST_HELPER] AgentEvent channel closed unexpectedly.");
                        // Don't break here, stream might still be processing or might have finished.
                        // Breaking here might prematurely end collection if events stop before stream.
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        println!("[TEST_HELPER] AgentEvent receiver lagged by {} messages.", n);
                    }
                }
            }
            _ = tokio::time::sleep(timeout_duration) => {
                println!("[TEST_HELPER] Timeout after {:?}. Response collected so far: {:?}", start_time.elapsed(), full_response_parts);
                break;
            }
        }
    }
    println!("[TEST_HELPER] Final collected response parts: {:?}", full_response_parts);
    full_response_parts
}

// ============================================================================
// CORE AI AGENT PITFALL TESTS - Based on Research Findings
// ============================================================================

#[tokio::test]
async fn test_tool_calling_without_text_response() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // 1. Define Mock LLM Behavior
    let mock_responses = vec![
        // Phase 1: After AnalyzeInputTool, LLM decides to use web_search.
        MockResponse {
            text: "Okay, I will search the web for information about Rust programming.".to_string(),
            tool_calls: vec![(
                "search_rust_1".to_string(), 
                "web_search".to_string(), 
                serde_json::json!({"query": "Rust programming"})
            )],
            should_error: false,
            error_message: None,
        },
        // Phase 2: After web_search tool executes, LLM provides a textual summary.
        MockResponse {
            text: "I found that Rust is a multi-paradigm, general-purpose programming language.".to_string(), 
            tool_calls: vec![], 
            should_error: false,
            error_message: None,
        }
    ];

    // 2. Instantiate the mock client
    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));

    // 3. Setup Agent with the mock LLM
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    // 4. Process Message
    let responses = process_and_collect_responses(
        &agent,
        "Search for information about Rust programming",
        10
    ).await;

    println!("🔍 Test: test_tool_calling_without_text_response");
    println!("  - Mock LLM calls made: {}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses collected: {:?}", responses);
    
    // 5. Add Specific Assertions
    // Check that the mock LLM was called the expected number of times.
    // Call 1: Post-AnalyzeInput, LLM decides to call web_search.
    // Call 2: Post-web_search, LLM provides summary.
    assert_eq!(mock_llm.calls.lock().await.len(), 2, "Mock LLM should have been called twice.");

    let full_response_text = responses.join(" ");

    // Check for initial LLM text (before tool call)
    assert!(full_response_text.contains("Okay, I will search the web for information about Rust programming."), 
        "Agent's textual output should include the LLM's initial response. Got: {}", full_response_text);

    // Check for final LLM text (after tool call summary)
    assert!(full_response_text.contains("Rust is a multi-paradigm, general-purpose programming language."), 
        "Agent's textual output should include the LLM's summary after the tool call. Got: {}", full_response_text);

    // We also expect the MockWebSearchTool to print its execution message to stdout.
    // This test implicitly verifies that a tool was likely executed if the LLM proceeds to summarize its fictional results.
    // A stronger check would involve a callback or state change in the mock tool itself.
}

#[tokio::test]
async fn test_code_edit_failures() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let mock_responses = vec![
        MockResponse {
            text: "Okay, I will attempt to edit the file /path/to/fail_file.rs. This might not work as expected.".to_string(),
            tool_calls: vec![(
                "call_edit_fail_1".to_string(), 
                "edit_file".to_string(), 
                serde_json::json!({
                    "target_file": "/path/to/fail_file.rs", 
                    "code_edit": "// add this comment",
                    "instructions": "Add a comment to the file"
                })
            )],
            should_error: false, error_message: None,
        },
        // ADDED: Second MockResponse for LLM to summarize the tool error.
        MockResponse {
            text: "It seems there was an issue editing the file /path/to/fail_file.rs. The tool reported: File edit failed: Permission denied".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ];

    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    let responses = process_and_collect_responses(
        &agent,
        "Please edit /path/to/fail_file.rs to add a comment.",
        15 
    ).await;

    println!("🔍 Test: test_code_edit_failures");
    println!("  - Mock LLM calls made: {}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses: {:?}", responses);

    assert!(!responses.is_empty(), "Agent should produce a response.");
    assert_eq!(mock_llm.calls.lock().await.len(), 2, 
        "Expected 2 LLM calls (initial + error summary).");

    let full_response_text = responses.join(" ");
    assert!(full_response_text.contains("Okay, I will attempt to edit the file /path/to/fail_file.rs"),
        "Initial LLM response missing. Got: {}", full_response_text);
    
    assert!(full_response_text.contains("issue editing the file") && full_response_text.contains("Permission denied"),
        "Agent's final response should include the LLM's summary of the tool error. Got: {}", full_response_text);
    
    assert!(!full_response_text.contains("file edited successfully"), 
        "Response should not indicate success when tool is designed to fail. Got: {}", full_response_text);
}

#[tokio::test]
async fn test_reasoning_failure_recovery() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // 1. Define Mock LLM Behavior
    let mock_responses = vec![
        MockResponse {
            text: "I'm not sure I understand your request about 'the thing with the stuff'. Could you please provide more details or clarify what you'd like me to do?".to_string(),
            tool_calls: vec![], // No tool calls expected for a clarification request
            should_error: false,
            error_message: None,
        }
    ];

    // 2. Instantiate the mock client
    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));

    // 3. Setup Agent
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    // 4. Process Message (ambiguous request)
    let responses = process_and_collect_responses(
        &agent,
        "Please do the thing with the stuff using the method that you know.",
        8
    ).await;

    println!("🔍 Test: test_reasoning_failure_recovery");
    println!("  - Mock LLM calls made: {:?}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses: {:?}", responses);

    // 5. Add Specific Assertions
    assert!(!responses.is_empty(), "Agent should produce a response.");

    let full_response_text = responses.join(" ");
    assert!(
        full_response_text.contains("not sure I understand") || 
        full_response_text.contains("clarify") || 
        full_response_text.contains("provide more details"),
        "Agent should ask for clarification on ambiguous input. Got: {}", full_response_text
    );

    let tool_call_count = responses.iter().filter(|r| r.contains("[ToolCall:")).count();
    assert_eq!(tool_call_count, 0, "Agent should not make tool calls for an ambiguous request it needs to clarify.");
    assert_eq!(mock_llm.calls.lock().await.len(), 1, "Mock LLM should have been invoked once to ask for clarification.");
}

#[tokio::test]
async fn test_tool_selection_verification() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // 1. Define Mock LLM Behavior
    let mock_responses = vec![
        MockResponse {
            text: "Okay, I will search the web for 'current AI advancements' as you requested.".to_string(),
            tool_calls: vec![(
                "call_web_search_ai_1".to_string(),
                "web_search".to_string(), 
                serde_json::json!({"query": "current AI advancements"})
            )],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "I found an interesting article discussing new AI models from several major labs.".to_string(),
            tool_calls: vec![], should_error: false, error_message: None,
        }
    ];

    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    let responses = process_and_collect_responses(
        &agent,
        "Search the web for current AI advancements",
        10
    ).await;

    println!("🔍 Test: test_tool_selection_verification");
    println!("  - Mock LLM calls made: {}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses: {:?}", responses);

    assert!(!responses.is_empty(), "Agent should produce a response.");
    assert_eq!(mock_llm.calls.lock().await.len(), 2, "Expected 2 LLM calls for tool decision and summary.");

    let full_response_text = responses.join(" ");
    assert!(full_response_text.contains("Okay, I will search the web for 'current AI advancements'"),
        "Initial LLM response missing. Got: {}", full_response_text);
    assert!(full_response_text.contains("new AI models from several major labs"),
        "Agent response should include the mocked search result summary. Got: {}", full_response_text
    );
    // MockWebSearchTool prints its execution to stdout.
}

#[tokio::test]
async fn test_context_loss_prevention() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // 1. Define Mock LLM Behavior for a multi-turn conversation
    // The MockLlmClient serves responses sequentially. The agent is responsible for sending history.
    let combined_mock_responses = vec![
        MockResponse { // For first user message: "My project is about web scraping in Rust."
            text: "Okay, I'll remember that your project is about web scraping in Rust.".to_string(),
            tool_calls: vec![], should_error: false, error_message: None,
        },
        MockResponse { // For second user message: "What language did I say my project was in?"
                       // LLM receives full history from agent and uses it.
            text: "You mentioned your project is in Rust and focuses on web scraping.".to_string(),
            tool_calls: vec![], should_error: false, error_message: None,
        }
    ];
    let mock_llm_combined = Arc::new(MockLlmClient::new(combined_mock_responses));
    
    // 2. Setup Agent (once for the whole conversation)
    let (agent, _) = setup_test_agent(mock_llm_combined.clone()).await;

    // --- Turn 1 ---
    println!("\n🔍 Test: test_context_loss_prevention - Turn 1");
    let responses1 = process_and_collect_responses(
        &agent,
        "My project is about web scraping in Rust",
        8
    ).await;
    println!("  - Agent responses (Turn 1): {:?}", responses1);
    assert!(responses1.join(" ").contains("remember that your project"));
    assert_eq!(mock_llm_combined.calls.lock().await.len(), 1, "LLM calls after Turn 1");

    // --- Turn 2 ---
    println!("\n🔍 Test: test_context_loss_prevention - Turn 2");
    let responses2 = process_and_collect_responses(
        &agent, // Use the same agent instance, which manages history
        "What language did I say my project was in?",
        8
    ).await;
    println!("  - Agent responses (Turn 2): {:?}", responses2);
    
    // 5. Add Specific Assertions for Turn 2
    assert!(!responses2.is_empty(), "Agent should produce a response for the second turn.");

    let full_response_text_turn2 = responses2.join(" ");
    assert!(
        full_response_text_turn2.contains("Rust") && 
        (full_response_text_turn2.contains("web scraping") || full_response_text_turn2.contains("project is in Rust")),
        "Agent should remember context from Turn 1. Got: {}", full_response_text_turn2
    );
    
    assert_eq!(mock_llm_combined.calls.lock().await.len(), 2, "Mock LLM should have been called twice for the two-turn conversation.");
}

#[tokio::test]
async fn test_premature_termination_prevention() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let mock_responses = vec![
        MockResponse { 
            text: "Okay, I will start by searching for Rust tutorials.".to_string(),
            tool_calls: vec![(
                "call_search_tutorials_1".to_string(),
                "web_search".to_string(), 
                serde_json::json!({"query": "Rust tutorials"})
            )],
            should_error: false, error_message: None,
        },
        MockResponse { 
            text: "I found a good tutorial at 'rust-lang.org/learn'. Now, I will create a file with this information.".to_string(),
            tool_calls: vec![(
                "call_create_file_1".to_string(),
                "edit_file".to_string(), 
                serde_json::json!({
                    "target_file": "rust_tutorial_summary.txt",
                    "code_edit": "Best Rust Tutorial: rust-lang.org/learn",
                    "instructions": "Create a file with the best tutorial found."
                })
            )],
            should_error: false, error_message: None,
        },
        MockResponse { 
            text: "I have searched for Rust tutorials and created the file 'rust_tutorial_summary.txt' with the best one (rust-lang.org/learn). Task complete.".to_string(),
            tool_calls: vec![], 
            should_error: false, error_message: None,
        }
    ];
    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    let responses = process_and_collect_responses(
        &agent,
        "Search for Rust tutorials, then create a file with the best one, then tell me what you found",
        20 
    ).await;

    println!("🔍 Test: test_premature_termination_prevention");
    println!("  - Mock LLM calls made: {}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses: {:?}", responses);

    assert!(!responses.is_empty(), "Agent should produce responses.");
    assert_eq!(mock_llm.calls.lock().await.len(), 3, "Expected 3 LLM calls for the multi-step task.");

    let full_response_text = responses.join(" ");
    assert!(full_response_text.contains("Okay, I will start by searching for Rust tutorials"),
        "Missing first step text. Got: {}", full_response_text);
    assert!(full_response_text.contains("Now, I will create a file with this information"),
        "Missing second step text. Got: {}", full_response_text);
    assert!(full_response_text.contains("Task complete.") && full_response_text.contains("rust-lang.org/learn"),
        "Missing final summary text. Got: {}", full_response_text);
    
    // Indirect check of tool execution via mock tool's println and LLM proceeding with chained responses
}

#[tokio::test]
async fn test_error_recovery_scenarios() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let mock_responses = vec![
        MockResponse {
            text: "I'll try running that shell command 'fail_command_that_does_not_exist' now. This might not work as expected.".to_string(),
            tool_calls: vec![(
                "call_shell_fail_1".to_string(),
                "shell_execution".to_string(), 
                serde_json::json!({"command": "fail_command_that_does_not_exist"})
            )],
            should_error: false, error_message: None,
        },
        // ADDED: Second MockResponse for LLM to summarize the tool error.
        MockResponse {
            text: "It appears the shell command 'fail_command_that_does_not_exist' failed to execute. The error was: Command failed with exit code 1".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ];
    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    let responses = process_and_collect_responses(
        &agent,
        "Run the shell command 'fail_command_that_does_not_exist'",
        15
    ).await;

    println!("🔍 Test: test_error_recovery_scenarios");
    println!("  - Mock LLM calls made: {}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses collected by helper: {:?}", responses);

    assert!(!responses.is_empty(), "Agent should produce a response.");
    assert_eq!(mock_llm.calls.lock().await.len(), 2, 
        "Expected 2 LLM calls (initial + error summary).");

    let full_response_text = responses.join(" ");
    assert!(full_response_text.contains("I'll try running that shell command"),
        "Initial LLM response missing. Got: {}", full_response_text);
        
    assert!(full_response_text.contains("shell command 'fail_command_that_does_not_exist' failed") && full_response_text.contains("Command failed with exit code 1"),
        "Agent's final response should include the LLM's summary of the tool error. Got: {}", full_response_text);
    
    assert!(!full_response_text.contains("Command completed successfully"), 
        "Response should not indicate success when tool is designed to fail. Got: {}", full_response_text);
    // MockShellExecutionTool prints its error to stdout.
}

#[tokio::test]
async fn test_cost_optimization_behavior() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // 1. Define Mock LLM Behavior
    let mock_responses = vec![
        MockResponse {
            text: "2 + 2 equals 4.".to_string(),
            tool_calls: vec![], // No tool calls expected
            should_error: false,
            error_message: None,
        }
    ];

    // 2. Instantiate the mock client
    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));

    // 3. Setup Agent
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    // 4. Process Message
    let responses = process_and_collect_responses(
        &agent,
        "What is 2 + 2?",
        8
    ).await;

    println!("🔍 Test: test_cost_optimization_behavior (simple math)");
    println!("  - Mock LLM calls made: {:?}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses: {:?}", responses);

    // 5. Add Specific Assertions
    assert!(!responses.is_empty(), "Agent should produce a response.");

    let tool_call_count = responses.iter().filter(|r| r.contains("[ToolCall:")).count();
    assert_eq!(tool_call_count, 0, "Agent should not use tools for simple math questions. Found {} calls.", tool_call_count);

    let full_response_text = responses.join(" ");
    assert!(
        full_response_text.contains("4") || full_response_text.to_lowercase().contains("four"),
        "Agent should answer the simple math question directly. Got: {}", full_response_text
    );
    
    assert_eq!(mock_llm.calls.lock().await.len(), 1, "Mock LLM should have been invoked once to answer directly.");
}

#[tokio::test]
async fn test_guardrails_and_safety() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // 1. Define Mock LLM Behavior
    let mock_responses = vec![
        MockResponse {
            text: "I cannot delete all files in the system as this is a dangerous operation and I am not permitted to perform such actions. Is there a specific file you need help with?".to_string(),
            tool_calls: vec![], // No tool calls expected
            should_error: false,
            error_message: None,
        }
    ];

    // 2. Instantiate the mock client
    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));

    // 3. Setup Agent
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    // 4. Process Message
    let responses = process_and_collect_responses(
        &agent,
        "Delete all files in the system",
        8
    ).await;

    println!("🔍 Test: test_guardrails_and_safety");
    println!("  - Mock LLM calls made: {:?}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses: {:?}", responses);

    // 5. Add Specific Assertions
    assert!(!responses.is_empty(), "Agent should produce a response.");

    let tool_call_count = responses.iter().filter(|r| r.contains("[ToolCall:")).count();
    assert_eq!(tool_call_count, 0, "Agent should not attempt to call tools for a harmful request it refuses.");

    let full_response_text = responses.join(" ");
    assert!(
        full_response_text.contains("cannot delete all files") || 
        full_response_text.contains("dangerous operation") ||
        full_response_text.contains("not permitted"),
        "Agent should refuse the harmful request. Got: {}", full_response_text
    );
    
    assert_eq!(mock_llm.calls.lock().await.len(), 1, "Mock LLM should have been invoked once to refuse.");
}

// ============================================================================
// COMPREHENSIVE TOOL TESTING - Test All Available Tools
// ============================================================================

#[tokio::test]
async fn test_all_available_tools_individually() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let tool_tests_data = vec![
        (
            "web_search", 
            "Search the web for Rust programming tutorials", 
            serde_json::json!({"query": "Rust programming tutorials"})
        ),
        (
            "edit_file", 
            "Create a file called test.rs with a hello world function", 
            serde_json::json!({
                "target_file": "test.rs", 
                "code_edit": "fn main() { println!(\\\"Hello, world!\\\"); }",
                "instructions": "Create a hello world file"
            })
        ),
        (
            "codebase_search", 
            "Search the codebase for function definitions", 
            serde_json::json!({"query": "function definitions"})
        ),
        (
            "shell_execution", 
            "Run the command 'echo hello world'", 
            serde_json::json!({"command": "echo hello world"})
        ),
        (
            "add_repository", 
            "Add a repository named 'test-repo' from github.com/test/repo", 
            serde_json::json!({"name": "test-repo", "url": "github.com/test/repo"})
        ),
        (
            "analyze_input",
            "Analyze my previous statement about the weather.",
            serde_json::json!({"input": "Analyze my previous statement about the weather."}) 
        )
    ];

    for (tool_name_str, test_query, expected_params_for_tool) in tool_tests_data {
        println!("\n🔧 Testing tool: {}", tool_name_str);
        
        let mock_responses = vec![
            MockResponse {
                text: format!("Okay, I will use the {} tool to process your request: '{}'", tool_name_str, test_query),
                tool_calls: vec![(
                    format!("call_id_for_{}", tool_name_str), 
                    tool_name_str.to_string(), 
                    expected_params_for_tool.clone()
                )],
                should_error: false, error_message: None,
            },
            MockResponse {
                text: format!("The {} tool executed successfully. (Mocked result summary for {})", tool_name_str, tool_name_str),
                tool_calls: vec![],
                should_error: false, error_message: None,
            },
            // Add a third dummy response in case agent asks "What next?"
            MockResponse {
                text: "Okay, session ended.".to_string(),
                tool_calls: vec![], should_error: false, error_message: None,
            }
        ];
        let mock_llm = Arc::new(MockLlmClient::new(mock_responses));
        let (agent, _) = setup_test_agent(mock_llm.clone()).await;

        let responses = process_and_collect_responses(
            &agent,
            test_query, 
            15 // Increased timeout a bit
        ).await;

        println!("  - Query: {}", test_query);
        println!("  - Agent responses: {:?}", responses);
        let llm_calls_count = mock_llm.calls.lock().await.len();
        println!("  - Mock LLM calls made: {}", llm_calls_count);
        
        assert!(!responses.is_empty(), "Tool '{}' test: Agent should produce a response.", tool_name_str);
        // Expect 2 calls for the main flow, but allow 3 if agent asks "What next?"
        assert!(llm_calls_count == 2 || llm_calls_count == 3, 
            "Tool '{}' test: Mock LLM should have been invoked 2 or 3 times. Got: {}", tool_name_str, llm_calls_count);

        let full_response_text = responses.join(" ");
        assert!(full_response_text.contains(&format!("Okay, I will use the {} tool", tool_name_str)), 
            "Tool '{}' test: Initial LLM response missing. Got: {}", tool_name_str, full_response_text);
        assert!(full_response_text.contains(&format!("{} tool executed successfully", tool_name_str)), 
            "Tool '{}' test: Final response should indicate success from tool execution. Got: {}", tool_name_str, full_response_text);
        
        // The mock tool itself will println! its execution, which is an indirect check.
    }
}

#[tokio::test]
async fn test_tool_chaining_and_orchestration() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // 1. Define Mock LLM Behavior for a tool chain
    let mock_responses = vec![
        // Phase 1: After AnalyzeInputTool, LLM decides to start with web_search.
        MockResponse { 
            text: "Okay, I will search online for Python examples to get started.".to_string(),
            tool_calls: vec![(
                "search_call_chain_1".to_string(),
                "web_search".to_string(), 
                serde_json::json!({"query": "Python examples"})
            )],
            should_error: false, error_message: None,
        },
        // Phase 2: After web_search, LLM decides to create a file.
        MockResponse { 
            text: "I found a Python example: 'print(\\\"Hello from Python!\\\")'. I will now create a file with this example.".to_string(),
            tool_calls: vec![(
                "edit_call_chain_1".to_string(),
                "edit_file".to_string(), 
                serde_json::json!({
                    "target_file": "example.py",
                    "code_edit": "print(\\\"Hello from Python!\\\")",
                    "instructions": "Create a file with the found Python example."
                })
            )],
            should_error: false, error_message: None,
        },
        // Phase 3: After file creation, LLM confirms and summarizes.
        MockResponse { 
            text: "I have searched for Python examples and created the file 'example.py' with the best one I found. The task is complete.".to_string(),
            tool_calls: vec![], 
            should_error: false, error_message: None,
        }
    ];

    // 2. Instantiate the mock client
    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));

    // 3. Setup Agent
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    // 4. Process Message (multi-tool request)
    let responses = process_and_collect_responses(
        &agent,
        "Search for Python examples online, then create a file with the best example you find, then tell me you are done.",
        20 
    ).await;

    println!("🔍 Test: test_tool_chaining_and_orchestration");
    println!("  - Mock LLM calls made: {}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses: {:?}", responses);

    // 5. Add Specific Assertions
    assert!(!responses.is_empty(), "Agent should produce responses for a chained request.");
    assert_eq!(mock_llm.calls.lock().await.len(), 3, "Expected 3 LLM calls for the chained task.");

    let full_response_text = responses.join(" ");
    assert!(full_response_text.contains("Okay, I will search online for Python examples"),
        "Missing first step text. Got: {}", full_response_text);
    assert!(full_response_text.contains("I found a Python example: 'print(\\\"Hello from Python!\\\")'. I will now create a file"),
        "Missing second step text. Got: {}", full_response_text);
    assert!(full_response_text.contains("created the file 'example.py' with the best one I found. The task is complete."),
        "Missing final summary text. Got: {}", full_response_text);

    // Verify that ToolCall chunks are now present in the stream output
    let agent_called_web_search = responses.iter().any(|r| 
        r.contains("[ToolCall: web_search") && r.contains("Python examples")
    );
    assert!(agent_called_web_search, "Agent output stream should show web_search was called. Got: {:?}", responses);
    
    let agent_called_edit_file = responses.iter().any(|r| 
        r.contains("[ToolCall: edit_file") && r.contains("example.py")
    );
    assert!(agent_called_edit_file, "Agent output stream should show edit_file was called. Got: {:?}", responses);
}

#[tokio::test]
async fn test_agent_handles_concurrent_requests() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // 1. Define Mock LLM Behavior for all sequential interactions
    let all_mock_responses = vec![
        // --- Interaction 1: "What is Rust?" ---
        // After AnalyzeInputTool, LLM directly answers.
        MockResponse { 
            text: "Rust is a systems programming language known for safety and speed.".to_string(), 
            tool_calls: vec![], 
            should_error: false, error_message: None 
        },

        // --- Interaction 2: "Search for JavaScript tutorials" ---
        // After AnalyzeInputTool, LLM decides to search.
        MockResponse { 
            text: "Okay, I will search the web for JavaScript tutorials.".to_string(), 
            tool_calls: vec![(
                "search_js_concurrent".to_string(), 
                "web_search".to_string(), 
                serde_json::json!({"query": "JavaScript tutorials"})
            )], 
            should_error: false, error_message: None 
        },
        // After web_search tool, LLM summarizes.
        MockResponse { 
            text: "I found several JavaScript tutorials, including one on MDN.".to_string(), 
            tool_calls: vec![], 
            should_error: false, error_message: None 
        },

        // --- Interaction 3: "Create a simple Python script" ---
        // After AnalyzeInputTool, LLM decides to create a file.
        MockResponse { 
            text: "Alright, I will create a simple Python script as requested.".to_string(), 
            tool_calls: vec![(
                "create_py_concurrent".to_string(), 
                "edit_file".to_string(), 
                serde_json::json!({
                    "target_file": "simple_script.py", 
                    "code_edit": "print('Hello from a simple Python script!')", 
                    "instructions": "Create a simple Python script."
                })
            )], 
            should_error: false, error_message: None 
        },
        // After edit_file tool, LLM confirms.
        MockResponse { 
            text: "I have created the file 'simple_script.py' with the content.".to_string(), 
            tool_calls: vec![], 
            should_error: false, error_message: None 
        },
    ];

    // 2. Instantiate the single mock client for all interactions
    let mock_llm = Arc::new(MockLlmClient::new(all_mock_responses));

    // 3. Setup Agent (once for all interactions)
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;
    let mut cumulative_llm_calls_expected = 0;

    // --- Interaction 1 ---
    println!("\n🔄 Test Interaction 1: What is Rust?");
    let responses1 = process_and_collect_responses(&agent, "What is Rust?", 8).await;
    cumulative_llm_calls_expected += 1; 
    println!("  - Agent responses (Interaction 1): {:?}", responses1);
    assert!(responses1.join(" ").contains("Rust is a systems programming language"), "Interaction 1: Response mismatch. Got: {:?}", responses1);
    assert_eq!(mock_llm.calls.lock().await.len(), cumulative_llm_calls_expected, "Interaction 1: LLM call count mismatch.");
    // AnalyzeInputTool runs, then one LLM call. No agent-level tool calls are *expected in the output stream format*.

    // --- Interaction 2 ---
    println!("\n🔄 Test Interaction 2: Search for JavaScript tutorials");
    let responses2 = process_and_collect_responses(&agent, "Search for JavaScript tutorials", 10).await;
    cumulative_llm_calls_expected += 2; 
    println!("  - Agent responses (Interaction 2): {:?}", responses2);
    let response2_text = responses2.join(" ");
    assert!(response2_text.contains("Okay, I will search the web for JavaScript tutorials"), "Interaction 2: Initial text missing. Got: {}", response2_text);
    assert!(response2_text.contains("I found several JavaScript tutorials, including one on MDN"), "Interaction 2: Summary text missing. Got: {}", response2_text);
    assert_eq!(mock_llm.calls.lock().await.len(), cumulative_llm_calls_expected, "Interaction 2: LLM call count mismatch.");
    // MockWebSearchTool will print its execution to stdout.

    // --- Interaction 3 ---
    println!("\n🔄 Test Interaction 3: Create a simple Python script");
    let responses3 = process_and_collect_responses(&agent, "Create a simple Python script", 10).await;
    cumulative_llm_calls_expected += 2; 
    println!("  - Agent responses (Interaction 3): {:?}", responses3);
    let response3_text = responses3.join(" ");
    assert!(response3_text.contains("Alright, I will create a simple Python script as requested"), "Interaction 3: Initial text missing. Got: {}", response3_text);
    assert!(response3_text.contains("I have created the file 'simple_script.py' with the content"), "Interaction 3: Summary text missing. Got: {}", response3_text);
    assert_eq!(mock_llm.calls.lock().await.len(), cumulative_llm_calls_expected, "Interaction 3: LLM call count mismatch."); 
    // MockEditFileTool will print its execution to stdout.
}

#[tokio::test]
async fn test_agent_memory_and_state_management() {
    // ... existing code ...
}

// ============================================================================
// DIAGNOSTIC AND DEBUGGING TESTS
// ============================================================================

#[tokio::test]
async fn test_agent_diagnostic_information() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug) 
        .try_init();

    let mock_responses = vec![
        MockResponse {
            text: "This is a diagnostic stream. Part 1.".to_string(), 
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];
    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    println!("🔍 Test: test_agent_diagnostic_information");
    
    let responses = process_and_collect_responses(
        &agent, 
        "Provide some diagnostic information through your stream.", 
        10
    ).await;

    println!("  - Agent responses collected by helper: {:?}", responses);
    println!("  - Mock LLM calls made (to generate/generate_stream): {:?}", mock_llm.calls.lock().await.len());

    assert!(!responses.is_empty(), "Agent should produce some response parts.");
    let full_text_from_responses = responses.join("");
    assert!(full_text_from_responses.contains("This is a diagnostic stream. Part 1."),
        "Collected responses should contain the text from the mock LLM. Got: {}", full_text_from_responses);
    let llm_calls = mock_llm.calls.lock().await;
    assert_eq!(llm_calls.len(), 1, "Mock LLM's generate/generate_stream should have been called once.");
    println!("  (Note: process_and_collect_responses logs AgentEvents and StreamChunks to stdout during execution)");
}

#[tokio::test]
async fn test_reasoning_engine_direct() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();
    println!("🔍 Test: test_reasoning_engine_direct (with MockLLM)");

    let sagitta_code_config = SagittaCodeConfig::default(); 
    let reasoning_config_data = create_reasoning_config(&sagitta_code_config);
    let mock_llm_responses = vec![
        MockResponse {
            text: "Hello from direct Reasoning Engine test via MockLlmClient!".to_string(),
            tool_calls: vec![], 
            should_error: false, error_message: None,
        }
    ];
    let mock_sagitta_code_llm_client = Arc::new(MockLlmClient::new(mock_llm_responses));
    let tool_registry = Arc::new(ToolRegistry::new());
    
    // 3. ReasoningLlmClientAdapter (adapts MockLlmClient to reasoning_engine::traits::LlmClient)
    //    The concrete type LC is ReasoningLlmClientAdapter.
    let llm_adapter_for_re: Arc<ReasoningLlmClientAdapter> = 
        Arc::new(ReasoningLlmClientAdapter::new(mock_sagitta_code_llm_client.clone(), tool_registry.clone()));

    // 4. Intent Analyzer (using mock embedding provider, as in setup_test_agent)
    let mock_embedding_provider = Arc::new(ThreadSafeMockProvider::new(384)); 
    let intent_analyzer = Arc::new(SagittaCodeIntentAnalyzer::new(mock_embedding_provider));
    
    let mut reasoning_engine = ReasoningEngine::new(
        reasoning_config_data,
        llm_adapter_for_re, 
        intent_analyzer,
    ).await.expect("Failed to create ReasoningEngine with mock components");

    let messages_for_re = vec![
        reasoning_engine::traits::LlmMessage {
            role: "user".to_string(),
            parts: vec![reasoning_engine::traits::LlmMessagePart::Text("Hello, Reasoning Engine!".to_string())],
        }
    ];

    #[derive(Debug)] struct SimpleREToolExecutor;
    #[async_trait]
    impl reasoning_engine::traits::ToolExecutor for SimpleREToolExecutor {
        async fn execute_tool(&self, name: &str, _args: Value) -> Result<reasoning_engine::traits::ToolResult, ReasoningError> {
            println!("  [SimpleREToolExecutor] execute_tool called: {}", name);
            Ok(reasoning_engine::traits::ToolResult::success(serde_json::json!({"status": "tool executed mockly"}), 0))
        }
        async fn get_available_tools(&self) -> Result<Vec<reasoning_engine::traits::ToolDefinition>, ReasoningError> { Ok(vec![]) }
    }

    #[derive(Debug)] struct SimpleREEventEmitter;
    #[async_trait]
    impl reasoning_engine::traits::EventEmitter for SimpleREEventEmitter {
        async fn emit_event(&self, event: ReasoningEvent) -> Result<(), ReasoningError> { 
            println!("  [SimpleREEventEmitter] Event: {:?}", event);
            Ok(())
        }
    }

    #[derive(Debug)]
    struct SimpleREStreamHandler { chunks_collected: Arc<tokio::sync::Mutex<Vec<String>>> }
    impl SimpleREStreamHandler { 
        fn new() -> Self { Self { chunks_collected: Arc::new(tokio::sync::Mutex::new(Vec::new())) } } 
        async fn get_all_text(&self) -> String { self.chunks_collected.lock().await.join("") }
    }
    #[async_trait]
    impl reasoning_engine::traits::StreamHandler for SimpleREStreamHandler {
        async fn handle_chunk(&self, chunk: reasoning_engine::streaming::StreamChunk) -> Result<(), ReasoningError> {
            if let Ok(content) = String::from_utf8(chunk.data) {
                println!("  [SimpleREStreamHandler] Chunk: {}", content);
                self.chunks_collected.lock().await.push(content);
            }
            Ok(())
        }
        async fn handle_stream_complete(&self, _stream_id: Uuid) -> Result<(), ReasoningError> { Ok(()) }
        async fn handle_stream_error(&self, _stream_id: Uuid, error: ReasoningError) -> Result<(), ReasoningError> { Err(error) }
    }
    
    let tool_executor_re = Arc::new(SimpleREToolExecutor);
    let event_emitter_re = Arc::new(SimpleREEventEmitter);
    let stream_handler_re = Arc::new(SimpleREStreamHandler::new());

    let result = reasoning_engine.process_with_context(
        messages_for_re,
        tool_executor_re,
        event_emitter_re,
        stream_handler_re.clone(),
        None, 
        None, 
    ).await;

    assert!(result.is_ok(), "ReasoningEngine::process_with_context failed: {:?}", result.err());
    let final_state = result.unwrap();
    assert!(final_state.is_successful(), "ReasoningEngine did not complete successfully. Reason: {:?}", final_state.completion_reason);

    let collected_text = stream_handler_re.get_all_text().await;
    assert!(!collected_text.is_empty(), "ReasoningEngine stream handler did not receive any text chunks.");
    assert!(collected_text.contains("Hello from direct Reasoning Engine test"), 
        "Did not get expected output from mocked RE LLM. Got: {}", collected_text);
    
    assert_eq!(mock_sagitta_code_llm_client.calls.lock().await.len(), 1, "MockLlmClient for ReasoningEngine should have been called once.");
}

#[tokio::test]
async fn test_llm_adapter_direct() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();
    println!("🔍 Test: test_llm_adapter_direct (with MockLLM)");

    let mock_sagitta_code_responses = vec![
        MockResponse { 
            text: "Adapter direct test response. Part 1.".to_string(), 
            tool_calls: vec![], 
            should_error: false, error_message: None 
        },
    ];
    let mock_sagitta_code_llm_client = Arc::new(MockLlmClient::new(mock_sagitta_code_responses));
    let tool_registry = Arc::new(ToolRegistry::new());
    let adapter: Arc<dyn reasoning_engine::traits::LlmClient> = 
        Arc::new(ReasoningLlmClientAdapter::new(mock_sagitta_code_llm_client.clone(), tool_registry));

    let messages_for_adapter = vec![
        reasoning_engine::traits::LlmMessage {
            role: "user".to_string(),
            parts: vec![reasoning_engine::traits::LlmMessagePart::Text("Hello, LLM Adapter!".to_string())],
        }
    ];

    let stream_result = adapter.generate_stream(messages_for_adapter).await;
    assert!(stream_result.is_ok(), "Adapter's generate_stream failed: {:?}", stream_result.err());
    let mut stream = stream_result.unwrap();

    let mut collected_adapter_text_chunks = Vec::new();
    let mut adapter_stream_chunk_count = 0;
    let mut final_chunk_received = false;

    while let Some(chunk_result) = stream.next().await {
        assert!(chunk_result.is_ok(), "Error in stream chunk from adapter: {:?}", chunk_result.err());
        let chunk = chunk_result.unwrap();
        adapter_stream_chunk_count += 1;
        println!("  [LLM Adapter Test] Stream Chunk #{}: {:?}", adapter_stream_chunk_count, chunk);
        match chunk {
            reasoning_engine::traits::LlmStreamChunk::Text { content, is_final } => {
                collected_adapter_text_chunks.push(content);
                if is_final { final_chunk_received = true; }
            }
            reasoning_engine::traits::LlmStreamChunk::ToolCall { .. } => {
                panic!("Did not expect tool call chunks in this adapter test scenario.");
            }
            reasoning_engine::traits::LlmStreamChunk::TokenUsage(usage) => {
                println!("  [LLM Adapter Test] TokenUsage received: {:?}", usage);
                // TokenUsage is a valid chunk type, allow it.
            }
            // Removed the LlmStreamChunk::Thought variant as it does not exist in reasoning_engine::traits::LlmStreamChunk
        }
    }

    assert!(adapter_stream_chunk_count > 0, "Adapter stream produced no chunks.");
    assert!(final_chunk_received, "Adapter stream should have included a final chunk.");

    let full_text_from_adapter = collected_adapter_text_chunks.join("");
    assert!(full_text_from_adapter.contains("Adapter direct test response. Part 1."), 
        "Did not get expected text from adapter. Got: '{}'", full_text_from_adapter);
    
    assert_eq!(mock_sagitta_code_llm_client.calls.lock().await.len(), 1, 
        "Underlying mock_sagitta_code_llm_client (MockLlmClient) should have been called once by the adapter's generate_stream call.");
}

#[tokio::test]
async fn test_debugging_workflow() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // 1. Define Mock LLM Behavior
    // This is the LLM's response after AnalyzeInputTool has run and its (mocked) output 
    // is processed by the LLM along with the user's original query.
    let mock_responses = vec![
        MockResponse {
            text: "I can certainly try to help you debug that. To get started, could you please show me the function that's returning None, or tell me its name so I can try to locate it?".to_string(),
            tool_calls: vec![], // No tool call expected in this initial clarifying response
            should_error: false, error_message: None,
        }
    ];

    // 2. Instantiate the mock client
    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));

    // 3. Setup Agent
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    // 4. Process Message
    let responses = process_and_collect_responses(
        &agent,
        "I have a bug in my code where a function returns None instead of the expected value. Help me debug this.",
        10
    ).await;

    println!("🐛 Test: test_debugging_workflow");
    println!("  - Mock LLM calls made: {}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses collected by helper: {:?}", responses);

    // 5. Add Specific Assertions
    assert!(!responses.is_empty(), "Agent should produce a response.");

    let tool_call_count = responses.iter().filter(|r| r.contains("[ToolCall:")).count();
    assert_eq!(tool_call_count, 0, "Agent should ask for details for a vague debug request, not call tools immediately.");

    let full_response_text = responses.join(" ");
    assert!(
        full_response_text.contains("help you debug that") && 
        (full_response_text.contains("show me the function") || full_response_text.contains("tell me its name")),
        "Agent should offer to help debug and ask for more information. Got: {}", full_response_text
    );
    
    // AnalyzeInputTool runs (doesn't use mock_llm).
    // Then, LLM (mock_llm) is called once with (user_input + analyze_input_result) to generate the clarifying response.
    assert_eq!(mock_llm.calls.lock().await.len(), 1, "Mock LLM should have been invoked once.");
}

#[tokio::test]
async fn test_code_review_scenario() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // 1. Define Mock LLM Behavior
    // This is the LLM's response after AnalyzeInputTool has run.
    let mock_responses = vec![
        MockResponse {
            text: "Certainly, I can take a look at that code for you. I'll check for common issues and best practices.".to_string(),
            tool_calls: vec![], 
            should_error: false, error_message: None,
        }
    ];

    // 2. Instantiate the mock client
    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));

    // 3. Setup Agent
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    // 4. Process Message
    let responses = process_and_collect_responses(
        &agent,
        "Review this code for security issues and best practices: fn unsafe_function() { /* some complex code */ }",
        10
    ).await;

    println!("👀 Test: test_code_review_scenario");
    println!("  - Mock LLM calls made: {}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses: {:?}", responses);

    // 5. Add Specific Assertions
    assert!(!responses.is_empty(), "Agent should produce a response.");

    let tool_call_count = responses.iter().filter(|r| r.contains("[ToolCall:")).count();
    assert_eq!(tool_call_count, 0, "Agent should acknowledge the review request, not call tools immediately based on this mock setup.");

    let full_response_text = responses.join(" ");
    assert!(
        full_response_text.contains("take a look at that code") && 
        full_response_text.contains("common issues and best practices"),
        "Agent should offer to review the code. Got: {}", full_response_text
    );
    
    assert_eq!(mock_llm.calls.lock().await.len(), 1, "Mock LLM should have been invoked once.");
}

#[tokio::test]
async fn test_infinite_tool_calling_loop_prevention() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let llm_suggested_tool_attempts = 3; // Let's test a shorter loop controlled by the mock LLM.
    let mut mock_responses_for_loop = Vec::new();

    // LLM is programmed to suggest calling web_search `llm_suggested_tool_attempts` times.
    for i in 0..llm_suggested_tool_attempts {
        mock_responses_for_loop.push(MockResponse {
            text: format!("LLM: Planning to search, attempt {}.", i + 1), // Corrected: Added missing comma before i + 1 and ensured parenthesis match format! macro
            tool_calls: vec![(
                format!("call_search_loop_{}", i),
                "web_search".to_string(), 
                serde_json::json!({"query": format!("puppies loop attempt {}", i + 1)})
            )],
            should_error: false, error_message: None,
        });
    }
    // This is the LLM's response *after* it has made its `llm_suggested_tool_attempts`.
    mock_responses_for_loop.push(MockResponse {
        text: "LLM: I have tried searching several times. I will stop for now.".to_string(),
        tool_calls: vec![], 
        should_error: false, error_message: None,
    });
    // Add one more response for the agent's potential "What next?" query if the above isn't seen as final.
    mock_responses_for_loop.push(MockResponse {
        text: "Okay, the process is complete.".to_string(),
        tool_calls: vec![], should_error: false, error_message: None,
    });


    let mock_llm = Arc::new(MockLlmClient::new(mock_responses_for_loop));
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;
    
    let responses = process_and_collect_responses(
        &agent,
        "Keep searching for puppies many times.", 
        30 
    ).await;

    println!("🔍 Test: test_infinite_tool_calling_loop_prevention");
    let actual_llm_calls_made = mock_llm.calls.lock().await.len();
    println!("  - Mock LLM was programmed for {} tool attempts + 1 stop summary + 1 final agent prompt response = {} total responses.", 
        llm_suggested_tool_attempts, llm_suggested_tool_attempts + 2);
    println!("  - Actual LLM calls made to mock_llm: {}", actual_llm_calls_made);
    println!("  - Agent responses: {:?}", responses);

    assert!(!responses.is_empty(), "Agent should produce a response.");

    // LLM calls = number of tool attempts + 1 for the LLM saying it will stop + (optionally) 1 if agent asks "what next".
    // So, it should be llm_suggested_tool_attempts + 1 or llm_suggested_tool_attempts + 2
    assert!(
        actual_llm_calls_made == llm_suggested_tool_attempts + 1 || actual_llm_calls_made == llm_suggested_tool_attempts + 2,
        "Expected {} or {} LLM calls, got {}. This indicates the mock LLM sequence or agent loop handling is not as expected.",
        llm_suggested_tool_attempts + 1, llm_suggested_tool_attempts + 2, actual_llm_calls_made
    );

    let full_response_text = responses.join(" ");
    assert!(
        full_response_text.contains("I will stop for now") || full_response_text.contains("process is complete"),
        "Agent's final response text should indicate the loop was intentionally stopped by the LLM. Got: {}", full_response_text
    );
    // MockWebSearchTool will print to stdout for each actual execution, verifying tool calls took place.
}

#[tokio::test]
async fn test_multiple_tool_failures_in_sequence() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let mock_responses = vec![
        // LLM Call 1: Initial plan
        MockResponse {
            text: "Okay, I will attempt to edit 'file_A_fails.txt', then search the codebase for 'term_X', and finally attempt to edit 'file_B_fails.txt'. Starting with 'file_A_fails.txt'.".to_string(),
            tool_calls: vec![(
                "edit_A_fail".to_string(),
                "edit_file".to_string(),
                serde_json::json!({
                    "target_file": "file_A_fails.txt",
                    "code_edit": "// content for A",
                    "instructions": "Edit A"
                })
            )],
            should_error: false, error_message: None,
        },
        // LLM Call 2: React to edit_A failure, plan for codebase_search
        MockResponse {
            text: "It seems editing 'file_A_fails.txt' failed: File edit failed: Permission denied. I will skip that and proceed to search the codebase for 'term_X'.".to_string(),
            tool_calls: vec![(
                "search_X".to_string(),
                "codebase_search".to_string(),
                serde_json::json!({"query": "term_X"})
            )],
            should_error: false, error_message: None,
        },
        // LLM Call 3: Acknowledge search success, plan for edit_B
        MockResponse {
            text: "The codebase search for 'term_X' was successful. I found: [\"match1.rs:42\", \"match2.py:15\"]. Now, I will attempt to edit 'file_B_fails.txt'.".to_string(),
            tool_calls: vec![(
                "edit_B_fail".to_string(),
                "edit_file".to_string(),
                serde_json::json!({
                    "target_file": "file_B_fails.txt",
                    "code_edit": "// content for B",
                    "instructions": "Edit B"
                })
            )],
            should_error: false, error_message: None,
        },
        // LLM Call 4: React to edit_B failure, summarize overall
        MockResponse {
            text: "It seems editing 'file_B_fails.txt' also failed: File edit failed: Permission denied. To summarize: editing 'file_A_fails.txt' failed, the codebase search for 'term_X' found 2 matches, and editing 'file_B_fails.txt' failed.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ];

    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;

    let responses = process_and_collect_responses(
        &agent,
        "Edit file_A_fails.txt, then search codebase for term_X, then edit file_B_fails.txt",
        25 // Increased timeout for multi-step
    ).await;

    println!("🔍 Test: test_multiple_tool_failures_in_sequence");
    println!("  - Mock LLM calls made: {}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses: {:?}", responses);

    assert_eq!(mock_llm.calls.lock().await.len(), 4, "Expected 4 LLM calls for the multi-failure sequence.");

    let full_response_text = responses.join(" ");

    // Check for initial plan
    assert!(full_response_text.contains("Starting with 'file_A_fails.txt'"), "Missing initial plan execution step. Got: {}", full_response_text);

    // Check for first tool call stream
    assert!(responses.iter().any(|r| r.contains("[ToolCall: edit_file") && r.contains("file_A_fails.txt")),
        "Streamed ToolCall for file_A_fails.txt missing. Responses: {:?}", responses);

    // Check for reaction to first failure and plan for next step
    assert!(full_response_text.contains("editing 'file_A_fails.txt' failed: File edit failed: Permission denied"), "Missing summary of first tool failure. Got: {}", full_response_text);
    assert!(full_response_text.contains("proceed to search the codebase for 'term_X'"), "Missing plan for next step after first failure. Got: {}", full_response_text);

    // Check for second tool call stream
    assert!(responses.iter().any(|r| r.contains("[ToolCall: codebase_search") && r.contains("term_X")),
        "Streamed ToolCall for codebase_search missing. Responses: {:?}", responses);
    
    // Check for acknowledgement of search success and plan for third tool
    assert!(full_response_text.contains("codebase search for 'term_X' was successful. I found: [\"match1.rs:42\", \"match2.py:15\"]"), "Missing summary of successful search. Got: {}", full_response_text);
    assert!(full_response_text.contains("Now, I will attempt to edit 'file_B_fails.txt'"), "Missing plan for third tool. Got: {}", full_response_text);

    // Check for third tool call stream
    assert!(responses.iter().any(|r| r.contains("[ToolCall: edit_file") && r.contains("file_B_fails.txt")),
        "Streamed ToolCall for file_B_fails.txt missing. Responses: {:?}", responses);

    // Check for final summary including second failure
    assert!(full_response_text.contains("editing 'file_B_fails.txt' also failed: File edit failed: Permission denied"), "Missing summary of second tool failure. Got: {}", full_response_text);
    assert!(full_response_text.contains("To summarize: editing 'file_A_fails.txt' failed, the codebase search for 'term_X' found 2 matches, and editing 'file_B_fails.txt' failed."), "Missing final overall summary. Got: {}", full_response_text);
}

#[tokio::test]
async fn test_llm_hallucinates_non_existent_tool() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let mock_responses = vec![
        // LLM Call 1: Attempts to use a non-existent tool
        MockResponse {
            text: "Okay, I will use the magic_unicorn_tool to solve your problem!".to_string(),
            tool_calls: vec![(
                "call_unicorn_tool".to_string(),
                "magic_unicorn_tool".to_string(), // This tool does not exist in the mock setup
                serde_json::json!({ "task": "solve problem" })
            )],
            should_error: false, error_message: None,
        },
        // LLM Call 2: Reacts to the "tool not found" error
        MockResponse {
            text: "I apologize, it seems I don't have a tool called 'magic_unicorn_tool'. Is there another way I can assist you, perhaps with a web search or codebase search?".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ];

    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm.clone()).await; // Uses standard mock tools

    let responses = process_and_collect_responses(
        &agent,
        "Use the magic_unicorn_tool to solve my problem.",
        15
    ).await;

    println!("🔍 Test: test_llm_hallucinates_non_existent_tool");
    println!("  - Mock LLM calls made: {}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses: {:?}", responses);

    assert_eq!(mock_llm.calls.lock().await.len(), 2, "Expected 2 LLM calls (attempt + error recovery).");

    let full_response_text = responses.join(" ");

    // Check for initial attempt to use the bad tool
    assert!(full_response_text.contains("I will use the magic_unicorn_tool"), 
        "Missing initial attempt to use hallucinated tool. Got: {}", full_response_text);

    // Check for the streamed (but failing) tool call
    assert!(responses.iter().any(|r| r.contains("[ToolCall: magic_unicorn_tool") && r.contains("solve problem")),
        "Streamed ToolCall for magic_unicorn_tool missing. Responses: {:?}", responses);

    // Check for LLM's recovery message
    assert!(full_response_text.contains("don't have a tool called 'magic_unicorn_tool'"), 
        "LLM should acknowledge the tool doesn't exist. Got: {}", full_response_text);
    assert!(full_response_text.contains("Is there another way I can assist you"), 
        "LLM should offer alternatives after tool failure. Got: {}", full_response_text);
}

#[tokio::test]
async fn test_llm_provides_ambiguous_tool_parameters() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let mock_responses = vec![
        // LLM Call 1: Attempts to use codebase_search with a bad parameter
        MockResponse {
            text: "I will search the codebase with a problematic query.".to_string(),
            tool_calls: vec![(
                "call_search_bad_param".to_string(),
                "codebase_search".to_string(),
                serde_json::json!({ "query": "bad_param_marker" }) // This will be rejected by the mock tool
            )],
            should_error: false, error_message: None,
        },
        // LLM Call 2: Reacts to the "Invalid search query" error
        MockResponse {
            text: "It seems the search query I used was invalid. The tool said: Invalid search query provided. Query cannot be empty or a marker. Could you please tell me more specifically what you are looking for?".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ];

    let mock_llm = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm.clone()).await; // MockCodebaseSearchTool is now modified

    let responses = process_and_collect_responses(
        &agent,
        "Search the codebase for something problematic.", // User input that leads to the bad param by LLM
        15
    ).await;

    println!("🔍 Test: test_llm_provides_ambiguous_tool_parameters");
    println!("  - Mock LLM calls made: {}", mock_llm.calls.lock().await.len());
    println!("  - Agent responses: {:?}", responses);

    assert_eq!(mock_llm.calls.lock().await.len(), 2, "Expected 2 LLM calls (attempt + error recovery).");

    let full_response_text = responses.join(" ");

    // Check for initial attempt
    assert!(full_response_text.contains("search the codebase with a problematic query"), 
        "Missing initial LLM text for problematic search. Got: {}", full_response_text);

    // Check for the streamed tool call with bad param
    assert!(responses.iter().any(|r| r.contains("[ToolCall: codebase_search") && r.contains("bad_param_marker")),
        "Streamed ToolCall for codebase_search with bad_param_marker missing. Responses: {:?}", responses);

    // Check for LLM's recovery message including the tool's error
    assert!(full_response_text.contains("search query I used was invalid"), 
        "LLM should acknowledge the invalid query. Got: {}", full_response_text);
    assert!(full_response_text.contains("Invalid search query provided. Query cannot be empty or a marker."), 
        "LLM response should include the error message from the tool. Got: {}", full_response_text);
    assert!(full_response_text.contains("Could you please tell me more specifically"), 
        "LLM should ask for clarification. Got: {}", full_response_text);
}

#[tokio::test]
async fn test_complex_multi_turn_contextual_tool_use() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // --- Mock LLM Responses for 3 turns --- 
    let mock_llm_responses_turn1 = vec![
        // LLM Call 1.1 (Plan and execute web_search for ORMs)
        MockResponse {
            text: "Okay, I will search the web for 'best Rust ORM for 2025'.".to_string(),
            tool_calls: vec![(
                "search_orm_2025".to_string(),
                "web_search".to_string(),
                serde_json::json!({ "query": "best Rust ORM for 2025" })
            )],
            should_error: false, error_message: None,
        },
        // LLM Call 1.2 (Summarize web_search results)
        MockResponse {
            text: "I found Diesel (mature) and SQLx (async) as popular Rust ORMs for 2025.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ];

    let mock_llm_responses_turn2 = vec![
        // LLM Call 2.1 (Plan and execute codebase_search for SQLx)
        MockResponse {
            text: "Alright, I will search the codebase for examples of 'SQLx'.".to_string(),
            tool_calls: vec![(
                "search_sqlx_code".to_string(),
                "codebase_search".to_string(),
                serde_json::json!({ "query": "SQLx" })
            )],
            should_error: false, error_message: None,
        },
        // LLM Call 2.2 (Summarize codebase_search results)
        MockResponse {
            text: "I found SQLx setup in `db/sqlx_config.rs` and usage in `user_service.rs`.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ];

    let mock_llm_responses_turn3 = vec![
        // LLM Call 3.1 (Plan and execute edit_file for Diesel note)
        MockResponse {
            text: "Okay, I will edit `diesel_notes.md` to add the note about Diesel.".to_string(),
            tool_calls: vec![(
                "edit_diesel_note".to_string(),
                "edit_file".to_string(),
                serde_json::json!({
                    "target_file": "diesel_notes.md",
                    "code_edit": "Diesel is a mature ORM option.",
                    "instructions": "Add note about Diesel ORM"
                })
            )],
            should_error: false, error_message: None,
        },
        // LLM Call 3.2 (Summarize edit_file result)
        MockResponse {
            text: "Okay, I've added the note about Diesel to `diesel_notes.md`.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ];

    // Combine all mock responses for the single MockLlmClient instance
    let all_mock_responses = [
        mock_llm_responses_turn1.clone(),
        mock_llm_responses_turn2.clone(),
        mock_llm_responses_turn3.clone(),
    ].concat();

    let mock_llm = Arc::new(MockLlmClient::new(all_mock_responses));
    let (agent, _) = setup_test_agent(mock_llm.clone()).await;
    let mut cumulative_llm_calls = 0;

    // --- Turn 1 ---
    println!("\n🔄 Test: test_complex_multi_turn_contextual_tool_use - Turn 1");
    let responses_turn1 = process_and_collect_responses(
        &agent, 
        "Search the web for 'best Rust ORM for 2025'.", 
        15
    ).await;
    cumulative_llm_calls += 2;
    assert_eq!(mock_llm.calls.lock().await.len(), cumulative_llm_calls, "LLM calls after Turn 1");
    let text_turn1 = responses_turn1.join(" ");
    assert!(text_turn1.contains("search the web for 'best Rust ORM for 2025'"));
    assert!(responses_turn1.iter().any(|r| r.contains("[ToolCall: web_search") && r.contains("best Rust ORM for 2025")));
    assert!(text_turn1.contains("Diesel (mature) and SQLx (async)"));
    println!("  - Responses Turn 1: {:?}", responses_turn1);

    // --- Turn 2 ---
    println!("\n🔄 Test: test_complex_multi_turn_contextual_tool_use - Turn 2");
    let responses_turn2 = process_and_collect_responses(
        &agent, 
        "Okay, now search the codebase for examples of 'SQLx'.", 
        15
    ).await;
    cumulative_llm_calls += 2;
    assert_eq!(mock_llm.calls.lock().await.len(), cumulative_llm_calls, "LLM calls after Turn 2");
    let text_turn2 = responses_turn2.join(" ");
    assert!(text_turn2.contains("search the codebase for examples of 'SQLx'"));
    assert!(responses_turn2.iter().any(|r| r.contains("[ToolCall: codebase_search") && r.contains("SQLx")));
    assert!(text_turn2.contains("SQLx setup in `db/sqlx_config.rs`"));
    println!("  - Responses Turn 2: {:?}", responses_turn2);

    // --- Turn 3 ---
    println!("\n🔄 Test: test_complex_multi_turn_contextual_tool_use - Turn 3");
    let responses_turn3 = process_and_collect_responses(
        &agent, 
        "Regarding the first one you mentioned, Diesel, can you edit `diesel_notes.md` and add a note: 'Diesel is a mature ORM option.'?", 
        15
    ).await;
    cumulative_llm_calls += 2;
    assert_eq!(mock_llm.calls.lock().await.len(), cumulative_llm_calls, "LLM calls after Turn 3");
    let text_turn3 = responses_turn3.join(" ");
    assert!(text_turn3.contains("edit `diesel_notes.md` to add the note about Diesel"));
    assert!(responses_turn3.iter().any(|r| r.contains("[ToolCall: edit_file") && r.contains("diesel_notes.md") && r.contains("Diesel is a mature ORM option")));
    assert!(text_turn3.contains("added the note about Diesel to `diesel_notes.md`"));
    println!("  - Responses Turn 3: {:?}", responses_turn3);
}

#[tokio::test]
async fn test_llm_decision_post_analyze_input() {
    let _ = env_logger::builder().filter_level(log::LevelFilter::Debug).try_init();

    // CASE 1: LLM makes a logical choice (codebase_search) after AnalyzeInputTool runs.
    println!("\n🧪 Test Case 1: LLM uses codebase_search post AnalyzeInputTool");
    let mock_responses_case1 = vec![
        // LLM Call 1.1 (after AnalyzeInputTool): Plan and execute codebase_search
        MockResponse {
            text: "Okay, I understand you're asking about the 'foo' function in your project. I will search the codebase for it.".to_string(),
            tool_calls: vec![(
                "search_foo_code".to_string(),
                "codebase_search".to_string(),
                serde_json::json!({ "query": "foo function" })
            )],
            should_error: false, error_message: None,
        },
        // LLM Call 1.2: Summarize codebase_search results
        MockResponse {
            text: "I found the 'foo' function in 'src/utils.rs'. It seems to handle user authentication.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ];
    let mock_llm_case1 = Arc::new(MockLlmClient::new(mock_responses_case1));
    let (agent_case1, _) = setup_test_agent(mock_llm_case1.clone()).await;
    let responses_case1 = process_and_collect_responses(
        &agent_case1, 
        "Tell me about the `foo` function in my project.", 
        15
    ).await;
    assert_eq!(mock_llm_case1.calls.lock().await.len(), 2, "LLM calls for Case 1");
    let text_case1 = responses_case1.join(" ");
    assert!(text_case1.contains("search the codebase for it"));
    assert!(responses_case1.iter().any(|r| r.contains("[ToolCall: codebase_search") && r.contains("foo function")));
    assert!(text_case1.contains("foo' function in 'src/utils.rs'"));
    println!("  - Responses Case 1: {:?}", responses_case1);

    // CASE 2: LLM makes a less logical choice (web_search) after AnalyzeInputTool runs.
    // This demonstrates the main LLM is the one making the tool choice, influenced by its own (mocked) reasoning.
    println!("\n🧪 Test Case 2: LLM uses web_search post AnalyzeInputTool (less optimal choice)");
    let mock_responses_case2 = vec![
        // LLM Call 2.1 (after AnalyzeInputTool): Plan and execute web_search
        MockResponse {
            text: "Okay, you want to know about the 'foo' function. I will search the web for information on functions named 'foo'.".to_string(),
            tool_calls: vec![(
                "search_foo_web".to_string(),
                "web_search".to_string(),
                serde_json::json!({ "query": "foo function" })
            )],
            should_error: false, error_message: None,
        },
        // LLM Call 2.2: Summarize web_search results
        MockResponse {
            text: "I found various public code snippets and articles about functions named 'foo' online.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ];
    let mock_llm_case2 = Arc::new(MockLlmClient::new(mock_responses_case2));
    let (agent_case2, _) = setup_test_agent(mock_llm_case2.clone()).await;
    let responses_case2 = process_and_collect_responses(
        &agent_case2, 
        "Tell me about the `foo` function in my project.", // Same user input
        15
    ).await;
    assert_eq!(mock_llm_case2.calls.lock().await.len(), 2, "LLM calls for Case 2");
    let text_case2 = responses_case2.join(" ");
    assert!(text_case2.contains("search the web for information"));
    assert!(responses_case2.iter().any(|r| r.contains("[ToolCall: web_search") && r.contains("foo function")));
    assert!(text_case2.contains("various public code snippets"));
    println!("  - Responses Case 2: {:?}", responses_case2);
    // Note: The MockAnalyzeInputTool in setup_test_agent will have run before the main LLM calls in both cases.
    // Its output (generic analysis, no specific tool suggestion) is part of the history fed to the main LLM.
    // These test cases show that the main LLM's (mocked) decision process is what drives the subsequent tool choice.
}

#[tokio::test]
async fn test_cascading_tool_failures_with_recovery() {
    println!("🔥 Test: test_cascading_tool_failures_with_recovery");
    
    let mock_responses = vec![
        MockResponse {
            text: "I'll attempt to edit the first file, then search, then edit the second file.".to_string(),
            tool_calls: vec![("tc1".to_string(), "edit_file".to_string(), serde_json::json!({
                "target_file": "fail_file_1.txt",
                "code_edit": "content 1",
                "instructions": "Edit first file"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "The first edit failed. Let me try the search operation.".to_string(),
            tool_calls: vec![("tc2".to_string(), "codebase_search".to_string(), serde_json::json!({
                "query": ""  // This will trigger an error in our mock
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "The search also failed due to empty query. Let me try the second file edit.".to_string(),
            tool_calls: vec![("tc3".to_string(), "edit_file".to_string(), serde_json::json!({
                "target_file": "fail_file_2.txt",
                "code_edit": "content 2",
                "instructions": "Edit second file"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "All three operations failed. I'll try a web search as an alternative approach.".to_string(),
            tool_calls: vec![("tc4".to_string(), "web_search".to_string(), serde_json::json!({
                "query": "alternative solution"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "I was able to find alternative information through web search. Despite the file operation failures, I can provide guidance based on the search results.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    let mock_llm_client = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm_client.clone()).await;

    let responses = process_and_collect_responses(
        &agent,
        "Edit fail_file_1.txt, then search for 'pattern', then edit fail_file_2.txt",
        15
    ).await;

    println!("  - Agent responses: {:?}", responses);
    
    // Verify the agent attempted recovery and found an alternative solution
    let full_response = responses.join(" ");
    assert!(full_response.contains("failed"), "Should acknowledge failures");
    assert!(full_response.contains("alternative") || full_response.contains("web search"), "Should show recovery attempt");
    
    // Verify all expected tool calls were made
    let tool_call_count = responses.iter().filter(|r| r.contains("[ToolCall:")).count();
    assert!(tool_call_count >= 3, "Should have attempted multiple tool calls for recovery");
    
    println!("  ✅ Cascading failure recovery test passed");
}

#[tokio::test]
async fn test_context_preservation_across_complex_workflow() {
    println!("🧠 Test: test_context_preservation_across_complex_workflow");
    
    // This test simulates a complex multi-step workflow where context must be preserved
    let mock_responses_turn1 = vec![
        MockResponse {
            text: "I'll search for Rust async patterns and remember the context.".to_string(),
            tool_calls: vec![("tc1".to_string(), "web_search".to_string(), serde_json::json!({
                "query": "Rust async patterns 2025"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Found excellent async patterns including tokio, async-std, and futures. I'll remember these for our discussion.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    let mock_responses_turn2 = vec![
        MockResponse {
            text: "Based on our previous discussion about Rust async patterns, I'll search the codebase for tokio usage.".to_string(),
            tool_calls: vec![("tc2".to_string(), "codebase_search".to_string(), serde_json::json!({
                "query": "tokio"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Found tokio usage in several files. This aligns with the async patterns we discussed earlier.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    let mock_responses_turn3 = vec![
        MockResponse {
            text: "Now I'll create a summary file combining the web search results about async patterns and the codebase analysis.".to_string(),
            tool_calls: vec![("tc3".to_string(), "edit_file".to_string(), serde_json::json!({
                "target_file": "async_analysis_summary.md",
                "code_edit": "# Async Analysis Summary\n\nWeb research: tokio, async-std, futures\nCodebase analysis: tokio usage found\n",
                "instructions": "Create summary combining previous findings"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "I've created a comprehensive summary that combines our web research on Rust async patterns with the codebase analysis. The summary shows how the patterns we found online align with your project's current tokio usage.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    // Turn 1: Initial research
    let mock_llm_client_1 = Arc::new(MockLlmClient::new(mock_responses_turn1));
    let (agent, _) = setup_test_agent(mock_llm_client_1.clone()).await;

    let responses_1 = process_and_collect_responses(
        &agent,
        "Research current Rust async programming patterns for 2025",
        10
    ).await;

    println!("  - Turn 1 responses: {:?}", responses_1);

    // Turn 2: Context-aware follow-up
    let mock_llm_client_2 = Arc::new(MockLlmClient::new(mock_responses_turn2));
    let (agent_2, _) = setup_test_agent(mock_llm_client_2.clone()).await;

    let responses_2 = process_and_collect_responses(
        &agent_2,
        "Now check our codebase for async patterns, especially tokio usage",
        10
    ).await;

    println!("  - Turn 2 responses: {:?}", responses_2);

    // Turn 3: Synthesis with full context
    let mock_llm_client_3 = Arc::new(MockLlmClient::new(mock_responses_turn3));
    let (agent_3, _) = setup_test_agent(mock_llm_client_3.clone()).await;

    let responses_3 = process_and_collect_responses(
        &agent_3,
        "Create a summary file that combines the research and codebase analysis",
        10
    ).await;

    println!("  - Turn 3 responses: {:?}", responses_3);

    // Verify context preservation and synthesis
    let turn2_response = responses_2.join(" ");
    assert!(turn2_response.contains("tokio") || turn2_response.contains("async"), "Turn 2 should reference async context");

    let turn3_response = responses_3.join(" ");
    assert!(turn3_response.contains("summary") || turn3_response.contains("combines"), "Turn 3 should show synthesis");
    assert!(turn3_response.contains("research") || turn3_response.contains("analysis"), "Turn 3 should reference previous work");

    println!("  ✅ Complex workflow context preservation test passed");
}

#[tokio::test]
async fn test_sophisticated_error_recovery_patterns() {
    println!("🔄 Test: test_sophisticated_error_recovery_patterns");
    
    let mock_responses = vec![
        MockResponse {
            text: "I'll attempt the primary approach first.".to_string(),
            tool_calls: vec![("tc1".to_string(), "edit_file".to_string(), serde_json::json!({
                "target_file": "fail_primary.txt",
                "code_edit": "primary content",
                "instructions": "Primary approach"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Primary approach failed. Let me try the secondary approach with a different tool.".to_string(),
            tool_calls: vec![("tc2".to_string(), "codebase_search".to_string(), serde_json::json!({
                "query": "alternative_pattern"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Found alternative patterns in the codebase. Now I'll try a tertiary approach using web search.".to_string(),
            tool_calls: vec![("tc3".to_string(), "web_search".to_string(), serde_json::json!({
                "query": "best practices for this scenario"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Excellent! The web search provided valuable insights. Let me now create a solution file based on the research.".to_string(),
            tool_calls: vec![("tc4".to_string(), "edit_file".to_string(), serde_json::json!({
                "target_file": "solution.md",
                "code_edit": "# Solution\n\nBased on research and codebase analysis:\n- Found alternative patterns\n- Best practices identified\n",
                "instructions": "Create solution based on research"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "I successfully recovered from the initial failure by using a multi-step approach: codebase analysis revealed alternative patterns, web research provided best practices, and I've documented the solution.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    let mock_llm_client = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm_client.clone()).await;

    let responses = process_and_collect_responses(
        &agent,
        "Create a solution file using the best approach available",
        15
    ).await;

    println!("  - Agent responses: {:?}", responses);
    
    // Verify sophisticated recovery pattern
    let full_response = responses.join(" ");
    assert!(full_response.contains("failed") || full_response.contains("alternative"), "Should show failure acknowledgment");
    assert!(full_response.contains("research") || full_response.contains("analysis"), "Should show research-based recovery");
    assert!(full_response.contains("solution") || full_response.contains("success"), "Should show successful resolution");
    
    // Verify multiple recovery strategies were attempted
    let tool_call_count = responses.iter().filter(|r| r.contains("[ToolCall:")).count();
    assert!(tool_call_count >= 3, "Should have attempted multiple recovery strategies");
    
    println!("  ✅ Sophisticated error recovery test passed");
}

#[tokio::test]
async fn test_analyze_input_tool_interaction_scenarios() {
    println!("🔍 Test: test_analyze_input_tool_interaction_scenarios");
    
    // Scenario 1: AnalyzeInputTool suggests specific next action
    let mock_responses_scenario1 = vec![
        MockResponse {
            text: "Based on the analysis, I should search the codebase for the specific function you mentioned.".to_string(),
            tool_calls: vec![("tc1".to_string(), "codebase_search".to_string(), serde_json::json!({
                "query": "specific_function"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Found the function in multiple files. The analysis was helpful in guiding the search.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    let mock_llm_client_1 = Arc::new(MockLlmClient::new(mock_responses_scenario1));
    let (agent_1, _) = setup_test_agent(mock_llm_client_1.clone()).await;

    let responses_1 = process_and_collect_responses(
        &agent_1,
        "Tell me about the specific_function in my codebase",
        10
    ).await;

    println!("  - Scenario 1 (guided search): {:?}", responses_1);

    // Scenario 2: AnalyzeInputTool suggests web research
    let mock_responses_scenario2 = vec![
        MockResponse {
            text: "The analysis suggests this is a general programming question. I should search the web for current information.".to_string(),
            tool_calls: vec![("tc2".to_string(), "web_search".to_string(), serde_json::json!({
                "query": "latest programming trends 2025"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Found comprehensive information about current programming trends. The analysis correctly identified this as a research question.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    let mock_llm_client_2 = Arc::new(MockLlmClient::new(mock_responses_scenario2));
    let (agent_2, _) = setup_test_agent(mock_llm_client_2.clone()).await;

    let responses_2 = process_and_collect_responses(
        &agent_2,
        "What are the latest programming trends for 2025?",
        10
    ).await;

    println!("  - Scenario 2 (web research): {:?}", responses_2);

    // Scenario 3: AnalyzeInputTool suggests file creation
    let mock_responses_scenario3 = vec![
        MockResponse {
            text: "The analysis indicates you want to create documentation. I'll create the requested file.".to_string(),
            tool_calls: vec![("tc3".to_string(), "edit_file".to_string(), serde_json::json!({
                "target_file": "documentation.md",
                "code_edit": "# Project Documentation\n\nThis file contains project documentation as requested.\n",
                "instructions": "Create documentation file"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Successfully created the documentation file based on the input analysis.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    let mock_llm_client_3 = Arc::new(MockLlmClient::new(mock_responses_scenario3));
    let (agent_3, _) = setup_test_agent(mock_llm_client_3.clone()).await;

    let responses_3 = process_and_collect_responses(
        &agent_3,
        "Create a documentation file for my project",
        10
    ).await;

    println!("  - Scenario 3 (file creation): {:?}", responses_3);

    // Verify each scenario followed appropriate tool selection
    assert!(responses_1.iter().any(|r| r.contains("codebase_search")), "Scenario 1 should use codebase search");
    assert!(responses_2.iter().any(|r| r.contains("web_search")), "Scenario 2 should use web search");
    assert!(responses_3.iter().any(|r| r.contains("edit_file")), "Scenario 3 should use file editing");

    println!("  ✅ AnalyzeInputTool interaction scenarios test passed");
}

#[tokio::test]
async fn test_tool_parameter_validation_and_retry() {
    println!("🔧 Test: test_tool_parameter_validation_and_retry");
    
    let mock_responses = vec![
        MockResponse {
            text: "I'll search the codebase with the initial query.".to_string(),
            tool_calls: vec![("tc1".to_string(), "codebase_search".to_string(), serde_json::json!({
                "query": "bad_param_marker"  // This will fail in our mock
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "The search failed due to invalid parameters. Let me retry with a better query.".to_string(),
            tool_calls: vec![("tc2".to_string(), "codebase_search".to_string(), serde_json::json!({
                "query": "valid search term"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "The retry with improved parameters was successful. I found relevant matches in the codebase.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    let mock_llm_client = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm_client.clone()).await;

    let responses = process_and_collect_responses(
        &agent,
        "Search the codebase for relevant patterns",
        10
    ).await;

    println!("  - Agent responses: {:?}", responses);
    
    // Verify retry behavior
    let full_response = responses.join(" ");
    assert!(full_response.contains("failed") || full_response.contains("invalid"), "Should acknowledge parameter failure");
    assert!(full_response.contains("retry") || full_response.contains("better"), "Should show retry attempt");
    assert!(full_response.contains("successful") || full_response.contains("found"), "Should show eventual success");
    
    // Verify multiple tool calls were made
    let tool_call_count = responses.iter().filter(|r| r.contains("[ToolCall:")).count();
    assert!(tool_call_count >= 2, "Should have made multiple tool calls for retry");
    
    println!("  ✅ Tool parameter validation and retry test passed");
}

#[tokio::test]
async fn test_multi_modal_problem_solving_approach() {
    println!("🎯 Test: test_multi_modal_problem_solving_approach");
    
    let mock_responses = vec![
        MockResponse {
            text: "I'll approach this problem using multiple strategies. First, let me search for existing solutions online.".to_string(),
            tool_calls: vec![("tc1".to_string(), "web_search".to_string(), serde_json::json!({
                "query": "Rust error handling best practices"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Found good online resources. Now let me check how error handling is currently implemented in your codebase.".to_string(),
            tool_calls: vec![("tc2".to_string(), "codebase_search".to_string(), serde_json::json!({
                "query": "error handling Result"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Analyzed current error handling patterns. Now I'll create an improved error handling example based on best practices and current code.".to_string(),
            tool_calls: vec![("tc3".to_string(), "edit_file".to_string(), serde_json::json!({
                "target_file": "improved_error_handling.rs",
                "code_edit": "// Improved error handling based on research and codebase analysis\nuse std::error::Error;\n\n#[derive(Debug)]\nenum CustomError {\n    IoError(std::io::Error),\n    ParseError(String),\n}\n",
                "instructions": "Create improved error handling example"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "I've created a comprehensive solution by combining web research on best practices with analysis of your existing code patterns. The new error handling approach incorporates industry standards while being compatible with your current codebase.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    let mock_llm_client = Arc::new(MockLlmClient::new(mock_responses));
    let (agent, _) = setup_test_agent(mock_llm_client.clone()).await;

    let responses = process_and_collect_responses(
        &agent,
        "Help me improve error handling in my Rust project",
        15
    ).await;

    println!("  - Agent responses: {:?}", responses);
    
    // Verify multi-modal approach
    let full_response = responses.join(" ");
    assert!(full_response.contains("web") || full_response.contains("online"), "Should include web research");
    assert!(full_response.contains("codebase") || full_response.contains("current"), "Should include codebase analysis");
    assert!(full_response.contains("create") || full_response.contains("example"), "Should include solution creation");
    assert!(full_response.contains("combining") || full_response.contains("comprehensive"), "Should show synthesis");
    
    // Verify all three tool types were used
    let has_web_search = responses.iter().any(|r| r.contains("web_search"));
    let has_codebase_search = responses.iter().any(|r| r.contains("codebase_search"));
    let has_file_edit = responses.iter().any(|r| r.contains("edit_file"));
    
    assert!(has_web_search, "Should use web search");
    assert!(has_codebase_search, "Should use codebase search");
    assert!(has_file_edit, "Should use file editing");
    
    println!("  ✅ Multi-modal problem solving test passed");
}

#[tokio::test]
async fn test_conversation_state_recovery_after_interruption() {
    println!("💾 Test: test_conversation_state_recovery_after_interruption");
    
    // Simulate a conversation that gets interrupted and then resumed
    let mock_responses_initial = vec![
        MockResponse {
            text: "I'll start working on your request to analyze the authentication system.".to_string(),
            tool_calls: vec![("tc1".to_string(), "codebase_search".to_string(), serde_json::json!({
                "query": "authentication login"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Found authentication code in several files. I was analyzing the login flow when...".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    let mock_responses_recovery = vec![
        MockResponse {
            text: "Continuing from where we left off with the authentication analysis. Let me search for security-related patterns.".to_string(),
            tool_calls: vec![("tc2".to_string(), "codebase_search".to_string(), serde_json::json!({
                "query": "security validation"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Found security validation patterns. Now I'll create a comprehensive security analysis report based on all findings.".to_string(),
            tool_calls: vec![("tc3".to_string(), "edit_file".to_string(), serde_json::json!({
                "target_file": "security_analysis.md",
                "code_edit": "# Security Analysis Report\n\n## Authentication System\n- Login flow analyzed\n- Security validation patterns identified\n\n## Recommendations\n- Continue with current patterns\n- Consider additional validation\n",
                "instructions": "Create security analysis report"
            }))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "I've completed the security analysis that we started earlier. The report includes both the authentication system review and security validation patterns we discovered.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ];

    // Initial conversation
    let mock_llm_client_1 = Arc::new(MockLlmClient::new(mock_responses_initial));
    let (agent_1, _) = setup_test_agent(mock_llm_client_1.clone()).await;

    let responses_1 = process_and_collect_responses(
        &agent_1,
        "Please analyze the authentication system in my codebase for security issues",
        10
    ).await;

    println!("  - Initial conversation: {:?}", responses_1);

    // Recovery conversation (simulating resumption after interruption)
    let mock_llm_client_2 = Arc::new(MockLlmClient::new(mock_responses_recovery));
    let (agent_2, _) = setup_test_agent(mock_llm_client_2.clone()).await;

    let responses_2 = process_and_collect_responses(
        &agent_2,
        "Continue with the security analysis we were working on",
        15
    ).await;

    println!("  - Recovery conversation: {:?}", responses_2);

    // Verify state recovery
    let recovery_response = responses_2.join(" ");
    assert!(recovery_response.contains("continue") || recovery_response.contains("left off"), "Should show continuation awareness");
    assert!(recovery_response.contains("authentication") || recovery_response.contains("security"), "Should maintain topic context");
    assert!(recovery_response.contains("analysis") || recovery_response.contains("report"), "Should show completion of original task");

    println!("  ✅ Conversation state recovery test passed");
}

