// Sync repository tool will go here

use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;
use sagitta_search::AppConfig as SagittaAppConfig;

/// Parameters for syncing a repository
#[derive(Debug, Deserialize, Serialize)]
pub struct SyncRepositoryParams {
    /// Name of the repository to sync
    pub name: String,
    /// Whether to force sync even if no changes detected
    #[serde(default)]
    pub force: bool,
}

/// Tool for syncing repositories in the management system
#[derive(Debug)]
pub struct SyncRepositoryTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl SyncRepositoryTool {
    /// Create a new sync repository tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
    
    /// Sync a repository
    async fn sync_repository(&self, params: &SyncRepositoryParams) -> Result<String, SagittaCodeError> {
        let mut repo_manager = self.repo_manager.lock().await;
        
        // Check if repository exists
        let repositories = repo_manager.list_repositories().await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to list repositories: {}", e)))?;
        
        let repo_exists = repositories.iter().any(|r| r.name == params.name);
        if !repo_exists {
            return Err(SagittaCodeError::ToolError(format!("Repository '{}' not found", params.name)));
        }
        
        // Perform the sync
        repo_manager.sync_repository(&params.name).await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to sync repository '{}': {}", params.name, e)))?;
        
        Ok(format!("Successfully synced repository '{}'", params.name))
    }
}

#[async_trait]
impl Tool for SyncRepositoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "sync_repository".to_string(),
            description: "Sync a repository in the management system.".to_string(),
            category: ToolCategory::Repository,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["name"],
                "properties": {
                    "name": { 
                        "type": "string", 
                        "description": "Name of the repository to sync" 
                    },
                    "force": { 
                        "type": ["boolean", "null"], 
                        "description": "Force sync even if no changes detected (defaults to false if null)" 
                    }
                }
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        let params: SyncRepositoryParams = match serde_json::from_value(parameters) {
            Ok(params) => params,
            Err(e) => return Ok(ToolResult::Error {
                error: format!("Invalid parameters: {}", e)
            })
        };
        
        let mut repo_manager = self.repo_manager.lock().await;
        let result = repo_manager.sync_repository(&params.name).await;
        
        match result {
            Ok(_) => Ok(ToolResult::Success(serde_json::json!({
                "repository_name": params.name,
                "status": "synced"
            }))),
            Err(e) => Ok(ToolResult::Error { 
                error: format!("Failed to sync repository: {}", e)
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
    async fn test_sync_repository_tool_creation() {
        let repo_manager = create_test_repo_manager();
        let tool = SyncRepositoryTool::new(repo_manager);
        
        assert!(format!("{:?}", tool).contains("SyncRepositoryTool"));
    }

    #[tokio::test]
    async fn test_tool_definition() {
        let repo_manager = create_test_repo_manager();
        let tool = SyncRepositoryTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "sync_repository");
        assert!(definition.description.contains("Sync a repository"));
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
        let tool = SyncRepositoryTool::new(repo_manager);
        
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
        let tool = SyncRepositoryTool::new(repo_manager);
        
        let params = json!({
            "force": true
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
    async fn test_execute_with_valid_params() {
        let repo_manager = create_test_repo_manager();
        let tool = SyncRepositoryTool::new(repo_manager);
        
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
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_execute_with_force_flag() {
        let repo_manager = create_test_repo_manager();
        let tool = SyncRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "test-repo",
            "force": true
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
    async fn test_sync_repository_params_serialization() {
        let params = SyncRepositoryParams {
            name: "test-repo".to_string(),
            force: true,
        };
        
        let serialized = serde_json::to_string(&params).unwrap();
        let deserialized: SyncRepositoryParams = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(params.name, deserialized.name);
        assert_eq!(params.force, deserialized.force);
    }

    #[tokio::test]
    async fn test_sync_repository_params_default_force() {
        let json_str = r#"{"name": "test-repo"}"#;
        let params: SyncRepositoryParams = serde_json::from_str(json_str).unwrap();
        
        assert_eq!(params.name, "test-repo");
        assert!(!params.force); // Should default to false
    }

    #[tokio::test]
    async fn test_sync_repository_params_with_force() {
        let params = SyncRepositoryParams {
            name: "test-repo".to_string(),
            force: false,
        };
        
        let json_value = serde_json::to_value(&params).unwrap();
        let parsed: SyncRepositoryParams = serde_json::from_value(json_value).unwrap();
        
        assert_eq!(params.name, parsed.name);
        assert_eq!(params.force, parsed.force);
    }

    #[tokio::test]
    async fn test_tool_definition_properties() {
        let repo_manager = create_test_repo_manager();
        let tool = SyncRepositoryTool::new(repo_manager);
        let definition = tool.definition();
        
        let params = definition.parameters;
        let properties = params.get("properties").unwrap().as_object().unwrap();
        
        // Should have name property
        assert!(properties.contains_key("name"));
        let name_prop = properties.get("name").unwrap().as_object().unwrap();
        assert_eq!(name_prop.get("type").unwrap().as_str().unwrap(), "string");
        
        // Should have force property
        assert!(properties.contains_key("force"));
        let force_prop = properties.get("force").unwrap().as_object().unwrap();
        assert_eq!(force_prop.get("type").unwrap().as_str().unwrap(), "boolean");
    }

    #[tokio::test]
    async fn test_parameter_validation_edge_cases() {
        let repo_manager = create_test_repo_manager();
        let tool = SyncRepositoryTool::new(repo_manager);
        
        // Test with empty name
        let params = json!({
            "name": "",
            "force": true
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
    async fn test_tool_category_and_metadata() {
        let repo_manager = create_test_repo_manager();
        let tool = SyncRepositoryTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(definition.metadata.is_empty());
        assert!(!definition.is_required);
    }

    #[tokio::test]
    async fn test_description_content() {
        let repo_manager = create_test_repo_manager();
        let tool = SyncRepositoryTool::new(repo_manager);
        let definition = tool.definition();
        
        let description = &definition.description;
        assert!(description.len() > 10, "Description should be meaningful");
        assert!(description.to_lowercase().contains("sync"));
        assert!(description.to_lowercase().contains("repository"));
    }
}

