use super::translator::OpenAITranslator;
use super::types::*;
use super::stream_processor::FinalChunk;
use crate::llm::client::{
    GroundingConfig, LlmClient, LlmResponse, Message, MessagePart, StreamChunk,
    ThinkingConfig, TokenUsage, ToolDefinition,
};
use crate::providers::claude_code::mcp_integration::McpIntegration;
use crate::utils::errors::SagittaCodeError;
use async_trait::async_trait;
use futures_util::Stream;
use reqwest::Client;
use serde_json::Value;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

pub struct OpenAICompatibleClient {
    base_url: String,
    api_key: Option<String>,
    model: Option<String>,
    http_client: Client,
    mcp_integration: Arc<McpIntegration>,
    timeout: Duration,
    max_retries: u32,
}

impl OpenAICompatibleClient {
    pub fn new(
        base_url: String,
        api_key: Option<String>,
        model: Option<String>,
        mcp_integration: Arc<McpIntegration>,
        timeout_seconds: u64,
        max_retries: u32,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url,
            api_key,
            model,
            http_client,
            mcp_integration,
            timeout: Duration::from_secs(timeout_seconds),
            max_retries,
        }
    }

    async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> Result<T, SagittaCodeError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, SagittaCodeError>>,
    {
        let mut retries = 0;
        let mut last_error = None;

        while retries <= self.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    if retries < self.max_retries {
                        let delay = Duration::from_millis(100 * 2u64.pow(retries));
                        tokio::time::sleep(delay).await;
                    }
                    retries += 1;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            SagittaCodeError::LlmError("Max retries exceeded".to_string())
        }))
    }

    async fn send_chat_request(
        &self,
        request: &OpenAIChatRequest,
    ) -> Result<OpenAIChatResponse, SagittaCodeError> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut req_builder = self.http_client.post(&url).json(request);

        // Add API key if provided
        if let Some(ref api_key) = &self.api_key {
            req_builder = req_builder.bearer_auth(api_key);
        }

        log::debug!("OpenAICompatible: Sending HTTP request to {}", url);
        
        let response = req_builder.send().await.map_err(|e| {
            log::error!("OpenAICompatible: Failed to send HTTP request: {}", e);
            SagittaCodeError::LlmError(format!("Failed to send request: {}", e))
        })?;

        let status = response.status();
        log::debug!("OpenAICompatible: Received HTTP status: {}", status);

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            log::error!("OpenAICompatible: HTTP error response: {}", error_text);

            // Try to parse as OpenAI error response
            if let Ok(error_response) = serde_json::from_str::<OpenAIErrorResponse>(&error_text) {
                return Err(SagittaCodeError::LlmError(format!(
                    "OpenAI API error ({}): {}",
                    status, error_response.error.message
                )));
            }

            return Err(SagittaCodeError::LlmError(format!(
                "HTTP error {}: {}",
                status, error_text
            )));
        }

        // Get the response text first so we can log it
        let response_text = response.text().await.map_err(|e| {
            log::error!("OpenAICompatible: Failed to read response body: {}", e);
            SagittaCodeError::LlmError(format!("Failed to read response: {}", e))
        })?;

        log::debug!("OpenAICompatible: Raw response: {}", response_text);

        // Parse the response
        serde_json::from_str(&response_text).map_err(|e| {
            log::error!("OpenAICompatible: Failed to parse JSON response: {}", e);
            log::error!("OpenAICompatible: Response was: {}", response_text);
            SagittaCodeError::LlmError(format!("Failed to parse response: {}", e))
        })
    }

    fn convert_response(&self, response: OpenAIChatResponse) -> Result<LlmResponse, SagittaCodeError> {
        let choice = response
            .choices
            .first()
            .ok_or_else(|| SagittaCodeError::ParseError("No choices in response".to_string()))?;

        let message = OpenAITranslator::openai_to_message(&choice.message);

        // Extract tool calls for the response
        let tool_calls = choice.message
            .tool_calls
            .as_ref()
            .map(|calls| {
                calls
                    .iter()
                    .map(|tc| {
                        let params: Value = serde_json::from_str(&tc.function.arguments)
                            .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
                        (tc.id.clone(), tc.function.name.clone(), params)
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Convert usage if present
        let usage = response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens as i32,
            completion_tokens: u.completion_tokens as i32,
            total_tokens: u.total_tokens as i32,
            thinking_tokens: None,
            model_name: response.model.clone(),
            cached_tokens: None,
        });

        Ok(LlmResponse {
            message,
            tool_calls,
            usage,
            grounding: None,
        })
    }

    fn build_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        stream: bool,
    ) -> Result<OpenAIChatRequest, SagittaCodeError> {
        // Log incoming tools
        log::info!("OpenAICompatible: building request with {} tools, stream={}", tools.len(), stream);
        for tool in tools {
            log::info!("  Tool: {} - {}", tool.name, tool.description);
        }

        // Convert tools to OpenAI format
        let openai_tools: Vec<OpenAITool> = tools
            .iter()
            .map(|tool| OpenAITranslator::tool_to_openai(tool))
            .collect();

        // Debug: Log converted OpenAI tools
        log::debug!("Converted to {} OpenAI tools", openai_tools.len());
        if !openai_tools.is_empty() {
            log::debug!("OpenAI tools: {}", serde_json::to_string_pretty(&openai_tools).unwrap_or_else(|_| "Failed to serialize".to_string()));
        }

        // Convert messages to OpenAI format
        let openai_messages = OpenAITranslator::messages_to_openai(messages);

        // Build OpenAI request
        Ok(OpenAIChatRequest {
            model: self.model.clone(),
            messages: openai_messages,
            tools: if openai_tools.is_empty() {
                None
            } else {
                Some(openai_tools.clone())
            },
            tool_choice: if openai_tools.is_empty() {
                None
            } else {
                Some(serde_json::json!("auto"))
            },
            temperature: Some(0.7),
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: stream,
        })
    }

    async fn generate_internal(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse, SagittaCodeError> {
        let openai_request = self.build_request(messages, tools, false)?;

        // Log the full request being sent
        log::info!("OpenAICompatible: Sending request to {}/chat/completions", self.base_url);
        log::info!("OpenAICompatible: Model: {:?}, Temperature: {:?}", openai_request.model, openai_request.temperature);
        log::info!("OpenAICompatible: Has tools: {}, Tool choice: {:?}", 
            openai_request.tools.is_some(), openai_request.tool_choice);
        
        // Always log the full request for debugging tool issues
        log::info!("OpenAICompatible: Full request JSON:\n{}", 
            serde_json::to_string_pretty(&openai_request).unwrap_or_else(|_| "Failed to serialize request".to_string()));

        // Send request with retry
        let response = self
            .execute_with_retry(|| async { self.send_chat_request(&openai_request).await })
            .await?;

        // Debug: Log the response
        log::debug!("Received OpenAI response: {}", serde_json::to_string_pretty(&response).unwrap_or_else(|_| "Failed to serialize response".to_string()));

        self.convert_response(response)
    }
}

#[async_trait]
impl LlmClient for OpenAICompatibleClient {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn client_type(&self) -> &str {
        "openai_compatible"
    }

    async fn generate(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse, SagittaCodeError> {
        log::info!("OpenAICompatibleClient::generate called with {} messages and {} tools", messages.len(), tools.len());
        if !tools.is_empty() {
            log::info!("OpenAICompatible: Tool names: {}", 
                tools.iter().map(|t| &t.name).cloned().collect::<Vec<_>>().join(", "));
        }
        self.generate_internal(messages, tools).await
    }

    async fn generate_with_thinking(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // OpenAI-compatible APIs don't support thinking mode
        // Just use regular generation
        self.generate(messages, tools).await
    }

    async fn generate_with_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // OpenAI-compatible APIs don't support grounding
        // Just use regular generation
        self.generate(messages, tools).await
    }

    async fn generate_with_thinking_and_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
        _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        // OpenAI-compatible APIs don't support thinking or grounding
        // Just use regular generation
        self.generate(messages, tools).await
    }

    async fn generate_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError>
    {
        log::debug!("OpenAICompatibleClient::generate_stream called with {} messages and {} tools", messages.len(), tools.len());
        
        let request = self.build_request(messages, tools, true)?;
        
        let response = self.http_client
            .post(&format!("{}/chat/completions", self.base_url))
            .json(&request)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| SagittaCodeError::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SagittaCodeError::LlmError(format!(
                "OpenAI-compatible API error: {}",
                error_text
            )));
        }
        
        // Get the byte stream from the response
        let byte_stream = response.bytes_stream();
        
        // Create SSE parser
        let sse_parser = super::sse_parser::SseParser::new(byte_stream);
        
        // Create the stream adapter
        let stream = OpenAIStreamAdapter::new(sse_parser);
        
        Ok(Box::pin(stream))
    }

    async fn generate_stream_with_thinking(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError>
    {
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError>
    {
        self.generate_stream(messages, tools).await
    }

    async fn generate_stream_with_thinking_and_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _thinking_config: &ThinkingConfig,
        _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError>
    {
        self.generate_stream(messages, tools).await
    }
}

use pin_project::pin_project;
use std::task::{Context as TaskContext, Poll};
use futures_util::stream::StreamExt;

/// Stream adapter that converts SSE events to StreamChunks
#[pin_project]
struct OpenAIStreamAdapter<S> {
    #[pin]
    sse_stream: S,
    stream_processor: super::stream_processor::StreamProcessor,
    done: bool,
    buffered_chunks: Vec<StreamChunk>,
    buffered_tool_calls: Vec<StreamChunk>,
}

impl<S> OpenAIStreamAdapter<S> {
    fn new(sse_stream: S) -> Self {
        Self {
            sse_stream,
            stream_processor: super::stream_processor::StreamProcessor::new(),
            done: false,
            buffered_chunks: Vec::new(),
            buffered_tool_calls: Vec::new(),
        }
    }
}

impl<S> Stream for OpenAIStreamAdapter<S>
where
    S: Stream<Item = Result<super::sse_parser::SseEvent, Box<dyn std::error::Error + Send + Sync>>>,
{
    type Item = Result<StreamChunk, SagittaCodeError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        
        if *this.done && this.buffered_tool_calls.is_empty() && this.buffered_chunks.is_empty() {
            return Poll::Ready(None);
        }
        
        // Return buffered tool calls first
        if !this.buffered_tool_calls.is_empty() {
            let chunk = this.buffered_tool_calls.remove(0);
            return Poll::Ready(Some(Ok(chunk)));
        }
        
        // Then return buffered chunks
        if !this.buffered_chunks.is_empty() {
            let chunk = this.buffered_chunks.remove(0);
            return Poll::Ready(Some(Ok(chunk)));
        }
        
        // If we're done and no more buffered items, return None
        if *this.done {
            return Poll::Ready(None);
        }
        
        loop {
            match this.sse_stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(event))) => {
                    match event {
                        super::sse_parser::SseEvent::Message(json) => {
                            // Check if this is an error message
                            if let Some(error) = json.get("error") {
                                let error_msg = error.get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("Unknown error");
                                return Poll::Ready(Some(Err(SagittaCodeError::LlmError(
                                    format!("OpenAI API error: {}", error_msg)
                                ))));
                            }
                            
                            // Process the delta
                            if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
                                if let Some(choice) = choices.first() {
                                    if let Some(delta) = choice.get("delta") {
                                        let chunks = this.stream_processor.process_delta(delta);
                                        
                                        // Buffer all chunks
                                        if !chunks.is_empty() {
                                            this.buffered_chunks.extend(chunks);
                                            // Return the first buffered chunk
                                            if !this.buffered_chunks.is_empty() {
                                                let chunk = this.buffered_chunks.remove(0);
                                                return Poll::Ready(Some(Ok(chunk)));
                                            }
                                        }
                                    }
                                    
                                    // Check for finish reason
                                    if let Some(finish_reason) = choice.get("finish_reason")
                                        .and_then(|v| v.as_str())
                                        .filter(|s| !s.is_empty() && *s != "null")
                                    {
                                        // Get any remaining tool calls
                                        let tool_calls = this.stream_processor.get_completed_tool_calls();
                                        if !tool_calls.is_empty() {
                                            // Buffer tool calls before final chunk
                                            for tool_call in tool_calls {
                                                this.buffered_tool_calls.push(StreamChunk {
                                                    part: MessagePart::ToolCall {
                                                        tool_call_id: tool_call.id,
                                                        name: tool_call.name,
                                                        parameters: tool_call.arguments,
                                                    },
                                                    is_final: false,
                                                    finish_reason: None,
                                                    token_usage: None,
                                                });
                                            }
                                            
                                            // Return the first tool call
                                            if !this.buffered_tool_calls.is_empty() {
                                                let chunk = this.buffered_tool_calls.remove(0);
                                                return Poll::Ready(Some(Ok(chunk)));
                                            }
                                        }
                                        
                                        // Create final chunk
                                        if let Some(final_chunk) = this.stream_processor.create_final_chunk(Some(finish_reason.to_string())) {
                                            return Poll::Ready(Some(Ok(StreamChunk {
                                                part: MessagePart::Text { 
                                                    text: final_chunk.content 
                                                },
                                                is_final: true,
                                                finish_reason: Some(finish_reason.to_string()),
                                                token_usage: None, // TODO: Extract token usage if available
                                            })));
                                        } else {
                                            // Even if no content, emit an empty final chunk to signal completion
                                            return Poll::Ready(Some(Ok(StreamChunk {
                                                part: MessagePart::Text { 
                                                    text: String::new()
                                                },
                                                is_final: true,
                                                finish_reason: Some(finish_reason.to_string()),
                                                token_usage: None,
                                            })));
                                        }
                                    }
                                }
                            }
                            
                            // Continue processing
                            continue;
                        }
                        super::sse_parser::SseEvent::Done => {
                            *this.done = true;
                            
                            // Emit any remaining content or tool calls
                            let tool_calls = this.stream_processor.get_completed_tool_calls();
                            if !tool_calls.is_empty() {
                                // Buffer all tool calls
                                for tool_call in tool_calls {
                                    this.buffered_tool_calls.push(StreamChunk {
                                        part: MessagePart::ToolCall {
                                            tool_call_id: tool_call.id,
                                            name: tool_call.name,
                                            parameters: tool_call.arguments,
                                        },
                                        is_final: false,
                                        finish_reason: None,
                                        token_usage: None,
                                    });
                                }
                                
                                // Return the first tool call
                                if !this.buffered_tool_calls.is_empty() {
                                    let chunk = this.buffered_tool_calls.remove(0);
                                    return Poll::Ready(Some(Ok(chunk)));
                                }
                            }
                            
                            // Create final chunk
                            if let Some(final_chunk) = this.stream_processor.create_final_chunk(Some("stop".to_string())) {
                                return Poll::Ready(Some(Ok(StreamChunk {
                                    part: MessagePart::Text { 
                                        text: final_chunk.content 
                                    },
                                    is_final: true,
                                    finish_reason: Some("stop".to_string()),
                                    token_usage: None,
                                })));
                            } else {
                                // Even if no content, emit an empty final chunk to signal completion
                                return Poll::Ready(Some(Ok(StreamChunk {
                                    part: MessagePart::Text { 
                                        text: String::new()
                                    },
                                    is_final: true,
                                    finish_reason: Some("stop".to_string()),
                                    token_usage: None,
                                })));
                            }
                        }
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(SagittaCodeError::NetworkError(
                        format!("SSE parse error: {}", e)
                    ))));
                }
                Poll::Ready(None) => {
                    *this.done = true;
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = OpenAICompatibleClient::new(
            "http://localhost:1234/v1".to_string(),
            None,
            None,
            Arc::new(McpIntegration::new()),
            120,
            3,
        );

        assert_eq!(client.base_url, "http://localhost:1234/v1");
        assert!(client.api_key.is_none());
        assert!(client.model.is_none());
    }
}