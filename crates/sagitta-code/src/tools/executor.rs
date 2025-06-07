// Tool execution orchestration will go here

use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;
use log::{debug, info, warn, error};

use crate::agent::message::types::ToolCall;
use crate::agent::state::manager::StateManager;
use crate::tools::registry::ToolRegistry;
use crate::tools::types::{ToolResult, ToolParameters};
use crate::tools::shell_execution::{StreamingShellExecutionTool, ShellExecutionParams};
use crate::utils::errors::SagittaCodeError;
use terminal_stream::events::StreamEvent;

/// Event emitted by the tool executor
#[derive(Debug, Clone)]
pub enum ToolExecutionEvent {
    /// A tool execution has started
    Started {
        /// The ID of the tool call
        tool_call_id: String,
        
        /// The name of the tool
        tool_name: String,
        
        /// The parameters passed to the tool
        parameters: Value,
    },
    
    /// A tool execution has completed
    Completed {
        /// The ID of the tool call
        tool_call_id: String,
        
        /// The name of the tool
        tool_name: String,
        
        /// The result of the tool execution
        result: ToolResult,
    },
    
    /// A tool execution has failed
    Failed {
        /// The ID of the tool call
        tool_call_id: String,
        
        /// The name of the tool
        tool_name: String,
        
        /// The error message
        error: String,
    },
}

/// Executor for tool calls
#[derive(Debug, Clone)]
pub struct ToolExecutor {
    /// The tool registry
    registry: Arc<ToolRegistry>,
    
    /// The state manager
    state_manager: Arc<StateManager>,
    
    /// The event sender
    event_sender: mpsc::Sender<ToolExecutionEvent>,
    
    /// Terminal event sender for streaming shell execution
    terminal_event_sender: Option<mpsc::Sender<StreamEvent>>,
}

impl ToolExecutor {
    /// Create a new tool executor
    pub fn new(
        registry: Arc<ToolRegistry>,
        state_manager: Arc<StateManager>,
    ) -> (Self, mpsc::Receiver<ToolExecutionEvent>) {
        let (event_sender, event_receiver) = mpsc::channel(100);
        
        debug!("ToolExecutor: Creating new instance.");
        
        let executor = Self {
            registry,
            state_manager,
            event_sender,
            terminal_event_sender: None,
        };
        
        (executor, event_receiver)
    }
    
    /// Set the terminal event sender for streaming shell execution
    pub fn set_terminal_event_sender(&mut self, sender: mpsc::Sender<StreamEvent>) {
        self.terminal_event_sender = Some(sender);
    }
    
