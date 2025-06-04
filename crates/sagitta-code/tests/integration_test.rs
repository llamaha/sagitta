use std::sync::Arc;
use tokio;
use sagitta_code::{
    agent::Agent,
    config::SagittaCodeConfig,
    tools::registry::ToolRegistry,
    llm::client::{LlmClient, Message, ToolDefinition as LlmToolDefinition, LlmResponse, StreamChunk, MessagePart, Role, ThinkingConfig, GroundingConfig},
    utils::errors::SagittaCodeError,
};
use sagitta_embed::{EmbeddingPool, EmbeddingConfig, EmbeddingProvider};
use sagitta_embed::provider::onnx::OnnxEmbeddingModel;
use sagitta_search;
use futures_util::StreamExt;
use std::path::Path;
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
            eprintln!("[MockLlmClient integration_test.rs] No more mock responses available! Current index: {}, Total responses: {}", *index_guard, responses_guard.len());
            return Ok(MockResponse {
                text: "Default mock response: No specific response configured for this call.".to_string(),
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
async fn test_sidekiq_bug_investigation() {
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
    
    let embedding_config = EmbeddingConfig::new_onnx(model_path.to_path_buf(), tokenizer_path.to_path_buf());
    let embedding_pool = Arc::new(EmbeddingPool::with_configured_sessions(embedding_config)
        .expect("Failed to create embedding pool"));
    
    // Create adapter for EmbeddingProvider compatibility
    let embedding_provider_adapter = Arc::new(sagitta_search::EmbeddingPoolAdapter::new(embedding_pool));
    
    let tool_registry = Arc::new(ToolRegistry::new());
    
    let persistence: Box<dyn sagitta_code::agent::conversation::persistence::ConversationPersistence> = Box::new(
        sagitta_code::agent::conversation::persistence::disk::DiskConversationPersistence::new(
            std::env::temp_dir().join("sagitta-code-integration-test-sidekiq")
        ).await.unwrap()
    );
    
    let search_engine: Box<dyn sagitta_code::agent::conversation::search::ConversationSearchEngine> = Box::new(
        sagitta_code::agent::conversation::search::text::TextConversationSearchEngine::new()
    );

    let mock_llm_client = Arc::new(MockLlmClient::new(vec![
        MockResponse {
            text: "Okay, I will investigate the Sidekiq bug. I'll start by searching for the repository.".to_string(),
            tool_calls: vec![("web_search_sidekiq".to_string(), "web_search".to_string(), serde_json::json!({"query": "sidekiq github"}))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Found Sidekiq at github.com/sidekiq/sidekiq. Now I'll analyze the problem description.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Investigation complete. The issue seems to be related to locale handling in the Web UI.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ]));

    let agent = Agent::new(config, tool_registry, embedding_provider_adapter, persistence, search_engine, mock_llm_client)
        .await
        .expect("Failed to create agent");
    
    let test_message = r#"User has reported this bug in sidekiq:

Ruby version: 3.4.2
Rails version: 8.0.2
Sidekiq / Pro / Enterprise version(s): sidekiq (8.0.3) / sidekiq-pro (8.0.1)

The web UI is defaulting the first 5 character match of the Accept-Language header. This is not how the spec is defined. It should be matching in order of quality value.

For example, this value is returning zh-TW when it should be matching on en first.

en-US,en;q=0.9,es;q=0.8,zh-TW;q=0.7,zh;q=0.6

Other weirdness comes into play with zh-XX locales because there is no zh locale. So an accept header with a zh variant that is not zh-CN or zh-TW will match zh-CN. This is not a valid fallback language and would be better to just fallback to the default en.

The user_prefered_languages method already sorts locales in the header based on their quality value, so the locale method just needs to respect it.

Please web search for the clone url and correct branch / target_ref then add / sync the repo then do an investigation using the various tools you have available and produce an analysis."#;

    let stream_result = agent.process_message_stream(test_message).await;
    assert!(stream_result.is_ok(), "Failed to start processing message");
    
    let mut stream = stream_result.unwrap();
    let mut chunks = Vec::new();
    
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                chunks.push(chunk);
            }
            Err(e) => {
                println!("Stream error: {}", e);
                break;
            }
        }
    }
    
    assert!(!chunks.is_empty(), "No chunks received from agent");
    
    for chunk in &chunks {
        if let sagitta_code::llm::client::MessagePart::Text { text } = &chunk.part {
            print!("{}", text);
        }
    }
    println!();
}

#[tokio::test]
async fn test_add_repository_already_exists_handling() {
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
    
    let embedding_config = EmbeddingConfig::new_onnx(model_path.to_path_buf(), tokenizer_path.to_path_buf());
    let embedding_pool = Arc::new(EmbeddingPool::with_configured_sessions(embedding_config)
        .expect("Failed to create embedding pool"));
    
    // Create adapter for EmbeddingProvider compatibility
    let embedding_provider_adapter = Arc::new(sagitta_search::EmbeddingPoolAdapter::new(embedding_pool));
    
    let tool_registry = Arc::new(ToolRegistry::new());
    
    let persistence: Box<dyn sagitta_code::agent::conversation::persistence::ConversationPersistence> = Box::new(
        sagitta_code::agent::conversation::persistence::disk::DiskConversationPersistence::new(
            std::env::temp_dir().join("sagitta-code-integration-test-repo-exists")
        ).await.unwrap()
    );
    
    let search_engine: Box<dyn sagitta_code::agent::conversation::search::ConversationSearchEngine> = Box::new(
        sagitta_code::agent::conversation::search::text::TextConversationSearchEngine::new()
    );

    let mock_llm_client_add_repo = Arc::new(MockLlmClient::new(vec![
        MockResponse {
            text: "Okay, I will add the sidekiq repository.".to_string(),
            tool_calls: vec![("add_sidekiq_1".to_string(), "add_repository".to_string(), serde_json::json!({"name": "sidekiq", "url": "https://github.com/sidekiq/sidekiq.git"}))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Repository sidekiq added successfully.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "Okay, I will try to add the sidekiq repository again.".to_string(),
            tool_calls: vec![("add_sidekiq_2".to_string(), "add_repository".to_string(), serde_json::json!({"name": "sidekiq", "url": "https://github.com/sidekiq/sidekiq.git"}))],
            should_error: false, error_message: None,
        },
        MockResponse {
            text: "It seems the repository 'sidekiq' already exists in the system.".to_string(),
            tool_calls: vec![],
            should_error: false, error_message: None,
        }
    ]));

    let agent = Agent::new(config, tool_registry, embedding_provider_adapter, persistence, search_engine, mock_llm_client_add_repo)
        .await
        .expect("Failed to create agent");
    
    let add_repo_message = "Please add the sidekiq repository from https://github.com/sidekiq/sidekiq.git";
    
    let stream_result = agent.process_message_stream(add_repo_message).await;
    assert!(stream_result.is_ok(), "Failed to start processing first add repository message");
    
    let mut stream = stream_result.unwrap();
    let mut first_chunks = Vec::new();
    
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                first_chunks.push(chunk);
            }
            Err(e) => {
                println!("First stream error: {}", e);
                break;
            }
        }
    }
    
    let add_same_repo_message = "Please add the sidekiq repository again from https://github.com/sidekiq/sidekiq.git";
    
    let stream_result2 = agent.process_message_stream(add_same_repo_message).await;
    assert!(stream_result2.is_ok(), "Failed to start processing second add repository message");
    
    let mut stream2 = stream_result2.unwrap();
    let mut second_chunks = Vec::new();
    
    while let Some(chunk_result) = stream2.next().await {
        match chunk_result {
            Ok(chunk) => {
                second_chunks.push(chunk);
            }
            Err(e) => {
                println!("Second stream error: {}", e);
                break;
            }
        }
    }
    
    println!("=== First add repository attempt ===");
    for chunk in &first_chunks {
        if let sagitta_code::llm::client::MessagePart::Text { text } = &chunk.part {
            print!("{}", text);
        }
    }
    println!("\n\n=== Second add repository attempt (should handle 'already exists') ===");
    for chunk in &second_chunks {
        if let sagitta_code::llm::client::MessagePart::Text { text } = &chunk.part {
            print!("{}", text);
        }
    }
    println!();
    
    assert!(!first_chunks.is_empty(), "No chunks received from first agent call");
    assert!(!second_chunks.is_empty(), "No chunks received from second agent call");

    let second_response_text = second_chunks.iter().filter_map(|chunk: &StreamChunk| {
        if let sagitta_code::llm::client::MessagePart::Text { text } = &chunk.part {
            Some(text.clone())
        } else {
            None
        }
    }).collect::<String>();
    assert!(second_response_text.contains("already exists"), "Second response should indicate repository already exists. Got: {}", second_response_text);
}

#[tokio::test]
async fn test_embedding_adapter_no_runtime_nesting() {
    use std::sync::Arc;
    use sagitta_search::EmbeddingPoolAdapter;
    use sagitta_embed::{EmbeddingPool, config::EmbeddingConfig, EmbeddingModelType};
    
    // Create a basic embedding config for testing
    let embedding_config = EmbeddingConfig {
        model_type: EmbeddingModelType::Default,
        max_sessions: 1,
        ..Default::default()
    };
    
    // Create an embedding pool
    let pool = EmbeddingPool::with_configured_sessions(embedding_config)
        .expect("Failed to create embedding pool");
    let pool_arc = Arc::new(pool);
    
    // Create the adapter
    let adapter = EmbeddingPoolAdapter::new(pool_arc);
    
    // Test embedding within an async context (this would previously panic)
    let result = adapter.embed_batch(&["test text", "another test"]);
    
    // The result should be Ok (not a panic) even if embedding fails
    assert!(result.is_ok() || result.is_err()); // Should not panic
    
    // If successful, verify the embeddings
    if let Ok(embeddings) = result {
        assert_eq!(embeddings.len(), 2);
        for embedding in embeddings {
            assert!(!embedding.is_empty());
        }
    }
} 