//! Session metadata and summary management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Session metadata for tracking across multiple reasoning calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Whether this session is a continuation of a previous one
    pub is_continuation: bool,
    
    /// ID of the previous session if this is a continuation
    pub previous_session_id: Option<Uuid>,
    
    /// Number of iterations completed in this session
    pub iterations_completed: u32,
    
    /// Total reasoning time across all sessions in this conversation
    pub total_reasoning_time: Duration,
    
    /// Success indicators from previous sessions
    pub previous_success_indicators: Vec<String>,
    
    /// Failure patterns to avoid
    pub failure_patterns: Vec<String>,
}

/// Summary of a completed reasoning session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: Uuid,
    pub input: String,
    pub result: Option<String>,
    pub success: bool,
    pub tools_used: Vec<String>,
    pub key_insights: Vec<String>,
    pub duration: Duration,
    pub completed_at: DateTime<Utc>,
}

impl Default for SessionMetadata {
    fn default() -> Self {
        Self {
            is_continuation: false,
            previous_session_id: None,
            iterations_completed: 0,
            total_reasoning_time: Duration::from_secs(0),
            previous_success_indicators: Vec::new(),
            failure_patterns: Vec::new(),
        }
    }
} 