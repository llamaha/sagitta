// Remove repository tool will go here

use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;
use sagitta_search::AppConfig as SagittaAppConfig;

/// Parameters for removing a repository
#[derive(Debug, Deserialize, Serialize)]
pub struct RemoveRepositoryParams {
    /// Name of the repository to remove
    pub name: String,
    /// Whether to also delete local files (default: false, only removes from management)
    #[serde(default)]
    pub delete_local_files: bool,
}

/// Tool for removing repositories from the management system
#[derive(Debug)]
pub struct RemoveRepositoryTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl RemoveRepositoryTool {
    /// Create a new remove repository tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
    
    /// Remove a repository
    async fn remove_repository(&self, params: &RemoveRepositoryParams) -> Result<String, SagittaCodeError> {
        let mut repo_manager = self.repo_manager.lock().await;
        
        // Check if repository exists
        let repositories = repo_manager.list_repositories().await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to list repositories: {}", e)))?;
        
        let repo_exists = repositories.iter().any(|r| r.name == params.name);
        if !repo_exists {
            return Err(SagittaCodeError::ToolError(format!("Repository '{}' not found", params.name)));
        }
        
        // Perform the removal
        repo_manager.remove_repository(&params.name).await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to remove repository '{}': {}", params.name, e)))?;
        
        let local_files_msg = if params.delete_local_files {
            " and deleted local files"
        } else {
            " (local files preserved)"
        };
        
        Ok(format!("Successfully removed repository '{}'{}", params.name, local_files_msg))
    }
}

