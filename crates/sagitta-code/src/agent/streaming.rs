use futures_util::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
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
    
    /// Track tool calls by ID for result matching
    tool_calls: Arc<Mutex<HashMap<String, (String, String)>>>, // ID -> (name, preview)
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
            tool_calls: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Process a user message with streaming
    pub async fn process_message_stream(&self, message: impl Into<String>) 
        -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send + '_>>, SagittaCodeError> 
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
                
                // Process the stream
                let event_sender = self.event_sender.clone();
                let history_manager = self.history.clone();
                let state_manager = self.state_manager.clone();
                
                let message_id = assistant_message.id;
                
                // Transform the stream to update state
                let mapped_stream = stream.map(move |result| {
                    let event_sender = event_sender.clone();
                    let history_manager = history_manager.clone();
                    let state_manager = state_manager.clone();
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
                                        // For Claude CLI, tools are executed through MCP
                                        info!("Stream: Claude CLI executed tool through MCP - ID: {}, Name: {}", tool_call_id, name);
                                        
                                        // Add tool execution preview as inline text content
                                        let tool_preview = crate::gui::app::events::format_tool_arguments_for_display(
                                            name, 
                                            &serde_json::to_string(parameters).unwrap_or_default()
                                        );
                                        
                                        // Store tool call info for result matching
                                        if let Ok(mut tool_calls) = self.tool_calls.lock() {
                                            tool_calls.insert(tool_call_id.clone(), (name.clone(), tool_preview.clone()));
                                        }
                                        
                                        let inline_text = format!("🔧 {} - [View Raw Result](tool://{})\n\n", tool_preview, tool_call_id);
                                        
                                        // Emit as text chunk so it appears inline
                                        let _ = event_sender.send(AgentEvent::LlmChunk {
                                            content: inline_text,
                                            is_final: false,
                                            is_thinking: false,
                                        });
                                        
                                        info!("Stream: Added inline tool preview - {}", name);
                                    },
                                    MessagePart::ToolResult { tool_call_id, name, result } => {
                                        info!("Stream: Received tool result for ID: {}", tool_call_id);
                                        
                                        // Get tool name from stored tool calls or use the provided name
                                        let tool_name = if let Ok(tool_calls) = self.tool_calls.lock() {
                                            tool_calls.get(tool_call_id).map(|(name, _)| name.clone()).unwrap_or_else(|| name.clone())
                                        } else {
                                            name.clone()
                                        };
                                        
                                        // Emit tool call complete event with the result
                                        let tool_result = crate::tools::types::ToolResult::Success(result.clone());
                                        
                                        let _ = event_sender.send(AgentEvent::ToolCallComplete {
                                            tool_call_id: tool_call_id.clone(),
                                            tool_name,
                                            result: tool_result,
                                        });
                                        
                                        info!("Stream: Emitted ToolCallComplete event for ID: {}", tool_call_id);
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