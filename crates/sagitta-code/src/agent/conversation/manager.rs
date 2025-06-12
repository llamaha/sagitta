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
use super::tagging::{TaggingPipeline, TaggingPipelineConfig, TaggingResult, TagMetadata, TagSuggestion};
use crate::agent::state::types::ConversationStatus;
use crate::agent::conversation::types::BranchStatus;
use crate::agent::events::AgentEvent;
use crate::config::types::ConversationConfig;
use crate::llm::title::{TitleGenerator, TitleGeneratorConfig};

/// Trait for managing conversations
#[async_trait]
pub trait ConversationManager: Send + Sync {
    /// Create a new conversation
    async fn create_conversation(&self, title: String, workspace_id: Option<Uuid>) -> Result<Uuid>;
    
    /// Get a conversation by ID
    async fn get_conversation(&self, id: Uuid) -> Result<Option<Conversation>>;
    
    /// Update an existing conversation
    async fn update_conversation(&self, conversation: Conversation) -> Result<()>;
    
    /// Delete a conversation
    async fn delete_conversation(&self, id: Uuid) -> Result<()>;
    
    /// List conversations with optional workspace filter
    async fn list_conversations(&self, workspace_id: Option<Uuid>) -> Result<Vec<ConversationSummary>>;
    
    /// Search conversations
    async fn search_conversations(&self, query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>>;
    
    /// Create a new branch in a conversation
    async fn create_branch(&self, conversation_id: Uuid, parent_message_id: Option<Uuid>, title: String) -> Result<Uuid>;
    
    /// Merge a branch back into the main conversation
    async fn merge_branch(&self, conversation_id: Uuid, branch_id: Uuid) -> Result<()>;
    
    /// Create a checkpoint in a conversation
    async fn create_checkpoint(&self, conversation_id: Uuid, message_id: Uuid, title: String) -> Result<Uuid>;
    
    /// Restore a conversation to a checkpoint
    async fn restore_checkpoint(&self, conversation_id: Uuid, checkpoint_id: Uuid) -> Result<()>;
    
    /// Get conversation statistics
    async fn get_statistics(&self) -> Result<ConversationStatistics>;
    
    /// Archive old conversations based on criteria
    async fn archive_conversations(&self, criteria: ArchiveCriteria) -> Result<usize>;
    
    /// Get tag suggestions for a conversation
    async fn get_tag_suggestions(&self, conversation_id: Uuid) -> Result<Vec<TagSuggestion>>;
    
    /// Get tag metadata for a conversation
    async fn get_tag_metadata(&self, conversation_id: Uuid) -> Result<Vec<TagMetadata>>;
    
    /// Manually trigger tagging for a conversation
    async fn retag_conversation(&self, conversation_id: Uuid) -> Result<TaggingResult>;
}

/// Implementation of the conversation manager
pub struct ConversationManagerImpl {
    /// In-memory cache of conversations
    conversations: Arc<RwLock<HashMap<Uuid, Conversation>>>,
    
    /// Persistence layer
    persistence: Box<dyn ConversationPersistence>,
    
    /// Search engine
    search_engine: Box<dyn ConversationSearchEngine>,
    
    /// Tagging pipeline
    tagging_pipeline: Option<Arc<TaggingPipeline>>,
    
    /// Title generator
    title_generator: Option<Arc<TitleGenerator>>,
    
    /// Whether to auto-save changes
    auto_save: bool,
    
    /// Track conversations currently being processed for title generation to prevent recursion
    title_generation_in_progress: Arc<RwLock<std::collections::HashSet<Uuid>>>,
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
            tagging_pipeline: None,
            title_generator: None,
            auto_save: true,
            title_generation_in_progress: Arc::new(RwLock::new(std::collections::HashSet::new())),
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
    
    /// Set the tagging pipeline
    pub fn with_tagging_pipeline(mut self, pipeline: Arc<TaggingPipeline>) -> Self {
        self.tagging_pipeline = Some(pipeline);
        self
    }
    
    /// Get the tagging pipeline if available
    pub fn tagging_pipeline(&self) -> Option<Arc<TaggingPipeline>> {
        self.tagging_pipeline.clone()
    }
    
    /// Set the title generator
    pub fn with_title_generator(mut self, generator: Arc<TitleGenerator>) -> Self {
        self.title_generator = Some(generator);
        self
    }
    
    /// Get the title generator if available
    pub fn title_generator(&self) -> Option<Arc<TitleGenerator>> {
        self.title_generator.clone()
    }
    
