use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::types::*;
use crate::agent::conversation::types::Conversation;

/// Task manager trait for handling task operations
#[async_trait]
pub trait TaskManager: Send + Sync {
    /// Create a new task
    async fn create_task(&self, request: CreateTaskRequest) -> Result<Uuid>;
    
    /// Get a task by ID
    async fn get_task(&self, id: Uuid) -> Result<Option<Task>>;
    
    /// Update an existing task
    async fn update_task(&self, id: Uuid, request: UpdateTaskRequest) -> Result<()>;
    
    /// Delete a task
    async fn delete_task(&self, id: Uuid) -> Result<()>;
    
    /// List tasks with optional filtering
    async fn list_tasks(&self, query: TaskQuery) -> Result<Vec<TaskSummary>>;
    
    /// Search tasks
    async fn search_tasks(&self, query: TaskQuery) -> Result<Vec<TaskSearchResult>>;
    
    /// Execute a task
    async fn execute_task(&self, id: Uuid) -> Result<TaskExecutionResult>;
    
    /// Get tasks by workspace
    async fn get_workspace_tasks(&self, workspace_id: Uuid) -> Result<Vec<TaskSummary>>;
    
    /// Get tasks by conversation
    async fn get_conversation_tasks(&self, conversation_id: Uuid) -> Result<Vec<TaskSummary>>;
    
    /// Get overdue tasks
    async fn get_overdue_tasks(&self) -> Result<Vec<TaskSummary>>;
    
    /// Get ready tasks (no pending dependencies)
    async fn get_ready_tasks(&self) -> Result<Vec<TaskSummary>>;
}

/// In-memory task manager implementation
pub struct InMemoryTaskManager {
    tasks: Arc<RwLock<HashMap<Uuid, Task>>>,
    conversation_manager: Option<Arc<dyn crate::agent::conversation::manager::ConversationManager>>,
}

