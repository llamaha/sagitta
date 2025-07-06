use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use super::types::*;
use super::manager::TaskManager;
use crate::agent::conversation::types::Conversation;
use crate::agent::message::types::AgentMessage;
use crate::agent::conversation::manager::ConversationManager;

/// Integration between conversations and tasks
pub struct ConversationTaskIntegration {
    task_manager: Arc<dyn TaskManager>,
    conversation_manager: Arc<dyn ConversationManager>,
}

impl ConversationTaskIntegration {
    /// Create a new conversation-task integration
    pub fn new(
        task_manager: Arc<dyn TaskManager>,
        conversation_manager: Arc<dyn ConversationManager>,
    ) -> Self {
        Self {
            task_manager,
            conversation_manager,
        }
    }
    
    /// Analyze a conversation message to detect task creation opportunities
    pub async fn analyze_message_for_tasks(&self, _conversation: &Conversation, message: &AgentMessage) -> Result<Vec<TaskSuggestion>> {
        let mut suggestions = Vec::new();
        
        // Look for task-related keywords and patterns
        let content = &message.content;
        let content_lower = content.to_lowercase();
        
        // TODO patterns that suggest task creation
        if content_lower.contains("todo") || content_lower.contains("to do") {
            suggestions.push(TaskSuggestion {
                task_type: TaskType::Custom("TODO".to_string()),
                title: extract_todo_title(content),
                description: Some(content.clone()),
                priority: TaskPriority::Normal,
                requirements: extract_requirements(content),
                expected_outcomes: extract_outcomes(content),
            });
        }
        
        // Code analysis requests
        if content_lower.contains("analyze") && (content_lower.contains("code") || content_lower.contains("codebase")) {
            suggestions.push(TaskSuggestion {
                task_type: TaskType::CodeAnalysis,
                title: "Code Analysis Task".to_string(),
                description: Some(content.clone()),
                priority: TaskPriority::Normal,
                requirements: extract_requirements(content),
                expected_outcomes: vec!["Analysis report".to_string()],
            });
        }
        
        // Documentation requests
        if content_lower.contains("document") || content_lower.contains("documentation") {
            suggestions.push(TaskSuggestion {
                task_type: TaskType::Documentation,
                title: "Documentation Task".to_string(),
                description: Some(content.clone()),
                priority: TaskPriority::Normal,
                requirements: extract_requirements(content),
                expected_outcomes: vec!["Updated documentation".to_string()],
            });
        }
        
        // Testing requests
        if content_lower.contains("test") && (content_lower.contains("write") || content_lower.contains("create")) {
            suggestions.push(TaskSuggestion {
                task_type: TaskType::Testing,
                title: "Testing Task".to_string(),
                description: Some(content.clone()),
                priority: TaskPriority::Normal,
                requirements: extract_requirements(content),
                expected_outcomes: vec!["Test cases created".to_string()],
            });
        }
        
        // Refactoring requests
        if content_lower.contains("refactor") || content_lower.contains("improve") {
            suggestions.push(TaskSuggestion {
                task_type: TaskType::Refactoring,
                title: "Refactoring Task".to_string(),
                description: Some(content.clone()),
                priority: TaskPriority::Normal,
                requirements: extract_requirements(content),
                expected_outcomes: vec!["Improved code structure".to_string()],
            });
        }
        
        // Research requests
        if content_lower.contains("research") || content_lower.contains("investigate") {
            suggestions.push(TaskSuggestion {
                task_type: TaskType::Research,
                title: "Research Task".to_string(),
                description: Some(content.clone()),
                priority: TaskPriority::Normal,
                requirements: extract_requirements(content),
                expected_outcomes: vec!["Research findings".to_string()],
            });
        }
        
        Ok(suggestions)
    }
    