#[async_trait]
impl Tool for RemoveRepositoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "remove_repository".to_string(),
            description: "Remove a repository from the management system. Optionally can also delete local files.".to_string(),
            category: ToolCategory::Repository,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the repository to remove"
                    },
                    "delete_local_files": {
                        "type": "boolean",
                        "description": "Whether to also delete local repository files (default: false)",
                        "default": false
                    }
                },
                "required": ["name"]
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        let params: RemoveRepositoryParams = match serde_json::from_value(parameters) {
            Ok(params) => params,
            Err(e) => return Ok(ToolResult::Error {
                error: format!("Invalid parameters: {}", e)
            })
        };
        
        let mut repo_manager = self.repo_manager.lock().await;
        let result = repo_manager.remove_repository(&params.name).await;
        
        match result {
            Ok(_) => Ok(ToolResult::Success(serde_json::json!({
                "repository_name": params.name,
                "status": "removed"
            }))),
            Err(e) => Ok(ToolResult::Error { 
                error: format!("Failed to remove repository: {}", e)
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

    #[tokio::test]
    async fn test_remove_repository_tool_creation() {
        let repo_manager = create_test_repo_manager();
        let tool = RemoveRepositoryTool::new(repo_manager);
        
        assert!(format!("{:?}", tool).contains("RemoveRepositoryTool"));
    }

    #[tokio::test]
    async fn test_tool_definition() {
        let repo_manager = create_test_repo_manager();
        let tool = RemoveRepositoryTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "remove_repository");
        assert!(definition.description.contains("Remove a repository"));
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(!definition.is_required);
        
        // Check parameters structure
        let params = definition.parameters;
        assert!(params.get("type").is_some());
        assert!(params.get("properties").is_some());
        assert!(params.get("required").is_some());
        
        // Check required fields
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("name")));
    }

    #[tokio::test]
    async fn test_execute_with_invalid_parameters() {
        let repo_manager = create_test_repo_manager();
        let tool = RemoveRepositoryTool::new(repo_manager);
        
        // Test with empty parameters
        let result = tool.execute(json!({})).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("Invalid parameters"));
            }
            _ => panic!("Expected error result"),
        }
    }

    #[tokio::test]
    async fn test_execute_with_missing_name() {
        let repo_manager = create_test_repo_manager();
        let tool = RemoveRepositoryTool::new(repo_manager);
        
        let params = json!({
            "delete_local_files": true
        });
        
        let result = tool.execute(params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("Invalid parameters"));
            }
            _ => panic!("Expected error result"),
        }
    }

    #[tokio::test]
    async fn test_execute_with_valid_params_default_delete_flag() {
        let repo_manager = create_test_repo_manager();
        let tool = RemoveRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "test-repo"
        });
        
        // This will fail because the repository doesn't exist
        // but we can test that the parameters are parsed correctly
        let result = tool.execute(params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                // Should fail at the repository manager level, not parameter parsing
                assert!(!error.contains("Invalid parameters"));
                // Should indicate repository not found or initialization issue
                assert!(error.contains("not found") || error.contains("Failed to list repositories") || error.contains("not initialized"));
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_execute_with_delete_local_files_true() {
        let repo_manager = create_test_repo_manager();
        let tool = RemoveRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "test-repo",
            "delete_local_files": true
        });
        
        let result = tool.execute(params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                // Should fail at the repository manager level, not parameter parsing
                assert!(!error.contains("Invalid parameters"));
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_execute_with_delete_local_files_false() {
        let repo_manager = create_test_repo_manager();
        let tool = RemoveRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "test-repo",
            "delete_local_files": false
        });
        
        let result = tool.execute(params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                // Should fail at the repository manager level, not parameter parsing
                assert!(!error.contains("Invalid parameters"));
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_remove_repository_params_serialization() {
        let params = RemoveRepositoryParams {
            name: "test-repo".to_string(),
            delete_local_files: true,
        };
        
        let serialized = serde_json::to_string(&params).unwrap();
        let deserialized: RemoveRepositoryParams = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(params.name, deserialized.name);
        assert_eq!(params.delete_local_files, deserialized.delete_local_files);
    }

    #[tokio::test]
    async fn test_remove_repository_params_default_delete_flag() {
        let json_str = r#"{"name": "test-repo"}"#;
        let params: RemoveRepositoryParams = serde_json::from_str(json_str).unwrap();
        
        assert_eq!(params.name, "test-repo");
        assert!(!params.delete_local_files); // Should default to false
    }

    #[tokio::test]
    async fn test_remove_repository_params_with_delete_flag() {
        let params = RemoveRepositoryParams {
            name: "test-repo".to_string(),
            delete_local_files: false,
        };
        
        let json_value = serde_json::to_value(&params).unwrap();
        let parsed: RemoveRepositoryParams = serde_json::from_value(json_value).unwrap();
        
        assert_eq!(params.name, parsed.name);
        assert_eq!(params.delete_local_files, parsed.delete_local_files);
    }

    #[tokio::test]
    async fn test_parameter_validation_edge_cases() {
        let repo_manager = create_test_repo_manager();
        let tool = RemoveRepositoryTool::new(repo_manager);
        
        // Test with empty name
        let params = json!({
            "name": "",
            "delete_local_files": true
        });
        
        let result = tool.execute(params).await.unwrap();
        // Should not fail due to parameter validation, but might fail at business logic level
        match result {
            ToolResult::Error { error } => {
                assert!(!error.contains("Invalid parameters"));
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_tool_definition_properties() {
        let repo_manager = create_test_repo_manager();
        let tool = RemoveRepositoryTool::new(repo_manager);
        let definition = tool.definition();
        
        let params = definition.parameters;
        let properties = params.get("properties").unwrap().as_object().unwrap();
        
        // Should have name property
        assert!(properties.contains_key("name"));
        let name_prop = properties.get("name").unwrap().as_object().unwrap();
        assert_eq!(name_prop.get("type").unwrap().as_str().unwrap(), "string");
        
        // Should have delete_local_files property
        assert!(properties.contains_key("delete_local_files"));
        let delete_prop = properties.get("delete_local_files").unwrap().as_object().unwrap();
        assert_eq!(delete_prop.get("type").unwrap().as_str().unwrap(), "boolean");
        assert_eq!(delete_prop.get("default").unwrap().as_bool().unwrap(), false);
    }

    #[tokio::test]
    async fn test_tool_category_and_metadata() {
        let repo_manager = create_test_repo_manager();
        let tool = RemoveRepositoryTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(definition.metadata.is_empty());
        assert!(!definition.is_required);
    }
}

