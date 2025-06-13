// List repositories tool will go here

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Tool for listing repositories
#[derive(Debug)]
pub struct ListRepositoriesTool {
    /// Repository manager for accessing repositories
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl ListRepositoriesTool {
    /// Create a new list repositories tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
}

#[async_trait]
impl Tool for ListRepositoriesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_repositories".to_string(),
            description: "List all repositories currently registered in the management system".to_string(),
            category: ToolCategory::Repository,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": [],
                "properties": {}
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, _parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let repo_manager = self.repo_manager.lock().await;
        match repo_manager.list_repositories().await {
            Ok(repositories) => {
                let total_count = repositories.len();
                Ok(ToolResult::Success(serde_json::json!({
                    "repositories": repositories,
                    "total_count": total_count
                })))
            }
            Err(e) => Ok(ToolResult::Error { 
                error: format!("Failed to list repositories: {}", e)
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
    async fn test_list_repositories_tool_creation() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        
        assert!(format!("{:?}", tool).contains("ListRepositoriesTool"));
    }

    #[tokio::test]
    async fn test_tool_definition() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "list_repositories");
        assert!(definition.description.contains("List all repositories currently registered in the management system"));
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(!definition.is_required);
        
        // Check parameters structure - should be empty object
        let params = definition.parameters;
        assert!(params.get("type").is_some());
        assert!(params.get("properties").is_some());
        
        // Properties should be empty for list operation
        let properties = params.get("properties").unwrap();
        assert!(properties.as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_execute_with_empty_parameters() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        
        // Test with empty parameters (should work fine)
        let result = tool.execute(json!({})).await.unwrap();
        match result {
            ToolResult::Success(data) => {
                assert!(data.get("repositories").is_some());
                assert!(data.get("total_count").is_some());
                
                // Should be empty list since no repositories are configured
                let repos = data.get("repositories").unwrap().as_array().unwrap();
                assert_eq!(repos.len(), 0);
                
                let count = data.get("total_count").unwrap().as_u64().unwrap();
                assert_eq!(count, 0);
            }
            ToolResult::Error { error } => {
                // This might fail due to uninitialized repository manager
                // but should not be a parameter error
                assert!(!error.contains("Invalid parameters"));
            }
        }
    }

    #[tokio::test]
    async fn test_execute_with_any_parameters() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        
        // Test with random parameters (should ignore them)
        let result = tool.execute(json!({
            "random_param": "value",
            "another_param": 123
        })).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                assert!(data.get("repositories").is_some());
                assert!(data.get("total_count").is_some());
            }
            ToolResult::Error { error } => {
                // This might fail due to uninitialized repository manager
                // but should not be a parameter error
                assert!(!error.contains("Invalid parameters"));
            }
        }
    }

    #[tokio::test]
    async fn test_execute_with_null_parameters() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        
        // Test with null value
        let result = tool.execute(Value::Null).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                assert!(data.get("repositories").is_some());
                assert!(data.get("total_count").is_some());
            }
            ToolResult::Error { error } => {
                // This might fail due to uninitialized repository manager
                // but should not be a parameter error
                assert!(!error.contains("Invalid parameters"));
            }
        }
    }

    #[tokio::test]
    async fn test_tool_category_and_metadata() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(definition.metadata.is_empty());
        assert!(!definition.is_required);
    }

    #[tokio::test]
    async fn test_tool_name_consistency() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        let definition = tool.definition();
        
        // Ensure the tool name follows the expected pattern
        assert_eq!(definition.name, "list_repositories");
        assert!(definition.name.contains("list"));
        assert!(definition.name.contains("repositories"));
    }

    #[tokio::test]
    async fn test_parameters_schema_structure() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        let definition = tool.definition();
        
        let params = definition.parameters;
        
        // Should be a valid JSON schema object
        assert_eq!(params.get("type").unwrap().as_str().unwrap(), "object");
        
        let properties = params.get("properties").unwrap().as_object().unwrap();
        assert!(properties.is_empty(), "List tool should not require any parameters");
    }

    #[tokio::test]
    async fn test_description_content() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        let definition = tool.definition();
        
        let description = &definition.description;
        assert!(description.len() > 10, "Description should be meaningful");
        assert!(description.to_lowercase().contains("list"));
        assert!(description.to_lowercase().contains("repositories"));
    }
}

