use crate::mcp::types::{TodoReadParams, TodoReadResult, TodoItem, TodoStatus, ErrorObject};
use crate::middleware::auth_middleware::AuthenticatedUser;
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use axum::Extension;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use serde_json;

// Import the shared function from todo_write module
use super::todo_write::get_todos_file_path;

/// Handler for reading todos
pub async fn handle_todo_read<C: QdrantClientTrait + Send + Sync + 'static>(
    _params: TodoReadParams,
    _config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<TodoReadResult, ErrorObject> {
    let todos_path = get_todos_file_path();
    
    // Read todos from file if it exists
    let todos = if todos_path.exists() {
        match fs::read_to_string(&todos_path).await {
            Ok(contents) => {
                match serde_json::from_str::<Vec<TodoItem>>(&contents) {
                    Ok(todos) => todos,
                    Err(e) => {
                        return Err(ErrorObject {
                            code: -32603,
                            message: format!("Failed to parse todos file: {}", e),
                            data: None,
                        });
                    }
                }
            }
            Err(e) => {
                return Err(ErrorObject {
                    code: -32603,
                    message: format!("Failed to read todos file: {}", e),
                    data: None,
                });
            }
        }
    } else {
        // Return empty list if file doesn't exist
        Vec::new()
    };
    
    // Calculate summary
    let total = todos.len();
    let completed = todos.iter().filter(|t| t.status == TodoStatus::Completed).count();
    let in_progress = todos.iter().filter(|t| t.status == TodoStatus::InProgress).count();
    let pending = todos.iter().filter(|t| t.status == TodoStatus::Pending).count();
    
    let summary = if total == 0 {
        "No todos found".to_string()
    } else {
        format!("{} todos: {} completed, {} in progress, {} pending", total, completed, in_progress, pending)
    };
    
    Ok(TodoReadResult {
        todos,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::path::PathBuf;
    use crate::handlers::test_utils::TODO_TEST_MUTEX;
    
    async fn create_test_config() -> (Arc<RwLock<AppConfig>>, TempDir, std::sync::MutexGuard<'static, ()>) {
        // Handle poisoned mutex by clearing the poison
        let guard = TODO_TEST_MUTEX.lock().unwrap_or_else(|poisoned| {
            poisoned.into_inner()
        });
        let temp_dir = TempDir::new().unwrap();
        let work_dir = temp_dir.path().to_path_buf();
        
        // Create .sagitta directory
        fs::create_dir_all(work_dir.join(".sagitta")).await.unwrap();
        
        // Change working directory to the temp dir for tests
        std::env::set_current_dir(&work_dir).unwrap();
        
        let config = AppConfig::default();
        
        (Arc::new(RwLock::new(config)), temp_dir, guard)
    }
    
    fn create_mock_qdrant() -> Arc<qdrant_client::Qdrant> {
        // This would normally be a mock, but for now we'll just create a dummy client
        // In a real test, you'd use a mocking library
        Arc::new(qdrant_client::Qdrant::from_url("http://localhost:6334").build().unwrap())
    }
    
    #[tokio::test]
    async fn test_todo_read_empty_list() {
        let (config, _temp_dir, _guard) = create_test_config().await;
        let qdrant_client = create_mock_qdrant();
        
        // Clean up any existing todos file from other tests
        let todos_path = get_todos_file_path();
        let _ = tokio::fs::remove_file(&todos_path).await; // Ignore errors if file doesn't exist
        
        let result = handle_todo_read(
            TodoReadParams {},
            config,
            qdrant_client,
            None,
        ).await.unwrap();
        
        assert_eq!(result.todos.len(), 0);
        assert_eq!(result.summary, "No todos found");
    }
    
    #[tokio::test]
    async fn test_todo_read_with_existing_todos() {
        let (config, _temp_dir, _guard) = create_test_config().await;
        let qdrant_client = create_mock_qdrant();
        
        // Create test todos
        let test_todos = vec![
            TodoItem {
                id: "1".to_string(),
                content: "Test todo 1".to_string(),
                status: TodoStatus::Completed,
                priority: TodoPriority::High,
                created_at: None,
                updated_at: None,
            },
            TodoItem {
                id: "2".to_string(),
                content: "Test todo 2".to_string(),
                status: TodoStatus::InProgress,
                priority: TodoPriority::Medium,
                created_at: None,
                updated_at: None,
            },
            TodoItem {
                id: "3".to_string(),
                content: "Test todo 3".to_string(),
                status: TodoStatus::Pending,
                priority: TodoPriority::Low,
                created_at: None,
                updated_at: None,
            },
        ];
        
        // Write test todos to file - use the path function since we've changed directory
        let todos_path = get_todos_file_path();
        // Ensure parent directory exists
        if let Some(parent) = todos_path.parent() {
            fs::create_dir_all(parent).await.unwrap();
        }
        fs::write(&todos_path, serde_json::to_string(&test_todos).unwrap()).await.unwrap();
        
        let result = handle_todo_read(
            TodoReadParams {},
            config,
            qdrant_client,
            None,
        ).await.unwrap();
        
        assert_eq!(result.todos.len(), 3);
        assert_eq!(result.summary, "3 todos: 1 completed, 1 in progress, 1 pending");
    }
    
    #[tokio::test]
    async fn test_todo_read_corrupted_file() {
        let (config, _temp_dir, _guard) = create_test_config().await;
        let qdrant_client = create_mock_qdrant();
        
        // Write corrupted JSON to file - use the path function since we've changed directory
        let todos_path = get_todos_file_path();
        // Ensure parent directory exists
        if let Some(parent) = todos_path.parent() {
            fs::create_dir_all(parent).await.unwrap();
        }
        fs::write(&todos_path, "{ invalid json }").await.unwrap();
        
        let result = handle_todo_read(
            TodoReadParams {},
            config,
            qdrant_client,
            None,
        ).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, -32603);
        assert!(error.message.contains("Failed to parse todos file"));
    }
}