//! State management for the reasoning engine

pub mod core;
pub mod step;
pub mod goal;
pub mod decision;
pub mod streaming;
pub mod session;
pub mod conversation;
pub mod tool_execution;
pub mod task_completion;

// Re-export main types for backward compatibility
pub use core::{ReasoningState, ReasoningContext, ReasoningMode, ProjectContext, WorkingMemoryItem};
pub use step::{ReasoningStep, StepType, StepInput, StepOutput};
pub use goal::{Goal, SubGoal, SubGoalStatus, CompletedGoal};
pub use decision::{DecisionPoint, DecisionOption, StateCheckpoint};
pub use streaming::{StreamingState, StreamInfo, StreamChunk, StreamError, BackpressureSignal};
pub use session::{SessionMetadata, SessionSummary};
pub use conversation::{ConversationContext, ConversationPhase};
pub use tool_execution::{
    ToolExecutionState, ToolExecutionCache, CachedToolResult, 
    FailedExecution, ToolExecutionRecord
};
pub use task_completion::{
    TaskCompletion, CompletionSignal, CompletionSignalType, TaskCompletionAnalyzer
}; 