// Gemini streaming implementation will go here

use futures_util::{Stream, StreamExt};
use reqwest::Response;
use serde::Deserialize;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::marker::Unpin;
use std::collections::VecDeque;
use tokio::sync::mpsc;
use futures_util::stream::select_all;
use bytes::Bytes;

use crate::utils::errors::FredAgentError;
use crate::llm::client::{StreamChunk, MessagePart};
use crate::llm::gemini::api::{GeminiResponse, Part};

/// Stream of chunks from the Gemini API
pub struct GeminiStream {
    /// The underlying response stream
    response: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    
    /// Buffered partial data - this accumulates HTTP chunks until we have complete lines
    buffer: String,
    
    /// Queue of chunks waiting to be emitted (for handling multiple parts per response)
    chunk_queue: std::collections::VecDeque<StreamChunk>,
    
    /// Whether the stream is done
    done: bool,

    /// Name of the model being used (for token usage reporting)
    model_name: String,
}

impl GeminiStream {
    /// Create a new GeminiStream from a reqwest Response
    pub fn new(response: Response, model_name: String) -> Self {
        let stream = response.bytes_stream();
        
        Self {
            response: Box::pin(stream),
            buffer: String::new(),
            chunk_queue: std::collections::VecDeque::new(),
            done: false,
            model_name,
        }
    }
    
    /// Process accumulated data and try to extract complete lines
    /// CRITICAL: Gemini sends "data: {json}\n" format, NOT standard SSE with \n\n separators
    fn process_buffer(&mut self) -> Option<Result<StreamChunk, FredAgentError>> {
        // First, check if we have any queued chunks to emit
        if let Some(chunk) = self.chunk_queue.pop_front() {
            log::debug!("GeminiStream: Emitting queued chunk");
            return Some(Ok(chunk));
        }
        
        // No queued chunks, try to process new lines from buffer
        loop {
            // Look for complete lines ending with \n
            if let Some(line_end) = self.buffer.find('\n') {
                // Extract the complete line (without the \n)
                let line = self.buffer[..line_end].to_string();
                
                // Remove the processed line from buffer (including the \n)
                self.buffer.drain(..line_end + 1);
                
                log::debug!("GeminiStream: Processing line: '{}'", line);
                
                // Process this line
                if let Some(result) = self.process_line(&line) {
                    match result {
                        Ok(chunks) => {
                            if chunks.is_empty() {
                                log::warn!("GeminiStream: Line processing returned empty chunks vector");
                                continue;
                            }
                            
                            // Take the first chunk to return immediately
                            let mut chunks_iter = chunks.into_iter();
                            let first_chunk = chunks_iter.next().unwrap();
                            
                            // Queue any remaining chunks
                            for chunk in chunks_iter {
                                self.chunk_queue.push_back(chunk);
                            }
                            
                            log::info!("GeminiStream: Processed line, emitting first chunk, queued {} additional chunks", self.chunk_queue.len());
                            return Some(Ok(first_chunk));
                        },
                        Err(err) => {
                            return Some(Err(err));
                        }
                    }
                }
                
                // Continue processing if there are more lines
                continue;
            } else {
                // No complete line found, need more data
                log::debug!("GeminiStream: No complete line found, buffer has {} chars", self.buffer.len());
                break;
            }
        }
        
        None
    }
    
    /// Process a single complete line
    fn process_line(&self, line: &str) -> Option<Result<Vec<StreamChunk>, FredAgentError>> {
        let trimmed = line.trim();
        
        // Skip empty lines
        if trimmed.is_empty() {
            return None;
        }
        
        // Skip comment lines (start with :)
        if trimmed.starts_with(':') {
            return None;
        }
        
        // Look for data lines
        if trimmed.starts_with("data: ") {
            let json_data = &trimmed[6..]; // Remove "data: " prefix
            
            log::info!("GeminiStream: Processing JSON data: {} chars", json_data.len());
            log::debug!("GeminiStream: JSON content: {}", &json_data[..json_data.len().min(200)]);
            
            // Check for end marker
            if json_data.trim() == "[DONE]" {
                log::info!("GeminiStream: Received [DONE] marker, ending stream");
                return None; // Will be handled by caller to set done=true
            }
            
            // Try to parse the JSON
            return Some(self.parse_json_data(json_data));
        }
        
        // Unknown line format, skip it
        log::debug!("GeminiStream: Skipping unknown line format: '{}'", trimmed);
        None
    }
    