    /// Create a task from a conversation message
    pub async fn create_task_from_message(
        &self,
        conversation: &Conversation,
        message: &AgentMessage,
        suggestion: TaskSuggestion,
    ) -> Result<Uuid> {
        let context = ConversationTaskContext {
            trigger_message_id: message.id,
            context_summary: format!("Task created from message in conversation: {}", conversation.title),
            requirements: suggestion.requirements,
            expected_outcomes: suggestion.expected_outcomes,
            branch_id: None, // Could be enhanced to detect current branch
            checkpoint_id: None, // Could be enhanced to reference latest checkpoint
        };
        
        let metadata = TaskMetadata {
            conversation_context: Some(context),
            file_references: extract_file_references(&message.content),
            repository_references: extract_repository_references(&message.content),
            ..Default::default()
        };
        
        let _request = CreateTaskRequest {
            title: suggestion.title,
            description: suggestion.description,
            task_type: suggestion.task_type,
            priority: suggestion.priority,
            due_date: None,
            scheduled_at: None,
            workspace_id: conversation.workspace_id,
            source_conversation_id: Some(conversation.id),
            metadata,
            tags: conversation.tags.clone(),
        };
        
        // Note: This requires the task manager to be mutable, which might need architectural changes
        // For now, we'll assume the task manager can handle this internally
        todo!("Implement task creation - requires mutable access to task manager")
    }
    
    /// Create a conversation from a task
    pub async fn create_conversation_from_task(&self, task: &Task) -> Result<Uuid> {
        let title = format!("Task: {}", task.title);
        let conversation_id = self.conversation_manager.create_conversation(title, task.workspace_id).await?;
        
        // Add initial message with task context
        if let Some(ref context) = task.metadata.conversation_context {
            let _initial_content = format!(
                "Starting work on task: {}\n\nRequirements:\n{}\n\nExpected outcomes:\n{}",
                task.title,
                context.requirements.iter().map(|r| format!("- {r}")).collect::<Vec<_>>().join("\n"),
                context.expected_outcomes.iter().map(|o| format!("- {o}")).collect::<Vec<_>>().join("\n")
            );
            
            // Note: This would require extending the conversation manager to add messages
            // For now, we just return the conversation ID
        }
        
        Ok(conversation_id)
    }
    
    /// Get tasks related to a conversation
    pub async fn get_conversation_tasks(&self, conversation_id: Uuid) -> Result<Vec<TaskSummary>> {
        self.task_manager.get_conversation_tasks(conversation_id).await
    }
    
    /// Get conversations related to a task
    pub async fn get_task_conversations(&self, task_id: Uuid) -> Result<Vec<Uuid>> {
        if let Some(task) = self.task_manager.get_task(task_id).await? {
            let mut conversation_ids = Vec::new();
            
            // Add source conversation
            if let Some(source_id) = task.source_conversation_id {
                conversation_ids.push(source_id);
            }
            
            // Add target conversation
            if let Some(target_id) = task.target_conversation_id {
                conversation_ids.push(target_id);
            }
            
            Ok(conversation_ids)
        } else {
            Ok(Vec::new())
        }
    }
    
    /// Update task progress based on conversation activity
    pub async fn update_task_progress(&self, _task_id: Uuid, conversation_id: Uuid) -> Result<()> {
        // Get the conversation to analyze progress
        if let Some(conversation) = self.conversation_manager.get_conversation(conversation_id).await? {
            // Analyze recent messages for progress indicators
            let recent_messages: Vec<&AgentMessage> = conversation.messages
                .iter()
                .rev()
                .take(5)
                .collect();
            
            let mut progress_indicators = Vec::new();
            
            for message in recent_messages {
                let content_lower = message.content.to_lowercase();
                
                if content_lower.contains("completed") || content_lower.contains("finished") || content_lower.contains("done") {
                    progress_indicators.push("completion");
                }
                
                if content_lower.contains("progress") || content_lower.contains("working on") {
                    progress_indicators.push("in_progress");
                }
                
                if content_lower.contains("blocked") || content_lower.contains("issue") || content_lower.contains("problem") {
                    progress_indicators.push("blocked");
                }
            }
            
            // Update task status based on indicators
            if progress_indicators.contains(&"completion") {
                let _update_request = UpdateTaskRequest {
                    status: Some(TaskStatus::Completed),
                    completed_at: Some(Utc::now()),
                    ..Default::default()
                };
                // Note: This requires mutable access to task manager
                todo!("Implement task update - requires mutable access to task manager");
            } else if progress_indicators.contains(&"in_progress") {
                let _update_request = UpdateTaskRequest {
                    status: Some(TaskStatus::InProgress),
                    ..Default::default()
                };
                // Note: This requires mutable access to task manager
                todo!("Implement task update - requires mutable access to task manager");
            }
        }
        
        Ok(())
    }
    
