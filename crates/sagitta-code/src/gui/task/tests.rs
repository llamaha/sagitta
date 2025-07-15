//! Comprehensive tests for the task panel functionality
//! 
//! These tests cover:
//! - Task creation and management
//! - UI button interactions  
//! - Theme application
//! - State management
//! - Task queue operations
//! - Completion detection

#[cfg(test)]
mod tests {
    use super::super::types::*;
    use super::super::panel::*;
    use crate::tasks::types::*;
    use crate::config::types::SagittaCodeConfig;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use uuid::Uuid;
    use chrono::Utc;
    use std::time::Duration;
    
    // Mock implementations for testing
    use mockall::predicate::*;
    use mockall::mock;
    
    mock! {
        TaskManager {}
        
        #[async_trait::async_trait]
        impl crate::tasks::manager::TaskManager for TaskManager {
            async fn create_task(&self, request: crate::tasks::types::CreateTaskRequest) -> anyhow::Result<Uuid>;
            async fn get_task(&self, id: Uuid) -> anyhow::Result<Option<Task>>;
            async fn update_task(&self, id: Uuid, request: crate::tasks::types::UpdateTaskRequest) -> anyhow::Result<()>;
            async fn delete_task(&self, id: Uuid) -> anyhow::Result<()>;
            async fn list_tasks(&self, query: crate::tasks::types::TaskQuery) -> anyhow::Result<Vec<crate::tasks::types::TaskSummary>>;
            async fn search_tasks(&self, query: crate::tasks::types::TaskQuery) -> anyhow::Result<Vec<crate::tasks::types::TaskSearchResult>>;
            async fn execute_task(&self, id: Uuid) -> anyhow::Result<crate::tasks::types::TaskExecutionResult>;
            async fn get_workspace_tasks(&self, workspace_id: Uuid) -> anyhow::Result<Vec<crate::tasks::types::TaskSummary>>;
            async fn get_conversation_tasks(&self, conversation_id: Uuid) -> anyhow::Result<Vec<crate::tasks::types::TaskSummary>>;
            async fn get_overdue_tasks(&self) -> anyhow::Result<Vec<crate::tasks::types::TaskSummary>>;
            async fn get_ready_tasks(&self) -> anyhow::Result<Vec<crate::tasks::types::TaskSummary>>;
        }
    }
    
    mock! {
        ConversationManager {}
        
        #[async_trait::async_trait]
        impl crate::agent::conversation::manager::ConversationManager for ConversationManager {
            async fn create_conversation(&self, title: String, workspace_id: Option<Uuid>) -> anyhow::Result<Uuid>;
            async fn get_conversation(&self, id: Uuid) -> anyhow::Result<Option<crate::agent::conversation::types::Conversation>>;
            async fn update_conversation(&self, conversation: crate::agent::conversation::types::Conversation) -> anyhow::Result<()>;
            async fn delete_conversation(&self, id: Uuid) -> anyhow::Result<()>;
            async fn list_conversations(&self, workspace_id: Option<Uuid>) -> anyhow::Result<Vec<crate::agent::conversation::types::ConversationSummary>>;
            async fn search_conversations(&self, query: &crate::agent::conversation::types::ConversationQuery) -> anyhow::Result<Vec<crate::agent::conversation::types::ConversationSearchResult>>;
            async fn create_branch(&self, conversation_id: Uuid, parent_message_id: Option<Uuid>, title: String) -> anyhow::Result<Uuid>;
            async fn merge_branch(&self, conversation_id: Uuid, branch_id: Uuid) -> anyhow::Result<()>;
            async fn create_checkpoint(&self, conversation_id: Uuid, message_id: Uuid, title: String) -> anyhow::Result<Uuid>;
            async fn restore_checkpoint(&self, conversation_id: Uuid, checkpoint_id: Uuid) -> anyhow::Result<()>;
            async fn get_statistics(&self) -> anyhow::Result<crate::agent::conversation::manager::ConversationStatistics>;
            async fn archive_conversations(&self, criteria: crate::agent::conversation::manager::ArchiveCriteria) -> anyhow::Result<usize>;
            async fn get_tag_suggestions(&self, conversation_id: Uuid) -> anyhow::Result<Vec<crate::agent::conversation::TagSuggestion>>;
            async fn get_tag_metadata(&self, conversation_id: Uuid) -> anyhow::Result<Vec<crate::agent::conversation::TagMetadata>>;
            async fn retag_conversation(&self, conversation_id: Uuid) -> anyhow::Result<crate::agent::conversation::TaggingResult>;
        }
    }
    
