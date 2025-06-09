use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use serde_json::json;

use sagitta_code::{
    agent::Agent,
    config::SagittaCodeConfig,
    tools::registry::ToolRegistry,
    tools::repository::add::AddRepositoryTool,
    tools::shell_execution::ShellExecutionTool,
    agent::events::AgentEvent,
    agent::conversation::persistence::disk::DiskConversationPersistence,
    agent::conversation::search::text::TextConversationSearchEngine,
    llm::client::{LlmClient, LlmResponse, Message, Role, StreamChunk, MessagePart, ToolDefinition, ThinkingConfig, GroundingConfig},
    utils::errors::SagittaCodeError,
    reasoning::{AgentToolExecutor, RecoveryStrategy, LoopDetectionInfo},
    tools::types::Tool,
};
use sagitta_embed::provider::EmbeddingProvider;

use async_trait::async_trait;
use serde_json::Value;
use futures_util::Stream;
use futures_util::stream;
use std::pin::Pin;
use tempfile;
use uuid::Uuid;
use std::sync::Mutex as StdMutex;

/// Mock embedding provider for testing
#[derive(Debug, Clone)]
struct MockEmbeddingProvider {
    dimension: usize,
}

impl MockEmbeddingProvider {
    fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

impl EmbeddingProvider for MockEmbeddingProvider {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_type(&self) -> sagitta_embed::EmbeddingModelType {
        sagitta_embed::EmbeddingModelType::Default
    }

    fn embed_batch(&self, texts: &[&str]) -> sagitta_embed::Result<Vec<Vec<f32>>> {
        // Return mock embeddings - just simple patterns for testing
        let embeddings = texts.iter()
            .map(|_| {
                (0..self.dimension)
                    .map(|i| (i as f32) / (self.dimension as f32)) // Simple pattern
                    .collect()
            })
            .collect();
        Ok(embeddings)
    }
}

/// Mock LLM client that simulates problematic tool calling patterns
struct ProblematicMockLlmClient {
    /// Tool calls to return - will loop through these
    tool_calls: Arc<StdMutex<Vec<(String, Value)>>>,
    /// Current call index
    call_index: Arc<StdMutex<usize>>,
    /// Response text to send
    response_text: String,
}

impl ProblematicMockLlmClient {
    fn new(tool_calls: Vec<(String, Value)>, response_text: String) -> Self {
        Self {
            tool_calls: Arc::new(StdMutex::new(tool_calls)),
            call_index: Arc::new(StdMutex::new(0)),
            response_text,
        }
    }
}

#[async_trait]
impl LlmClient for ProblematicMockLlmClient {
    async fn generate(&self, _messages: &[Message], _tools: &[ToolDefinition]) -> Result<LlmResponse, SagittaCodeError> {
        let tool_calls = self.tool_calls.lock().unwrap();
        let mut call_index = self.call_index.lock().unwrap();
        
        if tool_calls.is_empty() {
            return Ok(LlmResponse {
                message: Message {
                    id: Uuid::new_v4(),
                    role: Role::Assistant,
                    parts: vec![MessagePart::Text { text: self.response_text.clone() }],
                    metadata: std::collections::HashMap::new(),
                },
                tool_calls: vec![],
                usage: None,
                grounding: None,
            });
        }
        
        // Return the same tool call repeatedly to simulate the loop problem
        let (tool_name, tool_args) = &tool_calls[*call_index % tool_calls.len()];
        *call_index += 1;
        
        Ok(LlmResponse {
            message: Message {
                id: Uuid::new_v4(),
                role: Role::Assistant,
                parts: vec![MessagePart::Text { text: format!("{} (call #{})", self.response_text, *call_index) }],
                metadata: std::collections::HashMap::new(),
            },
            tool_calls: vec![(
                Uuid::new_v4().to_string(),
                tool_name.clone(),
                tool_args.clone(),
            )],
            usage: None,
            grounding: None,
        })
    }

    async fn generate_with_thinking(&self, messages: &[Message], tools: &[ToolDefinition], _thinking_config: &ThinkingConfig) -> Result<LlmResponse, SagittaCodeError> {
        self.generate(messages, tools).await
    }

    async fn generate_with_grounding(&self, messages: &[Message], tools: &[ToolDefinition], _grounding_config: &GroundingConfig) -> Result<LlmResponse, SagittaCodeError> {
        self.generate(messages, tools).await
    }

    async fn generate_with_thinking_and_grounding(&self, messages: &[Message], tools: &[ToolDefinition], thinking_config: &ThinkingConfig, grounding_config: &GroundingConfig) -> Result<LlmResponse, SagittaCodeError> {
        self.generate_with_thinking(messages, tools, thinking_config).await
    }

    async fn generate_stream(&self, messages: &[Message], tools: &[ToolDefinition]) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        let response = self.generate(messages, tools).await?;
        
