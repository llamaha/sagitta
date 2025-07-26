#[cfg(test)]
mod comprehensive_tests {
    use super::super::*;
    use crate::gui::chat::{StreamingChatManager, StreamingMessage, MessageAuthor};
    use crate::gui::theme::AppTheme;
    use std::sync::Arc;
    use tempfile::TempDir;
    use uuid::Uuid;
    
    /// Test that conversation titles update based on first user message
    #[test]
    fn test_conversation_title_updates_on_first_message() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let chat_manager = Arc::new(StreamingChatManager::new());
        let mut manager = SimpleConversationManager::new(storage_path, chat_manager.clone()).unwrap();
        
        // Create a new conversation
        let id = manager.create_conversation("New Conversation".to_string()).unwrap();
        assert_eq!(manager.current_conversation_title(), Some("New Conversation".to_string()));
        
        // Add a user message
        let user_message = StreamingMessage::from_text(MessageAuthor::User, "Help me implement a binary search tree".to_string());
        manager.add_message(user_message.clone()).unwrap();
        chat_manager.add_complete_message(user_message);
        
        // Title should update to reflect the first user message
        manager.update_conversation_title("Help me implement a binary search tree".to_string()).unwrap();
        assert_eq!(manager.current_conversation_title(), Some("Help me implement a binary search tree".to_string()));
        
