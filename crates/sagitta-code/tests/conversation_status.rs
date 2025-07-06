use anyhow::Result;
use chrono::{Duration, Utc};
use sagitta_code::agent::conversation::manager::{ConversationManager, ConversationManagerImpl};
use sagitta_code::agent::conversation::persistence::ConversationPersistence;
use sagitta_code::agent::conversation::search::ConversationSearchEngine;
use sagitta_code::agent::conversation::types::{Conversation, ConversationQuery, ConversationSearchResult, ConversationSummary};
use sagitta_code::agent::state::types::ConversationStatus;
use sagitta_code::agent::state::status_engine::{ConversationStatusEngine, StatusEngineConfig};
use sagitta_code::agent::events::AgentEvent;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;
use std::collections::HashMap;

// Mock implementations for testing
struct MockPersistence {
    conversations: Arc<RwLock<HashMap<Uuid, Conversation>>>,
}

impl Default for MockPersistence {
    fn default() -> Self {
        Self::new()
    }
}

impl MockPersistence {
    fn new() -> Self {
        Self {
            conversations: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl ConversationPersistence for MockPersistence {
    async fn save_conversation(&self, conversation: &Conversation) -> Result<()> {
        let mut conversations = self.conversations.write().await;
        conversations.insert(conversation.id, conversation.clone());
        Ok(())
    }
    
    async fn load_conversation(&self, id: Uuid) -> Result<Option<Conversation>> {
        let conversations = self.conversations.read().await;
        Ok(conversations.get(&id).cloned())
    }
    
    async fn delete_conversation(&self, id: Uuid) -> Result<()> {
        let mut conversations = self.conversations.write().await;
        conversations.remove(&id);
        Ok(())
    }
    
    async fn list_conversation_ids(&self) -> Result<Vec<Uuid>> {
        let conversations = self.conversations.read().await;
        Ok(conversations.keys().cloned().collect())
    }
    
    async fn list_conversation_summaries(&self, workspace_id: Option<Uuid>) -> Result<Vec<ConversationSummary>> {
        let conversations = self.conversations.read().await;
        let mut summaries = Vec::new();
        
        for conversation in conversations.values() {
            // Filter by workspace if specified
            if let Some(ws_id) = workspace_id {
                if conversation.workspace_id != Some(ws_id) {
                    continue;
                }
            }
            
            summaries.push(ConversationSummary {
                id: conversation.id,
                title: conversation.title.clone(),
                workspace_id: conversation.workspace_id,
                created_at: conversation.created_at,
                last_active: conversation.last_active,
                status: conversation.status.clone(),
                message_count: conversation.messages.len(),
                tags: conversation.tags.clone(),
                project_name: None,
                has_branches: !conversation.branches.is_empty(),
                has_checkpoints: !conversation.checkpoints.is_empty(),
            });
        }
        
        Ok(summaries)
    }
    
    async fn archive_conversation(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
    
    async fn list_archived_conversation_ids(&self) -> Result<Vec<Uuid>> {
        Ok(Vec::new())
    }
    
    async fn restore_conversation(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct MockSearchEngine;

#[async_trait]
impl ConversationSearchEngine for MockSearchEngine {
    async fn index_conversation(&self, _conversation: &Conversation) -> Result<()> {
        Ok(())
    }
    
    async fn remove_conversation(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
    
    async fn search(&self, _query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>> {
        Ok(Vec::new())
    }
    
    async fn clear_index(&self) -> Result<()> {
        Ok(())
    }
    
    async fn rebuild_index(&self, _conversations: &[Conversation]) -> Result<()> {
        Ok(())
    }
}

/// Test that new conversations default to Active status
#[tokio::test]
async fn test_new_conversation_defaults_to_active() -> Result<()> {
    let manager = create_test_manager().await?;
    
    let conversation_id = manager.create_conversation("Test Conversation".to_string(), None).await?;
    let conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    
    assert_eq!(conversation.status, ConversationStatus::Active);
    Ok(())
}

/// Test that conversations become Paused after 30 minutes of inactivity
#[tokio::test]
async fn test_conversation_pauses_after_inactivity() -> Result<()> {
    let manager = create_test_manager().await?;
    let status_engine = create_test_status_engine(Arc::clone(&manager)).await?;
    
    // Create a conversation
    let conversation_id = manager.create_conversation("Test Conversation".to_string(), None).await?;
    
    // Simulate 30 minutes of inactivity by manually setting last_active
    // Don't use the status engine to update, as that would mark it as a manual override
    let mut conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    conversation.last_active = Utc::now() - Duration::minutes(31);
    manager.update_conversation(conversation).await?;
    
    // Manually trigger status check (in real app this would be automatic)
    status_engine.trigger_status_check().await?;
    
    let updated_conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    
    assert_eq!(updated_conversation.status, ConversationStatus::Paused);
    Ok(())
}

/// Test that conversations become Completed when agent signals completion
#[tokio::test]
async fn test_conversation_completes_on_agent_signal() -> Result<()> {
    let manager = create_test_manager().await?;
    let status_engine = create_test_status_engine(Arc::clone(&manager)).await?;
    
    let conversation_id = manager.create_conversation("Test Conversation".to_string(), None).await?;
    
    // Simulate agent event for completion
    let event = AgentEvent::ConversationCompleted { conversation_id };
    status_engine.handle_agent_event(&event).await?;
    
    let conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    
    assert_eq!(conversation.status, ConversationStatus::Completed);
    Ok(())
}

/// Test that old completed conversations get archived
#[tokio::test]
async fn test_old_completed_conversations_archived() -> Result<()> {
    let manager = create_test_manager().await?;
    let status_engine = create_test_status_engine(Arc::clone(&manager)).await?;
    
    // Create a conversation and mark it as completed
    let conversation_id = manager.create_conversation("Old Conversation".to_string(), None).await?;
    let mut conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    
    // Manually set status to Completed and make it old
    // Don't use the status engine to update, as that would mark it as a manual override
    conversation.status = ConversationStatus::Completed;
    conversation.last_active = Utc::now() - Duration::days(91);
    manager.update_conversation(conversation).await?;
    
    // Manually trigger status check (in real app this would be automatic)
    status_engine.trigger_status_check().await?;
    
    let updated_conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    
    assert_eq!(updated_conversation.status, ConversationStatus::Archived);
    Ok(())
}

/// Test status transitions are properly logged and events are emitted
#[tokio::test]
async fn test_status_transitions_emit_events() -> Result<()> {
    let manager = create_test_manager().await?;
    let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
    let status_engine = ConversationStatusEngine::new(
        StatusEngineConfig::default(),
        Arc::clone(&manager),
    ).with_event_sender(event_sender);
    
    let conversation_id = manager.create_conversation("Test Conversation".to_string(), None).await?;
    
    // Simulate status change
    status_engine.set_conversation_status(conversation_id, ConversationStatus::Paused).await?;
    
    // Check that event was emitted
    let event = event_receiver.try_recv();
    assert!(event.is_ok(), "Event should have been emitted");
    
    if let Ok(AgentEvent::ConversationUpdated { conversation_id: id, old_status, new_status }) = event {
        assert_eq!(id, conversation_id);
        assert_eq!(old_status, ConversationStatus::Active);
        assert_eq!(new_status, ConversationStatus::Paused);
    } else {
        panic!("Wrong event type emitted");
    }
    
    Ok(())
}

/// Test that status engine respects manual status overrides
#[tokio::test]
async fn test_manual_status_override_respected() -> Result<()> {
    let manager = create_test_manager().await?;
    let status_engine = create_test_status_engine(Arc::clone(&manager)).await?;
    
    let conversation_id = manager.create_conversation("Test Conversation".to_string(), None).await?;
    
    // Manually set status to Archived
    status_engine.set_conversation_status(conversation_id, ConversationStatus::Archived).await?;
    
    // Make conversation appear inactive (should normally trigger Paused status)
    let mut conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    conversation.last_active = Utc::now() - Duration::minutes(31);
    manager.update_conversation(conversation).await?;
    
    // Trigger status check - should not change because of manual override
    status_engine.trigger_status_check().await?;
    
    let updated_conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    
    // Should still be Archived, not Paused
    assert_eq!(updated_conversation.status, ConversationStatus::Archived);
    Ok(())
}

/// Helper function to create a test conversation manager
async fn create_test_manager() -> Result<Arc<dyn ConversationManager>> {
    let manager = ConversationManagerImpl::new(
        Box::new(MockPersistence::new()),
        Box::new(MockSearchEngine),
    ).await?;
    Ok(Arc::new(manager) as Arc<dyn ConversationManager>)
}

/// Helper function to create a test status engine
async fn create_test_status_engine(manager: Arc<dyn ConversationManager>) -> Result<ConversationStatusEngine> {
    let config = StatusEngineConfig {
        inactivity_threshold_minutes: 30,
        archive_threshold_days: 90,
        check_interval_seconds: 60,
        respect_manual_overrides: true,
    };
    
    Ok(ConversationStatusEngine::new(config, manager))
} 