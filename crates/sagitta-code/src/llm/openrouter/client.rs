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
        Self::new_with_base_url(config, "https://openrouter.ai/api/v1")
    }

    /// Create a new OpenRouter client with a custom base URL (useful for testing)
    pub fn new_with_base_url(config: &SagittaCodeConfig, base_url: &str) -> Result<Self, OpenRouterError> {
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

        let base_url_string = base_url.to_string();
        let model_manager = ModelManager::new(http_client.clone(), base_url_string.clone());

        Ok(Self {
            config: config.openrouter.clone(),
            http_client,
            base_url: base_url_string,
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

    /// Get recent models from the last year
    pub async fn get_recent_models(&self) -> Result<Vec<ModelInfo>, OpenRouterError> {
        self.model_manager.get_recent_models().await
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

    /// Get the model manager for external use (e.g., in settings panel)
    pub fn get_model_manager(&self) -> &ModelManager {
        &self.model_manager
    }

    /// Convert our Message format to OpenRouter's ChatMessage format
    fn convert_messages(&self, messages: &[Message]) -> Vec<ChatMessage> {
        let raw_converted: Vec<ChatMessage> = messages.iter().filter_map(|msg| {
            match msg.role {
                Role::User | Role::System => {
                    // For user/system messages, combine all text parts
                    let content = msg.parts.iter()
                        .filter_map(|part| match part {
                            MessagePart::Text { text } => Some(text.clone()),
                            MessagePart::Thought { text } => Some(format!("<thinking>{}</thinking>", text)),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Skip messages with empty content - this can cause 400 errors with OpenRouter
                    if content.trim().is_empty() {
                        log::warn!("Skipping message with empty content for role: {:?}", msg.role);
                        return None;
                    }

                    Some(ChatMessage {
                        role: match msg.role {
                            Role::User => "user".to_string(),
                            Role::System => "system".to_string(),
                            _ => unreachable!(),
                        },
                        content: Some(content),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    })
                }
                Role::Assistant => {
                    // For assistant messages, handle both text and tool calls
                    let content = msg.parts.iter()
                        .filter_map(|part| match part {
                            MessagePart::Text { text } => Some(text.clone()),
                            MessagePart::Thought { text } => Some(format!("<thinking>{}</thinking>", text)),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    let tool_calls = msg.parts.iter()
                        .filter_map(|part| match part {
                            MessagePart::ToolCall { tool_call_id, name, parameters } => {
                                Some(super::api::ToolCall {
                                    id: tool_call_id.clone(),
                                    tool_type: "function".to_string(),
                                    function: super::api::FunctionCall {
                                        name: name.clone(),
                                        arguments: serde_json::to_string(parameters).unwrap_or_default(),
                                    },
                                })
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>();

                    // Assistant messages need either content or tool calls
                    if content.trim().is_empty() && tool_calls.is_empty() {
                        log::warn!("Skipping assistant message with no content or tool calls");
                        return None;
                    }

                    Some(ChatMessage {
                        role: "assistant".to_string(),
                        content: if content.trim().is_empty() { None } else { Some(content) },
                        tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                        tool_call_id: None,
                        name: None,
                    })
                }
                Role::Function => {
                    // For function/tool result messages
                    if let Some(MessagePart::ToolResult { tool_call_id, name, result }) = msg.parts.first() {
                        let content_str = serde_json::to_string(result).unwrap_or_default();
                        if content_str.trim().is_empty() {
                            log::warn!("Skipping tool result message with empty content");
                            return None;
                        }
                        
                        Some(ChatMessage {
                            role: "tool".to_string(),
                            content: Some(content_str),
                            tool_calls: None,
                            tool_call_id: Some(tool_call_id.clone()),
                            name: Some(name.clone()),
                        })
                    } else {
                        // Fallback for function messages without proper tool result
                        let content = msg.parts.iter()
                            .filter_map(|part| match part {
                                MessagePart::Text { text } => Some(text.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n");

                        if content.trim().is_empty() {
                            log::warn!("Skipping function message with empty content");
                            return None;
                        }

                        Some(ChatMessage {
                            role: "assistant".to_string(),
                            content: Some(content),
                            tool_calls: None,
                            tool_call_id: None,
                            name: None,
                        })
                    }
                }
            }
        }).collect();

        // Apply history truncation to prevent payload size issues
        self.truncate_conversation_history(raw_converted)
    }

    /// Truncate conversation history to prevent 400 errors due to large payloads
    fn truncate_conversation_history(&self, messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
        const MAX_HISTORY_MESSAGES: usize = 10; // Keep system + last 9 messages
        
        if messages.len() <= MAX_HISTORY_MESSAGES {
            return messages;
        }

        let mut result = Vec::new();
        
        // Always keep system message if present
        if let Some(first_msg) = messages.first() {
            if first_msg.role == "system" {
                result.push(first_msg.clone());
            }
        }

        // Take the last N messages (excluding system if already added)
        let start_index = if !result.is_empty() && messages[0].role == "system" {
            // We have system message, take last (MAX-1) messages after system
            std::cmp::max(1, messages.len().saturating_sub(MAX_HISTORY_MESSAGES - 1))
        } else {
            // No system message, take last MAX messages
            messages.len().saturating_sub(MAX_HISTORY_MESSAGES)
        };

        result.extend(messages[start_index..].iter().cloned());
        
        log::debug!("Truncated conversation history from {} to {} messages", messages.len(), result.len());
        result
    }

    /// Check if the current model supports structured outputs
    async fn supports_structured_outputs(&self) -> bool {
        // Try to get model info from cache/API
        match self.model_manager.get_model_by_id(&self.config.model).await {
            Ok(Some(model_info)) => {
                // Check if supported_parameters contains "structured_outputs"
                let supports = model_info.supported_parameters
                    .as_ref()
                    .map(|params| params.iter().any(|p| p == "structured_outputs"))
                    .unwrap_or(false);
                
                if let Some(params) = &model_info.supported_parameters {
                    log::debug!("Model {} supported parameters: {:?}", self.config.model, params);
                }
                
                supports
            }
            Ok(None) => {
                log::warn!("Model {} not found in model list, assuming no structured outputs support", self.config.model);
                false
            }
            Err(e) => {
                log::warn!("Failed to get model info for {}: {}, assuming no structured outputs support", self.config.model, e);
                false
            }
        }
    }

    /// Convert Sagitta ToolDefinition to OpenRouter Tool format
    async fn convert_tools(&self, tools: &[ToolDefinition]) -> Vec<super::api::Tool> {
        let supports_structured = self.supports_structured_outputs().await;
        
        log::debug!(
            "Model {} supports structured outputs: {}", 
            self.config.model, 
            supports_structured
        );
        
        tools.iter().map(|tool| {
            super::api::Tool {
                tool_type: "function".to_string(),
                function: super::api::FunctionDefinition {
                    name: tool.name.clone(),
                    description: Some(tool.description.clone()),
                    parameters: Some(self.make_schema_strict_compliant(tool.parameters.clone())),
                    strict: if supports_structured { Some(true) } else { None },
                },
            }
        }).collect()
    }

    /// Make a JSON schema strict-mode compliant for OpenAI function calling
    /// This ensures additionalProperties is false and handles optional fields properly
    fn make_schema_strict_compliant(&self, mut schema: serde_json::Value) -> serde_json::Value {
        if let Some(obj) = schema.as_object_mut() {
            // Ensure additionalProperties is set to false for all objects
            if obj.get("type").and_then(|t| t.as_str()) == Some("object") {
                obj.insert("additionalProperties".to_string(), serde_json::Value::Bool(false));
                
                // Get the required fields list (if any)
                let required_fields: std::collections::HashSet<String> = obj
                    .get("required")
                    .and_then(|r| r.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default();
                
                // Recursively apply to nested properties and handle optional fields
                if let Some(properties) = obj.get_mut("properties").and_then(|p| p.as_object_mut()) {
                    for (prop_name, prop_value) in properties.iter_mut() {
                        // If this property is not required, ensure it allows null
                        if !required_fields.contains(prop_name) {
                            if let Some(prop_obj) = prop_value.as_object_mut() {
                                // Check if type is already an array
                                if let Some(type_value) = prop_obj.get_mut("type") {
                                    match type_value {
                                        serde_json::Value::String(type_str) => {
                                            // Convert single type to array with null
                                            if type_str != "null" {
                                                *type_value = serde_json::json!([type_str.clone(), "null"]);
                                            }
                                        }
                                        serde_json::Value::Array(type_array) => {
                                            // Ensure null is in the array if not already present
                                            let has_null = type_array.iter().any(|v| v.as_str() == Some("null"));
                                            if !has_null {
                                                type_array.push(serde_json::Value::String("null".to_string()));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        
                        // Recursively process nested schemas
                        *prop_value = self.make_schema_strict_compliant(prop_value.clone());
                    }
                }
                
                // Don't override the required array - respect what the tool definition specifies
                // The tool definitions should already have the correct required fields
            }
            
            // Handle arrays
            if obj.get("type").and_then(|t| t.as_str()) == Some("array") {
                if let Some(items) = obj.get_mut("items") {
                    *items = self.make_schema_strict_compliant(items.clone());
                }
            }
        }
        
        schema
    }

    /// Convert OpenRouter response to our LlmResponse format
    fn convert_response(&self, response: ChatCompletionResponse) -> LlmResponse {
        let choice = response.choices.into_iter().next()
            .unwrap_or_else(|| Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: Some("No response generated".to_string()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
                finish_reason: Some("error".to_string()),
            });

        let mut parts = Vec::new();
        
        // Add text content if present
        if let Some(content) = &choice.message.content {
            if !content.trim().is_empty() {
                parts.push(MessagePart::Text { 
                    text: content.clone() 
                });
            }
        }

        // Add tool calls if present
        if let Some(tool_calls) = &choice.message.tool_calls {
            for tool_call in tool_calls {
                let arguments = serde_json::from_str(&tool_call.function.arguments)
                    .unwrap_or(serde_json::Value::String(tool_call.function.arguments.clone()));
                
                parts.push(MessagePart::ToolCall {
                    tool_call_id: tool_call.id.clone(),
                    name: tool_call.function.name.clone(),
                    parameters: arguments,
                });
            }
        }

        // If no parts, add empty text
        if parts.is_empty() {
            parts.push(MessagePart::Text { text: String::new() });
        }

        let message = Message {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            parts,
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

        // Extract tool calls for the response
        let tool_calls = choice.message.tool_calls.unwrap_or_default().into_iter().map(|tc| {
            let arguments = serde_json::from_str(&tc.function.arguments)
                .unwrap_or(serde_json::Value::String(tc.function.arguments));
            (tc.id, tc.function.name, arguments)
        }).collect();

        LlmResponse {
            message,
            tool_calls,
            usage,
            grounding: None,
        }
    }
}

#[async_trait]
impl LlmClient for OpenRouterClient {
    async fn generate(&self, 
        messages: &[Message], 
        tools: &[ToolDefinition]
    ) -> Result<LlmResponse, SagittaCodeError> {
        let openrouter_tools = if !tools.is_empty() {
            Some(self.convert_tools(tools).await)
        } else {
            None
        };

        let tool_choice = if !tools.is_empty() {
            Some(super::api::ToolChoice::String("auto".to_string()))
        } else {
            None
        };

        let converted_messages = self.convert_messages(messages);
        
        // Validate we have at least one message - required by OpenRouter API
        if converted_messages.is_empty() {
            return Err(SagittaCodeError::LlmError(
                "No valid messages to send to OpenRouter. All messages were empty or invalid.".to_string()
            ));
        }

        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: converted_messages,
            stream: Some(false),
            max_tokens: None, // Use model defaults
            temperature: None,
            top_p: None,
            tools: openrouter_tools,
            tool_choice,
            provider: self.config.provider_preferences.as_ref().map(|p| ProviderPreferences {
                order: p.order.clone(),
                allow_fallbacks: p.allow_fallbacks,
                sort: p.sort.clone(),
                data_collection: p.data_collection.clone(),
            }),
            plugins: None,
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
        grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // OpenRouter supports web search via plugins
        if grounding_config.enable_web_search {
            let openrouter_tools = if !tools.is_empty() {
                Some(self.convert_tools(tools).await)
            } else {
                None
            };

            let tool_choice = if !tools.is_empty() {
                Some(super::api::ToolChoice::String("auto".to_string()))
            } else {
                None
            };

            let converted_messages = self.convert_messages(messages);
            
            if converted_messages.is_empty() {
                return Err(SagittaCodeError::LlmError(
                    "No valid messages to send to OpenRouter. All messages were empty or invalid.".to_string()
                ));
            }

            // Create web search plugin configuration
            let plugins = vec![super::api::Plugin {
                id: "web".to_string(),
                max_results: Some(5),
                search_prompt: Some(format!(
                    "A web search was conducted on {}. Incorporate the following web search results into your response.",
                    chrono::Utc::now().format("%Y-%m-%d")
                )),
            }];

            let request = ChatCompletionRequest {
                model: self.config.model.clone(),
                messages: converted_messages,
                stream: Some(false),
                max_tokens: None,
                temperature: None,
                top_p: None,
                tools: openrouter_tools,
                tool_choice,
                provider: self.config.provider_preferences.as_ref().map(|p| ProviderPreferences {
                    order: p.order.clone(),
                    allow_fallbacks: p.allow_fallbacks,
                    sort: p.sort.clone(),
                    data_collection: p.data_collection.clone(),
                }),
                plugins: Some(plugins),
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
        } else {
            // No web search requested, use regular generate
            self.generate(messages, tools).await
        }
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
        tools: &[ToolDefinition]
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        let openrouter_tools = if !tools.is_empty() {
            Some(self.convert_tools(tools).await)
        } else {
            None
        };

        let tool_choice = if !tools.is_empty() {
            Some(super::api::ToolChoice::String("auto".to_string()))
        } else {
            None
        };

        let converted_messages = self.convert_messages(messages);
        
        // Validate we have at least one message - required by OpenRouter API
        if converted_messages.is_empty() {
            return Err(SagittaCodeError::LlmError(
                "No valid messages to send to OpenRouter. All messages were empty or invalid.".to_string()
            ));
        }

        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: converted_messages,
            stream: Some(true), // Enable streaming
            max_tokens: None,
            temperature: None,
            top_p: None,
            tools: openrouter_tools,
            tool_choice,
            provider: self.config.provider_preferences.as_ref().map(|p| ProviderPreferences {
                order: p.order.clone(),
                allow_fallbacks: p.allow_fallbacks,
                sort: p.sort.clone(),
                data_collection: p.data_collection.clone(),
            }),
            plugins: None,
        };

        // Debug logging to diagnose issues
        log::debug!("OpenRouter streaming request - Model: {}", &request.model);
        log::debug!("OpenRouter streaming request - Messages count: {}", request.messages.len());
        log::debug!("OpenRouter streaming request - Tools count: {}", request.tools.as_ref().map_or(0, |t| t.len()));
        if let Some(ref tools) = request.tools {
            for (i, tool) in tools.iter().enumerate() {
                log::debug!("Tool {}: {} ({})", i, tool.function.name, tool.tool_type);
                // Log each tool's parameters schema for debugging
                if let Some(ref params) = tool.function.parameters {
                    if let Ok(schema_str) = serde_json::to_string_pretty(params) {
                        log::debug!("Tool {} schema:\n{}", tool.function.name, schema_str);
                    }
                }
            }
        }
        
        // Serialize request for debugging but truncate if too large
        if let Ok(json_str) = serde_json::to_string_pretty(&request) {
            if json_str.len() > 10000 {
                log::debug!("OpenRouter streaming request JSON (truncated):\n{}", &json_str[..10000]);
            } else {
                log::debug!("OpenRouter streaming request JSON:\n{}", json_str);
            }
        }

        let url = format!("{}/chat/completions", self.base_url);
        log::debug!("OpenRouter streaming URL: {}", url);
        
        // First, test the request with a cloned builder to get detailed error info
        let test_request_builder = self.http_client
            .post(&url)
            .json(&request);
        
        // Try a HEAD request first to validate the request without streaming
        let validation_request = self.http_client
            .post(&url)
            .header("Accept", "application/json") // Regular JSON response instead of SSE
            .json(&{
                let mut test_req = request.clone();
                test_req.stream = Some(false); // Disable streaming for validation
                test_req
            });
        
        log::debug!("Sending validation request to check for 400 errors...");
        match validation_request.send().await {
            Ok(response) => {
                let status = response.status();
                log::debug!("Validation request status: {}", status);
                
                if !status.is_success() {
                    let error_text = response.text().await.unwrap_or_else(|e| {
                        log::error!("Failed to read validation error response: {}", e);
                        "Failed to read error response".to_string()
                    });
                    
                    log::error!("OpenRouter validation request failed with status {}: {}", status, error_text);
                    
                    // Try to parse the error as JSON for better details
                    if let Ok(json_error) = serde_json::from_str::<serde_json::Value>(&error_text) {
                        log::error!("OpenRouter validation error details (JSON): {}", 
                            serde_json::to_string_pretty(&json_error).unwrap_or_default());
                    } else {
                        log::error!("OpenRouter validation error details (raw): {}", error_text);
                    }
                    
                    return Err(SagittaCodeError::LlmError(
                        format!("OpenRouter request validation failed with HTTP {}: {}", status, error_text)
                    ));
                } else {
                    log::debug!("Validation request succeeded, proceeding with streaming...");
                }
            }
            Err(e) => {
                log::error!("Validation request failed: {}", e);
                return Err(SagittaCodeError::LlmError(
                    format!("OpenRouter validation request failed: {}", e)
                ));
            }
        }
        
        // Now create the actual streaming request
        let request_builder = self.http_client
            .post(&url)
            .json(&request);

        let stream = OpenRouterStream::new(request_builder)?;
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
        // Save the current environment variable state
        let original_env_key = std::env::var("OPENROUTER_API_KEY").ok();
        
        // Clear any existing environment variable first
        std::env::remove_var("OPENROUTER_API_KEY");
        
        // Test with API key from environment
        std::env::set_var("OPENROUTER_API_KEY", "env-test-key");
        
        let mut config = create_test_config();
        config.openrouter.api_key = None; // No API key in config
        
        let client = OpenRouterClient::new(&config);
        
        // Restore the original environment variable state immediately
        match original_env_key {
            Some(key) => std::env::set_var("OPENROUTER_API_KEY", key),
            None => std::env::remove_var("OPENROUTER_API_KEY"),
        }
        
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
        // Save the current environment variable state
        let original_env_key = std::env::var("OPENROUTER_API_KEY").ok();
        
        let mut config = create_test_config();
        config.openrouter.api_key = None;
        
        // Ensure no environment variable
        std::env::remove_var("OPENROUTER_API_KEY");
        
        let client = OpenRouterClient::new(&config);
        
        // Restore the original environment variable state immediately
        match original_env_key {
            Some(key) => std::env::set_var("OPENROUTER_API_KEY", key),
            None => std::env::remove_var("OPENROUTER_API_KEY"),
        }
        
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
        assert_eq!(converted[0].content, Some("You are a helpful assistant.".to_string()));
        assert_eq!(converted[1].role, "user");
        assert_eq!(converted[1].content, Some("Hello, how are you?".to_string()));
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
        let content = converted[0].content.as_ref().unwrap();
        assert!(content.contains("First part"));
        assert!(content.contains("Second part"));
        assert!(content.contains("<thinking>Thinking about this</thinking>"));
    }

    #[test]
    fn test_convert_response() {
        let config = create_test_config();
        let client = OpenRouterClient::new(&config).unwrap();
        
        let response = ChatCompletionResponse {
            id: "test-id".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "test-model".to_string(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: Some("Hello, world!".to_string()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        };
        
        let converted = client.convert_response(response);
        
        assert_eq!(converted.message.role, Role::Assistant);
        assert_eq!(converted.message.parts.len(), 1);
        
        if let MessagePart::Text { text } = &converted.message.parts[0] {
            assert_eq!(text, "Hello, world!");
        } else {
            panic!("Expected text message part");
        }
        
        assert!(converted.usage.is_some());
        let usage = converted.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn test_make_schema_strict_compliant() {
        let config = create_test_config();
        let client = OpenRouterClient::new(&config).unwrap();
        
        // Test schema with optional fields (like our ReadFileTool)
        let schema = serde_json::json!({
            "type": "object",
            "required": ["file_path"],
            "properties": {
                "repository_name": {
                    "type": ["string", "null"],
                    "description": "Optional repository name"
                },
                "file_path": {
                    "type": "string",
                    "description": "Required file path"
                },
                "start_line": {
                    "type": ["integer", "null"],
                    "description": "Optional start line"
                }
            }
        });
        
        let result = client.make_schema_strict_compliant(schema);
        
        // Should have additionalProperties: false
        assert_eq!(result["additionalProperties"], false);
        
        // Should preserve the original required array
        let required = result["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "file_path");
        
        // Optional fields should already have null type, so they should be unchanged
        let props = result["properties"].as_object().unwrap();
        assert_eq!(props["repository_name"]["type"], serde_json::json!(["string", "null"]));
        assert_eq!(props["file_path"]["type"], "string");
        assert_eq!(props["start_line"]["type"], serde_json::json!(["integer", "null"]));
    }

    #[test]
    fn test_make_schema_strict_compliant_converts_optional_fields() {
        let config = create_test_config();
        let client = OpenRouterClient::new(&config).unwrap();
        
        // Test schema where optional fields don't already have null type
        let schema = serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Required name"
                },
                "age": {
                    "type": "integer",
                    "description": "Optional age"
                }
            }
        });
        
        let result = client.make_schema_strict_compliant(schema);
        
        // Required field should remain unchanged
        let props = result["properties"].as_object().unwrap();
        assert_eq!(props["name"]["type"], "string");
        
        // Optional field should now allow null
        assert_eq!(props["age"]["type"], serde_json::json!(["integer", "null"]));
    }
} 