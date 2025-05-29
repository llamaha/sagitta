// Add repository tool will go here

use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::FredAgentError;

/// Parameters for adding a repository
#[derive(Debug, Deserialize, Serialize)]
pub struct AddRepositoryParams {
    /// Name for the repository
    pub name: String,
    /// Git URL for the repository
    pub url: Option<String>,
    /// Branch to use (optional, defaults to main/master)
    pub branch: Option<String>,
    /// Local path to an existing repository (alternative to URL)
    pub local_path: Option<String>,
}

/// Tool for adding repositories to the management system
#[derive(Debug)]
pub struct AddRepositoryTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl AddRepositoryTool {
    /// Create a new add repository tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
    
    /// Add a repository
    async fn add_repository(&self, params: &AddRepositoryParams) -> Result<String, FredAgentError> {
        let mut repo_manager = self.repo_manager.lock().await;
        
        if let Some(local_path) = &params.local_path {
            // Add local repository
            match repo_manager.add_local_repository(&params.name, local_path).await {
                Ok(_) => Ok(format!("Successfully added local repository '{}' from path: {}", params.name, local_path)),
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("already exists in configuration") {
                        // Treat "already exists" as success since the repository is available
                        Ok(format!("Repository '{}' already exists and is available for use", params.name))
                    } else {
                        Err(FredAgentError::ToolError(format!("Failed to add local repository: {}", e)))
                    }
                }
            }
        } else if let Some(url) = &params.url {
            // Add remote repository
            let branch = params.branch.as_deref();
            match repo_manager.add_repository(&params.name, url, branch).await {
                Ok(_) => {
            let branch_msg = if let Some(b) = branch {
                format!(" (branch: {})", b)
            } else {
                String::new()
            };
            Ok(format!("Successfully added repository '{}' from URL: {}{}", params.name, url, branch_msg))
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("already exists in configuration") {
                        // Treat "already exists" as success since the repository is available
                        Ok(format!("Repository '{}' already exists and is available for use", params.name))
                    } else {
                        Err(FredAgentError::ToolError(format!("Failed to add repository: {}", e)))
                    }
                }
            }
        } else {
            Err(FredAgentError::ToolError("Either URL or local_path must be provided".to_string()))
        }
    }
}

#[async_trait]
impl Tool for AddRepositoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "add_repository".to_string(),
            description: "Add a new repository to the management system. Can add either a remote Git repository by URL or a local repository by path.".to_string(),
            category: ToolCategory::Repository,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Unique name for the repository"
                    },
                    "url": {
                        "type": "string",
                        "description": "Git URL for remote repository (HTTPS or SSH)"
                    },
                    "branch": {
                        "type": "string",
                        "description": "Optional branch to checkout (defaults to main/master)"
                    },
                    "local_path": {
                        "type": "string",
                        "description": "Path to existing local repository (alternative to URL)"
                    }
                },
                "required": ["name"],
                "oneOf": [
                    {"required": ["url"]},
                    {"required": ["local_path"]}
                ]
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: Value) -> Result<ToolResult, FredAgentError> {
        match serde_json::from_value::<AddRepositoryParams>(parameters) {
            Ok(params) => {
                match self.add_repository(&params).await {
                    Ok(message) => Ok(ToolResult::Success(serde_json::json!({
                        "success": true,
                        "message": message,
                        "repository_name": params.name
                    }))),
                    Err(e) => Ok(ToolResult::Error {
                        error: format!("Add repository failed: {}", e),
                    })
                }
            },
            Err(e) => Ok(ToolResult::Error {
                error: format!("Invalid parameters for add_repository: {}", e),
            })
        }
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
    async fn test_add_repository_tool_creation() {
        let repo_manager = create_test_repo_manager();
        let tool = AddRepositoryTool::new(repo_manager);
        
        assert!(format!("{:?}", tool).contains("AddRepositoryTool"));
    }

    #[tokio::test]
    async fn test_tool_definition() {
        let repo_manager = create_test_repo_manager();
        let tool = AddRepositoryTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "add_repository");
        assert!(definition.description.contains("Add a new repository"));
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(!definition.is_required);
        
        // Check parameters structure
        let params = definition.parameters;
        assert!(params.get("type").is_some());
        assert!(params.get("properties").is_some());
        assert!(params.get("required").is_some());
    }

    #[tokio::test]
    async fn test_execute_with_invalid_parameters() {
        let repo_manager = create_test_repo_manager();
        let tool = AddRepositoryTool::new(repo_manager);
        
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
    async fn test_execute_with_missing_url_and_path() {
        let repo_manager = create_test_repo_manager();
        let tool = AddRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "test-repo"
        });
        
        let result = tool.execute(params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("Either URL or local_path must be provided"));
            }
            _ => panic!("Expected error result"),
        }
    }

    #[tokio::test]
    async fn test_execute_with_valid_local_path_params() {
        let repo_manager = create_test_repo_manager();
        let tool = AddRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "test-repo",
            "local_path": "/tmp/test-repo"
        });
        
        // This will fail because the repository manager isn't fully initialized
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
    async fn test_execute_with_valid_url_params() {
        let repo_manager = create_test_repo_manager();
        let tool = AddRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "test-repo",
            "url": "https://github.com/test/repo.git",
            "branch": "main"
        });
        
        // This will fail because the repository manager isn't fully initialized
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
    async fn test_add_repository_params_serialization() {
        let params = AddRepositoryParams {
            name: "test-repo".to_string(),
            url: Some("https://github.com/test/repo.git".to_string()),
            branch: Some("main".to_string()),
            local_path: None,
        };
        
        let serialized = serde_json::to_string(&params).unwrap();
        let deserialized: AddRepositoryParams = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(params.name, deserialized.name);
        assert_eq!(params.url, deserialized.url);
        assert_eq!(params.branch, deserialized.branch);
        assert_eq!(params.local_path, deserialized.local_path);
    }

    #[tokio::test]
    async fn test_add_repository_params_with_local_path() {
        let params = AddRepositoryParams {
            name: "local-repo".to_string(),
            url: None,
            branch: None,
            local_path: Some("/path/to/repo".to_string()),
        };
        
        let json_value = serde_json::to_value(&params).unwrap();
        let parsed: AddRepositoryParams = serde_json::from_value(json_value).unwrap();
        
        assert_eq!(params.name, parsed.name);
        assert_eq!(params.local_path, parsed.local_path);
        assert!(parsed.url.is_none());
        assert!(parsed.branch.is_none());
    }

    #[tokio::test]
    async fn test_parameter_validation_edge_cases() {
        let repo_manager = create_test_repo_manager();
        let tool = AddRepositoryTool::new(repo_manager);
        
        // Test with both URL and local_path (should still work, but local_path takes precedence)
        let params = json!({
            "name": "test-repo",
            "url": "https://github.com/test/repo.git",
            "local_path": "/tmp/test-repo"
        });
        
        let result = tool.execute(params).await.unwrap();
        // Should not fail due to parameter validation
        match result {
            ToolResult::Error { error } => {
                assert!(!error.contains("Invalid parameters"));
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_empty_name_parameter() {
        let repo_manager = create_test_repo_manager();
        let tool = AddRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "",
            "url": "https://github.com/test/repo.git"
        });
        
        // Empty name should be handled by the repository manager, not parameter validation
        let result = tool.execute(params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                assert!(!error.contains("Invalid parameters"));
            }
            _ => {}
        }
    }
}

