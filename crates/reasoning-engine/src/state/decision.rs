//! Decision point and checkpoint management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Decision point encountered during reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPoint {
    /// Unique decision identifier
    pub id: Uuid,
    /// Decision description
    pub description: String,
    /// Available options
    pub options: Vec<DecisionOption>,
    /// Chosen option
    pub chosen_option: Option<String>,
    /// Confidence in the decision
    pub confidence: f32,
    /// Reasoning for the decision
    pub reasoning: String,
    /// When this decision was made
    pub timestamp: DateTime<Utc>,
}

/// Option available at a decision point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOption {
    /// Option identifier
    pub id: String,
    /// Option description
    pub description: String,
    /// Estimated cost/effort
    pub estimated_cost: f32,
    /// Estimated benefit
    pub estimated_benefit: f32,
    /// Risk level (0.0 to 1.0)
    pub risk_level: f32,
    /// Prerequisites for this option
    pub prerequisites: Vec<String>,
}

/// State checkpoint for backtracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCheckpoint {
    /// Unique checkpoint identifier
    pub id: Uuid,
    /// Checkpoint description
    pub description: String,
    /// When this checkpoint was created
    pub created_at: DateTime<Utc>,
    /// Reasoning state at this point (serialized)
    pub state_snapshot: Vec<u8>,
    /// Step that created this checkpoint
    pub step_id: Uuid,
    /// Confidence at this checkpoint
    pub confidence: f32,
} 