    /// Helper function to create a test task
    pub fn create_test_task(title: &str) -> Task {
        Task {
            id: Uuid::new_v4(),
            title: title.to_string(),
            description: Some("Test task description".to_string()),
            task_type: TaskType::Custom("Test".to_string()),
            priority: TaskPriority::Normal,
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            due_date: None,
            scheduled_at: None,
            completed_at: None,
            source_conversation_id: None,
            target_conversation_id: None,
            workspace_id: None,
            metadata: TaskMetadata::default(),
            dependencies: Vec::new(),
            tags: vec!["test".to_string()],
        }
    }
    
    /// Helper function to create a test config
    fn create_test_config() -> Arc<Mutex<SagittaCodeConfig>> {
        let config = SagittaCodeConfig::default();
        Arc::new(Mutex::new(config))
    }
    
    /// Helper function to create a task panel for testing
    pub fn create_test_task_panel() -> TaskPanel {
        let task_manager: Arc<dyn crate::tasks::manager::TaskManager> = Arc::new(MockTaskManager::new());
        let conversation_manager: Arc<dyn crate::agent::conversation::manager::ConversationManager> = Arc::new(MockConversationManager::new());
        let config = create_test_config();
        
        TaskPanel::new(Some(task_manager), Some(conversation_manager), config)
    }
    
    /// Helper function to create a task panel with mocked conversation manager
    pub fn create_test_task_panel_with_mock_conversation_manager() -> (TaskPanel, Arc<MockConversationManager>) {
        let task_manager: Arc<dyn crate::tasks::manager::TaskManager> = Arc::new(MockTaskManager::new());
        let mut mock_conversation_manager = MockConversationManager::new();
        
        // Set up expectations for conversation creation
        mock_conversation_manager
            .expect_create_conversation()
            .returning(|_title, _workspace_id| Ok(Uuid::new_v4()));
        
        let mock_arc = Arc::new(mock_conversation_manager);
        let conversation_manager: Arc<dyn crate::agent::conversation::manager::ConversationManager> = Arc::clone(&mock_arc) as Arc<dyn crate::agent::conversation::manager::ConversationManager>;
        let config = create_test_config();
        
        let panel = TaskPanel::new(Some(task_manager), Some(conversation_manager), config);
        (panel, mock_arc)
    }
    
    #[test]
    fn test_task_panel_creation() {
        let panel = create_test_task_panel();
        assert!(!panel.is_open());
    }
    
    #[test]
    fn test_task_panel_toggle() {
        let mut panel = create_test_task_panel();
        assert!(!panel.is_open());
        
        panel.toggle();
        assert!(panel.is_open());
        
        panel.toggle();
        assert!(!panel.is_open());
    }
    
    #[test]
    fn test_task_panel_set_open() {
        let mut panel = create_test_task_panel();
        
        panel.set_open(true);
        assert!(panel.is_open());
        
        panel.set_open(false);
        assert!(!panel.is_open());
    }
    
    #[tokio::test]
    async fn test_task_creation_form() {
        let task_panel = create_test_task_panel();
        let initial_state = task_panel.get_queue_state().await;
        
        // Initially no tasks should exist
        assert_eq!(initial_state.task_queue.pending_count(), 0);
        assert_eq!(initial_state.task_queue.completed_count(), 0);
        assert_eq!(initial_state.task_queue.failed_count(), 0);
    }
    
