use async_trait::async_trait;
use tempfile::TempDir;
use uuid::Uuid;
use super::{ConversationPersistence, disk::DiskConversationPersistence};
use crate::agent::conversation::types::{Conversation, ConversationSummary};
use anyhow::Result;

/// Test conversation persistence that uses a temporary directory
pub struct TestConversationPersistence {
    _temp_dir: TempDir,
    inner: DiskConversationPersistence,
}

impl TestConversationPersistence {
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let inner = DiskConversationPersistence::new(temp_dir.path().to_path_buf()).await?;
        
        Ok(Self {
            _temp_dir: temp_dir,
            inner,
        })
    }
}

#[async_trait]
impl ConversationPersistence for TestConversationPersistence {
    async fn save_conversation(&self, conversation: &Conversation) -> Result<()> {
        self.inner.save_conversation(conversation).await
    }
    
    async fn load_conversation(&self, id: Uuid) -> Result<Option<Conversation>> {
        self.inner.load_conversation(id).await
    }
    
    async fn delete_conversation(&self, id: Uuid) -> Result<()> {
        self.inner.delete_conversation(id).await
    }
    
    async fn list_conversation_ids(&self) -> Result<Vec<Uuid>> {
        self.inner.list_conversation_ids().await
    }
    
    async fn list_conversation_summaries(&self, workspace_id: Option<Uuid>) -> Result<Vec<ConversationSummary>> {
        self.inner.list_conversation_summaries(workspace_id).await
    }
    
    async fn archive_conversation(&self, id: Uuid) -> Result<()> {
        self.inner.archive_conversation(id).await
    }
    
    async fn list_archived_conversation_ids(&self) -> Result<Vec<Uuid>> {
        self.inner.list_archived_conversation_ids().await
    }
    
    async fn restore_conversation(&self, id: Uuid) -> Result<()> {
        self.inner.restore_conversation(id).await
    }
} 