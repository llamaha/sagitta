use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::tools::working_directory::{WorkingDirectoryManager, DirectoryContext};
use crate::utils::errors::SagittaCodeError;

/// Parameters for getting current working directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCurrentDirectoryParams {}

/// Parameters for changing working directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDirectoryParams {
    /// The directory to change to (absolute or relative)
    pub path: String,
}

/// Parameters for setting repository context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetRepositoryContextParams {
    /// The name of the repository to switch to
    pub repository_name: String,
}

/// Tool to get the current working directory context
#[derive(Debug)]
pub struct GetCurrentDirectoryTool {
    working_dir_manager: Arc<WorkingDirectoryManager>,
}

impl GetCurrentDirectoryTool {
    pub fn new(working_dir_manager: Arc<WorkingDirectoryManager>) -> Self {
        Self { working_dir_manager }
    }
}

#[async_trait]
impl Tool for GetCurrentDirectoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_current_directory".to_string(),
            description: "Get information about the current working directory context".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            is_required: false,
            category: ToolCategory::FileOperations,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, _parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let context = self.working_dir_manager.get_directory_context().await;
        
        let result_value = serde_json::json!({
            "current_directory": context.current_directory,
            "base_directory": context.base_directory,
            "exists": context.exists,
            "writable": context.writable,
            "stack_depth": context.stack_depth,
            "relative_to_base": context.current_directory.strip_prefix(&context.base_directory)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "outside_base".to_string())
        });
        
        Ok(ToolResult::Success(result_value))
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Tool to change the current working directory
#[derive(Debug)]
pub struct ChangeDirectoryTool {
    working_dir_manager: Arc<WorkingDirectoryManager>,
}

impl ChangeDirectoryTool {
    pub fn new(working_dir_manager: Arc<WorkingDirectoryManager>) -> Self {
        Self { working_dir_manager }
    }
}

#[async_trait]
impl Tool for ChangeDirectoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "change_directory".to_string(),
            description: "Change the current working directory for subsequent operations".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The directory to change to (absolute or relative to current directory)"
                    }
                },
                "required": ["path"]
            }),
            is_required: false,
            category: ToolCategory::FileOperations,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let params: ChangeDirectoryParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;
        
        let new_path = PathBuf::from(params.path);
        
        match self.working_dir_manager.change_directory(new_path).await {
            Ok(result) => {
                let result_value = serde_json::json!({
                    "success": result.success,
                    "previous_directory": result.previous_directory,
                    "new_directory": result.new_directory,
                    "message": result.message.unwrap_or_else(|| 
                        format!("Changed directory to {}", result.new_directory.display())
                    )
                });
                
                Ok(ToolResult::Success(result_value))
            }
            Err(e) => {
                Ok(ToolResult::Error { 
                    error: format!("Failed to change directory: {}", e)
                })
            }
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Tool to set the current working directory to a specific repository
#[derive(Debug)]
pub struct SetRepositoryContextTool {
    working_dir_manager: Arc<WorkingDirectoryManager>,
    repo_manager: Arc<tokio::sync::Mutex<crate::gui::repository::manager::RepositoryManager>>,
}

impl SetRepositoryContextTool {
    pub fn new(
        working_dir_manager: Arc<WorkingDirectoryManager>,
        repo_manager: Arc<tokio::sync::Mutex<crate::gui::repository::manager::RepositoryManager>>,
    ) -> Self {
        Self { 
            working_dir_manager,
            repo_manager,
        }
    }
}

#[async_trait]
impl Tool for SetRepositoryContextTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "set_repository_context".to_string(),
            description: "Set the working directory context to a specific repository for subsequent git and file operations".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "The name of the repository to switch to (must be in the repository list)"
                    }
                },
                "required": ["repository_name"]
            }),
            is_required: false,
            category: ToolCategory::FileOperations,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let params: SetRepositoryContextParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;
        
        let repo_manager = self.repo_manager.lock().await;
        
        match self.working_dir_manager.set_repository_context(&params.repository_name, &*repo_manager).await {
            Ok(result) => {
                let result_value = serde_json::json!({
                    "success": result.success,
                    "repository_name": params.repository_name,
                    "previous_directory": result.previous_directory,
                    "new_directory": result.new_directory,
                    "message": result.message.unwrap_or_else(|| 
                        format!("Switched to repository '{}' at {}", params.repository_name, result.new_directory.display())
                    )
                });
                
                Ok(ToolResult::Success(result_value))
            }
            Err(e) => {
                Ok(ToolResult::Error { 
                    error: format!("Failed to switch to repository '{}': {}", params.repository_name, e)
                })
            }
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[tokio::test]
    async fn test_get_current_directory_tool() {
        let temp_dir = TempDir::new().unwrap();
        let manager = Arc::new(WorkingDirectoryManager::new(temp_dir.path().to_path_buf()).unwrap());
        let tool = GetCurrentDirectoryTool::new(manager);
        
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(result.is_success());
        
        if let Some(data) = result.success_value() {
            assert!(data.get("current_directory").is_some());
            assert!(data.get("base_directory").is_some());
            assert!(data.get("exists").and_then(|v| v.as_bool()).unwrap_or(false));
        }
    }

    #[tokio::test]
    async fn test_change_directory_tool() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        
        let manager = Arc::new(WorkingDirectoryManager::new(temp_dir.path().to_path_buf()).unwrap());
        let tool = ChangeDirectoryTool::new(manager);
        
        let params = serde_json::json!({
            "path": "subdir"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        // Debug: print the actual result if it's not successful
        if !result.is_success() {
            match &result {
                ToolResult::Error { error } => {
                    println!("Tool returned error: {}", error);
                    panic!("Expected success but got error: {}", error);
                }
                ToolResult::Success(data) => {
                    println!("Tool returned success data: {}", serde_json::to_string_pretty(data).unwrap());
                    if let Some(success_val) = data.get("success").and_then(|v| v.as_bool()) {
                        if !success_val {
                            if let Some(message) = data.get("message").and_then(|v| v.as_str()) {
                                panic!("Change directory reported failure: {}", message);
                            } else {
                                panic!("Change directory reported failure with no message");
                            }
                        }
                    }
                }
            }
        }
        
        assert!(result.is_success());
        
        if let Some(data) = result.success_value() {
            assert_eq!(data.get("success").and_then(|v| v.as_bool()), Some(true));
        }
    }

    #[tokio::test]
    async fn test_change_directory_invalid_path() {
        let temp_dir = TempDir::new().unwrap();
        let manager = Arc::new(WorkingDirectoryManager::new(temp_dir.path().to_path_buf()).unwrap());
        let tool = ChangeDirectoryTool::new(manager);
        
        let params = serde_json::json!({
            "path": "nonexistent"
        });
        
        let result = tool.execute(params).await.unwrap();
        assert!(result.is_error());
    }
} 