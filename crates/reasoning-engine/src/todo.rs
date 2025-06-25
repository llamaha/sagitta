//! TODO management for multi-step task execution

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::error::{Result, ReasoningError};
use crate::state::core::{TodoItem, TodoList, TodoStatus};

/// TODO parser for extracting TODO lists from LLM responses
pub struct TodoParser;

impl TodoParser {
    /// Parse a TODO list from LLM response text
    pub fn parse_todo_list(response: &str, original_request: &str) -> Result<TodoList> {
        let mut items = Vec::new();
        
        // Look for numbered lists or bullet points
        let lines: Vec<&str> = response.lines().collect();
        let mut in_todo_section = false;
        let mut todo_counter = 0;
        
        for line in lines {
            let trimmed = line.trim();
            
            // Detect TODO section markers
            if trimmed.to_lowercase().contains("todo") || 
               trimmed.to_lowercase().contains("task list") ||
               trimmed.to_lowercase().contains("plan:") ||
               trimmed.to_lowercase().contains("steps:") {
                in_todo_section = true;
                continue;
            }
            
            // Stop at section end markers
            if in_todo_section && (trimmed.is_empty() || 
                trimmed.starts_with("---") || 
                trimmed.to_lowercase().starts_with("note")) {
                if !items.is_empty() {
                    break;
                }
            }
            
            // Parse TODO items
            if in_todo_section || Self::is_todo_item(trimmed) {
                if let Some(todo_text) = Self::extract_todo_text(trimmed) {
                    todo_counter += 1;
                    
                    let expected_tools = Self::infer_expected_tools(&todo_text);
                    let priority = Self::infer_priority(&todo_text, todo_counter);
                    
                    let todo_item = TodoItem {
                        id: Uuid::new_v4(),
                        description: todo_text,
                        status: TodoStatus::Pending,
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                        dependencies: Vec::new(), // TODO: Parse dependencies
                        expected_tools,
                        priority,
                        parent_id: None,
                    };
                    
                    items.push(todo_item);
                }
            }
        }
        
        if items.is_empty() {
            return Err(ReasoningError::configuration(
                "No TODO items found in response"
            ));
        }
        
        Ok(TodoList {
            items,
            created_at: Utc::now(),
            context: format!("Generated from request: {}", original_request),
            original_request: original_request.to_string(),
            user_approved: false,
            active_todo_id: None,
        })
    }
    
    /// Check if a line looks like a TODO item
    fn is_todo_item(line: &str) -> bool {
        // Check for numbered lists
        if line.chars().next().map(|c| c.is_numeric()).unwrap_or(false) {
            return line.contains('.') || line.contains(')');
        }
        
        // Check for bullet points
        line.starts_with('-') || line.starts_with('*') || line.starts_with('•') ||
        line.starts_with('+') || line.starts_with("[ ]") || line.starts_with("[x]")
    }
    
    /// Extract TODO text from a line
    fn extract_todo_text(line: &str) -> Option<String> {
        // Remove list markers
        let text = line
            .trim_start_matches(|c: char| c.is_numeric() || c.is_whitespace())
            .trim_start_matches('.')
            .trim_start_matches(')')
            .trim_start_matches('-')
            .trim_start_matches('*')
            .trim_start_matches('•')
            .trim_start_matches('+')
            .trim_start_matches("[ ]")
            .trim_start_matches("[x]")
            .trim();
        
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    }
    
    /// Infer expected tools from TODO description
    fn infer_expected_tools(description: &str) -> Vec<String> {
        let mut tools = Vec::new();
        let lower = description.to_lowercase();
        
        // File operations
        if lower.contains("create") || lower.contains("write") {
            tools.push("Write".to_string());
        }
        if lower.contains("read") || lower.contains("examine") || lower.contains("look at") {
            tools.push("Read".to_string());
        }
        if lower.contains("edit") || lower.contains("modify") || lower.contains("update") {
            tools.push("Edit".to_string());
        }
        
        // Search operations
        if lower.contains("search") || lower.contains("find") || lower.contains("locate") {
            tools.push("Grep".to_string());
            tools.push("Glob".to_string());
        }
        
        // Repository operations
        if lower.contains("repository") || lower.contains("repo") {
            if lower.contains("add") {
                tools.push("repository_add".to_string());
            }
            if lower.contains("sync") {
                tools.push("repository_sync".to_string());
            }
            if lower.contains("query") || lower.contains("search") {
                tools.push("query".to_string());
            }
        }
        
        // Execution
        if lower.contains("run") || lower.contains("execute") || lower.contains("test") {
            tools.push("Bash".to_string());
        }
        
        tools
    }
    
