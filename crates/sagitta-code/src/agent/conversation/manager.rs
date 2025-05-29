use async_trait::async_trait;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::types::{
    Conversation, ConversationBranch, ConversationCheckpoint, ConversationSummary,
    ConversationQuery, ConversationSearchResult, ContextSnapshot,
};
use super::persistence::ConversationPersistence;
use super::search::ConversationSearchEngine;
use crate::agent::state::types::ConversationStatus;
use crate::agent::conversation::types::BranchStatus;
use crate::agent::events::AgentEvent;
use crate::config::types::ConversationConfig;

/// Trait for managing conversations
#[async_trait]
pub trait ConversationManager: Send + Sync {
    /// Create a new conversation
    async fn create_conversation(&mut self, title: String, workspace_id: Option<Uuid>) -> Result<Uuid>;
    
    /// Get a conversation by ID
    async fn get_conversation(&self, id: Uuid) -> Result<Option<Conversation>>;
    
    /// Update an existing conversation
    async fn update_conversation(&mut self, conversation: Conversation) -> Result<()>;
    
    /// Delete a conversation
    async fn delete_conversation(&mut self, id: Uuid) -> Result<()>;
    
    /// List conversations with optional workspace filter
    async fn list_conversations(&self, workspace_id: Option<Uuid>) -> Result<Vec<ConversationSummary>>;
    
    /// Search conversations
    async fn search_conversations(&self, query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>>;
    
    /// Create a new branch in a conversation
    async fn create_branch(&mut self, conversation_id: Uuid, parent_message_id: Option<Uuid>, title: String) -> Result<Uuid>;
    
    /// Merge a branch back into the main conversation
    async fn merge_branch(&mut self, conversation_id: Uuid, branch_id: Uuid) -> Result<()>;
    
    /// Create a checkpoint in a conversation
    async fn create_checkpoint(&mut self, conversation_id: Uuid, message_id: Uuid, title: String) -> Result<Uuid>;
    
    /// Restore a conversation to a checkpoint
    async fn restore_checkpoint(&mut self, conversation_id: Uuid, checkpoint_id: Uuid) -> Result<()>;
    
    /// Get conversation statistics
    async fn get_statistics(&self) -> Result<ConversationStatistics>;
    
    /// Archive old conversations based on criteria
    async fn archive_conversations(&mut self, criteria: ArchiveCriteria) -> Result<usize>;
}

/// Implementation of the conversation manager
pub struct ConversationManagerImpl {
    /// In-memory cache of conversations
    conversations: Arc<RwLock<HashMap<Uuid, Conversation>>>,
    
    /// Persistence layer
    persistence: Box<dyn ConversationPersistence>,
    
    /// Search engine
    search_engine: Box<dyn ConversationSearchEngine>,
    
    /// Whether to auto-save changes
    auto_save: bool,
}

/// Statistics about conversations
#[derive(Debug, Clone)]
pub struct ConversationStatistics {
    pub total_conversations: usize,
    pub active_conversations: usize,
    pub total_messages: usize,
    pub total_branches: usize,
    pub total_checkpoints: usize,
    pub conversations_by_workspace: HashMap<Option<Uuid>, usize>,
    pub average_messages_per_conversation: f64,
}

/// Criteria for archiving conversations
#[derive(Debug, Clone)]
pub struct ArchiveCriteria {
    /// Archive conversations older than this many days
    pub older_than_days: Option<u32>,
    
    /// Archive conversations with fewer than this many messages
    pub fewer_than_messages: Option<usize>,
    
    /// Archive conversations in specific workspaces
    pub workspace_ids: Option<Vec<Uuid>>,
    
    /// Archive conversations with specific statuses
    pub statuses: Option<Vec<ConversationStatus>>,
}

impl ConversationManagerImpl {
    /// Create a new conversation manager
    pub async fn new(
        persistence: Box<dyn ConversationPersistence>,
        search_engine: Box<dyn ConversationSearchEngine>,
    ) -> Result<Self> {
        let mut manager = Self {
            conversations: Arc::new(RwLock::new(HashMap::new())),
            persistence,
            search_engine,
            auto_save: true,
        };
        
        // Load existing conversations from persistence
        manager.load_all_conversations().await?;
        
        Ok(manager)
    }
    