        let mut chunks: Vec<Result<StreamChunk, SagittaCodeError>> = vec![];
        
        // Send text content
        for part in &response.message.parts {
            match part {
                MessagePart::Text { text } => {
                    chunks.push(Ok(StreamChunk {
                        part: MessagePart::Text { text: text.clone() },
                        is_final: false,
                        finish_reason: None,
                        token_usage: None,
                    }));
                }
                _ => {}
            }
        }
        
        // Send tool calls
        for (call_id, name, args) in &response.tool_calls {
            chunks.push(Ok(StreamChunk {
                part: MessagePart::ToolCall {
                    tool_call_id: call_id.clone(),
                    name: name.clone(),
                    parameters: args.clone(),
                },
                is_final: false,
                finish_reason: None,
                token_usage: None,
            }));
        }
        
        // Send final chunk
        chunks.push(Ok(StreamChunk {
            part: MessagePart::Text { text: "".to_string() },
            is_final: true,
            finish_reason: Some("stop".to_string()),
            token_usage: None,
        }));
        
        Ok(Box::pin(stream::iter(chunks)))
    }

    async fn generate_stream_with_thinking(&self, messages: &[Message], tools: &[ToolDefinition], _thinking_config: &ThinkingConfig) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_grounding(&self, messages: &[Message], tools: &[ToolDefinition], _grounding_config: &GroundingConfig) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_thinking_and_grounding(&self, messages: &[Message], tools: &[ToolDefinition], thinking_config: &ThinkingConfig, grounding_config: &GroundingConfig) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream_with_thinking(messages, tools, thinking_config).await
    }
}

async fn create_test_agent_with_problematic_llm(tool_calls: Vec<(String, Value)>) -> Result<Agent, SagittaCodeError> {
    let config = SagittaCodeConfig::default();
    let tool_registry = Arc::new(ToolRegistry::new());
    
    // Add the AddRepositoryTool to the registry so we can test it
    let search_config = sagitta_search::config::AppConfig::default();
    let repo_manager = sagitta_code::gui::repository::manager::RepositoryManager::new(
        Arc::new(tokio::sync::Mutex::new(search_config))
    );
    let add_repo_tool = Arc::new(AddRepositoryTool::new(Arc::new(tokio::sync::Mutex::new(repo_manager))));
    tool_registry.register(add_repo_tool).await.unwrap();
    
    // Add the ShellExecutionTool to the registry for shell command tests
    let shell_tool = Arc::new(ShellExecutionTool::new(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))));
    tool_registry.register(shell_tool).await.unwrap();
    
    let embedding_provider = Arc::new(MockEmbeddingProvider::new(384));
    
    // Create temporary storage for persistence
    let temp_dir = tempfile::tempdir().unwrap();
    let persistence = Box::new(
        DiskConversationPersistence::new(temp_dir.path().to_path_buf())
            .await
            .unwrap()
    );
    
    let search_engine = Box::new(TextConversationSearchEngine::new());
    
    let llm_client = Arc::new(ProblematicMockLlmClient::new(
        tool_calls,
        "I'll help you with that task.".to_string()
    ));
    
    Agent::new(
        config,
        tool_registry,
        embedding_provider,
        persistence,
        search_engine,
        llm_client,
    ).await
}