    /// Infer priority from TODO description and position
    fn infer_priority(description: &str, position: usize) -> u8 {
        let lower = description.to_lowercase();
        
        // High priority keywords
        if lower.contains("critical") || lower.contains("urgent") || lower.contains("immediately") {
            return 5;
        }
        
        // Setup/initialization tasks are high priority
        if lower.contains("setup") || lower.contains("initialize") || lower.contains("configure") {
            return 4;
        }
        
        // Default priority based on position (earlier = higher)
        match position {
            1..=3 => 3,
            4..=6 => 2,
            _ => 1,
        }
    }
}

/// TODO execution tracker
pub struct TodoExecutor {
    /// Track execution attempts for retry logic
    pub execution_attempts: std::collections::HashMap<Uuid, u32>,
    /// Track errors for each TODO
    pub todo_errors: std::collections::HashMap<Uuid, Vec<String>>,
}

impl TodoExecutor {
    pub fn new() -> Self {
        Self {
            execution_attempts: std::collections::HashMap::new(),
            todo_errors: std::collections::HashMap::new(),
        }
    }
    
    /// Check if a TODO should be retried
    pub fn should_retry(&self, todo_id: &Uuid) -> bool {
        self.execution_attempts.get(todo_id).unwrap_or(&0) < &3
    }
    
    /// Record execution attempt
    pub fn record_attempt(&mut self, todo_id: Uuid) {
        *self.execution_attempts.entry(todo_id).or_insert(0) += 1;
    }
    
    /// Record an error for a TODO
    pub fn record_error(&mut self, todo_id: Uuid, error: String) {
        self.todo_errors.entry(todo_id).or_insert_with(Vec::new).push(error);
    }
    
    /// Get errors for a TODO
    pub fn get_errors(&self, todo_id: &Uuid) -> Option<&Vec<String>> {
        self.todo_errors.get(todo_id)
    }
    
    /// Should skip a TODO based on dependency failures
    pub fn should_skip_due_to_dependencies(&self, todo: &TodoItem, all_todos: &[TodoItem]) -> bool {
        // Check if any dependencies failed
        todo.dependencies.iter().any(|dep_id| {
            all_todos.iter().any(|t| 
                &t.id == dep_id && matches!(t.status, TodoStatus::Failed(_))
            )
        })
    }
    
    /// Get recovery suggestions for a failed TODO
    pub fn get_recovery_suggestions(&self, todo: &TodoItem, error: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        
        // Repository-related errors
        if error.to_lowercase().contains("repository") {
            if error.contains("not found") || error.contains("does not exist") {
                suggestions.push("Verify the repository name is correct".to_string());
                suggestions.push("Use 'list_repositories' tool to see available repositories".to_string());
            } else if error.contains("already exists") {
                suggestions.push("Repository may already be added - try syncing instead".to_string());
                suggestions.push("Use a different name for the repository".to_string());
            }
        }
        
        // Network-related errors
        if error.to_lowercase().contains("network") || error.to_lowercase().contains("connection") {
            suggestions.push("Check your internet connection".to_string());
            suggestions.push("Verify the URL is accessible".to_string());
            suggestions.push("Try again later if it's a temporary issue".to_string());
        }
        
        // Permission errors
        if error.to_lowercase().contains("permission") || error.to_lowercase().contains("access denied") {
            suggestions.push("Check file/directory permissions".to_string());
            suggestions.push("Verify you have the necessary access rights".to_string());
        }
        
        // File not found errors
        if error.to_lowercase().contains("file not found") || error.to_lowercase().contains("no such file") {
            suggestions.push("Verify the file path is correct".to_string());
            suggestions.push("Check if the file was moved or deleted".to_string());
        }
        
        suggestions
    }
    