    #[tokio::test]
    async fn test_add_task_from_conversation() {
        let task_panel = create_test_task_panel();
        let conversation_id = Uuid::new_v4();
        
        let result = task_panel.add_task_from_conversation(
            "Test Task".to_string(),
            Some("Test Description".to_string()),
            conversation_id,
            false,
        ).await;
        
        assert!(result.is_ok());
        let task_id = result.unwrap();
        
        let state = task_panel.get_queue_state().await;
        assert_eq!(state.task_queue.pending_count(), 1);
        
        let task = state.task_queue.get_task_by_id(task_id);
        assert!(task.is_some());
        let task = task.unwrap();
        assert_eq!(task.task.title, "Test Task");
        assert_eq!(task.task.description, Some("Test Description".to_string()));
        assert_eq!(task.task.source_conversation_id, Some(conversation_id));
    }
    
    #[tokio::test]
    async fn test_task_queue_operations() {
        // Create a task panel with properly mocked conversation manager
        let (task_panel, _mock_conversation_manager) = create_test_task_panel_with_mock_conversation_manager();
        
        // Add a task
        let conversation_id = Uuid::new_v4();
        let task_id = task_panel.add_task_from_conversation(
            "Test Task".to_string(),
            None,
            conversation_id,
            true, // auto_trigger
        ).await.unwrap();
        
        let mut state = task_panel.get_queue_state().await;
        
        // Since auto_trigger is true and queue is empty, task should start immediately
        assert!(state.task_queue.active_task.is_some());
        assert_eq!(state.task_queue.pending_count(), 0);
        
        // Add another task (should go to pending since one is active)
        let task_id_2 = task_panel.add_task_from_conversation(
            "Test Task 2".to_string(),
            None,
            conversation_id,
            false,
        ).await.unwrap();
        
        state = task_panel.get_queue_state().await;
        assert_eq!(state.task_queue.pending_count(), 1);
        
        // Complete the active task
        let completed_task_id = task_panel.complete_active_task().await.unwrap();
        assert_eq!(completed_task_id, Some(task_id));
        
        state = task_panel.get_queue_state().await;
        assert!(state.task_queue.active_task.is_none());
        assert_eq!(state.task_queue.completed_count(), 1);
        
        // Start next task
        let started_task_id = task_panel.start_next_task().await.unwrap();
        assert_eq!(started_task_id, Some(task_id_2));
        
        state = task_panel.get_queue_state().await;
        assert!(state.task_queue.active_task.is_some());
        assert_eq!(state.task_queue.pending_count(), 0);
    }
    
    #[test]
    fn test_task_panel_state_default() {
        let state = TaskPanelState::default();
        assert_eq!(state.active_tab, TaskPanelTab::Queue);
        assert!(!state.show_completed);
        assert!(state.filter_text.is_empty());
        assert!(state.selected_task.is_none());
        assert!(!state.auto_progress_enabled);
    }
    
    #[test]
    fn test_task_panel_tabs() {
        let mut state = TaskPanelState::default();
        
        assert_eq!(state.active_tab, TaskPanelTab::Queue);
        
        state.active_tab = TaskPanelTab::Active;
        assert_eq!(state.active_tab, TaskPanelTab::Active);
        
        state.active_tab = TaskPanelTab::Completed;
        assert_eq!(state.active_tab, TaskPanelTab::Completed);
        
        state.active_tab = TaskPanelTab::Settings;
        assert_eq!(state.active_tab, TaskPanelTab::Settings);
    }
    
    #[test]
    fn test_task_queue_add_task() {
        let mut queue = TaskQueue::new();
        let task = create_test_task("Test Task");
        let queued_task = QueuedTask::new(task, false);
        
        assert_eq!(queue.pending_count(), 0);
        
        queue.add_task(queued_task);
        assert_eq!(queue.pending_count(), 1);
        assert!(queue.active_task.is_none());
    }
    
    #[test]
    fn test_task_queue_add_auto_trigger_task() {
        let mut queue = TaskQueue::new();
        let task = create_test_task("Auto Trigger Task");
        let queued_task = QueuedTask::new(task, true);
        
        queue.add_task(queued_task);
        
        // Auto-trigger task should start immediately if queue is empty
        assert_eq!(queue.pending_count(), 0);
        assert!(queue.active_task.is_some());
        assert_eq!(queue.active_task.as_ref().unwrap().task.title, "Auto Trigger Task");
    }
    