    /// Execute a tool with the given parameters, using streaming if it's a shell execution
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        parameters: Value,
    ) -> Result<ToolResult, SagittaCodeError> {
        // Generate a unique ID for this tool call
        let tool_call_id = Uuid::new_v4().to_string();
        
        // Check if this is a shell execution tool and we have terminal streaming enabled
        if (tool_name == "shell_execution" || tool_name == "streaming_shell_execution") 
            && self.terminal_event_sender.is_some() {
            self.execute_streaming_shell_tool(&tool_call_id, tool_name, parameters).await
        } else {
            // Execute the tool normally
            self.execute_tool_with_id(&tool_call_id, tool_name, parameters).await
        }
    }
    
    /// Execute a streaming shell command
    async fn execute_streaming_shell_tool(
        &self,
        tool_call_id: &str,
        tool_name: &str,
        parameters: Value,
    ) -> Result<ToolResult, SagittaCodeError> {
        debug!("ToolExecutor: Executing streaming shell tool: '{}' with ID: '{}'", tool_name, tool_call_id);
        
        // Parse shell execution parameters
        let shell_params: ShellExecutionParams = serde_json::from_value(parameters.clone())
            .map_err(|e| SagittaCodeError::ToolError(
                format!("Invalid shell execution parameters: {}", e)
            ))?;
        
        // Get the streaming shell tool
        let streaming_tool = match self.registry.get("streaming_shell_execution").await {
            Some(tool) => tool,
            None => {
                return Err(SagittaCodeError::ToolNotFound(
                    "streaming_shell_execution".to_string()
                ));
            }
        };
        
        // Cast to streaming shell tool
        let streaming_shell_tool = streaming_tool
            .as_any()
            .downcast_ref::<StreamingShellExecutionTool>()
            .ok_or_else(|| SagittaCodeError::ToolError(
                "Failed to cast to StreamingShellExecutionTool".to_string()
            ))?;
        
        // Get terminal event sender
        let terminal_sender = self.terminal_event_sender.as_ref()
            .ok_or_else(|| SagittaCodeError::ToolError(
                "Terminal event sender not configured".to_string()
            ))?;
        
        // Send tool execution started event
        self.event_sender.send(ToolExecutionEvent::Started {
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            parameters,
        }).await.map_err(|e| SagittaCodeError::Unknown(format!("Failed to send tool started event: {}", e)))?;
        
        // Execute with streaming
        let result = streaming_shell_tool.execute_streaming(shell_params, terminal_sender.clone()).await;
        
        match result {
            Ok(shell_result) => {
                let tool_result = ToolResult::Success(serde_json::to_value(shell_result)?);
                
                // Send completion event
                self.event_sender.send(ToolExecutionEvent::Completed {
                    tool_call_id: tool_call_id.to_string(),
                    tool_name: tool_name.to_string(),
                    result: tool_result.clone(),
                }).await.map_err(|e| SagittaCodeError::Unknown(format!("Failed to send tool completed event: {}", e)))?;
                
                Ok(tool_result)
            }
            Err(error) => {
                let error_message = error.to_string();
                
                // Send failure event
                self.event_sender.send(ToolExecutionEvent::Failed {
                    tool_call_id: tool_call_id.to_string(),
                    tool_name: tool_name.to_string(),
                    error: error_message.clone(),
                }).await.map_err(|e| SagittaCodeError::Unknown(format!("Failed to send tool failed event: {}", e)))?;
                
                Err(SagittaCodeError::ToolError(error_message))
            }
        }
    }
    
    /// Execute a tool with the given parameters
    pub async fn execute_tool_with_id(
        &self,
        tool_call_id: &str,
        tool_name: &str,
        parameters: Value,
    ) -> Result<ToolResult, SagittaCodeError> {
        debug!(
            "ToolExecutor (executor.rs): Executing tool: '{}' with call ID: '{}'. Parameters: {:?}",
            tool_name,
            tool_call_id,
            parameters
        );
        debug!("DEBUG: ToolExecutor::execute_tool_with_id called for tool: {} (ID: {})", tool_name, tool_call_id);
        
        // Let the UI know that a tool is starting
        self.state_manager.set_thinking(&format!("Executing tool (executor.rs): {}", tool_name)).await?;
        self.event_sender.send(ToolExecutionEvent::Started {
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            parameters: parameters.clone(),
        }).await.map_err(|e| SagittaCodeError::Unknown(format!("Failed to send tool started event: {}", e)))?;
        
        debug!("ToolExecutor: Looking up tool '{}' in registry.", tool_name);
        match self.registry.get(tool_name).await {
            Some(tool) => {
                debug!("ToolExecutor: Tool '{}' found. Executing...", tool_name);
                let start_time = std::time::Instant::now();
                
                let exec_result = match tool.execute(parameters).await {
                    Ok(result) => {
                        let duration = start_time.elapsed();
                        debug!(
                            "ToolExecutor: Tool '{}' (ID: '{}') execution finished successfully in {:?}. Result: {:?}",
                            tool_name,
                            tool_call_id,
                            duration,
                            result
                        );
                        
                        debug!("DEBUG: ToolExecutor sending completion event for tool: {} (ID: {})", tool_name, tool_call_id);
                        self.event_sender.send(ToolExecutionEvent::Completed {
                            tool_call_id: tool_call_id.to_string(),
                            tool_name: tool_name.to_string(),
                            result: result.clone(),
                        }).await.map_err(|e| SagittaCodeError::Unknown(format!("ToolExecutor: Failed to send tool completed event: {}", e)))?;
                        debug!("DEBUG: ToolExecutor successfully sent completion event for tool: {}", tool_name);
                        
                        Ok(result)
                    },
                    Err(err) => {
                        let duration = start_time.elapsed();
                        error!(
                            "ToolExecutor: Tool '{}' (ID: '{}') execution failed in {:?}. Error: {:?}",
                            tool_name,
                            tool_call_id,
                            duration,
                            err
                        );
                        let error_message = err.to_string();
                        self.event_sender.send(ToolExecutionEvent::Failed {
                            tool_call_id: tool_call_id.to_string(),
                            tool_name: tool_name.to_string(),
                            error: error_message.clone(),
                        }).await.map_err(|e| SagittaCodeError::Unknown(format!("ToolExecutor: Failed to send tool failed event: {}", e)))?;
                        
                        Err(SagittaCodeError::ToolError(error_message))
                    }
                };
                exec_result
            },
            None => {
                warn!("ToolExecutor: Tool '{}' with call ID '{}' not found in registry.", tool_name, tool_call_id);
                // Send a Failed event even if tool not found, so UI is updated.
                self.event_sender.send(ToolExecutionEvent::Failed {
                    tool_call_id: tool_call_id.to_string(),
                    tool_name: tool_name.to_string(),
                    error: format!("Tool '{}' not found in registry.", tool_name),
                }).await.map_err(|e| SagittaCodeError::Unknown(format!("ToolExecutor: Failed to send tool not found event: {}", e)))?;
                
                // Return an error to the caller (Agent)
                Err(SagittaCodeError::ToolNotFound(tool_name.to_string()))
            }
        }
    }
    
    /// Execute a tool call
    pub async fn execute_tool_call(&self, tool_call: &ToolCall) -> Result<ToolResult, SagittaCodeError> {
        self.execute_tool_with_id(
            &tool_call.id,
            &tool_call.name,
            tool_call.arguments.clone(),
        ).await
    }
    
    /// Create a new event sender
    pub fn new_event_sender(&self) -> mpsc::Sender<ToolExecutionEvent> {
        self.event_sender.clone()
    }
}

