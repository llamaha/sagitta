// TODO: Implement OpenRouter streaming in Phase 2
// This is a placeholder to make the code compile

use std::pin::Pin;
use std::task::{Context, Poll};
use futures_util::{Stream, StreamExt};
use reqwest_eventsource::{Event, EventSource};
use serde_json;
use uuid::Uuid;

use crate::llm::client::{StreamChunk, MessagePart, TokenUsage};
use crate::utils::errors::SagittaCodeError;
use super::api::{StreamChunk as OpenRouterStreamChunk, ToolCallDelta, FunctionCallDelta};

/// Stream for OpenRouter SSE responses using proper SSE parsing
pub struct OpenRouterStream {
    event_source: EventSource,
    accumulated_tool_calls: std::collections::HashMap<u32, AccumulatedToolCall>,
    is_finished: bool,
}

/// Accumulated tool call during streaming
#[derive(Debug, Clone)]
struct AccumulatedToolCall {
    index: u32,
    id: Option<String>,
    tool_type: Option<String>,
    function_name: Option<String>,
    function_arguments: String,
}

impl OpenRouterStream {
    /// Create a new OpenRouter stream from a RequestBuilder
    pub fn new(request_builder: reqwest::RequestBuilder) -> Result<Self, SagittaCodeError> {
        // Note: Do NOT add Content-Type or Accept headers here as they're already set by the client
        // and reqwest_eventsource will handle the Accept header correctly
        // Adding duplicate headers can cause 400 Bad Request errors
        
        let event_source = EventSource::new(request_builder)
            .map_err(|e| {
                log::error!("Failed to create EventSource for OpenRouter stream: {}", e);
                SagittaCodeError::LlmError(format!("Failed to create EventSource: {}", e))
            })?;
        Ok(Self {
            event_source,
            accumulated_tool_calls: std::collections::HashMap::new(),
            is_finished: false,
        })
    }

    /// Parse OpenRouter SSE event into a stream chunk
    fn parse_openrouter_event(&mut self, event: Event) -> Result<Option<StreamChunk>, SagittaCodeError> {
        match event {
            Event::Open => {
                // Connection opened, no chunk to return
                Ok(None)
            }
            Event::Message(msg) => {
                // Check for end marker
                if msg.data.trim() == "[DONE]" {
                    self.is_finished = true;
                    return Ok(Some(StreamChunk {
                        part: MessagePart::Text { text: String::new() },
                        is_final: true,
                        finish_reason: Some("stop".to_string()),
                        token_usage: None,
                    }));
                }

                // Parse JSON chunk
                let chunk: OpenRouterStreamChunk = serde_json::from_str(&msg.data)
                    .map_err(|e| SagittaCodeError::LlmError(format!("Failed to parse OpenRouter streaming chunk: {}", e)))?;

                // Extract content from the first choice
                if let Some(choice) = chunk.choices.into_iter().next() {
                    let is_final = choice.finish_reason.is_some();
                    let finish_reason = choice.finish_reason;
                    
                    // Handle text content
                    if let Some(content) = choice.delta.content {
                        if !content.is_empty() {
                            return Ok(Some(StreamChunk {
                                part: MessagePart::Text { text: content },
                                is_final,
                                finish_reason,
                                token_usage: None,
                            }));
                        }
                    }
                    
                    // Handle tool calls
                    if let Some(tool_calls) = choice.delta.tool_calls {
                        for tool_call_delta in tool_calls {
                            self.accumulate_tool_call(tool_call_delta);
                        }
                        
                        // If final and we have tool calls, return them
                        if is_final && !self.accumulated_tool_calls.is_empty() {
                            if let Some(first_tool_call) = self.accumulated_tool_calls.values().next() {
                                let parameters = if !first_tool_call.function_arguments.is_empty() {
                                    serde_json::from_str(&first_tool_call.function_arguments)
                                        .unwrap_or(serde_json::Value::String(first_tool_call.function_arguments.clone()))
                                } else {
                                    serde_json::Value::Null
                                };
                                
                                return Ok(Some(StreamChunk {
                                    part: MessagePart::ToolCall {
                                        tool_call_id: first_tool_call.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string()),
                                        name: first_tool_call.function_name.clone().unwrap_or_default(),
                                        parameters,
                                    },
                                    is_final,
                                    finish_reason,
                                    token_usage: None,
                                }));
                            }
                        }
                    }
                }

                Ok(None)
            }
        }
    }
    
    /// Accumulate tool call delta into our internal state
    fn accumulate_tool_call(&mut self, delta: ToolCallDelta) {
        // Find or create tool call entry
        let entry = self.accumulated_tool_calls.entry(delta.index).or_insert_with(|| {
            AccumulatedToolCall {
                index: delta.index,
                id: None,
                tool_type: None,
                function_name: None,
                function_arguments: String::new(),
            }
        });
        
        // Update fields from delta
        if let Some(id) = delta.id {
            entry.id = Some(id);
        }
        if let Some(tool_type) = delta.tool_type {
            entry.tool_type = Some(tool_type);
        }
        if let Some(function) = delta.function {
            if let Some(name) = function.name {
                entry.function_name = Some(name);
            }
            if let Some(args) = function.arguments {
                entry.function_arguments.push_str(&args);
            }
        }
    }
}

impl Stream for OpenRouterStream {
    type Item = Result<StreamChunk, SagittaCodeError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.is_finished {
            return Poll::Ready(None);
        }

        // Poll the event source for new events
        match Pin::new(&mut self.event_source).poll_next(cx) {
            Poll::Ready(Some(Ok(event))) => {
                match self.parse_openrouter_event(event) {
                    Ok(Some(chunk)) => Poll::Ready(Some(Ok(chunk))),
                    Ok(None) => {
                        // No chunk generated, continue polling
                        self.poll_next(cx)
                    }
                    Err(e) => Poll::Ready(Some(Err(e))),
                }
            }
            Poll::Ready(Some(Err(e))) => {
                log::error!("SSE stream error from OpenRouter: {}", e);
                
                // Try to extract more details from the error
                let error_details = format!("{:?}", e);
                log::error!("Full SSE error details: {}", error_details);
                
                // Check if this is a specific HTTP status error
                if error_details.contains("400") || error_details.contains("Bad Request") {
                    log::error!("Detected 400 Bad Request error in SSE stream");
                    // Try to extract any response body if available
                    if error_details.contains("body") {
                        log::error!("Error may contain response body details");
                    }
                }
                
                Poll::Ready(Some(Err(SagittaCodeError::LlmError(
                    format!("SSE stream error: {}", e)
                ))))
            }
            Poll::Ready(None) => {
                // Stream ended
                if !self.is_finished {
                    self.is_finished = true;
                    Poll::Ready(Some(Ok(StreamChunk {
                        part: MessagePart::Text { text: String::new() },
                        is_final: true,
                        finish_reason: Some("stop".to_string()),
                        token_usage: None,
                    })))
                } else {
                    Poll::Ready(None)
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
} 