    /// Generate task summary for a conversation
    pub async fn generate_conversation_task_summary(&self, conversation_id: Uuid) -> Result<ConversationTaskSummary> {
        let tasks = self.get_conversation_tasks(conversation_id).await?;
        
        let total_tasks = tasks.len();
        let completed_tasks = tasks.iter().filter(|t| t.status == TaskStatus::Completed).count();
        let in_progress_tasks = tasks.iter().filter(|t| t.status == TaskStatus::InProgress).count();
        let pending_tasks = tasks.iter().filter(|t| t.status == TaskStatus::Pending).count();
        let overdue_tasks = tasks.iter().filter(|t| {
            if let Some(due_date) = t.due_date {
                due_date < Utc::now() && t.status != TaskStatus::Completed
            } else {
                false
            }
        }).count();
        
        Ok(ConversationTaskSummary {
            conversation_id,
            total_tasks,
            completed_tasks,
            in_progress_tasks,
            pending_tasks,
            overdue_tasks,
            task_types: tasks.iter().map(|t| t.task_type.clone()).collect(),
            recent_tasks: tasks.into_iter().take(5).collect(),
        })
    }
}

/// Task suggestion from conversation analysis
#[derive(Debug, Clone)]
pub struct TaskSuggestion {
    pub task_type: TaskType,
    pub title: String,
    pub description: Option<String>,
    pub priority: TaskPriority,
    pub requirements: Vec<String>,
    pub expected_outcomes: Vec<String>,
}

/// Summary of tasks related to a conversation
#[derive(Debug, Clone)]
pub struct ConversationTaskSummary {
    pub conversation_id: Uuid,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub in_progress_tasks: usize,
    pub pending_tasks: usize,
    pub overdue_tasks: usize,
    pub task_types: Vec<TaskType>,
    pub recent_tasks: Vec<TaskSummary>,
}

/// Extract TODO title from message content
fn extract_todo_title(content: &str) -> String {
    // Simple extraction - look for "TODO:" or "To do:" followed by text
    let lines: Vec<&str> = content.lines().collect();
    
    for line in lines {
        let line_lower = line.to_lowercase();
        if line_lower.contains("todo:") || line_lower.contains("to do:") {
            // Extract text after the colon
            if let Some(colon_pos) = line.find(':') {
                let title = line[colon_pos + 1..].trim();
                if !title.is_empty() {
                    return title.to_string();
                }
            }
        }
    }
    
    "TODO Task".to_string()
}

/// Extract requirements from message content
fn extract_requirements(content: &str) -> Vec<String> {
    let mut requirements = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    let mut in_requirements_section = false;
    
    for line in lines {
        let line_lower = line.to_lowercase();
        
        // Look for requirements section
        if line_lower.contains("requirement") || line_lower.contains("need") || line_lower.contains("must") {
            in_requirements_section = true;
            
            // Extract requirement from current line
            if line.contains('-') || line.contains('*') {
                let req = line.trim_start_matches('-').trim_start_matches('*').trim();
                if !req.is_empty() {
                    requirements.push(req.to_string());
                }
            }
        } else if in_requirements_section && (line.starts_with('-') || line.starts_with('*')) {
            let req = line.trim_start_matches('-').trim_start_matches('*').trim();
            if !req.is_empty() {
                requirements.push(req.to_string());
            }
        } else if in_requirements_section && line.trim().is_empty() {
            // Empty line might end requirements section
            in_requirements_section = false;
        }
    }
    
    // If no specific requirements found, use the whole content as a single requirement
    if requirements.is_empty() {
        requirements.push(content.trim().to_string());
    }
    
    requirements
}

/// Extract expected outcomes from message content
fn extract_outcomes(content: &str) -> Vec<String> {
    let mut outcomes = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    let mut in_outcomes_section = false;
    
    for line in lines {
        let line_lower = line.to_lowercase();
        
        // Look for outcomes section
        if line_lower.contains("outcome") || line_lower.contains("result") || line_lower.contains("deliverable") || line_lower.contains("expect") {
            in_outcomes_section = true;
            
            // Extract outcome from current line
            if line.contains('-') || line.contains('*') {
                let outcome = line.trim_start_matches('-').trim_start_matches('*').trim();
                if !outcome.is_empty() {
                    outcomes.push(outcome.to_string());
                }
            }
        } else if in_outcomes_section && (line.starts_with('-') || line.starts_with('*')) {
            let outcome = line.trim_start_matches('-').trim_start_matches('*').trim();
            if !outcome.is_empty() {
                outcomes.push(outcome.to_string());
            }
        } else if in_outcomes_section && line.trim().is_empty() {
            // Empty line might end outcomes section
            in_outcomes_section = false;
        }
    }
    
    // Default outcomes if none found
    if outcomes.is_empty() {
        outcomes.push("Task completion".to_string());
    }
    
    outcomes
}

