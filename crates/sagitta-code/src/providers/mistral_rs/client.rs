use std::sync::Arc;
use std::pin::Pin;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use futures_util::stream::StreamExt as FuturesStreamExt;
use futures_util::Stream;

use crate::llm::client::{LlmClient, Message, ToolDefinition, LlmResponse, StreamChunk, MessagePart};
use crate::utils::errors::SagittaCodeError;
use crate::providers::claude_code::mcp_integration::McpIntegration;
use super::{MistralRsConfig, MistralRsStream};

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_p: Option<f32>,
    stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAITool>,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunction,
}

#[derive(Debug, Serialize)]
struct OpenAIFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIResponseChoice>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponseChoice {
    message: OpenAIResponseMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponseMessage {
    #[serde(default)]
    content: Option<String>,
    role: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
    #[serde(default)]
    reasoning_tokens: Option<u32>,
    #[serde(default)]
    thinking_tokens: Option<u32>,
}

pub struct MistralRsClient {
    config: MistralRsConfig,
    client: Client,
    mcp_integration: Arc<McpIntegration>,
}

impl MistralRsClient {
    pub fn new(config: MistralRsConfig, mcp_integration: Arc<McpIntegration>) -> Result<Self, SagittaCodeError> {
        config.validate()
            .map_err(|e| SagittaCodeError::ConfigError(format!("Invalid Mistral.rs config: {}", e)))?;

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5 minute timeout
            .build()
            .map_err(|e| SagittaCodeError::LlmError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client,
            mcp_integration,
        })
    }

    /// Configure Mistral.rs to use MCP integration 
    /// NOTE: This logs the MCP setup for now. In production, this would create MCP config files
    /// that Mistral.rs can read to discover tools from the same MCP server Claude Code uses
    pub async fn configure_mcp_for_mistral_rs(mcp_integration: Arc<McpIntegration>) -> Result<(), SagittaCodeError> {
        log::info!("MCP integration available for Mistral.rs - tools will be available via shared MCP server");
        log::info!("NOTE: Mistral.rs server must be configured to use MCP config for tools to work");
        log::info!("MCP Integration: {:p}", mcp_integration.as_ref());
        
        // TODO: In a production implementation, we would need to:
        // 1. Start the MCP integration and get the config file path
        // 2. Write MCP config to a file that Mistral.rs can read 
        // 3. Configure Mistral.rs to use that MCP config file
        // 4. This would allow Mistral.rs to discover tools from the same internal MCP server
        // For now, we just log that MCP integration is available
        
        Ok(())
    }

    fn convert_messages(&self, messages: &[Message]) -> Vec<OpenAIMessage> {
        messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    crate::llm::client::Role::User => "user".to_string(),
                    crate::llm::client::Role::Assistant => "assistant".to_string(),
                    crate::llm::client::Role::System => "system".to_string(),
                    crate::llm::client::Role::Function => "function".to_string(),
                };
                
                // Extract text content from message parts
                let content = msg.parts
                    .iter()
                    .filter_map(|part| match part {
                        MessagePart::Text { text } => Some(text.clone()),
                        _ => None, // Skip non-text parts for now
                    })
                    .collect::<Vec<String>>()
                    .join(" ");
                    
                OpenAIMessage { role, content }
            })
            .collect()
    }

    // NOTE: convert_tools method removed - tools are now provided via MCP integration
    // Mistral.rs discovers tools through MCP server configuration, not via HTTP API
    
    #[cfg(test)]
    fn convert_tools(&self, tools: &[ToolDefinition]) -> Vec<OpenAITool> {
        // Helper method for tests only - tools are not actually used via HTTP API
        tools
            .iter()
            .map(|tool| OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunction {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.parameters.clone(),
                },
            })
            .collect()
    }

    async fn make_request(&self, request: OpenAIRequest) -> Result<reqwest::Response, SagittaCodeError> {
        let url = format!("{}/v1/chat/completions", self.config.base_url);
        
        let mut req_builder = self.client.post(&url)
            .json(&request);

        // Add API key if provided
        if let Some(ref api_key) = self.config.api_key {
            req_builder = req_builder.bearer_auth(api_key);
        }

        let response = req_builder
            .send()
            .await
            .map_err(|e| SagittaCodeError::LlmError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SagittaCodeError::LlmError(format!(
                "HTTP error {}: {}",
                status, error_text
            )));
        }

        Ok(response)
    }
}

#[async_trait]
impl LlmClient for MistralRsClient {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn client_type(&self) -> &str {
        "mistral_rs"
    }

    async fn generate(
        &self, 
        messages: &[Message], 
        _tools: &[ToolDefinition] // Tools provided via MCP, not HTTP API
    ) -> Result<LlmResponse, SagittaCodeError> {
        // NOTE: Tools are provided via MCP integration, not through the OpenAI HTTP API
        // Mistral.rs discovers tools through MCP server configuration, not via chat completions
        
        let openai_request = OpenAIRequest {
            model: self.config.model.clone(),
            messages: self.convert_messages(messages),
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            top_k: self.config.top_k,
            min_p: self.config.min_p,
            stream: false,
            tools: vec![], // Tools handled via MCP, not HTTP API
        };

        let response = self.make_request(openai_request).await?;
        let openai_response: OpenAIResponse = response
            .json()
            .await
            .map_err(|e| SagittaCodeError::LlmError(format!("Failed to parse response: {}", e)))?;

        if openai_response.choices.is_empty() {
            return Err(SagittaCodeError::LlmError(
                "No choices in response".to_string(),
            ));
        }

        let choice = &openai_response.choices[0];
        let content = choice.message.content.as_deref().unwrap_or("").to_string();

        // Create the response message
        let message = Message {
            id: uuid::Uuid::new_v4(),
            role: crate::llm::client::Role::Assistant,
            parts: vec![crate::llm::client::MessagePart::Text { text: content }],
            metadata: std::collections::HashMap::new(),
        };

        Ok(LlmResponse {
            message,
            tool_calls: vec![], // TODO: Parse tool calls from response
            usage: openai_response.usage.map(|u| crate::llm::client::TokenUsage {
                prompt_tokens: u.prompt_tokens as i32,
                completion_tokens: u.completion_tokens as i32,
                thinking_tokens: u.thinking_tokens.or(u.reasoning_tokens).map(|t| t as i32),
                model_name: self.config.model.clone(),
                cached_tokens: Some(0),
                total_tokens: u.total_tokens as i32,
            }),
            grounding: None,
        })
    }