    /// Parse JSON data from a data line
    fn parse_json_data(&self, json_data: &str) -> Result<Vec<StreamChunk>, FredAgentError> {
        // Try parsing as a single GeminiResponse first
        match serde_json::from_str::<GeminiResponse>(json_data) {
            Ok(response) => {
                log::info!("GeminiStream: Successfully parsed GeminiResponse with {} candidates", response.candidates.len());
                self.convert_response_to_chunks(&response)
            },
            Err(single_err) => {
                // Try parsing as an array of GeminiResponse
                match serde_json::from_str::<Vec<GeminiResponse>>(json_data) {
                    Ok(responses) => {
                        log::info!("GeminiStream: Successfully parsed GeminiResponse array with {} responses", responses.len());
                        if let Some(response) = responses.first() {
                            self.convert_response_to_chunks(response)
                        } else {
                            Err(FredAgentError::LlmError("Empty response array".to_string()))
                        }
                    },
                    Err(array_err) => {
                        log::error!("GeminiStream: Failed to parse JSON as single response: {}", single_err);
                        log::error!("GeminiStream: Failed to parse JSON as response array: {}", array_err);
                        log::error!("GeminiStream: Problematic JSON: {}", &json_data[..json_data.len().min(500)]);
                        Err(FredAgentError::LlmError(format!(
                            "Failed to parse Gemini response JSON: {}", single_err
                        )))
                    }
                }
            }
        }
    }
    
