use super::*;
use chrono::{DateTime, Utc};
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

    #[test]
    fn test_sidebar_creation() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        assert_eq!(sidebar.organization_mode, OrganizationMode::Recency);
        assert!(sidebar.search_query.is_none());
        assert!(sidebar.expanded_groups.is_empty());
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
    fn test_group_expansion() {
        let config = SidebarConfig::default();
        let mut sidebar = ConversationSidebar::new(config);
        
        let group_id = "test_group";
        
        // Initially not expanded
        assert!(!sidebar.expanded_groups.contains(group_id));
        
        // Toggle to expand
        sidebar.toggle_group(group_id);
        assert!(sidebar.expanded_groups.contains(group_id));
        
        // Toggle to collapse
        sidebar.toggle_group(group_id);
        assert!(!sidebar.expanded_groups.contains(group_id));
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
} 