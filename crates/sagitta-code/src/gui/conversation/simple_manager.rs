// Simplified conversation manager - synchronous and direct
use super::simple_types::{SimplifiedConversation, PersistedMessage};
use super::simple_persistence::SimplePersistence;
use crate::gui::chat::{StreamingChatManager, StreamingMessage};
use std::path::PathBuf;
use std::collections::HashMap;
use uuid::Uuid;
use anyhow::Result;
use std::sync::{Arc, Mutex};

pub struct SimpleConversationManager {
    persistence: SimplePersistence,
    current_conversation: Option<SimplifiedConversation>,
    chat_manager: Arc<StreamingChatManager>,
}

impl SimpleConversationManager {
    pub fn new(storage_dir: PathBuf, chat_manager: Arc<StreamingChatManager>) -> Result<Self> {
        let persistence = SimplePersistence::new(storage_dir)?;
        Ok(Self {
            persistence,
            current_conversation: None,
            chat_manager,
        })
    }
    
    /// Create a new conversation
    pub fn create_conversation(&mut self, title: String) -> Result<Uuid> {
        let conversation = SimplifiedConversation::new(title);
        let id = conversation.id;
        
        // Save immediately
        self.persistence.save_conversation(&conversation)?;
        
        // Set as current
        self.current_conversation = Some(conversation);
        
        // Clear chat manager
        self.chat_manager.clear_all_messages();
        
        Ok(id)
    }
    
    /// Switch to a different conversation - synchronous and direct
    pub fn switch_conversation(&mut self, id: Uuid) -> Result<()> {
        // Save current conversation if there is one
        if let Some(_current) = &self.current_conversation {
            self.save_current_conversation()?;
        }
        
        // Load the new conversation
        let conversation = self.persistence.load_conversation(id)?;
        
        // Clear and restore chat manager state
        self.chat_manager.clear_all_messages();
        
        // Restore all messages and tool cards
        for persisted_msg in &conversation.messages {
            // Convert to streaming message
            let msg = persisted_msg.to_streaming_message();
            
            // Add the complete message
            self.chat_manager.add_complete_message(msg);
            
            // Restore tool cards for this message
            for persisted_card in &persisted_msg.tool_cards {
                let tool_card = persisted_card.to_tool_card();
                self.chat_manager.restore_tool_card_direct(tool_card);
            }
        }
        
        self.current_conversation = Some(conversation);
        Ok(())
    }
    
    /// Save the current conversation state
    pub fn save_current_conversation(&mut self) -> Result<()> {
        if let Some(conversation) = &mut self.current_conversation {
            // Update messages from chat manager
            let messages = self.chat_manager.get_all_messages();
            let tool_cards = self.chat_manager.get_all_tool_cards();
            
            // TODO: Get actual tool card UI states from the UI
            let tool_card_ui_states = HashMap::new();
            
            conversation.messages = messages
                .into_iter()
                .map(|msg| PersistedMessage::from_streaming_message(&msg, &tool_cards, &tool_card_ui_states))
                .collect();
            
            conversation.update_last_active();
            
            // Save to disk
            self.persistence.save_conversation(conversation)?;
        }
        Ok(())
    }
    
    /// Get list of all conversations
    pub fn list_conversations(&self) -> Result<Vec<(Uuid, String, chrono::DateTime<chrono::Utc>)>> {
        self.persistence.list_conversations()
    }
    
    /// Delete a conversation
    pub fn delete_conversation(&mut self, id: Uuid) -> Result<()> {
        // If deleting current conversation, clear it
        if self.current_conversation.as_ref().map(|c| c.id) == Some(id) {
            self.current_conversation = None;
            self.chat_manager.clear_all_messages();
        }
        
        self.persistence.delete_conversation(id)
    }
    
    /// Get current conversation ID
    pub fn current_conversation_id(&self) -> Option<Uuid> {
        self.current_conversation.as_ref().map(|c| c.id)
    }
    
    /// Update current conversation title
    pub fn update_conversation_title(&mut self, title: String) -> Result<()> {
        if let Some(conversation) = &mut self.current_conversation {
            conversation.title = title;
            self.save_current_conversation()?;
        }
        Ok(())
    }
    
    /// Add a message to current conversation
    pub fn add_message(&mut self, _message: StreamingMessage) -> Result<()> {
        if let Some(conversation) = &mut self.current_conversation {
            conversation.update_last_active();
            // Messages are saved when we call save_current_conversation
            // The actual message is stored in the chat manager
        }
        Ok(())
    }
}