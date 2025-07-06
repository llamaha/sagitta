use tokio::sync::{mpsc, broadcast};
use log::{debug, info, warn};
use std::sync::Arc;
use uuid::Uuid;

use crate::agent::message::types::AgentMessage;
use crate::agent::message::history::ConversationAwareHistoryManager;
use crate::agent::state::manager::{StateManager, StateEvent};
use crate::agent::state::types::{AgentState, ConversationStatus};
// Tool types removed - tools now via MCP

/// Simple tool result type for events
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ToolResult {
    Success { output: String },
    Error { error: String },
}

impl ToolResult {
    pub fn is_success(&self) -> bool {
        matches!(self, ToolResult::Success { .. })
    }
}

/// Tool execution event for internal event handling
#[derive(Debug, Clone)]
pub enum ToolExecutionEvent {
    Started {
        tool_call_id: String,
        tool_name: String,
        parameters: serde_json::Value,
    },
    Completed {
        tool_call_id: String,
        tool_name: String,
        result: ToolResult,
    },
    Failed {
        tool_call_id: String,
        tool_name: String,
        error: String,
    },
}
use crate::llm::client::Role;

/// Type alias for tool run identification
pub type ToolRunId = Uuid;

/// Events that can be emitted by the agent
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// A message from the LLM
    LlmMessage(AgentMessage),
    
    /// A chunk from the LLM (streaming response)
    LlmChunk {
        /// The chunk content
        content: String,
        
        /// Whether this is the final chunk
        is_final: bool,
        
        /// Whether this is thinking content
        is_thinking: bool,
    },
    
    /// Token usage update from LLM
    TokenUsageUpdate {
        /// The token usage information
        usage: crate::llm::client::TokenUsage,
    },
    
    /// A tool call from the LLM
    ToolCall {
        /// The tool call being executed
        tool_call: crate::agent::message::types::ToolCall,
    },
    
    /// A tool call is pending human approval
    ToolCallPending {
        /// The tool call waiting for approval
        tool_call: crate::agent::message::types::ToolCall,
    },
    
    /// A tool call was approved by human
    ToolCallApproved {
        /// The tool call ID that was approved
        tool_call_id: String,
    },
    
    /// A tool call was rejected by human
    ToolCallRejected {
        /// The tool call ID that was rejected
        tool_call_id: String,
        
        /// Optional reason for rejection
        reason: Option<String>,
    },
    
    /// A tool call has completed
    ToolCallComplete {
        /// The tool call ID
        tool_call_id: String,
        
        /// The name of the tool
        tool_name: String,
        
        /// The result of the tool call
        result: ToolResult,
    },
    
    /// The agent state has changed
    StateChanged(AgentState),
    
    /// The conversation status has changed
    ConversationStatusChanged(ConversationStatus),
    
    /// An error has occurred
    Error(String),
    
    // Additional agent events
    /// Reasoning session started
    ReasoningStarted {
        session_id: Uuid,
        input: String,
    },
    
    /// Reasoning session completed
    ReasoningCompleted {
        session_id: Uuid,
        success: bool,
        duration_ms: u64,
        steps: u32,
        tools: Vec<String>,
    },
    
    /// Reasoning step event
    ReasoningStep {
        session_id: Uuid,
        step: u32,
        description: String,
    },
    
    /// Tool execution completed
    ToolCompleted {
        tool_name: String,
        success: bool,
        duration_ms: u64,
    },
    
    /// Decision made by agent
    DecisionMade {
        session_id: Uuid,
        decision: String,
        confidence: f32,
    },

    /// Token usage report from an LLM interaction
    TokenUsageReport {
        conversation_id: Option<Uuid>, // Optional, as some usage might not be tied to a specific convo
        model_name: String,
        prompt_tokens: u32,
        completion_tokens: u32,
        cached_tokens: Option<u32>,
        total_tokens: u32,
    },

    /// Generic log message from the agent or its components
    Log(String),

    /// Tool result available for UI display
    ToolResult {
        call_id: String,
        name: String,
        result: serde_json::Value,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    // Checkpoint-related events for Phase 5 - Smart Checkpoints
    /// A checkpoint was created
    CheckpointCreated {
        conversation_id: Uuid,
        checkpoint_id: Uuid,
        message_id: Uuid,
        title: String,
        auto_generated: bool,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// A checkpoint suggestion is available
    CheckpointSuggested {
        conversation_id: Uuid,
        message_id: Uuid,
        suggestion_title: String,
        importance: f32,
        reason: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// A checkpoint was restored
    CheckpointRestored {
        conversation_id: Uuid,
        checkpoint_id: Uuid,
        title: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    // Conversation lifecycle events for Phase 11a - Status Lifecycle
    /// A conversation was completed
    ConversationCompleted {
        conversation_id: Uuid,
    },

    /// A conversation is being summarized
    ConversationSummarizing {
        conversation_id: Uuid,
    },

    /// A conversation status was updated
    ConversationUpdated {
        conversation_id: Uuid,
        old_status: ConversationStatus,
        new_status: ConversationStatus,
    },

    // Branch-related events for Phase 11e - Branch Actions
    /// A branch suggestion is available
    BranchSuggested {
        conversation_id: Uuid,
        parent_message_id: Option<Uuid>,
        suggestion_title: String,
        confidence: f32,
        reason: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// A branch was created
    BranchCreated {
        conversation_id: Uuid,
        branch_id: Uuid,
        title: String,
        parent_message_id: Option<Uuid>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    // Tool lifecycle and streaming events for chat UI cards
    /// A tool run has started
    ToolRunStarted {
        run_id: ToolRunId,
        tool: String,
    },

    /// A tool run has completed
    ToolRunCompleted {
        run_id: ToolRunId,
        tool: String,
        success: bool,
    },

    /// Streaming event from a tool run
    ToolStream {
        run_id: ToolRunId,
        event: String, // Simplified from StreamEvent
    },
}

/// Event handler for agent events
#[derive(Clone)]
pub struct EventHandler {
    event_sender: broadcast::Sender<AgentEvent>,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new(event_sender: broadcast::Sender<AgentEvent>) -> Self {
        Self { event_sender }
    }

    /// Start listening for tool execution events
    pub fn start_tool_event_listener(
        &self,
        mut tool_event_receiver: mpsc::Receiver<ToolExecutionEvent>,
        history: Arc<ConversationAwareHistoryManager>,
    ) {
        let event_sender = self.event_sender.clone();
            
        tokio::spawn(async move {
            info!("Tool event listener task started.");
            while let Some(event) = tool_event_receiver.recv().await {
                debug!("Received tool execution event: {event:?}");
                match event {
                    ToolExecutionEvent::Started { tool_call_id, tool_name, parameters } => {
                        info!("Tool call ID: {tool_call_id} ({tool_name}) started. Args: {parameters:?}");
                    },
                    ToolExecutionEvent::Completed { tool_call_id, tool_name, result } => {
                        info!("Tool execution completed: {} (success: {})", tool_call_id, result.is_success());
                        
                        // Add the tool result to history as a function role message
                        let tool_result_message = AgentMessage {
                            id: Uuid::new_v4(),
                            role: Role::Function,
                            content: String::new(),
                            is_streaming: false,
                            timestamp: chrono::Utc::now(),
                            metadata: Default::default(),
                            tool_calls: vec![crate::agent::message::types::ToolCall {
                                id: tool_call_id.clone(),
                                name: tool_name.clone(),
                                arguments: serde_json::json!({}),
                                result: Some(serde_json::to_value(&result).unwrap_or_default()),
                                successful: result.is_success(),
                                execution_time: Some(chrono::Utc::now()),
                            }],
                        };
                        history.add_message(tool_result_message).await;
                        
                        // Emit tool completion event
                        let _ = event_sender.send(AgentEvent::ToolCallComplete {
                            tool_call_id: tool_call_id.clone(),
                            tool_name: tool_name.clone(),
                            result: result.clone(),
                        });
                        
                        // CRITICAL FIX: Remove duplicate continuation logic
                        // The unified reasoning stream already handles continuation after tool completion
                        // This was causing duplicate LLM calls to Gemini
                        info!("Tool result added to history. Unified reasoning stream will handle continuation.");
                    },
                    ToolExecutionEvent::Failed { tool_call_id, tool_name, error } => {
                        warn!("Tool call ID: {tool_call_id} ({tool_name}) failed. Error: {error}");
                        // Add the error to the message history
                        let error_value = serde_json::json!({ "error": error });
                        debug!("Serialized tool error for history: {error_value:?}");
                        let _ = history.add_tool_result(&tool_call_id, error_value.clone(), false).await;
                        debug!("Added tool error for ID: {tool_call_id} to history. Error value: {error_value:?}");
                        
                        // --- PATCH: Add a new function response message for Gemini on error ---
                        let mut function_response_message = AgentMessage {
                            id: Uuid::new_v4(),
                            role: Role::Function,
                            content: String::new(),
                            is_streaming: false,
                            timestamp: chrono::Utc::now(),
                            metadata: Default::default(),
                            tool_calls: Vec::new(),
                        };

                        // Wrap the error output in a {"content": ...} structure
                        let response_payload_for_gemini = serde_json::json!({ "content": error_value });

                        function_response_message.tool_calls.push(crate::agent::message::types::ToolCall {
                            id: tool_call_id.clone(),
                            name: tool_name.clone(),
                            arguments: serde_json::json!({}),
                            result: Some(response_payload_for_gemini), // The wrapped error
                            successful: false,
                            execution_time: Some(chrono::Utc::now()),
                        });
                        history.add_message(function_response_message).await;
                        debug!("Added Function role message to history for failed tool: {tool_name}");
                        // --- END PATCH ---
                        
                        // Emit an event about the tool failure
                        let _ = event_sender.send(AgentEvent::ToolCallComplete {
                            tool_call_id,
                            tool_name,
                            result: ToolResult::Error { error },
                        });
                    },
                }
            }
        });
    }
    
    /// Start listening for state events
    pub fn start_state_event_listener(&self, state_manager: Arc<StateManager>) {
        let mut receiver = state_manager.subscribe();
        let event_sender = self.event_sender.clone();
        
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                match event {
                    StateEvent::StateChanged { transition } => {
                        let _ = event_sender.send(AgentEvent::StateChanged(transition.to_state));
                    },
                    StateEvent::ConversationStatusChanged { to, .. } => {
                        let _ = event_sender.send(AgentEvent::ConversationStatusChanged(to));
                    },
                    _ => {}, // Ignore other events
                }
            }
        });
    }

    /// Subscribe to agent events
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.event_sender.subscribe()
    }
} 