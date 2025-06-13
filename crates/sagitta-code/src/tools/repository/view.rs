use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;
use sagitta_search::AppConfig as SagittaAppConfig;

/// Parameters for viewing a file in a repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewFileParams {
    /// The repository containing the file
    pub repository_name: String,
    
    /// The path to the file within the repository
    pub file_path: String,
    
    /// Optional start line (1-indexed)
    pub start_line: Option<u32>,
    
    /// Optional end line (1-indexed, inclusive)
    pub end_line: Option<u32>,
}

/// Tool for viewing files in repositories
#[derive(Debug)]
pub struct ViewFileInRepositoryTool {
    /// Repository manager for accessing repositories
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl ViewFileInRepositoryTool {
    /// Create a new view file tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
}

#[async_trait]
impl Tool for ViewFileInRepositoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "view_file".to_string(),
            description: "View the complete contents of a file in a repository. **Best for files under 200 lines** - provides full context without filtering. For larger files (200+ lines), consider using 'targeted_view' to focus on relevant code elements and reduce context sent to LLM.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
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
            category: ToolCategory::Repository,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let params: ViewFileParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;
        
        let repo_manager = self.repo_manager.lock().await;
        let result = repo_manager.view_file(&params.repository_name, &params.file_path, params.start_line, params.end_line).await;
        
        match result {
            Ok(content) => Ok(ToolResult::Success(serde_json::json!({
                "file_path": params.file_path,
                "content": content,
                "start_line": params.start_line,
                "end_line": params.end_line
            }))),
            Err(e) => Err(SagittaCodeError::ToolError(format!("Failed to view file: {}", e)))
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
    fn test_view_file_tool_creation() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let definition = tool.definition();
        assert_eq!(definition.name, "view_file");
    }

    #[test]
    fn test_tool_definition() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let definition = tool.definition();
        
        assert_eq!(definition.name, "view_file");
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(definition.description.contains("View"));
    }

    #[test]
    fn test_tool_definition_properties() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
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

    #[test]
    fn test_tool_category_and_metadata() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let definition = tool.definition();
        
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(definition.description.len() > 10);
    }

    #[test]
    fn test_description_content() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let definition = tool.definition();
        
        assert!(definition.description.contains("View"));
        assert!(definition.description.contains("file"));
        assert!(definition.description.contains("repository"));
    }

    #[test]
    fn test_view_file_params_without_line_numbers() {
        let params = ViewFileParams {
            repository_name: "test-repo".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: None,
            end_line: None,
        };
        
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.file_path, "src/main.rs");
        assert!(params.start_line.is_none());
        assert!(params.end_line.is_none());
    }

    #[test]
    fn test_view_file_params_with_partial_range() {
        let params = ViewFileParams {
            repository_name: "test-repo".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: Some(10),
            end_line: None,
        };
        
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.file_path, "src/main.rs");
        assert_eq!(params.start_line, Some(10));
        assert!(params.end_line.is_none());
    }

    #[test]
    fn test_view_file_params_serialization() {
        let params = ViewFileParams {
            repository_name: "test-repo".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: Some(1),
            end_line: Some(50),
        };
        
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("test-repo"));
        assert!(json.contains("src/main.rs"));
        
        let deserialized: ViewFileParams = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.repository_name, params.repository_name);
        assert_eq!(deserialized.file_path, params.file_path);
        assert_eq!(deserialized.start_line, params.start_line);
        assert_eq!(deserialized.end_line, params.end_line);
    }

    #[tokio::test]
    async fn test_execute_with_invalid_parameters() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let invalid_params = json!({
            "file_path": "src/main.rs"
            // missing repository_name
        });
        
        let result = tool.execute(invalid_params).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("missing field `repository_name`"));
    }

    #[tokio::test]
    async fn test_execute_with_missing_repository_name() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "file_path": "src/main.rs"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("missing field `repository_name`"));
    }

    #[tokio::test]
    async fn test_execute_with_missing_file_path() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("missing field `file_path`"));
    }

    #[tokio::test]
    async fn test_execute_with_valid_params() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo",
            "file_path": "src/main.rs"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error since test-repo doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository 'test-repo' not found"));
    }

    #[tokio::test]
    async fn test_execute_with_line_range() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo",
            "file_path": "src/main.rs",
            "start_line": 1,
            "end_line": 50
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error since test-repo doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository 'test-repo' not found"));
    }

    #[tokio::test]
    async fn test_execute_with_only_start_line() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo",
            "file_path": "src/main.rs",
            "start_line": 10
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error since test-repo doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository 'test-repo' not found"));
    }

    #[tokio::test]
    async fn test_empty_file_path_parameter() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo",
            "file_path": ""
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error since test-repo doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository 'test-repo' not found"));
    }

    #[tokio::test]
    async fn test_parameter_validation_edge_cases() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "",
            "file_path": "src/main.rs"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error for empty repository name
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository '' not found"));
    }

    #[tokio::test]
    async fn test_invalid_line_numbers() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo",
            "file_path": "src/main.rs",
            "start_line": 50,
            "end_line": 10  // end_line < start_line
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error since test-repo doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository 'test-repo' not found"));
    }

    #[tokio::test]
    async fn test_zero_line_numbers() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo",
            "file_path": "src/main.rs",
            "start_line": 0,
            "end_line": 10
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error since test-repo doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository 'test-repo' not found"));
    }

    #[tokio::test]
    async fn test_negative_line_numbers() {
        let tool = ViewFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo",
            "file_path": "src/main.rs",
            "start_line": -1,
            "end_line": 10
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get parsing error for negative line numbers (u32 type)
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("invalid value: integer `-1`, expected u32"));
    }
} 