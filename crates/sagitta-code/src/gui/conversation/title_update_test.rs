#[cfg(test)]
mod title_update_tests {
    use super::super::*;
    use crate::gui::chat::{StreamingChatManager, StreamingMessage, MessageAuthor};
    // Remove unused imports
    use std::sync::Arc;
    use tempfile::TempDir;
    
    /// Test that title updates when user sends first message
    #[test]
    fn test_title_updates_on_user_message() {
        // This test would need access to the full app context
        // For now, let's test the manager directly
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let chat_manager = Arc::new(StreamingChatManager::new());
        let mut manager = SimpleConversationManager::new(storage_path, chat_manager).unwrap();
        
        // Create conversation with default title
        let id = manager.create_conversation("New Conversation".to_string()).unwrap();
        assert_eq!(manager.current_conversation_title(), Some("New Conversation".to_string()));
        
        // Simulate what happens in rendering.rs when user sends a message
        let user_message = "How do I implement a REST API in Rust?";
        
        // This is what the rendering code does:
        if manager.current_conversation_title() == Some("New Conversation".to_string()) {
            let new_title = if user_message.len() > 50 {
                format!("{}...", &user_message[..47])
            } else {
                user_message.to_string()
            };
            manager.update_conversation_title(new_title).unwrap();
        }
        
        // Verify title was updated
        assert_eq!(manager.current_conversation_title(), Some("How do I implement a REST API in Rust?".to_string()));
        
        // Save and verify persistence
        manager.save_current_conversation().unwrap();
        let conversations = manager.list_conversations().unwrap();
        assert_eq!(conversations[0].1, "How do I implement a REST API in Rust?");
    }
    
    /// Test long messages get truncated for title
    #[test]
    fn test_long_message_title_truncation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let chat_manager = Arc::new(StreamingChatManager::new());
        let mut manager = SimpleConversationManager::new(storage_path, chat_manager).unwrap();
        
        let id = manager.create_conversation("New Conversation".to_string()).unwrap();
        
        let long_message = "I need help understanding the differences between Box, Rc, and Arc in Rust, particularly when it comes to multi-threaded applications";
        
        // Simulate title update logic
        if manager.current_conversation_title() == Some("New Conversation".to_string()) {
            let new_title = if long_message.len() > 50 {
                format!("{}...", &long_message[..47])
            } else {
                long_message.to_string()
            };
            manager.update_conversation_title(new_title).unwrap();
        }
        
        // Should be truncated to 47 chars + "..."
        assert_eq!(
            manager.current_conversation_title(), 
            Some("I need help understanding the differences betwe...".to_string())
        );
    }
}