    /// Load all conversations from persistence into memory (Phase 2: Lazy loading optimization)
    /// This now loads only conversation summaries from the index for fast startup
    pub async fn load_all_conversations(&mut self) -> Result<()> {
        // Phase 2 optimization: Instead of loading full conversations, only load IDs
        // and rely on lazy loading in get_conversation()
        let conversation_ids = self.persistence.list_conversation_ids().await?;
        
        // For lazy loading, we don't pre-load conversations into memory
        // We just ensure the conversations cache exists but keep it empty
        let conversations = self.conversations.read().await;
        let current_count = conversations.len();
        drop(conversations);
        
        log::info!("Phase 2 lazy loading: Discovered {} conversations (loaded {} summaries only, full conversations loaded on-demand)", 
                   conversation_ids.len(), current_count);
        
        Ok(())
    }
    
    /// Save a conversation to persistence if auto-save is enabled
    async fn maybe_save_conversation(&self, conversation: &Conversation) -> Result<()> {
        if self.auto_save {
            self.persistence.save_conversation(conversation).await?;
        }
        Ok(())
    }
    
    /// Run tagging pipeline on a conversation if enabled
    async fn maybe_run_tagging(&self, conversation_id: Uuid) -> Result<()> {
        if let Some(pipeline) = self.tagging_pipeline.clone() {
            tokio::spawn(async move {
                if let Err(e) = pipeline.process_conversation(conversation_id).await {
                    log::warn!("Failed to tag conversation {}: {}", conversation_id, e);
                }
            });
        }
        Ok(())
    }
    
