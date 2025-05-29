// State data structures will go here

// ... file removed, all types now imported from fred-engine ...

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc}; // Import Utc
use std::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentMode {
    ChatOnly,
    ToolsWithConfirmation,
    FullyAutonomous,
}

impl Default for AgentMode {
    fn default() -> Self {
        AgentMode::ToolsWithConfirmation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Idle,
    Thinking { message: String },
    Responding { is_streaming: bool, step_info: Option<String> },
    ExecutingTool { tool_call_id: String, tool_name: String },
    InLoop { step: u32, interruptible: bool },
    Error { message: String, details: Option<String> }, // Added details field
}

impl Default for AgentState {
    fn default() -> Self {
        AgentState::Idle
    }
}

// Add other necessary state-related types here if they were previously in this file
// e.g., ConversationStatus, AgentStateInfo, StateTransition if they are truly agent-level concepts
// For now, focusing on AgentMode and AgentState as they are widely used.

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ConversationStatus {
    Active,
    Archived,
    Summarizing,
    Paused,
    Completed,
}

impl Default for ConversationStatus {
    fn default() -> Self {
        ConversationStatus::Active
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentStateInfo {
    pub current_mode: AgentMode,
    pub current_state: AgentState, // Renamed from current_status for clarity with AgentState enum
    pub conversation_status: ConversationStatus, // Added for distinct conversation status
    pub thinking_process: Option<String>,
    pub last_error: Option<String>,
    pub typing_indicator: bool,        // Added
    pub transitions: Vec<StateTransition>, // Added
}

// StateTransition might be an event rather than a persisted type, or part of a state machine logic.
// Placeholder for now if it's a concrete type that needs to be resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from_state: AgentState, // Changed to AgentState enum
    pub to_state: AgentState,   // Changed to AgentState enum
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

impl Default for StateTransition {
    fn default() -> Self {
        Self {
            from_state: AgentState::Idle,
            to_state: AgentState::Idle,
            reason: "Default transition".to_string(),
            timestamp: Utc::now(),
        }
    }
}

