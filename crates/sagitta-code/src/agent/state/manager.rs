// State transitions and updates will go here

use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use serde::Serialize;

use crate::agent::state::types::{AgentState, AgentMode, ConversationStatus, AgentStateInfo, StateTransition};
use crate::utils::errors::SagittaCodeError;

/// Maximum number of state change events to buffer
const MAX_EVENTS: usize = 100;

/// Events emitted by the state manager
#[derive(Debug, Clone, Serialize)]
pub enum StateEvent {
    /// The agent state has changed
    StateChanged {
        /// The state transition that occurred
        transition: StateTransition,
    },
    
    /// The agent mode has changed
    ModeChanged {
        /// The previous mode
        from: AgentMode,
        
        /// The new mode
        to: AgentMode,
    },
    
    /// The conversation status has changed
    ConversationStatusChanged {
        /// The previous status
        from: ConversationStatus,
        
        /// The new status
        to: ConversationStatus,
    },
}

/// Manages the state of the agent and emits events when it changes
#[derive(Debug, Clone)]
pub struct StateManager {
    /// The current state info
    state: Arc<RwLock<AgentStateInfo>>,
    
    /// Channel for state change events
    event_sender: broadcast::Sender<StateEvent>,
}

impl StateManager {
    /// Create a new state manager with the default state
    pub fn new() -> Self {
        let (event_sender, _) = broadcast::channel(MAX_EVENTS);
        
        Self {
            state: Arc::new(RwLock::new(AgentStateInfo::default())),
            event_sender,
        }
    }
    
    /// Create a new state manager with a specific state
    pub fn with_state(state: AgentStateInfo) -> Self {
        let (event_sender, _) = broadcast::channel(MAX_EVENTS);
        
        Self {
            state: Arc::new(RwLock::new(state)),
            event_sender,
        }
    }
    
    /// Subscribe to state events
    pub fn subscribe(&self) -> broadcast::Receiver<StateEvent> {
        self.event_sender.subscribe()
    }
    
    /// Get the current state info
    pub async fn get_state(&self) -> AgentStateInfo {
        let state = self.state.read().await;
        state.clone()
    }
    
    /// Get the current agent state
    pub async fn get_agent_state(&self) -> AgentState {
        let state = self.state.read().await;
        state.current_state.clone()
    }
    
    /// Get the current agent mode
    pub async fn get_agent_mode(&self) -> AgentMode {
        let state = self.state.read().await;
        state.current_mode.clone()
    }
    
    /// Get the current conversation status
    pub async fn get_conversation_status(&self) -> ConversationStatus {
        let state = self.state.read().await;
        state.conversation_status.clone()
    }
    
    /// Set the agent state and emit an event
    pub async fn set_agent_state(&self, new_state: AgentState, reason: impl Into<String>) -> Result<(), SagittaCodeError> {
        let reason_str = reason.into();
        let mut state = self.state.write().await;
        
        let transition = StateTransition {
            from_state: state.current_state.clone(),
            to_state: new_state.clone(),
            reason: reason_str,
            timestamp: chrono::Utc::now(),
        };
        
        state.transitions.push(transition.clone());
        state.current_state = new_state;
        
        // Emit event
        let _ = self.event_sender.send(StateEvent::StateChanged {
            transition,
        });
        
        Ok(())
    }
    
    /// Set the agent to idle state
    pub async fn set_idle(&self, reason: impl Into<String>) -> Result<(), SagittaCodeError> {
        self.set_agent_state(AgentState::Idle, reason).await
    }
    
    /// Set the agent to thinking state
    pub async fn set_thinking(&self, reason: impl Into<String>) -> Result<(), SagittaCodeError> {
        let reason_str = reason.into();
        self.set_agent_state(AgentState::Thinking { message: reason_str.clone() }, reason_str).await
    }
    
    /// Set the agent to responding state
    pub async fn set_responding(&self, streaming: bool, reason: impl Into<String>) -> Result<(), SagittaCodeError> {
        self.set_agent_state(AgentState::Responding { is_streaming: streaming, step_info: None }, reason).await
    }
    
    /// Set the agent to executing tool state
    pub async fn set_executing_tool(&self, tool_call_id: impl Into<String>, tool_name: impl Into<String>, reason: impl Into<String>) -> Result<(), SagittaCodeError> {
        self.set_agent_state(
            AgentState::ExecutingTool {
                tool_call_id: tool_call_id.into(),
                tool_name: tool_name.into(),
            },
            reason
        ).await
    }
    
    /// Set the agent to error state
    pub async fn set_error(&self, message: impl Into<String>, reason: impl Into<String>) -> Result<(), SagittaCodeError> {
        self.set_agent_state(
            AgentState::Error {
                message: message.into(),
                details: None,
            },
            reason
        ).await
    }
    
    /// Set the agent mode and emit an event
    pub async fn set_agent_mode(&self, mode: AgentMode) -> Result<(), SagittaCodeError> {
        let mut state = self.state.write().await;
        
        let old_mode = state.current_mode.clone();
        state.current_mode = mode.clone();
        
        // Emit event
        let _ = self.event_sender.send(StateEvent::ModeChanged {
            from: old_mode,
            to: mode,
        });
        
        Ok(())
    }
    
    /// Set the conversation status and emit an event
    pub async fn set_conversation_status(&self, status: ConversationStatus) -> Result<(), SagittaCodeError> {
        let mut state = self.state.write().await;
        
        let old_status = state.conversation_status.clone();
        state.conversation_status = status.clone();
        
        // Emit event
        let _ = self.event_sender.send(StateEvent::ConversationStatusChanged {
            from: old_status,
            to: status,
        });
        
        Ok(())
    }
    
    /// Mark the conversation as failed
    pub async fn set_conversation_failed(&self, _error: impl Into<String>) -> Result<(), SagittaCodeError> {
        self.set_conversation_status(ConversationStatus::Paused).await
    }
    
    /// Set whether typing indicators are enabled
    pub async fn set_typing_indicator(&self, enabled: bool) -> Result<(), SagittaCodeError> {
        let mut state = self.state.write().await;
        state.typing_indicator = enabled;
        Ok(())
    }
    
    /// Get whether typing indicators are enabled
    pub async fn get_typing_indicator(&self) -> bool {
        let state = self.state.read().await;
        state.typing_indicator
    }
    
    /// Get the state transition history
    pub async fn get_transitions(&self) -> Vec<StateTransition> {
        let state = self.state.read().await;
        state.transitions.clone()
    }
    
    /// Clear the state transition history
    pub async fn clear_transitions(&self) -> Result<(), SagittaCodeError> {
        let mut state = self.state.write().await;
        state.transitions.clear();
        Ok(())
    }

    // Added getter for the internal state Arc
    pub fn get_state_arc(&self) -> Arc<RwLock<AgentStateInfo>> {
        Arc::clone(&self.state)
    }
}

