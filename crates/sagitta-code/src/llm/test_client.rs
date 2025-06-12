use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use futures_util::{stream, Stream};
use std::pin::Pin;
use uuid::Uuid;

use crate::llm::client::{
    LlmClient, Message, MessagePart, Role, ToolDefinition, LlmResponse, 
    StreamChunk, ThinkingConfig, GroundingConfig
};
use crate::utils::errors::SagittaCodeError;

/// A simple test LLM client that returns predefined responses
#[derive(Debug, Clone)]
pub struct TestLlmClient {
    responses: Arc<Mutex<Vec<String>>>,
    tool_calls: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl TestLlmClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(vec!["I understand.".to_string()])),
            tool_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub fn with_responses(responses: Vec<String>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
            tool_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub fn with_tool_calls(mut self, tool_calls: Vec<(String, serde_json::Value)>) -> Self {
        *self.tool_calls.lock().unwrap() = tool_calls;
        self
    }
}

impl Default for TestLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmClient for TestLlmClient {
    async fn generate(&self, _messages: &[Message], _tools: &[ToolDefinition]) -> Result<LlmResponse, SagittaCodeError> {
        let mut responses = self.responses.lock().unwrap();
        let response = responses.pop().unwrap_or_else(|| "I understand.".to_string());
        
        // Check if we should make a tool call
        let mut tool_calls_guard = self.tool_calls.lock().unwrap();
        let tool_calls = if !tool_calls_guard.is_empty() {
            let tool_call = tool_calls_guard.remove(0);
            vec![(
                Uuid::new_v4().to_string(),
                tool_call.0,
                tool_call.1,
            )]
        } else {
            vec![]
        };
        
        Ok(LlmResponse {
            message: Message {
                id: Uuid::new_v4(),
                role: Role::Assistant,
                parts: vec![MessagePart::Text { text: response }],
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

    async fn generate_stream(&self, _messages: &[Message], _tools: &[ToolDefinition]) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        let mut responses = self.responses.lock().unwrap();
        let response = responses.pop().unwrap_or_else(|| "I understand.".to_string());
        
        let chunks: Vec<Result<StreamChunk, SagittaCodeError>> = vec![
            Ok(StreamChunk {
                part: MessagePart::Text { text: response },
                is_final: false,
                finish_reason: None,
                token_usage: None,
            }),
            Ok(StreamChunk {
                part: MessagePart::Text { text: "".to_string() },
                is_final: true,
                finish_reason: Some("stop".to_string()),
                token_usage: None,
            }),
        ];
        
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