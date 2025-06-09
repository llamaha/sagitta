use std::sync::Arc;
use uuid::Uuid;
use std::collections::HashMap;
use terminal_stream::events::StreamEvent;

pub mod config;
pub mod llm_adapter;
pub mod intent_analyzer;

use async_trait::async_trait;
use reasoning_engine::{
    traits::{EventEmitter, StreamHandler, ToolExecutor, StatePersistence, MetricsCollector},
    ReasoningEvent, ToolResult,
    ReasoningError,
    Result as ReasoningEngineResult,
    state::ReasoningState,
    streaming::StreamChunk,
};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

use crate::{agent::events::AgentEvent, tools::{registry::ToolRegistry, types::Tool}, llm::client::{MessagePart, StreamChunk as SagittaCodeStreamChunk}, utils::errors::SagittaCodeError};

#[derive(Clone)]
pub struct AgentToolExecutor {
    tool_registry: Arc<ToolRegistry>,
    terminal_event_sender: Option<tokio::sync::mpsc::Sender<StreamEvent>>,
    /// Phase 1 Fix: Event sender for feedback to LLM
    event_sender: Option<broadcast::Sender<AgentEvent>>,
    /// Phase 1 Fix: Loop detection - track tool call history
    recent_tool_calls: Arc<tokio::sync::Mutex<Vec<(String, Value, std::time::Instant)>>>,
    /// Phase 1 Fix: Maximum identical tool calls before triggering loop detection
    max_identical_calls: usize,
}

