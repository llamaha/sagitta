use crate::mcp::types::{TodoWriteParams, TodoWriteResult, TodoItem, TodoStatus, ErrorObject};
use crate::middleware::auth_middleware::AuthenticatedUser;
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use axum::Extension;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use serde_json;
use chrono::Utc;

/// Get the path to the todos JSON file
fn get_todos_file_path() -> std::path::PathBuf {
    // Store in .sagitta directory in the current working directory
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    workspace_root.join(".sagitta").join("todos.json")
}

/// Handler for writing todos
pub async fn handle_todo_write<C: QdrantClientTrait + Send + Sync + 'static>(
    params: TodoWriteParams,
    _config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<TodoWriteResult, ErrorObject> {
    let todos_path = get_todos_file_path();
    
    // Ensure .sagitta directory exists
    if let Some(parent) = todos_path.parent() {
        if let Err(e) = fs::create_dir_all(parent).await {
            return Err(ErrorObject {
                code: -32603,
                message: format!("Failed to create .sagitta directory: {}", e),
                data: None,
            });
        }
    }
    
    // Add timestamps to todos that don't have them
    let now = Utc::now().to_rfc3339();
    let mut todos = params.todos;
    for todo in &mut todos {
        if todo.created_at.is_none() {
            todo.created_at = Some(now.clone());
        }
        todo.updated_at = Some(now.clone());
    }
    
    // Write todos to file
    let json_content = match serde_json::to_string_pretty(&todos) {
        Ok(content) => content,
        Err(e) => {
            return Err(ErrorObject {
                code: -32603,
                message: format!("Failed to serialize todos: {}", e),
                data: None,
            });
        }
    };
    
    if let Err(e) = fs::write(&todos_path, &json_content).await {
        return Err(ErrorObject {
            code: -32603,
            message: format!("Failed to write todos file: {}", e),
            data: None,
        });
    }
    
    // Calculate summary
    let total = todos.len();
    let completed = todos.iter().filter(|t| t.status == TodoStatus::Completed).count();
    let in_progress = todos.iter().filter(|t| t.status == TodoStatus::InProgress).count();
    let pending = todos.iter().filter(|t| t.status == TodoStatus::Pending).count();
    
    let summary = format!("Updated {} todos: {} completed, {} in progress, {} pending", 
                         total, completed, in_progress, pending);
    
    Ok(TodoWriteResult {
        todos,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::TodoPriority;
    use tempfile::TempDir;
    use std::sync::Mutex;
    
    // Use a mutex to ensure tests don't run concurrently since they change the current directory
    static TEST_MUTEX: Mutex<()> = Mutex::new(());
    
    async fn create_test_config() -> (Arc<RwLock<AppConfig>>, TempDir, std::sync::MutexGuard<'static, ()>) {
        let guard = TEST_MUTEX.lock().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let work_dir = temp_dir.path().to_path_buf();
        
        // Change working directory to the temp dir for tests
        std::env::set_current_dir(&work_dir).unwrap();
        
        let config = AppConfig::default();
        
        (Arc::new(RwLock::new(config)), temp_dir, guard)
    }
    
    fn create_mock_qdrant() -> Arc<qdrant_client::Qdrant> {
        Arc::new(qdrant_client::Qdrant::from_url("http://localhost:6334").build().unwrap())
    }
    
    #[tokio::test]
    async fn test_todo_write_creates_file() {
        let (config, temp_dir, _guard) = create_test_config().await;
        let qdrant_client = create_mock_qdrant();
        
        let todos = vec![
            TodoItem {
                id: "1".to_string(),
                content: "Test todo 1".to_string(),
                status: TodoStatus::Pending,
                priority: TodoPriority::High,
                created_at: None,
                updated_at: None,
            },
        ];
        
        let params = TodoWriteParams { todos: todos.clone() };
        
        let result = handle_todo_write(
            params,
            config,
            qdrant_client,
            None,
        ).await.unwrap();
        
        assert_eq!(result.todos.len(), 1);
        assert_eq!(result.summary, "Updated 1 todos: 0 completed, 0 in progress, 1 pending");
        
        // Verify file was created
        let todos_path = temp_dir.path().join(".sagitta").join("todos.json");
        assert!(todos_path.exists());
        
        // Verify timestamps were added
        assert!(result.todos[0].created_at.is_some());
        assert!(result.todos[0].updated_at.is_some());
    }
    
    #[tokio::test]
    async fn test_todo_write_overwrites_existing() {
        let (config, temp_dir, _guard) = create_test_config().await;
        let qdrant_client = create_mock_qdrant();
        
        // Create initial todos file
        let todos_path = get_todos_file_path();
        fs::create_dir_all(todos_path.parent().unwrap()).await.unwrap();
        fs::write(&todos_path, "[{\"id\":\"old\",\"content\":\"Old todo\"}]").await.unwrap();
        
        let new_todos = vec![
            TodoItem {
                id: "new1".to_string(),
                content: "New todo 1".to_string(),
                status: TodoStatus::Completed,
                priority: TodoPriority::Low,
                created_at: None,
                updated_at: None,
            },
            TodoItem {
                id: "new2".to_string(),
                content: "New todo 2".to_string(),
                status: TodoStatus::InProgress,
                priority: TodoPriority::Medium,
                created_at: None,
                updated_at: None,
            },
        ];
        
        let params = TodoWriteParams { todos: new_todos };
        
        let result = handle_todo_write(
            params,
            config,
            qdrant_client,
            None,
        ).await.unwrap();
        
        assert_eq!(result.todos.len(), 2);
        assert_eq!(result.summary, "Updated 2 todos: 1 completed, 1 in progress, 0 pending");
        
        // Verify old content was replaced
        let file_content = fs::read_to_string(&todos_path).await.unwrap();
        assert!(!file_content.contains("old"));
        assert!(file_content.contains("new1"));
        assert!(file_content.contains("new2"));
    }
    
    #[tokio::test]
    async fn test_todo_write_preserves_existing_timestamps() {
        let (config, _temp_dir, _guard) = create_test_config().await;
        let qdrant_client = create_mock_qdrant();
        
        let existing_timestamp = "2024-01-01T00:00:00Z";
        let todos = vec![
            TodoItem {
                id: "1".to_string(),
                content: "Test todo with timestamp".to_string(),
                status: TodoStatus::Pending,
                priority: TodoPriority::High,
                created_at: Some(existing_timestamp.to_string()),
                updated_at: Some("2024-01-01T12:00:00Z".to_string()),
            },
        ];
        
        let params = TodoWriteParams { todos };
        
        let result = handle_todo_write(
            params,
            config,
            qdrant_client,
            None,
        ).await.unwrap();
        
        // Verify created_at was preserved but updated_at was changed
        assert_eq!(result.todos[0].created_at, Some(existing_timestamp.to_string()));
        assert_ne!(result.todos[0].updated_at, Some("2024-01-01T12:00:00Z".to_string()));
    }
}