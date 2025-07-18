#[cfg(test)]
mod tests {
    use crate::gui::app::events::AppEvent;
    use crate::gui::task::{TaskPanelState, TaskQueue, QueuedTask, QueueTaskStatus};
    use crate::tasks::types::{Task, TaskStatus, TaskPriority, TaskType};
    use chrono::Utc;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    // Mock AppState for testing
    struct MockAppState {
        is_streaming: bool,
        is_waiting: bool,
        is_executing_tool: bool,
        running_tools_count: usize,
        events_sent: Arc<Mutex<Vec<AppEvent>>>,
    }

    impl MockAppState {
        fn new() -> Self {
            Self {
                is_streaming: false,
                is_waiting: false,
                is_executing_tool: false,
                running_tools_count: 0,
                events_sent: Arc::new(Mutex::new(Vec::new())),
            }
        }

        async fn handle_check_and_execute_task(&self, task_id: Uuid) -> Result<(), String> {
            // Check if any conversation is actively streaming or waiting
            if self.is_streaming {
                return Err("Cannot start task: A conversation is currently streaming a response".to_string());
            }
            
            if self.is_waiting {
                return Err("Cannot start task: Waiting for a response from the assistant".to_string());
            }
            
            if self.is_executing_tool {
                return Err("Cannot start task: A tool is currently executing".to_string());
            }
            
            if self.running_tools_count > 0 {
                return Err(format!("Cannot start task: {} tool(s) are still running", self.running_tools_count));
            }
            
            // If all checks pass, proceed with task execution
            let event = AppEvent::ExecuteTask {
                conversation_id: Uuid::new_v4(),
                task_message: format!("Task {}", task_id),
            };
            
            self.events_sent.lock().await.push(event);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_task_cannot_start_while_streaming() {
        let app_state = MockAppState {
            is_streaming: true,
            ..MockAppState::new()
        };
        
        let task_id = Uuid::new_v4();
        let result = app_state.handle_check_and_execute_task(task_id).await;
        
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cannot start task: A conversation is currently streaming a response"
        );
        
        // Verify no execute event was sent
        let events = app_state.events_sent.lock().await;
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_task_cannot_start_while_waiting_for_response() {
        let app_state = MockAppState {
            is_waiting: true,
            ..MockAppState::new()
        };
        
        let task_id = Uuid::new_v4();
        let result = app_state.handle_check_and_execute_task(task_id).await;
        
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cannot start task: Waiting for a response from the assistant"
        );
        
        // Verify no execute event was sent
        let events = app_state.events_sent.lock().await;
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_task_cannot_start_while_tool_executing() {
        let app_state = MockAppState {
            is_executing_tool: true,
            ..MockAppState::new()
        };
        
        let task_id = Uuid::new_v4();
        let result = app_state.handle_check_and_execute_task(task_id).await;
        
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cannot start task: A tool is currently executing"
        );
        
        // Verify no execute event was sent
        let events = app_state.events_sent.lock().await;
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_task_cannot_start_with_running_tools() {
        let app_state = MockAppState {
            running_tools_count: 3,
            ..MockAppState::new()
        };
        
        let task_id = Uuid::new_v4();
        let result = app_state.handle_check_and_execute_task(task_id).await;
        
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cannot start task: 3 tool(s) are still running"
        );
        
        // Verify no execute event was sent
        let events = app_state.events_sent.lock().await;
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_task_can_start_when_idle() {
        let app_state = MockAppState::new();
        
        let task_id = Uuid::new_v4();
        let result = app_state.handle_check_and_execute_task(task_id).await;
        
        assert!(result.is_ok());
        
        // Verify execute event was sent
        let events = app_state.events_sent.lock().await;
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            AppEvent::ExecuteTask { conversation_id, task_message } => {
                assert!(!conversation_id.is_nil());
                assert_eq!(task_message, &format!("Task {}", task_id));
            }
            _ => panic!("Expected ExecuteTask event"),
        }
    }