    /// Set auto-save behavior
    pub fn with_auto_save(mut self, auto_save: bool) -> Self {
        self.auto_save = auto_save;
        self
    }
    
    /// Load all conversations from persistence into memory
    async fn load_all_conversations(&mut self) -> Result<()> {
        let conversation_ids = self.persistence.list_conversation_ids().await?;
        let mut conversations = self.conversations.write().await;
        
        for id in conversation_ids {
            if let Some(conversation) = self.persistence.load_conversation(id).await? {
                conversations.insert(id, conversation);
            }
        }
        
        Ok(())
    }
    
    /// Save a conversation to persistence if auto-save is enabled
    async fn maybe_save_conversation(&self, conversation: &Conversation) -> Result<()> {
        if self.auto_save {
            self.persistence.save_conversation(conversation).await?;
        }
        Ok(())
    }
    
    /// Create a context snapshot for checkpoints
    async fn create_context_snapshot(&self, _conversation_id: Uuid, _message_id: Uuid) -> Result<ContextSnapshot> {
        // TODO: Implement actual context capture
        // This would involve:
        // - Capturing current file states
        // - Getting git repository states
        // - Capturing environment variables
        // - Getting current working directory
        // - Capturing agent state
        
        Ok(ContextSnapshot {
            file_states: HashMap::new(),
            repository_states: HashMap::new(),
            environment: std::env::vars().collect(),
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            agent_state: "active".to_string(),
        })
    }
}

#[async_trait]
impl ConversationManager for ConversationManagerImpl {
    async fn create_conversation(&mut self, title: String, workspace_id: Option<Uuid>) -> Result<Uuid> {
        let conversation = Conversation::new(title, workspace_id);
        let id = conversation.id;
        
        // Add to memory cache
        {
            let mut conversations = self.conversations.write().await;
            conversations.insert(id, conversation.clone());
        }
        
        // Save to persistence
        self.maybe_save_conversation(&conversation).await?;
        
        // Index for search
        self.search_engine.index_conversation(&conversation).await?;
        
        Ok(id)
    }
    
    async fn get_conversation(&self, id: Uuid) -> Result<Option<Conversation>> {
        let conversations = self.conversations.read().await;
        Ok(conversations.get(&id).cloned())
    }
    
    async fn update_conversation(&mut self, conversation: Conversation) -> Result<()> {
        let id = conversation.id;
        
        // Update memory cache
        {
            let mut conversations = self.conversations.write().await;
            conversations.insert(id, conversation.clone());
        }
        
        // Save to persistence
        self.maybe_save_conversation(&conversation).await?;
        
        // Update search index
        self.search_engine.index_conversation(&conversation).await?;
        
        Ok(())
    }
    
    async fn delete_conversation(&mut self, id: Uuid) -> Result<()> {
        // Remove from memory cache
        {
            let mut conversations = self.conversations.write().await;
            conversations.remove(&id);
        }
        
        // Remove from persistence
        self.persistence.delete_conversation(id).await?;
        
        // Remove from search index
        self.search_engine.remove_conversation(id).await?;
        
        Ok(())
    }
    
    async fn list_conversations(&self, workspace_id: Option<Uuid>) -> Result<Vec<ConversationSummary>> {
        let conversations = self.conversations.read().await;
        let mut summaries: Vec<ConversationSummary> = conversations
            .values()
            .filter(|conv| {
                if let Some(workspace_id) = workspace_id {
                    conv.workspace_id == Some(workspace_id)
                } else {
                    true
                }
            })
            .map(|conv| conv.to_summary())
            .collect();
        
        // Sort by last activity (most recent first)
        summaries.sort_by(|a, b| b.last_active.cmp(&a.last_active));
        
        Ok(summaries)
    }
    
    async fn search_conversations(&self, query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>> {
        self.search_engine.search(query).await
    }
    
    async fn create_branch(&mut self, conversation_id: Uuid, parent_message_id: Option<Uuid>, title: String) -> Result<Uuid> {
        let mut conversation = self.get_conversation(conversation_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conversation_id))?;
        
        let branch = ConversationBranch::new(title, parent_message_id);
        let branch_id = branch.id;
        
        conversation.branches.push(branch);
        conversation.last_active = chrono::Utc::now();
        
        self.update_conversation(conversation).await?;
        
        Ok(branch_id)
    }
    
