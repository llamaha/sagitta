use anyhow::Result;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{interval, Duration as TokioDuration};
use uuid::Uuid;

use crate::agent::conversation::manager::ConversationManager;
use crate::agent::conversation::types::Conversation;
use crate::agent::state::types::ConversationStatus;
use crate::agent::events::AgentEvent;
use crate::llm::fast_model::{FastModelProvider, FastModelOperations};

/// Configuration for the status engine
#[derive(Debug, Clone)]
pub struct StatusEngineConfig {
    /// How long before a conversation is considered inactive (minutes)
    pub inactivity_threshold_minutes: i64,
    
    /// How old completed conversations must be before archiving (days)
    pub archive_threshold_days: i64,
    
    /// How often to check for status updates (seconds)
    pub check_interval_seconds: u64,
    
    /// Whether to respect manual status overrides
    pub respect_manual_overrides: bool,
}

impl Default for StatusEngineConfig {
    fn default() -> Self {
        Self {
            inactivity_threshold_minutes: 30,
            archive_threshold_days: 90,
            check_interval_seconds: 1800, // Check every 30 minutes
            respect_manual_overrides: true,
        }
    }
}

/// Engine that manages automatic conversation status transitions
pub struct ConversationStatusEngine {
    config: StatusEngineConfig,
    conversation_manager: Arc<dyn ConversationManager>,
    event_sender: Option<mpsc::UnboundedSender<AgentEvent>>,
    manual_overrides: Arc<RwLock<std::collections::HashSet<Uuid>>>,
    fast_model_provider: Option<Arc<dyn FastModelOperations>>,
}

impl ConversationStatusEngine {
    /// Create a new status engine
    pub fn new(
        config: StatusEngineConfig,
        conversation_manager: Arc<dyn ConversationManager>,
    ) -> Self {
        Self {
            config,
            conversation_manager,
            event_sender: None,
            manual_overrides: Arc::new(RwLock::new(std::collections::HashSet::new())),
            fast_model_provider: None,
        }
    }
    
    /// Set the event sender for publishing status change events
    pub fn with_event_sender(mut self, sender: mpsc::UnboundedSender<AgentEvent>) -> Self {
        self.event_sender = Some(sender);
        self
    }
    
    /// Set the fast model provider for status evaluation
    pub fn with_fast_model_provider(mut self, provider: Arc<dyn FastModelOperations>) -> Self {
        self.fast_model_provider = Some(provider);
        self
    }
    
    /// Start the status engine background task
    pub async fn start(&self) -> Result<()> {
        let config = self.config.clone();
        let manager = Arc::clone(&self.conversation_manager);
        let event_sender = self.event_sender.clone();
        let manual_overrides = Arc::clone(&self.manual_overrides);
        
        tokio::spawn(async move {
            let mut interval = interval(TokioDuration::from_secs(config.check_interval_seconds));
            
            loop {
                interval.tick().await;
                
                if let Err(e) = Self::check_and_update_statuses(
                    &config,
                    &manager,
                    &event_sender,
                    &manual_overrides,
                ).await {
                    eprintln!("Error updating conversation statuses: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    /// Mark a conversation as manually overridden (won't be auto-updated)
    pub async fn mark_manual_override(&self, conversation_id: Uuid) {
        let mut overrides = self.manual_overrides.write().await;
        overrides.insert(conversation_id);
    }
    
    /// Remove manual override for a conversation
    pub async fn remove_manual_override(&self, conversation_id: Uuid) {
        let mut overrides = self.manual_overrides.write().await;
        overrides.remove(&conversation_id);
    }
    
    /// Handle agent events that might trigger status changes
    pub async fn handle_agent_event(&self, event: &AgentEvent) -> Result<()> {
        match event {
            AgentEvent::ConversationCompleted { conversation_id } => {
                self.set_conversation_status(*conversation_id, ConversationStatus::Completed).await?;
            }
            AgentEvent::ConversationSummarizing { conversation_id } => {
                self.set_conversation_status(*conversation_id, ConversationStatus::Summarizing).await?;
            }
            _ => {
                // Other events don't directly affect conversation status
            }
        }
        Ok(())
    }
    
    /// Manually set a conversation status and mark as override
    pub async fn set_conversation_status(&self, conversation_id: Uuid, status: ConversationStatus) -> Result<()> {
        if let Some(mut conversation) = self.conversation_manager.get_conversation(conversation_id).await? {
            let old_status = conversation.status.clone();
            conversation.status = status.clone();
            
            self.conversation_manager.update_conversation(conversation).await?;
            
            // Mark as manual override if not an automatic transition
            if self.config.respect_manual_overrides {
                self.mark_manual_override(conversation_id).await;
            }
            
            // Emit event
            if let Some(ref sender) = self.event_sender {
                let _ = sender.send(AgentEvent::ConversationUpdated {
                    conversation_id,
                    old_status: old_status.clone(),
                    new_status: status,
                });
            }
        }
        
        Ok(())
    }
    
    /// Check and update conversation statuses based on rules
    async fn check_and_update_statuses(
        config: &StatusEngineConfig,
        manager: &Arc<dyn ConversationManager>,
        event_sender: &Option<mpsc::UnboundedSender<AgentEvent>>,
        manual_overrides: &Arc<RwLock<std::collections::HashSet<Uuid>>>,
    ) -> Result<()> {
        let conversations = manager.list_conversations(None).await?;
        let overrides = manual_overrides.read().await;
        
        for summary in conversations {
            // Skip manually overridden conversations
            if config.respect_manual_overrides && overrides.contains(&summary.id) {
                continue;
            }
            
            let mut needs_update = false;
            let mut new_status = summary.status.clone();
            
            // Check for inactivity (Active -> Paused)
            if summary.status == ConversationStatus::Active {
                let inactive_duration = Utc::now() - summary.last_active;
                if inactive_duration > Duration::minutes(config.inactivity_threshold_minutes) {
                    new_status = ConversationStatus::Paused;
                    needs_update = true;
                }
            }
            
            // Check for archival (Completed -> Archived)
            if summary.status == ConversationStatus::Completed {
                let age = Utc::now() - summary.last_active;
                if age > Duration::days(config.archive_threshold_days) {
                    new_status = ConversationStatus::Archived;
                    needs_update = true;
                }
            }
            
            // Update if needed
            if needs_update {
                if let Some(mut conversation) = manager.get_conversation(summary.id).await? {
                    let old_status = conversation.status.clone();
                    conversation.status = new_status.clone();
                    
                    manager.update_conversation(conversation).await?;
                    
                    // Emit event
                    if let Some(ref sender) = event_sender {
                        let _ = sender.send(AgentEvent::ConversationUpdated {
                            conversation_id: summary.id,
                            old_status,
                            new_status,
                        });
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Manually trigger a status check (useful for testing)
    pub async fn trigger_status_check(&self) -> Result<()> {
        Self::check_and_update_statuses(
            &self.config,
            &self.conversation_manager,
            &self.event_sender,
            &self.manual_overrides,
        ).await
    }
} 