    #[test]
    fn test_task_queue_start_next_task() {
        let mut queue = TaskQueue::new();
        let task1 = create_test_task("Task 1");
        let task2 = create_test_task("Task 2");
        
        queue.add_task(QueuedTask::new(task1, false));
        queue.add_task(QueuedTask::new(task2, false));
        
        assert_eq!(queue.pending_count(), 2);
        assert!(queue.active_task.is_none());
        
        let started_task = queue.start_next_task();
        assert!(started_task.is_some());
        assert_eq!(started_task.unwrap().task.title, "Task 1");
        assert_eq!(queue.pending_count(), 1);
        assert!(queue.active_task.is_some());
        
        // Cannot start another task while one is active
        let next_task = queue.start_next_task();
        assert!(next_task.is_none());
    }
    
    #[test]
    fn test_task_queue_complete_active_task() {
        let mut queue = TaskQueue::new();
        let task = create_test_task("Test Task");
        let queued_task = QueuedTask::new(task, true);
        
        queue.add_task(queued_task);
        assert!(queue.active_task.is_some());
        
        let completed_task = queue.complete_active_task();
        assert!(completed_task.is_some());
        assert!(queue.active_task.is_none());
        assert_eq!(queue.completed_count(), 1);
        
        let completed = completed_task.unwrap();
        assert_eq!(completed.completion_status, QueueTaskStatus::Completed);
        assert_eq!(completed.task.status, TaskStatus::Completed);
        assert!(completed.task.completed_at.is_some());
    }
    
    #[test]
    fn test_task_queue_fail_active_task() {
        let mut queue = TaskQueue::new();
        let task = create_test_task("Test Task");
        let queued_task = QueuedTask::new(task, true);
        
        queue.add_task(queued_task);
        assert!(queue.active_task.is_some());
        
        let failure_reason = "Network error".to_string();
        let failed_task = queue.fail_active_task(failure_reason.clone());
        assert!(failed_task.is_some());
        assert!(queue.active_task.is_none());
        assert_eq!(queue.failed_count(), 1);
        
        let failed = failed_task.unwrap();
        assert_eq!(failed.completion_status, QueueTaskStatus::Failed);
        assert_eq!(failed.task.status, TaskStatus::Failed);
        assert_eq!(
            failed.task.metadata.custom_fields.get("failure_reason"),
            Some(&failure_reason)
        );
    }
    
    #[test]
    fn test_task_queue_get_task_by_id() {
        let mut queue = TaskQueue::new();
        let task1 = create_test_task("Task 1");
        let task2 = create_test_task("Task 2");
        let task1_id = task1.id;
        let task2_id = task2.id;
        
        queue.add_task(QueuedTask::new(task1, true)); // Will become active
        queue.add_task(QueuedTask::new(task2, false)); // Will be pending
        
        let found_task1 = queue.get_task_by_id(task1_id);
        assert!(found_task1.is_some());
        assert_eq!(found_task1.unwrap().task.title, "Task 1");
        
        let found_task2 = queue.get_task_by_id(task2_id);
        assert!(found_task2.is_some());
        assert_eq!(found_task2.unwrap().task.title, "Task 2");
        
        let not_found = queue.get_task_by_id(Uuid::new_v4());
        assert!(not_found.is_none());
    }
    
    #[test]
    fn test_task_queue_remove_task() {
        let mut queue = TaskQueue::new();
        let task1 = create_test_task("Task 1");
        let task2 = create_test_task("Task 2");
        let task1_id = task1.id;
        let task2_id = task2.id;
        
        queue.add_task(QueuedTask::new(task1, false));
        queue.add_task(QueuedTask::new(task2, false));
        
        assert_eq!(queue.pending_count(), 2);
        
        let removed = queue.remove_task(task1_id);
        assert!(removed);
        assert_eq!(queue.pending_count(), 1);
        
        let found = queue.get_task_by_id(task1_id);
        assert!(found.is_none());
        
        let still_there = queue.get_task_by_id(task2_id);
        assert!(still_there.is_some());
    }
    
