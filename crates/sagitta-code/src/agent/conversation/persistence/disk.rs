// Disk-based conversation persistence implementation
// TODO: Implement actual disk persistence

use async_trait::async_trait;
use anyhow::Result;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::ConversationPersistence;
use crate::agent::conversation::types::{Conversation, ConversationSummary};
use crate::agent::state::types::ConversationStatus;

/// Disk-based conversation persistence
pub struct DiskConversationPersistence {
    /// Base directory for storing conversations
    storage_path: PathBuf,
    
    /// Directory for active conversations
    conversations_dir: PathBuf,
    
    /// Directory for archived conversations
    archive_dir: PathBuf,
}

impl DiskConversationPersistence {
    /// Create a new disk persistence manager
    pub async fn new(storage_path: PathBuf) -> Result<Self> {
        let conversations_dir = storage_path.join("conversations");
        let archive_dir = storage_path.join("archive");
        
        // Create directories if they don't exist
        fs::create_dir_all(&conversations_dir).await?;
        fs::create_dir_all(&archive_dir).await?;
        
        Ok(Self {
            storage_path,
            conversations_dir,
            archive_dir,
        })
    }
    
    /// Get the file path for a conversation
    fn get_conversation_path(&self, id: Uuid) -> PathBuf {
        self.conversations_dir.join(format!("{}.json", id))
    }
    
    /// Get the archive file path for a conversation
    fn get_archive_path(&self, id: Uuid) -> PathBuf {
        self.archive_dir.join(format!("{}.json", id))
    }
    
    /// Get the index file path
    fn get_index_path(&self) -> PathBuf {
        self.storage_path.join("index.json")
    }
    
    /// Load conversation index
    async fn load_index(&self) -> Result<ConversationIndex> {
        let index_path = self.get_index_path();
        
        if index_path.exists() {
            let content = match fs::read_to_string(&index_path).await {
                Ok(content) => content,
                Err(e) => {
                    log::error!("Failed to read index file {}: {}", index_path.display(), e);
                    return Err(e.into());
                }
            };
            
            match serde_json::from_str::<ConversationIndex>(&content) {
                Ok(index) => Ok(index),
                Err(e) => {
                    log::error!("Failed to parse index JSON: {}", e);
                    
                    // Move corrupted index to backup location
                    let backup_path = self.storage_path.join("index.json.corrupted");
                    if let Err(backup_err) = fs::rename(&index_path, &backup_path).await {
                        log::warn!("Failed to move corrupted index to backup: {}", backup_err);
                    } else {
                        log::info!("Moved corrupted index file to: {}", backup_path.display());
                    }
                    
                    // Return default index to allow the application to continue
                    log::info!("Creating new index after corruption");
                    Ok(ConversationIndex::default())
                }
            }
        } else {
            Ok(ConversationIndex::default())
        }
    }
    
    /// Save conversation index
    async fn save_index(&self, index: &ConversationIndex) -> Result<()> {
        let index_path = self.get_index_path();
        let content = serde_json::to_string_pretty(index)?;
        fs::write(&index_path, content).await?;
        Ok(())
    }
    
    /// Update index when conversation is saved
    async fn update_index_on_save(&self, conversation: &Conversation) -> Result<()> {
        let mut index = self.load_index().await?;
        
        // Update or add conversation entry
        let entry = ConversationIndexEntry {
            id: conversation.id,
            title: conversation.title.clone(),
            workspace_id: conversation.workspace_id,
            created_at: conversation.created_at,
            last_active: conversation.last_active,
            status: conversation.status.clone(),
            message_count: conversation.messages.len(),
            tags: conversation.tags.clone(),
        };
        
        index.conversations.insert(conversation.id, entry);
        self.save_index(&index).await?;
        Ok(())
    }
    
    /// Update index when conversation is deleted
    async fn update_index_on_delete(&self, id: Uuid) -> Result<()> {
        let mut index = self.load_index().await?;
        index.conversations.remove(&id);
        self.save_index(&index).await?;
        Ok(())
    }
    