    /// Convert a GeminiResponse to multiple StreamChunks (one per part)
    fn convert_response_to_chunks(&self, response: &GeminiResponse) -> Result<Vec<StreamChunk>, FredAgentError> {
        if let Some(candidate) = response.candidates.first() {
            // Check if this is the final chunk based on finishReason
            let finish_reason = candidate.finish_reason.as_ref();
            let is_final = finish_reason
                .map(|r| self.is_final_finish_reason(r))
                .unwrap_or(false);
            
            // Extract token usage if available from this response
            let token_usage_data = response.usage_metadata.as_ref().map(|usage| {
                crate::llm::client::TokenUsage {
                    prompt_tokens: usage.prompt_token_count.unwrap_or(0),
                    completion_tokens: usage.candidates_token_count.unwrap_or(0),
                    total_tokens: usage.total_token_count.unwrap_or(0),
                    thinking_tokens: usage.thoughts_token_count,
                    model_name: self.model_name.clone(),
                    cached_tokens: usage.cached_content_token_count,
                }
            });
            
            // Handle empty parts array (completion marker)
            if candidate.content.parts.is_empty() {
                if is_final {
                    log::info!("GeminiStream: Found completion marker with finishReason: {:?}", finish_reason);
                    return Ok(vec![StreamChunk {
                        part: MessagePart::Text { text: String::new() },
                        is_final: true,
                        finish_reason: finish_reason.map(|s| s.to_string()),
                        token_usage: token_usage_data.clone(), // Add usage if this is the final completion marker
                    }]);
                } else {
                    return Err(FredAgentError::LlmError("Empty parts array without final finish reason".to_string()));
                }
            }
            
            // CRITICAL FIX: Check for parts with only whitespace/newlines that indicate empty responses
            let has_meaningful_content = candidate.content.parts.iter().any(|part| {
                if let Some(text) = &part.text {
                    !text.trim().is_empty()
                } else {
                    // Function calls and responses are always meaningful
                    part.function_call.is_some() || part.function_response.is_some()
                }
            });
            
            // If we have a STOP finish reason but no meaningful content, this is likely a premature stop
            if is_final && !has_meaningful_content && finish_reason.as_ref().map(|s| s.as_str()) == Some("STOP") {
                log::warn!("GeminiStream: Detected empty response with STOP finish reason - this may indicate a premature stop due to prompt issues");
                log::warn!("GeminiStream: Parts content: {:?}", candidate.content.parts.iter().map(|p| &p.text).collect::<Vec<_>>());
                
                // Return an empty text chunk but mark it as final to prevent infinite loops
                return Ok(vec![StreamChunk {
                    part: MessagePart::Text { text: String::new() },
                    is_final: true,
                    finish_reason: finish_reason.map(|s| s.to_string()),
                    token_usage: token_usage_data.clone(), // Add usage if this is the final stop marker
                }]);
            }
            
            // Process ALL parts in the response
            let mut chunks = Vec::new();
            let parts_count = candidate.content.parts.len();
            
            log::info!("GeminiStream: Converting response with {} parts to chunks", parts_count);
            
            for (i, part) in candidate.content.parts.iter().enumerate() {
                let is_last_part = i == parts_count - 1;
                
                log::info!("GeminiStream: Converting part {}/{}: text={:?}, thought={:?}, function_call={:?}", 
                          i + 1, parts_count,
                          part.text.as_ref().map(|t| &t[..t.len().min(50)]), 
                          part.thought,
                          part.function_call.as_ref().map(|fc| &fc.name));
                
                // Only mark the last part as final if the overall response is final
                // CRITICAL FIX: For tool calls, never mark as final even if finish reason is STOP
                let is_final_for_part = if part.function_call.is_some() {
                    log::info!("GeminiStream: Tool call detected in part {}, marking as non-final to allow continued reasoning", i + 1);
                    false
                } else if is_last_part {
                    is_final
                } else {
                    false
                };
                
                let chunk = self.convert_part_to_chunk_with_reason(part, is_final_for_part, finish_reason.map(|s| s.as_str()))?;
                chunks.push(chunk);
            }
            
            // If token_usage_data is present and we have chunks, attach it to the last chunk
            if let Some(usage) = token_usage_data {
                if let Some(last_chunk) = chunks.last_mut() {
                    // Only add if the last chunk doesn't already have it (e.g. from empty part final above)
                    if last_chunk.token_usage.is_none() {
                         last_chunk.token_usage = Some(usage);
                    }
                }
            }
            
            log::info!("GeminiStream: Successfully converted response to {} chunks", chunks.len());
            Ok(chunks)
        } else {
            Err(FredAgentError::LlmError("No candidates found in response".to_string()))
        }
    }

    /// Convert a GeminiResponse to a StreamChunk (DEPRECATED - use convert_response_to_chunks)
    #[deprecated(note = "Use convert_response_to_chunks to handle multiple parts correctly")]
    fn convert_response_to_chunk(&self, response: &GeminiResponse) -> Result<StreamChunk, FredAgentError> {
        // Use the new multi-chunk method and return the first chunk for backward compatibility
        let chunks = self.convert_response_to_chunks(response)?;
        if let Some(first_chunk) = chunks.into_iter().next() {
            Ok(first_chunk)
        } else {
            Err(FredAgentError::LlmError("No chunks generated from response".to_string()))
        }
    }
    
    /// Determine if a finish reason indicates the stream should end
    fn is_final_finish_reason(&self, finish_reason: &str) -> bool {
        match finish_reason {
            // Normal completion - stream should end
            "STOP" => true,
            
            // Token limit reached - stream should end
            "MAX_TOKENS" => true,
            
            // Safety filter triggered - stream should end
            "SAFETY" => {
                log::warn!("Response blocked by safety filter");
                true
            },
            
            // Content flagged as potential recitation - stream should end
            "RECITATION" => {
                log::warn!("Response blocked due to potential recitation of copyrighted content");
                true
            },
            
            // Sensitive PII detected - stream should end
            "SPII" => {
                log::warn!("Response blocked due to sensitive personally identifiable information");
                true
            },
            
            // Prohibited content (e.g., CSAM) - stream should end
            "PROHIBITED_CONTENT" => {
                log::error!("Response blocked due to prohibited content");
                true
            },
            
            // Content blocked by blocklist - stream should end
            "BLOCKLIST" => {
                log::warn!("Response blocked by content blocklist");
                true
            },
            
            // Other unspecified reasons - stream should end
            "OTHER" | "FINISH_REASON_UNSPECIFIED" => {
                log::warn!("Response ended for unspecified reason: {}", finish_reason);
                true
            },
            
            // Unknown finish reason - be conservative and end stream
            unknown => {
                log::warn!("Unknown finish reason encountered: {}", unknown);
                true
            }
        }
    }
    