/// Extract file references from message content
fn extract_file_references(content: &str) -> Vec<String> {
    let mut file_refs = Vec::new();
    
    // Look for common file patterns
    let file_patterns = [
        r"\b\w+\.\w+\b", // filename.extension
        r"\b[\w/]+\.rs\b", // Rust files
        r"\b[\w/]+\.py\b", // Python files
        r"\b[\w/]+\.js\b", // JavaScript files
        r"\b[\w/]+\.ts\b", // TypeScript files
        r"\b[\w/]+\.md\b", // Markdown files
        r"\b[\w/]+\.toml\b", // TOML files
        r"\b[\w/]+\.json\b", // JSON files
    ];
    
    for pattern in &file_patterns {
        if let Ok(regex) = regex::Regex::new(pattern) {
            for mat in regex.find_iter(content) {
                let file_ref = mat.as_str().to_string();
                if !file_refs.contains(&file_ref) {
                    file_refs.push(file_ref);
                }
            }
        }
    }
    
    file_refs
}

/// Extract repository references from message content
fn extract_repository_references(content: &str) -> Vec<String> {
    let mut repo_refs = Vec::new();
    
    // Look for repository patterns
    let repo_patterns = [
        r"https://github\.com/[\w-]+/[\w-]+",
        r"https://gitlab\.com/[\w-]+/[\w-]+",
        r"git@github\.com:[\w-]+/[\w-]+\.git",
        r"git@gitlab\.com:[\w-]+/[\w-]+\.git",
    ];
    
    for pattern in &repo_patterns {
        if let Ok(regex) = regex::Regex::new(pattern) {
            for mat in regex.find_iter(content) {
                let repo_ref = mat.as_str().to_string();
                if !repo_refs.contains(&repo_ref) {
                    repo_refs.push(repo_ref);
                }
            }
        }
    }
    
    repo_refs
}

#[cfg(test)]
mod tests {
    use super::*;
    
    
    #[test]
    fn test_extract_todo_title() {
        let content = "TODO: Implement user authentication\nThis is important for security.";
        let title = extract_todo_title(content);
        assert_eq!(title, "Implement user authentication");
        
        let content2 = "We need to do several things:\nTo do: Fix the bug in login";
        let title2 = extract_todo_title(content2);
        assert_eq!(title2, "Fix the bug in login");
    }
    
    #[test]
    fn test_extract_requirements() {
        let content = "Requirements:\n- Must support OAuth\n- Should be secure\n- Need error handling";
        let requirements = extract_requirements(content);
        assert_eq!(requirements.len(), 3);
        assert!(requirements.contains(&"Must support OAuth".to_string()));
        assert!(requirements.contains(&"Should be secure".to_string()));
        assert!(requirements.contains(&"Need error handling".to_string()));
    }
    
    #[test]
    fn test_extract_outcomes() {
        let content = "Expected outcomes:\n- Working authentication system\n- Secure user sessions";
        let outcomes = extract_outcomes(content);
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.contains(&"Working authentication system".to_string()));
        assert!(outcomes.contains(&"Secure user sessions".to_string()));
    }
    
    #[test]
    fn test_extract_file_references() {
        let content = "Please check src/auth.rs and update config.toml. Also look at package.json.";
        let file_refs = extract_file_references(content);
        assert!(file_refs.contains(&"auth.rs".to_string()));
        assert!(file_refs.contains(&"config.toml".to_string()));
        assert!(file_refs.contains(&"package.json".to_string()));
    }
    
    #[test]
    fn test_extract_repository_references() {
        let content = "Check out https://github.com/user/repo and also git@gitlab.com:user/project.git";
        let repo_refs = extract_repository_references(content);
        assert!(repo_refs.contains(&"https://github.com/user/repo".to_string()));
        assert!(repo_refs.contains(&"git@gitlab.com:user/project.git".to_string()));
    }
} 