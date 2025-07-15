use std::pin::Pin;
use std::task::{Context, Poll};
use futures_util::Stream;
use pin_project::pin_project;
use tokio_stream::StreamExt;
use serde::{Deserialize, Serialize};
use crate::llm::client::{StreamChunk, MessagePart};
use crate::utils::errors::SagittaCodeError;

// Helper function to process buffer and extract complete SSE lines
fn process_buffer_direct(buffer: &mut String) -> Vec<Result<StreamChunk, SagittaCodeError>> {
    let mut events = Vec::new();
    let lines: Vec<&str> = buffer.split('\n').collect();
    
    // Process all complete lines (all but the last one unless it ends with \n)
    let (complete_lines, remainder) = if buffer.ends_with('\n') {
        (lines.as_slice(), "")
    } else if lines.len() > 1 {
        (&lines[..lines.len() - 1], lines[lines.len() - 1])
    } else {
        let empty: &[&str] = &[];
        (empty, *lines.get(0).unwrap_or(&""))
    };

    for line in complete_lines {
        if let Some(event) = parse_sse_line_direct(line) {
            events.push(event);
        }
    }

    // Keep the remainder in the buffer
    *buffer = remainder.to_string();
    events
}

// Helper function to parse SSE lines
fn parse_sse_line_direct(line: &str) -> Option<Result<StreamChunk, SagittaCodeError>> {
    // Skip empty lines and non-data lines
    if line.is_empty() || !line.starts_with("data: ") {
        return None;
    }

    let data = &line[6..]; // Remove "data: " prefix

    // Handle the termination signal
    if data == "[DONE]" {
        return Some(Ok(StreamChunk {
            part: MessagePart::Text { text: String::new() },
            is_final: true,
            finish_reason: Some("done".to_string()),
            token_usage: None,
        }));
    }

    // Parse JSON data
    match serde_json::from_str::<OpenAIStreamResponse>(data) {
        Ok(response) => {
            if response.done {
                return Some(Ok(StreamChunk {
                    part: MessagePart::Text { text: String::new() },
                    is_final: true,
                    finish_reason: Some("done".to_string()),
                    token_usage: None,
                }));
            }

            for choice in response.choices {
                // Check for reasoning/thinking content first
                if let Some(reasoning) = choice.delta.reasoning_content {
                    if !reasoning.is_empty() {
                        return Some(Ok(StreamChunk {
                            part: MessagePart::Thought { text: reasoning },
                            is_final: false,
                            finish_reason: None,
                            token_usage: None,
                        }));
                    }
                }
                
                // Check if content is marked as thinking/reasoning
                if let Some(content_type) = &choice.delta.content_type {
                    if (content_type == "thinking" || content_type == "reasoning") && choice.delta.content.is_some() {
                        let content = choice.delta.content.as_ref().unwrap();
                        if !content.is_empty() {
                            return Some(Ok(StreamChunk {
                                part: MessagePart::Thought { text: content.clone() },
                                is_final: false,
                                finish_reason: None,
                                token_usage: None,
                            }));
                        }
                    }
                }
                
                // Regular content
                if let Some(content) = choice.delta.content {
                    if !content.is_empty() {
                        return Some(Ok(StreamChunk {
                            part: MessagePart::Text { text: content },
                            is_final: false,
                            finish_reason: None,
                            token_usage: None,
                        }));
                    }
                }

                if let Some(finish_reason) = choice.finish_reason {
                    if finish_reason == "stop" || finish_reason == "length" {
                        return Some(Ok(StreamChunk {
                            part: MessagePart::Text { text: String::new() },
                            is_final: true,
                            finish_reason: Some(finish_reason),
                            token_usage: None,
                        }));
                    }
                }
            }
            None
        }
        Err(e) => {
            // Log the error but don't fail the stream for parsing errors
            log::debug!("Failed to parse SSE data: {}, error: {}", data, e);
            None
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamResponse {
    choices: Vec<OpenAIChoice>,
    #[serde(default)]
    done: bool,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    delta: OpenAIDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIDelta {
    content: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    content_type: Option<String>,
}

#[pin_project]
pub struct MistralRsStream {
    #[pin]
    response_stream: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    buffer: String,
}

impl MistralRsStream {
    pub fn new(response_stream: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>) -> Self {
        Self {
            response_stream,
            buffer: String::new(),
        }
    }


}

impl Stream for MistralRsStream {
    type Item = Result<StreamChunk, SagittaCodeError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // First, check if we have any complete events in our buffer
        let events = process_buffer_direct(this.buffer);
        if !events.is_empty() {
            return Poll::Ready(Some(events.into_iter().next().unwrap()));
        }

        // Poll the underlying stream for more data
        match this.response_stream.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                // Add the new chunk to our buffer
                if let Ok(text) = std::str::from_utf8(&chunk) {
                    this.buffer.push_str(text);
                    
                    // Process the buffer and return the first event if any
                    let events = process_buffer_direct(this.buffer);
                    if !events.is_empty() {
                        Poll::Ready(Some(events.into_iter().next().unwrap()))
                    } else {
                        // No complete events yet, need more data
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                } else {
                    Poll::Ready(Some(Err(SagittaCodeError::ConfigError(
                        "Invalid UTF-8 in stream response".to_string(),
                    ))))
                }
            }
            Poll::Ready(Some(Err(e))) => {
                Poll::Ready(Some(Err(SagittaCodeError::LlmError(format!(
                    "Stream error: {}",
                    e
                )))))
            }
            Poll::Ready(None) => {
                // Stream ended, check if we have any remaining data in the buffer
                if !this.buffer.is_empty() {
                    let events = process_buffer_direct(this.buffer);
                    if !events.is_empty() {
                        return Poll::Ready(Some(events.into_iter().next().unwrap()));
                    }
                }
                Poll::Ready(Some(Ok(StreamChunk {
                    part: MessagePart::Text { text: String::new() },
                    is_final: true,
                    finish_reason: Some("done".to_string()),
                    token_usage: None,
                })))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream::iter;

    #[tokio::test]
    async fn test_stream_parsing() {
        let test_data = vec![
            Ok(bytes::Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n")),
            Ok(bytes::Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n")),
            Ok(bytes::Bytes::from("data: [DONE]\n")),
        ];

        let mock_stream = Box::pin(iter(test_data));
        let mut mistral_stream = MistralRsStream::new(mock_stream);

        // First event should be "Hello"
        let event1 = mistral_stream.next().await.unwrap().unwrap();
        match event1.part {
            MessagePart::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Text part"),
        }
        assert!(!event1.is_final);

        // Second event should be " world"
        let event2 = mistral_stream.next().await.unwrap().unwrap();
        match event2.part {
            MessagePart::Text { text } => assert_eq!(text, " world"),
            _ => panic!("Expected Text part"),
        }
        assert!(!event2.is_final);

        // Third event should be final
        let event3 = mistral_stream.next().await.unwrap().unwrap();
        assert!(event3.is_final);
    }

    #[test]
    fn test_sse_parsing() {
        let mut stream = MistralRsStream {
            response_stream: Box::pin(iter(vec![])),
            buffer: String::new(),
        };

        // Test valid SSE data
        let event = parse_sse_line_direct("data: {\"choices\":[{\"delta\":{\"content\":\"test\"}}]}");
        let chunk = event.unwrap().unwrap();
        match chunk.part {
            MessagePart::Text { text } => assert_eq!(text, "test"),
            _ => panic!("Expected Text part"),
        }
        assert!(!chunk.is_final);

        // Test done signal
        let event = parse_sse_line_direct("data: [DONE]");
        let chunk = event.unwrap().unwrap();
        assert!(chunk.is_final);

        // Test non-data line
        let event = parse_sse_line_direct("event: message");
        assert!(event.is_none());
    }
}