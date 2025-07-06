use sagitta_code::gui::app::events::AppEvent;
use sagitta_code::gui::conversation::sidebar::types::SidebarAction;
use uuid::Uuid;

/// Test that the manual title update event is properly created and handled
#[test]
fn test_manual_title_update_event_creation() {
    let conversation_id = Uuid::new_v4();
    
    // Test SidebarAction creation
    let action = SidebarAction::UpdateConversationTitle(conversation_id);
    match action {
        SidebarAction::UpdateConversationTitle(id) => {
            assert_eq!(id, conversation_id);
        }
        _ => panic!("Expected UpdateConversationTitle action"),
    }
    
    // Test AppEvent creation
    let event = AppEvent::UpdateConversationTitle { conversation_id };
    match event {
        AppEvent::UpdateConversationTitle { conversation_id: id } => {
            assert_eq!(id, conversation_id);
        }
        _ => panic!("Expected UpdateConversationTitle event"),
    }
}

/// Test that the sidebar action is properly converted to an app event
#[test]
fn test_sidebar_action_to_app_event_conversion() {
    let conversation_id = Uuid::new_v4();
    let sidebar_action = SidebarAction::UpdateConversationTitle(conversation_id);
    
    // Simulate the conversion that happens in the sidebar's handle_sidebar_actions
    let app_event = match sidebar_action {
        SidebarAction::UpdateConversationTitle(id) => {
            Some(AppEvent::UpdateConversationTitle { conversation_id: id })
        }
        _ => None,
    };
    
    assert!(app_event.is_some());
    match app_event.unwrap() {
        AppEvent::UpdateConversationTitle { conversation_id: id } => {
            assert_eq!(id, conversation_id);
        }
        _ => panic!("Expected UpdateConversationTitle event"),
    }
}