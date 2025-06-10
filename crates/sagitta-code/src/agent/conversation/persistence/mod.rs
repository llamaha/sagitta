pub mod disk;
pub mod index;

use async_trait::async_trait;
use anyhow::Result;
use uuid::Uuid;

use super::types::{Conversation, ConversationSummary};

/// Trait for conversation persistence
#[async_trait]
pub trait ConversationPersistence: Send + Sync {
    /// Save a conversation to persistent storage
    async fn save_conversation(&self, conversation: &Conversation) -> Result<()>;
    
    /// Load a conversation from persistent storage
    async fn load_conversation(&self, id: Uuid) -> Result<Option<Conversation>>;
    
    /// Delete a conversation from persistent storage
    async fn delete_conversation(&self, id: Uuid) -> Result<()>;
    
    /// List all conversation IDs
    async fn list_conversation_ids(&self) -> Result<Vec<Uuid>>;
    
    /// Load conversation summaries from index (Phase 2: for lazy loading)
    async fn list_conversation_summaries(&self, workspace_id: Option<Uuid>) -> Result<Vec<ConversationSummary>>;
    
    /// Archive a conversation (move to archive storage)
    async fn archive_conversation(&self, id: Uuid) -> Result<()>;
    
    /// List archived conversation IDs
    async fn list_archived_conversation_ids(&self) -> Result<Vec<Uuid>>;
    
    /// Restore a conversation from archive
    async fn restore_conversation(&self, id: Uuid) -> Result<()>;
}

// Mock implementation for testing
#[cfg(test)]
mockall::mock! {
    pub ConversationPersistence {}
    
    #[async_trait]
    impl ConversationPersistence for ConversationPersistence {
        async fn save_conversation(&self, conversation: &Conversation) -> Result<()>;
        async fn load_conversation(&self, id: Uuid) -> Result<Option<Conversation>>;
        async fn delete_conversation(&self, id: Uuid) -> Result<()>;
        async fn list_conversation_ids(&self) -> Result<Vec<Uuid>>;
        async fn list_conversation_summaries(&self, workspace_id: Option<Uuid>) -> Result<Vec<ConversationSummary>>;
        async fn archive_conversation(&self, id: Uuid) -> Result<()>;
        async fn list_archived_conversation_ids(&self) -> Result<Vec<Uuid>>;
        async fn restore_conversation(&self, id: Uuid) -> Result<()>;
    }
} 