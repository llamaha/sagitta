// Simplified conversation types for direct persistence
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::gui::chat::{MessageAuthor, MessageStatus};

/// Simplified conversation structure - just the essentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplifiedConversation {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub messages: Vec<PersistedMessage>,
}

/// Message that directly mirrors UI state - saved exactly as displayed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedMessage {
    pub id: Uuid,
    pub author: MessageAuthor,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub status: MessageStatus,
    pub tool_cards: Vec<PersistedToolCard>,
    // UI state preserved
    pub is_collapsed: bool,
    pub tool_cards_collapsed_state: HashMap<String, bool>,
}

/// Tool card with its exact state - matches the actual ToolCard structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedToolCard {
    pub run_id: String, // Serialized ToolRunId
    pub tool_name: String,
    pub status: PersistedToolCardStatus,
    pub progress: Option<f32>,
    pub logs: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub input_params: serde_json::Value,
    pub result: Option<serde_json::Value>,
    // UI state
    pub is_collapsed: bool,
    pub content: String, // The rendered content
}

/// Serializable version of ToolCardStatus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PersistedToolCardStatus {
    Running,
    Completed { success: bool },
    Cancelled,
    Failed { error: String },
}

impl SimplifiedConversation {
    pub fn new(title: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title,
            created_at: now,
            last_active: now,
            messages: Vec::new(),
        }
    }
    
    pub fn update_last_active(&mut self) {
        self.last_active = Utc::now();
    }
}