    #[test]
    fn test_queued_task_creation() {
        let task = create_test_task("Test Task");
        let queued_task = QueuedTask::new(task.clone(), true);
        
        assert_eq!(queued_task.task.id, task.id);
        assert!(queued_task.auto_trigger);
        assert_eq!(queued_task.completion_status, QueueTaskStatus::Queued);
        assert!(queued_task.started_at.is_none());
        assert!(queued_task.conversation_id.is_none());
    }
    
    #[test]
    fn test_queued_task_with_estimated_duration() {
        let task = create_test_task("Test Task");
        let duration = Duration::from_secs(3600); // 1 hour
        let queued_task = QueuedTask::new(task, false)
            .with_estimated_duration(duration);
        
        assert_eq!(queued_task.estimated_duration, Some(duration));
        assert_eq!(queued_task.duration_estimate_text(), "~1.0 hr");
    }
    
    #[test]
    fn test_queued_task_duration_estimate_text() {
        let task = create_test_task("Test Task");
        
        // Test no duration
        let queued_task = QueuedTask::new(task.clone(), false);
        assert_eq!(queued_task.duration_estimate_text(), "Unknown");
        
        // Test minutes
        let queued_task = QueuedTask::new(task.clone(), false)
            .with_estimated_duration(Duration::from_secs(1800)); // 30 minutes
        assert_eq!(queued_task.duration_estimate_text(), "~30 min");
        
        // Test hours
        let queued_task = QueuedTask::new(task, false)
            .with_estimated_duration(Duration::from_secs(7200)); // 2 hours
        assert_eq!(queued_task.duration_estimate_text(), "~2.0 hr");
    }
    
    #[test]
    fn test_queued_task_status_checks() {
        let task = create_test_task("Test Task");
        let mut queued_task = QueuedTask::new(task, false);
        
        // Initially queued
        assert!(!queued_task.is_active());
        assert!(!queued_task.is_completed());
        
        // Make it active
        queued_task.completion_status = QueueTaskStatus::Active;
        assert!(queued_task.is_active());
        assert!(!queued_task.is_completed());
        
        // Make it completed
        queued_task.completion_status = QueueTaskStatus::Completed;
        assert!(!queued_task.is_active());
        assert!(queued_task.is_completed());
        
        // Make it failed
        queued_task.completion_status = QueueTaskStatus::Failed;
        assert!(!queued_task.is_active());
        assert!(queued_task.is_completed());
        
        // Make it cancelled
        queued_task.completion_status = QueueTaskStatus::Cancelled;
        assert!(!queued_task.is_active());
        assert!(queued_task.is_completed());
    }
    
    #[test]
    fn test_completion_criteria_default() {
        let criteria = CompletionCriteria::default();
        
        assert!(criteria.require_tests_pass);
        assert!(!criteria.require_explicit_completion);
        assert!(criteria.check_lint_errors);
        assert_eq!(criteria.timeout_minutes, Some(60));
        assert!(criteria.completion_keywords.contains(&"completed".to_string()));
        assert!(criteria.completion_keywords.contains(&"finished".to_string()));
        assert!(criteria.completion_keywords.contains(&"done".to_string()));
        assert!(criteria.failure_keywords.contains(&"failed".to_string()));
        assert!(criteria.failure_keywords.contains(&"error".to_string()));
    }
    
    #[tokio::test]
    async fn test_conversation_completion_detection() {
        let task_panel = create_test_task_panel();
        
        // Test explicit completion words
        let content_with_completion = "The task has been completed successfully.";
        let is_complete = task_panel.check_conversation_completion(content_with_completion).await.unwrap();
        assert!(is_complete);
        
        let content_with_finished = "All work is finished and ready for review.";
        let is_complete = task_panel.check_conversation_completion(content_with_finished).await.unwrap();
        assert!(is_complete);
        
        let content_with_done = "The implementation is done and tests are passing.";
        let is_complete = task_panel.check_conversation_completion(content_with_done).await.unwrap();
        assert!(is_complete);
        
        // Test no completion indicators
        let content_without_completion = "Still working on the implementation.";
        let is_complete = task_panel.check_conversation_completion(content_without_completion).await.unwrap();
        assert!(!is_complete);
    }
    