    async fn generate_stream(&self, 
        messages: &[Message], 
        _tools: &[ToolDefinition] // Tools provided via MCP, not HTTP API
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        let openai_request = OpenAIRequest {
            model: self.config.model.clone(),
            messages: self.convert_messages(messages),
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            top_k: self.config.top_k,
            min_p: self.config.min_p,
            stream: true,
            tools: vec![], // Tools handled via MCP, not HTTP API
        };

        let response = self.make_request(openai_request).await?;
        
        // Convert the response body stream to bytes stream
        let bytes_stream = response.bytes_stream();
        let stream = Box::pin(bytes_stream);
        
        let mistral_stream = MistralRsStream::new(stream);
        Ok(Box::pin(mistral_stream))
    }

    async fn generate_with_thinking(
        &self, 
        messages: &[Message], 
        tools: &[ToolDefinition], // Tools provided via MCP, not HTTP API
        _thinking_config: &crate::llm::client::ThinkingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // For now, ignore thinking config and use regular generation
        self.generate(messages, tools).await
    }

    async fn generate_with_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition], // Tools provided via MCP, not HTTP API
        _grounding_config: &crate::llm::client::GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // For now, ignore grounding config and use regular generation
        self.generate(messages, tools).await
    }

    async fn generate_with_thinking_and_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &crate::llm::client::ThinkingConfig,
        _grounding_config: &crate::llm::client::GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // For now, ignore both configs and use regular generation
        self.generate(messages, tools).await
    }

    async fn generate_stream_with_thinking(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &crate::llm::client::ThinkingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        // For now, ignore thinking config and use regular streaming
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _grounding_config: &crate::llm::client::GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        // For now, ignore grounding config and use regular streaming
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_thinking_and_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &crate::llm::client::ThinkingConfig,
        _grounding_config: &crate::llm::client::GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        // For now, ignore both configs and use regular streaming
        self.generate_stream(messages, tools).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::client::{Message, Role, MessagePart};
    use std::sync::Arc;
    use crate::providers::claude_code::mcp_integration::McpIntegration;
    
    // For testing, we'll use a real McpIntegration instance

    fn create_test_config() -> MistralRsConfig {
        MistralRsConfig::new("http://localhost:1234".to_string())
            .with_model("test-model".to_string())
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = create_test_config();
        let mcp = Arc::new(McpIntegration::new());
        
        let result = MistralRsClient::new(config, mcp);
        assert!(result.is_ok());
    }

    #[test]
    fn test_message_conversion() {
        let config = create_test_config();
        let mcp = Arc::new(McpIntegration::new());
        let client = MistralRsClient::new(config, mcp).unwrap();

        let messages = vec![
            Message {
                id: uuid::Uuid::new_v4(),
                role: Role::System,
                parts: vec![MessagePart::Text { text: "System prompt".to_string() }],
                metadata: std::collections::HashMap::new(),
            },
            Message {
                id: uuid::Uuid::new_v4(),
                role: Role::User,
                parts: vec![MessagePart::Text { text: "User message".to_string() }],
                metadata: std::collections::HashMap::new(),
            },
            Message {
                id: uuid::Uuid::new_v4(),
                role: Role::Assistant,
                parts: vec![MessagePart::Text { text: "Assistant response".to_string() }],
                metadata: std::collections::HashMap::new(),
            },
        ];

        let openai_messages = client.convert_messages(&messages);
        
        assert_eq!(openai_messages.len(), 3);
        assert_eq!(openai_messages[0].role, "system");
        assert_eq!(openai_messages[0].content, "System prompt");
        assert_eq!(openai_messages[1].role, "user");
        assert_eq!(openai_messages[1].content, "User message");
        assert_eq!(openai_messages[2].role, "assistant");
        assert_eq!(openai_messages[2].content, "Assistant response");
    }

    #[test]
    fn test_tool_conversion() {
        let config = create_test_config();
        let mcp = Arc::new(McpIntegration::new());
        let client = MistralRsClient::new(config, mcp).unwrap();

        let tools = vec![
            ToolDefinition {
                name: "test_tool".to_string(),
                description: "A test tool".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "param": {"type": "string"}
                    }
                }),
                is_required: false,
            },
        ];

        let openai_tools = client.convert_tools(&tools);
        
        assert_eq!(openai_tools.len(), 1);
        assert_eq!(openai_tools[0].tool_type, "function");
        assert_eq!(openai_tools[0].function.name, "test_tool");
        assert_eq!(openai_tools[0].function.description, "A test tool");
    }

    #[test]
    fn test_invalid_config() {
        let invalid_config = MistralRsConfig {
            base_url: "invalid_url".to_string(),
            ..Default::default()
        };
        let mcp = Arc::new(McpIntegration::new());
        
        let result = MistralRsClient::new(invalid_config, mcp);
        assert!(result.is_err());
    }
}