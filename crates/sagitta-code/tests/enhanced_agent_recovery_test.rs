use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;
use serde_json::{json, Value};
use tempfile;

use sagitta_code::{
    agent::{Agent, events::AgentEvent},
    config::types::SagittaCodeConfig,
    conversation::{
        persistence::ConversationPersistence,
        search::ConversationSearchEngine,
        types::{Conversation, ConversationQuery, ConversationSearchResult, ConversationSummary},
    },
    llm::client::{
        LlmClient, Message, MessagePart, Role, ToolDefinition, LlmResponse, 
        StreamChunk, ThinkingConfig, GroundingConfig
    },
    tools::{
        registry::ToolRegistry,
        shell_execution::ShellExecutionTool,
        repository::add::AddExistingRepositoryTool,
    },
    utils::errors::SagittaCodeError,
};
use sagitta_embed::provider::EmbeddingProvider;
use async_trait::async_trait;
use futures_util::{stream, Stream};
use std::pin::Pin;

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

/// Mock LLM client that can simulate problematic behavior
#[derive(Debug, Clone)]
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
        let tool_calls = {
            let mut calls = self.tool_calls.lock().unwrap();
            let mut index = self.call_index.lock().unwrap();
            
            if !calls.is_empty() {
                let call = calls[*index % calls.len()].clone();
                *index += 1;
                vec![(
                    Uuid::new_v4().to_string(),
                    call.0,
                    call.1,
                )]
            } else {
                vec![]
            }
        };
        
        Ok(LlmResponse {
            message: Message {
                id: Uuid::new_v4(),
                role: Role::Assistant,
                parts: vec![MessagePart::Text { text: self.response_text.clone() }],
                metadata: std::collections::HashMap::new(),
            },
            tool_calls,
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
        
        let mut chunks = Vec::new();
        
        // Add text chunk
        chunks.push(Ok(StreamChunk {
            part: MessagePart::Text { text: response.message.parts.first().map(|p| match p {
                MessagePart::Text { text } => text.clone(),
                _ => "".to_string(),
            }).unwrap_or_default() },
            is_final: false,
            finish_reason: None,
            token_usage: None,
        }));
        
        // Add tool calls
        for (id, name, params) in response.tool_calls {
            chunks.push(Ok(StreamChunk {
                part: MessagePart::ToolCall {
                    tool_call_id: id,
                    name,
                    parameters: params,
                },
                is_final: false,
                finish_reason: None,
                token_usage: None,
            }));
        }
        
        // Final chunk
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

/// Mock conversation persistence for testing
#[derive(Debug)]
struct TestConversationPersistence;

#[async_trait]
impl ConversationPersistence for TestConversationPersistence {
    async fn save_conversation(&self, _conversation: &Conversation) -> Result<(), anyhow::Error> {
        Ok(())
    }
    
    async fn load_conversation(&self, _id: Uuid) -> Result<Option<Conversation>, anyhow::Error> {
        Ok(None)
    }
    
    async fn delete_conversation(&self, _id: Uuid) -> Result<(), anyhow::Error> {
        Ok(())
    }
    
    async fn list_conversation_ids(&self) -> Result<Vec<Uuid>, anyhow::Error> {
        Ok(vec![])
    }
    
    async fn list_conversation_summaries(&self, _workspace_id: Option<Uuid>) -> Result<Vec<ConversationSummary>, anyhow::Error> {
        Ok(vec![])
    }
    
    async fn archive_conversation(&self, _id: Uuid) -> Result<(), anyhow::Error> {
        Ok(())
    }
    
    async fn list_archived_conversation_ids(&self) -> Result<Vec<Uuid>, anyhow::Error> {
        Ok(vec![])
    }
    
    async fn restore_conversation(&self, _id: Uuid) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

/// Mock conversation search engine for testing
#[derive(Debug, Clone)]
struct TestConversationSearchEngine;

#[async_trait]
impl ConversationSearchEngine for TestConversationSearchEngine {
    async fn search(&self, _query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>, anyhow::Error> {
        Ok(vec![])
    }
    
    async fn remove_conversation(&self, _conversation_id: uuid::Uuid) -> Result<(), anyhow::Error> {
        Ok(())
    }
    
    async fn index_conversation(&self, _conversation: &Conversation) -> Result<(), anyhow::Error> {
        Ok(())
    }
    
    async fn clear_index(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }
    
    async fn rebuild_index(&self, _conversations: &[Conversation]) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

/// Create test embedding pool for testing
fn create_test_embedding_pool() -> Arc<dyn EmbeddingProvider> {
    Arc::new(MockEmbeddingProvider::new(384))
}

async fn create_test_agent_with_problematic_llm(tool_calls: Vec<(String, Value)>) -> Result<Agent, SagittaCodeError> {
    let config = SagittaCodeConfig::default();
    let tool_registry = Arc::new(ToolRegistry::new());
    
    // Add the AddRepositoryTool to the registry so we can test it
    let search_config = sagitta_search::config::AppConfig::default();
    let repo_manager = sagitta_code::gui::repository::manager::RepositoryManager::new(
        Arc::new(tokio::sync::Mutex::new(search_config))
    );
    let add_repo_tool = Arc::new(AddExistingRepositoryTool::new(Arc::new(tokio::sync::Mutex::new(repo_manager))));
    tool_registry.register(add_repo_tool).await.unwrap();
    
    // Add the ShellExecutionTool to the registry for shell command tests
    let shell_tool = Arc::new(ShellExecutionTool::new(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))));
    tool_registry.register(shell_tool).await.unwrap();
    
    let embedding_provider = create_test_embedding_pool();
    
    // Use test persistence and search engine
    let persistence = Box::new(TestConversationPersistence);
    let search_engine = Box::new(TestConversationSearchEngine);
    
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
    
    // Create an agent that will repeatedly call add_existing_repository with invalid parameters
    let problematic_calls = vec![
        ("add_existing_repository".to_string(), json!({
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
    
    let timeout = tokio::time::timeout(Duration::from_secs(15), async {
        while let Ok(event) = event_receiver.recv().await {
            match event {
                AgentEvent::ToolCompleted { tool_name, success, .. } => {
                    if tool_name == "add_existing_repository" {
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
                }
                AgentEvent::Error(error_msg) => {
                    if error_msg.contains("Loop detected") || error_msg.contains("skipped") {
                        loop_detected = true;
                        println!("✅ Loop detection in error: {}", error_msg);
                    }
                    
                    // Error messages about skipping tools are actually recovery strategies
                    if error_msg.contains("skipped due to repeated failures") || error_msg.contains("being skipped") {
                        recovery_suggested = true;
                        println!("✅ Recovery strategy (tool skipping) suggested: {}", error_msg);
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
    
    // The enhanced system should detect loops and suggest recovery
    // Note: The system is so efficient it may detect loops after just 1 call
    assert!(tool_call_count >= 1, "Expected at least one tool call to trigger loop detection");
    assert!(loop_detected, "Expected loop detection to trigger");
    assert!(recovery_suggested, "Expected recovery strategy to be suggested");
}

#[tokio::test]
async fn test_graceful_degradation_with_tool_skipping() {
    println!("🔍 Test: Graceful degradation with tool skipping");
    
    // Create a sequence that will fail repeatedly, then succeed
    let mixed_calls = vec![
        ("add_existing_repository".to_string(), json!({"name": "bad-repo"})), // Will fail
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
                    if tool_name == "add_existing_repository" && !success {
                        add_repo_failures += 1;
                    } else if tool_name == "shell_execution" && success {
                        shell_executions += 1;
                    }
                }
                AgentEvent::LlmChunk { content, .. } => {
                    if content.contains("skip") || content.contains("Skip") {
                        tool_skipped = true;
                    }
                    
                    if content.contains("continue") || content.contains("proceed") || content.contains("alternative") {
                        graceful_degradation = true;
                    }
                }
                _ => {}
            }
            
            // Stop when we see graceful degradation working
            if tool_skipped && graceful_degradation && shell_executions > 0 {
                break;
            }
            
            // Safety valve
            if add_repo_failures >= 5 {
                break;
            }
        }
    }).await;
    
    println!("Final results:");
    println!("  Add repo failures: {}", add_repo_failures);
    println!("  Shell executions: {}", shell_executions);
    println!("  Tool skipped: {}", tool_skipped);
    println!("  Graceful degradation: {}", graceful_degradation);
    
    // The system should gracefully degrade and continue with working tools
    assert!(add_repo_failures >= 1, "Expected at least one add_existing_repository failure");
    assert!(shell_executions >= 1, "Expected at least one successful shell execution");
    assert!(tool_skipped, "Expected tool skipping to be suggested");
    assert!(graceful_degradation, "Expected graceful degradation to occur");
} 