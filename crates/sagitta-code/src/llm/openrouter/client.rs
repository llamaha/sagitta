use std::sync::Arc;
use async_trait::async_trait;
use std::pin::Pin;
use futures_util::Stream;
use uuid::Uuid;

use crate::config::types::{SagittaCodeConfig, OpenRouterConfig};
use crate::llm::client::{LlmClient, Message, MessagePart, Role, ToolDefinition, LlmResponse, StreamChunk, ThinkingConfig, GroundingConfig, TokenUsage};
use crate::utils::errors::SagittaCodeError;
use super::error::OpenRouterError;
use super::api::*;
use super::streaming::OpenRouterStream;
use super::models::{ModelManager, ModelFilter, ModelCategory};

/// OpenRouter client implementing the LlmClient trait
pub struct OpenRouterClient {
    config: OpenRouterConfig,
    http_client: reqwest::Client,
    base_url: String,
    model_manager: ModelManager,
}

impl OpenRouterClient {
    /// Create a new OpenRouter client
    pub fn new(config: &SagittaCodeConfig) -> Result<Self, OpenRouterError> {
        // Try config first, then environment
        let api_key = config.openrouter.api_key.clone()
            .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
            .ok_or_else(|| OpenRouterError::AuthenticationError(
                "OpenRouter API key not found in config or OPENROUTER_API_KEY environment variable".to_string()
            ))?;

        if api_key.is_empty() {
            return Err(OpenRouterError::AuthenticationError(
                "OpenRouter API key cannot be empty".to_string()
            ));
        }

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", api_key))
                .map_err(|e| OpenRouterError::ConfigError(format!("Invalid API key format: {}", e)))?
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json")
        );
        headers.insert(
            "HTTP-Referer",
            reqwest::header::HeaderValue::from_static("https://github.com/user/sagitta")
        );
        headers.insert(
            "X-Title",
            reqwest::header::HeaderValue::from_static("Sagitta Code AI")
        );

        let http_client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(config.openrouter.request_timeout))
            .build()
            .map_err(|e| OpenRouterError::ConfigError(format!("Failed to create HTTP client: {}", e)))?;

        let base_url = "https://openrouter.ai/api/v1".to_string();
        let model_manager = ModelManager::new(http_client.clone(), base_url.clone());

        Ok(Self {
            config: config.openrouter.clone(),
            http_client,
            base_url,
            model_manager,
        })
    }

    /// Get available models from OpenRouter
    pub async fn get_models(&self) -> Result<Vec<ModelInfo>, OpenRouterError> {
        self.model_manager.get_available_models(None).await
    }

    /// Get popular models
    pub async fn get_popular_models(&self) -> Result<Vec<ModelInfo>, OpenRouterError> {
        self.model_manager.get_popular_models().await
    }

    /// Search models by query
    pub async fn search_models(&self, query: &str) -> Result<Vec<ModelInfo>, OpenRouterError> {
        self.model_manager.search_models(query).await
    }

    /// Get models filtered by criteria
    pub async fn get_filtered_models(&self, filter: ModelFilter) -> Result<Vec<ModelInfo>, OpenRouterError> {
        self.model_manager.get_available_models(Some(filter)).await
    }

    /// Get model by ID
    pub async fn get_model_by_id(&self, model_id: &str) -> Result<Option<ModelInfo>, OpenRouterError> {
        self.model_manager.get_model_by_id(model_id).await
    }

    /// Get available providers
    pub async fn get_providers(&self) -> Result<Vec<String>, OpenRouterError> {
        self.model_manager.get_providers().await
    }

    /// Refresh model cache
    pub async fn refresh_model_cache(&self) -> Result<(), OpenRouterError> {
        self.model_manager.refresh_cache().await
    }

    /// Convert our Message format to OpenRouter's ChatMessage format
    fn convert_messages(&self, messages: &[Message]) -> Vec<ChatMessage> {
        messages.iter().map(|msg| {
            // Combine all text parts into a single content string
            let content = msg.parts.iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text } => Some(text.clone()),
                    MessagePart::Thought { text } => Some(format!("<thinking>{}</thinking>", text)),
                    _ => None, // Skip tool calls and results for now
                })
                .collect::<Vec<_>>()
                .join("\n");

            ChatMessage {
                role: match msg.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => "system".to_string(),
                    Role::Function => "assistant".to_string(), // Map function to assistant
                },
                content,
            }
        }).collect()
    }

    /// Convert OpenRouter response to our LlmResponse format
    fn convert_response(&self, response: ChatCompletionResponse) -> LlmResponse {
        let choice = response.choices.into_iter().next()
            .unwrap_or_else(|| Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: "No response generated".to_string(),
                },
                finish_reason: Some("error".to_string()),
            });

        let message = Message {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            parts: vec![MessagePart::Text { 
                text: choice.message.content 
            }],
            metadata: Default::default(),
        };

        let usage = response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens as i32,
            completion_tokens: u.completion_tokens as i32,
            total_tokens: u.total_tokens as i32,
            thinking_tokens: None,
            model_name: response.model,
            cached_tokens: None,
        });

        LlmResponse {
            message,
            tool_calls: Vec::new(), // TODO: Implement tool calling
            usage,
            grounding: None, // TODO: Implement grounding
        }
    }
}