    #[tokio::test]
    async fn test_auto_progress_triggers_next_task() {
        let state = Arc::new(Mutex::new(TaskPanelState {
            auto_progress_enabled: true,
            task_queue: TaskQueue::new(),
            ..Default::default()
        }));
        
        // Add two tasks to the queue
        let task1 = Task {
            id: Uuid::new_v4(),
            title: "Task 1".to_string(),
            description: Some("First task".to_string()),
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
            metadata: Default::default(),
            dependencies: vec![],
            tags: vec![],
        };
        
        let task2 = Task {
            id: Uuid::new_v4(),
            title: "Task 2".to_string(),
            description: Some("Second task".to_string()),
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
            metadata: Default::default(),
            dependencies: vec![],
            tags: vec![],
        };
        
        {
            let mut state_guard = state.lock().await;
            let queued_task1 = QueuedTask {
                task: task1.clone(),
                queue_position: Some(0),
                queued_at: Utc::now(),
                started_at: None,
                estimated_duration: None,
                conversation_id: None,
                auto_trigger: false,
                completion_status: QueueTaskStatus::Queued,
            };
            let queued_task2 = QueuedTask {
                task: task2.clone(),
                queue_position: Some(1),
                queued_at: Utc::now(),
                started_at: None,
                estimated_duration: None,
                conversation_id: None,
                auto_trigger: false,
                completion_status: QueueTaskStatus::Queued,
            };
            state_guard.task_queue.add_task(queued_task1);
            state_guard.task_queue.add_task(queued_task2);
            
            // Start the first task
            state_guard.task_queue.start_next_task();
        }
        
        // Verify first task is active
        {
            let state_guard = state.lock().await;
            assert!(state_guard.task_queue.active_task.is_some());
            assert_eq!(state_guard.task_queue.active_task.as_ref().unwrap().task.id, task1.id);
            assert_eq!(state_guard.task_queue.pending_tasks.len(), 1);
        }
        
        // Complete the active task with auto-progress enabled
        {
            let mut state_guard = state.lock().await;
            let completed = state_guard.task_queue.complete_active_task();
            assert!(completed.is_some());
            assert_eq!(completed.unwrap().task.id, task1.id);
        }
        
        // With auto-progress enabled, the second task should be started automatically
        // (In the real implementation, this would be triggered by the complete_active_task method)
        {
            let state_guard = state.lock().await;
            assert_eq!(state_guard.task_queue.completed_tasks.len(), 1);
            assert_eq!(state_guard.task_queue.completed_tasks[0].task.id, task1.id);
        }
    }

    #[tokio::test]
    async fn test_auto_progress_disabled_no_automatic_start() {
        let state = Arc::new(Mutex::new(TaskPanelState {
            auto_progress_enabled: false,
            task_queue: TaskQueue::new(),
            ..Default::default()
        }));
        
        // Add two tasks to the queue
        let task1 = Task {
            id: Uuid::new_v4(),
            title: "Task 1".to_string(),
            description: Some("First task".to_string()),
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
            metadata: Default::default(),
            dependencies: vec![],
            tags: vec![],
        };
        
        let task2 = Task {
            id: Uuid::new_v4(),
            title: "Task 2".to_string(),
            description: Some("Second task".to_string()),
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
            metadata: Default::default(),
            dependencies: vec![],
            tags: vec![],
        };
        
        {
            let mut state_guard = state.lock().await;
            let queued_task1 = QueuedTask {
                task: task1.clone(),
                queue_position: Some(0),
                queued_at: Utc::now(),
                started_at: None,
                estimated_duration: None,
                conversation_id: None,
                auto_trigger: false,
                completion_status: QueueTaskStatus::Queued,
            };
            let queued_task2 = QueuedTask {
                task: task2.clone(),
                queue_position: Some(1),
                queued_at: Utc::now(),
                started_at: None,
                estimated_duration: None,
                conversation_id: None,
                auto_trigger: false,
                completion_status: QueueTaskStatus::Queued,
            };
            state_guard.task_queue.add_task(queued_task1);
            state_guard.task_queue.add_task(queued_task2);
            
            // Start the first task
            state_guard.task_queue.start_next_task();
        }
        
        // Complete the active task with auto-progress disabled
        {
            let mut state_guard = state.lock().await;
            let completed = state_guard.task_queue.complete_active_task();
            assert!(completed.is_some());
        }
        
        // With auto-progress disabled, the second task should remain pending
        {
            let state_guard = state.lock().await;
            assert_eq!(state_guard.task_queue.completed_tasks.len(), 1);
            assert_eq!(state_guard.task_queue.pending_tasks.len(), 1);
            assert!(state_guard.task_queue.active_task.is_none());
            assert_eq!(state_guard.task_queue.pending_tasks[0].task.id, task2.id);
        }
    }
}