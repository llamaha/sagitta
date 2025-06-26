use std::io::{BufReader, BufRead};
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
use uuid::Uuid;

/// Stream implementation for Claude Code output
pub struct ClaudeCodeStream {
    receiver: mpsc::UnboundedReceiver<Result<StreamChunk, SagittaCodeError>>,
}

/// Parse tool calls from text containing XML tags
pub fn parse_tool_calls_from_text(text: &str) -> (String, Vec<MessagePart>) {
    let mut remaining_text = String::new();
    let mut tool_calls = Vec::new();
    let mut current_pos = 0;
    
    // Find all tool use blocks
    while let Some(start) = text[current_pos..].find("<tool_use>") {
        let start = current_pos + start;
        
        // Add any text before this tool call
        if start > current_pos {
            remaining_text.push_str(&text[current_pos..start]);
        }
        
        // Find the end of this tool use block
        if let Some(end) = text[start..].find("</tool_use>") {
            let end = start + end + "</tool_use>".len();
            let tool_block = &text[start..end];
            
            // Parse the tool name and parameters
            if let Some(tool_name_start) = tool_block.find("<tool_name>") {
                if let Some(tool_name_end) = tool_block.find("</tool_name>") {
                    let name_start = tool_name_start + "<tool_name>".len();
                    let tool_name = tool_block[name_start..tool_name_end].trim().to_string();
                    
                    // Parse parameters
                    let mut params = serde_json::Map::new();
                    if let Some(params_start) = tool_block.find("<parameters>") {
                        if let Some(params_end) = tool_block.find("</parameters>") {
                            let params_start = params_start + "<parameters>".len();
                            let params_block = &tool_block[params_start..params_end];
                            
                            // Simple XML parameter parsing
                            let lines = params_block.lines();
                            for line in lines {
                                let line = line.trim();
                                if line.starts_with('<') && line.contains('>') {
                                    // Extract tag name and value
                                    if let Some(tag_end) = line.find('>') {
                                        let tag = &line[1..tag_end];
                                        if !tag.starts_with('/') && tag.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                            // Find the closing tag
                                            let close_tag = format!("</{}>", tag);
                                            if let Some(close_pos) = line.find(&close_tag) {
                                                let value_start = tag_end + 1;
                                                let value = &line[value_start..close_pos];
                                                
                                                // Remove comments if present
                                                let value = if let Some(comment_pos) = value.find("<!--") {
                                                    value[..comment_pos].trim()
                                                } else {
                                                    value.trim()
                                                };
                                                
                                                // Try to parse as JSON value
                                                let json_value = if value == "true" || value == "false" {
                                                    serde_json::Value::Bool(value == "true")
                                                } else if let Ok(num) = value.parse::<i64>() {
                                                    serde_json::Value::Number(num.into())
                                                } else if let Ok(num) = value.parse::<f64>() {
                                                    serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap_or(0.into()))
                                                } else if value.starts_with('[') || value.starts_with('{') {
                                                    serde_json::from_str(value).unwrap_or(serde_json::Value::String(value.to_string()))
                                                } else {
                                                    serde_json::Value::String(value.to_string())
                                                };
                                                
                                                params.insert(tag.to_string(), json_value);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    let tool_call_id = Uuid::new_v4().to_string();
                    tool_calls.push(MessagePart::ToolCall {
                        tool_call_id,
                        name: tool_name,
                        parameters: serde_json::Value::Object(params),
                    });
                }
            }
            
            current_pos = end;
        } else {
            // Incomplete tool block, include it as text
            remaining_text.push_str(&text[start..]);
            break;
        }
    }
    
    // Add any remaining text
    if current_pos < text.len() {
        remaining_text.push_str(&text[current_pos..]);
    }
    
    (remaining_text.trim().to_string(), tool_calls)
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
                
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            log::trace!("CLAUDE_CODE: Raw line: {}", line);
                            
                            if line.trim().is_empty() {
                                continue;
                            }
                            
                            // Handle partial JSON
                            let json_str = if !partial_data.is_empty() {
                                partial_data.push_str(&line);
                                let complete = partial_data.clone();
                                partial_data.clear();
                                complete
                            } else {
                                line.clone()
                            };
                            
                            // Try to parse the JSON
                            match serde_json::from_str::<ClaudeChunk>(&json_str) {
                                Ok(chunk) => {
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
                                            // Process content blocks
                                            for content in message.content {
                                                match content {
                                                    ContentBlock::Text { text } => {
                                                        // Parse tool calls from the text
                                                        let (remaining_text, tool_calls) = parse_tool_calls_from_text(&text);
                                                        
                                                        // Send any remaining text first
                                                        if !remaining_text.is_empty() {
                                                            let _ = sender.send(Ok(StreamChunk {
                                                                part: MessagePart::Text { text: remaining_text },
                                                                is_final: false,
                                                                finish_reason: None,
                                                                token_usage: None,
                                                            }));
                                                        }
                                                        
                                                        // Send tool calls
                                                        for tool_call in tool_calls {
                                                            let _ = sender.send(Ok(StreamChunk {
                                                                part: tool_call,
                                                                is_final: false,
                                                                finish_reason: None,
                                                                token_usage: None,
                                                            }));
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
                                        ClaudeChunk::Result { result, total_cost_usd } => {
                                            // Extract model name if result is an object with model field
                                            if let Some(model) = result.as_object()
                                                .and_then(|obj| obj.get("model"))
                                                .and_then(|v| v.as_str()) {
                                                usage.model_name = model.to_string();
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
                                    }
                                }
                                Err(e) => {
                                    // Check if it's a partial JSON
                                    if json_str.ends_with('}') || json_str.ends_with(']') {
                                        // Complete JSON that failed to parse
                                        log::error!("CLAUDE_CODE: Failed to parse complete JSON: {} - Error: {}", json_str, e);
                                        
                                        // Check for error messages
                                        if json_str.contains("API Error") {
                                            let _ = sender.send(Err(SagittaCodeError::LlmError(
                                                format!("Claude API Error: {}", json_str)
                                            )));
                                        }
                                    } else {
                                        // Partial JSON, save for next iteration
                                        log::trace!("CLAUDE_CODE: Saving partial data: {}", json_str);
                                        partial_data = json_str;
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