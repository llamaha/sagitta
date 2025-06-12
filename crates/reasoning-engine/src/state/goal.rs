//! Goal and sub-goal management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// A goal in the reasoning process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    /// Unique goal identifier
    pub id: Uuid,
    /// Goal description
    pub description: String,
    /// Goal priority (higher = more important)
    pub priority: u32,
    /// Estimated complexity (1-10)
    pub complexity: u32,
    /// Required tools for this goal
    pub required_tools: Vec<String>,
    /// Success criteria
    pub success_criteria: Vec<String>,
    /// When this goal was created
    pub created_at: DateTime<Utc>,
    /// Deadline if any
    pub deadline: Option<DateTime<Utc>>,
}

/// A sub-goal that contributes to a larger goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubGoal {
    /// Unique sub-goal identifier
    pub id: Uuid,
    /// Parent goal identifier
    pub parent_goal: Uuid,
    /// Sub-goal description
    pub description: String,
    /// Dependencies on other sub-goals
    pub dependencies: Vec<Uuid>,
    /// Estimated effort (1-10)
    pub effort: u32,
    /// Current status
    pub status: SubGoalStatus,
}

/// Status of a sub-goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubGoalStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Blocked,
}

/// A completed goal with its results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedGoal {
    /// Original goal
    pub goal: Goal,
    /// Final result
    pub result: Value,
    /// Success status
    pub success: bool,
    /// Completion time
    pub completed_at: DateTime<Utc>,
    /// Steps taken to complete this goal
    pub steps_taken: Vec<Uuid>,
    /// Lessons learned
    pub lessons_learned: Vec<String>,
} 