#[cfg(test)]
mod tests {
    
    use egui::Context;
    use crate::gui::app::panels::{ActivePanel, PanelManager};
    
    #[test]
    fn test_conversation_panel_hotkey_toggle() {
        // Create a test context
        let ctx = Context::default();
        let mut panel_manager = PanelManager::new();
        
        // Initially, no panel should be active
        assert_eq!(panel_manager.active_panel, ActivePanel::None);
        
        // Simulate Ctrl+T press to open conversation panel
        panel_manager.toggle_panel(ActivePanel::Conversation);
        assert_eq!(panel_manager.active_panel, ActivePanel::Conversation);
        
        // Simulate Ctrl+T press again to close conversation panel
        panel_manager.toggle_panel(ActivePanel::Conversation);
        assert_eq!(panel_manager.active_panel, ActivePanel::None);
        
        // Test multiple toggles
        for _ in 0..5 {
            panel_manager.toggle_panel(ActivePanel::Conversation);
            assert_eq!(panel_manager.active_panel, ActivePanel::Conversation);
            
            panel_manager.toggle_panel(ActivePanel::Conversation);
            assert_eq!(panel_manager.active_panel, ActivePanel::None);
        }
    }
    
    #[test]
    fn test_panel_mutual_exclusion() {
        let mut panel_manager = PanelManager::new();
        
        // Open conversation panel
        panel_manager.toggle_panel(ActivePanel::Conversation);
        assert_eq!(panel_manager.active_panel, ActivePanel::Conversation);
        
        // Open another panel - conversation should close
        panel_manager.toggle_panel(ActivePanel::Settings);
        assert_eq!(panel_manager.active_panel, ActivePanel::Settings);
        
        // Open conversation again
        panel_manager.toggle_panel(ActivePanel::Conversation);
        assert_eq!(panel_manager.active_panel, ActivePanel::Conversation);
    }
    
    #[test]
    fn test_conversation_panel_visibility() {
        // TODO: Update this test to use the new ConversationPanel
        // For now, just verify the panel can be created
        use crate::gui::conversation::panel::ConversationPanel;
        
        let panel = ConversationPanel::new();
        assert!(panel.is_visible());
    }
}