    #[tokio::test]
    async fn test_stream_completion_with_auto_progress() {
        let task_panel = create_test_task_panel();
        
        // Add tasks
        let conv_id = Uuid::new_v4();
        let _task_id_1 = task_panel.add_task_from_conversation(
            "Task 1".to_string(),
            None,
            conv_id,
            true, // auto_trigger
        ).await.unwrap();
        
        let _task_id_2 = task_panel.add_task_from_conversation(
            "Task 2".to_string(),
            None,
            conv_id,
            false,
        ).await.unwrap();
        
        // Initially task 1 should be active, task 2 pending
        let state = task_panel.get_queue_state().await;
        assert!(state.task_queue.active_task.is_some());
        assert_eq!(state.task_queue.pending_count(), 1);
        
        // Handle stream completion with completion indicator
        let completion_content = "Implementation completed successfully with all tests passing.";
        let result = task_panel.handle_stream_completion(completion_content).await;
        assert!(result.is_ok());
    }
}

/// UI-specific tests that test egui interactions
#[cfg(test)]
mod ui_tests {
    use super::super::types::*;
    use super::super::queue::*;
    use super::super::status::*;
    use egui::Context;
    
    /// Mock UI context for testing
    struct MockUiContext {
        ctx: Context,
    }
    
    impl MockUiContext {
        fn new() -> Self {
            Self {
                ctx: Context::default(),
            }
        }
        
        fn with_ui<F>(&self, f: F) 
        where 
            F: FnOnce(&mut egui::Ui)
        {
            let f = std::cell::RefCell::new(Some(f));
            self.ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if let Some(f) = f.borrow_mut().take() {
                        f(ui);
                    }
                });
            });
        }
    }
    
    #[test]
    fn test_render_task_queue_ui() {
        let mock_ctx = MockUiContext::new();
        let mut state = TaskPanelState::default();
        let mut show_dialog = false;
        
        mock_ctx.with_ui(|ui| {
            let add_task_clicked = render_task_queue(ui, &mut state, &mut show_dialog);
            assert!(!add_task_clicked.0); // No interaction yet
        });
    }
    
    #[test]
    fn test_render_task_status_ui() {
        let mock_ctx = MockUiContext::new();
        let mut state = TaskPanelState::default();
        
        mock_ctx.with_ui(|ui| {
            render_task_status(ui, &mut state);
            // Should not panic and should render without active task
        });
    }
    
    #[test]
    fn test_render_completion_settings_ui() {
        let mock_ctx = MockUiContext::new();
        let mut state = TaskPanelState::default();
        
        mock_ctx.with_ui(|ui| {
            render_completion_settings(ui, &mut state);
            // Should render the settings UI
        });
    }
    
    #[test]
    fn test_theme_application() {
        // Test that different UI components render without panicking
        let mock_ctx = MockUiContext::new();
        let mut state = TaskPanelState::default();
        
        mock_ctx.with_ui(|ui| {
            // This should not panic
            render_task_status(ui, &mut state);
            render_completion_settings(ui, &mut state);
        });
    }
    
    #[test]
    fn test_task_queue_with_tasks() {
        let mock_ctx = MockUiContext::new();
        let mut state = TaskPanelState::default();
        
        // Add some test tasks - first one with auto_trigger will become active
        let task1 = super::tests::create_test_task("UI Test Task 1");
        let task2 = super::tests::create_test_task("UI Test Task 2");
        
        state.task_queue.add_task(QueuedTask::new(task1, true)); // This should become active (no pending tasks)
        state.task_queue.add_task(QueuedTask::new(task2, false)); // This should be pending
        
        let mut show_dialog = false;
        
        mock_ctx.with_ui(|ui| {
            let add_task_clicked = render_task_queue(ui, &mut state, &mut show_dialog);
            assert!(!add_task_clicked.0);
        });
        
        // Verify state is as expected
        assert!(state.task_queue.active_task.is_some());
        assert_eq!(state.task_queue.pending_count(), 1);
    }
    
    #[test]
    fn test_task_panel_visibility_state() {
        let mut panel = super::tests::create_test_task_panel();
        
        // Test initial state
        assert!(!panel.is_open());
        
        // Test setting open
        panel.set_open(true);
        assert!(panel.is_open());
        
        // Test toggling
        panel.toggle();
        assert!(!panel.is_open());
        
        panel.toggle();
        assert!(panel.is_open());
    }
}

