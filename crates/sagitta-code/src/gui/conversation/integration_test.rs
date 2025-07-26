#[cfg(test)]
mod integration_tests {
    use super::super::*;
    use crate::gui::chat::StreamingChatManager;
    use std::sync::Arc;
    use tempfile::TempDir;
    
    #[test]
    fn test_conversation_creation_and_listing() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        
        // Create chat manager
        let chat_manager = Arc::new(StreamingChatManager::new());
        
        // Create simple conversation manager
        let mut manager = SimpleConversationManager::new(storage_path.clone(), chat_manager).unwrap();
        
        // Initially should have no conversations
        let conversations = manager.list_conversations().unwrap();
        assert_eq!(conversations.len(), 0, "Should start with no conversations");
        
        // Create a conversation
        let id1 = manager.create_conversation("Test Conversation 1".to_string()).unwrap();
        
        // Should now have one conversation
        let conversations = manager.list_conversations().unwrap();
        assert_eq!(conversations.len(), 1, "Should have one conversation after creation");
        assert_eq!(conversations[0].0, id1);
        assert_eq!(conversations[0].1, "Test Conversation 1");
        
        // Create another conversation
        let id2 = manager.create_conversation("Test Conversation 2".to_string()).unwrap();
        
        // Should now have two conversations
        let conversations = manager.list_conversations().unwrap();
        assert_eq!(conversations.len(), 2, "Should have two conversations after second creation");
        
        // Verify both conversations are in the list
        let ids: Vec<_> = conversations.iter().map(|(id, _, _)| *id).collect();
        assert!(ids.contains(&id1), "First conversation should be in list");
        assert!(ids.contains(&id2), "Second conversation should be in list");
        
        // Create a new manager instance and verify persistence
        let chat_manager2 = Arc::new(StreamingChatManager::new());
        let manager2 = SimpleConversationManager::new(storage_path, chat_manager2).unwrap();
        
        let persisted_conversations = manager2.list_conversations().unwrap();
        assert_eq!(persisted_conversations.len(), 2, "Conversations should be persisted to disk");
    }
    
    #[test]
    fn test_current_conversation_in_list() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        
        // Create chat manager
        let chat_manager = Arc::new(StreamingChatManager::new());
        
        // Create simple conversation manager
        let mut manager = SimpleConversationManager::new(storage_path, chat_manager).unwrap();
        
        // Create a conversation but don't save it yet
        let id = manager.create_conversation("Unsaved Conversation".to_string()).unwrap();
        
        // The current conversation should appear in the list even if not explicitly saved
        let conversations = manager.list_conversations().unwrap();
        assert_eq!(conversations.len(), 1, "Current conversation should appear in list");
        assert_eq!(conversations[0].0, id);
        assert_eq!(conversations[0].1, "Unsaved Conversation");
    }
}