impl AgentToolExecutor {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { 
            tool_registry,
            terminal_event_sender: None,
            event_sender: None,
            recent_tool_calls: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            max_identical_calls: 3,
        }
    }
    
    /// Set the terminal event sender for streaming shell execution
    pub fn set_terminal_event_sender(&mut self, sender: tokio::sync::mpsc::Sender<StreamEvent>) {
        self.terminal_event_sender = Some(sender);
    }
    
    /// Phase 1 Fix: Set the event sender for LLM feedback
    pub fn set_event_sender(&mut self, sender: broadcast::Sender<AgentEvent>) {
        self.event_sender = Some(sender);
    }
    
    /// Phase 1 Fix: Check for infinite loops in tool calling
    async fn check_loop_detection(&self, tool_name: &str, args: &Value) -> bool {
        let mut recent_calls = self.recent_tool_calls.lock().await;
        let now = std::time::Instant::now();
        let current_call = (tool_name.to_string(), args.clone(), now);
        
        // Clean up old calls (older than 30 seconds)
        recent_calls.retain(|(_, _, timestamp)| now.duration_since(*timestamp).as_secs() < 30);
        
        // Check for identical calls
        let identical_count = recent_calls
            .iter()
            .filter(|(name, call_args, _)| {
                name == tool_name && call_args == args
            })
            .count();
        
        // Add current call to history
        recent_calls.push(current_call);
        
        // Keep only recent calls (last 10) - fix borrow checker issue
        if recent_calls.len() > 10 {
            let len = recent_calls.len();
            recent_calls.drain(0..len - 10);
        }
        
        identical_count >= self.max_identical_calls
    }
    
    /// Phase 1 Fix: Surface error feedback to LLM via event system
    async fn surface_error_to_llm(&self, tool_name: &str, error_message: &str, error_code: &str) {
        if let Some(ref event_sender) = self.event_sender {
            // Create a helpful error message for the LLM
            let llm_feedback = match error_code {
                "invalid_parameters" => {
                    format!("⚠️ Tool '{}' failed: {}. Please check the required parameters and try again with the correct values.", tool_name, error_message)
                }
                "tool_not_found" => {
                    format!("⚠️ Tool '{}' not found. Please check the tool name and available tools.", tool_name)
                }
                "execution_error" => {
                    format!("⚠️ Tool '{}' execution failed: {}. Please analyze the error and try a different approach.", tool_name, error_message)
                }
                "loop_detected" => {
                    format!("🔄 Loop detected: Tool '{}' has been called repeatedly with the same parameters. Please try a different approach or ask for clarification.", tool_name)
                }
                _ => {
                    format!("❌ Tool '{}' error: {}", tool_name, error_message)
                }
            };
            
            // Send as both a streaming chunk and an event
            let _ = event_sender.send(AgentEvent::LlmChunk {
                content: format!("\n{}\n\n", llm_feedback),
                is_final: false,
            });
            
            let _ = event_sender.send(AgentEvent::Error(error_message.to_string()));
        }
    }
    
    /// Phase 1 Fix: Validate tool parameters against the tool's JSON schema
    async fn validate_tool_parameters(&self, tool_name: &str, args: &Value) -> Result<(), String> {
        // Get the tool definition to access its parameter schema
        let tool_definitions = self.tool_registry.get_definitions().await;
        let tool_def = tool_definitions
            .iter()
            .find(|def| def.name == tool_name)
            .ok_or_else(|| format!("Tool '{}' not found in registry", tool_name))?;

        // Check required fields
        if let Some(required_fields) = tool_def.parameters.get("required").and_then(|r| r.as_array()) {
            for required_field in required_fields {
                if let Some(field_name) = required_field.as_str() {
                    if !args.get(field_name).is_some() {
                        return Err(format!("Missing required parameter: '{}'", field_name));
                    }
                }
            }
        }

        // Check oneOf constraints (like add_repository requiring either url OR local_path)
        if let Some(one_of) = tool_def.parameters.get("oneOf").and_then(|o| o.as_array()) {
            let mut satisfied_constraint = false;
            
            for constraint in one_of {
                if let Some(required_in_constraint) = constraint.get("required").and_then(|r| r.as_array()) {
                    let all_fields_present = required_in_constraint
                        .iter()
                        .all(|field| {
                            field.as_str()
                                .map(|field_name| args.get(field_name).is_some())
                                .unwrap_or(false)
                        });
                    
                    if all_fields_present {
                        satisfied_constraint = true;
                        break;
                    }
                }
            }
            
            if !satisfied_constraint {
                // Build helpful error message for oneOf constraints
                let constraint_descriptions: Vec<String> = one_of
                    .iter()
                    .filter_map(|constraint| {
                        constraint.get("required")
                            .and_then(|r| r.as_array())
                            .map(|fields| {
                                let field_names: Vec<String> = fields
                                    .iter()
                                    .filter_map(|f| f.as_str().map(|s| s.to_string()))
                                    .collect();
                                field_names.join(" AND ")
                            })
                    })
                    .collect();
                
                return Err(format!(
                    "Must satisfy one of these parameter combinations: {}",
                    constraint_descriptions.join(" OR ")
                ));
            }
        }

        // TODO: Add more sophisticated schema validation here (types, formats, etc.)
        // For now, basic required fields and oneOf validation should catch most issues

        Ok(())
    }
    
    /// Execute shell command with streaming when terminal sender is available
    async fn execute_streaming_shell_tool(&self, name: &str, args: Value) -> ReasoningEngineResult<ToolResult> {
        // Get the streaming shell execution tool
        let streaming_tool = self
            .tool_registry
            .get("streaming_shell_execution")
            .await
            .ok_or_else(|| ReasoningError::tool_execution(name.to_string(), "Streaming shell execution tool not found".to_string()))?;

        // Parse shell execution parameters
        let shell_params: crate::tools::shell_execution::ShellExecutionParams = serde_json::from_value(args.clone())
            .map_err(|e| ReasoningError::tool_execution(name.to_string(), format!("Invalid shell execution parameters: {}", e)))?;

        // Cast to streaming shell tool
        let streaming_shell_tool = streaming_tool
            .as_any()
            .downcast_ref::<crate::tools::shell_execution::StreamingShellExecutionTool>()
            .ok_or_else(|| ReasoningError::tool_execution(name.to_string(), "Failed to cast to StreamingShellExecutionTool".to_string()))?;

        // Get terminal event sender
        let terminal_sender = self.terminal_event_sender.as_ref()
            .ok_or_else(|| ReasoningError::tool_execution(name.to_string(), "Terminal event sender not configured".to_string()))?;

        // Execute with streaming
        match streaming_shell_tool.execute_streaming(shell_params, terminal_sender.clone()).await {
            Ok(shell_result) => {
                let result_value = serde_json::to_value(shell_result)
                    .map_err(|e| ReasoningError::tool_execution(name.to_string(), format!("Failed to serialize shell result: {}", e)))?;
                
                // Post-process the result for better completion confidence
                let processed_result = self.post_process_shell_result(&result_value).unwrap_or(result_value);
                
                Ok(ToolResult {
                    success: true,
                    data: processed_result,
                    error: None,
                    execution_time_ms: 0,
                    metadata: HashMap::new(),
                })
            }
            Err(error) => {
                Err(ReasoningError::tool_execution(name.to_string(), error.to_string()))
            }
        }
    }
    
    /// Post-process shell execution results to provide better context and completion confidence
    fn post_process_shell_result(&self, result_value: &Value) -> Option<Value> {
        // Try to extract meaningful information from shell execution results
        if let Some(result_obj) = result_value.as_object() {
            if let (Some(stdout), Some(exit_code)) = (
                result_obj.get("stdout").and_then(|v| v.as_str()),
                result_obj.get("exit_code").and_then(|v| v.as_i64())
            ) {
                if exit_code == 0 && !stdout.trim().is_empty() {
                    // Try to detect common patterns and provide helpful summaries
                    let stdout_trimmed = stdout.trim();
                    
                    // Line counting commands (wc -l, find | wc -l, etc.)
                    if let Ok(line_count) = stdout_trimmed.parse::<u64>() {
                        if line_count > 0 {
                            let mut enhanced_result = result_obj.clone();
                            enhanced_result.insert(
                                "summary".to_string(),
                                Value::String(format!("Command completed successfully. Total count: {}", line_count))
                            );
                            return Some(Value::Object(enhanced_result));
                        }
                    }
                    
                    // Multi-line output with numbers (like detailed file counts)
                    if stdout_trimmed.lines().count() > 1 {
                        let lines: Vec<&str> = stdout_trimmed.lines().collect();
                        if let Some(last_line) = lines.last() {
                            if last_line.contains("total") || last_line.trim().parse::<u64>().is_ok() {
                                let mut enhanced_result = result_obj.clone();
                                enhanced_result.insert(
                                    "summary".to_string(),
                                    Value::String(format!("Command completed successfully. Result summary: {}", last_line.trim()))
                                );
                                return Some(Value::Object(enhanced_result));
                            }
                        }
                    }
                    
                    // For other successful commands, add a basic summary
                    if stdout_trimmed.len() < 200 {
                        let mut enhanced_result = result_obj.clone();
                        enhanced_result.insert(
                            "summary".to_string(),
                            Value::String(format!("Command completed successfully. Output: {}", stdout_trimmed))
                        );
                        return Some(Value::Object(enhanced_result));
                    } else {
                        let mut enhanced_result = result_obj.clone();
                        enhanced_result.insert(
                            "summary".to_string(),
                            Value::String(format!("Command completed successfully. Output length: {} characters", stdout_trimmed.len()))
                        );
                        return Some(Value::Object(enhanced_result));
                    }
                }
            }
        }
        
        None
    }
}