    /// Update index when conversation is archived
    async fn update_index_on_archive(&self, id: Uuid) -> Result<()> {
        let mut index = self.load_index().await?;
        
        if let Some(entry) = index.conversations.remove(&id) {
            index.archived_conversations.insert(id, entry);
            self.save_index(&index).await?;
        }
        
        Ok(())
    }
    
    /// Update index when conversation is restored
    async fn update_index_on_restore(&self, id: Uuid) -> Result<()> {
        let mut index = self.load_index().await?;
        
        if let Some(entry) = index.archived_conversations.remove(&id) {
            index.conversations.insert(id, entry);
            self.save_index(&index).await?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl ConversationPersistence for DiskConversationPersistence {
    async fn save_conversation(&self, conversation: &Conversation) -> Result<()> {
        let file_path = self.get_conversation_path(conversation.id);
        let temp_file_path = self.conversations_dir.join(format!("{}.json.tmp", conversation.id));
        
        let content = serde_json::to_string_pretty(conversation)?;
        
        // Write to a temporary file first
        let mut temp_file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_file_path)
            .await?;
        temp_file.write_all(content.as_bytes()).await?;
        temp_file.sync_all().await?; // Ensure data is flushed to disk
        drop(temp_file); // Close the file explicitly before renaming

        // Rename the temporary file to the final destination
        fs::rename(&temp_file_path, &file_path).await?;
        
        // Update the index only after successful save and rename
        self.update_index_on_save(conversation).await?;
        
        Ok(())
    }
    
    async fn load_conversation(&self, id: Uuid) -> Result<Option<Conversation>> {
        let file_path = self.get_conversation_path(id);
        
        if file_path.exists() {
            let content = match fs::read_to_string(&file_path).await {
                Ok(content) => content,
                Err(e) => {
                    log::error!("Failed to read conversation file {}: {}", file_path.display(), e);
                    return Err(e.into());
                }
            };
            
            match serde_json::from_str::<Conversation>(&content) {
                Ok(conversation) => Ok(Some(conversation)),
                Err(e) => {
                    log::error!("Failed to parse conversation JSON for {}: {}", id, e);
                    
                    // Move corrupted file to backup location
                    let backup_path = self.storage_path.join("corrupted").join(format!("{}.json.corrupted", id));
                    if let Err(backup_err) = fs::create_dir_all(backup_path.parent().unwrap()).await {
                        log::warn!("Failed to create corrupted backup directory: {}", backup_err);
                    } else if let Err(backup_err) = fs::rename(&file_path, &backup_path).await {
                        log::warn!("Failed to move corrupted file to backup: {}", backup_err);
                    } else {
                        log::info!("Moved corrupted conversation file to: {}", backup_path.display());
                    }
                    
                    // Remove from index to prevent future load attempts
                    if let Err(index_err) = self.update_index_on_delete(id).await {
                        log::warn!("Failed to update index after removing corrupted conversation: {}", index_err);
                    }
                    
                    // Return None instead of error to allow the application to continue
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }
    
    async fn delete_conversation(&self, id: Uuid) -> Result<()> {
        let file_path = self.get_conversation_path(id);
        
        if file_path.exists() {
            fs::remove_file(&file_path).await?;
        }
        
        self.update_index_on_delete(id).await?;
        Ok(())
    }
    
    async fn list_conversation_ids(&self) -> Result<Vec<Uuid>> {
        let index = self.load_index().await?;
        Ok(index.conversations.keys().copied().collect())
    }
    
    async fn list_conversation_summaries(&self, workspace_id: Option<Uuid>) -> Result<Vec<ConversationSummary>> {
        let index = self.load_index().await?;
        
        let summaries: Vec<ConversationSummary> = index.conversations
            .values()
            .filter(|entry| {
                // Filter by workspace if specified
                workspace_id.map_or(true, |ws_id| entry.workspace_id == Some(ws_id))
            })
            .map(|entry| ConversationSummary {
                id: entry.id,
                title: entry.title.clone(),
                workspace_id: entry.workspace_id,
                created_at: entry.created_at,
                last_active: entry.last_active,
                status: entry.status.clone(),
                message_count: entry.message_count,
                tags: entry.tags.clone(),
                project_name: None, // Not stored in index for performance
                has_branches: false, // Not stored in index for performance
                has_checkpoints: false, // Not stored in index for performance
            })
            .collect();
        
        log::debug!("Loaded {} conversation summaries from index (workspace filter: {:?})", 
                   summaries.len(), workspace_id);
        
        Ok(summaries)
    }
    
    async fn archive_conversation(&self, id: Uuid) -> Result<()> {
        let conversation_path = self.get_conversation_path(id);
        let archive_path = self.get_archive_path(id);
        
        if conversation_path.exists() {
            // Move file from conversations to archive
            fs::rename(&conversation_path, &archive_path).await?;
            self.update_index_on_archive(id).await?;
        }
        
        Ok(())
    }
    
    async fn list_archived_conversation_ids(&self) -> Result<Vec<Uuid>> {
        let index = self.load_index().await?;
        Ok(index.archived_conversations.keys().copied().collect())
    }
    
    async fn restore_conversation(&self, id: Uuid) -> Result<()> {
        let archive_path = self.get_archive_path(id);
        let conversation_path = self.get_conversation_path(id);
        
        if archive_path.exists() {
            // Move file from archive to conversations
            fs::rename(&archive_path, &conversation_path).await?;
            self.update_index_on_restore(id).await?;
        }
        
        Ok(())
    }
}

/// Conversation index for fast lookups
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct ConversationIndex {
    /// Active conversations
    conversations: std::collections::HashMap<Uuid, ConversationIndexEntry>,
    
    /// Archived conversations
    archived_conversations: std::collections::HashMap<Uuid, ConversationIndexEntry>,
    
    /// Index version for future migrations
    version: u32,
}

/// Index entry for a conversation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ConversationIndexEntry {
    id: Uuid,
    title: String,
    workspace_id: Option<Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_active: chrono::DateTime<chrono::Utc>,
    status: ConversationStatus,
    message_count: usize,
    tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::agent::conversation::types::Conversation;

    #[tokio::test]
    async fn test_disk_persistence_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        
        let persistence = DiskConversationPersistence::new(storage_path.clone()).await.unwrap();
        
        // Check that directories were created
        assert!(storage_path.join("conversations").exists());
        assert!(storage_path.join("archive").exists());
    }
    
    #[tokio::test]
    async fn test_save_and_load_conversation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let persistence = DiskConversationPersistence::new(storage_path).await.unwrap();
        
        let conversation = Conversation::new("Test Conversation".to_string(), None);
        let conversation_id = conversation.id;
        
        // Save conversation
        persistence.save_conversation(&conversation).await.unwrap();
        
        // Load conversation
        let loaded = persistence.load_conversation(conversation_id).await.unwrap();
        assert!(loaded.is_some());
        
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, conversation_id);
        assert_eq!(loaded.title, "Test Conversation");
    }
    
    #[tokio::test]
    async fn test_delete_conversation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let persistence = DiskConversationPersistence::new(storage_path).await.unwrap();
        
        let conversation = Conversation::new("Test Conversation".to_string(), None);
        let conversation_id = conversation.id;
        
        // Save and then delete
        persistence.save_conversation(&conversation).await.unwrap();
        persistence.delete_conversation(conversation_id).await.unwrap();
        
        // Should not be loadable
        let loaded = persistence.load_conversation(conversation_id).await.unwrap();
        assert!(loaded.is_none());
    }
    