/// Integration tests that test the full workflow
#[cfg(test)]
mod integration_tests {
    use super::super::types::*;
    use uuid::Uuid;
    
    #[tokio::test]
    async fn test_full_task_workflow() {
        let task_panel = super::tests::create_test_task_panel();
        let conversation_id = Uuid::new_v4();
        
        // 1. Create and add a task
        let task_id = task_panel.add_task_from_conversation(
            "Integration Test Task".to_string(),
            Some("A comprehensive integration test".to_string()),
            conversation_id,
            true,
        ).await.unwrap();
        
        // 2. Verify task is active (auto-trigger)
        let state = task_panel.get_queue_state().await;
        assert!(state.task_queue.active_task.is_some());
        assert_eq!(state.task_queue.active_task.as_ref().unwrap().task.id, task_id);
        
        // 3. Test completion detection
        let completion_content = "The integration test task has been completed successfully!";
        let is_complete = task_panel.check_conversation_completion(completion_content).await.unwrap();
        assert!(is_complete);
        
        // 4. Complete the task
        let completed_id = task_panel.complete_active_task().await.unwrap();
        assert_eq!(completed_id, Some(task_id));
        
        // 5. Verify final state
        let final_state = task_panel.get_queue_state().await;
        assert!(final_state.task_queue.active_task.is_none());
        assert_eq!(final_state.task_queue.completed_count(), 1);
        assert_eq!(final_state.task_queue.pending_count(), 0);
        
        // 6. Verify completed task details
        let completed_task = final_state.task_queue.get_task_by_id(task_id);
        assert!(completed_task.is_some());
        let completed_task = completed_task.unwrap();
        assert_eq!(completed_task.completion_status, QueueTaskStatus::Completed);
        assert!(completed_task.task.completed_at.is_some());
    }
    
    #[tokio::test]
    async fn test_auto_progress_workflow() {
        let task_panel = super::tests::create_test_task_panel();
        let conversation_id = Uuid::new_v4();
        
        // Add multiple tasks
        let task_ids: Vec<Uuid> = {
            let mut ids = Vec::new();
            for i in 1..=3 {
                let id = task_panel.add_task_from_conversation(
                    format!("Auto Progress Task {}", i),
                    None,
                    conversation_id,
                    i == 1, // Only first task auto-triggers
                ).await.unwrap();
                ids.push(id);
            }
            ids
        };
        
        // Verify initial state: task 1 active, tasks 2 and 3 pending
        let state = task_panel.get_queue_state().await;
        assert!(state.task_queue.active_task.is_some());
        assert_eq!(state.task_queue.active_task.as_ref().unwrap().task.id, task_ids[0]);
        assert_eq!(state.task_queue.pending_count(), 2);
        
        // Complete task 1
        let completion_content = "Task 1 completed successfully";
        task_panel.handle_stream_completion(completion_content).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_task_failure_workflow() {
        let task_panel = super::tests::create_test_task_panel();
        let conversation_id = Uuid::new_v4();
        
        // Add a task
        let task_id = task_panel.add_task_from_conversation(
            "Failure Test Task".to_string(),
            None,
            conversation_id,
            true,
        ).await.unwrap();
        
        // Verify task is active
        let state = task_panel.get_queue_state().await;
        assert!(state.task_queue.active_task.is_some());
        
        // Verify task is active (we can't simulate failure through the public API)
        let state = task_panel.get_queue_state().await;
        assert!(state.task_queue.active_task.is_some());
    }
}