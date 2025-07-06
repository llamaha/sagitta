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
    fn test_conversation_sidebar_visibility() {
        use crate::gui::conversation::sidebar::types::ConversationSidebar;
        use crate::gui::conversation::sidebar::types::SidebarConfig;
        use crate::gui::app::AppState;
        use crate::gui::theme::AppTheme;
        use std::sync::Arc;
        use tokio::sync::mpsc;
        use crate::gui::app::events::AppEvent;
        use crate::config::SagittaCodeConfig;
        use crate::agent::conversation::service::ConversationService;
        
        // Create a sidebar instance
        let mut sidebar = ConversationSidebar::new(SidebarConfig::default());
        
        // Verify the show method exists with the correct signature
        let _show_method: fn(&mut ConversationSidebar, &Context, &mut AppState, &AppTheme, Option<Arc<ConversationService>>, mpsc::UnboundedSender<AppEvent>, Arc<tokio::sync::Mutex<SagittaCodeConfig>>) = ConversationSidebar::show;
        
        // Verify sidebar maintains its state
        assert_eq!(sidebar.organization_mode, crate::gui::conversation::sidebar::types::OrganizationMode::Recency);
        
        // Test that sidebar can be toggled
        sidebar.toggle_branch_suggestions();
        assert!(sidebar.show_branch_suggestions);
        
        sidebar.toggle_checkpoint_suggestions();
        assert!(sidebar.show_checkpoint_suggestions);
    }
}