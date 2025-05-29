// Core Gemini client implementation will go here

use async_trait::async_trait;
use futures_util::Stream;
use reqwest::Client as ReqwestClient;
use serde_json::Value;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;
use log::{debug, trace, warn};

use crate::config::FredAgentConfig;
use crate::llm::client::{LlmClient, Message, ToolDefinition, LlmResponse, StreamChunk, MessagePart, Role, ThinkingConfig, TokenUsage, GroundingConfig, GroundingInfo, GroundingSource};
use crate::llm::gemini::api::{GeminiRequest, Content, Tool, FunctionDeclaration, GeminiResponse, GeminiError, GenerationConfig, ThinkingConfig as GeminiThinkingConfig, GoogleSearch, DynamicRetrieval};
use crate::llm::gemini::streaming::{GeminiStream, MergedTextStream};
use crate::llm::gemini::models::{GeminiModel, ModelParameters, DEFAULT_MODEL};
use crate::utils::errors::FredAgentError;

/// Gemini API client
#[derive(Debug)]
pub struct GeminiClient {
    /// HTTP client for API requests
    client: ReqwestClient,
    
    /// API key for Gemini
    api_key: String,
    
    /// Model to use
    model: GeminiModel,
    
    /// Parameters for generation
    parameters: ModelParameters,
}

impl GeminiClient {
    /// Create a new GeminiClient
    pub fn new(config: &FredAgentConfig) -> Result<Self, FredAgentError> {
        // Try to get the API key from the config first
        let api_key = match config.gemini.api_key.clone() {
            Some(key) if !key.is_empty() => key,
            _ => {
                // Try to get from environment variable as fallback
                std::env::var("GEMINI_API_KEY").map_err(|_| {
                    FredAgentError::ConfigError("Gemini API key is not set. Check your Gemini API key in settings.".to_string())
                })?
            }
        };
        
        let model_id_str = &config.gemini.model;
        let model = GeminiModel::from_id(model_id_str).unwrap_or_else(|| {
            // Check if it's a deprecated 1.x model and provide a 2.5 fallback
            if model_id_str.contains("gemini-1.") {
                log::warn!("Specified Gemini model '{}' is deprecated. Upgrading to gemini-2.5-flash-preview-05-20 for better performance and features.", model_id_str);
                GeminiModel::Flash25Preview
            } else {
                log::warn!("Specified Gemini model '{}' not recognized. Defaulting to gemini-2.5-flash-preview-05-20.", model_id_str);
                GeminiModel::Flash25Preview
            }
        });
        
        // Remove the old fallback logic since we now handle deprecated models above
        let model = model;

        Ok(Self {
            client: ReqwestClient::new(),
            api_key,
            model, // model is now correctly initialized
            parameters: model.default_parameters(),
        })
    }
    
    /// Set the model to use
    pub fn with_model(mut self, model: GeminiModel) -> Self {
        self.model = model;
        self.parameters = model.default_parameters();
        self
    }
    
    /// Set the parameters for generation
    pub fn with_parameters(mut self, parameters: ModelParameters) -> Self {
        self.parameters = parameters;
        self
    }
    
