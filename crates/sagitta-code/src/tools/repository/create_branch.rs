use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Parameters for creating a new branch
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateBranchParams {
    /// Name of the repository to create branch in
    pub repository_name: String,
    /// Name for the new branch
    pub branch_name: String,
    /// Starting point for the branch (commit hash, branch name, or tag). If not provided, uses current HEAD
    pub start_point: Option<String>,
    /// Whether to switch to the new branch after creation
    #[serde(default)]
    pub checkout: bool,
}

/// Tool for creating branches in repositories
#[derive(Debug)]
pub struct CreateBranchTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl CreateBranchTool {
    /// Create a new create branch tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self { repo_manager }
    }
}

#[async_trait]
impl Tool for CreateBranchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "create_branch".to_string(),
            description: "Create a new git branch in a repository".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Name of the repository to create branch in"
                    },
                    "branch_name": {
                        "type": "string",
                        "description": "Name for the new branch"
                    },
                    "start_point": {
                        "type": "string",
                        "description": "Starting point for the branch (commit hash, branch name, or tag). If not provided, uses current HEAD"
                    },
                    "checkout": {
                        "type": "boolean",
                        "description": "Whether to switch to the new branch after creation (default: false)"
                    }
                },
                "required": ["repository_name", "branch_name"]
            }),
            category: ToolCategory::Repository,
            is_required: false,
            metadata: std::collections::HashMap::new(),
        }
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let params: CreateBranchParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;

        // Get repository manager
        let mut repo_manager = self.repo_manager.lock().await;

        // Check if repository exists
        let repositories = repo_manager.list_repositories().await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to list repositories: {}", e)))?;
        
        let repo_config = repositories.iter()
            .find(|r| r.name == params.repository_name);
        
        let repo_config = match repo_config {
            Some(config) => config,
            None => {
                return Ok(ToolResult::Error { 
                    error: format!("Repository '{}' not found", params.repository_name)
                });
            }
        };

        // Create the branch
        match repo_manager.create_branch(&params.repository_name, &params.branch_name, params.checkout).await {
            Ok(_) => {
                let message = if params.checkout {
                    format!("Successfully created and switched to branch '{}' in repository '{}'", 
                           params.branch_name, params.repository_name)
                } else {
                    format!("Successfully created branch '{}' in repository '{}'", 
                           params.branch_name, params.repository_name)
                };

                Ok(ToolResult::Success(serde_json::json!({
                    "message": message,
                    "repository_name": params.repository_name,
                    "branch_name": params.branch_name,
                    "checked_out": params.checkout
                })))
            }
            Err(e) => {
                let error_msg = format!("Failed to create branch '{}' in repository '{}': {}", 
                                       params.branch_name, params.repository_name, e);
                Ok(ToolResult::Error { error: error_msg })
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
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use crate::gui::repository::manager::RepositoryManager;
    use crate::tools::types::ToolResult;
    use sagitta_search::config::AppConfig;
    use serde_json::json;

    #[test]
    fn test_create_branch_tool_creation() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = CreateBranchTool::new(repo_manager);
        
        // Should be able to create the tool
        assert_eq!(std::mem::size_of_val(&tool), std::mem::size_of::<CreateBranchTool>());
    }

    #[test]
    fn test_tool_definition() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = CreateBranchTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "create_branch");
        assert!(!definition.description.is_empty());
        assert!(definition.parameters.is_object());
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(!definition.is_required);
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = CreateBranchTool::new(repo_manager);

        // Test with missing required field
        let invalid_params = json!({
            "repository_name": "test-repo"
            // Missing branch_name
        });

        let result = tool.execute(invalid_params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid parameters"));
    }

    #[tokio::test]
    async fn test_nonexistent_repository() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = CreateBranchTool::new(repo_manager);

        let params = json!({
            "repository_name": "nonexistent-repo",
            "branch_name": "new-branch"
        });

        let result = tool.execute(params).await;
        assert!(result.is_ok());
        if let Ok(ToolResult::Error { error }) = result {
            assert!(error.contains("Repository 'nonexistent-repo' not found"));
        } else {
            panic!("Expected error result");
        }
    }
} 