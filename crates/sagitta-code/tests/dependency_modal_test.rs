use sagitta_code::gui::repository::dependency_modal::{DependencyModal, AddDependencyForm};
use sagitta_search::config::RepositoryDependency;

#[test]
fn test_dependency_modal_initialization() {
    let modal = DependencyModal::default();
    
    assert!(!modal.visible);
    assert!(modal.repository_name.is_empty());
    assert!(modal.dependencies.is_empty());
    assert!(modal.error_message.is_none());
    assert!(modal.success_message.is_none());
    assert!(!modal.is_saving);
    assert!(modal.confirm_remove.is_none());
}

#[test]
fn test_dependency_modal_show_for_repository() {
    let mut modal = DependencyModal::default();
    
    let dependencies = vec![
        RepositoryDependency {
            repository_name: "test-dep".to_string(),
            target_ref: Some("v1.0".to_string()),
            purpose: Some("Testing".to_string()),
        },
    ];
    
    modal.show_for_repository("test-repo".to_string(), dependencies.clone());
    
    assert!(modal.visible);
    assert_eq!(modal.repository_name, "test-repo");
    assert_eq!(modal.dependencies.len(), 1);
    assert_eq!(modal.dependencies[0].repository_name, "test-dep");
    assert!(modal.error_message.is_none());
    assert!(modal.success_message.is_none());
    
    // Form should be reset
    assert!(modal.add_form.selected_repository.is_empty());
    assert!(modal.add_form.target_ref.is_empty());
    assert!(modal.add_form.purpose.is_empty());
    assert!(!modal.add_form.is_adding);
}

#[test]
fn test_dependency_modal_hide() {
    let mut modal = DependencyModal::default();
    modal.visible = true;
    modal.confirm_remove = Some(0);
    
    modal.hide();
    
    assert!(!modal.visible);
    assert!(modal.confirm_remove.is_none());
}

#[test]
fn test_add_dependency_form_initialization() {
    let form = AddDependencyForm::default();
    
    assert!(form.selected_repository.is_empty());
    assert!(form.target_ref.is_empty());
    assert!(form.purpose.is_empty());
    assert!(!form.is_adding);
}

#[test]
fn test_dependency_modal_add_dependency() {
    let mut modal = DependencyModal::default();
    modal.add_form.selected_repository = "new-dep".to_string();
    modal.add_form.target_ref = "v2.0".to_string();
    modal.add_form.purpose = "New dependency".to_string();
    
    modal.add_dependency();
    
    assert_eq!(modal.dependencies.len(), 1);
    assert_eq!(modal.dependencies[0].repository_name, "new-dep");
    assert_eq!(modal.dependencies[0].target_ref, Some("v2.0".to_string()));
    assert_eq!(modal.dependencies[0].purpose, Some("New dependency".to_string()));
    
    // Form should be reset
    assert!(modal.add_form.selected_repository.is_empty());
    assert!(modal.add_form.target_ref.is_empty());
    assert!(modal.add_form.purpose.is_empty());
    
    // Success message should be set
    assert!(modal.success_message.is_some());
    assert!(modal.success_message.as_ref().unwrap().contains("added"));
}

#[test]
fn test_dependency_modal_add_dependency_empty_optional_fields() {
    let mut modal = DependencyModal::default();
    modal.add_form.selected_repository = "new-dep".to_string();
    modal.add_form.target_ref = "".to_string(); // Empty
    modal.add_form.purpose = "".to_string();    // Empty
    
    modal.add_dependency();
    
    assert_eq!(modal.dependencies.len(), 1);
    assert_eq!(modal.dependencies[0].repository_name, "new-dep");
    assert_eq!(modal.dependencies[0].target_ref, None); // Should be None, not empty string
    assert_eq!(modal.dependencies[0].purpose, None);   // Should be None, not empty string
}

