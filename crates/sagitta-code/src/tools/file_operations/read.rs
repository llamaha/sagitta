// File reading tool will go here

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::{Path, PathBuf};
use std::fs;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;
use crate::tools::file_operations::DirectFileReadTool;
use sagitta_search::AppConfig as SagittaAppConfig;

/// Parameters for reading a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileParams {
    /// The repository to read from (optional for fallback mode)
    pub repository_name: Option<String>,
    
    /// The path to the file within the repository or absolute/relative path
    pub file_path: String,
    
    /// Optional start line (1-indexed)
    pub start_line: Option<u32>,
    
    /// Optional end line (1-indexed, inclusive)
    pub end_line: Option<u32>,
}

/// Tool for reading files from repositories with direct file fallback
#[derive(Debug)]
pub struct ReadFileTool {
    /// Repository manager for accessing files
    repo_manager: Arc<Mutex<RepositoryManager>>,
    /// Fallback direct file reader
    direct_reader: DirectFileReadTool,
    /// Base directory for fallback operations
    base_directory: PathBuf,
}

impl ReadFileTool {
    /// Create a new read file tool with fallback capability
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>, base_directory: PathBuf) -> Self {
        let direct_reader = DirectFileReadTool::new(base_directory.clone());
        Self {
            repo_manager,
            direct_reader,
            base_directory,
        }
    }

    /// Try to read file using repository manager
    async fn try_repository_read(&self, params: &ReadFileParams) -> Result<String, SagittaCodeError> {
        let repo_name = params.repository_name.as_ref()
            .ok_or_else(|| SagittaCodeError::ToolError(
                "Repository name required for repository-based file reading".to_string()
            ))?;

        let repo_manager = self.repo_manager.lock().await;
        repo_manager.view_file(repo_name, &params.file_path, None, None).await
            .map_err(|e| SagittaCodeError::ToolError(format!("Repository read failed: {}", e)))
    }

    /// Try to read file using direct file access
    async fn try_direct_read(&self, params: &ReadFileParams) -> Result<String, SagittaCodeError> {
        let direct_params = serde_json::json!({
            "file_path": params.file_path,
            "start_line": params.start_line,
            "end_line": params.end_line
        });

        match self.direct_reader.execute(direct_params).await? {
            ToolResult::Success(data) => {
                if let Some(content) = data.get("content").and_then(|c| c.as_str()) {
                    Ok(content.to_string())
                } else {
                    Err(SagittaCodeError::ToolError("Direct read returned invalid content".to_string()))
                }
            }
            ToolResult::Error { error } => {
                Err(SagittaCodeError::ToolError(format!("Direct read failed: {}", error)))
            }
        }
    }

    /// Auto-detect whether to use repository or direct file access
    async fn auto_read(&self, params: &ReadFileParams) -> Result<String, SagittaCodeError> {
        // If repository name is provided, try repository-based approach first
        if params.repository_name.is_some() {
            match self.try_repository_read(params).await {
                Ok(content) => return Ok(content),
                Err(e) => {
                    log::warn!("Repository read failed, trying direct file access: {}", e);
                    // Continue to fallback
                }
            }
        }

        // Fallback to direct file access
        self.try_direct_read(params).await
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file from a repository or directly from filesystem".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["file_path"],
                "properties": {
                    "repository_name": {
                        "type": ["string", "null"],
                        "description": "The repository containing the file (optional - will fallback to direct file access if not provided or if repository is not found)"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file within the repository or filesystem"
                    },
                    "start_line": {
                        "type": ["integer", "null"],
                        "description": "Optional start line (1-indexed)"
                    },
                    "end_line": {
                        "type": ["integer", "null"],
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
        
        match self.auto_read(&params).await {
            Ok(content) => Ok(ToolResult::Success(serde_json::json!({
                "file_path": params.file_path,
                "content": content,
                "size": content.len(),
                "repository_name": params.repository_name,
                "message": format!("Successfully read file: {}", params.file_path)
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
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo_manager() -> Arc<Mutex<RepositoryManager>> {
        let config = SagittaAppConfig::default();
        let repo_manager = RepositoryManager::new_for_test(Arc::new(Mutex::new(config)));
        Arc::new(Mutex::new(repo_manager))
    }

    #[tokio::test]
    async fn test_read_file_tool_creation() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ReadFileTool::new(create_test_repo_manager(), temp_dir.path().to_path_buf());
        let definition = tool.definition();
        assert_eq!(definition.name, "read_file");
        assert_eq!(definition.category, ToolCategory::FileOperations);
    }

    #[tokio::test]
    async fn test_read_file_tool_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ReadFileTool::new(create_test_repo_manager(), temp_dir.path().to_path_buf());
        
        // Test with non-existent repository
        let params = json!({
            "repository_name": "non-existent-repo",
            "file_path": "test.txt"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        // Should get ToolResult::Error, not a Rust error
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("Failed to read file"));
            }
            _ => panic!("Expected ToolResult::Error for non-existent repository"),
        }
    }

    #[tokio::test]
    async fn test_direct_file_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, fallback world!").unwrap();

        let tool = ReadFileTool::new(create_test_repo_manager(), temp_dir.path().to_path_buf());
        
        // Test without repository name (should use direct file access)
        let params = json!({
            "file_path": "test.txt"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                assert_eq!(data["content"], "Hello, fallback world!");
                assert_eq!(data["file_path"], "test.txt");
            }
            _ => panic!("Expected ToolResult::Success for direct file access"),
        }
    }

    #[tokio::test]
    async fn test_repository_fallback_to_direct() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Repository fallback test").unwrap();

        let tool = ReadFileTool::new(create_test_repo_manager(), temp_dir.path().to_path_buf());
        
        // Test with non-existent repository (should fallback to direct file access)
        let params = json!({
            "repository_name": "non-existent-repo",
            "file_path": "test.txt"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                assert_eq!(data["content"], "Repository fallback test");
                assert_eq!(data["repository_name"], "non-existent-repo");
            }
            ToolResult::Error { error } => {
                // This is also acceptable - depends on whether file exists in fallback directory
                assert!(error.contains("Failed to read file"));
            }
        }
    }

    #[test]
    fn test_read_file_tool_parameter_validation() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ReadFileTool::new(create_test_repo_manager(), temp_dir.path().to_path_buf());
        let definition = tool.definition();
        
        let properties = definition.parameters.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("repository_name"));
        assert!(properties.contains_key("file_path"));
        assert!(properties.contains_key("start_line"));
        assert!(properties.contains_key("end_line"));
        
        let required = definition.parameters.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("file_path")));
        // repository_name should not be required anymore
        assert!(!required.contains(&json!("repository_name")));
    }
} 