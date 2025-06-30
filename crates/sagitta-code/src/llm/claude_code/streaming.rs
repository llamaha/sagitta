use std::io::{BufReader, BufRead, Read};
use std::process::Child;
use futures_util::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::task;
use tokio::sync::mpsc;
use crate::llm::client::{StreamChunk, MessagePart, TokenUsage};
use crate::utils::errors::SagittaCodeError;
use super::error::ClaudeCodeError;
use super::message_converter::{ClaudeChunk, ContentBlock};
use serde_json::Deserializer;

/// Stream implementation for Claude Code output
pub struct ClaudeCodeStream {
    receiver: mpsc::UnboundedReceiver<Result<StreamChunk, SagittaCodeError>>,
}

impl ClaudeCodeStream {
    /// Create a new stream from a Claude process
    pub fn new(mut child: Child) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        // Spawn a task to read from the process
        task::spawn_blocking(move || {
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            
            if let Some(mut stdout) = stdout {
                let mut buffer = [0u8; 16384]; // Larger buffer for better performance
                let mut json_buffer = Vec::new();
                let mut usage = TokenUsage::default();
                let mut is_paid_usage = true;
                
                // Collect stderr in a shared buffer
                let stderr_buffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                let stderr_buffer_clone = stderr_buffer.clone();
                
                if let Some(stderr) = stderr {
                    std::thread::spawn(move || {
                        let stderr_reader = BufReader::new(stderr);
                        for line in stderr_reader.lines() {
                            if let Ok(line) = line {
                                // Only log stderr for debugging
                                log::debug!("CLAUDE_CODE stderr: {}", line);
                                if let Ok(mut buffer) = stderr_buffer_clone.lock() {
                                    buffer.push(line);
                                }
                            }
                        }
                    });
                }
                
                // Don't delay - start processing immediately
                
                log::debug!("CLAUDE_CODE: Starting to read from stdout");
                
                // Check if process already exited
                if let Ok(Some(status)) = child.try_wait() {
                    if !status.success() {
                        let code = status.code().unwrap_or(-1);
                        let stderr_content = if let Ok(buffer) = stderr_buffer.lock() {
                            buffer.join("\n")
                        } else {
                            String::new()
                        };
                        
                        log::error!("CLAUDE_CODE: Process failed immediately with code: {}", code);
                        if !stderr_content.is_empty() {
                            log::error!("CLAUDE_CODE: Early stderr:\n{}", stderr_content);
                            let _ = sender.send(Err(SagittaCodeError::LlmError(
                                format!("Claude process failed to start: {}", stderr_content)
                            )));
                        } else {
                            let _ = sender.send(Err(SagittaCodeError::LlmError(
                                format!("Claude process failed to start with code: {}", code)
                            )));
                        }
                        return;
                    }
                }
                
                // Process bytes as they arrive for real-time streaming
                loop {
                    match stdout.read(&mut buffer) {
                        Ok(0) => {
                            // EOF reached
                            log::debug!("CLAUDE_CODE: Reached end of stdout");
                            break;
                        }
                        Ok(n) => {
                            // Append new bytes to buffer
                            json_buffer.extend_from_slice(&buffer[..n]);
                            // log::trace!("CLAUDE_CODE: Read {} bytes, total buffer size: {}", n, json_buffer.len());
                            
                            // Create a streaming deserializer from the buffer
                            let deserializer = Deserializer::from_slice(&json_buffer).into_iter::<ClaudeChunk>();
                            let mut bytes_consumed = 0;
                            
                            // Process all complete JSON objects in the buffer
                            for (idx, result) in deserializer.enumerate() {
                                match result {
                                    Ok(chunk) => {
                                        // Track how many bytes this chunk consumed by looking for newline
                                        // JSON chunks are typically newline-delimited
                                        bytes_consumed = json_buffer.iter().position(|&b| b == b'\n')
                                            .map(|p| p + 1)
                                            .unwrap_or(json_buffer.len());
                                        
                                        // Only log important chunks
                                        if matches!(&chunk, ClaudeChunk::Result { .. }) {
                                            log::debug!("CLAUDE_CODE: Received result chunk");
                                        }
                                    
                                    match chunk {
                                        ClaudeChunk::System { subtype, api_key_source } => {
                                            if subtype == "init" {
                                                // Subscription usage sets api_key_source to "none"
                                                is_paid_usage = api_key_source.as_deref() != Some("none");
                                                log::debug!("CLAUDE_CODE: Paid usage: {}", is_paid_usage);
                                            }
                                        }
                                        ClaudeChunk::Assistant { message } => {
                                            // Check if we have any non-empty text content
                                            let has_text_content = message.content.iter().any(|content| {
                                                matches!(content, ContentBlock::Text { text } if !text.trim().is_empty())
                                            });
                                            
                                            // Process content blocks directly
                                            for (idx, content) in message.content.into_iter().enumerate() {
                                                // Process content blocks without logging
                                                match content {
                                                    ContentBlock::Text { text } => {
                                                        // Send text directly without logging
                                                        
                                                        // Send text directly
                                                        let _ = sender.send(Ok(StreamChunk {
                                                            part: MessagePart::Text { text },
                                                            is_final: false,
                                                            finish_reason: None,
                                                            token_usage: None,
                                                        }));
                                                    }
                                                    ContentBlock::Thinking { thinking } => {
                                                        // Only send thinking if there's no text content and thinking is not empty
                                                        if !has_text_content && !thinking.trim().is_empty() {
                                                            log::info!("CLAUDE_CODE: Sending Thinking block as main content (length: {})", thinking.len());
                                                            let _ = sender.send(Ok(StreamChunk {
                                                                part: MessagePart::Thought { text: thinking },
                                                                is_final: false,
                                                                finish_reason: None,
                                                                token_usage: None,
                                                            }));
                                                        } else {
                                                            log::info!("CLAUDE_CODE: Skipping Thinking block (has_text: {}, length: {})", has_text_content, thinking.len());
                                                        }
                                                    }
                                                    ContentBlock::RedactedThinking => {
                                                        log::info!("CLAUDE_CODE: Skipping RedactedThinking block");
                                                        // Skip redacted thinking blocks as well
                                                    }
                                                    ContentBlock::ToolUse { id, name, input } => {
                                                        // Tools are now handled through MCP
                                                        log::debug!("CLAUDE_CODE: Tool use '{}' should be handled via MCP", name);
                                                        
                                                        // Send tool call part for MCP tools
                                                        let _ = sender.send(Ok(StreamChunk {
                                                            part: MessagePart::ToolCall {
                                                                tool_call_id: id,
                                                                name,
                                                                parameters: input,
                                                            },
                                                            is_final: false,
                                                            finish_reason: None,
                                                            token_usage: None,
                                                        }));
                                                    }
                                                }
                                            }
                                            
                                            // Update usage
                                            usage.prompt_tokens += message.usage.input_tokens;
                                            usage.completion_tokens += message.usage.output_tokens;
                                            usage.cached_tokens = message.usage.cache_read_input_tokens;
                                            
                                            // Don't send is_final here - wait for Result chunk
                                            // This allows tool calls to be processed as part of the same message
                                        }
                                        ClaudeChunk::Result { result, total_cost_usd, subtype, is_error } => {
                                            // Check for error results
                                            if subtype.as_deref() == Some("error_max_turns") {
                                                log::warn!("CLAUDE_CODE: Received error_max_turns result");
                                            }
                                            
                                            // Extract model name if result is an object with model field
                                            if let Some(result_value) = result {
                                                if let Some(model) = result_value.as_object()
                                                    .and_then(|obj| obj.get("model"))
                                                    .and_then(|v| v.as_str()) {
                                                    usage.model_name = model.to_string();
                                                }
                                            }
                                            
                                            // Final usage update
                                            usage.total_tokens = usage.prompt_tokens + usage.completion_tokens;
                                            
                                            // Only include cost if paid usage
                                            if !is_paid_usage {
                                                log::debug!("CLAUDE_CODE: Not including cost for subscription usage");
                                            }
                                            
                                            let _ = sender.send(Ok(StreamChunk {
                                                part: MessagePart::Text { text: String::new() },
                                                is_final: true,
                                                finish_reason: Some("stop".to_string()),
                                                token_usage: Some(usage.clone()),
                                            }));
                                        }
                                        ClaudeChunk::User { message } => {
                                            // Handle user messages
                                            log::debug!("CLAUDE_CODE: Received user message with {} content blocks", message.content.len());
                                            
                                            // Process user content blocks
                                            for content in message.content {
                                                match content {
                                                    super::message_converter::UserContentBlock::Text { text } => {
                                                        log::debug!("CLAUDE_CODE: User text: {}", text);
                                                    }
                                                    super::message_converter::UserContentBlock::ToolResult { content, tool_use_id, is_error } => {
                                                        // Tool results from MCP - store for tool preview instead of displaying inline
                                                        log::info!("CLAUDE_CODE: Tool result for {} (is_error: {:?}): {}", tool_use_id, is_error, content);
                                                        
                                                        // Send tool result regardless of error status
                                                        let result_value = if is_error == Some(true) {
                                                            // Wrap error content in an error object
                                                            serde_json::json!({
                                                                "error": content,
                                                                "is_error": true
                                                            })
                                                        } else {
                                                            // Parse normal result
                                                            serde_json::from_str(&content).unwrap_or(serde_json::Value::String(content))
                                                        };
                                                        
                                                        let _ = sender.send(Ok(StreamChunk {
                                                            part: MessagePart::ToolResult { 
                                                                tool_call_id: tool_use_id.clone(),
                                                                name: "mcp_tool".to_string(),
                                                                result: result_value,
                                                            },
                                                            is_final: false,
                                                            finish_reason: None,
                                                            token_usage: None,
                                                        }));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    }
                                    Err(e) => {
                                        if e.is_eof() {
                                            // Need more data - this is normal for streaming
                                            log::trace!("CLAUDE_CODE: Need more data for complete JSON object");
                                        } else {
                                            // Actual parse error
                                            log::error!("CLAUDE_CODE: JSON parse error: {}", e);
                                            // Log the problematic JSON for debugging
                                            if let Ok(json_str) = std::str::from_utf8(&json_buffer) {
                                                log::error!("CLAUDE_CODE: Problematic JSON (first 500 chars): {}", 
                                                    &json_str[..json_str.len().min(500)]);
                                            }
                                            // Skip to next newline to recover
                                            bytes_consumed = json_buffer.iter()
                                                .position(|&b| b == b'\n')
                                                .map(|p| p + 1)
                                                .unwrap_or(json_buffer.len());
                                        }
                                        break; // Exit the deserialization loop
                                    }
                                }
                            }
                            
                            // Remove consumed bytes from buffer
                            if bytes_consumed > 0 {
                                json_buffer.drain(..bytes_consumed);
                                log::trace!("CLAUDE_CODE: Removed {} consumed bytes, {} bytes remaining", bytes_consumed, json_buffer.len());
                            }
                        }
                        Err(e) => {
                            log::error!("CLAUDE_CODE: Error reading bytes: {}", e);
                            let _ = sender.send(Err(SagittaCodeError::LlmError(
                                format!("Error reading claude output: {}", e)
                            )));
                            break;
                        }
                    }
                }
                
                // Handle any remaining data in buffer
                if !json_buffer.is_empty() {
                    log::warn!("CLAUDE_CODE: {} bytes of unprocessed data at end", json_buffer.len());
                    if let Ok(s) = std::str::from_utf8(&json_buffer) {
                        log::warn!("CLAUDE_CODE: Unprocessed data: {}", s);
                    }
                }
                
                // Wait for process to exit
                match child.wait() {
                    Ok(status) => {
                        if !status.success() {
                            let code = status.code().unwrap_or(-1);
                            log::error!("CLAUDE_CODE: Process exited with code: {}", code);
                            
                            // Get stderr content
                            let stderr_content = if let Ok(buffer) = stderr_buffer.lock() {
                                buffer.join("\n")
                            } else {
                                String::new()
                            };
                            
                            if !stderr_content.is_empty() {
                                log::error!("CLAUDE_CODE: Process stderr:\n{}", stderr_content);
                                let _ = sender.send(Err(SagittaCodeError::LlmError(
                                    format!("Claude process failed: {}", stderr_content)
                                )));
                            } else {
                                let _ = sender.send(Err(SagittaCodeError::LlmError(
                                    format!("Claude process exited with code: {}", code)
                                )));
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("CLAUDE_CODE: Error waiting for process: {}", e);
                        let _ = sender.send(Err(SagittaCodeError::LlmError(
                            format!("Error waiting for claude process: {}", e)
                        )));
                    }
                }
            }
        });
        
        Self { receiver }
    }
}

impl Stream for ClaudeCodeStream {
    type Item = Result<StreamChunk, SagittaCodeError>;
    
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}