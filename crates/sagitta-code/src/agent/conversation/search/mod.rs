pub mod text;
pub mod semantic;
pub mod filters;

use async_trait::async_trait;
use anyhow::Result;
use uuid::Uuid;

use super::types::{Conversation, ConversationQuery, ConversationSearchResult};

/// Trait for conversation search engine
#[async_trait]
pub trait ConversationSearchEngine: Send + Sync {
    /// Index a conversation for search
    async fn index_conversation(&self, conversation: &Conversation) -> Result<()>;
    
    /// Remove a conversation from the search index
    async fn remove_conversation(&self, id: Uuid) -> Result<()>;
    
    /// Search conversations based on query
    async fn search(&self, query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>>;
    
    /// Clear all search indices
    async fn clear_index(&self) -> Result<()>;
    
    /// Rebuild the entire search index
    async fn rebuild_index(&self, conversations: &[Conversation]) -> Result<()>;
}

// Mock implementation for testing
#[cfg(test)]
mockall::mock! {
    pub ConversationSearchEngine {}
    
    #[async_trait]
    impl ConversationSearchEngine for ConversationSearchEngine {
        async fn index_conversation(&self, conversation: &Conversation) -> Result<()>;
        async fn remove_conversation(&self, id: Uuid) -> Result<()>;
        async fn search(&self, query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>>;
        async fn clear_index(&self) -> Result<()>;
        async fn rebuild_index(&self, conversations: &[Conversation]) -> Result<()>;
    }
} 