#[async_trait]
impl ToolExecutor for AgentToolExecutor {
    async fn execute_tool(&self, name: &str, args: Value) -> ReasoningEngineResult<ToolResult> {
        // Phase 1 Fix: Check for infinite loops first
        if self.check_loop_detection(name, &args).await {
            self.surface_error_to_llm(name, "Tool called repeatedly with same parameters", "loop_detected").await;
            return Ok(ToolResult {
                success: false,
                data: Value::Null,
                error: Some("Loop detected: this tool has been called repeatedly with the same parameters. Please try a different approach.".to_string()),
                execution_time_ms: 0,
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("error_code".to_string(), Value::String("loop_detected".to_string()));
                    meta
                },
            });
        }

        // Phase 1 Fix: Add parameter validation before tool execution
        if let Err(validation_error) = self.validate_tool_parameters(name, &args).await {
            let error_message = format!("Parameter validation failed: {}", validation_error);
            self.surface_error_to_llm(name, &error_message, "invalid_parameters").await;
            
            return Ok(ToolResult {
                success: false,
                data: Value::Null,
                error: Some(error_message),
                execution_time_ms: 0,
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("error_code".to_string(), Value::String("invalid_parameters".to_string()));
                    meta.insert("validation_error".to_string(), Value::String(validation_error));
                    meta
                },
            });
        }

        // Special handling for shell_execution when terminal streaming is available
        if name == "shell_execution" && self.terminal_event_sender.is_some() {
            return self.execute_streaming_shell_tool(name, args).await;
        }
        
        let tool = self
            .tool_registry
            .get(name)
            .await
            .ok_or_else(|| {
                // Surface tool not found error to LLM
                tokio::spawn({
                    let executor = self.clone();
                    let tool_name = name.to_string();
                    async move {
                        executor.surface_error_to_llm(&tool_name, "Tool not found in registry", "tool_not_found").await;
                    }
                });
                ReasoningError::tool_execution(name.to_string(), "Tool not found".to_string())
            })?;

        match tool.execute(args.clone()).await {
            Ok(sagitta_code_tool_result) => {
                match sagitta_code_tool_result {
                    crate::tools::types::ToolResult::Success(val) => {
                        // Post-process shell execution results to provide better completion confidence
                        let processed_result = if name == "shell_execution" {
                            self.post_process_shell_result(&val).unwrap_or(val)
                        } else {
                            val
                        };
                        
                        Ok(ToolResult {
                            success: true,
                            data: processed_result,
                            error: None,
                            execution_time_ms: 0, // Placeholder for duration
                            metadata: HashMap::new(),
                        })
                    },
                    crate::tools::types::ToolResult::Error { error: msg } => {
                        // Phase 1 Fix: Surface execution errors to LLM
                        self.surface_error_to_llm(name, &msg, "execution_error").await;
                        
                        Ok(ToolResult { 
                            success: false,
                            data: Value::Null,
                            error: Some(msg),
                            execution_time_ms: 0, // Placeholder for duration
                            metadata: {
                                let mut meta = HashMap::new();
                                meta.insert("error_code".to_string(), Value::String("execution_error".to_string()));
                                meta
                            },
                        })
                    },
                }
            }
            Err(sagitta_code_error) => {
                // Phase 1 Fix: Surface system errors to LLM
                let error_msg = sagitta_code_error.to_string();
                self.surface_error_to_llm(name, &error_msg, "system_error").await;
                
                Err(ReasoningError::tool_execution(
                    name.to_string(),
                    error_msg,
                ))
            }
        }
    }

    async fn get_available_tools(&self) -> ReasoningEngineResult<Vec<reasoning_engine::traits::ToolDefinition>> {
        let sagitta_code_tools: Vec<crate::tools::types::ToolDefinition> = self.tool_registry.get_definitions().await;
        let mut reasoning_tools = Vec::new();
        for sagitta_code_tool in sagitta_code_tools {
            reasoning_tools.push(reasoning_engine::traits::ToolDefinition {
                name: sagitta_code_tool.name,
                description: sagitta_code_tool.description,
                parameters: sagitta_code_tool.parameters, 
                is_required: sagitta_code_tool.is_required,
                category: Some(sagitta_code_tool.category.to_string()), 
                estimated_duration_ms: None, 
            });
        }
        Ok(reasoning_tools)
    }
}

