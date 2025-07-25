// Tests for the conversation panel implementation

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::gui::theme::AppTheme;
    use egui::{Context, Ui};
    
    /// Test that the conversation panel can be created with default settings
    #[test]
    fn test_conversation_panel_creation() {
        let panel = ConversationPanel::new();
        assert_eq!(panel.default_width(), 350.0);
        assert_eq!(panel.min_width(), 250.0);
        assert!(panel.is_resizable());
    }
    
    /// Test that the panel properly applies theme settings
    #[test]
    fn test_conversation_panel_theming() {
        let panel = ConversationPanel::new();
        let theme = AppTheme::Dark;
        
        // The panel should accept and use the theme
        assert_eq!(panel.get_background_color(&theme), theme.panel_background());
        assert_eq!(panel.get_text_color(&theme), theme.text_color());
    }
    
    /// Test that the panel can be resized within constraints
    #[test]
    fn test_conversation_panel_resizing() {
        let mut panel = ConversationPanel::new();
        
        // Test setting width
        panel.set_width(400.0);
        assert_eq!(panel.current_width(), 400.0);
        
        // Test minimum width constraint
        panel.set_width(100.0);
        assert_eq!(panel.current_width(), 250.0); // Should be clamped to min_width
        
        // Test maximum width constraint (if any)
        panel.set_width(1000.0);
        assert!(panel.current_width() <= 800.0); // Reasonable max width
    }
    
    /// Test conversation search functionality
    #[test]
    fn test_conversation_search() {
        let mut panel = ConversationPanel::new();
        let conversations = vec![
            ("Conv 1", "Hello world"),
            ("Conv 2", "Goodbye world"),
            ("Conv 3", "Hello again"),
        ];
        
        panel.set_conversations(conversations);
        
        // Test search
        panel.set_search_query("Hello");
        let results = panel.get_filtered_conversations();
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|conv| conv.title == "Conv 1"));
        assert!(results.iter().any(|conv| conv.title == "Conv 3"));
        
        // Test case-insensitive search
        panel.set_search_query("hello");
        let results = panel.get_filtered_conversations();
        assert_eq!(results.len(), 2);
        
        // Test empty search returns all
        panel.set_search_query("");
        let results = panel.get_filtered_conversations();
        assert_eq!(results.len(), 3);
    }
    
    /// Test conversation deletion functionality
    #[test]
    fn test_conversation_deletion() {
        let mut panel = ConversationPanel::new();
        let conv_id = uuid::Uuid::new_v4();
        
        panel.add_conversation(conv_id, "Test Conv", "Content");
        assert_eq!(panel.conversation_count(), 1);
        
        // Test deletion
        let result = panel.delete_conversation(conv_id);
        assert!(result.is_ok());
        assert_eq!(panel.conversation_count(), 0);
        
        // Test deleting non-existent conversation
        let result = panel.delete_conversation(uuid::Uuid::new_v4());
        assert!(result.is_err());
    }
    
    /// Test panel visibility toggle
    #[test]
    fn test_panel_visibility() {
        let mut panel = ConversationPanel::new();
        
        assert!(panel.is_visible()); // Default should be visible
        
        panel.set_visible(false);
        assert!(!panel.is_visible());
        
        panel.toggle_visibility();
        assert!(panel.is_visible());
    }
    
    /// Test panel event handling
    #[test]
    fn test_panel_events() {
        let mut panel = ConversationPanel::new();
        let conv_id = uuid::Uuid::new_v4();
        
        // Test conversation selection event
        panel.select_conversation(conv_id);
        assert_eq!(panel.selected_conversation(), Some(conv_id));
        
        // Test new conversation event
        panel.request_new_conversation();
        assert!(panel.has_pending_new_conversation());
        
        // Clear pending action
        panel.clear_pending_actions();
        assert!(!panel.has_pending_new_conversation());
    }
}