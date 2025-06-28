use futures_util::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use log::{debug, info, trace, warn, error};
use uuid::Uuid;

use crate::agent::message::types::{AgentMessage, ToolCall};
use crate::agent::message::history::ConversationAwareHistoryManager;
use crate::agent::state::manager::StateManager;
use crate::agent::state::types::AgentMode;
use crate::agent::events::AgentEvent;
use crate::tools::executor::ToolExecutor;
use crate::tools::registry::ToolRegistry;
use crate::utils::errors::SagittaCodeError;
use crate::config::types::SagittaCodeConfig;
use crate::llm::client::{LlmClient, StreamChunk, MessagePart, ToolDefinition};
use crate::tools::types::ToolDefinition as ToolDefinitionType;

/// Streaming processor for handling LLM streaming responses and tool execution
#[derive(Clone)]
pub struct StreamingProcessor {
    /// The LLM client
    llm_client: Arc<dyn LlmClient>,
    
    /// The tool registry
    tool_registry: Arc<ToolRegistry>,
    
    /// The message history manager
    history: Arc<ConversationAwareHistoryManager>,
    
    /// The state manager
    state_manager: Arc<StateManager>,
    
    /// The tool executor
    tool_executor: Arc<tokio::sync::Mutex<ToolExecutor>>,
    
    /// Sender for agent events
    event_sender: broadcast::Sender<AgentEvent>,
    
    /// Flag to track if we need to continue reasoning after tool completion in streaming mode
    continue_reasoning_after_tool: Arc<Mutex<bool>>,
}

impl StreamingProcessor {
    /// Create a new streaming processor
    pub fn new(
        llm_client: Arc<dyn LlmClient>,
        tool_registry: Arc<ToolRegistry>,
        history: Arc<ConversationAwareHistoryManager>,
        state_manager: Arc<StateManager>,
        tool_executor: Arc<tokio::sync::Mutex<ToolExecutor>>,
        event_sender: broadcast::Sender<AgentEvent>,
        continue_reasoning_after_tool: Arc<Mutex<bool>>,
    ) -> Self {
        Self {
            llm_client,
            tool_registry,
            history,
            state_manager,
            tool_executor,
            event_sender,
            continue_reasoning_after_tool,
        }
    }
    