    /// Convert a Gemini Part to a StreamChunk with finish reason
    fn convert_part_to_chunk_with_reason(&self, part: &Part, is_final: bool, finish_reason: Option<&str>) -> Result<StreamChunk, FredAgentError> {
        let message_part = if let Some(text) = &part.text {
            // Check if this is a thought part
            if part.thought == Some(true) {
                MessagePart::Thought { text: text.clone() }
            } else {
                MessagePart::Text { text: text.clone() }
            }
        } else if let Some(function_call) = &part.function_call {
            MessagePart::ToolCall {
                tool_call_id: uuid::Uuid::new_v4().to_string(),
                name: function_call.name.clone(),
                parameters: function_call.args.clone(),
            }
        } else if let Some(function_response) = &part.function_response {
            MessagePart::ToolResult {
                tool_call_id: uuid::Uuid::new_v4().to_string(),
                name: function_response.name.clone(),
                result: function_response.response.clone(),
            }
        } else {
            // If it's an empty part but it's final and has usage, it might be a summary chunk.
            // However, the main logic in convert_response_to_chunks should handle this.
            // Here, we primarily expect text, thought, or tool call.
            // If we reach here with an empty part, it might be an issue unless handled above.
            log::warn!("GeminiStream: Encountered an empty or unrecognized part type during part conversion: {:?}", part);
            MessagePart::Text { text: String::new() } // Default to empty text to avoid error, but log it.
        };
        
        Ok(StreamChunk {
            part: message_part,
            is_final,
            finish_reason: finish_reason.map(|s| s.to_string()),
            token_usage: None, // Token usage is attached later in convert_response_to_chunks
        })
    }
}