#[tokio::test]
async fn test_loop_detection_prevents_infinite_calls() {
    println!("🔍 Test: Loop detection prevents infinite tool calls");
    
    // Create an agent that will repeatedly call add_repository with invalid parameters
    let problematic_calls = vec![
        ("add_repository".to_string(), json!({
            "name": "test-repo"
            // Missing url and local_path - will fail validation
        }))
    ];
    
    let agent = create_test_agent_with_problematic_llm(problematic_calls).await.unwrap();
    let mut event_receiver = agent.subscribe();
    
    let message = "Add a repository for my project";
    let _result = agent.process_message_stream(message).await;
    
    let mut tool_call_count = 0;
    let mut loop_detected = false;
    let mut recovery_suggested = false;
    let mut workflow_continued = false;
    
    let timeout = tokio::time::timeout(Duration::from_secs(15), async {
        while let Ok(event) = event_receiver.recv().await {
            match event {
                AgentEvent::ToolCompleted { tool_name, success, .. } => {
                    if tool_name == "add_repository" {
                        tool_call_count += 1;
                        println!("DEBUG: Tool call #{}: {} (success: {})", tool_call_count, tool_name, success);
                        
                        // After 3 calls, loop detection should kick in
                        if tool_call_count >= 3 && !success {
                            println!("⚠️ Multiple failed calls detected - loop detection should activate");
                        }
                    }
                }
                AgentEvent::LlmChunk { content, .. } => {
                    if content.contains("🔄") || content.contains("Loop Detected") {
                        loop_detected = true;
                        println!("✅ Loop detection message found: {}", content);
                    }
                    
                    if content.contains("Recommended Action") || content.contains("Alternative") || content.contains("Skip") {
                        recovery_suggested = true;
                        println!("✅ Recovery strategy suggested: {}", content);
                    }
                    
                    if content.contains("continue") || content.contains("proceed") {
                        workflow_continued = true;
                        println!("✅ Workflow continuation indicated: {}", content);
                    }
                }
                AgentEvent::Error(error_msg) => {
                    if error_msg.contains("Loop detected") || error_msg.contains("skipped") {
                        loop_detected = true;
                        println!("✅ Loop detection in error: {}", error_msg);
                    }
                }
                _ => {}
            }
            
            // Stop the test if loop detection is working and suggests recovery
            if loop_detected && recovery_suggested {
                println!("✅ Loop detection and recovery system working correctly");
                break;
            }
            
            // Safety valve - if we see too many calls without loop detection, something is wrong
            if tool_call_count >= 5 {
                println!("❌ Too many tool calls without loop detection - system may be broken");
                break;
            }
        }
    }).await;
    
    println!("Final results:");
    println!("  Tool calls: {}", tool_call_count);
    println!("  Loop detected: {}", loop_detected);
    println!("  Recovery suggested: {}", recovery_suggested);
    println!("  Workflow continued: {}", workflow_continued);
    
    // The enhanced system should detect loops and suggest recovery
    assert!(tool_call_count >= 2, "Expected multiple tool calls to trigger loop detection");
    assert!(loop_detected, "Expected loop detection to trigger");
    assert!(recovery_suggested, "Expected recovery strategy to be suggested");
}

#[tokio::test]
async fn test_graceful_degradation_with_tool_skipping() {
    println!("🔍 Test: Graceful degradation with tool skipping");
    
    // Create a sequence that will fail repeatedly, then succeed
    let mixed_calls = vec![
        ("add_repository".to_string(), json!({"name": "bad-repo"})), // Will fail
        ("shell_execution".to_string(), json!({"command": "echo 'test'"})), // Should work
    ];
    
    let agent = create_test_agent_with_problematic_llm(mixed_calls).await.unwrap();
    let mut event_receiver = agent.subscribe();
    
    let message = "Set up my development environment and run a test";
    let _result = agent.process_message_stream(message).await;
    
    let mut add_repo_failures = 0;
    let mut shell_executions = 0;
    let mut tool_skipped = false;
    let mut graceful_degradation = false;
    
    let timeout = tokio::time::timeout(Duration::from_secs(15), async {
        while let Ok(event) = event_receiver.recv().await {
            match event {
                AgentEvent::ToolCompleted { tool_name, success, .. } => {
                    println!("DEBUG: Tool '{}' completed, success: {}", tool_name, success);
                    
                    if tool_name == "add_repository" {
                        if !success {
                            add_repo_failures += 1;
                        }
                    } else if tool_name == "shell_execution" {
                        shell_executions += 1;
                        if success {
                            println!("✅ Shell execution succeeded despite repository failures");
                        }
                    }
                }
                AgentEvent::LlmChunk { content, .. } => {
                    if content.contains("Skipping") || content.contains("skip") {
                        tool_skipped = true;
                        println!("✅ Tool skipping detected: {}", content);
                    }
                    
                    if content.contains("continue") || content.contains("proceed") || content.contains("workflow") {
                        graceful_degradation = true;
                        println!("✅ Graceful degradation message: {}", content);
                    }
                }
                _ => {}
            }
            
            // Break when we see evidence of the system working correctly
            if (tool_skipped || graceful_degradation) && shell_executions > 0 {
                println!("✅ Graceful degradation system working - continuing workflow despite failures");
                break;
            }
            
            // Safety valve
            if add_repo_failures >= 5 {
                break;
            }
        }
    }).await;
    
    println!("Final results:");
    println!("  Repository failures: {}", add_repo_failures);
    println!("  Shell executions: {}", shell_executions);
    println!("  Tool skipped: {}", tool_skipped);
    println!("  Graceful degradation: {}", graceful_degradation);
    
    // The system should handle failures gracefully and continue with other tools
    assert!(add_repo_failures > 0, "Expected repository tool to fail");
    assert!(tool_skipped || graceful_degradation, "Expected graceful degradation behavior");
}

