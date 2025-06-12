// TODO: Implement OpenRouter streaming in Phase 2
// This is a placeholder to make the code compile

use std::pin::Pin;
use std::task::{Context, Poll};
use futures_util::{Stream, StreamExt};
use serde_json;
use uuid::Uuid;

use crate::llm::client::{StreamChunk, MessagePart, TokenUsage};
use crate::utils::errors::SagittaCodeError;
use super::api::{StreamChunk as OpenRouterStreamChunk, ChatCompletionRequest};

/// Stream for OpenRouter SSE responses
pub struct OpenRouterStream {
    inner: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    current_content: String,
    is_finished: bool,
}

impl OpenRouterStream {
    /// Create a new OpenRouter stream from an HTTP response
    pub fn new(response: reqwest::Response) -> Self {
        let stream = response.bytes_stream();
        Self {
            inner: Box::pin(stream),
            current_content: String::new(),
            is_finished: false,
        }
    }

    /// Parse an SSE event line into a stream chunk
    fn parse_sse_chunk(&mut self, line: &str) -> Result<Option<StreamChunk>, SagittaCodeError> {
        // SSE format: "data: {json}"
        if !line.starts_with("data: ") {
            return Ok(None);
        }

        let data = line.strip_prefix("data: ").unwrap();
        
        // Check for end marker
        if data.trim() == "[DONE]" {
            self.is_finished = true;
            return Ok(Some(StreamChunk {
                part: MessagePart::Text { 
                    text: self.current_content.clone() 
                },
                is_final: true,
                finish_reason: Some("stop".to_string()),
                token_usage: None,
            }));
        }

        // Parse JSON chunk
        let chunk: OpenRouterStreamChunk = serde_json::from_str(data)
            .map_err(|e| SagittaCodeError::LlmError(format!("Failed to parse streaming chunk: {}", e)))?;

        // Extract content from the first choice
        let choice = chunk.choices.into_iter().next();
        if let Some(choice) = choice {
            if let Some(content) = choice.delta.content {
                self.current_content.push_str(&content);
                
                let is_final = choice.finish_reason.is_some();
                let finish_reason = choice.finish_reason;
                
                return Ok(Some(StreamChunk {
                    part: MessagePart::Text { text: content },
                    is_final,
                    finish_reason,
                    token_usage: None, // OpenRouter doesn't send usage in streaming chunks
                }));
            }
        }

        Ok(None)
    }
}

impl Stream for OpenRouterStream {
    type Item = Result<StreamChunk, SagittaCodeError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.is_finished {
            return Poll::Ready(None);
        }

        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                let text = String::from_utf8_lossy(&bytes);
                let lines: Vec<&str> = text.lines().collect();
                
                for line in lines {
                    match self.parse_sse_chunk(line) {
                        Ok(Some(chunk)) => {
                            if chunk.is_final {
                                self.is_finished = true;
                            }
                            return Poll::Ready(Some(Ok(chunk)));
                        }
                        Ok(None) => continue, // Skip non-data lines or empty chunks
                        Err(e) => return Poll::Ready(Some(Err(e))),
                    }
                }
                
                // If we get here, none of the lines produced a chunk, poll again
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Poll::Ready(Some(Err(e))) => {
                Poll::Ready(Some(Err(SagittaCodeError::LlmError(
                    format!("Stream error: {}", e)
                ))))
            }
            Poll::Ready(None) => {
                // Stream ended
                if !self.is_finished {
                    // Send final chunk with accumulated content
                    self.is_finished = true;
                    Poll::Ready(Some(Ok(StreamChunk {
                        part: MessagePart::Text { 
                            text: String::new() // Empty since we already sent content
                        },
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