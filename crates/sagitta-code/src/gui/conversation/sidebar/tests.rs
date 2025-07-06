use super::*;
use chrono::Utc;
use uuid::Uuid;
use crate::agent::conversation::types::{ConversationSummary, ProjectType};
use crate::agent::state::types::ConversationStatus;
use super::types::{SidebarConfig, OrganizationMode};

pub fn create_test_conversation(
    title: &str,
    status: ConversationStatus,
    _project_type: Option<ProjectType>,
    workspace_id: Option<Uuid>,
) -> ConversationSummary {
    ConversationSummary {
        id: Uuid::new_v4(),
        title: title.to_string(),
        created_at: Utc::now(),
        last_active: Utc::now(),
        message_count: 5,
        status,
        tags: vec!["test".to_string()],
        workspace_id,
        has_branches: false,
        has_checkpoints: false,
        project_name: Some("Test Project".to_string()),
    }
}

fn create_test_conversations() -> Vec<ConversationSummary> {
    vec![
        create_test_conversation("Rust talk", ConversationStatus::Active, Some(ProjectType::Rust), None),
        create_test_conversation("JS progress", ConversationStatus::Completed, Some(ProjectType::JavaScript), None),
        create_test_conversation("Python script", ConversationStatus::Paused, Some(ProjectType::Python), None),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Context;
    use crate::gui::app::AppState;
    use crate::gui::theme::AppTheme;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use crate::gui::app::events::AppEvent;
    use crate::config::SagittaCodeConfig;
    use crate::agent::conversation::service::ConversationService;

    #[test]
    fn test_sidebar_creation() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        assert_eq!(sidebar.organization_mode, OrganizationMode::Recency);
        assert!(sidebar.search_query.is_none());
        assert!(sidebar.expanded_groups.is_empty());
    }

    #[test]
    fn test_sidebar_rendering_does_not_panic() {
        // This test ensures the sidebar rendering code exists and compiles correctly
        // We can't actually run the rendering without a full egui context, but we can verify
        // that all the methods exist and are callable
        
        let sidebar = ConversationSidebar::new(SidebarConfig::default());
        let conversations = create_test_conversations();
        
        // Test that the organization method exists and works
        let result = sidebar.organize_conversations(&conversations, None);
        assert!(result.is_ok());
        
        // Test that rendering methods exist (they will be called by show())
        // If these don't compile, it means the implementation is missing
        let _show_fn: fn(&mut ConversationSidebar, &Context, &mut AppState, &AppTheme, Option<Arc<ConversationService>>, mpsc::UnboundedSender<AppEvent>, Arc<tokio::sync::Mutex<SagittaCodeConfig>>) = ConversationSidebar::show;
        
        // Verify the sidebar has all expected fields
        assert!(sidebar.expanded_groups.is_empty());
        assert!(sidebar.search_query.is_none());
    }

    #[test]
    fn test_conversation_organization() {
        let mut sidebar = ConversationSidebar::new(SidebarConfig::default());
        let conversations = create_test_conversations();
        
        // Test organizing by recency
        sidebar.set_organization_mode(OrganizationMode::Recency);
        let organized = sidebar.organize_conversations(&conversations, None).unwrap();
        assert!(!organized.groups.is_empty());
        assert_eq!(organized.total_count, 3);
        
        // Test organizing by status
        sidebar.set_organization_mode(OrganizationMode::Status);
        let organized = sidebar.organize_conversations(&conversations, None).unwrap();
        assert!(!organized.groups.is_empty());
        
        // Verify we have groups for different statuses
        let group_names: Vec<&str> = organized.groups.iter().map(|g| g.name.as_str()).collect();
        assert!(group_names.contains(&"Active"));
        assert!(group_names.contains(&"Completed"));
    }

    #[test]
    fn test_search_functionality() {
        let mut sidebar = ConversationSidebar::new(SidebarConfig::default());
        let conversations = create_test_conversations();
        
        // Test search
        sidebar.set_search_query(Some("Rust".to_string()));
        let organized = sidebar.organize_conversations(&conversations, None).unwrap();
        assert_eq!(organized.filtered_count, 1);
        
        // Clear search
        sidebar.set_search_query(None);
        let organized = sidebar.organize_conversations(&conversations, None).unwrap();
        assert_eq!(organized.filtered_count, 3);
    }

    #[test]
    fn test_toggle_functionality() {
        let mut sidebar = ConversationSidebar::new(SidebarConfig::default());
        
        // Test branch suggestions toggle
        assert!(!sidebar.show_branch_suggestions);
        sidebar.toggle_branch_suggestions();
        assert!(sidebar.show_branch_suggestions);
        sidebar.toggle_branch_suggestions();
        assert!(!sidebar.show_branch_suggestions);
        
        // Test checkpoint suggestions toggle
        assert!(!sidebar.show_checkpoint_suggestions);
        sidebar.toggle_checkpoint_suggestions();
        assert!(sidebar.show_checkpoint_suggestions);
    }

    #[test]
    fn test_group_expansion() {
        let mut sidebar = ConversationSidebar::new(SidebarConfig::default());
        
        // Test group toggle
        assert!(!sidebar.expanded_groups.contains("expanded_test_group"));
        sidebar.toggle_group("test_group");
        assert!(sidebar.expanded_groups.contains("expanded_test_group"));
        sidebar.toggle_group("test_group");
        assert!(sidebar.expanded_groups.contains("collapsed_test_group"));
    }

    #[test]
    fn test_theme_application() {
        // This test verifies that theme methods are called correctly
        let theme = AppTheme::default();
        
        // Verify theme methods exist and return valid values
        let _panel_bg = theme.panel_background();
        let _accent = theme.accent_color();
        let _muted = theme.muted_text_color();
        
        // If these compile, the theme integration is working
    }

    #[test]
    fn test_sidebar_actions() {
        let mut sidebar = ConversationSidebar::new(SidebarConfig::default());
        let conversation_id = Uuid::new_v4();
        
        // Test setting pending actions
        sidebar.pending_action = Some(SidebarAction::SwitchToConversation(conversation_id));
        assert!(matches!(sidebar.pending_action, Some(SidebarAction::SwitchToConversation(_))));
        
        // Test action handling
        let mut app_state = AppState::default();
        let ctx = Context::default();
        let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
        
        sidebar.handle_sidebar_actions(&mut app_state, &ctx, None, tx);
        
        // Verify the action was processed
        assert!(sidebar.pending_action.is_none());
        assert_eq!(app_state.current_conversation_id, Some(conversation_id));
        
        // Verify event was sent
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn test_filter_application() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        let conversations = vec![
            create_test_conversation("Active Conv", ConversationStatus::Active, Some(ProjectType::Rust), None),
            create_test_conversation("Completed Conv", ConversationStatus::Completed, Some(ProjectType::Python), None),
            create_test_conversation("Archived Conv", ConversationStatus::Archived, Some(ProjectType::JavaScript), None),
        ];
        
        // Test status filter
        let mut sidebar_with_filter = sidebar.clone();
        sidebar_with_filter.filters.statuses = vec![ConversationStatus::Active];
        let filtered = sidebar_with_filter.apply_filters(&conversations);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Active Conv");
        
        // Test tag filter
        let mut sidebar_with_tag_filter = sidebar.clone();
        sidebar_with_tag_filter.filters.tags = vec!["test".to_string()];
        let filtered = sidebar_with_tag_filter.apply_filters(&conversations);
        assert_eq!(filtered.len(), 3); // All have "test" tag
        
        // Test non-matching tag filter
        sidebar_with_tag_filter.filters.tags = vec!["nonexistent".to_string()];
        let filtered = sidebar_with_tag_filter.apply_filters(&conversations);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_search_application() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        let conversations = vec![
            create_test_conversation("Rust Programming Help", ConversationStatus::Active, Some(ProjectType::Rust), None),
            create_test_conversation("Python Data Analysis", ConversationStatus::Active, Some(ProjectType::Python), None),
            create_test_conversation("JavaScript Frontend", ConversationStatus::Active, Some(ProjectType::JavaScript), None),
        ];
        
        // Test title search
        let results = sidebar.apply_search(&conversations, "rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming Help");
        
        // Test case insensitive search
        let results = sidebar.apply_search(&conversations, "PYTHON");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Python Data Analysis");
        
        // Test partial match
        let results = sidebar.apply_search(&conversations, "data");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Python Data Analysis");
        
        // Test no match
        let results = sidebar.apply_search(&conversations, "nonexistent");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_organization_mode_switching() {
        let config = SidebarConfig::default();
        let mut sidebar = ConversationSidebar::new(config);
        
        assert_eq!(sidebar.organization_mode, OrganizationMode::Recency);
        
        sidebar.set_organization_mode(OrganizationMode::Project);
        assert_eq!(sidebar.organization_mode, OrganizationMode::Project);
        
        sidebar.set_organization_mode(OrganizationMode::Status);
        assert_eq!(sidebar.organization_mode, OrganizationMode::Status);
    }

    #[test]
    fn test_group_expansion_duplicate() {
        let config = SidebarConfig::default();
        let mut sidebar = ConversationSidebar::new(config);
        
        let group_id = "test_group";
        
        // Initially not expanded
        assert!(!sidebar.expanded_groups.contains(&format!("expanded_{group_id}")));
        
        // Toggle to expand
        sidebar.toggle_group(group_id);
        assert!(sidebar.expanded_groups.contains(&format!("expanded_{group_id}")));
        
        // Toggle to collapse
        sidebar.toggle_group(group_id);
        assert!(sidebar.expanded_groups.contains(&format!("collapsed_{group_id}")));
    }

    #[test]
    fn test_relative_time_formatting() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        let now = Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);
        let one_day_ago = now - chrono::Duration::days(1);
        let one_week_ago = now - chrono::Duration::weeks(1);
        
        let formatted = sidebar.format_relative_time(one_hour_ago);
        assert!(formatted.contains("h ago") || formatted.contains("hour"));
        
        let formatted = sidebar.format_relative_time(one_day_ago);
        assert!(formatted.contains("d ago") || formatted.contains("day"));
        
        let formatted = sidebar.format_relative_time(one_week_ago);
        // For times older than a week, it shows date format like "12/25"
        assert!(formatted.contains("/") || formatted.contains("week") || formatted.contains("7d") || formatted.contains("1w"));
    }

    #[test]
    fn test_organization_by_recency() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        let conversations = create_test_conversations();
        
        let result = sidebar.organize_by_recency(&conversations);
        assert!(result.is_ok());
        
        let groups = result.unwrap();
        assert!(!groups.is_empty());
        
        // Should have at least one group for recent conversations
        let first_group = &groups[0];
        assert!(!first_group.conversations.is_empty());
    }

    #[test]
    fn test_organization_by_status() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        let conversations = create_test_conversations();
        
        let result = sidebar.organize_by_status(&conversations);
        assert!(result.is_ok());
        
        let groups = result.unwrap();
        assert!(!groups.is_empty());
        
        // Should organize by status
        for group in &groups {
            if group.name == "Active" {
                assert_eq!(group.conversations.len(), 1);
                assert_eq!(group.conversations[0].summary.title, "Rust talk");
            }
        }
    }
    
    #[test]
    fn test_conversation_click_triggers_load() {
        // Create sidebar with test data
        let mut sidebar = ConversationSidebar::with_default_config();
        let conversations = create_test_conversations();
        
        // Create a channel to receive events
        let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
        
        // Simulate clicking on a conversation
        let conversation_id = conversations[0].id;
        sidebar.pending_action = Some(SidebarAction::SwitchToConversation(conversation_id));
        
        // Create app state
        let mut app_state = AppState::default();
        app_state.conversation_list = conversations;
        
        // Process the pending action
        sidebar.handle_sidebar_actions(
            &mut app_state,
            &Context::default(),
            None,
            tx
        );
        
        // Verify the event was sent
        let event = rx.try_recv().unwrap();
        match event {
            AppEvent::SwitchToConversation(id) => {
                assert_eq!(id, conversation_id);
            },
            _ => panic!("Expected SwitchToConversation event"),
        }
        
        // Verify state was updated
        assert_eq!(app_state.current_conversation_id, Some(conversation_id));
        assert_eq!(sidebar.selected_conversation, Some(conversation_id));
    }
} 