#[tokio::test]
async fn test_enhanced_parameter_validation_with_helpful_feedback() {
    println!("🔍 Test: Enhanced parameter validation with helpful feedback");
    
    // Test the exact scenario from the conversation
    let invalid_calls = vec![
        ("add_repository".to_string(), json!({
            "name": "fibonacci_calculator"
            // Missing both url and local_path
        }))
    ];
    
    let agent = create_test_agent_with_problematic_llm(invalid_calls).await.unwrap();
    let mut event_receiver = agent.subscribe();
    
    let message = "Create a new project, a fibonacci calculator in rust";
    let _result = agent.process_message_stream(message).await;
    
    let mut parameter_error_caught = false;
    let mut helpful_guidance_provided = false;
    let mut alternative_tool_suggested = false;
    let mut quick_fix_provided = false;
    
    let timeout = tokio::time::timeout(Duration::from_secs(10), async {
        while let Ok(event) = event_receiver.recv().await {
            match event {
                AgentEvent::LlmChunk { content, .. } => {
                    println!("DEBUG: Validation feedback: {}", content);
                    
                    if content.contains("parameter") && content.contains("validation") {
                        parameter_error_caught = true;
                        println!("✅ Parameter validation error caught");
                    }
                    
                    if content.contains("**Quick Fix**") {
                        quick_fix_provided = true;
                        println!("✅ Quick fix guidance provided");
                    }
                    
                    if content.contains("must provide EITHER") && content.contains("`url`") && content.contains("`local_path`") {
                        helpful_guidance_provided = true;
                        println!("✅ Helpful parameter guidance provided");
                    }
                    
                    if content.contains("create_project") && content.contains("Alternative") {
                        alternative_tool_suggested = true;
                        println!("✅ Alternative tool suggested (create_project)");
                    }
                }
                AgentEvent::Error(error_msg) => {
                    if error_msg.contains("Parameter validation failed") {
                        parameter_error_caught = true;
                    }
                }
                _ => {}
            }
            
            // Break when we have comprehensive feedback
            if parameter_error_caught && helpful_guidance_provided && alternative_tool_suggested {
                println!("✅ Enhanced parameter validation system working correctly");
                break;
            }
        }
    }).await;
    
    println!("Final results:");
    println!("  Parameter error caught: {}", parameter_error_caught);
    println!("  Helpful guidance: {}", helpful_guidance_provided);
    println!("  Alternative suggested: {}", alternative_tool_suggested);
    println!("  Quick fix provided: {}", quick_fix_provided);
    
    // The enhanced system should provide comprehensive parameter validation feedback
    assert!(parameter_error_caught, "Expected parameter validation error to be caught");
    assert!(helpful_guidance_provided, "Expected helpful parameter guidance");
    assert!(alternative_tool_suggested, "Expected alternative tool to be suggested");
}

#[tokio::test]
async fn test_recovery_strategies_are_tool_specific() {
    println!("🔍 Test: Recovery strategies are specific to tool types");
    
    // Test different tools to ensure they get appropriate recovery strategies
    let various_failing_calls = vec![
        ("add_repository".to_string(), json!({"name": "test"})),
        ("create_project".to_string(), json!({"name": "test"})),
        ("shell_execution".to_string(), json!({"command": ""})),
    ];
    
    let agent = create_test_agent_with_problematic_llm(various_failing_calls).await.unwrap();
    let mut event_receiver = agent.subscribe();
    
    let message = "Help me set up my development environment";
    let _result = agent.process_message_stream(message).await;
    
    let mut add_repo_recovery = String::new();
    let mut create_project_recovery = String::new();
    let mut shell_execution_recovery = String::new();
    
    let timeout = tokio::time::timeout(Duration::from_secs(15), async {
        while let Ok(event) = event_receiver.recv().await {
            match event {
                AgentEvent::LlmChunk { content, .. } => {
                    if content.contains("add_repository") && content.contains("Recovery") || content.contains("Alternative") {
                        add_repo_recovery = content.clone();
                        println!("✅ Add repository recovery strategy: {}", content);
                    }
                    
                    if content.contains("create_project") && (content.contains("Critical") || content.contains("essential")) {
                        create_project_recovery = content.clone();
                        println!("✅ Create project recovery strategy: {}", content);
                    }
                    
                    if content.contains("shell_execution") && content.contains("smaller steps") {
                        shell_execution_recovery = content.clone();
                        println!("✅ Shell execution recovery strategy: {}", content);
                    }
                }
                _ => {}
            }
            
            // Break when we have recovery strategies for different tool types
            if !add_repo_recovery.is_empty() && (!create_project_recovery.is_empty() || !shell_execution_recovery.is_empty()) {
                break;
            }
        }
    }).await;
    
    println!("Final results:");
    println!("  Add repository recovery: {}", !add_repo_recovery.is_empty());
    println!("  Create project recovery: {}", !create_project_recovery.is_empty());
    println!("  Shell execution recovery: {}", !shell_execution_recovery.is_empty());
    
    // The system should provide tool-specific recovery strategies
    assert!(!add_repo_recovery.is_empty(), "Expected add_repository specific recovery strategy");
    // At least one other tool should have a specific strategy
    assert!(!create_project_recovery.is_empty() || !shell_execution_recovery.is_empty(), 
           "Expected tool-specific recovery strategies for different tools");
} 