impl InMemoryTaskManager {
    /// Create a new in-memory task manager
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            conversation_manager: None,
        }
    }
    
    /// Create with conversation manager for integration
    pub fn with_conversation_manager(
        conversation_manager: Arc<dyn crate::agent::conversation::manager::ConversationManager>
    ) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            conversation_manager: Some(conversation_manager),
        }
    }
    
    /// Create a task from a conversation
    pub async fn create_task_from_conversation(
        &self,
        conversation: &Conversation,
        message_id: Uuid,
        task_type: TaskType,
        title: String,
        requirements: Vec<String>,
        expected_outcomes: Vec<String>,
    ) -> Result<Uuid> {
        let context = ConversationTaskContext {
            trigger_message_id: message_id,
            context_summary: format!("Task created from conversation: {}", conversation.title),
            requirements,
            expected_outcomes,
            branch_id: None, // Could be enhanced to detect current branch
            checkpoint_id: None, // Could be enhanced to reference latest checkpoint
        };
        
        let metadata = TaskMetadata {
            conversation_context: Some(context),
            ..Default::default()
        };
        
        let request = CreateTaskRequest {
            title,
            description: Some(format!("Task generated from conversation: {}", conversation.title)),
            task_type,
            priority: TaskPriority::Normal,
            due_date: None,
            scheduled_at: None,
            workspace_id: conversation.workspace_id,
            source_conversation_id: Some(conversation.id),
            metadata,
            tags: conversation.tags.clone(),
        };
        
        self.create_task(request).await
    }
    
    /// Create a conversation from a task
    pub async fn create_conversation_from_task(&self, task: &Task) -> Result<Option<Uuid>> {
        if let Some(ref conv_manager) = self.conversation_manager {
            let title = format!("Task: {}", task.title);
            let conversation_id = conv_manager.create_conversation(title, task.workspace_id).await?;
            
            // Add initial message with task context
            if let Some(ref context) = task.metadata.conversation_context {
                let _initial_message = format!(
                    "Starting work on task: {}\n\nRequirements:\n{}\n\nExpected outcomes:\n{}",
                    task.title,
                    context.requirements.join("\n- "),
                    context.expected_outcomes.join("\n- ")
                );
                
                // Note: This would require extending the conversation manager to add messages
                // For now, we just return the conversation ID
            }
            
            Ok(Some(conversation_id))
        } else {
            Ok(None)
        }
    }
    
    /// Filter tasks based on query
    fn filter_tasks(&self, tasks: &[Task], query: &TaskQuery) -> Vec<Task> {
        tasks.iter().filter(|task| {
            // Workspace filter
            if let Some(workspace_id) = query.workspace_id {
                if task.workspace_id != Some(workspace_id) {
                    return false;
                }
            }
            
            // Status filter
            if let Some(ref status) = query.status {
                if &task.status != status {
                    return false;
                }
            }
            
            // Task type filter
            if let Some(ref task_type) = query.task_type {
                if &task.task_type != task_type {
                    return false;
                }
            }
            
            // Priority filter
            if let Some(priority) = query.priority {
                if task.priority != priority {
                    return false;
                }
            }
            
            // Tags filter
            if !query.tags.is_empty()
                && !query.tags.iter().any(|tag| task.tags.contains(tag)) {
                    return false;
                }
            
            // Date filters
            if let Some(created_after) = query.created_after {
                if task.created_at < created_after {
                    return false;
                }
            }
            
            if let Some(created_before) = query.created_before {
                if task.created_at > created_before {
                    return false;
                }
            }
            
            if let Some(due_after) = query.due_after {
                if let Some(due_date) = task.due_date {
                    if due_date < due_after {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            
            if let Some(due_before) = query.due_before {
                if let Some(due_date) = task.due_date {
                    if due_date > due_before {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            
            // Text search
            if let Some(ref search_text) = query.text_search {
                let search_lower = search_text.to_lowercase();
                if !task.title.to_lowercase().contains(&search_lower) &&
                   !task.description.as_ref().is_some_and(|d| d.to_lowercase().contains(&search_lower)) &&
                   !task.tags.iter().any(|tag| tag.to_lowercase().contains(&search_lower)) {
                    return false;
                }
            }
            
            true
        }).cloned().collect()
    }
    
    /// Get completed task IDs for dependency checking
    async fn get_completed_task_ids(&self) -> Vec<Uuid> {
        let tasks = self.tasks.read().await;
        tasks.values()
            .filter(|task| task.status == TaskStatus::Completed)
            .map(|task| task.id)
            .collect()
    }
}

#[async_trait]
impl TaskManager for InMemoryTaskManager {
    async fn create_task(&self, request: CreateTaskRequest) -> Result<Uuid> {
        let task = Task::from(request);
        let task_id = task.id;
        
        let mut tasks = self.tasks.write().await;
        tasks.insert(task_id, task);
        
        Ok(task_id)
    }
    
    async fn get_task(&self, id: Uuid) -> Result<Option<Task>> {
        let tasks = self.tasks.read().await;
        Ok(tasks.get(&id).cloned())
    }
    
    async fn update_task(&self, id: Uuid, request: UpdateTaskRequest) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        
        if let Some(task) = tasks.get_mut(&id) {
            if let Some(title) = request.title {
                task.title = title;
            }
            if let Some(description) = request.description {
                task.description = Some(description);
            }
            if let Some(task_type) = request.task_type {
                task.task_type = task_type;
            }
            if let Some(priority) = request.priority {
                task.priority = priority;
            }
            if let Some(status) = request.status {
                task.update_status(status);
            }
            if let Some(due_date) = request.due_date {
                task.due_date = Some(due_date);
            }
            if let Some(scheduled_at) = request.scheduled_at {
                task.scheduled_at = Some(scheduled_at);
            }
            if let Some(completed_at) = request.completed_at {
                task.completed_at = Some(completed_at);
            }
            if let Some(metadata) = request.metadata {
                task.metadata = metadata;
            }
            if let Some(tags) = request.tags {
                task.tags = tags;
            }
            
            task.updated_at = Utc::now();
        }
        
        Ok(())
    }
    
    async fn delete_task(&self, id: Uuid) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        tasks.remove(&id);
        Ok(())
    }
    
    async fn list_tasks(&self, query: TaskQuery) -> Result<Vec<TaskSummary>> {
        let tasks = self.tasks.read().await;
        let all_tasks: Vec<Task> = tasks.values().cloned().collect();
        let filtered_tasks = self.filter_tasks(&all_tasks, &query);
        
        let mut summaries: Vec<TaskSummary> = filtered_tasks.iter().map(TaskSummary::from).collect();
        
        // Sort by priority and creation date
        summaries.sort_by(|a, b| {
            b.priority.cmp(&a.priority).then(b.created_at.cmp(&a.created_at))
        });
        
        // Apply limit and offset
        if let Some(offset) = query.offset {
            if offset < summaries.len() {
                summaries = summaries.into_iter().skip(offset).collect();
            } else {
                summaries.clear();
            }
        }
        
        if let Some(limit) = query.limit {
            summaries.truncate(limit);
        }
        
        Ok(summaries)
    }
    
    async fn search_tasks(&self, query: TaskQuery) -> Result<Vec<TaskSearchResult>> {
        let tasks = self.tasks.read().await;
        let all_tasks: Vec<Task> = tasks.values().cloned().collect();
        let filtered_tasks = self.filter_tasks(&all_tasks, &query);
        
        let mut results: Vec<TaskSearchResult> = filtered_tasks.into_iter().map(|task| {
            let mut relevance_score = 1.0;
            let mut matching_fields = Vec::new();
            
            // Calculate relevance based on search criteria
            if let Some(ref search_text) = query.text_search {
                let search_lower = search_text.to_lowercase();
                
                if task.title.to_lowercase().contains(&search_lower) {
                    relevance_score += 0.5;
                    matching_fields.push("title".to_string());
                }
                
                if task.description.as_ref().is_some_and(|d| d.to_lowercase().contains(&search_lower)) {
                    relevance_score += 0.3;
                    matching_fields.push("description".to_string());
                }
                
                if task.tags.iter().any(|tag| tag.to_lowercase().contains(&search_lower)) {
                    relevance_score += 0.2;
                    matching_fields.push("tags".to_string());
                }
            }
            
            // Boost score for high priority tasks
            match task.priority {
                TaskPriority::Critical => relevance_score += 0.4,
                TaskPriority::High => relevance_score += 0.2,
                _ => {}
            }
            
            // Boost score for overdue tasks
            if task.is_overdue() {
                relevance_score += 0.3;
                matching_fields.push("overdue".to_string());
            }
            
            TaskSearchResult {
                task,
                relevance_score,
                matching_fields,
            }
        }).collect();
        
        // Sort by relevance score
        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
        
        // Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }
        
        Ok(results)
    }
    
    async fn execute_task(&self, id: Uuid) -> Result<TaskExecutionResult> {
        let started_at = Utc::now();
        
        // Update task status to in progress
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(&id) {
                task.status = TaskStatus::InProgress;
            } else {
                return Err(anyhow::anyhow!("Task not found: {}", id));
            }
        }
        
        // Get the task
        let task = self.get_task(id).await?
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", id))?;
        
        // Simulate task execution
        // In a real implementation, this would:
        // 1. Parse the task requirements
        // 2. Execute the appropriate actions
        // 3. Generate artifacts
        // 4. Update progress
        
        let artifacts = vec![
            TaskArtifact {
                name: format!("{}_result.txt", task.title.replace(' ', "_")),
                artifact_type: ArtifactType::Code,
                content: format!("Task '{}' completed successfully", task.title),
                file_path: Some(format!("/tmp/{}_result.txt", task.id)),
                created_at: Utc::now(),
            }
        ];
        
        let completed_at = Utc::now();
        
        // Update task status to completed
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(&id) {
                task.status = TaskStatus::Completed;
                task.completed_at = Some(completed_at);
            }
        }
        
        Ok(TaskExecutionResult {
            task_id: id,
            success: true,
            started_at,
            completed_at,
            conversation_id: None,
            output: Some(format!("Task '{}' completed successfully", task.title)),
            error_message: None,
            artifacts,
        })
    }
    
    async fn get_workspace_tasks(&self, workspace_id: Uuid) -> Result<Vec<TaskSummary>> {
        let query = TaskQuery {
            workspace_id: Some(workspace_id),
            ..Default::default()
        };
        self.list_tasks(query).await
    }
    
    async fn get_conversation_tasks(&self, conversation_id: Uuid) -> Result<Vec<TaskSummary>> {
        let tasks = self.tasks.read().await;
        let summaries: Vec<TaskSummary> = tasks.values()
            .filter(|task| task.source_conversation_id == Some(conversation_id))
            .map(TaskSummary::from)
            .collect();
        Ok(summaries)
    }
    
    async fn get_overdue_tasks(&self) -> Result<Vec<TaskSummary>> {
        let tasks = self.tasks.read().await;
        let summaries: Vec<TaskSummary> = tasks.values()
            .filter(|task| task.is_overdue())
            .map(TaskSummary::from)
            .collect();
        Ok(summaries)
    }
    
    async fn get_ready_tasks(&self) -> Result<Vec<TaskSummary>> {
        let completed_task_ids = self.get_completed_task_ids().await;
        let tasks = self.tasks.read().await;
        let summaries: Vec<TaskSummary> = tasks.values()
            .filter(|task| task.is_ready_to_execute(&completed_task_ids))
            .map(TaskSummary::from)
            .collect();
        Ok(summaries)
    }
}

impl Default for InMemoryTaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for UpdateTaskRequest {
    fn default() -> Self {
        Self {
            title: None,
            description: None,
            task_type: None,
            priority: None,
            status: None,
            due_date: None,
            scheduled_at: None,
            completed_at: None,
            metadata: None,
            tags: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    
    #[tokio::test]
    async fn test_create_and_get_task() {
        let manager = InMemoryTaskManager::new();
        
        let request = CreateTaskRequest {
            title: "Test Task".to_string(),
            description: Some("Test description".to_string()),
            task_type: TaskType::CodeAnalysis,
            priority: TaskPriority::High,
            due_date: None,
            scheduled_at: None,
            workspace_id: None,
            source_conversation_id: None,
            metadata: TaskMetadata::default(),
            tags: vec!["test".to_string()],
        };
        
        let task_id = manager.create_task(request).await.unwrap();
        let task = manager.get_task(task_id).await.unwrap().unwrap();
        
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.task_type, TaskType::CodeAnalysis);
        assert_eq!(task.priority, TaskPriority::High);
        assert_eq!(task.tags, vec!["test"]);
    }
    
    #[tokio::test]
    async fn test_update_task() {
        let manager = InMemoryTaskManager::new();
        
        let request = CreateTaskRequest {
            title: "Original Title".to_string(),
            description: None,
            task_type: TaskType::Documentation,
            priority: TaskPriority::Normal,
            due_date: None,
            scheduled_at: None,
            workspace_id: None,
            source_conversation_id: None,
            metadata: TaskMetadata::default(),
            tags: Vec::new(),
        };
        
        let task_id = manager.create_task(request).await.unwrap();
        
        let update_request = UpdateTaskRequest {
            title: Some("Updated Title".to_string()),
            status: Some(TaskStatus::InProgress),
            priority: Some(TaskPriority::High),
            ..Default::default()
        };
        
        manager.update_task(task_id, update_request).await.unwrap();
        
        let task = manager.get_task(task_id).await.unwrap().unwrap();
        assert_eq!(task.title, "Updated Title");
        assert_eq!(task.status, TaskStatus::InProgress);
        assert_eq!(task.priority, TaskPriority::High);
    }
    
    #[tokio::test]
    async fn test_list_tasks_with_filters() {
        let manager = InMemoryTaskManager::new();
        
        // Create test tasks
        let workspace_id = Uuid::new_v4();
        
        let request1 = CreateTaskRequest {
            title: "High Priority Task".to_string(),
            description: None,
            task_type: TaskType::CodeAnalysis,
            priority: TaskPriority::High,
            due_date: None,
            scheduled_at: None,
            workspace_id: Some(workspace_id),
            source_conversation_id: None,
            metadata: TaskMetadata::default(),
            tags: vec!["urgent".to_string()],
        };
        
        let request2 = CreateTaskRequest {
            title: "Normal Task".to_string(),
            description: None,
            task_type: TaskType::Documentation,
            priority: TaskPriority::Normal,
            due_date: None,
            scheduled_at: None,
            workspace_id: Some(workspace_id),
            source_conversation_id: None,
            metadata: TaskMetadata::default(),
            tags: vec!["docs".to_string()],
        };
        
        manager.create_task(request1).await.unwrap();
        manager.create_task(request2).await.unwrap();
        
        // Test filtering by priority
        let query = TaskQuery {
            priority: Some(TaskPriority::High),
            ..Default::default()
        };
        
        let results = manager.list_tasks(query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "High Priority Task");
        
        // Test filtering by workspace
        let query = TaskQuery {
            workspace_id: Some(workspace_id),
            ..Default::default()
        };
        
        let results = manager.list_tasks(query).await.unwrap();
        assert_eq!(results.len(), 2);
    }
    
    #[tokio::test]
    async fn test_search_tasks() {
        let manager = InMemoryTaskManager::new();
        
        let request = CreateTaskRequest {
            title: "Code Analysis Task".to_string(),
            description: Some("Analyze the codebase for issues".to_string()),
            task_type: TaskType::CodeAnalysis,
            priority: TaskPriority::Normal,
            due_date: None,
            scheduled_at: None,
            workspace_id: None,
            source_conversation_id: None,
            metadata: TaskMetadata::default(),
            tags: vec!["analysis".to_string(), "code".to_string()],
        };
        
        manager.create_task(request).await.unwrap();
        
        let query = TaskQuery {
            text_search: Some("code".to_string()),
            ..Default::default()
        };
        
        let results = manager.search_tasks(query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].relevance_score > 1.0);
        assert!(results[0].matching_fields.contains(&"title".to_string()));
        assert!(results[0].matching_fields.contains(&"tags".to_string()));
    }
    
    #[tokio::test]
    async fn test_task_execution() {
        let manager = InMemoryTaskManager::new();
        
        let request = CreateTaskRequest {
            title: "Test Execution".to_string(),
            description: None,
            task_type: TaskType::Testing,
            priority: TaskPriority::Normal,
            due_date: None,
            scheduled_at: None,
            workspace_id: None,
            source_conversation_id: None,
            metadata: TaskMetadata::default(),
            tags: Vec::new(),
        };
        
        let task_id = manager.create_task(request).await.unwrap();
        let result = manager.execute_task(task_id).await.unwrap();
        
        assert!(result.success);
        assert_eq!(result.task_id, task_id);
        
        // Verify task status was updated
        let task = manager.get_task(task_id).await.unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.completed_at.is_some());
    }
} 