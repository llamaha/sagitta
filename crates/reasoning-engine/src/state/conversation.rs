//! Conversation context and phase management

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::{
    session::SessionSummary,
    task_completion::TaskCompletion,
    tool_execution::ToolExecutionState,
};

/// Enhanced conversation phase management
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConversationPhase {
    /// Starting a new conversation
    Fresh,
    /// Continuing an ongoing conversation
    Ongoing,
    /// Investigating a specific topic
    Investigating { topic: String },
    /// Working on a specific task
    TaskFocused { task: String },
    /// Task execution in progress
    TaskExecution { task: String, progress: f32 },
    /// Task completed successfully
    TaskCompleted { task: String, completion_marker: String },
    /// Waiting for user clarification
    AwaitingClarification,
    /// Follow-up question about completed task
    FollowUpQuestion { completed_task: String },
    /// New task request after completion
    NewTaskRequest,
    /// Completed successfully (for backward compatibility)
    Completed,
    /// Planning phase
    Planning,
    /// Review phase
    Review,
    /// Completion phase
    Completion,
}

/// Conversation context for maintaining state across reasoning sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    /// The conversation ID this reasoning session belongs to
    pub conversation_id: Option<Uuid>,
    
    /// Previous reasoning sessions in this conversation
    pub previous_sessions: Vec<SessionSummary>,
    
    /// Accumulated knowledge from previous sessions
    pub accumulated_knowledge: HashMap<String, Value>,
    
    /// Tools that have been used successfully in this conversation
    pub successful_tools: HashSet<String>,
    
    /// Patterns that have been effective in this conversation
    pub effective_patterns: Vec<String>,
    
    /// Current conversation phase
    pub conversation_phase: ConversationPhase,

    /// Task completion tracking
    pub completed_tasks: Vec<TaskCompletion>,

    /// Tool execution state for this conversation
    pub tool_execution_state: ToolExecutionState,
}

/// Legacy conversation state enum (kept for backward compatibility)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConversationState {
    /// Starting a new conversation
    Fresh,
    /// Continuing an ongoing conversation
    Ongoing,
    /// Investigating a specific topic
    Investigating { topic: String },
    /// Working on a specific task
    TaskFocused { task: String },
    /// Waiting for user clarification
    AwaitingClarification,
    /// Completed successfully
    Completed,
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self {
            conversation_id: None,
            previous_sessions: Vec::new(),
            accumulated_knowledge: HashMap::new(),
            successful_tools: HashSet::new(),
            effective_patterns: Vec::new(),
            conversation_phase: ConversationPhase::Fresh,
            completed_tasks: Vec::new(),
            tool_execution_state: ToolExecutionState::default(),
        }
    }
} 