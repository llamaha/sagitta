use std::io::{BufReader, BufRead};
use std::process::Child;
use futures_util::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::task;
use tokio::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::llm::client::{StreamChunk, MessagePart, TokenUsage};
use crate::utils::errors::SagittaCodeError;
use super::error::ClaudeCodeError;
use super::message_converter::{ClaudeChunk, ContentBlock};
use super::xml_tools::parse_xml_tool_calls;

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
            
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                let mut partial_data = String::new();
                let mut usage = TokenUsage::default();
                let mut is_paid_usage = true;
                let tool_emitted = Arc::new(AtomicBool::new(false));
                
                // Collect stderr in a shared buffer
                let stderr_buffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                let stderr_buffer_clone = stderr_buffer.clone();
                
                if let Some(stderr) = stderr {
                    std::thread::spawn(move || {
                        let stderr_reader = BufReader::new(stderr);
                        for line in stderr_reader.lines() {
                            if let Ok(line) = line {
                                log::error!("CLAUDE_CODE stderr: {}", line);
                                if let Ok(mut buffer) = stderr_buffer_clone.lock() {
                                    buffer.push(line);
                                }
                            }
                        }
                    });
                }
                
                // Give stderr thread a moment to capture early errors
                std::thread::sleep(std::time::Duration::from_millis(100));
                
                log::info!("CLAUDE_CODE: Starting to read from stdout");
                
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
                
                let mut line_count = 0;
                
                // Process lines as they arrive, similar to Roo-Code's readline approach
                for line_result in reader.lines() {
                    match line_result {
                        Ok(line) => {
                            line_count += 1;
                            
                            // Skip empty lines
                            if line.trim().is_empty() {
                                continue;
                            }
                            
                            log::info!("CLAUDE_CODE: Received line {}: {}", line_count, line.chars().take(100).collect::<String>());
                            log::trace!("CLAUDE_CODE: Raw line: {}", line);
                            
                            // Accumulate partial JSON data between lines
                            partial_data.push_str(&line);
                            
                            // Try to parse complete JSON objects
                            let json_str = partial_data.trim();
                            
                            // Try to parse the JSON - if successful, clear partial data
                            match serde_json::from_str::<ClaudeChunk>(&json_str) {
                                Ok(chunk) => {
                                    // Successfully parsed, clear accumulated data
                                    partial_data.clear();
                                    log::info!("CLAUDE_CODE: Parsed chunk type: {}", match &chunk {
                                        ClaudeChunk::System { .. } => "System",
                                        ClaudeChunk::Assistant { .. } => "Assistant",
                                        ClaudeChunk::Result { .. } => "Result", 
                                        ClaudeChunk::User { .. } => "User",
                                    });
                                    log::debug!("CLAUDE_CODE: Parsed chunk: {:?}", chunk);
                                    
                                    match chunk {
                                        ClaudeChunk::System { subtype, api_key_source } => {
                                            if subtype == "init" {
                                                // Subscription usage sets api_key_source to "none"
                                                is_paid_usage = api_key_source.as_deref() != Some("none");
                                                log::debug!("CLAUDE_CODE: Paid usage: {}", is_paid_usage);
                                            }
                                        }
                                        ClaudeChunk::Assistant { message } => {
                                            log::info!("CLAUDE_CODE: Received Assistant message with {} content blocks", message.content.len());
                                            // Process content blocks
                                            log::debug!("CLAUDE_CODE: Processing {} content blocks", message.content.len());
                                            for (idx, content) in message.content.into_iter().enumerate() {
                                                log::info!("CLAUDE_CODE: Processing content block {}: type = {:?}", idx, match &content {
                                                    ContentBlock::Text { .. } => "Text",
                                                    ContentBlock::ToolUse { .. } => "ToolUse",
                                                    ContentBlock::Thinking { .. } => "Thinking",
                                                    ContentBlock::RedactedThinking => "RedactedThinking",
                                                });
                                                match content {
                                                    ContentBlock::Text { text } => {
                                                        log::info!("CLAUDE_CODE: Processing Text block with length {}: {}", text.len(), if text.len() < 200 { &text } else { &text[..200] });
                                                        // Also log if text contains XML-like content
                                                        if text.contains("<") && text.contains(">") {
                                                            log::info!("CLAUDE_CODE: Text contains XML-like content");
                                                            // Find and log the XML part
                                                            if let Some(xml_start) = text.find("<") {
                                                                let preview = &text[xml_start..std::cmp::min(xml_start + 100, text.len())];
                                                                log::info!("CLAUDE_CODE: XML preview starting at position {}: {}", xml_start, preview);
                                                            }
                                                        }
                                                        // Parse XML tool calls from the text
                                                        let (remaining_text, tool_calls) = parse_xml_tool_calls(&text);
                                                        log::info!("CLAUDE_CODE: After XML parsing - found {} tool calls", tool_calls.len());
                                                        if tool_calls.is_empty() && text.contains("<") && text.contains(">") {
                                                            log::warn!("CLAUDE_CODE: Text contained XML but no tool calls were parsed!");
                                                        }
                                                        
                                                        // Send any remaining text first
                                                        if !remaining_text.is_empty() {
                                                            let _ = sender.send(Ok(StreamChunk {
                                                                part: MessagePart::Text { text: remaining_text },
                                                                is_final: false,
                                                                finish_reason: None,
                                                                token_usage: None,
                                                            }));
                                                        }
                                                        
                                                        // Send tool calls with enforcement
                                                        for tool_call in tool_calls {
                                                            // Check if we've already emitted a tool
                                                            if !tool_emitted.load(Ordering::Relaxed) {
                                                                // First tool - allow it
                                                                tool_emitted.store(true, Ordering::Relaxed);
                                                                let _ = sender.send(Ok(StreamChunk {
                                                                    part: tool_call,
                                                                    is_final: false,
                                                                    finish_reason: None,
                                                                    token_usage: None,
                                                                }));
                                                            } else {
                                                                // Additional tool - log warning and skip
                                                                if let MessagePart::ToolCall { name, .. } = &tool_call {
                                                                    log::warn!("CLAUDE_CODE: Skipping additional tool call '{}' - only one tool per response allowed", name);
                                                                }
                                                            }
                                                        }
                                                    }
                                                    ContentBlock::Thinking { thinking } => {
                                                        let _ = sender.send(Ok(StreamChunk {
                                                            part: MessagePart::Thought { text: thinking },
                                                            is_final: false,
                                                            finish_reason: None,
                                                            token_usage: None,
                                                        }));
                                                    }
                                                    ContentBlock::RedactedThinking => {
                                                        let _ = sender.send(Ok(StreamChunk {
                                                            part: MessagePart::Thought { text: "[Redacted thinking]".to_string() },
                                                            is_final: false,
                                                            finish_reason: None,
                                                            token_usage: None,
                                                        }));
                                                    }
                                                    ContentBlock::ToolUse { id, name, input } => {
                                                        // Handle tool use content blocks
                                                        if !tool_emitted.load(Ordering::Relaxed) {
                                                            tool_emitted.store(true, Ordering::Relaxed);
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
                                                        } else {
                                                            log::warn!("CLAUDE_CODE: Skipping additional tool_use '{}' - only one tool per response allowed", name);
                                                        }
                                                    }
                                                }
                                            }
                                            
                                            // Update usage
                                            usage.prompt_tokens += message.usage.input_tokens;
                                            usage.completion_tokens += message.usage.output_tokens;
                                            usage.cached_tokens = message.usage.cache_read_input_tokens;
                                            
                                            // Check if this is the final message
                                            if message.stop_reason.is_some() {
                                                let _ = sender.send(Ok(StreamChunk {
                                                    part: MessagePart::Text { text: String::new() },
                                                    is_final: true,
                                                    finish_reason: message.stop_reason,
                                                    token_usage: Some(usage.clone()),
                                                }));
                                            }
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
                                            // Handle user messages (tool results, etc.)
                                            log::debug!("CLAUDE_CODE: Received user message with {} content blocks", message.content.len());
                                            
                                            // Process user content blocks
                                            for content in message.content {
                                                match content {
                                                    super::message_converter::UserContentBlock::Text { text } => {
                                                        log::debug!("CLAUDE_CODE: User text: {}", text);
                                                    }
                                                    super::message_converter::UserContentBlock::ToolResult { content, tool_use_id, is_error } => {
                                                        // Check for permission denial messages
                                                        if content.contains("Permission to use") && content.contains("has been denied") {
                                                            log::warn!("CLAUDE_CODE: MCP tool permission denied: {}", content);
                                                            // Don't propagate permission errors as API errors
                                                            // Instead, send as a regular tool result
                                                        } else {
                                                            log::debug!("CLAUDE_CODE: Tool result for {}: {} (error: {:?})", tool_use_id, content, is_error);
                                                        }
                                                        
                                                        // Send tool result as a message part
                                                        let _ = sender.send(Ok(StreamChunk {
                                                            part: MessagePart::ToolResult {
                                                                tool_call_id: tool_use_id,
                                                                name: "".to_string(), // Name not provided in user messages
                                                                result: serde_json::json!({
                                                                    "content": content,
                                                                    "is_error": is_error.unwrap_or(false)
                                                                }),
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
                                    // Check if it looks like a complete JSON object that failed to parse
                                    let looks_complete = json_str.trim_start().starts_with('{') && 
                                                        json_str.trim_end().ends_with('}');
                                    
                                    if looks_complete {
                                        // Complete JSON that failed to parse - this is an error
                                        log::error!("CLAUDE_CODE: Failed to parse complete JSON: {} - Error: {}", json_str.chars().take(200).collect::<String>(), e);
                                        
                                        // Check for error messages (but ignore permission denials)
                                        if json_str.contains("Permission to use") && json_str.contains("has been denied") {
                                            log::debug!("CLAUDE_CODE: Ignoring MCP permission denial in error handling");
                                        } else if json_str.contains("API Error") || json_str.contains("error") {
                                            let _ = sender.send(Err(SagittaCodeError::LlmError(
                                                format!("Claude API Error: {}", json_str)
                                            )));
                                        }
                                        
                                        // Clear partial data to avoid carrying forward bad data
                                        partial_data.clear();
                                    } else {
                                        // Partial JSON - keep accumulating
                                        log::trace!("CLAUDE_CODE: Accumulating partial JSON data (length: {})", partial_data.len());
                                        // partial_data already contains the accumulated data, so we don't need to do anything
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("CLAUDE_CODE: Error reading line: {}", e);
                            let _ = sender.send(Err(SagittaCodeError::LlmError(
                                format!("Error reading claude output: {}", e)
                            )));
                        }
                    }
                }
                
                // Handle any remaining partial data
                if !partial_data.is_empty() {
                    log::warn!("CLAUDE_CODE: Unprocessed partial data at end: {}", partial_data);
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