    /// Build the API URL for the specified endpoint
    fn build_url(&self, endpoint: &str) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:{}?key={}",
            self.model.model_id(),
            endpoint,
            self.api_key
        )
    }
    
    /// Build the API URL for streaming endpoints with SSE support
    fn build_streaming_url(&self, endpoint: &str) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:{}?alt=sse&key={}",
            self.model.model_id(),
            endpoint,
            self.api_key
        )
    }
    
    /// Convert client messages to Gemini contents
    fn convert_messages(&self, messages: &[Message]) -> Vec<Content> {
        messages.iter().map(|m| m.into()).collect()
    }
    
    /// Convert client tools to Gemini tools
    fn convert_tools(&self, tools: &[ToolDefinition]) -> Option<Vec<Tool>> {
        if tools.is_empty() {
            return None;
        }
        
        let function_declarations: Vec<FunctionDeclaration> = tools
            .iter()
            .map(|tool| tool.into())
            .collect();
        
        Some(vec![Tool {
            function_declarations: Some(function_declarations),
            google_search: None,
        }])
    }
    
    /// Create tools list with grounding support
    fn convert_tools_with_grounding(&self, tools: &[ToolDefinition], enable_grounding: bool) -> Option<Vec<Tool>> {
        let mut gemini_tools = Vec::new();
        
        // Add function tools if any
        if !tools.is_empty() {
            let function_declarations: Vec<FunctionDeclaration> = tools
                .iter()
                .map(|tool| tool.into())
                .collect();
            
            gemini_tools.push(Tool {
                function_declarations: Some(function_declarations),
                google_search: None,
            });
        }
        
        // Add grounding tool if enabled
        if enable_grounding {
            gemini_tools.push(Tool {
                function_declarations: None,
                google_search: Some(crate::llm::gemini::api::GoogleSearch {
                    dynamic_retrieval: None, // Use default behavior
                }),
            });
        }
        
        if gemini_tools.is_empty() {
            None
        } else {
            Some(gemini_tools)
        }
    }
    
    /// Build generation config from parameters
    fn build_generation_config(&self) -> GenerationConfig {
        GenerationConfig {
            temperature: Some(self.parameters.temperature),
            top_p: Some(self.parameters.top_p),
            top_k: Some(self.parameters.top_k),
            max_output_tokens: Some(self.parameters.max_output_tokens),
            stop_sequences: None,
            thinking_config: None,
        }
    }
    
    /// Build generation config with thinking configuration
    fn build_generation_config_with_thinking(&self, thinking_config: &ThinkingConfig) -> GenerationConfig {
        let gemini_thinking_config = if self.model.supports_thinking() {
            Some(GeminiThinkingConfig {
                include_thoughts: Some(thinking_config.include_thoughts),
                thinking_budget: thinking_config.thinking_budget,
            })
        } else {
            None
        };
        
        GenerationConfig {
            temperature: Some(self.parameters.temperature),
            top_p: Some(self.parameters.top_p),
            top_k: Some(self.parameters.top_k),
            max_output_tokens: Some(self.parameters.max_output_tokens),
            stop_sequences: None,
            thinking_config: gemini_thinking_config,
        }
    }
    
    /// Extract tool calls from a GeminiResponse
    fn extract_tool_calls(&self, response: &GeminiResponse) -> Vec<(String, String, Value)> {
        let mut tool_calls = Vec::new();
        log::trace!("GeminiClient::extract_tool_calls: Examining response: {:?}", response);
        
        if let Some(candidate) = response.candidates.first() {
            log::trace!("GeminiClient::extract_tool_calls: Found candidate: {:?}", candidate);
            for part in &candidate.content.parts {
                log::trace!("GeminiClient::extract_tool_calls: Examining part: {:?}", part);
                if let Some(function_call) = &part.function_call {
                    let tool_id = Uuid::new_v4().to_string();
                    log::trace!("GeminiClient::extract_tool_calls: Found functionCall: Name: {}, Args: {:?}, Generated ID: {}", function_call.name, function_call.args, tool_id);
                    tool_calls.push((
                        tool_id,
                        function_call.name.clone(),
                        function_call.args.clone(),
                    ));
                }
            }
        }
        log::trace!("GeminiClient::extract_tool_calls: Extracted tool_calls: {:?}", tool_calls);
        tool_calls
    }
    
    /// Convert a GeminiResponse to an LlmResponse
    fn convert_response(&self, response: GeminiResponse) -> Result<LlmResponse, FredAgentError> {
        if response.candidates.is_empty() {
            return Err(FredAgentError::LlmError(
                "Gemini API returned no candidates".to_string()
            ));
        }
        
        let candidate = &response.candidates[0];
        let tool_calls = self.extract_tool_calls(&response);
        
        // Convert the content to a message
        let parts = candidate.content.parts.iter()
            .map(|part| {
                if let Some(text) = &part.text {
                    // Check if this is a thought part
                    if part.thought == Some(true) {
                        MessagePart::Thought { text: text.clone() }
                    } else {
                        MessagePart::Text { text: text.clone() }
                    }
                } else if let Some(function_call) = &part.function_call {
                    MessagePart::ToolCall {
                        tool_call_id: Uuid::new_v4().to_string(),
                        name: function_call.name.clone(),
                        parameters: function_call.args.clone(),
                    }
                } else if let Some(function_response) = &part.function_response {
                    MessagePart::ToolResult {
                        tool_call_id: Uuid::new_v4().to_string(),
                        name: function_response.name.clone(),
                        result: function_response.response.clone(),
                    }
                } else {
                    // Empty part, use empty text
                    MessagePart::Text { text: String::new() }
                }
            })
            .collect();
        
        let message = Message {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            parts,
            metadata: Default::default(),
        };
        
        // Convert usage metadata if available
        let usage = response.usage_metadata.map(|usage_meta| TokenUsage {
            prompt_tokens: usage_meta.prompt_token_count.unwrap_or(0),
            completion_tokens: usage_meta.candidates_token_count.unwrap_or(0),
            total_tokens: usage_meta.total_token_count.unwrap_or(0),
            thinking_tokens: usage_meta.thoughts_token_count,
            model_name: self.model.model_id().to_string(),
            cached_tokens: usage_meta.cached_content_token_count,
        });
        
        // Extract grounding information if available
        let grounding = candidate.grounding_metadata.as_ref().and_then(|gm| {
            let search_queries = gm.web_search_queries.clone().unwrap_or_default();
            
            let sources: Vec<GroundingSource> = gm.grounding_chunks.as_ref()
                .map(|chunks| {
                    chunks.iter()
                        .filter_map(|chunk| {
                            if let Some(web) = &chunk.web {
                                Some(GroundingSource {
                                    uri: web.uri.clone().unwrap_or_default(),
                                    title: web.title.clone().unwrap_or_default(),
                                    confidence: 1.0, // Gemini doesn't provide per-chunk confidence directly
                                })
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
                
            if !search_queries.is_empty() || !sources.is_empty() {
                Some(GroundingInfo {
                    search_queries,
                    sources,
                })
            } else {
                None
            }
        });
        
        Ok(LlmResponse {
            message,
            tool_calls,
            usage,
            grounding,
        })
    }
}

#[async_trait]
impl LlmClient for GeminiClient {
    async fn generate(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse, FredAgentError> {
        let url = self.build_url("generateContent");
        
        let request = GeminiRequest {
            contents: self.convert_messages(messages),
            safety_settings: None,
            generation_config: Some(self.build_generation_config()),
            tools: self.convert_tools(tools),
        };
        
        match serde_json::to_string_pretty(&request) {
            Ok(json_request) => log::trace!("GeminiClient: Sending generate request: {}", json_request),
            Err(e) => log::warn!("GeminiClient: Failed to serialize request for logging: {}" , e),
        }
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| FredAgentError::NetworkError(format!("Failed to send request to Gemini API: {}", e)))?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Could not read error response".to_string());
            
            log::warn!("GeminiClient: Raw error response from API (HTTP {}): {}", status.as_u16(), error_text);

            // Try to parse as a GeminiError
            if let Ok(error) = serde_json::from_str::<GeminiError>(&error_text) {
                return Err(FredAgentError::LlmError(format!(
                    "Gemini API error: {} ({})",
                    error.error.message,
                    error.error.status
                )));
            }
            
            return Err(FredAgentError::LlmError(format!(
                "Gemini API returned an error: HTTP {}: {}",
                status.as_u16(),
                error_text
            )));
        }
        
        let response_text = response.text().await
            .map_err(|e| FredAgentError::LlmError(format!("Failed to read Gemini API success response text: {}", e)))?;
        log::trace!("GeminiClient: Raw success response from API: {}", response_text);
        
        let response_data = serde_json::from_str::<GeminiResponse>(&response_text)
            .map_err(|e| FredAgentError::LlmError(format!("Failed to parse Gemini API response: {}. Raw text: {}", e, response_text)))?;
        
        self.convert_response(response_data)
    }
    
    async fn generate_with_thinking(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
    ) -> Result<LlmResponse, FredAgentError> {
        let url = self.build_url("generateContent");
        
        let request = GeminiRequest {
            contents: self.convert_messages(messages),
            safety_settings: None,
            generation_config: Some(self.build_generation_config_with_thinking(thinking_config)),
            tools: self.convert_tools(tools),
        };
        
        match serde_json::to_string_pretty(&request) {
            Ok(json_request) => log::trace!("GeminiClient: Sending generate_with_thinking request: {}", json_request),
            Err(e) => log::warn!("GeminiClient: Failed to serialize request for logging: {}" , e),
        }
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| FredAgentError::NetworkError(format!("Failed to send request to Gemini API: {}", e)))?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Could not read error response".to_string());
            
            log::warn!("GeminiClient: Raw error response from API (thinking) (HTTP {}): {}", status.as_u16(), error_text);

            // Try to parse as a GeminiError
            if let Ok(error) = serde_json::from_str::<GeminiError>(&error_text) {
                return Err(FredAgentError::LlmError(format!(
                    "Gemini API error: {} ({})",
                    error.error.message,
                    error.error.status
                )));
            }
            
            return Err(FredAgentError::LlmError(format!(
                "Gemini API returned an error: HTTP {}: {}",
                status.as_u16(),
                error_text
            )));
        }
        
        let response_text = response.text().await
            .map_err(|e| FredAgentError::LlmError(format!("Failed to read Gemini API success response text: {}", e)))?;
        log::trace!("GeminiClient: Raw success response from API (thinking): {}", response_text);
        
        let response_data = serde_json::from_str::<GeminiResponse>(&response_text)
            .map_err(|e| FredAgentError::LlmError(format!("Failed to parse Gemini API response: {}. Raw text: {}", e, response_text)))?;
        
        self.convert_response(response_data)
    }
    
    async fn generate_with_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, FredAgentError> {
        let url = self.build_url("generateContent");
        
        let request = GeminiRequest {
            contents: self.convert_messages(messages),
            safety_settings: None,
            generation_config: Some(self.build_generation_config()),
            tools: self.convert_tools_with_grounding(tools, grounding_config.enable_web_search),
        };
        
        match serde_json::to_string_pretty(&request) {
            Ok(json_request) => log::trace!("GeminiClient: Sending generate_with_grounding request: {}", json_request),
            Err(e) => log::warn!("GeminiClient: Failed to serialize request for logging: {}" , e),
        }
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| FredAgentError::NetworkError(format!("Failed to send request to Gemini API: {}", e)))?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Could not read error response".to_string());
            
            log::warn!("GeminiClient: Raw error response from API (grounding) (HTTP {}): {}", status.as_u16(), error_text);

            // Try to parse as a GeminiError
            if let Ok(error) = serde_json::from_str::<GeminiError>(&error_text) {
                return Err(FredAgentError::LlmError(format!(
                    "Gemini API error: {} ({})",
                    error.error.message,
                    error.error.status
                )));
            }
            
            return Err(FredAgentError::LlmError(format!(
                "Gemini API returned an error: HTTP {}: {}",
                status.as_u16(),
                error_text
            )));
        }
        
        let response_text = response.text().await
            .map_err(|e| FredAgentError::LlmError(format!("Failed to read Gemini API success response text: {}", e)))?;
        log::trace!("GeminiClient: Raw success response from API (grounding): {}", response_text);
        
        let response_data = serde_json::from_str::<GeminiResponse>(&response_text)
            .map_err(|e| FredAgentError::LlmError(format!("Failed to parse Gemini API response: {}. Raw text: {}", e, response_text)))?;
        
        self.convert_response(response_data)
    }
    
    async fn generate_with_thinking_and_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
        grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, FredAgentError> {
        let url = self.build_url("generateContent");
        
        let request = GeminiRequest {
            contents: self.convert_messages(messages),
            safety_settings: None,
            generation_config: Some(self.build_generation_config_with_thinking(thinking_config)),
            tools: self.convert_tools_with_grounding(tools, grounding_config.enable_web_search),
        };
        
        match serde_json::to_string_pretty(&request) {
            Ok(json_request) => log::trace!("GeminiClient: Sending generate_with_thinking_and_grounding request: {}", json_request),
            Err(e) => log::warn!("GeminiClient: Failed to serialize request for logging: {}" , e),
        }
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| FredAgentError::NetworkError(format!("Failed to send request to Gemini API: {}", e)))?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Could not read error response".to_string());
            
            log::warn!("GeminiClient: Raw error response from API (thinking+grounding) (HTTP {}): {}", status.as_u16(), error_text);

            // Try to parse as a GeminiError
            if let Ok(error) = serde_json::from_str::<GeminiError>(&error_text) {
                return Err(FredAgentError::LlmError(format!(
                    "Gemini API error: {} ({})",
                    error.error.message,
                    error.error.status
                )));
            }
            
            return Err(FredAgentError::LlmError(format!(
                "Gemini API returned an error: HTTP {}: {}",
                status.as_u16(),
                error_text
            )));
        }
        
        let response_text = response.text().await
            .map_err(|e| FredAgentError::LlmError(format!("Failed to read Gemini API success response text: {}", e)))?;
        log::trace!("GeminiClient: Raw success response from API (thinking+grounding): {}", response_text);
        
        let response_data = serde_json::from_str::<GeminiResponse>(&response_text)
            .map_err(|e| FredAgentError::LlmError(format!("Failed to parse Gemini API response: {}. Raw text: {}", e, response_text)))?;
        
        self.convert_response(response_data)
    }
    
    async fn generate_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, FredAgentError>> + Send>>, FredAgentError> {
        let url = self.build_streaming_url("streamGenerateContent");
        
        let request = GeminiRequest {
            contents: self.convert_messages(messages),
            safety_settings: None,
            generation_config: Some(self.build_generation_config()),
            tools: self.convert_tools(tools),
        };
        
        match serde_json::to_string_pretty(&request) {
            Ok(json_request) => log::trace!("GeminiClient: Sending generate_stream request: {}", json_request),
            Err(e) => log::warn!("GeminiClient: Failed to serialize request for logging: {}" , e),
        }
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| FredAgentError::NetworkError(format!("Failed to send request to Gemini API: {}", e)))?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Could not read error response".to_string());

            log::warn!("GeminiClient: Raw error response from API (stream) (HTTP {}): {}", status.as_u16(), error_text);

            // Try to parse as a GeminiError
            if let Ok(error) = serde_json::from_str::<GeminiError>(&error_text) {
                return Err(FredAgentError::LlmError(format!(
                    "Gemini API error: {} ({})",
                    error.error.message,
                    error.error.status
                )));
            }
            
            return Err(FredAgentError::LlmError(format!(
                "Gemini API returned an error: HTTP {}: {}",
                status.as_u16(),
                error_text
            )));
        }
        
        let model_name_for_stream = self.model.model_id().to_string();
        Ok(Box::pin(GeminiStream::new(response, model_name_for_stream)))
    }
    
    async fn generate_stream_with_thinking(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, FredAgentError>> + Send>>, FredAgentError> {
        let url = self.build_streaming_url("streamGenerateContent");
        
        let request = GeminiRequest {
            contents: self.convert_messages(messages),
            safety_settings: None,
            generation_config: Some(self.build_generation_config_with_thinking(thinking_config)),
            tools: self.convert_tools(tools),
        };
        
        match serde_json::to_string_pretty(&request) {
            Ok(json_request) => log::trace!("GeminiClient: Sending generate_stream_with_thinking request: {}", json_request),
            Err(e) => log::warn!("GeminiClient: Failed to serialize request for logging: {}" , e),
        }
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| FredAgentError::NetworkError(format!("Failed to send request to Gemini API: {}", e)))?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Could not read error response".to_string());
            
            log::warn!("GeminiClient: Raw error response from API (stream/thinking) (HTTP {}): {}", status.as_u16(), error_text);

            // Try to parse as a GeminiError
            if let Ok(error) = serde_json::from_str::<GeminiError>(&error_text) {
                return Err(FredAgentError::LlmError(format!(
                    "Gemini API error: {} ({})",
                    error.error.message,
                    error.error.status
                )));
            }
            
            return Err(FredAgentError::LlmError(format!(
                "Gemini API returned an error: HTTP {}: {}",
                status.as_u16(),
                error_text
            )));
        }
        
        let model_name_for_stream = self.model.model_id().to_string();
        Ok(Box::pin(GeminiStream::new(response, model_name_for_stream)))
    }
    
    async fn generate_stream_with_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, FredAgentError>> + Send>>, FredAgentError> {
        let url = self.build_streaming_url("streamGenerateContent");
        
        let request = GeminiRequest {
            contents: self.convert_messages(messages),
            safety_settings: None,
            generation_config: Some(self.build_generation_config()),
            tools: self.convert_tools_with_grounding(tools, grounding_config.enable_web_search),
        };
        
        match serde_json::to_string_pretty(&request) {
            Ok(json_request) => log::trace!("GeminiClient: Sending generate_stream_with_grounding request: {}", json_request),
            Err(e) => log::warn!("GeminiClient: Failed to serialize request for logging: {}" , e),
        }
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| FredAgentError::NetworkError(format!("Failed to send request to Gemini API: {}", e)))?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Could not read error response".to_string());

            log::warn!("GeminiClient: Raw error response from API (stream grounding) (HTTP {}): {}", status.as_u16(), error_text);

            // Try to parse as a GeminiError
            if let Ok(error) = serde_json::from_str::<GeminiError>(&error_text) {
                return Err(FredAgentError::LlmError(format!(
                    "Gemini API error: {} ({})",
                    error.error.message,
                    error.error.status
                )));
            }
            
            return Err(FredAgentError::LlmError(format!(
                "Gemini API returned an error: HTTP {}: {}",
                status.as_u16(),
                error_text
            )));
        }
        
        let model_name_for_stream = self.model.model_id().to_string();
        Ok(Box::pin(GeminiStream::new(response, model_name_for_stream)))
    }
    
    async fn generate_stream_with_thinking_and_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
        grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, FredAgentError>> + Send>>, FredAgentError> {
        let url = self.build_streaming_url("streamGenerateContent");
        
        let request = GeminiRequest {
            contents: self.convert_messages(messages),
            safety_settings: None,
            generation_config: Some(self.build_generation_config_with_thinking(thinking_config)),
            tools: self.convert_tools_with_grounding(tools, grounding_config.enable_web_search),
        };
        
        match serde_json::to_string_pretty(&request) {
            Ok(json_request) => log::trace!("GeminiClient: Sending generate_stream_with_thinking_and_grounding request: {}", json_request),
            Err(e) => log::warn!("GeminiClient: Failed to serialize request for logging: {}" , e),
        }
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| FredAgentError::NetworkError(format!("Failed to send request to Gemini API: {}", e)))?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Could not read error response".to_string());

            log::warn!("GeminiClient: Raw error response from API (stream thinking+grounding) (HTTP {}): {}", status.as_u16(), error_text);

            // Try to parse as a GeminiError
            if let Ok(error) = serde_json::from_str::<GeminiError>(&error_text) {
                return Err(FredAgentError::LlmError(format!(
                    "Gemini API error: {} ({})",
                    error.error.message,
                    error.error.status
                )));
            }
            
            return Err(FredAgentError::LlmError(format!(
                "Gemini API returned an error: HTTP {}: {}",
                status.as_u16(),
                error_text
            )));
        }
        
        let model_name_for_stream = self.model.model_id().to_string();
        Ok(Box::pin(GeminiStream::new(response, model_name_for_stream)))
    }
}

