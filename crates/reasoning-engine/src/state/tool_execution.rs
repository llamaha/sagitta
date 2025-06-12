//! Tool execution state management and caching

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::traits::ToolResult;
use super::task_completion::CompletionSignal;

/// Tool execution state caching to prevent re-execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionCache {
    /// Successfully executed tools with their results
    pub successful_executions: HashMap<String, CachedToolResult>,
    /// Failed execution attempts for learning
    pub failed_attempts: Vec<FailedExecution>,
    /// Signals that indicate task completion
    pub completion_signals: Vec<CompletionSignal>,
}

/// Cached result from successful tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedToolResult {
    /// Tool name
    pub tool_name: String,
    /// Input arguments used
    pub args: Value,
    /// Result from the tool
    pub result: ToolResult,
    /// When this was cached
    pub cached_at: DateTime<Utc>,
    /// How many times this has been referenced
    pub reference_count: u32,
}

/// Record of a failed tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedExecution {
    /// Tool that failed
    pub tool_name: String,
    /// Arguments used
    pub args: Value,
    /// Error message
    pub error: String,
    /// When it failed
    pub failed_at: DateTime<Utc>,
    /// Whether to retry or avoid
    pub should_retry: bool,
}

/// Tool execution state management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionState {
    /// Current execution cache
    pub cache: ToolExecutionCache,
    /// Tools currently being executed
    pub active_executions: HashMap<String, DateTime<Utc>>,
    /// Tools that should not be retried
    pub blocked_tools: HashSet<String>,
    /// Execution history for this session
    pub execution_history: Vec<ToolExecutionRecord>,
}

/// Record of a tool execution attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRecord {
    /// Tool name
    pub tool_name: String,
    /// Arguments used
    pub args: Value,
    /// Whether it succeeded
    pub success: bool,
    /// Result or error message
    pub result_or_error: String,
    /// When it was executed
    pub executed_at: DateTime<Utc>,
    /// Step ID that triggered execution
    pub step_id: Uuid,
}

impl Default for ToolExecutionState {
    fn default() -> Self {
        Self {
            cache: ToolExecutionCache::default(),
            active_executions: HashMap::new(),
            blocked_tools: HashSet::new(),
            execution_history: Vec::new(),
        }
    }
}

impl Default for ToolExecutionCache {
    fn default() -> Self {
        Self {
            successful_executions: HashMap::new(),
            failed_attempts: Vec::new(),
            completion_signals: Vec::new(),
        }
    }
} 