#[test]
fn test_dependency_modal_add_dependency_whitespace_handling() {
    let mut modal = DependencyModal::default();
    modal.add_form.selected_repository = "new-dep".to_string();
    modal.add_form.target_ref = "  v2.0  ".to_string(); // Whitespace around
    modal.add_form.purpose = "  Test purpose  ".to_string(); // Whitespace around
    
    modal.add_dependency();
    
    assert_eq!(modal.dependencies.len(), 1);
    // Fixed: Now properly trims whitespace
    assert_eq!(modal.dependencies[0].target_ref, Some("v2.0".to_string()));
    assert_eq!(modal.dependencies[0].purpose, Some("Test purpose".to_string()));
}

#[test]
fn test_dependency_modal_multiple_dependencies() {
    let mut modal = DependencyModal::default();
    
    // Add first dependency
    modal.add_form.selected_repository = "dep-1".to_string();
    modal.add_form.target_ref = "v1.0".to_string();
    modal.add_form.purpose = "First".to_string();
    modal.add_dependency();
    
    // Add second dependency
    modal.add_form.selected_repository = "dep-2".to_string();
    modal.add_form.target_ref = "v2.0".to_string();
    modal.add_form.purpose = "Second".to_string();
    modal.add_dependency();
    
    assert_eq!(modal.dependencies.len(), 2);
    assert_eq!(modal.dependencies[0].repository_name, "dep-1");
    assert_eq!(modal.dependencies[1].repository_name, "dep-2");
}

#[test]
fn test_dependency_modal_duplicate_dependency_prevention() {
    let mut modal = DependencyModal::default();
    
    // Add initial dependency
    modal.dependencies.push(RepositoryDependency {
        repository_name: "existing-dep".to_string(),
        target_ref: Some("v1.0".to_string()),
        purpose: Some("Existing".to_string()),
    });
    
    // Try to add the same dependency again
    modal.add_form.selected_repository = "existing-dep".to_string();
    modal.add_form.target_ref = "v2.0".to_string();
    modal.add_form.purpose = "Updated".to_string();
    modal.add_dependency();
    
    // Fixed: Implementation now correctly prevents duplicates
    assert_eq!(modal.dependencies.len(), 1); // Should reject duplicate
    assert_eq!(modal.dependencies[0].target_ref, Some("v1.0".to_string())); // Original should be unchanged
    
    // Should show error message (form remains intact for better UX)
    assert!(modal.error_message.is_some());
    assert!(modal.error_message.as_ref().unwrap().contains("already exists"));
}

#[test]
fn test_dependency_modal_save_dependencies_async_handling() {
    // This test documents the current async handling issues
    let mut modal = DependencyModal::default();
    modal.repository_name = "test-repo".to_string();
    modal.dependencies = vec![
        RepositoryDependency {
            repository_name: "dep-1".to_string(),
            target_ref: Some("v1.0".to_string()),
            purpose: Some("Test".to_string()),
        },
    ];
    
    // FIXME: The current save_dependencies implementation has several issues:
    // 1. It uses Handle::current() which might not be available in tests
    // 2. It doesn't properly wait for the async operation to complete
    // 3. It assumes success without checking the actual result
    // 4. It doesn't handle errors properly
    
    // The method should be refactored to return a Future or use proper async handling
    
    assert!(!modal.is_saving);
    // Can't test save_dependencies here due to async handling issues
}

#[test]
fn test_dependency_modal_error_handling() {
    let mut modal = DependencyModal::default();
    
    // Test setting error message
    modal.error_message = Some("Test error".to_string());
    modal.success_message = Some("Previous success".to_string());
    
    // When showing for repository, messages should be cleared
    modal.show_for_repository("test-repo".to_string(), vec![]);
    
    assert!(modal.error_message.is_none());
    assert!(modal.success_message.is_none());
}

#[test]
fn test_dependency_modal_confirm_remove_state() {
    let mut modal = DependencyModal::default();
    modal.dependencies = vec![
        RepositoryDependency {
            repository_name: "dep-1".to_string(),
            target_ref: None,
            purpose: None,
        },
        RepositoryDependency {
            repository_name: "dep-2".to_string(),
            target_ref: None,
            purpose: None,
        },
    ];
    
    // Test setting confirm remove state
    modal.confirm_remove = Some(1);
    assert_eq!(modal.confirm_remove, Some(1));
    
    // Test clearing confirm remove state via hide
    modal.hide();
    assert!(modal.confirm_remove.is_none());
}

