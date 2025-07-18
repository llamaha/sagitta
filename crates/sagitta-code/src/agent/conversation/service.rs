use anyhow::Result;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use super::types::{Conversation, ConversationSummary};
use super::clustering::{ConversationCluster, ConversationClusteringManager};
use super::analytics::{ConversationAnalyticsManager, AnalyticsReport};
use super::manager::ConversationManager;

/// Events emitted by the conversation service
#[derive(Debug, Clone)]
pub enum ConversationEvent {
    /// A conversation was created
    ConversationCreated(Uuid),
    
    /// A conversation was updated
    ConversationUpdated(Uuid),
    
    /// A conversation was deleted
    ConversationDeleted(Uuid),
    
    /// Clusters were updated
    ClustersUpdated(Vec<ConversationCluster>),
    
    /// Analytics report is ready
    AnalyticsReady(AnalyticsReport),
    
    /// Conversation list was refreshed
    ListRefreshed(Vec<ConversationSummary>),
}

/// Unified conversation service providing real-time data and events
pub struct ConversationService {
    /// Core conversation manager
    manager: Arc<dyn ConversationManager>,
    
    /// Clustering manager
    clustering_manager: Option<Arc<RwLock<ConversationClusteringManager>>>,
    
    /// Analytics manager
    analytics_manager: Arc<RwLock<ConversationAnalyticsManager>>,
    
    /// Event broadcaster
    event_sender: broadcast::Sender<ConversationEvent>,
    
    /// Cached conversation summaries
    cached_summaries: Arc<RwLock<Vec<ConversationSummary>>>,
    
    /// Cached clusters
    cached_clusters: Arc<RwLock<Vec<ConversationCluster>>>,
    
    /// Last refresh timestamp
    last_refresh: Arc<RwLock<DateTime<Utc>>>,
}

