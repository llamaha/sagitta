use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

use crate::tasks::types::{Task, TaskStatus, TaskPriority};

/// Task panel state
#[derive(Debug, Clone, Default)]
pub struct TaskPanelState {
    pub active_tab: TaskPanelTab,
    pub task_queue: TaskQueue,
    pub show_completed: bool,
    pub filter_text: String,
    pub selected_task: Option<Uuid>,
    pub auto_progress_enabled: bool,
    pub completion_criteria: CompletionCriteria,
}

/// Task panel tabs
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TaskPanelTab {
    #[default]
    Queue,
    Active,
    Completed,
    Settings,
}

/// Task queue management
#[derive(Debug, Clone, Default)]
pub struct TaskQueue {
    pub pending_tasks: VecDeque<QueuedTask>,
    pub active_task: Option<QueuedTask>,
    pub completed_tasks: Vec<QueuedTask>,
    pub failed_tasks: Vec<QueuedTask>,
}

/// Task in the queue with additional queue-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTask {
    pub task: Task,
    pub queue_position: Option<usize>,
    pub queued_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub estimated_duration: Option<std::time::Duration>,
    pub conversation_id: Option<Uuid>,
    pub auto_trigger: bool,
    pub completion_status: QueueTaskStatus,
}

/// Status of a task in the queue
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum QueueTaskStatus {
    #[default]
    Queued,
    Active,
    WaitingForInput,
    Completed,
    Failed,
    Cancelled,
    Paused,
}

/// Criteria for determining task/conversation completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionCriteria {
    pub require_tests_pass: bool,
    pub require_explicit_completion: bool,
    pub check_lint_errors: bool,
    pub timeout_minutes: Option<u32>,
    pub completion_keywords: Vec<String>,
    pub failure_keywords: Vec<String>,
}

impl Default for CompletionCriteria {
    fn default() -> Self {
        Self {
            require_tests_pass: true,
            require_explicit_completion: false,
            check_lint_errors: true,
            timeout_minutes: Some(60),
            completion_keywords: vec![
                "completed".to_string(),
                "finished".to_string(),
                "done".to_string(),
                "success".to_string(),
                "implemented".to_string(),
            ],
            failure_keywords: vec![
                "failed".to_string(),
                "error".to_string(),
                "blocked".to_string(),
                "unable to".to_string(),
            ],
        }
    }
}

/// Task creation request from UI
#[derive(Debug, Clone)]
pub struct TaskCreationRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: TaskPriority,
    pub auto_trigger: bool,
    pub estimated_duration: Option<std::time::Duration>,
    pub source_conversation_id: Option<Uuid>,
}

/// Task queue operations
#[derive(Debug, Clone)]
pub enum QueueOperation {
    AddTask(TaskCreationRequest),
    RemoveTask(Uuid),
    MoveTaskUp(Uuid),
    MoveTaskDown(Uuid),
    PauseTask(Uuid),
    ResumeTask(Uuid),
    CancelTask(Uuid),
    RetryTask(Uuid),
    StartNextTask,
    ClearCompleted,
}

/// Task queue events
#[derive(Debug, Clone)]
pub enum TaskQueueEvent {
    TaskAdded(Uuid),
    TaskStarted(Uuid),
    TaskCompleted(Uuid),
    TaskFailed(Uuid, String),
    TaskCancelled(Uuid),
    ConversationCreated(Uuid, Uuid), // task_id, conversation_id
    QueueEmpty,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_task(&mut self, task: QueuedTask) {
        if task.auto_trigger && self.active_task.is_none() && self.pending_tasks.is_empty() {
            self.active_task = Some(task);
        } else {
            self.pending_tasks.push_back(task);
        }
    }

    pub fn start_next_task(&mut self) -> Option<QueuedTask> {
        if self.active_task.is_some() {
            return None;
        }

        if let Some(mut task) = self.pending_tasks.pop_front() {
            task.started_at = Some(Utc::now());
            task.completion_status = QueueTaskStatus::Active;
            self.active_task = Some(task.clone());
            Some(task)
        } else {
            None
        }
    }

    pub fn complete_active_task(&mut self) -> Option<QueuedTask> {
        if let Some(mut task) = self.active_task.take() {
            task.completion_status = QueueTaskStatus::Completed;
            task.task.status = TaskStatus::Completed;
            task.task.completed_at = Some(Utc::now());
            self.completed_tasks.push(task.clone());
            Some(task)
        } else {
            None
        }
    }

    pub fn fail_active_task(&mut self, reason: String) -> Option<QueuedTask> {
        if let Some(mut task) = self.active_task.take() {
            task.completion_status = QueueTaskStatus::Failed;
            task.task.status = TaskStatus::Failed;
            task.task.metadata.custom_fields.insert("failure_reason".to_string(), reason);
            self.failed_tasks.push(task.clone());
            Some(task)
        } else {
            None
        }
    }

    pub fn get_task_by_id(&self, task_id: Uuid) -> Option<&QueuedTask> {
        // Check active task
        if let Some(task) = &self.active_task {
            if task.task.id == task_id {
                return Some(task);
            }
        }

        // Check pending tasks
        for task in &self.pending_tasks {
            if task.task.id == task_id {
                return Some(task);
            }
        }

        // Check completed tasks
        for task in &self.completed_tasks {
            if task.task.id == task_id {
                return Some(task);
            }
        }

        // Check failed tasks
        for task in &self.failed_tasks {
            if task.task.id == task_id {
                return Some(task);
            }
        }

        None
    }

    pub fn remove_task(&mut self, task_id: Uuid) -> bool {
        // Remove from pending tasks
        let original_len = self.pending_tasks.len();
        self.pending_tasks.retain(|task| task.task.id != task_id);
        if self.pending_tasks.len() != original_len {
            return true;
        }

        // Cancel active task if it matches
        if let Some(task) = &self.active_task {
            if task.task.id == task_id {
                self.active_task = None;
                return true;
            }
        }

        false
    }

    pub fn total_tasks(&self) -> usize {
        self.pending_tasks.len()
            + if self.active_task.is_some() { 1 } else { 0 }
            + self.completed_tasks.len()
            + self.failed_tasks.len()
    }

    pub fn pending_count(&self) -> usize {
        self.pending_tasks.len()
    }

    pub fn completed_count(&self) -> usize {
        self.completed_tasks.len()
    }

    pub fn failed_count(&self) -> usize {
        self.failed_tasks.len()
    }
}

impl QueuedTask {
    pub fn new(task: Task, auto_trigger: bool) -> Self {
        Self {
            task,
            queue_position: None,
            queued_at: Utc::now(),
            started_at: None,
            estimated_duration: None,
            conversation_id: None,
            auto_trigger,
            completion_status: QueueTaskStatus::Queued,
        }
    }

    pub fn with_estimated_duration(mut self, duration: std::time::Duration) -> Self {
        self.estimated_duration = Some(duration);
        self
    }

    pub fn is_active(&self) -> bool {
        self.completion_status == QueueTaskStatus::Active
    }

    pub fn is_completed(&self) -> bool {
        matches!(self.completion_status, QueueTaskStatus::Completed | QueueTaskStatus::Failed | QueueTaskStatus::Cancelled)
    }

    pub fn duration_estimate_text(&self) -> String {
        if let Some(duration) = self.estimated_duration {
            let minutes = duration.as_secs() / 60;
            if minutes < 60 {
                format!("~{} min", minutes)
            } else {
                format!("~{:.1} hr", minutes as f32 / 60.0)
            }
        } else {
            "Unknown".to_string()
        }
    }
}