    async fn merge_branch(&mut self, conversation_id: Uuid, branch_id: Uuid) -> Result<()> {
        let mut conversation = self.get_conversation(conversation_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conversation_id))?;
        
        // Find the branch to merge
        let branch_index = conversation.branches.iter().position(|b| b.id == branch_id)
            .ok_or_else(|| anyhow::anyhow!("Branch not found: {}", branch_id))?;
        
        let mut branch = conversation.branches.remove(branch_index);
        
        // Clone messages before marking branch as merged
        let branch_messages = branch.messages.clone();
        
        // Mark branch as merged
        branch.merged = true;
        branch.status = BranchStatus::Merged;
        
        // Append branch messages to main conversation
        conversation.messages.extend(branch_messages);
        conversation.last_active = chrono::Utc::now();
        
        // Add the merged branch back to history
        conversation.branches.push(branch);
        
        self.update_conversation(conversation).await?;
        
        Ok(())
    }
    
    async fn create_checkpoint(&mut self, conversation_id: Uuid, message_id: Uuid, title: String) -> Result<Uuid> {
        let mut conversation = self.get_conversation(conversation_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conversation_id))?;
        
        // Verify message exists
        let message_exists = conversation.messages.iter().any(|m| m.id == message_id) ||
            conversation.branches.iter().any(|b| b.messages.iter().any(|m| m.id == message_id));
        
        if !message_exists {
            return Err(anyhow::anyhow!("Message not found: {}", message_id));
        }
        
        let context_snapshot = self.create_context_snapshot(conversation_id, message_id).await?;
        let checkpoint = ConversationCheckpoint::new(
            message_id, 
            title, 
            None,
            Some(context_snapshot), 
            false
        );
        let checkpoint_id = checkpoint.id;
        
        conversation.checkpoints.push(checkpoint);
        conversation.last_active = chrono::Utc::now();
        
        self.update_conversation(conversation).await?;
        
