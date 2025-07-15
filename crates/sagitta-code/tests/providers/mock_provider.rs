use std::sync::Arc;
use sagitta_code::{
    llm::client::{LlmClient, Message, ToolDefinition, LlmResponse, StreamChunk, MessagePart, Role, ThinkingConfig, GroundingConfig},
    utils::errors::SagittaCodeError,
};
use futures_util::Stream;
use std::pin::Pin;
use serde_json::Value;
use uuid::Uuid;

/// Mock response for testing provider behavior
#[derive(Debug, Clone)]
pub struct MockResponse {
    pub text: String,
    pub tool_calls: Vec<(String, String, Value)>,
    pub should_error: bool,
    pub error_message: Option<String>,
}

impl MockResponse {
    /// Create a simple text response
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tool_calls: vec![],
            should_error: false,
            error_message: None,
        }
    }

    /// Create an error response
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            text: String::new(),
            tool_calls: vec![],
            should_error: true,
            error_message: Some(message.into()),
        }
    }

    /// Create a response with tool calls
    pub fn with_tools(text: impl Into<String>, tool_calls: Vec<(String, String, Value)>) -> Self {
        Self {
            text: text.into(),
            tool_calls,
            should_error: false,
            error_message: None,
        }
    }
}

/// Provider type enum for testing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MockProviderType {
    ClaudeCode,
    MistralRs,
    TestProvider,
}

impl MockProviderType {
    pub fn display_name(&self) -> &'static str {
        match self {
            MockProviderType::ClaudeCode => "Claude Code",
            MockProviderType::MistralRs => "Mistral.rs",
            MockProviderType::TestProvider => "Test Provider",
        }
    }
}

/// Provider configuration enum for testing
#[derive(Debug, Clone)]
pub enum MockProviderConfig {
    Valid { api_key: Option<String> },
    Invalid { error: String },
}

impl MockProviderConfig {
    pub fn mock_valid() -> Self {
        Self::Valid { api_key: Some("test-key".to_string()) }
    }

    pub fn mock_invalid() -> Self {
        Self::Invalid { error: "Invalid configuration".to_string() }
    }
}

/// Mock provider trait for testing
pub trait MockProvider: Send + Sync + std::fmt::Debug {
    fn provider_type(&self) -> MockProviderType;
    fn display_name(&self) -> &str;
    fn create_client(&self, config: &MockProviderConfig, mcp_integration: Option<Arc<MockMcpIntegration>>) -> Result<Box<dyn LlmClient>, SagittaCodeError>;
    fn validate_config(&self, config: &MockProviderConfig) -> Result<(), SagittaCodeError>;
    fn default_config(&self) -> MockProviderConfig;
    fn supported_features(&self) -> Vec<String>;
    fn requires_api_key(&self) -> bool;
    fn is_available(&self) -> bool;
}

/// Mock LLM client implementation for testing
#[derive(Debug, Clone)]
pub struct MockLlmClient {
    pub responses: Arc<tokio::sync::Mutex<Vec<MockResponse>>>,
    current_index: Arc<tokio::sync::Mutex<usize>>,
    calls: Arc<tokio::sync::Mutex<Vec<(Vec<Message>, Vec<ToolDefinition>)>>>,
    provider_type: MockProviderType,
}

impl MockLlmClient {
    pub fn new(responses: Vec<MockResponse>, provider_type: MockProviderType) -> Self {
        Self {
            responses: Arc::new(tokio::sync::Mutex::new(responses)),
            current_index: Arc::new(tokio::sync::Mutex::new(0)),
            calls: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            provider_type,
        }
    }

    pub fn with_responses(responses: Arc<tokio::sync::Mutex<Vec<MockResponse>>>, provider_type: MockProviderType) -> Self {
        Self {
            responses,
            current_index: Arc::new(tokio::sync::Mutex::new(0)),
            calls: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            provider_type,
        }
    }

    /// Get the recorded calls for verification
    pub async fn get_calls(&self) -> Vec<(Vec<Message>, Vec<ToolDefinition>)> {
        self.calls.lock().await.clone()
    }

    /// Reset the client for reuse
    pub async fn reset(&self) {
        *self.current_index.lock().await = 0;
        self.calls.lock().await.clear();
    }