/// Factory function to create a new GeminiClient from a configuration
pub fn create_gemini_client(config: &FredAgentConfig) -> Result<Arc<dyn LlmClient>, FredAgentError> {
    let client = GeminiClient::new(config)?;
    Ok(Arc::new(client))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FredAgentConfig, GeminiConfig};
    use serde_json::json;
    use std::collections::HashMap;
    use std::any::Any;

    fn create_test_config() -> FredAgentConfig {
        let mut config = FredAgentConfig::default();
        config.gemini.api_key = Some("test-api-key".to_string());
        config.gemini.model = "gemini-2.5-pro-preview-05-06".to_string();
        config
    }

    fn create_test_client() -> GeminiClient {
        GeminiClient::new(&create_test_config()).unwrap()
    }

    #[test]
    fn test_gemini_client_creation() {
        let client = create_test_client();
        assert_eq!(client.api_key, "test-api-key");
        assert_eq!(client.model, GeminiModel::Pro25Preview);
        assert_eq!(client.parameters.temperature, 0.4);
    }

    #[test]
    fn test_gemini_client_creation_with_env_var() {
        // Test fallback to environment variable
        std::env::set_var("GEMINI_API_KEY", "env-api-key");
        
        let mut config = FredAgentConfig::default();
        config.gemini.api_key = None; // No API key in config
        config.gemini.model = "gemini-1.5-flash-latest".to_string();
        
        let client = GeminiClient::new(&config).unwrap();
        assert_eq!(client.api_key, "env-api-key");
        assert_eq!(client.model, GeminiModel::Flash);
        
        std::env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn test_gemini_client_creation_no_api_key() {
        std::env::remove_var("GEMINI_API_KEY");
        
        let mut config = FredAgentConfig::default();
        config.gemini.api_key = None;
        config.gemini.model = "gemini-1.5-pro-latest".to_string();
        
        let result = GeminiClient::new(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API key is not set"));
    }

    #[test]
    fn test_gemini_client_creation_empty_api_key() {
        let mut config = FredAgentConfig::default();
        config.gemini.api_key = Some("".to_string());
        config.gemini.model = "gemini-1.5-pro-latest".to_string();
        
        // Should fall back to environment variable
        std::env::remove_var("GEMINI_API_KEY");
        let result = GeminiClient::new(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_gemini_client_with_unknown_model() {
        let mut config = FredAgentConfig::default();
        config.gemini.api_key = Some("test-key".to_string());
        config.gemini.model = "unknown-model".to_string();
        
        let client = GeminiClient::new(&config).unwrap();
        assert_eq!(client.model, GeminiModel::Flash25Preview); // Should default to Flash25Preview
    }

    #[test]
    fn test_gemini_client_with_model() {
        let client = create_test_client();
        let client_with_flash = client.with_model(GeminiModel::Flash);
        
        assert_eq!(client_with_flash.model, GeminiModel::Flash);
        assert_eq!(client_with_flash.parameters.temperature, 0.4); // Flash default
    }

    #[test]
    fn test_gemini_client_with_parameters() {
        let custom_params = ModelParameters {
            temperature: 0.8,
            top_p: 0.9,
            top_k: 50,
            max_output_tokens: 2048,
            response_mime_type: Some("application/json".to_string()),
        };
        
        let client = create_test_client();
        let client_with_params = client.with_parameters(custom_params.clone());
        
        assert_eq!(client_with_params.parameters.temperature, 0.8);
        assert_eq!(client_with_params.parameters.top_p, 0.9);
        assert_eq!(client_with_params.parameters.top_k, 50);
        assert_eq!(client_with_params.parameters.max_output_tokens, 2048);
    }

    #[test]
    fn test_build_url() {
        let client = create_test_client();
        let url = client.build_url("generateContent");
        assert_eq!(url, "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro-preview-05-06:generateContent?key=test-api-key");
        
        // Test with different model
        let client_flash = client.with_model(GeminiModel::Flash);
        let url = client_flash.build_url("streamGenerateContent");
        assert_eq!(url, "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash-latest:streamGenerateContent?key=test-api-key");
    }

    #[test]
    fn test_convert_messages_simple() {
        let client = create_test_client();
        
        let messages = vec![
            Message {
                id: Uuid::new_v4(),
                role: Role::User,
                parts: vec![MessagePart::Text { text: "Hello".to_string() }],
                metadata: HashMap::new(),
            }
        ];
        
        let contents = client.convert_messages(&messages);
        
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, "user");
        assert_eq!(contents[0].parts.len(), 1);
        assert_eq!(contents[0].parts[0].text, Some("Hello".to_string()));
    }

    #[test]
    fn test_convert_tools_empty() {
        let client = create_test_client();
        let tools = vec![];
        let result = client.convert_tools(&tools);
        assert!(result.is_none());
    }

    #[test]
    fn test_convert_tools_with_tools() {
        let client = create_test_client();
        
        let tools = vec![
            ToolDefinition {
                name: "get_weather".to_string(),
                description: "Get current weather".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    },
                    "required": ["location"]
                }),
                is_required: false,
            }
        ];
        
        let result = client.convert_tools(&tools);
        assert!(result.is_some());
        
        let gemini_tools = result.unwrap();
        assert_eq!(gemini_tools.len(), 1);
        assert!(gemini_tools[0].function_declarations.is_some());
        let function_declarations = gemini_tools[0].function_declarations.as_ref().unwrap();
        assert_eq!(function_declarations.len(), 1);
        assert_eq!(function_declarations[0].name, "get_weather");
    }

    #[test]
    fn test_build_generation_config() {
        let client = create_test_client();
        let config = client.build_generation_config();
        
        assert_eq!(config.temperature, Some(client.parameters.temperature));
        assert_eq!(config.top_p, Some(client.parameters.top_p));
        assert_eq!(config.top_k, Some(client.parameters.top_k));
        assert_eq!(config.max_output_tokens, Some(client.parameters.max_output_tokens));
        assert!(config.stop_sequences.is_none());
    }

    #[test]
    fn test_extract_tool_calls_empty() {
        let client = create_test_client();
        
        let response = GeminiResponse {
            candidates: vec![],
            prompt_feedback: None,
            usage_metadata: None,
        };
        
        let tool_calls = client.extract_tool_calls(&response);
        assert!(tool_calls.is_empty());
    }

    #[test]
    fn test_extract_tool_calls_with_function_calls() {
        use crate::llm::gemini::api::{Candidate, Content, Part, FunctionCall};
        
        let client = create_test_client();
        
        let response = GeminiResponse {
            candidates: vec![
                Candidate {
                    content: Content {
                        role: "model".to_string(),
                        parts: vec![
                            Part {
                                text: Some("I'll help you with that.".to_string()),
                                function_call: None,
                                function_response: None,
                                thought: None,
                            },
                            Part {
                                text: None,
                                function_call: Some(FunctionCall {
                                    name: "search_web".to_string(),
                                    args: json!({"query": "rust programming"}),
                                }),
                                function_response: None,
                                thought: None,
                            },
                        ],
                    },
                    safety_ratings: vec![],
                    finish_reason: Some("STOP".to_string()),
                    grounding_metadata: None,
                }
            ],
            prompt_feedback: None,
            usage_metadata: None,
        };
        
        let tool_calls = client.extract_tool_calls(&response);
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].1, "search_web");
        assert_eq!(tool_calls[0].2, json!({"query": "rust programming"}));
    }

    #[test]
    fn test_convert_response_empty_candidates() {
        let client = create_test_client();
        
        let response = GeminiResponse {
            candidates: vec![],
            prompt_feedback: None,
            usage_metadata: None,
        };
        
        let result = client.convert_response(response);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no candidates"));
    }

    #[test]
    fn test_convert_response_success() {
        use crate::llm::gemini::api::{Candidate, Content, Part};
        
        let client = create_test_client();
        
        let response = GeminiResponse {
            candidates: vec![
                Candidate {
                    content: Content {
                        role: "model".to_string(),
                        parts: vec![
                            Part {
                                text: Some("Hello! How can I help you today?".to_string()),
                                function_call: None,
                                function_response: None,
                                thought: None,
                            },
                        ],
                    },
                    safety_ratings: vec![],
                    finish_reason: Some("STOP".to_string()),
                    grounding_metadata: None,
                }
            ],
            prompt_feedback: None,
            usage_metadata: None,
        };
        
        let result = client.convert_response(response).unwrap();
        
        assert_eq!(result.message.role, Role::Assistant);
        assert_eq!(result.message.parts.len(), 1);
        
        if let MessagePart::Text { text } = &result.message.parts[0] {
            assert_eq!(text, "Hello! How can I help you today?");
        } else {
            panic!("Expected text part");
        }
        
        assert!(result.tool_calls.is_empty());
    }

    #[test]
    fn test_convert_response_with_tool_calls() {
        use crate::llm::gemini::api::{Candidate, Content, Part, FunctionCall};
        
        let client = create_test_client();
        
        let response = GeminiResponse {
            candidates: vec![
                Candidate {
                    content: Content {
                        role: "model".to_string(),
                        parts: vec![
                            Part {
                                text: Some("I'll search for that.".to_string()),
                                function_call: None,
                                function_response: None,
                                thought: None,
                            },
                            Part {
                                text: None,
                                function_call: Some(FunctionCall {
                                    name: "web_search".to_string(),
                                    args: json!({"query": "latest news"}),
                                }),
                                function_response: None,
                                thought: None,
                            },
                        ],
                    },
                    safety_ratings: vec![],
                    finish_reason: Some("STOP".to_string()),
                    grounding_metadata: None,
                }
            ],
            prompt_feedback: None,
            usage_metadata: None,
        };
        
        let result = client.convert_response(response).unwrap();
        
        assert_eq!(result.message.role, Role::Assistant);
        assert_eq!(result.message.parts.len(), 2);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].1, "web_search");
    }

    #[test]
    fn test_convert_response_with_function_response() {
        use crate::llm::gemini::api::{Candidate, Content, Part, FunctionResponse};
        
        let client = create_test_client();
        
        let response = GeminiResponse {
            candidates: vec![
                Candidate {
                    content: Content {
                        role: "model".to_string(),
                        parts: vec![
                            Part {
                                text: None,
                                function_call: None,
                                function_response: Some(FunctionResponse {
                                    name: "get_weather".to_string(),
                                    response: json!({"temperature": 72, "condition": "sunny"}),
                                }),
                                thought: None,
                            },
                        ],
                    },
                    safety_ratings: vec![],
                    finish_reason: Some("STOP".to_string()),
                    grounding_metadata: None,
                }
            ],
            prompt_feedback: None,
            usage_metadata: None,
        };
        
        let result = client.convert_response(response).unwrap();
        
        assert_eq!(result.message.parts.len(), 1);
        if let MessagePart::ToolResult { name, result: tool_result, .. } = &result.message.parts[0] {
            assert_eq!(name, "get_weather");
            assert_eq!(tool_result, &json!({"temperature": 72, "condition": "sunny"}));
        } else {
            panic!("Expected tool result part");
        }
    }

    #[test]
    fn test_convert_response_empty_parts() {
        use crate::llm::gemini::api::{Candidate, Content, Part};
        
        let client = create_test_client();
        
        let response = GeminiResponse {
            candidates: vec![
                Candidate {
                    content: Content {
                        role: "model".to_string(),
                        parts: vec![
                            Part {
                                text: None,
                                function_call: None,
                                function_response: None,
                                thought: None,
                            },
                        ],
                    },
                    safety_ratings: vec![],
                    finish_reason: Some("STOP".to_string()),
                    grounding_metadata: None,
                }
            ],
            prompt_feedback: None,
            usage_metadata: None,
        };
        
        let result = client.convert_response(response).unwrap();
        
        assert_eq!(result.message.parts.len(), 1);
        if let MessagePart::Text { text } = &result.message.parts[0] {
            assert_eq!(text, "");
        } else {
            panic!("Expected empty text part");
        }
    }

    #[test]
    fn test_create_gemini_client_function() {
        let config = create_test_config();
        let result = create_gemini_client(&config);
        
        // Test that we can successfully create a client
        assert!(result.is_ok());
        let _client_arc = result.unwrap();
        // If we get here, the client was created successfully
    }

    #[test]
    fn test_create_gemini_client_with_invalid_config() {
        let mut config = FredAgentConfig::default();
        config.gemini.api_key = None;
        config.gemini.model = "invalid-model".to_string();
        
        std::env::remove_var("GEMINI_API_KEY");
        let result = create_gemini_client(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_model_fallback_behavior() {
        // Test specific model ID that should trigger fallback
        let mut config = FredAgentConfig::default();
        config.gemini.api_key = Some("test-key".to_string());
        config.gemini.model = "gemini-1.5-pro-preview-05-06".to_string();
        
        let client = GeminiClient::new(&config).unwrap();
        // Should use Flash25Preview for deprecated 1.x models
        assert_eq!(client.model, GeminiModel::Flash25Preview);
    }

    #[test]
    fn test_client_chaining() {
        let client = create_test_client();
        
        let custom_params = ModelParameters {
            temperature: 0.9,
            top_p: 0.8,
            top_k: 60,
            max_output_tokens: 4096,
            response_mime_type: None,
        };
        
        let chained_client = client
            .with_model(GeminiModel::Flash)
            .with_parameters(custom_params);
        
        assert_eq!(chained_client.model, GeminiModel::Flash);
        assert_eq!(chained_client.parameters.temperature, 0.9);
        assert_eq!(chained_client.parameters.top_p, 0.8);
        assert_eq!(chained_client.parameters.top_k, 60);
        assert_eq!(chained_client.parameters.max_output_tokens, 4096);
    }
}