        Ok(checkpoint_id)
    }
    
    async fn restore_checkpoint(&mut self, conversation_id: Uuid, checkpoint_id: Uuid) -> Result<()> {
        let mut conversation = self.get_conversation(conversation_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conversation_id))?;
        
        // Find the checkpoint
        let checkpoint = conversation.checkpoints.iter()
            .find(|c| c.id == checkpoint_id)
            .ok_or_else(|| anyhow::anyhow!("Checkpoint not found: {}", checkpoint_id))?;
        
        let target_message_id = checkpoint.message_id;
        
        // Find the position of the target message
        if let Some(pos) = conversation.messages.iter().position(|m| m.id == target_message_id) {
            // Truncate messages after the checkpoint
            conversation.messages.truncate(pos + 1);
            conversation.last_active = chrono::Utc::now();
            
            self.update_conversation(conversation).await?;
        } else {
            return Err(anyhow::anyhow!("Checkpoint message not found in main conversation"));
        }
        
        Ok(())
    }
    
    async fn get_statistics(&self) -> Result<ConversationStatistics> {
        let conversations = self.conversations.read().await;
        
        let total_conversations = conversations.len();
        let active_conversations = conversations.values()
            .filter(|c| c.status == ConversationStatus::Active)
            .count();
        
        let total_messages: usize = conversations.values()
            .map(|c| c.messages.len())
            .sum();
        
        let total_branches: usize = conversations.values()
            .map(|c| c.branches.len())
            .sum();
        
        let total_checkpoints: usize = conversations.values()
            .map(|c| c.checkpoints.len())
            .sum();
        
        let mut conversations_by_workspace: HashMap<Option<Uuid>, usize> = HashMap::new();
        for conversation in conversations.values() {
            *conversations_by_workspace.entry(conversation.workspace_id).or_insert(0) += 1;
        }
        
        let average_messages_per_conversation = if total_conversations > 0 {
            total_messages as f64 / total_conversations as f64
        } else {
            0.0
        };
        
        Ok(ConversationStatistics {
            total_conversations,
            active_conversations,
            total_messages,
            total_branches,
            total_checkpoints,
            conversations_by_workspace,
            average_messages_per_conversation,
        })
    }
    
    async fn archive_conversations(&mut self, criteria: ArchiveCriteria) -> Result<usize> {
        let conversations = self.conversations.read().await;
        let mut to_archive = Vec::new();
        
        for conversation in conversations.values() {
            let mut should_archive = false;
            
            // Check age criteria
            if let Some(older_than_days) = criteria.older_than_days {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(older_than_days as i64);
                if conversation.last_active < cutoff {
                    should_archive = true;
                }
            }
            
            // Check message count criteria
            if let Some(fewer_than_messages) = criteria.fewer_than_messages {
                if conversation.messages.len() < fewer_than_messages {
                    should_archive = true;
                }
            }
            
            // Check workspace criteria
            if let Some(ref workspace_ids) = criteria.workspace_ids {
                if let Some(workspace_id) = conversation.workspace_id {
                    if workspace_ids.contains(&workspace_id) {
                        should_archive = true;
                    }
                }
            }
            
            // Check status criteria
            if let Some(ref statuses) = criteria.statuses {
                if statuses.contains(&conversation.status) {
                    should_archive = true;
                }
            }
            
            if should_archive {
                to_archive.push(conversation.id);
            }
        }
        
        drop(conversations); // Release read lock
        
        // Archive the selected conversations
        for conversation_id in &to_archive {
            self.persistence.archive_conversation(*conversation_id).await?;
        }
        
        Ok(to_archive.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::persistence::MockConversationPersistence;
    use crate::agent::conversation::search::MockConversationSearchEngine;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_conversation_manager_creation() {
        let mut mock_persistence = MockConversationPersistence::new();
        mock_persistence
            .expect_list_conversation_ids()
            .returning(|| Ok(Vec::new()));
        
        let mock_search = MockConversationSearchEngine::new();
        
        let manager = ConversationManagerImpl::new(
            Box::new(mock_persistence),
            Box::new(mock_search),
        ).await;
        
        assert!(manager.is_ok());
    }
    
    #[tokio::test]
    async fn test_create_conversation() {
        let mut mock_persistence = MockConversationPersistence::new();
        mock_persistence
            .expect_list_conversation_ids()
            .returning(|| Ok(Vec::new()));
        mock_persistence
            .expect_save_conversation()
            .returning(|_| Ok(()));
        
        let mut mock_search = MockConversationSearchEngine::new();
        mock_search
            .expect_index_conversation()
            .returning(|_| Ok(()));
        
        let mut manager = ConversationManagerImpl::new(
            Box::new(mock_persistence),
            Box::new(mock_search),
        ).await.unwrap();
        
        let conversation_id = manager.create_conversation("Test Conversation".to_string(), None).await.unwrap();
        
        let conversation = manager.get_conversation(conversation_id).await.unwrap();
        assert!(conversation.is_some());
        assert_eq!(conversation.unwrap().title, "Test Conversation");
    }
    
    #[tokio::test]
    async fn test_update_conversation() {
        let mut mock_persistence = MockConversationPersistence::new();
        mock_persistence
            .expect_list_conversation_ids()
            .returning(|| Ok(Vec::new()));
        mock_persistence
            .expect_save_conversation()
            .returning(|_| Ok(()));
        
        let mut mock_search = MockConversationSearchEngine::new();
        mock_search
            .expect_index_conversation()
            .returning(|_| Ok(()));
        
        let mut manager = ConversationManagerImpl::new(
            Box::new(mock_persistence),
            Box::new(mock_search),
        ).await.unwrap();
        
        let mut conversation = Conversation::new("Original Title".to_string(), None);
        conversation.title = "Updated Title".to_string();
        
        manager.update_conversation(conversation.clone()).await.unwrap();
        
        let retrieved = manager.get_conversation(conversation.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Updated Title");
    }
    
    #[tokio::test]
    async fn test_create_branch() {
        let mut mock_persistence = MockConversationPersistence::new();
        mock_persistence
            .expect_list_conversation_ids()
            .returning(|| Ok(Vec::new()));
        mock_persistence
            .expect_save_conversation()
            .returning(|_| Ok(()));
        
        let mut mock_search = MockConversationSearchEngine::new();
        mock_search
            .expect_index_conversation()
            .returning(|_| Ok(()));
        
        let mut manager = ConversationManagerImpl::new(
            Box::new(mock_persistence),
            Box::new(mock_search),
        ).await.unwrap();
        
        let conversation_id = manager.create_conversation("Test".to_string(), None).await.unwrap();
        let branch_id = manager.create_branch(conversation_id, None, "Test Branch".to_string()).await.unwrap();
        
        let conversation = manager.get_conversation(conversation_id).await.unwrap().unwrap();
        assert_eq!(conversation.branches.len(), 1);
        assert_eq!(conversation.branches[0].id, branch_id);
        assert_eq!(conversation.branches[0].title, "Test Branch");
    }
    
    #[tokio::test]
    async fn test_create_checkpoint() {
        let mut mock_persistence = MockConversationPersistence::new();
        mock_persistence
            .expect_list_conversation_ids()
            .returning(|| Ok(Vec::new()));
        mock_persistence
            .expect_save_conversation()
            .returning(|_| Ok(()));
        
        let mut mock_search = MockConversationSearchEngine::new();
        mock_search
            .expect_index_conversation()
            .returning(|_| Ok(()));
        
        let mut manager = ConversationManagerImpl::new(
            Box::new(mock_persistence),
            Box::new(mock_search),
        ).await.unwrap();
        
        let conversation_id = manager.create_conversation("Test".to_string(), None).await.unwrap();
        
        // Add a message first
        let mut conversation = manager.get_conversation(conversation_id).await.unwrap().unwrap();
        let message = crate::agent::message::types::AgentMessage::user("Test message");
        let message_id = message.id;
        conversation.add_message(message);
        manager.update_conversation(conversation).await.unwrap();
        
        // Create checkpoint
        let checkpoint_id = manager.create_checkpoint(conversation_id, message_id, "Test Checkpoint".to_string()).await.unwrap();
        
        let conversation = manager.get_conversation(conversation_id).await.unwrap().unwrap();
        assert_eq!(conversation.checkpoints.len(), 1);
        assert_eq!(conversation.checkpoints[0].id, checkpoint_id);
        assert_eq!(conversation.checkpoints[0].title, "Test Checkpoint");
    }
    
    #[tokio::test]
    async fn test_list_conversations() {
        let mut mock_persistence = MockConversationPersistence::new();
        mock_persistence
            .expect_list_conversation_ids()
            .returning(|| Ok(Vec::new()));
        mock_persistence
            .expect_save_conversation()
            .returning(|_| Ok(()));
        
        let mut mock_search = MockConversationSearchEngine::new();
        mock_search
            .expect_index_conversation()
            .returning(|_| Ok(()));
        
        let mut manager = ConversationManagerImpl::new(
            Box::new(mock_persistence),
            Box::new(mock_search),
        ).await.unwrap();
        
        let workspace_id = Uuid::new_v4();
        
        // Create conversations
        manager.create_conversation("Conv 1".to_string(), Some(workspace_id)).await.unwrap();
        manager.create_conversation("Conv 2".to_string(), None).await.unwrap();
        manager.create_conversation("Conv 3".to_string(), Some(workspace_id)).await.unwrap();
        
        // List all conversations
        let all_conversations = manager.list_conversations(None).await.unwrap();
        assert_eq!(all_conversations.len(), 3);
        
        // List conversations for specific workspace
        let workspace_conversations = manager.list_conversations(Some(workspace_id)).await.unwrap();
        assert_eq!(workspace_conversations.len(), 2);
    }
    
    #[tokio::test]
    async fn test_get_statistics() {
        let mut mock_persistence = MockConversationPersistence::new();
        mock_persistence
            .expect_list_conversation_ids()
            .returning(|| Ok(Vec::new()));
        mock_persistence
            .expect_save_conversation()
            .returning(|_| Ok(()));
        
        let mut mock_search = MockConversationSearchEngine::new();
        mock_search
            .expect_index_conversation()
            .returning(|_| Ok(()));
        
        let mut manager = ConversationManagerImpl::new(
            Box::new(mock_persistence),
            Box::new(mock_search),
        ).await.unwrap();
        
        // Create some test data
        let conv_id = manager.create_conversation("Test".to_string(), None).await.unwrap();
        manager.create_branch(conv_id, None, "Branch".to_string()).await.unwrap();
        
        let stats = manager.get_statistics().await.unwrap();
        
        assert_eq!(stats.total_conversations, 1);
        assert_eq!(stats.active_conversations, 1);
        assert_eq!(stats.total_branches, 1);
    }
} 