pub struct AgentEventEmitter {
    event_sender: broadcast::Sender<AgentEvent>,
}

impl AgentEventEmitter {
    pub fn new(event_sender: broadcast::Sender<AgentEvent>) -> Self {
        Self { event_sender }
    }
    
    /// Helper method to emit a streaming text chunk
    async fn emit_streaming_text(&self, content: String, is_final: bool) {
        if let Err(e) = self.event_sender.send(AgentEvent::LlmChunk {
            content: content.clone(),
            is_final,
        }) {
            log::warn!("AgentEventEmitter: Failed to send streaming text chunk: {}", e);
        }
    }
}

#[async_trait]
impl EventEmitter for AgentEventEmitter {
    async fn emit_event(&self, event: ReasoningEvent) -> ReasoningEngineResult<()> {
        let agent_event = match event {
            ReasoningEvent::SessionStarted { session_id, input, timestamp: _ } => {
                // Don't emit streaming text for session start to avoid clutter
                AgentEvent::ReasoningStarted {
                    session_id,
                    input,
                }
            }
            ReasoningEvent::SessionCompleted {
                session_id,
                success,
                total_duration_ms,
                steps_executed,
                tools_used,
            } => AgentEvent::ReasoningCompleted {
                session_id,
                success,
                duration_ms: total_duration_ms,
                steps: steps_executed,
                tools: tools_used,
            },
            ReasoningEvent::StepCompleted { session_id, step_id: _, step_type, confidence: _, duration_ms } => {
                AgentEvent::ReasoningStep {
                    session_id,
                    step: duration_ms as u32,
                    description: step_type,
                }
            }
            ReasoningEvent::ToolExecutionStarted { session_id: _, tool_name, tool_args: _ } => {
                // Only emit a log event, do not emit streaming text to avoid duplicate UI messages
                AgentEvent::Log(format!("Tool execution started: {}", tool_name))
            }
            ReasoningEvent::ToolExecutionCompleted { session_id: _, tool_name, success, duration_ms } => {
                // Emit streaming text for tool execution completion
                let status_icon = if success { "✅" } else { "❌" };
                self.emit_streaming_text(
                    format!("\n{} Tool **{}** completed in {}ms\n\n", status_icon, tool_name, duration_ms),
                    false
                ).await;
                
                AgentEvent::ToolCompleted {
                    tool_name,
                    success,
                    duration_ms,
                }
            }
            ReasoningEvent::Summary { session_id: _, content, timestamp: _ } => {
                // Don't emit LlmChunk here - the AgentStreamHandler already handles this
                // Just emit a log for tracking
                AgentEvent::Log(format!("Summary generated: {}", content.chars().take(100).collect::<String>()))
            }
            ReasoningEvent::DecisionMade { session_id, decision_id: _, options_considered: _, chosen_option, confidence } => {
                AgentEvent::DecisionMade {
                    session_id,
                    decision: chosen_option,
                    confidence,
                }
            }
            ReasoningEvent::StreamChunkReceived { session_id: _, chunk_type, chunk_size } => {
                AgentEvent::Log(format!("ReasoningEngine: StreamChunkReceived - Type: {}, Size: {}", chunk_type, chunk_size))
            }
            ReasoningEvent::ErrorOccurred { session_id: _, error_type: _, error_message, recoverable: _ } => {
                // Emit streaming text for errors
                self.emit_streaming_text(
                    format!("❌ Error: {}", error_message),
                    false
                ).await;
                
                AgentEvent::Error(error_message)
            }
            ReasoningEvent::TokenUsageReceived { session_id, usage } => {
                AgentEvent::TokenUsageReport {
                    conversation_id: Some(session_id),
                    model_name: usage.model_name,
                    prompt_tokens: usage.prompt_tokens as u32,
                    completion_tokens: usage.completion_tokens as u32,
                    cached_tokens: usage.cached_tokens.map(|ct| ct as u32),
                    total_tokens: usage.total_tokens as u32,
                }
            }
            _ => {
                AgentEvent::Log(format!("Unhandled/Generic ReasoningEvent: {:?}", event))
            }
        };

        if let Err(e) = self.event_sender.send(agent_event) {
            log::error!("Failed to broadcast AgentEvent from AgentEventEmitter: {}", e);
        }
        Ok(())
    }
}

