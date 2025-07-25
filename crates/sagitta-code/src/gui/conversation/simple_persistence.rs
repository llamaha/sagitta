// Simple, direct persistence for conversations
use super::simple_types::{SimplifiedConversation, PersistedMessage, PersistedToolCard, PersistedToolCardStatus};
use crate::gui::chat::{StreamingMessage, ToolCard, ChatItem, ToolCardStatus};
use crate::agent::events::ToolRunId;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use uuid::Uuid;
use anyhow::{Result, Context};

pub struct SimplePersistence {
    storage_dir: PathBuf,
}

impl SimplePersistence {
    pub fn new(storage_dir: PathBuf) -> Result<Self> {
        // Ensure storage directory exists
        fs::create_dir_all(&storage_dir)?;
        Ok(Self { storage_dir })
    }
    
    /// Get the file path for a conversation
    fn conversation_path(&self, id: Uuid) -> PathBuf {
        self.storage_dir.join(format!("{}.json", id))
    }
    
    /// Save a conversation - direct serialization, no conversion
    pub fn save_conversation(&self, conversation: &SimplifiedConversation) -> Result<()> {
        let path = self.conversation_path(conversation.id);
        let json = serde_json::to_string_pretty(conversation)?;
        
        // Atomic write: write to temp file, then rename
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, json)?;
        fs::rename(temp_path, path)?;
        
        Ok(())
    }
    
    /// Load a conversation - direct deserialization, no conversion
    pub fn load_conversation(&self, id: Uuid) -> Result<SimplifiedConversation> {
        let path = self.conversation_path(id);
        let json = fs::read_to_string(path)
            .with_context(|| format!("Failed to read conversation {}", id))?;
        let conversation = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse conversation {}", id))?;
        Ok(conversation)
    }
    
    /// List all conversations (id and title only for performance)
    pub fn list_conversations(&self) -> Result<Vec<(Uuid, String, chrono::DateTime<chrono::Utc>)>> {
        let mut conversations = Vec::new();
        
        for entry in fs::read_dir(&self.storage_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(id_str) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(id) = Uuid::parse_str(id_str) {
                        // Read just enough to get title and last_active
                        if let Ok(json) = fs::read_to_string(&path) {
                            if let Ok(conv) = serde_json::from_str::<SimplifiedConversation>(&json) {
                                conversations.push((id, conv.title, conv.last_active));
                            }
                        }
                    }
                }
            }
        }
        
        // Sort by last active, newest first
        conversations.sort_by(|a, b| b.2.cmp(&a.2));
        Ok(conversations)
    }
    
    /// Delete a conversation
    pub fn delete_conversation(&self, id: Uuid) -> Result<()> {
        let path = self.conversation_path(id);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
    
    /// Check if a conversation exists
    pub fn conversation_exists(&self, id: Uuid) -> bool {
        self.conversation_path(id).exists()
    }
}

/// Convert from StreamingMessage to PersistedMessage
impl PersistedMessage {
    pub fn from_streaming_message(msg: &StreamingMessage, tool_cards: &[ToolCard], tool_card_ui_states: &HashMap<String, (String, bool)>) -> Self {
        // For now, we'll store all tool cards since we can't easily determine which belong to this message
        // In a real implementation, you'd want to track this relationship properly
        let message_tool_cards: Vec<PersistedToolCard> = tool_cards
            .iter()
            .map(|card| {
                // Get UI state for this tool card
                let (content, is_collapsed) = tool_card_ui_states
                    .get(&card.run_id.to_string())
                    .cloned()
                    .unwrap_or_else(|| ("".to_string(), false));
                    
                PersistedToolCard::from_tool_card(card, content, is_collapsed)
            })
            .collect();
        
        Self {
            id: msg.id.parse().unwrap_or_else(|_| Uuid::new_v4()),
            author: msg.author.clone(),
            content: msg.content.clone(),
            timestamp: msg.timestamp,
            status: msg.status.clone(),
            tool_cards: message_tool_cards,
            is_collapsed: false, // Default, can be updated from UI state
            tool_cards_collapsed_state: HashMap::new(), // Default, can be updated from UI state
        }
    }
    
    /// Convert back to StreamingMessage
    pub fn to_streaming_message(&self) -> StreamingMessage {
        StreamingMessage::from_text(self.author.clone(), self.content.clone())
    }
}

impl PersistedToolCard {
    pub fn from_tool_card(card: &ToolCard, content: String, is_collapsed: bool) -> Self {
        let status = match &card.status {
            ToolCardStatus::Running => PersistedToolCardStatus::Running,
            ToolCardStatus::Completed { success } => PersistedToolCardStatus::Completed { success: *success },
            ToolCardStatus::Cancelled => PersistedToolCardStatus::Cancelled,
            ToolCardStatus::Failed { error } => PersistedToolCardStatus::Failed { error: error.clone() },
        };
        
        Self {
            run_id: card.run_id.to_string(),
            tool_name: card.tool_name.clone(),
            status,
            progress: card.progress,
            logs: card.logs.clone(),
            started_at: card.started_at,
            completed_at: card.completed_at,
            input_params: card.input_params.clone(),
            result: card.result.clone(),
            is_collapsed,
            content,
        }
    }
    
    pub fn to_tool_card(&self) -> ToolCard {
        let status = match &self.status {
            PersistedToolCardStatus::Running => ToolCardStatus::Running,
            PersistedToolCardStatus::Completed { success } => ToolCardStatus::Completed { success: *success },
            PersistedToolCardStatus::Cancelled => ToolCardStatus::Cancelled,
            PersistedToolCardStatus::Failed { error } => ToolCardStatus::Failed { error: error.clone() },
        };
        
        // Try to parse the run_id, or generate a new one
        let run_id = self.run_id.parse::<ToolRunId>()
            .unwrap_or_else(|_| ToolRunId::new_v4());
        
        ToolCard {
            run_id,
            tool_name: self.tool_name.clone(),
            status,
            progress: self.progress,
            logs: self.logs.clone(),
            started_at: self.started_at,
            completed_at: self.completed_at,
            input_params: self.input_params.clone(),
            result: self.result.clone(),
        }
    }
}