    /// Process a user message with streaming
    pub async fn process_message_stream(&self, message: impl Into<String>) 
        -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> 
    {
        let message_text = message.into();
        info!("Processing user message (stream): '{}'", message_text);
        
        // Set state to thinking
        self.state_manager.set_thinking("Processing user message").await?;
        
        // Add the user message to history
        self.history.add_user_message(&message_text).await;
        debug!("Added user message to history (stream).");
        
        // Get all messages from history
        let messages = self.history.to_llm_messages().await;
        debug!("Retrieved {} messages from history for LLM (stream).", messages.len());
        trace!("Messages for LLM (stream): {:?}", messages);
        
        // Get available tool definitions
        let tool_defs_from_registry: Vec<ToolDefinitionType> = self.tool_registry.get_definitions().await;
        debug!("Retrieved {} tool definitions from registry for LLM (stream).", tool_defs_from_registry.len());
        let tool_defs_for_llm: Vec<ToolDefinition> = tool_defs_from_registry
            .iter()
            .map(|td| ToolDefinition {
                name: td.name.clone(),
                description: td.description.clone(),
                parameters: td.parameters.clone(),
                is_required: td.is_required,
            })
            .collect();
        
        // Create the agent response message
        let assistant_message = AgentMessage::assistant_streaming("".to_string());
        self.history.add_message(assistant_message.clone()).await;
        
        // Call the LLM with streaming
        info!("Sending request to LLM (stream)...");
        match self.llm_client.generate_stream(&messages, &tool_defs_for_llm).await {
            Ok(stream) => {
                info!("Received LLM stream.");
                // Set state to responding with streaming
                self.state_manager.set_responding(true, "Streaming response").await?;
                
                // Process the stream and handle tool calls
                let event_sender = self.event_sender.clone();
                let history_manager = self.history.clone();
                let state_manager = self.state_manager.clone();
                let tool_executor = self.tool_executor.clone();
                let continue_reasoning_flag = self.continue_reasoning_after_tool.clone();
                
                let message_id = assistant_message.id;
                
                // Transform the stream to handle tool calls and update state
                let mapped_stream = stream.map(move |result| {
                    let event_sender = event_sender.clone();
                    let history_manager = history_manager.clone();
                    let state_manager = state_manager.clone();
                    let tool_executor = tool_executor.clone();
                    let message_id = message_id;
                    
                    async move {
                        match result {
                            Ok(chunk) => {
                                match &chunk.part {
                                    MessagePart::Text { text } => {
                                        // Emit text chunk event
                                        let _ = event_sender.send(AgentEvent::LlmChunk {
                                            content: text.clone(),
                                            is_final: chunk.is_final,
                                            is_thinking: false,
                                        });
                                        trace!("Stream: Text chunk received: '{}', final: {}", text, chunk.is_final);
                                        
                                        // Update message content in history
                                        if let Some(msg) = history_manager.get_message(message_id).await {
                                            let mut updated = msg.clone();
                                            updated.content.push_str(text);
                                            
                                            if chunk.is_final {
                                                updated.is_streaming = false;
                                            }
                                            
                                            let _ = history_manager.remove_message(message_id).await;
                                            let _ = history_manager.add_message(updated).await;
                                        }
                                        
                                        // If final, update state to idle
                                        if chunk.is_final {
                                            let _ = state_manager.set_idle("Response complete").await;
                                        }
                                    },
                                    MessagePart::Thought { text } => {
                                        // Emit thought chunk event (different from text chunks)
                                        let _ = event_sender.send(AgentEvent::LlmChunk {
                                            content: text.clone(),
                                            is_final: chunk.is_final,
                                            is_thinking: true,
                                        });
                                        trace!("Stream: Thought chunk received: '{}', final: {}", text, chunk.is_final);
                                        
                                        // Note: We might want to handle thoughts differently in the future
                                        // For now, treat them similar to text but don't add to main content
                                        
                                        // If final, update state to idle
                                        if chunk.is_final {
                                            let _ = state_manager.set_idle("Thinking complete").await;
                                        }
                                    },
                                    MessagePart::ToolCall { tool_call_id, name, parameters } => {
                                        // Create a tool call
                                        let tool_call = ToolCall {
                                            id: tool_call_id.clone(),
                                            name: name.clone(),
                                            arguments: parameters.clone(),
                                            result: None,
                                            successful: false,
                                            execution_time: None,
                                        };
                                        info!("Stream: LLM requested tool call. ID: {}, Name: {}, Args: {:?}", tool_call_id, name, parameters);
                                        
                                        // Emit tool call event
                                        let _ = event_sender.send(AgentEvent::ToolCall {
                                            tool_call: tool_call.clone(),
                                        });
                                        
                                        // Add the tool call to the message
                                        if let Some(msg) = history_manager.get_message(message_id).await {
                                            let mut updated = msg.clone();
                                            updated.tool_calls.push(tool_call);
                                            
                                            let _ = history_manager.remove_message(message_id).await;
                                            let _ = history_manager.add_message(updated).await;
                                        }
                                        
                                        // Execute the tool asynchronously if in auto mode
                                        if state_manager.get_agent_mode().await == AgentMode::FullyAutonomous {
                                            let tc_id = tool_call_id.clone();
                                            let tc_name = name.clone();
                                            let tc_params = parameters.clone();
                                            let tool_executor_clone = tool_executor.clone();
                                            
                                            // Execute the tool asynchronously without blocking the stream
                                            tokio::spawn(async move {
                                                match tool_executor_clone.lock().await.execute_tool_with_id(&tc_id, &tc_name, tc_params).await {
                                                    Ok(tool_result) => {
                                                        info!("Stream: Tool execution completed for ID: {}. Success: {}", tc_id, tool_result.is_success());
                                                        
                                                        // Note: The reasoning continuation should be handled by the tool event listener
                                                        // The stream will naturally end here, and the agent should continue reasoning
                                                        // via the tool completion event handling mechanism
                                                    },
                                                    Err(err) => {
                                                        error!("Stream: Tool execution failed for ID: {}: {}", tc_id, err);
                                                        // The error will be handled by the tool event listener
                                                    }
                                                }
                                            });
                                        }
                                    },
                                    MessagePart::ToolResult { .. } => {
                                        warn!("Stream: Received unexpected ToolResult part in stream. This should be handled via ToolExecutionEvent.");
                                        // Tool results are typically added separately, not part of the stream
                                    },
                                }
                                
                                Ok(chunk)
                            },
                            Err(err) => {
                                error!("Stream: Error receiving chunk: {}", err);
                                // Set state to error
                                let _ = state_manager.set_error(err.to_string(), "Streaming failed").await;
                                let _ = event_sender.send(AgentEvent::Error(err.to_string()));
                                
                                Err(err)
                            }
                        }
                    }
                }).then(|fut| fut);
                
                Ok(Box::pin(mapped_stream))
            },
            Err(err) => {
                error!("LLM streaming call failed: {}", err);
                // Set state to error
                self.state_manager.set_error(err.to_string(), "LLM streaming call failed").await?;
                self.event_sender.send(AgentEvent::Error(err.to_string()))?;
                
                Err(err)
            }
        }
    }
} 