pub struct AgentStreamHandler {
    output_chunk_sender: mpsc::UnboundedSender<Result<SagittaCodeStreamChunk, SagittaCodeError>>,
}

impl AgentStreamHandler {
    pub fn new(output_chunk_sender: mpsc::UnboundedSender<Result<SagittaCodeStreamChunk, SagittaCodeError>>) -> Self {
        Self { output_chunk_sender }
    }
}

#[async_trait]
impl StreamHandler for AgentStreamHandler {
    async fn handle_chunk(&self, chunk: StreamChunk) -> ReasoningEngineResult<()> {
        let sagitta_code_chunk_conversion_result: Result<Option<SagittaCodeStreamChunk>, SagittaCodeError> = {
            match chunk.chunk_type.as_str() {
                "llm_text" | "text" | "llm_output" => {
                    String::from_utf8(chunk.data)
                        .map_err(|e| SagittaCodeError::ParseError(format!("Stream chunk data is not valid UTF-8 for text: {}", e)))
                        .map(|text_content| Some(SagittaCodeStreamChunk {
                            part: MessagePart::Text { text: text_content },
                            is_final: chunk.is_final,
                            finish_reason: if chunk.is_final { chunk.metadata.get("finish_reason").cloned() } else { None },
                            token_usage: None,
                        }))
                }
                "summary" => {
                    // Handle summary chunks with proper metadata
                    String::from_utf8(chunk.data)
                        .map_err(|e| SagittaCodeError::ParseError(format!("Stream chunk data is not valid UTF-8 for summary: {}", e)))
                        .map(|text_content| {
                            // Create a text chunk with summary metadata
                            let mut sagitta_code_chunk = SagittaCodeStreamChunk {
                                part: MessagePart::Text { text: text_content },
                                is_final: chunk.is_final,
                                finish_reason: if chunk.is_final { chunk.metadata.get("finish_reason").cloned() } else { None },
                                token_usage: None,
                            };
                            // The metadata will be handled by the agent event system
                            Some(sagitta_code_chunk)
                        })
                }
                "tool_call" => {
                    serde_json::from_slice(&chunk.data)
                        .map_err(|e| SagittaCodeError::ParseError(format!("Stream chunk data is not valid JSON for tool_call: {}", e)))
                        .map(|tool_call_data: serde_json::Value| {
                            let call_id = tool_call_data.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let name = tool_call_data.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let parameters = tool_call_data.get("parameters").cloned().unwrap_or(serde_json::Value::Null);
                            Some(SagittaCodeStreamChunk {
                                part: MessagePart::ToolCall { tool_call_id: call_id, name, parameters },
                                is_final: chunk.is_final,
                                finish_reason: None,
                                token_usage: None,
                            })
                        })
                }
                "tool_result" => {
                    serde_json::from_slice(&chunk.data)
                        .map_err(|e| SagittaCodeError::ParseError(format!("Stream chunk data is not valid JSON for tool_result: {}", e)))
                        .map(|tool_result_data: serde_json::Value| {
                            let call_id = tool_result_data.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let name = tool_result_data.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let result_val = tool_result_data.get("result").cloned().unwrap_or(serde_json::Value::Null);
                            Some(SagittaCodeStreamChunk {
                                part: MessagePart::ToolResult { tool_call_id: call_id, name, result: result_val },
                                is_final: chunk.is_final,
                                finish_reason: None,
                                token_usage: None,
                            })
                        })
                }
                _ => Ok(None), 
            }
        };

        match sagitta_code_chunk_conversion_result {
            Ok(Some(sagitta_code_chunk)) => {
                if self.output_chunk_sender.send(Ok(sagitta_code_chunk)).is_err() {
                    return Err(ReasoningError::streaming("output_channel_closed".to_string(), "Error sending SagittaCodeStreamChunk".to_string()));
                }
            }
            Ok(None) => {}
            Err(sagitta_code_error) => {
                if self.output_chunk_sender.send(Err(sagitta_code_error)).is_err() {
                    return Err(ReasoningError::streaming("output_channel_closed_sending_error".to_string(), "Error sending SagittaCodeError".to_string()));
                }
            }
        }
        Ok(())
    }