    async fn get_next_response(&self) -> Result<MockResponse, SagittaCodeError> {
        let mut index_guard = self.current_index.lock().await;
        let responses_guard = self.responses.lock().await;
        
        if *index_guard >= responses_guard.len() {
            return Ok(MockResponse::text("Default mock response"));
        }
        
        let response = responses_guard[*index_guard].clone();
        *index_guard += 1;
        Ok(response)
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmClient {
    fn client_type(&self) -> &str {
        match self.provider_type {
            MockProviderType::ClaudeCode => "claude-code",
            MockProviderType::MistralRs => "mistral-rs",
            MockProviderType::TestProvider => "test",
        }
    }
    
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
            tool_calls: mock_response.tool_calls.clone(),
            usage: None,
            grounding: None,
        })
    }

    async fn generate_with_thinking(
        &self, messages: &[Message], tools: &[ToolDefinition], _thinking_config: &ThinkingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        self.generate(messages, tools).await
    }

    async fn generate_with_grounding(
        &self, messages: &[Message], tools: &[ToolDefinition], _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        self.generate(messages, tools).await
    }

    async fn generate_with_thinking_and_grounding(
        &self, messages: &[Message], tools: &[ToolDefinition], _thinking_config: &ThinkingConfig, _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        self.generate(messages, tools).await
    }

    async fn generate_stream(
        &self, messages: &[Message], tools: &[ToolDefinition],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        let llm_response = self.generate(messages, tools).await?;
        let mut stream_chunks: Vec<Result<StreamChunk, SagittaCodeError>> = Vec::new();
        
        for part in llm_response.message.parts {
            stream_chunks.push(Ok(StreamChunk {
                part: part.clone(),
                is_final: false,
                finish_reason: None,
                token_usage: None,
            }));
        }
        
        stream_chunks.push(Ok(StreamChunk {
            part: MessagePart::Text { text: String::new() },
            is_final: true,
            finish_reason: Some("stop".to_string()),
            token_usage: None,
        }));
        
        Ok(Box::pin(futures_util::stream::iter(stream_chunks)))
    }

    async fn generate_stream_with_thinking(
        &self, messages: &[Message], tools: &[ToolDefinition], _thinking_config: &ThinkingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_grounding(
        &self, messages: &[Message], tools: &[ToolDefinition], _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_thinking_and_grounding(
        &self, messages: &[Message], tools: &[ToolDefinition], _thinking_config: &ThinkingConfig, _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        self.generate_stream(messages, tools).await
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Mock provider implementation for testing
#[derive(Debug, Clone)]
pub struct TestProvider {
    provider_type: MockProviderType,
    display_name: String,
    responses: Arc<tokio::sync::Mutex<Vec<MockResponse>>>,
    should_fail_validation: bool,
    should_fail_creation: bool,
}

impl TestProvider {
    pub fn new(provider_type: MockProviderType, display_name: impl Into<String>) -> Self {
        Self {
            provider_type,
            display_name: display_name.into(),
            responses: Arc::new(tokio::sync::Mutex::new(vec![MockResponse::text("Test response")])),
            should_fail_validation: false,
            should_fail_creation: false,
        }
    }

    pub fn new_with_failures(provider_type: MockProviderType, display_name: impl Into<String>, fail_validation: bool, fail_creation: bool) -> Self {
        Self {
            provider_type,
            display_name: display_name.into(),
            responses: Arc::new(tokio::sync::Mutex::new(vec![MockResponse::text("Test response")])),
            should_fail_validation: fail_validation,
            should_fail_creation: fail_creation,
        }
    }

    pub fn with_responses(provider_type: MockProviderType, display_name: impl Into<String>, responses: Vec<MockResponse>) -> Self {
        Self {
            provider_type,
            display_name: display_name.into(),
            responses: Arc::new(tokio::sync::Mutex::new(responses)),
            should_fail_validation: false,
            should_fail_creation: false,
        }
    }
}

impl MockProvider for TestProvider {
    fn provider_type(&self) -> MockProviderType {
        self.provider_type
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn create_client(&self, config: &MockProviderConfig, _mcp: Option<Arc<MockMcpIntegration>>) -> Result<Box<dyn LlmClient>, SagittaCodeError> {
        if self.should_fail_creation {
            return Err(SagittaCodeError::LlmError("Mock creation failure".to_string()));
        }

        match config {
            MockProviderConfig::Valid { .. } => {
                Ok(Box::new(MockLlmClient::with_responses(self.responses.clone(), self.provider_type)))
            }
            MockProviderConfig::Invalid { error } => {
                Err(SagittaCodeError::ConfigError(error.clone()))
            }
        }
    }

    fn validate_config(&self, config: &MockProviderConfig) -> Result<(), SagittaCodeError> {
        if self.should_fail_validation {
            return Err(SagittaCodeError::ConfigError("Mock validation failure".to_string()));
        }

        match config {
            MockProviderConfig::Valid { .. } => Ok(()),
            MockProviderConfig::Invalid { error } => {
                Err(SagittaCodeError::ConfigError(error.clone()))
            }
        }
    }

    fn default_config(&self) -> MockProviderConfig {
        MockProviderConfig::mock_valid()
    }

    fn supported_features(&self) -> Vec<String> {
        vec![
            "text_generation".to_string(),
            "tool_calling".to_string(),
            "streaming".to_string(),
        ]
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    fn is_available(&self) -> bool {
        true
    }
}

/// Mock MCP integration for testing
#[derive(Debug)]
pub struct MockMcpIntegration {
    pub tools: Vec<ToolDefinition>,
    pub call_results: Arc<tokio::sync::Mutex<Vec<(String, String, Value, Value)>>>,
}

impl MockMcpIntegration {
    pub fn new() -> Self {
        Self {
            tools: vec![],
            call_results: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn with_tools(tools: Vec<ToolDefinition>) -> Self {
        Self {
            tools,
            call_results: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    pub async fn get_tool_calls(&self) -> Vec<(String, String, Value, Value)> {
        self.call_results.lock().await.clone()
    }
}

impl Default for MockMcpIntegration {
    fn default() -> Self {
        Self::new()
    }
}