    /// Run title generation on a conversation if enabled and needed
    async fn maybe_run_title_generation(&self, conversation_id: Uuid) -> Result<()> {
        if let Some(title_generator) = self.title_generator.clone() {
            // Check if title generation is already in progress for this conversation
            {
                let in_progress = self.title_generation_in_progress.read().await;
                if in_progress.contains(&conversation_id) {
                    return Ok(()); // Skip to avoid recursion
                }
            }
            
            // Mark as in progress
            {
                let mut in_progress = self.title_generation_in_progress.write().await;
                in_progress.insert(conversation_id);
            }
            
            // Get the conversation
            let conversation = {
                let conversations_guard = self.conversations.read().await;
                conversations_guard.get(&conversation_id).cloned()
            };
            
            if let Some(conversation) = conversation {
                // Only generate title if:
                // 1. Current title is empty or generic
                // 2. OR title looks like a user question/input rather than a proper title
                // 3. Conversation has enough messages
                let title_looks_like_user_input = conversation.title.ends_with("?") || 
                                                 conversation.title.to_lowercase().starts_with("how") ||
                                                 conversation.title.to_lowercase().starts_with("what") ||
                                                 conversation.title.to_lowercase().starts_with("why") ||
                                                 conversation.title.to_lowercase().starts_with("when") ||
                                                 conversation.title.to_lowercase().starts_with("where") ||
                                                 conversation.title.to_lowercase().starts_with("can") ||
                                                 conversation.title.to_lowercase().starts_with("should") ||
                                                 conversation.title.to_lowercase().starts_with("i need") ||
                                                 conversation.title.to_lowercase().starts_with("help");
                
                let should_generate = (conversation.title.is_empty() || 
                                     conversation.title.starts_with("Conversation") ||
                                     conversation.title == "New Conversation" ||
                                     title_looks_like_user_input) &&
                                    conversation.messages.len() >= 2;
                
                if should_generate {
                    log::info!("Triggering title generation for conversation {} with current title: '{}'", conversation_id, conversation.title);
                    match title_generator.generate_title(&conversation.messages).await {
                        Ok(new_title) => {
                            log::info!("Generated title '{}' for conversation {}", new_title, conversation_id);
                            
                            // Update the conversation with the new title
                            let mut updated_conversation = conversation.clone();
                            updated_conversation.title = new_title;
                            
                            // Update in memory cache
                            {
                                let mut conversations_guard = self.conversations.write().await;
                                conversations_guard.insert(conversation_id, updated_conversation.clone());
                            }
                            
                            // Save to persistence if auto-save is enabled (without triggering title generation again)
                            if self.auto_save {
                                if let Err(e) = self.persistence.save_conversation(&updated_conversation).await {
                                    log::error!("Failed to save conversation after title generation: {}", e);
                                }
                                
                                // Update search index
                                if let Err(e) = self.search_engine.index_conversation(&updated_conversation).await {
                                    log::error!("Failed to update search index after title generation: {}", e);
                                }
                            }
                            
                            log::debug!("Title generation completed for conversation {}", conversation_id);
                        },
                        Err(e) => {
                            log::warn!("Failed to generate title for conversation {}: {}", conversation_id, e);
                        }
                    }
                } else {
                    log::debug!("Skipping title generation for conversation {} - title: '{}', messages: {}, should_generate: {}", 
                              conversation_id, conversation.title, conversation.messages.len(), should_generate);
                }
            }
            
            // Mark as completed
            {
                let mut in_progress = self.title_generation_in_progress.write().await;
                in_progress.remove(&conversation_id);
            }
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
    async fn create_conversation(&self, title: String, workspace_id: Option<Uuid>) -> Result<Uuid> {
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
        
        // Run tagging pipeline (async, don't block on it)
        self.maybe_run_tagging(id).await?;
        
        // Run title generation (async, don't block on it)
        self.maybe_run_title_generation(id).await?;
        
        Ok(id)
    }
    
    async fn get_conversation(&self, id: Uuid) -> Result<Option<Conversation>> {
        // Phase 2 lazy loading: Check cache first, then load from persistence if needed
        {
            let conversations = self.conversations.read().await;
            if let Some(conversation) = conversations.get(&id) {
                log::debug!("Conversation {} found in cache", id);
                return Ok(Some(conversation.clone()));
            }
        }
        
        // Not in cache, try to load from persistence
        log::debug!("Conversation {} not in cache, loading from persistence", id);
        match self.persistence.load_conversation(id).await? {
            Some(conversation) => {
                // Cache the loaded conversation for future access
                {
                    let mut conversations = self.conversations.write().await;
                    conversations.insert(id, conversation.clone());
                }
                log::debug!("Conversation {} loaded from persistence and cached", id);
                Ok(Some(conversation))
            }
            None => {
                log::debug!("Conversation {} not found in persistence", id);
                Ok(None)
            }
        }
    }
    
    async fn update_conversation(&self, conversation: Conversation) -> Result<()> {
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
        
        // Run tagging pipeline (async, don't block on it)
        self.maybe_run_tagging(id).await?;
        
        // Run title generation (async, don't block on it)
        self.maybe_run_title_generation(id).await?;
        
        Ok(())
    }
    
    async fn delete_conversation(&self, id: Uuid) -> Result<()> {
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
        // Phase 2 lazy loading: Use the index-based summaries instead of loading full conversations
        self.persistence.list_conversation_summaries(workspace_id).await
    }
    
    async fn search_conversations(&self, query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>> {
        self.search_engine.search(query).await
    }
    
    async fn create_branch(&self, conversation_id: Uuid, parent_message_id: Option<Uuid>, title: String) -> Result<Uuid> {
        let conversation = self.get_conversation(conversation_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conversation_id))?;
        
        let branch = ConversationBranch::new(title, parent_message_id);
        let branch_id = branch.id;
        
        let mut conversation = conversation.clone();
        conversation.branches.push(branch);
        conversation.last_active = chrono::Utc::now();
        
        self.update_conversation(conversation).await?;
        
        Ok(branch_id)
    }
    
    async fn merge_branch(&self, conversation_id: Uuid, branch_id: Uuid) -> Result<()> {
        let conversation = self.get_conversation(conversation_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conversation_id))?;
        
        // Find the branch to merge
        let branch_index = conversation.branches.iter().position(|b| b.id == branch_id)
            .ok_or_else(|| anyhow::anyhow!("Branch not found: {}", branch_id))?;
        
        let mut branch = conversation.branches[branch_index].clone();
        let branch_messages = branch.messages.clone();
        
        // Mark branch as merged
        branch.merged = true;
        branch.status = BranchStatus::Merged;
        
        // Append branch messages to main conversation
        let mut conversation = conversation.clone();
        conversation.messages.extend(branch_messages);
        conversation.last_active = chrono::Utc::now();
        
        // Update the branch in the conversation
        conversation.branches[branch_index] = branch;
        
        self.update_conversation(conversation).await?;
        
        Ok(())
    }
    
    async fn create_checkpoint(&self, conversation_id: Uuid, message_id: Uuid, title: String) -> Result<Uuid> {
        let conversation = self.get_conversation(conversation_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conversation_id))?;
        
        // Verify the message exists in the conversation
        if !conversation.messages.iter().any(|m| m.id == message_id) {
            return Err(anyhow::anyhow!("Message not found in conversation: {}", message_id));
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
        
        let mut conversation = conversation.clone();
        conversation.checkpoints.push(checkpoint);
        conversation.last_active = chrono::Utc::now();
        
        self.update_conversation(conversation).await?;
        
        Ok(checkpoint_id)
    }
    
    async fn restore_checkpoint(&self, conversation_id: Uuid, checkpoint_id: Uuid) -> Result<()> {
        let conversation = self.get_conversation(conversation_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conversation_id))?;
        
        // Find the checkpoint
        let checkpoint = conversation.checkpoints.iter()
            .find(|c| c.id == checkpoint_id)
            .ok_or_else(|| anyhow::anyhow!("Checkpoint not found: {}", checkpoint_id))?;
        
        let target_message_id = checkpoint.message_id;
        
        // Find the message position and truncate
        if let Some(pos) = conversation.messages.iter().position(|m| m.id == target_message_id) {
            // Truncate messages after the checkpoint
            let mut conversation = conversation.clone();
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
    
    async fn archive_conversations(&self, criteria: ArchiveCriteria) -> Result<usize> {
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
    
    async fn get_tag_suggestions(&self, conversation_id: Uuid) -> Result<Vec<TagSuggestion>> {
        if let Some(ref pipeline) = self.tagging_pipeline {
            // Get the UI state which contains suggestions
            let ui_state = pipeline.get_ui_state(conversation_id).await;
            Ok(ui_state.get_state().suggestions.iter()
                .filter(|s| matches!(s.action, super::tagging::TagAction::Pending))
                .map(|s| s.suggestion.clone())
                .collect())
        } else {
            Ok(Vec::new())
        }
    }
    
    async fn get_tag_metadata(&self, conversation_id: Uuid) -> Result<Vec<TagMetadata>> {
        if let Some(ref pipeline) = self.tagging_pipeline {
            Ok(pipeline.get_tag_metadata(conversation_id).await)
        } else {
            Ok(Vec::new())
        }
    }
    
    async fn retag_conversation(&self, conversation_id: Uuid) -> Result<TaggingResult> {
        if let Some(ref pipeline) = self.tagging_pipeline {
            pipeline.process_conversation(conversation_id).await
        } else {
            // Return empty result if no pipeline
            Ok(TaggingResult {
                conversation_id,
                suggestions_generated: Vec::new(),
                tags_applied: Vec::new(),
                tags_rejected: Vec::new(),
                metadata: Vec::new(),
            })
        }
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
        
        let workspace_id = Uuid::new_v4();
        let conv1_id = Uuid::new_v4();
        let conv2_id = Uuid::new_v4();
        let conv3_id = Uuid::new_v4();
        
        // Mock list_conversation_summaries to return created conversations
        mock_persistence
            .expect_list_conversation_summaries()
            .returning(move |workspace_filter| {
                use crate::agent::conversation::types::ConversationSummary;
                use crate::agent::state::types::ConversationStatus;
                use chrono::Utc;
                
                let all_summaries = vec![
                    ConversationSummary {
                        id: conv1_id,
                        title: "Conv 1".to_string(),
                        workspace_id: Some(workspace_id),
                        created_at: Utc::now(),
                        last_active: Utc::now(),
                        status: ConversationStatus::Active,
                        message_count: 0,
                        tags: vec![],
                        project_name: None,
                        has_branches: false,
                        has_checkpoints: false,
                    },
                    ConversationSummary {
                        id: conv2_id,
                        title: "Conv 2".to_string(),
                        workspace_id: None,
                        created_at: Utc::now(),
                        last_active: Utc::now(),
                        status: ConversationStatus::Active,
                        message_count: 0,
                        tags: vec![],
                        project_name: None,
                        has_branches: false,
                        has_checkpoints: false,
                    },
                    ConversationSummary {
                        id: conv3_id,
                        title: "Conv 3".to_string(),
                        workspace_id: Some(workspace_id),
                        created_at: Utc::now(),
                        last_active: Utc::now(),
                        status: ConversationStatus::Active,
                        message_count: 0,
                        tags: vec![],
                        project_name: None,
                        has_branches: false,
                        has_checkpoints: false,
                    },
                ];
                
                if let Some(ws_id) = workspace_filter {
                    Ok(all_summaries.into_iter().filter(|s| s.workspace_id == Some(ws_id)).collect())
                } else {
                    Ok(all_summaries)
                }
            });
        
        let mut mock_search = MockConversationSearchEngine::new();
        mock_search
            .expect_index_conversation()
            .returning(|_| Ok(()));
        
        let mut manager = ConversationManagerImpl::new(
            Box::new(mock_persistence),
            Box::new(mock_search),
        ).await.unwrap();
        
        // Create conversations (these will be stored in memory cache but listing comes from persistence)
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