    async fn handle_stream_complete(&self, _stream_id: Uuid) -> ReasoningEngineResult<()> {
        let final_chunk = SagittaCodeStreamChunk {
            part: MessagePart::Text { text: String::new() }, 
            is_final: true,
            finish_reason: Some("REASONING_INTERNAL_STREAM_ENDED".to_string()),
            token_usage: None,
        };
        if self.output_chunk_sender.send(Ok(final_chunk)).is_err() {
            // eprintln!("Output channel closed before sending final stream complete marker.");
        }
        Ok(())
    }

    async fn handle_stream_error(&self, stream_id: Uuid, error: ReasoningError) -> ReasoningEngineResult<()> {
        let sagitta_code_error = SagittaCodeError::ReasoningError(format!("Internal stream {} failed: {}", stream_id, error));
        if self.output_chunk_sender.send(Err(sagitta_code_error)).is_err() {
            return Err(ReasoningError::streaming("output_channel_closed_on_error".to_string(), "Error sending SagittaCodeError after stream error".to_string()));
        }
        Ok(())
    }
}

pub struct AgentStatePersistence {}

impl AgentStatePersistence {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl StatePersistence for AgentStatePersistence {
    async fn save_state(&self, session_id: Uuid, state_bytes: &[u8]) -> ReasoningEngineResult<()> {
        println!("AgentStatePersistence: Saving state for session {}", session_id);
        // Placeholder: actual save logic for state_bytes
        // Example: std::fs::write(format!("{}_state.bin", session_id), state_bytes).map_err(|e| ReasoningError::state("save_error".to_string(), e.to_string()))?;
        Ok(())
    }