    #[tokio::test]
    async fn test_list_conversation_ids() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let persistence = DiskConversationPersistence::new(storage_path).await.unwrap();
        
        let conv1 = Conversation::new("Conv 1".to_string(), None);
        let conv2 = Conversation::new("Conv 2".to_string(), None);
        
        persistence.save_conversation(&conv1).await.unwrap();
        persistence.save_conversation(&conv2).await.unwrap();
        
        let ids = persistence.list_conversation_ids().await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&conv1.id));
        assert!(ids.contains(&conv2.id));
    }
    
    #[tokio::test]
    async fn test_archive_and_restore() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let persistence = DiskConversationPersistence::new(storage_path).await.unwrap();
        
        let conversation = Conversation::new("Test Conversation".to_string(), None);
        let conversation_id = conversation.id;
        
        // Save conversation
        persistence.save_conversation(&conversation).await.unwrap();
        
        // Archive it
        persistence.archive_conversation(conversation_id).await.unwrap();
        
        // Should not be in active conversations
        let active_ids = persistence.list_conversation_ids().await.unwrap();
        assert!(!active_ids.contains(&conversation_id));
        
        // Should be in archived conversations
        let archived_ids = persistence.list_archived_conversation_ids().await.unwrap();
        assert!(archived_ids.contains(&conversation_id));
        
        // Restore it
        persistence.restore_conversation(conversation_id).await.unwrap();
        
        // Should be back in active conversations
        let active_ids = persistence.list_conversation_ids().await.unwrap();
        assert!(active_ids.contains(&conversation_id));
        
        // Should not be in archived conversations
        let archived_ids = persistence.list_archived_conversation_ids().await.unwrap();
        assert!(!archived_ids.contains(&conversation_id));
    }
    
    #[tokio::test]
    async fn test_index_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let persistence = DiskConversationPersistence::new(storage_path.clone()).await.unwrap();
        
        let conversation = Conversation::new("Test Conversation".to_string(), None);
        persistence.save_conversation(&conversation).await.unwrap();
        
        // Create new persistence instance to test index loading
        let persistence2 = DiskConversationPersistence::new(storage_path).await.unwrap();
        let ids = persistence2.list_conversation_ids().await.unwrap();
        
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&conversation.id));
    }
    
    #[tokio::test]
    async fn test_corrupted_conversation_handling() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let persistence = DiskConversationPersistence::new(storage_path.clone()).await.unwrap();
        
        let conversation_id = Uuid::new_v4();
        let corrupted_file_path = persistence.get_conversation_path(conversation_id);
        
        // Create a corrupted JSON file (missing closing brace)
        let corrupted_content = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Test Conversation",
            "created_at": "2023-01-01T00:00:00Z",
            "last_active": "2023-01-01T00:00:00Z",
            "messages": [],
            "status": "Active",
            "workspace_id": null,
            "tags": [],
            "branches": [],
            "checkpoints": [],
            "project_context": null
        "#; // Missing closing brace
        
        fs::write(&corrupted_file_path, corrupted_content).await.unwrap();
        
        // Try to load the corrupted conversation - should return None instead of error
        let result = persistence.load_conversation(conversation_id).await.unwrap();
        assert!(result.is_none());
        
        // Check that corrupted file was moved to backup
        let backup_path = storage_path.join("corrupted").join(format!("{}.json.corrupted", conversation_id));
        assert!(backup_path.exists());
        
        // Original file should be gone
        assert!(!corrupted_file_path.exists());
    }
    
    #[tokio::test]
    async fn test_corrupted_index_handling() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        
        // Create corrupted index file
        let index_path = storage_path.join("index.json");
        let corrupted_index = r#"{
            "conversations": {},
            "archived_conversations": {},
            "version": 0
        "#; // Missing closing brace
        
        fs::create_dir_all(&storage_path).await.unwrap();
        fs::write(&index_path, corrupted_index).await.unwrap();
        
        // Creating persistence should handle corrupted index gracefully
        let persistence = DiskConversationPersistence::new(storage_path.clone()).await.unwrap();
        
        // Should be able to load index (returns default)
        let index = persistence.load_index().await.unwrap();
        assert_eq!(index.conversations.len(), 0);
        
        // Check that corrupted index was moved to backup
        let backup_path = storage_path.join("index.json.corrupted");
        assert!(backup_path.exists());
    }
} 