impl ConversationService {
    /// Create a new conversation service
    pub fn new(
        manager: Arc<dyn ConversationManager>,
        clustering_manager: Option<ConversationClusteringManager>,
        analytics_manager: ConversationAnalyticsManager,
    ) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        
        Self {
            manager,
            clustering_manager: clustering_manager.map(|cm| Arc::new(RwLock::new(cm))),
            analytics_manager: Arc::new(RwLock::new(analytics_manager)),
            event_sender,
            cached_summaries: Arc::new(RwLock::new(Vec::new())),
            cached_clusters: Arc::new(RwLock::new(Vec::new())),
            last_refresh: Arc::new(RwLock::new(Utc::now())),
        }
    }
    
    /// Subscribe to conversation events
    pub fn subscribe(&self) -> broadcast::Receiver<ConversationEvent> {
        self.event_sender.subscribe()
    }
    
    /// Get all conversation summaries (cached)
    pub async fn get_summaries(&self) -> Result<Vec<ConversationSummary>> {
        let summaries = self.cached_summaries.read().await;
        Ok(summaries.clone())
    }
    
    /// Get conversation clusters (cached)
    pub async fn get_clusters(&self) -> Result<Vec<ConversationCluster>> {
        let clusters = self.cached_clusters.read().await;
        Ok(clusters.clone())
    }
    
    /// Refresh all data and emit events
    pub async fn refresh(&self) -> Result<()> {
        // Refresh conversation summaries
        let summaries = self.manager.list_conversations(None).await?;
        
        // Update cache
        {
            let mut cached = self.cached_summaries.write().await;
            *cached = summaries.clone();
        }
        
        // Emit event
        let _ = self.event_sender.send(ConversationEvent::ListRefreshed(summaries.clone()));
        
        // Refresh clusters if available
        if let Some(ref clustering_manager) = self.clustering_manager {
            let clustering = clustering_manager.read().await;
            match clustering.cluster_conversations(&summaries).await {
                Ok(clustering_result) => {
                    let clusters = clustering_result.clusters;
                    
                    // Update cache
                    {
                        let mut cached = self.cached_clusters.write().await;
                        *cached = clusters.clone();
                    }
                    
                    // Emit event
                    let _ = self.event_sender.send(ConversationEvent::ClustersUpdated(clusters));
                },
                Err(e) => {
                    log::warn!("Failed to refresh clusters: {e}");
                }
            }
        }
        
        // Update last refresh time
        {
            let mut last_refresh = self.last_refresh.write().await;
            *last_refresh = Utc::now();
        }
        
        Ok(())
    }
    
    /// Create a new conversation
    pub async fn create_conversation(&self, title: String) -> Result<Uuid> {
        let conversation_id = self.manager.create_conversation(title, None).await?;
        
        // Emit event
        let _ = self.event_sender.send(ConversationEvent::ConversationCreated(conversation_id));
        
        // Refresh data
        self.refresh().await?;
        
        Ok(conversation_id)
    }
    
    /// Update a conversation
    pub async fn update_conversation(&self, conversation: Conversation) -> Result<()> {
        let conversation_id = conversation.id;
        self.manager.update_conversation(conversation).await?;
        
        // Emit event
        let _ = self.event_sender.send(ConversationEvent::ConversationUpdated(conversation_id));
        
        // Refresh data
        self.refresh().await?;
        
        Ok(())
    }
    
    /// Delete a conversation
    pub async fn delete_conversation(&self, conversation_id: Uuid) -> Result<()> {
        self.manager.delete_conversation(conversation_id).await?;
        
        // Emit event
        let _ = self.event_sender.send(ConversationEvent::ConversationDeleted(conversation_id));
        
        // Refresh data
        self.refresh().await?;
        
        Ok(())
    }
    
    /// Get a specific conversation
    pub async fn get_conversation(&self, conversation_id: Uuid) -> Result<Option<Conversation>> {
        self.manager.get_conversation(conversation_id).await
    }
    
    /// List conversations
    pub async fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        self.get_summaries().await
    }
    
    /// Rename a conversation
    pub async fn rename_conversation(&self, conversation_id: Uuid, new_title: String) -> Result<()> {
        if let Some(mut conversation) = self.manager.get_conversation(conversation_id).await? {
            conversation.title = new_title;
            self.update_conversation(conversation).await?;
        }
        Ok(())
    }
    
    /// Generate analytics report
    pub async fn generate_analytics(&self) -> Result<AnalyticsReport> {
        let summaries = self.get_summaries().await?;
        
        // Convert summaries to conversations for analytics
        let mut conversations = Vec::new();
        for summary in summaries {
            if let Some(conversation) = self.get_conversation(summary.id).await? {
                conversations.push(conversation);
            }
        }
        
        let analytics = self.analytics_manager.read().await;
        let report = analytics.generate_report(&conversations, None).await?;
        
        // Emit event
        let _ = self.event_sender.send(ConversationEvent::AnalyticsReady(report.clone()));
        
        Ok(report)
    }
    
    /// Force refresh clusters
    pub async fn refresh_clusters(&self) -> Result<()> {
        if let Some(ref clustering_manager) = self.clustering_manager {
            let summaries = self.get_summaries().await?;
            let clustering = clustering_manager.read().await;
            
            match clustering.cluster_conversations(&summaries).await {
                Ok(clustering_result) => {
                    let clusters = clustering_result.clusters;
                    
                    // Update cache
                    {
                        let mut cached = self.cached_clusters.write().await;
                        *cached = clusters.clone();
                    }
                    
                    // Emit event
                    let _ = self.event_sender.send(ConversationEvent::ClustersUpdated(clusters));
                },
                Err(e) => {
                    log::error!("Failed to refresh clusters: {e}");
                    return Err(e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Get the underlying conversation manager
    pub fn manager(&self) -> Arc<dyn ConversationManager> {
        self.manager.clone()
    }
    
    /// Get last refresh timestamp
    pub async fn last_refresh_time(&self) -> DateTime<Utc> {
        let last_refresh = self.last_refresh.read().await;
        *last_refresh
    }
    
    /// Check if data is stale (older than threshold)
    pub async fn is_stale(&self, threshold_seconds: u64) -> bool {
        let last_refresh = self.last_refresh_time().await;
        let now = Utc::now();
        let age = now.signed_duration_since(last_refresh);
        age.num_seconds() as u64 > threshold_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use crate::agent::conversation::persistence::MockConversationPersistence;
    use crate::agent::conversation::search::MockConversationSearchEngine;
    use crate::agent::conversation::types::ConversationSummary;
    use crate::conversation::ConversationManagerImpl;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use std::collections::HashMap;
    
    async fn create_test_service() -> ConversationService {
        // Create shared state for the mock to track conversations
        let conversations_state: Arc<StdMutex<HashMap<Uuid, ConversationSummary>>> = Arc::new(StdMutex::new(HashMap::new()));
        let conversations_data: Arc<StdMutex<HashMap<Uuid, Conversation>>> = Arc::new(StdMutex::new(HashMap::new()));
        
        let conversations_state_clone = Arc::clone(&conversations_state);
        let conversations_state_clone2 = Arc::clone(&conversations_state);
        let conversations_state_clone3 = Arc::clone(&conversations_state);
        let conversations_data_clone = Arc::clone(&conversations_data);
        let conversations_data_clone2 = Arc::clone(&conversations_data);
        let conversations_data_clone3 = Arc::clone(&conversations_data);
        
        let mut mock_persistence = MockConversationPersistence::new();
        mock_persistence
            .expect_list_conversation_ids()
            .returning(|| Ok(Vec::new()));
        mock_persistence
            .expect_load_conversation()
            .returning(move |id| {
                let data = conversations_data_clone.lock().unwrap();
                Ok(data.get(&id).cloned())
            });
        mock_persistence
            .expect_save_conversation()
            .returning(move |conversation| {
                let mut state = conversations_state_clone.lock().unwrap();
                let mut data = conversations_data_clone2.lock().unwrap();
                let summary = ConversationSummary {
                    id: conversation.id,
                    title: conversation.title.clone(),
                    workspace_id: conversation.workspace_id,
                    created_at: conversation.created_at,
                    last_active: conversation.last_active,
                    message_count: conversation.messages.len(),
                    tags: conversation.tags.clone(),
                    status: conversation.status.clone(),
                    has_branches: !conversation.branches.is_empty(),
                    has_checkpoints: !conversation.checkpoints.is_empty(),
                    project_name: conversation.project_context.as_ref().map(|ctx| ctx.name.clone()),
                };
                state.insert(conversation.id, summary);
                data.insert(conversation.id, conversation.clone());
                Ok(())
            });
        mock_persistence
            .expect_delete_conversation()
            .returning(move |id| {
                let mut state = conversations_state_clone2.lock().unwrap();
                let mut data = conversations_data_clone3.lock().unwrap();
                state.remove(&id);
                data.remove(&id);
                Ok(())
            });
        mock_persistence
            .expect_list_conversation_summaries()
            .returning(move |_workspace_id| {
                let state = conversations_state_clone3.lock().unwrap();
                Ok(state.values().cloned().collect())
            });
        
        let mut mock_search = MockConversationSearchEngine::new();
        mock_search
            .expect_index_conversation()
            .returning(|_| Ok(()));
        mock_search
            .expect_remove_conversation()
            .returning(|_| Ok(()));
        
        let manager = ConversationManagerImpl::new(
            Box::new(mock_persistence),
            Box::new(mock_search),
        ).await.unwrap();
        
        let analytics = ConversationAnalyticsManager::with_default_config();
        
        ConversationService::new(Arc::new(manager), None, analytics)
    }
    
    #[tokio::test]
    async fn test_service_creation() {
        let service = create_test_service().await;
        
        // Should start with empty summaries
        let summaries = service.get_summaries().await.unwrap();
        assert!(summaries.is_empty());
        
        // Should start with empty clusters
        let clusters = service.get_clusters().await.unwrap();
        assert!(clusters.is_empty());
    }
    
    #[tokio::test]
    async fn test_event_subscription() {
        let service = create_test_service().await;
        let mut receiver = service.subscribe();
        
        // Create a conversation
        let conversation_id = service.create_conversation("Test Conversation".to_string()).await.unwrap();
        
        // Should receive creation event
        let event = receiver.recv().await.unwrap();
        match event {
            ConversationEvent::ConversationCreated(id) => {
                assert_eq!(id, conversation_id);
            },
            _ => panic!("Expected ConversationCreated event"),
        }
    }
    
    #[tokio::test]
    async fn test_refresh_functionality() {
        let service = create_test_service().await;
        
        // Initial refresh should work
        service.refresh().await.unwrap();
        
        // Should not be stale immediately
        assert!(!service.is_stale(60).await);
        
        // Should have a recent refresh time
        let last_refresh = service.last_refresh_time().await;
        let now = Utc::now();
        let age = now.signed_duration_since(last_refresh);
        assert!(age.num_seconds() < 5);
    }
    
    #[tokio::test]
    async fn test_conversation_lifecycle() {
        let service = create_test_service().await;
        
        // Create conversation
        let conversation_id = service.create_conversation("Test".to_string()).await.unwrap();
        
        // Should be able to get it
        let conversation = service.get_conversation(conversation_id).await.unwrap();
        assert!(conversation.is_some());
        
        // Should appear in summaries
        let summaries = service.get_summaries().await.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, conversation_id);
        
        // Delete conversation
        service.delete_conversation(conversation_id).await.unwrap();
        
        // Should no longer exist
        let conversation = service.get_conversation(conversation_id).await.unwrap();
        assert!(conversation.is_none());
        
        // Should not appear in summaries
        let summaries = service.get_summaries().await.unwrap();
        assert!(summaries.is_empty());
    }
} 