    async fn load_state(&self, session_id: Uuid) -> ReasoningEngineResult<Option<Vec<u8>>> {
        println!("AgentStatePersistence: Loading state for session {}", session_id);
        // Placeholder: actual load logic
        // Example: match std::fs::read(format!("{}_state.bin", session_id)) {
        //     Ok(bytes) => Ok(Some(bytes)),
        //     Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        //     Err(e) => Err(ReasoningError::state("load_error".to_string(), e.to_string())),
        // }
        Ok(None)
    }

    async fn delete_state(&self, _session_id: Uuid) -> ReasoningEngineResult<()> {
        Ok(())
    }

    async fn list_states(&self) -> ReasoningEngineResult<Vec<Uuid>> {
        Ok(Vec::new())
    }
}

pub struct AgentMetricsCollector {}

impl AgentMetricsCollector {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl MetricsCollector for AgentMetricsCollector {
    async fn record_counter(&self, counter_name: &str, value: u64, tags: HashMap<String, String>) -> ReasoningEngineResult<()> {
        println!("AgentMetricsCollector: Recording counter: {}, value: {}, tags: {:?}", counter_name, value, tags);
        Ok(())
    }

    async fn record_gauge(&self, gauge_name: &str, value: f64, tags: HashMap<String, String>) -> ReasoningEngineResult<()> {
        println!("AgentMetricsCollector: Recording gauge: {}, value: {}, tags: {:?}", gauge_name, value, tags);
        Ok(())
    }

    async fn record_histogram(&self, histogram_name: &str, value: f64, tags: HashMap<String, String>) -> ReasoningEngineResult<()> {
        println!("AgentMetricsCollector: Recording histogram: {}, value: {}, tags: {:?}", histogram_name, value, tags);
        Ok(())
    }
    // record_timing has a default implementation in the trait
}

pub use config::create_reasoning_config;
pub use llm_adapter::ReasoningLlmClientAdapter;
pub use intent_analyzer::SagittaCodeIntentAnalyzer;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;
    use uuid::Uuid;
    use chrono::Utc;

    #[tokio::test]
    async fn test_summary_event_conversion() {
        let (sender, mut receiver) = broadcast::channel(100);
        let emitter = AgentEventEmitter::new(sender);

        let session_id = Uuid::new_v4();
        let summary_content = "Okay, I've finished those tasks. Successfully completed: web search.";
        
        // Emit a Summary event
        let summary_event = ReasoningEvent::Summary {
            session_id,
            content: summary_content.to_string(),
            timestamp: Utc::now(),
        };

        let result = emitter.emit_event(summary_event).await;
        assert!(result.is_ok(), "Summary event emission should succeed");

        // Check that the correct AgentEvent was emitted
        let received_event = receiver.recv().await.expect("Should receive an event");
        
        match received_event {
            AgentEvent::Log(log_message) => {
                assert!(log_message.contains("Summary generated"));
                assert!(log_message.contains(&summary_content.chars().take(100).collect::<String>()));
            }
            _ => panic!("Expected Log event, got: {:?}", received_event),
        }
    }

