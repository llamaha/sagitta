use anyhow::Result;
use log::{debug, error, info};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::agent::conversation::types::{Conversation, ConversationManager};
use crate::agent::message::types::AgentMessage;
use crate::gui::app::conversation_title_updater::ConversationTitleUpdater;

/// Event triggered when a conversation should be checked for title update
#[derive(Debug, Clone)]
pub struct ConversationUpdateEvent {
    pub conversation_id: Uuid,
    pub message_count: usize,
}

/// Tracks conversation state for auto-title decisions
#[derive(Debug, Clone)]
struct ConversationState {
    /// Last time we updated the title
    last_title_update: Option<Instant>,
    /// Last message count when we checked
    last_message_count: usize,
    /// Whether the title has been manually set
    has_custom_title: bool,
}

/// Configuration for auto title updater
#[derive(Debug, Clone)]
pub struct AutoTitleConfig {
    /// Whether auto title updating is enabled
    pub enabled: bool,
    /// Minimum number of messages before generating a title
    pub min_messages: usize,
    /// Minimum time between title updates (in seconds)
    pub cooldown_seconds: u64,
    /// Only update titles that match this pattern
    pub default_title_pattern: String,
}

impl Default for AutoTitleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_messages: 4, // Need at least 2 exchanges (user + assistant)
            cooldown_seconds: 300, // 5 minutes between updates
            default_title_pattern: "Conversation 20".to_string(),
        }
    }
}

/// Service that automatically updates conversation titles
pub struct AutoTitleUpdater {
    config: AutoTitleConfig,
    title_updater: Arc<ConversationTitleUpdater>,
    conversation_states: Arc<RwLock<HashMap<Uuid, ConversationState>>>,
    update_tx: mpsc::UnboundedSender<ConversationUpdateEvent>,
    update_rx: Option<mpsc::UnboundedReceiver<ConversationUpdateEvent>>,
}

impl AutoTitleUpdater {
    /// Create a new auto title updater
    pub fn new(config: AutoTitleConfig, title_updater: Arc<ConversationTitleUpdater>) -> Self {
        let (update_tx, update_rx) = mpsc::unbounded_channel();
        
        Self {
            config,
            title_updater,
            conversation_states: Arc::new(RwLock::new(HashMap::new())),
            update_tx,
            update_rx: Some(update_rx),
        }
    }
    
    /// Start the auto title updater and return the sender for events
    pub fn start(&mut self) -> mpsc::UnboundedSender<ConversationUpdateEvent> {
        let rx = self.update_rx.take().expect("Auto title updater already started");
        let updater = self.clone_for_task();
        
        tokio::spawn(async move {
            updater.run_update_loop(rx).await;
        });
        
        self.update_tx.clone()
    }
    
    /// Clone necessary parts for the background task
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            title_updater: self.title_updater.clone(),
            conversation_states: self.conversation_states.clone(),
            update_tx: self.update_tx.clone(),
            update_rx: None,
        }
    }
    
    /// Run the update loop
    async fn run_update_loop(&self, mut update_rx: mpsc::UnboundedReceiver<ConversationUpdateEvent>) {
        info!("Starting auto title updater, enabled: {}", self.config.enabled);
        
        while let Some(event) = update_rx.recv().await {
            if !self.config.enabled {
                continue;
            }
            
            debug!("Auto title updater received event for conversation: {}", event.conversation_id);
            
            if let Err(e) = self.process_update_event(event).await {
                error!("Error processing title update: {}", e);
            }
        }
        
        info!("Auto title updater stopped");
    }
    
    /// Process a single update event
    async fn process_update_event(&self, event: ConversationUpdateEvent) -> Result<()> {
        // Check if we should update this conversation
        if !self.should_update_title(&event).await? {
            return Ok(());
        }
        
        // Update the title
        info!("Auto-updating title for conversation {}", event.conversation_id);
        
        // Call the existing title updater which already handles everything correctly
        self.title_updater.maybe_update_title(event.conversation_id).await?;
        
        // Update our state
        {
            let mut states = self.conversation_states.write().await;
            let state = states.entry(event.conversation_id).or_insert_with(|| ConversationState {
                last_title_update: None,
                last_message_count: 0,
                has_custom_title: false,
            });
            
            state.last_title_update = Some(Instant::now());
            state.last_message_count = event.message_count;
        }
        
        Ok(())
    }
    
    /// Determine if we should update the title for this conversation
    async fn should_update_title(&self, event: &ConversationUpdateEvent) -> Result<bool> {
        // Check minimum message count
        if event.message_count < self.config.min_messages {
            debug!(
                "Conversation {} has only {} messages, need at least {}",
                event.conversation_id, event.message_count, self.config.min_messages
            );
            return Ok(false);
        }
        
        let states = self.conversation_states.read().await;
        if let Some(state) = states.get(&event.conversation_id) {
            // Check if title was manually set
            if state.has_custom_title {
                debug!("Conversation {} has custom title, skipping auto-update", event.conversation_id);
                return Ok(false);
            }
            
            // Check cooldown period
            if let Some(last_update) = state.last_title_update {
                let cooldown = Duration::from_secs(self.config.cooldown_seconds);
                if last_update.elapsed() < cooldown {
                    debug!(
                        "Conversation {} in cooldown, {} seconds remaining",
                        event.conversation_id,
                        (cooldown - last_update.elapsed()).as_secs()
                    );
                    return Ok(false);
                }
            }
            
            // Check if message count has increased significantly
            if event.message_count <= state.last_message_count {
                debug!(
                    "Conversation {} message count hasn't increased ({})",
                    event.conversation_id, event.message_count
                );
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// Notify that a conversation was manually updated (to prevent auto-updates)
    pub async fn mark_custom_title(&self, conversation_id: Uuid) {
        let mut states = self.conversation_states.write().await;
        if let Some(state) = states.get_mut(&conversation_id) {
            state.has_custom_title = true;
        }
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: AutoTitleConfig) {
        self.config = config;
    }
}

/// Helper to send conversation update events when messages are added
pub fn notify_conversation_updated(
    sender: &mpsc::UnboundedSender<ConversationUpdateEvent>,
    conversation_id: Uuid,
    message_count: usize,
) {
    let event = ConversationUpdateEvent {
        conversation_id,
        message_count,
    };
    
    if let Err(e) = sender.send(event) {
        debug!("Failed to send conversation update event: {}", e);
    }
}