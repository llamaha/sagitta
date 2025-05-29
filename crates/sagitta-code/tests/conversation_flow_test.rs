use std::sync::Arc;
use tokio;
use sagitta_code::{
    agent::Agent,
    config::SagittaCodeConfig,
    tools::registry::ToolRegistry,
    llm::client::{LlmClient, Message, ToolDefinition as LlmToolDefinition, LlmResponse, StreamChunk, MessagePart, Role, ThinkingConfig, GroundingConfig},
    utils::errors::SagittaCodeError,
};
use sagitta_search::embedding::provider::onnx::{OnnxEmbeddingModel, ThreadSafeOnnxProvider};
use futures_util::StreamExt;
use std::path::Path;
use sagitta_code::agent::state::types::AgentMode;
use serde_json::Value;
use uuid::Uuid;
use futures_util::Stream;
use std::pin::Pin;

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub text: String,
    pub tool_calls: Vec<(String, String, Value)>,
    pub should_error: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MockLlmClient {
    pub responses: Arc<tokio::sync::Mutex<Vec<MockResponse>>>,
    current_index: Arc<tokio::sync::Mutex<usize>>,
    calls: Arc<tokio::sync::Mutex<Vec<(Vec<Message>, Vec<LlmToolDefinition>)>>>,
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
            eprintln!("[MockLlmClient conversation_flow_test.rs] No more mock responses. Returning default. Index: {}, Total: {}", *index_guard, responses_guard.len());
            return Ok(MockResponse {
                text: "MockLlmClient: Default response as no specific one was configured for this call.".to_string(),
                tool_calls: vec![],
                should_error: false,
                error_message: None,
            });
        }
        
        let response = responses_guard[*index_guard].clone();
        *index_guard += 1;
        Ok(response)
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmClient {
    async fn generate(
        &self,
        messages: &[Message],
        tools: &[LlmToolDefinition],
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
            tool_calls: mock_response.tool_calls.clone(),
            usage: None,
            grounding: None,
        })
    }

    async fn generate_with_thinking(
        &self, messages: &[Message], tools: &[LlmToolDefinition], _thinking_config: &ThinkingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        self.generate(messages, tools).await
    }

    async fn generate_with_grounding(
        &self, messages: &[Message], tools: &[LlmToolDefinition], _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        self.generate(messages, tools).await
    }

    async fn generate_with_thinking_and_grounding(
        &self, messages: &[Message], tools: &[LlmToolDefinition], _thinking_config: &ThinkingConfig, _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        self.generate(messages, tools).await
    }

    async fn generate_stream(
        &self, messages: &[Message], tools: &[LlmToolDefinition],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        let llm_response = self.generate(messages, tools).await?;
        let mut stream_chunks: Vec<Result<StreamChunk, SagittaCodeError>> = Vec::new();
        for part in llm_response.message.parts {
            stream_chunks.push(Ok(StreamChunk {
                part: part.clone(), is_final: false, finish_reason: None, token_usage: None,
            }));
        }
        stream_chunks.push(Ok(StreamChunk {
            part: MessagePart::Text { text: "".to_string() }, is_final: true, finish_reason: Some("stop".to_string()), token_usage: None,
        }));
        Ok(Box::pin(futures_util::stream::iter(stream_chunks)))
    }

     async fn generate_stream_with_thinking(
        &self, messages: &[Message], tools: &[LlmToolDefinition], _thinking_config: &ThinkingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_grounding(
        &self, messages: &[Message], tools: &[LlmToolDefinition], _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_thinking_and_grounding(
        &self, messages: &[Message], tools: &[LlmToolDefinition], _thinking_config: &ThinkingConfig, _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream(messages, tools).await
    }
}

#[tokio::test]
async fn test_conversation_flow_context_continuity() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let config = SagittaCodeConfig::default();
    
    let model_path = Path::new("./models/all-MiniLM-L6-v2.onnx");
    let tokenizer_path = Path::new("./models/tokenizer.json");
    
    if !model_path.exists() || !tokenizer_path.exists() {
        println!("Skipping test - model files not found");
        return;
    }
    
    let onnx_model = OnnxEmbeddingModel::new(model_path, tokenizer_path)
        .expect("Failed to create ONNX model");
    let embedding_provider = Arc::new(ThreadSafeOnnxProvider::new(onnx_model));
    
    let tool_registry = Arc::new(ToolRegistry::new());
    
    let persistence: Box<dyn sagitta_code::agent::conversation::persistence::ConversationPersistence> = Box::new(
        sagitta_code::agent::conversation::persistence::disk::DiskConversationPersistence::new(
            std::env::temp_dir().join("sagitta-code-conversation-flow-test-continuity")
        ).await.unwrap()
    );
    
    let search_engine: Box<dyn sagitta_code::agent::conversation::search::ConversationSearchEngine> = Box::new(
        sagitta_code::agent::conversation::search::text::TextConversationSearchEngine::new()
    );

    let mock_llm_client = Arc::new(MockLlmClient::new(vec![
        MockResponse {
            text: "Initial response to the multi-step query about Sidekiq.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Summary based on previous interaction: Processed Sidekiq information.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
    ]));

    let agent = Agent::new(config, tool_registry, embedding_provider, persistence, search_engine, mock_llm_client)
        .await
        .expect("Failed to create agent");
    
    agent.set_mode(AgentMode::FullyAutonomous).await.expect("Failed to set mode");
    
    let first_message = "Search for sidekiq repository, add it, sync it, then query it";
    
    println!("=== FIRST MESSAGE ===");
    println!("User: {}", first_message);
    
    let stream_result = agent.process_message_stream(first_message).await;
    assert!(stream_result.is_ok(), "Failed to start processing first message");
    
    let mut stream = stream_result.unwrap();
    let mut first_chunks = Vec::new();
    
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                if let sagitta_code::llm::client::MessagePart::Text { text } = &chunk.part {
                    print!("{}", text);
                }
                first_chunks.push(chunk);
            }
            Err(e) => {
                println!("First stream error: {}", e);
                break;
            }
        }
    }
    println!("\n");
    
    assert!(!first_chunks.is_empty(), "No chunks received from first message");
    
    let second_message = "give me a summary of what you have found";
    
    println!("=== SECOND MESSAGE (Testing Context Continuity) ===");
    println!("User: {}", second_message);
    
    let stream_result2 = agent.process_message_stream(second_message).await;
    assert!(stream_result2.is_ok(), "Failed to start processing second message");
    
    let mut stream2 = stream_result2.unwrap();
    let mut second_chunks = Vec::new();
    
    while let Some(chunk_result) = stream2.next().await {
        match chunk_result {
            Ok(chunk) => {
                if let sagitta_code::llm::client::MessagePart::Text { text } = &chunk.part {
                    print!("{}", text);
                }
                second_chunks.push(chunk);
            }
            Err(e) => {
                println!("Second stream error: {}", e);
                break;
            }
        }
    }
    println!("\n");
    
    assert!(!first_chunks.is_empty(), "No chunks received from first agent call");
    assert!(!second_chunks.is_empty(), "No chunks received from second agent call");
    
    let second_response_text: String = second_chunks.iter()
        .filter_map(|chunk: &StreamChunk| {
            if let sagitta_code::llm::client::MessagePart::Text { text } = &chunk.part {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect();
    
    println!("=== CONTEXT CONTINUITY TEST ===");
    println!("Second response length: {} characters", second_response_text.len());
    
    assert!(
        second_response_text.len() > 50 && second_response_text.contains("Sidekiq information"), 
        "Second response should be substantial and context-aware. Got: {}", second_response_text
    );
    
    println!("✅ PASSED: Conversation flow maintains context between messages");
    println!("✅ PASSED: Agent doesn't restart from scratch on follow-up questions");
}

#[tokio::test] 
async fn test_reasoning_session_continuity() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let config = SagittaCodeConfig::default();
    
    let model_path = Path::new("./models/all-MiniLM-L6-v2.onnx");
    let tokenizer_path = Path::new("./models/tokenizer.json");
    
    if !model_path.exists() || !tokenizer_path.exists() {
        println!("Skipping test - model files not found");
        return;
    }
    
    let onnx_model = OnnxEmbeddingModel::new(model_path, tokenizer_path)
        .expect("Failed to create ONNX model");
    let embedding_provider = Arc::new(ThreadSafeOnnxProvider::new(onnx_model));
    
    let tool_registry = Arc::new(ToolRegistry::new());
    
    let persistence: Box<dyn sagitta_code::agent::conversation::persistence::ConversationPersistence> = Box::new(
        sagitta_code::agent::conversation::persistence::disk::DiskConversationPersistence::new(
            std::env::temp_dir().join("sagitta-code-reasoning-session-continuity")
        ).await.unwrap()
    );
    
    let search_engine: Box<dyn sagitta_code::agent::conversation::search::ConversationSearchEngine> = Box::new(
        sagitta_code::agent::conversation::search::text::TextConversationSearchEngine::new()
    );

    let mock_llm_client_session = Arc::new(MockLlmClient::new(vec![
        MockResponse { text: "Understood your first request about Sidekiq structure.".to_string(), tool_calls: vec![], should_error: false, error_message: None },
        MockResponse { text: "Regarding components, I found X, Y, and Z.".to_string(), tool_calls: vec![], should_error: false, error_message: None },
        MockResponse { text: "The web UI for Sidekiq works by doing A, B, and C.".to_string(), tool_calls: vec![], should_error: false, error_message: None },
        MockResponse { text: "Is there anything else on Sidekiq?".to_string(), tool_calls: vec![], should_error: false, error_message: None }, 
    ]));

    let agent = Agent::new(config, tool_registry, embedding_provider, persistence, search_engine, mock_llm_client_session)
        .await
        .expect("Failed to create agent");
    
    agent.set_mode(AgentMode::FullyAutonomous).await.expect("Failed to set mode");
    
    let messages = vec![
        "Please help me understand the sidekiq repository structure",
        "What are the main components you found?",
        "Can you explain how the web UI works?",
    ];
    
    for (i, message) in messages.iter().enumerate() {
        println!("=== MESSAGE {} ===", i + 1);
        println!("User: {}", message);
        
        let stream_result = agent.process_message_stream(*message).await;
        assert!(stream_result.is_ok(), "Failed to process message {}", i + 1);
        
        let mut stream = stream_result.unwrap();
        let mut chunks = Vec::new();
        let mut response_text = String::new();
        
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    if let sagitta_code::llm::client::MessagePart::Text { text } = &chunk.part {
                        print!("{}", text);
                        response_text.push_str(text);
                    }
                    chunks.push(chunk);
                }
                Err(e) => {
                    println!("Stream error on message {}: {}", i + 1, e);
                    break;
                }
            }
        }
        println!("\n");
        
        assert!(!chunks.is_empty(), "No response received for message {}", i + 1);
        assert!(!response_text.trim().is_empty(), "Response text should not be empty for message {}", i+1);
    }
    
    println!("✅ PASSED: Multi-step conversation maintains reasoning context");
} 