    #[tokio::test]
    async fn test_tool_execution_started_event_conversion() {
        let (sender, mut receiver) = broadcast::channel(100);
        let emitter = AgentEventEmitter::new(sender);

        let session_id = Uuid::new_v4();
        let tool_name = "test_tool";
        
        // Emit a ToolExecutionStarted event
        let tool_event = ReasoningEvent::ToolExecutionStarted {
            session_id,
            tool_name: tool_name.to_string(),
            tool_args: serde_json::json!({"param": "value"}),
        };

        let result = emitter.emit_event(tool_event).await;
        assert!(result.is_ok(), "Tool execution started event emission should succeed");

        // Check that only a log event was emitted (no streaming text)
        let received_event = receiver.recv().await.expect("Should receive an event");
        
        match received_event {
            AgentEvent::Log(log_message) => {
                assert!(log_message.contains("Tool execution started"));
                assert!(log_message.contains(tool_name));
            }
            _ => panic!("Expected Log event, got: {:?}", received_event),
        }
    }

    #[tokio::test]
    async fn test_session_completed_event_conversion() {
        let (sender, mut receiver) = broadcast::channel(100);
        let emitter = AgentEventEmitter::new(sender);

        let session_id = Uuid::new_v4();
        
        // Emit a SessionCompleted event
        let completed_event = ReasoningEvent::SessionCompleted {
            session_id,
            success: true,
            total_duration_ms: 1500,
            steps_executed: 3,
            tools_used: vec!["analyze_input".to_string(), "web_search".to_string()],
        };

        let result = emitter.emit_event(completed_event).await;
        assert!(result.is_ok(), "Session completed event emission should succeed");

        // Check that the correct AgentEvent was emitted
        let received_event = receiver.recv().await.expect("Should receive an event");
        
        match received_event {
            AgentEvent::ReasoningCompleted { session_id: recv_session_id, success, .. } => {
                assert_eq!(recv_session_id, session_id);
                assert!(success);
            }
            _ => panic!("Expected ReasoningCompleted event, got: {:?}", received_event),
        }
    }

    #[tokio::test]
    async fn test_tool_execution_completed_icons() {
        let (sender, mut receiver) = broadcast::channel(100);
        let emitter = AgentEventEmitter::new(sender);

        let session_id = Uuid::new_v4();
        let tool_name = "test_tool";

        // Emit a successful ToolExecutionCompleted event
        let success_event = ReasoningEvent::ToolExecutionCompleted {
            session_id,
            tool_name: tool_name.to_string(),
            success: true,
            duration_ms: 42,
        };

        emitter.emit_event(success_event).await.unwrap();

        // Expect an LlmChunk with the ✅ icon and without the old [DONE] text
        let received_event = receiver.recv().await.expect("Should receive an event");

        match received_event {
            AgentEvent::LlmChunk { content, .. } => {
                assert!(content.contains("✅"), "Content should contain success icon");
                assert!(!content.contains("[DONE]"), "Content should not contain old [DONE] tag");
            }
            _ => panic!("Expected LlmChunk event"),
        }

        // Emit a failed ToolExecutionCompleted event
        let fail_event = ReasoningEvent::ToolExecutionCompleted {
            session_id,
            tool_name: tool_name.to_string(),
            success: false,
            duration_ms: 50,
        };

        emitter.emit_event(fail_event).await.unwrap();

        let received_event = receiver.recv().await.expect("Should receive an event");

        match received_event {
            AgentEvent::LlmChunk { content, .. } => {
                assert!(content.contains("❌"), "Content should contain failure icon");
                assert!(!content.contains("[FAIL]"), "Content should not contain old [FAIL] tag");
            }
            _ => panic!("Expected LlmChunk event"),
        }
    }
} 