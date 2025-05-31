use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TaskPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
    Scheduled,
}

/// Task type for categorization
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    ConversationFollowUp,
    CodeAnalysis,
    Documentation,
    Testing,
    Refactoring,
    Research,
    Custom(String),
}

/// A task that can be triggered by conversations or scheduled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub task_type: TaskType,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub due_date: Option<DateTime<Utc>>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    
    /// Associated conversation that triggered this task
    pub source_conversation_id: Option<Uuid>,
    
    /// Conversation to create when this task is executed
    pub target_conversation_id: Option<Uuid>,
    
    /// Project workspace context
    pub workspace_id: Option<Uuid>,
    
    /// Task metadata and context
    pub metadata: TaskMetadata,
    
    /// Dependencies on other tasks
    pub dependencies: Vec<Uuid>,
    
    /// Tags for organization
    pub tags: Vec<String>,
}

/// Task metadata for additional context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetadata {
    /// Estimated effort in hours
    pub estimated_hours: Option<f32>,
    
    /// Actual time spent in hours
    pub actual_hours: Option<f32>,
    
    /// Files or code references related to this task
    pub file_references: Vec<String>,
    
    /// Repository references
    pub repository_references: Vec<String>,
    
    /// Custom metadata fields
    pub custom_fields: HashMap<String, String>,
    
    /// Conversation context that triggered this task
    pub conversation_context: Option<ConversationTaskContext>,
}

/// Context from a conversation that triggered a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTaskContext {
    /// The message that triggered the task creation
    pub trigger_message_id: Uuid,
    
    /// Summary of the conversation context
    pub context_summary: String,
    
    /// Key points or requirements extracted from the conversation
    pub requirements: Vec<String>,
    
    /// Expected outcomes or deliverables
    pub expected_outcomes: Vec<String>,
    
    /// Conversation branch if applicable
    pub branch_id: Option<Uuid>,
    
    /// Checkpoint reference if applicable
    pub checkpoint_id: Option<Uuid>,
}

/// Task creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub task_type: TaskType,
    pub priority: TaskPriority,
    pub due_date: Option<DateTime<Utc>>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub workspace_id: Option<Uuid>,
    pub source_conversation_id: Option<Uuid>,
    pub metadata: TaskMetadata,
    pub tags: Vec<String>,
}

/// Task update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub task_type: Option<TaskType>,
    pub priority: Option<TaskPriority>,
    pub status: Option<TaskStatus>,
    pub due_date: Option<DateTime<Utc>>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub metadata: Option<TaskMetadata>,
    pub tags: Option<Vec<String>>,
}

/// Task query for searching and filtering
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskQuery {
    pub workspace_id: Option<Uuid>,
    pub status: Option<TaskStatus>,
    pub task_type: Option<TaskType>,
    pub priority: Option<TaskPriority>,
    pub assigned_to: Option<String>,
    pub tags: Vec<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub due_after: Option<DateTime<Utc>>,
    pub due_before: Option<DateTime<Utc>>,
    pub text_search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Task search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSearchResult {
    pub task: Task,
    pub relevance_score: f32,
    pub matching_fields: Vec<String>,
}

/// Task summary for lists and overviews
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: Uuid,
    pub title: String,
    pub task_type: TaskType,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub due_date: Option<DateTime<Utc>>,
    pub workspace_id: Option<Uuid>,
    pub tags: Vec<String>,
}

/// Task execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionResult {
    pub task_id: Uuid,
    pub success: bool,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub conversation_id: Option<Uuid>,
    pub output: Option<String>,
    pub error_message: Option<String>,
    pub artifacts: Vec<TaskArtifact>,
}

/// Artifacts produced by task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskArtifact {
    pub name: String,
    pub artifact_type: ArtifactType,
    pub content: String,
    pub file_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Types of artifacts that can be produced
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactType {
    Code,
    Documentation,
    TestCase,
    Configuration,
    Report,
    Other(String),
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Normal
    }
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Pending
    }
}

impl Default for TaskType {
    fn default() -> Self {
        TaskType::Custom("General".to_string())
    }
}

impl Default for TaskMetadata {
    fn default() -> Self {
        Self {
            estimated_hours: None,
            actual_hours: None,
            file_references: Vec::new(),
            repository_references: Vec::new(),
            custom_fields: HashMap::new(),
            conversation_context: None,
        }
    }
}

impl Task {
    /// Create a new task
    pub fn new(title: String, task_type: TaskType) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title,
            description: None,
            task_type,
            priority: TaskPriority::default(),
            status: TaskStatus::default(),
            created_at: now,
            updated_at: now,
            due_date: None,
            scheduled_at: None,
            completed_at: None,
            source_conversation_id: None,
            target_conversation_id: None,
            workspace_id: None,
            metadata: TaskMetadata::default(),
            dependencies: Vec::new(),
            tags: Vec::new(),
        }
    }
    
    /// Check if task is overdue
    pub fn is_overdue(&self) -> bool {
        if let Some(due_date) = self.due_date {
            due_date < Utc::now() && !matches!(self.status, TaskStatus::Completed | TaskStatus::Cancelled)
        } else {
            false
        }
    }
    
    /// Check if task is ready to execute (all dependencies completed)
    pub fn is_ready_to_execute(&self, completed_tasks: &[Uuid]) -> bool {
        matches!(self.status, TaskStatus::Pending | TaskStatus::Scheduled) &&
        self.dependencies.iter().all(|dep| completed_tasks.contains(dep))
    }
    
    /// Update task status and timestamp
    pub fn update_status(&mut self, status: TaskStatus) {
        self.status = status;
        self.updated_at = Utc::now();
        
        if status == TaskStatus::Completed {
            self.completed_at = Some(Utc::now());
        }
    }
    
    /// Add a dependency
    pub fn add_dependency(&mut self, task_id: Uuid) {
        if !self.dependencies.contains(&task_id) {
            self.dependencies.push(task_id);
            self.updated_at = Utc::now();
        }
    }
    
    /// Remove a dependency
    pub fn remove_dependency(&mut self, task_id: Uuid) {
        if let Some(pos) = self.dependencies.iter().position(|&id| id == task_id) {
            self.dependencies.remove(pos);
            self.updated_at = Utc::now();
        }
    }
    
    /// Add a tag
    pub fn add_tag(&mut self, tag: String) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
            self.updated_at = Utc::now();
        }
    }
    
    /// Remove a tag
    pub fn remove_tag(&mut self, tag: &str) {
        if let Some(pos) = self.tags.iter().position(|t| t == tag) {
            self.tags.remove(pos);
            self.updated_at = Utc::now();
        }
    }
}

impl From<CreateTaskRequest> for Task {
    fn from(request: CreateTaskRequest) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title: request.title,
            description: request.description,
            task_type: request.task_type,
            priority: request.priority,
            status: if request.scheduled_at.is_some() { TaskStatus::Scheduled } else { TaskStatus::Pending },
            created_at: now,
            updated_at: now,
            due_date: request.due_date,
            scheduled_at: request.scheduled_at,
            completed_at: None,
            source_conversation_id: request.source_conversation_id,
            target_conversation_id: None,
            workspace_id: request.workspace_id,
            metadata: request.metadata,
            dependencies: Vec::new(),
            tags: request.tags,
        }
    }
}

impl From<&Task> for TaskSummary {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id,
            title: task.title.clone(),
            task_type: task.task_type.clone(),
            priority: task.priority,
            status: task.status.clone(),
            created_at: task.created_at,
            due_date: task.due_date,
            workspace_id: task.workspace_id,
            tags: task.tags.clone(),
        }
    }
} 