        // Verify it persists
        manager.save_current_conversation().unwrap();
        let conversations = manager.list_conversations().unwrap();
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].1, "Help me implement a binary search tree");
    }
    
    /// Test that clicking a conversation switches to it
    #[test]
    fn test_conversation_switching() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let chat_manager = Arc::new(StreamingChatManager::new());
        let mut manager = SimpleConversationManager::new(storage_path, chat_manager.clone()).unwrap();
        
        // Create two conversations
        let id1 = manager.create_conversation("Conversation 1".to_string()).unwrap();
        
        // Add a message to conversation 1
        let msg1 = StreamingMessage::from_text(MessageAuthor::User, "Message in conv 1".to_string());
        chat_manager.add_complete_message(msg1.clone());
        manager.add_message(msg1).unwrap();
        manager.save_current_conversation().unwrap();
        
        let id2 = manager.create_conversation("Conversation 2".to_string()).unwrap();
        
        // Add a message to conversation 2
        let msg2 = StreamingMessage::from_text(MessageAuthor::User, "Message in conv 2".to_string());
        chat_manager.add_complete_message(msg2.clone());
        manager.add_message(msg2).unwrap();
        
        // Current should be conversation 2
        assert_eq!(manager.current_conversation_id(), Some(id2));
        assert_eq!(chat_manager.get_all_messages().len(), 1);
        assert_eq!(chat_manager.get_all_messages()[0].content, "Message in conv 2");
        
        // Switch back to conversation 1
        manager.switch_conversation(id1).unwrap();
        assert_eq!(manager.current_conversation_id(), Some(id1));
        
        // Chat manager should now have conversation 1's messages
        let messages = chat_manager.get_all_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Message in conv 1");
    }
    
    /// Test panel interaction and clicking behavior
    #[test]
    fn test_panel_click_actions() {
        let mut panel = ConversationPanel::new();
        
        // Add some conversations
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        panel.add_conversation(id1, "First Conversation", "Preview 1");
        panel.add_conversation(id2, "Second Conversation", "Preview 2");
        
        // Test selection
        panel.select_conversation(id1);
        assert_eq!(panel.selected_conversation(), Some(id1));
        
        // Simulate clicking on conversation 2
        panel.select_conversation(id2);
        panel.set_pending_action(PanelAction::SelectConversation(id2));
        
        // Take the action
        let action = panel.take_pending_action();
        assert_eq!(action, Some(PanelAction::SelectConversation(id2)));
        assert_eq!(panel.selected_conversation(), Some(id2));
        
        // Test new conversation action
        panel.request_new_conversation();
        panel.set_pending_action(PanelAction::CreateNewConversation);
        let action = panel.take_pending_action();
        assert_eq!(action, Some(PanelAction::CreateNewConversation));
    }
    
    /// Test that conversations persist across app restarts
    #[test]
    fn test_conversation_persistence_across_restarts() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        
        let conv_id;
        let conv_title = "Persistent Conversation";
        
        // Create and save a conversation
        {
            let chat_manager = Arc::new(StreamingChatManager::new());
            let mut manager = SimpleConversationManager::new(storage_path.clone(), chat_manager.clone()).unwrap();
            
            conv_id = manager.create_conversation(conv_title.to_string()).unwrap();
            
            // Add some messages
            let msg = StreamingMessage::from_text(MessageAuthor::User, "Test message".to_string());
            chat_manager.add_complete_message(msg.clone());
            manager.add_message(msg).unwrap();
            manager.save_current_conversation().unwrap();
        }
        
        // Create a new manager instance (simulating app restart)
        {
            let chat_manager = Arc::new(StreamingChatManager::new());
            let manager = SimpleConversationManager::new(storage_path, chat_manager).unwrap();
            
            let conversations = manager.list_conversations().unwrap();
            assert_eq!(conversations.len(), 1);
            assert_eq!(conversations[0].0, conv_id);
            assert_eq!(conversations[0].1, conv_title);
        }
    }
    
    /// Test auto-title generation from first user message
    #[test]
    fn test_auto_title_generation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let chat_manager = Arc::new(StreamingChatManager::new());
        let mut manager = SimpleConversationManager::new(storage_path, chat_manager.clone()).unwrap();
        
        // Create a new conversation
        let id = manager.create_conversation("New Conversation".to_string()).unwrap();
        
        // Test short message becomes title
        let short_msg = "How do I use async/await in Rust?";
        manager.update_conversation_title(short_msg.to_string()).unwrap();
        assert_eq!(manager.current_conversation_title(), Some(short_msg.to_string()));
        
        // Test long message gets truncated
        let long_msg = "I need help understanding the differences between Box, Rc, and Arc in Rust, particularly when it comes to multi-threaded applications and shared ownership patterns";
        manager.update_conversation_title(long_msg[..47].to_string() + "...").unwrap();
        assert_eq!(manager.current_conversation_title(), Some("I need help understanding the differences betwe...".to_string()));
    }
    
    /// Test conversation list ordering (newest first)
    #[test]
    fn test_conversation_list_ordering() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        let chat_manager = Arc::new(StreamingChatManager::new());
        let mut manager = SimpleConversationManager::new(storage_path, chat_manager).unwrap();
        
        // Create conversations with some delay
        let id1 = manager.create_conversation("Old Conversation".to_string()).unwrap();
        manager.save_current_conversation().unwrap();
        
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        let id2 = manager.create_conversation("New Conversation".to_string()).unwrap();
        manager.save_current_conversation().unwrap();
        
        let conversations = manager.list_conversations().unwrap();
        assert_eq!(conversations.len(), 2);
        
        // Newest should be first
        assert_eq!(conversations[0].0, id2);
        assert_eq!(conversations[0].1, "New Conversation");
        assert_eq!(conversations[1].0, id1);
        assert_eq!(conversations[1].1, "Old Conversation");
    }
    
    /// Test that panel actions are properly handled
    #[test]
    fn test_panel_action_handling() {
        let mut panel = ConversationPanel::new();
        
        // Initially no pending actions
        assert!(panel.take_pending_action().is_none());
        
        // Test delete action
        let conv_id = Uuid::new_v4();
        panel.add_conversation(conv_id, "To Delete", "Preview");
        panel.set_pending_action(PanelAction::DeleteConversation(conv_id));
        
        let action = panel.take_pending_action();
        assert_eq!(action, Some(PanelAction::DeleteConversation(conv_id)));
        
        // Test rename action
        let new_title = "Renamed Conversation".to_string();
        panel.set_pending_action(PanelAction::RenameConversation(conv_id, new_title.clone()));
        
        let action = panel.take_pending_action();
        assert_eq!(action, Some(PanelAction::RenameConversation(conv_id, new_title)));
    }
}