    /// Generate progress report
    pub fn generate_progress_report(todo_list: &TodoList) -> String {
        let total = todo_list.items.len();
        let completed = todo_list.items.iter()
            .filter(|item| matches!(item.status, TodoStatus::Completed))
            .count();
        let failed = todo_list.items.iter()
            .filter(|item| matches!(item.status, TodoStatus::Failed(_)))
            .count();
        let skipped = todo_list.items.iter()
            .filter(|item| matches!(item.status, TodoStatus::Skipped(_)))
            .count();
        let in_progress = todo_list.items.iter()
            .filter(|item| matches!(item.status, TodoStatus::InProgress))
            .count();
        
        let mut report = format!(
            "TODO Progress: {}/{} completed ({:.0}%)\n",
            completed, total, (completed as f32 / total as f32) * 100.0
        );
        
        if in_progress > 0 {
            report.push_str(&format!("- {} in progress\n", in_progress));
        }
        if failed > 0 {
            report.push_str(&format!("- {} failed\n", failed));
        }
        if skipped > 0 {
            report.push_str(&format!("- {} skipped\n", skipped));
        }
        
        // Add current task if any
        if let Some(active_id) = &todo_list.active_todo_id {
            if let Some(active_todo) = todo_list.items.iter().find(|i| &i.id == active_id) {
                report.push_str(&format!("\nCurrent task: {}", active_todo.description));
            }
        }
        
        report
    }
}

/// Check if response indicates TODO list creation is appropriate
pub fn should_create_todo_list(request: &str, autonomous_mode: bool) -> bool {
    if !autonomous_mode {
        return false;
    }
    
    let lower = request.to_lowercase();
    
    // Multi-step indicators
    let multi_step_keywords = [
        "and then", "followed by", "after that", "next",
        "steps", "multi", "several", "multiple",
        "workflow", "process", "procedure",
        "implement", "create a feature", "build",
    ];
    
    // Check for multiple verbs/actions
    let action_verbs = [
        "create", "add", "modify", "update", "delete",
        "implement", "build", "setup", "configure", "install",
        "test", "run", "execute", "analyze", "search",
        "sync", "fetch", "clone", "commit"
    ];
    
    let action_count = action_verbs.iter()
        .filter(|&&verb| lower.contains(verb))
        .count();
    
    // Create TODO list if:
    // 1. Multiple actions detected
    // 2. Multi-step keywords present
    // 3. Explicit list in request (numbered or bulleted)
    action_count >= 2 || 
    multi_step_keywords.iter().any(|&kw| lower.contains(kw)) ||
    contains_explicit_list(&lower)
}

/// Check if request contains an explicit list
fn contains_explicit_list(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    let mut list_item_count = 0;
    
    for line in lines {
        let trimmed = line.trim();
        // Check for numbered items or bullet points
        if trimmed.chars().next().map(|c| c.is_numeric()).unwrap_or(false) ||
           trimmed.starts_with('-') || trimmed.starts_with('*') || 
           trimmed.starts_with('•') || trimmed.starts_with('+') {
            list_item_count += 1;
        }
    }
    
    list_item_count >= 2
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_numbered_todo_list() {
        let response = r#"I'll help you with that. Here's what I'll do:

1. Add the repository to the index
2. Sync the repository to get latest changes  
3. Search for the specific function
4. Analyze the results

Let me start with step 1."#;
        
        let result = TodoParser::parse_todo_list(response, "test request").unwrap();
        assert_eq!(result.items.len(), 4);
        assert_eq!(result.items[0].description, "Add the repository to the index");
        assert!(result.items[0].expected_tools.contains(&"repository_add".to_string()));
    }
    
    #[test]
    fn test_should_create_todo_list() {
        assert!(should_create_todo_list("First create a file, then run tests, and finally commit", true));
        assert!(should_create_todo_list("Setup the environment and then build the project", true));
        assert!(!should_create_todo_list("What is 2+2?", true));
        assert!(!should_create_todo_list("Read the config file", true));
    }
}