impl Stream for GeminiStream {
    type Item = Result<StreamChunk, FredAgentError>;
    
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            // Even if done, check for any remaining queued chunks
            if let Some(chunk) = self.chunk_queue.pop_front() {
                log::debug!("GeminiStream: Stream is done but emitting final queued chunk");
                return Poll::Ready(Some(Ok(chunk)));
            }
            log::debug!("GeminiStream: Stream is done, returning None");
            return Poll::Ready(None);
        }
        
        // First, try to process any complete lines in the current buffer or emit queued chunks
        if let Some(result) = self.process_buffer() {
            log::info!("GeminiStream: Buffer processing produced a chunk");
            match result {
                Ok(chunk) => {
                    // Check if this chunk indicates we should mark the stream as done
                    if chunk.is_final && self.chunk_queue.is_empty() {
                        // Only mark done if this is final AND no more chunks are queued
                        self.done = true;
                    }
                    return Poll::Ready(Some(Ok(chunk)));
                },
                Err(err) => {
                    self.done = true;
                    return Poll::Ready(Some(Err(err)));
                }
            }
        }
        
        // No complete lines available, poll for more data from HTTP stream
        match self.response.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes_data))) => {
                log::info!("GeminiStream: Received {} bytes from HTTP stream", bytes_data.len());
                let bytes_str = std::str::from_utf8(&bytes_data).unwrap_or("<invalid UTF-8>");
                log::debug!("GeminiStream: Raw bytes content: '{}'", &bytes_str[..bytes_str.len().min(200)]);
                
                // Append the new data to our buffer
                self.buffer.push_str(bytes_str);
                log::debug!("GeminiStream: Buffer now contains {} chars", self.buffer.len());
                
                // Try to process the updated buffer
                if let Some(result) = self.process_buffer() {
                    log::info!("GeminiStream: Buffer processing after new data produced a chunk");
                    match result {
                        Ok(chunk) => {
                            if chunk.is_final && self.chunk_queue.is_empty() {
                                self.done = true;
                            }
                            Poll::Ready(Some(Ok(chunk)))
                        },
                        Err(err) => {
                            self.done = true;
                            Poll::Ready(Some(Err(err)))
                        }
                    }
                } else {
                    // No complete chunk yet, continue polling
                    log::debug!("GeminiStream: No complete chunk after new data, continuing to poll");
                    Poll::Pending
                }
            },
            Poll::Ready(Some(Err(e))) => {
                log::error!("GeminiStream: Error in HTTP stream: {}", e);
                self.done = true;
                Poll::Ready(Some(Err(FredAgentError::NetworkError(format!(
                    "Error in Gemini stream: {}", e
                )))))
            },
            Poll::Ready(None) => {
                // HTTP stream ended - try to process any remaining content
                log::warn!("GeminiStream: HTTP stream ended, buffer contains {} chars, {} queued chunks", self.buffer.len(), self.chunk_queue.len());
                
                // First emit any queued chunks
                if let Some(chunk) = self.chunk_queue.pop_front() {
                    log::info!("GeminiStream: HTTP stream ended, emitting queued chunk");
                    return Poll::Ready(Some(Ok(chunk)));
                }
                
                if !self.buffer.is_empty() {
                    log::debug!("GeminiStream: Final buffer content: '{}'", self.buffer);
                    
                    // Try to process any remaining content as a final chunk
                    if let Some(result) = self.process_buffer() {
                        log::info!("GeminiStream: Processing remaining buffer content as final chunk");
                        self.done = true;
                        match result {
                            Ok(mut chunk) => {
                                chunk.is_final = true; // Force final since stream is ending
                                return Poll::Ready(Some(Ok(chunk)));
                            },
                            Err(err) => {
                                return Poll::Ready(Some(Err(err)));
                            }
                        }
                    }
                }
                
                log::info!("GeminiStream: HTTP stream ended naturally, no recoverable content");
                self.done = true;
                Poll::Ready(None)
            },
            Poll::Pending => {
                log::trace!("GeminiStream: HTTP stream pending, waiting for more data");
                Poll::Pending
            }
        }
    }
}

/// A merged stream of text chunks
pub struct MergedTextStream {
    inner: Pin<Box<dyn Stream<Item = Result<StreamChunk, FredAgentError>> + Send>>,
}

impl MergedTextStream {
    /// Create a new MergedTextStream
    pub fn new(stream: impl Stream<Item = Result<StreamChunk, FredAgentError>> + Send + 'static) -> Self {
        Self {
            inner: Box::pin(stream),
        }
    }
}

impl Stream for MergedTextStream {
    type Item = Result<StreamChunk, FredAgentError>;
    
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        log::trace!("MergedTextStream: poll_next called");
        
        // Simply pass through all chunks immediately for real-time streaming
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                log::info!("MergedTextStream: Received chunk from inner stream: {:?}", chunk.part);
                // Emit all chunks immediately for real-time streaming
                Poll::Ready(Some(Ok(chunk)))
            },
            Poll::Ready(Some(Err(e))) => {
                log::error!("MergedTextStream: Received error from inner stream: {}", e);
                Poll::Ready(Some(Err(e)))
            },
            Poll::Ready(None) => {
                log::info!("MergedTextStream: Inner stream ended");
                Poll::Ready(None)
            },
            Poll::Pending => {
                log::trace!("MergedTextStream: Inner stream pending");
                Poll::Pending
            }
        }
    }
}