#[test]
fn test_dependency_modal_repository_filtering() {
    // Test that the modal properly filters available repositories
    let available_repos = vec![
        "main-repo".to_string(),
        "dep-1".to_string(),
        "dep-2".to_string(),
        "dep-3".to_string(),
    ];
    
    let mut modal = DependencyModal::default();
    modal.repository_name = "main-repo".to_string();
    modal.dependencies = vec![
        RepositoryDependency {
            repository_name: "dep-1".to_string(),
            target_ref: None,
            purpose: None,
        },
    ];
    
    // When rendering the combo box, it should filter out:
    // 1. The current repository ("main-repo")
    // 2. Already added dependencies ("dep-1")
    // Leaving: "dep-2", "dep-3"
    
    // The actual filtering logic is in the render method
    // We can't easily test UI rendering, but we can document the expected behavior
    
    let available_for_adding: Vec<String> = available_repos.into_iter()
        .filter(|repo| repo != &modal.repository_name)
        .filter(|repo| !modal.dependencies.iter().any(|d| &d.repository_name == repo))
        .collect();
    
    assert_eq!(available_for_adding.len(), 2);
    assert!(available_for_adding.contains(&"dep-2".to_string()));
    assert!(available_for_adding.contains(&"dep-3".to_string()));
}

#[test]
fn test_dependency_modal_edge_cases() {
    let mut modal = DependencyModal::default();
    
    // Test with very long repository names
    let long_name = "a".repeat(1000);
    modal.add_form.selected_repository = long_name.clone();
    modal.add_dependency();
    
    assert_eq!(modal.dependencies.len(), 1);
    assert_eq!(modal.dependencies[0].repository_name, long_name);
    
    // Test with special characters
    modal.add_form.selected_repository = "repo-with-special-chars!@#$%^&*()".to_string();
    modal.add_form.target_ref = "feature/special-branch!@#".to_string();
    modal.add_dependency();
    
    assert_eq!(modal.dependencies.len(), 2);
    
    // Test with unicode characters
    modal.add_form.selected_repository = "repo-with-unicode-🚀-chars".to_string();
    modal.add_dependency();
    
    assert_eq!(modal.dependencies.len(), 3);
}

#[test]
fn test_dependency_modal_state_consistency() {
    let mut modal = DependencyModal::default();
    
    // Test that state remains consistent through various operations
    modal.show_for_repository("test-repo".to_string(), vec![]);
    assert!(modal.visible);
    assert_eq!(modal.repository_name, "test-repo");
    
    // Add dependency
    modal.add_form.selected_repository = "dep-1".to_string();
    modal.add_dependency();
    assert_eq!(modal.dependencies.len(), 1);
    
    // Hide and show again
    modal.hide();
    assert!(!modal.visible);
    
    modal.show_for_repository("new-repo".to_string(), vec![]);
    assert!(modal.visible);
    assert_eq!(modal.repository_name, "new-repo");
    assert_eq!(modal.dependencies.len(), 0); // Should be reset
}

// Integration tests that would require running the actual UI
// These are documented here but can't be easily tested in unit tests

/*
#[test]
fn test_dependency_modal_ui_rendering() {
    // Test that the modal renders correctly
    // - Window title includes repository name
    // - Dependencies table shows correct columns
    // - Add form has proper validation
    // - Buttons are enabled/disabled correctly
    // - Theme colors are applied properly
}

#[test]
fn test_dependency_modal_keyboard_shortcuts() {
    // Test keyboard interactions
    // - Escape key closes modal
    // - Enter key in form adds dependency
    // - Tab navigation works correctly
}

#[test]
fn test_dependency_modal_async_operations() {
    // Test async save operation
    // - Shows spinner while saving
    // - Handles success correctly
    // - Handles errors correctly
    // - Updates UI state properly
}
*/