use std::sync::Arc;
use async_trait::async_trait;
use std::pin::Pin;
use futures_util::Stream;

use crate::config::types::{SagittaCodeConfig, OpenRouterConfig};
use crate::llm::client::{LlmClient, Message, ToolDefinition, LlmResponse, StreamChunk, ThinkingConfig, GroundingConfig};
use crate::utils::errors::SagittaCodeError;
use super::error::OpenRouterError;
use super::api::*;

/// OpenRouter client implementing the LlmClient trait
pub struct OpenRouterClient {
    config: OpenRouterConfig,
    http_client: reqwest::Client,
    base_url: String,
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

        Ok(Self {
            config: config.openrouter.clone(),
            http_client,
            base_url: "https://openrouter.ai/api/v1".to_string(),
        })
    }

    /// Get available models from OpenRouter
    pub async fn get_models(&self) -> Result<Vec<ModelInfo>, OpenRouterError> {
        let url = format!("{}/models", self.base_url);
        let response = self.http_client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(OpenRouterError::HttpError(
                format!("HTTP {}: {}", response.status(), response.text().await.unwrap_or_default())
            ));
        }

        let models_response: ModelsResponse = response.json().await?;
        Ok(models_response.data)
    }
}

#[async_trait]
impl LlmClient for OpenRouterClient {
    async fn generate(&self, 
        _messages: &[Message], 
        _tools: &[ToolDefinition]
    ) -> Result<LlmResponse, SagittaCodeError> {
        // TODO: Implement this properly in Phase 2
        Err(SagittaCodeError::LlmError("OpenRouter client not yet fully implemented".to_string()))
    }

    async fn generate_with_thinking(&self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // TODO: Implement this properly in Phase 2
        Err(SagittaCodeError::LlmError("OpenRouter thinking not yet implemented".to_string()))
    }

    async fn generate_with_grounding(&self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // TODO: Implement this properly in Phase 2
        Err(SagittaCodeError::LlmError("OpenRouter grounding not yet implemented".to_string()))
    }

    async fn generate_with_thinking_and_grounding(&self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
        _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // TODO: Implement this properly in Phase 2
        Err(SagittaCodeError::LlmError("OpenRouter thinking+grounding not yet implemented".to_string()))
    }

    async fn generate_stream(&self, 
        _messages: &[Message], 
        _tools: &[ToolDefinition]
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        // TODO: Implement this properly in Phase 2
        Err(SagittaCodeError::LlmError("OpenRouter streaming not yet implemented".to_string()))
    }

    async fn generate_stream_with_thinking(&self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        // TODO: Implement this properly in Phase 2
        Err(SagittaCodeError::LlmError("OpenRouter streaming+thinking not yet implemented".to_string()))
    }

    async fn generate_stream_with_grounding(&self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        // TODO: Implement this properly in Phase 2
        Err(SagittaCodeError::LlmError("OpenRouter streaming+grounding not yet implemented".to_string()))
    }

    async fn generate_stream_with_thinking_and_grounding(&self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
        _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        // TODO: Implement this properly in Phase 2
        Err(SagittaCodeError::LlmError("OpenRouter streaming+thinking+grounding not yet implemented".to_string()))
    }
} 