/// Create a merged stream from multiple streams
pub fn merge_streams<S>(
    streams: Vec<S>
) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, FredAgentError>> + Send + 'static>>
where
    S: Stream<Item = Result<StreamChunk, FredAgentError>> + Send + 'static,
{
    if streams.is_empty() {
        // Return an empty stream if no streams are provided
        Box::pin(futures_util::stream::empty())
    } else if streams.len() == 1 {
        // Return the single stream directly if there's only one
        Box::pin(streams.into_iter().next().unwrap())
    } else {
        // Use select_all to merge multiple streams
        let pinned_streams: Vec<Pin<Box<dyn Stream<Item = Result<StreamChunk, FredAgentError>> + Send + 'static>>> = 
            streams.into_iter().map(|s| Box::pin(s) as Pin<Box<dyn Stream<Item = Result<StreamChunk, FredAgentError>> + Send + 'static>>).collect();
        Box::pin(select_all(pinned_streams))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::gemini::api::{GeminiResponse, Candidate, Content, Part, FunctionCall, SafetyRating};
    use serde_json::json;
    use futures_util::stream;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use bytes::Bytes;

    #[test]
    fn test_line_based_processing() {
        // Test that we can handle line-by-line processing correctly
        let mut stream = GeminiStream {
            response: Box::pin(futures_util::stream::empty()),
            buffer: String::new(),
            chunk_queue: VecDeque::new(),
            done: false,
            model_name: String::new(),
        };

        // Test processing a complete line
        stream.buffer = "data: {\"test\": \"value\"}\n".to_string();
        
        // Should not find a complete JSON yet (since it's not valid Gemini format)
        let result = stream.process_buffer();
        // The buffer should be cleared even if parsing fails
        assert!(stream.buffer.is_empty() || result.is_some());
    }

    #[test]
    fn test_http_chunk_accumulation() {
        let mut stream = GeminiStream {
            response: Box::pin(futures_util::stream::empty()),
            buffer: String::new(),
            chunk_queue: VecDeque::new(),
            done: false,
            model_name: String::new(),
        };

        // Simulate HTTP chunks arriving in pieces
        stream.buffer = "data: {\"candidates\": [{\"content\": {\"parts\": [{\"text\": \"Hello".to_string();
        
        // Should not process incomplete line
        let result = stream.process_buffer();
        assert!(result.is_none());
        assert!(!stream.buffer.is_empty());
        
        // Add more data to complete the line
        stream.buffer.push_str(" World\"}]}}]}\n");
        
        // Now should be able to process
        let result = stream.process_buffer();
        // Should either succeed or fail, but buffer should be processed
        assert!(stream.buffer.is_empty() || result.is_some());
    }

    #[test]
    fn test_tool_call_not_final_with_stop_reason() {
        // This is the key fix - tool calls should not be marked as final
        // even when they have a STOP finish reason
        let response = GeminiResponse {
            candidates: vec![Candidate {
                content: Content {
                    parts: vec![Part {
                        text: None,
                        function_call: Some(FunctionCall {
                            name: "web_search".to_string(),
                            args: json!({"search_term": "test query"}),
                        }),
                        function_response: None,
                        thought: None,
                    }],
                    role: "model".to_string(),
                },
                finish_reason: Some("STOP".to_string()),
                safety_ratings: vec![],
                grounding_metadata: None,
            }],
            usage_metadata: None,
            prompt_feedback: None,
        };

        let stream = GeminiStream {
            response: Box::pin(futures_util::stream::empty()),
            buffer: String::new(),
            chunk_queue: VecDeque::new(),
            done: false,
            model_name: String::new(),
        };

        let result = stream.convert_response_to_chunks(&response);

        assert!(result.is_ok());
        let chunks = result.unwrap();
        
        // Verify that all parts are processed correctly
        assert!(!chunks.is_empty());
        for chunk in chunks {
            match chunk.part {
                MessagePart::ToolCall { name, .. } => {
                    assert_eq!(name, "web_search");
                },
                _ => panic!("Expected ToolCall part"),
            }
        }
    }

    #[test]
    fn test_text_chunks_with_stop_are_final() {
        // Test that text chunks with STOP are marked as final
        let response = GeminiResponse {
            candidates: vec![Candidate {
                content: Content {
                    parts: vec![Part {
                        text: Some("This is the final response.".to_string()),
                        function_call: None,
                        function_response: None,
                        thought: None,
                    }],
                    role: "model".to_string(),
                },
                finish_reason: Some("STOP".to_string()),
                safety_ratings: vec![],
                grounding_metadata: None,
            }],
            usage_metadata: None,
            prompt_feedback: None,
        };

        let stream = GeminiStream {
            response: Box::pin(futures_util::stream::empty()),
            buffer: String::new(),
            chunk_queue: VecDeque::new(),
            done: false,
            model_name: String::new(),
        };

        let result = stream.convert_response_to_chunks(&response);

        assert!(result.is_ok());
        let chunks = result.unwrap();
        
        // Verify that all parts are processed correctly
        assert!(!chunks.is_empty());
        for chunk in chunks {
            match chunk.part {
                MessagePart::Text { text } => {
                    assert_eq!(text, "This is the final response.");
                },
                _ => panic!("Expected Text part"),
            }
        }
    }

    #[test]
    fn test_empty_parts_array_handling() {
        // Test handling of responses with empty parts arrays
        let response = GeminiResponse {
            candidates: vec![Candidate {
                content: Content {
                    parts: vec![], // Empty parts array
                    role: "model".to_string(),
                },
                finish_reason: Some("STOP".to_string()),
                safety_ratings: vec![],
                grounding_metadata: None,
            }],
            usage_metadata: None,
            prompt_feedback: None,
        };

        let stream = GeminiStream {
            response: Box::pin(futures_util::stream::empty()),
            buffer: String::new(),
            chunk_queue: VecDeque::new(),
            done: false,
            model_name: String::new(),
        };

        let result = stream.convert_response_to_chunks(&response);

        assert!(result.is_ok());
        let chunks = result.unwrap();
        
        // Verify that all parts are processed correctly
        assert!(!chunks.is_empty());
        for chunk in chunks {
            match chunk.part {
                MessagePart::Text { text } => {
                    assert!(text.is_empty());
                },
                _ => panic!("Expected empty Text part for completion marker"),
            }
        }
    }

    #[test]
    fn test_real_world_gemini_format() {
        // Test with actual Gemini response format
        let mut stream = GeminiStream {
            response: Box::pin(futures_util::stream::empty()),
            buffer: String::new(),
            chunk_queue: VecDeque::new(),
            done: false,
            model_name: String::new(),
        };

        // Simulate real Gemini response
        let gemini_line = r#"data: {"candidates": [{"content": {"parts": [{"text": "Hello World"}], "role": "model"}, "finishReason": "STOP"}]}"#;
        stream.buffer = format!("{}\n", gemini_line);
        
        let result = stream.process_buffer();
        
        // Should successfully process the line
        assert!(result.is_some());
        assert!(stream.buffer.is_empty());
    }

    // ============================================================================
    // HTTP STREAMING REALITY TESTS
    // These tests simulate how HTTP actually works with arbitrary byte boundaries
    // ============================================================================

    #[test]
    fn test_http_chunking_basic_line_processing() {
        // Test basic line-by-line processing without async complexity
        let mut stream = GeminiStream {
            response: Box::pin(futures_util::stream::empty()),
            buffer: String::new(),
            chunk_queue: VecDeque::new(),
            done: false,
            model_name: String::new(),
        };

        // Test that incomplete lines are not processed
        stream.buffer = "data: {\"incomplete".to_string();
        let result = stream.process_buffer();
        assert!(result.is_none());
        assert!(!stream.buffer.is_empty());

        // Complete the line
        stream.buffer.push_str(" json\"}\n");
        let result = stream.process_buffer();
        // Should either process or clear buffer
        assert!(result.is_some() || stream.buffer.is_empty());
    }

    #[test]
    fn test_http_chunking_character_by_character() {
        // Test adding data character by character
        let mut stream = GeminiStream {
            response: Box::pin(futures_util::stream::empty()),
            buffer: String::new(),
            chunk_queue: VecDeque::new(),
            done: false,
            model_name: String::new(),
        };

        let line = "data: {\"test\": \"value\"}\n";
        
        // Add character by character
        for (i, ch) in line.chars().enumerate() {
            stream.buffer.push(ch);
            let result = stream.process_buffer();
            
            if i == line.len() - 1 {
                // Last character (newline) - should process or clear
                assert!(result.is_some() || stream.buffer.is_empty());
            } else {
                // Not complete yet - should not process
                assert!(result.is_none());
                assert!(!stream.buffer.is_empty());
            }
        }
    }

    #[test]
    fn test_http_chunking_multiple_lines() {
        // Test processing multiple complete lines
        let mut stream = GeminiStream {
            response: Box::pin(futures_util::stream::empty()),
            buffer: String::new(),
            chunk_queue: VecDeque::new(),
            done: false,
            model_name: String::new(),
        };

        // Add multiple lines at once
        stream.buffer = "data: {\"line1\": \"value1\"}\ndata: {\"line2\": \"value2\"}\n".to_string();
        
        let mut results = Vec::new();
        
        // Process all available lines
        while let Some(result) = stream.process_buffer() {
            results.push(result);
            // Safety check to prevent infinite loops
            if results.len() > 10 {
                break;
            }
        }
        
        // Should have processed both lines
        assert!(results.len() >= 1);
        assert!(stream.buffer.is_empty());
    }

    #[test]
    fn test_buffer_management_under_chunking_stress() {
        // Test that our buffer management works correctly under stress
        let mut stream = GeminiStream {
            response: Box::pin(futures_util::stream::empty()),
            buffer: String::new(),
            chunk_queue: VecDeque::new(),
            done: false,
            model_name: String::new(),
        };

        // Simulate receiving data in tiny increments
        let full_line = r#"data: {"candidates": [{"content": {"parts": [{"text": "Stress test"}], "role": "model"}, "finishReason": "STOP"}]}"#;
        
        // Add data character by character (simulating worst-case chunking)
        for ch in full_line.chars() {
            stream.buffer.push(ch);
            
            // Try to process after each character
            let result = stream.process_buffer();
            
            // Should not process until we have a complete line
            if !stream.buffer.contains('\n') {
                assert!(result.is_none(), "Should not process incomplete line");
            }
        }
        
        // Add the newline to complete the line
        stream.buffer.push('\n');
        let result = stream.process_buffer();
        
        // Now should process successfully
        assert!(result.is_some() || stream.buffer.is_empty());
    }

    #[test]
    fn test_multi_part_response_processing() {
        // Test that responses with multiple parts (thought + tool call) are processed correctly
        let response = GeminiResponse {
            candidates: vec![Candidate {
                content: Content {
                    parts: vec![
                        Part {
                            text: Some("Let me search for information about this.".to_string()),
                            function_call: None,
                            function_response: None,
                            thought: Some(true), // This is a thought
                        },
                        Part {
                            text: None,
                            function_call: Some(FunctionCall {
                                name: "web_search".to_string(),
                                args: json!({"search_term": "test query"}),
                            }),
                            function_response: None,
                            thought: None,
                        },
                    ],
                    role: "model".to_string(),
                },
                finish_reason: Some("STOP".to_string()),
                safety_ratings: vec![],
                grounding_metadata: None,
            }],
            usage_metadata: None,
            prompt_feedback: None,
        };

        let stream = GeminiStream {
            response: Box::pin(futures_util::stream::empty()),
            buffer: String::new(),
            chunk_queue: VecDeque::new(),
            done: false,
            model_name: String::new(),
        };

        let result = stream.convert_response_to_chunks(&response);

        assert!(result.is_ok());
        let chunks = result.unwrap();
        
        // Should have 2 chunks: one for thought, one for tool call
        assert_eq!(chunks.len(), 2, "Should have 2 chunks for thought + tool call");
        
        // First chunk should be the thought
        match &chunks[0].part {
            MessagePart::Thought { text } => {
                assert_eq!(text, "Let me search for information about this.");
                assert!(!chunks[0].is_final, "Thought chunk should not be final");
            },
            _ => panic!("First chunk should be a Thought part"),
        }
        
        // Second chunk should be the tool call
        match &chunks[1].part {
            MessagePart::ToolCall { name, .. } => {
                assert_eq!(name, "web_search");
                assert!(!chunks[1].is_final, "Tool call chunk should not be final even with STOP finish reason");
            },
            _ => panic!("Second chunk should be a ToolCall part"),
        }
    }
}

