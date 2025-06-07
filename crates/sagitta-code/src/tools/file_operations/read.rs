// File reading tool will go here

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;
use sagitta_search::AppConfig as SagittaAppConfig;

/// Parameters for reading a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileParams {
    /// The repository to read from
    pub repository_name: String,
    
    /// The path to the file within the repository
    pub file_path: String,
    
    /// Optional start line (1-indexed)
    pub start_line: Option<u32>,
    
    /// Optional end line (1-indexed, inclusive)
    pub end_line: Option<u32>,
}

/// Tool for reading files from repositories
#[derive(Debug)]
pub struct ReadFileTool {
    /// Repository manager for accessing files
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl ReadFileTool {
    /// Create a new read file tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file from a repository".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["repository_name", "file_path"],
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "The repository containing the file"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file within the repository"
                    },
                    "start_line": {
                        "type": "integer",
                        "description": "Optional start line (1-indexed)"
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Optional end line (1-indexed, inclusive)"
                    }
                }
            }),
            is_required: false,
            category: ToolCategory::FileOperations,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        let params: ReadFileParams = match serde_json::from_value(parameters) {
            Ok(params) => params,
            Err(e) => return Ok(ToolResult::Error {
                error: format!("Invalid parameters: {}", e)
            })
        };
        
        let repo_manager = self.repo_manager.lock().await;
        let result = repo_manager.view_file(&params.repository_name, &params.file_path, None, None).await;
        
        match result {
            Ok(content) => Ok(ToolResult::Success(serde_json::json!({
                "file_path": params.file_path,
                "content": content,
                "size": content.len()
            }))),
            Err(e) => Ok(ToolResult::Error { 
                error: format!("Failed to read file: {}", e)
            })
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::repository::manager::RepositoryManager;
    use sagitta_search::AppConfig as SagittaAppConfig;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use serde_json::json;

    fn create_test_repo_manager() -> Arc<Mutex<RepositoryManager>> {
        let config = SagittaAppConfig::default();
        let repo_manager = RepositoryManager::new_for_test(Arc::new(Mutex::new(config)));
        Arc::new(Mutex::new(repo_manager))
    }

    #[test]
    fn test_read_file_tool_creation() {
        let tool = ReadFileTool::new(create_test_repo_manager());
        let definition = tool.definition();
        assert_eq!(definition.name, "read_file");
        assert_eq!(definition.category, ToolCategory::FileOperations);
    }

    #[tokio::test]
    async fn test_read_file_tool_error_handling() {
        let tool = ReadFileTool::new(create_test_repo_manager());
        
        // Test with non-existent repository
        let params = json!({
            "repository_name": "non-existent-repo",
            "file_path": "test.txt"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        // Should get ToolResult::Error, not a Rust error
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("non-existent-repo") || error.contains("Failed to read file"));
            }
            _ => panic!("Expected ToolResult::Error for non-existent repository"),
        }
    }

    #[test]
    fn test_read_file_tool_parameter_validation() {
        let tool = ReadFileTool::new(create_test_repo_manager());
        let definition = tool.definition();
        
        let properties = definition.parameters.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("repository_name"));
        assert!(properties.contains_key("file_path"));
        assert!(properties.contains_key("start_line"));
        assert!(properties.contains_key("end_line"));
        
        let required = definition.parameters.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("repository_name")));
        assert!(required.contains(&json!("file_path")));
    }
} 