#[async_trait]
impl LlmClient for OpenRouterClient {
    async fn generate(&self, 
        messages: &[Message], 
        _tools: &[ToolDefinition]
    ) -> Result<LlmResponse, SagittaCodeError> {
        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: self.convert_messages(messages),
            stream: Some(false),
            max_tokens: None, // Use model defaults
            temperature: None,
            top_p: None,
            provider: self.config.provider_preferences.as_ref().map(|p| ProviderPreferences {
                order: p.order.clone(),
                allow_fallbacks: p.allow_fallbacks,
                sort: p.sort.clone(),
                data_collection: p.data_collection.clone(),
            }),
        };

        let url = format!("{}/chat/completions", self.base_url);
        let response = self.http_client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| SagittaCodeError::LlmError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(SagittaCodeError::LlmError(
                format!("HTTP {}: {}", status, error_text)
            ));
        }

        let completion: ChatCompletionResponse = response.json().await
            .map_err(|e| SagittaCodeError::LlmError(format!("Failed to parse response: {}", e)))?;

        Ok(self.convert_response(completion))
    }

    async fn generate_with_thinking(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // For now, just call the basic generate method
        // TODO: Implement thinking-specific logic when OpenRouter supports it
        self.generate(messages, tools).await
    }

    async fn generate_with_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // For now, just call the basic generate method
        // TODO: Implement grounding-specific logic (web search) when OpenRouter supports it
        self.generate(messages, tools).await
    }

    async fn generate_with_thinking_and_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
        _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // For now, just call the basic generate method
        // TODO: Implement both thinking and grounding when OpenRouter supports it
        self.generate(messages, tools).await
    }

    async fn generate_stream(&self, 
        messages: &[Message], 
        _tools: &[ToolDefinition]
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: self.convert_messages(messages),
            stream: Some(true), // Enable streaming
            max_tokens: None,
            temperature: None,
            top_p: None,
            provider: self.config.provider_preferences.as_ref().map(|p| ProviderPreferences {
                order: p.order.clone(),
                allow_fallbacks: p.allow_fallbacks,
                sort: p.sort.clone(),
                data_collection: p.data_collection.clone(),
            }),
        };

        let url = format!("{}/chat/completions", self.base_url);
        let response = self.http_client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| SagittaCodeError::LlmError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(SagittaCodeError::LlmError(
                format!("HTTP {}: {}", status, error_text)
            ));
        }

        let stream = OpenRouterStream::new(response);
        Ok(Box::pin(stream))
    }

    async fn generate_stream_with_thinking(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        // For now, just call the basic streaming method
        // TODO: Implement thinking-specific logic when OpenRouter supports it
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        // For now, just call the basic streaming method
        // TODO: Implement grounding-specific logic when OpenRouter supports it
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_thinking_and_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
        _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        // For now, just call the basic streaming method
        // TODO: Implement both thinking and grounding when OpenRouter supports it
        self.generate_stream(messages, tools).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{SagittaCodeConfig, OpenRouterConfig, ProviderPreferences};

    fn create_test_config() -> SagittaCodeConfig {
        SagittaCodeConfig {
            openrouter: OpenRouterConfig {
                api_key: Some("test-api-key".to_string()),
                model: "openai/gpt-4o".to_string(),
                provider_preferences: None,
                max_history_size: 50,
                max_reasoning_steps: 10,
                request_timeout: 30,
            },
            // ... other fields would be filled with defaults in a real implementation
            ..Default::default()
        }
    }

    #[test]
    fn test_openrouter_client_creation() {
        let config = create_test_config();
        let client = OpenRouterClient::new(&config);
        assert!(client.is_ok(), "OpenRouter client should be created successfully");
    }

    #[test]
    fn test_openrouter_client_creation_with_env_api_key() {
        // Use a unique environment variable name to avoid conflicts with other tests
        let env_var_name = "OPENROUTER_API_KEY_TEST_345";
        
        // Clear any existing environment variable first
        std::env::remove_var(env_var_name);
        std::env::remove_var("OPENROUTER_API_KEY");
        
        // Test with API key from environment
        std::env::set_var(env_var_name, "env-test-key");
        
        // Modify the client creation to check our test variable first
        let mut config = create_test_config();
        config.openrouter.api_key = None; // No API key in config
        
        // Temporarily override the API key lookup for this test
        std::env::set_var("OPENROUTER_API_KEY", std::env::var(env_var_name).unwrap());
        
        let client = OpenRouterClient::new(&config);
        
        // Clean up environment variables immediately after creation attempt
        std::env::remove_var(env_var_name);
        std::env::remove_var("OPENROUTER_API_KEY");
        
        match client {
            Ok(_) => {
                // Success case - environment API key was used
            }
            Err(e) => {
                panic!("OpenRouter client should use environment API key, but got error: {:?}", e);
            }
        }
    }

    #[test]
    fn test_openrouter_client_creation_missing_api_key() {
        let mut config = create_test_config();
        config.openrouter.api_key = None;
        
        // Ensure no environment variable
        std::env::remove_var("OPENROUTER_API_KEY");
        
        let client = OpenRouterClient::new(&config);
        assert!(client.is_err(), "OpenRouter client creation should fail without API key");
        
        if let Err(e) = client {
            match e {
                OpenRouterError::AuthenticationError(_) => {
                    // Expected error type
                }
                _ => panic!("Expected AuthenticationError, got {:?}", e),
            }
        }
    }

    #[test]
    fn test_convert_messages() {
        let config = create_test_config();
        let client = OpenRouterClient::new(&config).unwrap();
        
        let messages = vec![
            Message {
                id: uuid::Uuid::new_v4(),
                role: Role::System,
                parts: vec![MessagePart::Text { text: "You are a helpful assistant.".to_string() }],
                metadata: Default::default(),
            },
            Message {
                id: uuid::Uuid::new_v4(),
                role: Role::User,
                parts: vec![MessagePart::Text { text: "Hello, how are you?".to_string() }],
                metadata: Default::default(),
            },
        ];
        
        let converted = client.convert_messages(&messages);
        
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "system");
        assert_eq!(converted[0].content, "You are a helpful assistant.");
        assert_eq!(converted[1].role, "user");
        assert_eq!(converted[1].content, "Hello, how are you?");
    }

    #[test]
    fn test_convert_messages_with_multiple_parts() {
        let config = create_test_config();
        let client = OpenRouterClient::new(&config).unwrap();
        
        let messages = vec![
            Message {
                id: uuid::Uuid::new_v4(),
                role: Role::User,
                parts: vec![
                    MessagePart::Text { text: "First part".to_string() },
                    MessagePart::Text { text: "Second part".to_string() },
                    MessagePart::Thought { text: "Thinking about this".to_string() },
                ],
                metadata: Default::default(),
            },
        ];
        
        let converted = client.convert_messages(&messages);
        
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].role, "user");
        assert!(converted[0].content.contains("First part"));
        assert!(converted[0].content.contains("Second part"));
        assert!(converted[0].content.contains("<thinking>Thinking about this</thinking>"));
    }

    #[test]
    fn test_convert_response() {
        let config = create_test_config();
        let client = OpenRouterClient::new(&config).unwrap();
        
        let openrouter_response = ChatCompletionResponse {
            id: "test-id".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "openai/gpt-4o".to_string(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: "Hello! I'm doing well, thank you for asking.".to_string(),
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(Usage {
                prompt_tokens: 20,
                completion_tokens: 15,
                total_tokens: 35,
            }),
        };
        
        let converted = client.convert_response(openrouter_response);
        
        assert_eq!(converted.message.role, Role::Assistant);
        assert_eq!(converted.message.parts.len(), 1);
        
        if let MessagePart::Text { text } = &converted.message.parts[0] {
            assert_eq!(text, "Hello! I'm doing well, thank you for asking.");
        } else {
            panic!("Expected text message part");
        }
        
        assert!(converted.usage.is_some());
        let usage = converted.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 20);
        assert_eq!(usage.completion_tokens, 15);
        assert_eq!(usage.total_tokens, 35);
        assert_eq!(usage.model_name, "openai/gpt-4o");
    }
} 