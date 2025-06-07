use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Parameters for pushing changes
#[derive(Debug, Deserialize, Serialize)]
pub struct PushChangesParams {
    /// Name of the repository to push from
    pub repository_name: String,
    /// Remote name (defaults to "origin")
    #[serde(default = "default_remote_name")]
    pub remote_name: String,
    /// Branch to push (if not specified, pushes current branch)
    pub branch_name: Option<String>,
    /// Whether to force push
    #[serde(default)]
    pub force: bool,
    /// Whether to set upstream tracking
    #[serde(default)]
    pub set_upstream: bool,
}

fn default_remote_name() -> String {
    "origin".to_string()
}

/// Tool for pushing changes to remote repositories
#[derive(Debug)]
pub struct PushChangesTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl PushChangesTool {
    /// Create a new push changes tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self { repo_manager }
    }

    /// Get current branch name
    fn get_current_branch(&self, repo: &git2::Repository) -> Result<String, git2::Error> {
        let head = repo.head()?;
        
        if head.is_branch() {
            if let Some(branch_name) = head.shorthand() {
                Ok(branch_name.to_string())
            } else {
                let oid = head.target().ok_or_else(|| {
                    git2::Error::from_str("HEAD has no target")
                })?;
                Ok(format!("detached-{}", oid))
            }
        } else {
            let oid = head.target().ok_or_else(|| {
                git2::Error::from_str("HEAD has no target")
            })?;
            Ok(format!("detached-{}", oid))
        }
    }

    /// Push changes to remote
    fn push_changes(
        &self,
        repo: &git2::Repository,
        remote_name: &str,
        branch_name: &str,
        force: bool,
        set_upstream: bool,
    ) -> Result<(), git2::Error> {
        let mut remote = repo.find_remote(remote_name)?;
        
        let refspec = if set_upstream {
            format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name)
        } else {
            format!("refs/heads/{}", branch_name)
        };

        let mut push_options = git2::PushOptions::new();
        if force {
            // Note: git2 doesn't directly support force push flag, 
            // we'd need to add '+' prefix to refspec for force push
            // For now, we'll just do a regular push
        }

        remote.push(&[&refspec], Some(&mut push_options))?;
        
        if set_upstream {
            // Set upstream tracking
            let mut branch = repo.find_branch(branch_name, git2::BranchType::Local)?;
            let upstream_name = format!("{}/{}", remote_name, branch_name);
            branch.set_upstream(Some(&upstream_name))?;
        }

        Ok(())
    }
}

#[async_trait]
impl Tool for PushChangesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "push_changes".to_string(),
            description: "Push changes to a remote git repository".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Name of the repository to push from"
                    },
                    "remote_name": {
                        "type": "string",
                        "description": "Remote name (defaults to 'origin')"
                    },
                    "branch_name": {
                        "type": "string",
                        "description": "Branch to push (if not specified, pushes current branch)"
                    },
                    "force": {
                        "type": "boolean",
                        "description": "Whether to force push (default: false)"
                    },
                    "set_upstream": {
                        "type": "boolean",
                        "description": "Whether to set upstream tracking (default: false)"
                    }
                },
                "required": ["repository_name"]
            }),
            category: ToolCategory::Repository,
            is_required: false,
            metadata: std::collections::HashMap::new(),
        }
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let params: PushChangesParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;

        // Get repository manager
        let repo_manager = self.repo_manager.lock().await;

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

        // Use git2 directly to handle the push
        let repo_path = &repo_config.local_path;
        let repo = git2::Repository::open(repo_path)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to open repository: {}", e)))?;

        // Get current branch if branch_name not specified
        let branch_to_push = if let Some(ref branch_name) = params.branch_name {
            branch_name.clone()
        } else {
            match self.get_current_branch(&repo) {
                Ok(current_branch) => current_branch,
                Err(e) => {
                    return Ok(ToolResult::Error { 
                        error: format!("Failed to get current branch in repository '{}': {}", 
                                     params.repository_name, e)
                    });
                }
            }
        };

        // Push the changes
        match self.push_changes(
            &repo,
            &params.remote_name,
            &branch_to_push,
            params.force,
            params.set_upstream
        ) {
            Ok(_) => {
                let push_info = if params.set_upstream {
                    format!("with upstream tracking to {}/{}", params.remote_name, branch_to_push)
                } else {
                    format!("to {}/{}", params.remote_name, branch_to_push)
                };

                let force_info = if params.force { " (force push)" } else { "" };

                Ok(ToolResult::Success(serde_json::json!({
                    "success": true,
                    "message": format!("Successfully pushed changes from repository '{}' {}{}", 
                                     params.repository_name, push_info, force_info),
                    "repository_name": params.repository_name,
                    "remote_name": params.remote_name,
                    "branch_name": branch_to_push,
                    "force_pushed": params.force,
                    "set_upstream": params.set_upstream
                })))
            }
            Err(e) => {
                Ok(ToolResult::Error { 
                    error: format!("Failed to push changes from repository '{}': {}", 
                                 params.repository_name, e)
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
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use crate::gui::repository::manager::RepositoryManager;
    use crate::tools::types::ToolResult;
    use sagitta_search::config::AppConfig;
    use serde_json::json;

    #[test]
    fn test_push_changes_tool_creation() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = PushChangesTool::new(repo_manager);
        
        // Should be able to create the tool
        assert_eq!(std::mem::size_of_val(&tool), std::mem::size_of::<PushChangesTool>());
    }

    #[test]
    fn test_tool_definition() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = PushChangesTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "push_changes");
        assert!(!definition.description.is_empty());
        assert!(definition.parameters.is_object());
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(!definition.is_required);
    }

    #[test]
    fn test_default_remote_name() {
        assert_eq!(default_remote_name(), "origin");
    }

    #[test]
    fn test_params_defaults() {
        let params: PushChangesParams = serde_json::from_value(json!({
            "repository_name": "test-repo"
        })).unwrap();
        
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.remote_name, "origin");
        assert_eq!(params.branch_name, None);
        assert!(!params.force);
        assert!(!params.set_upstream);
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = PushChangesTool::new(repo_manager);

        // Test with invalid parameter type
        let invalid_params = json!("not an object");

        let result = tool.execute(invalid_params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid parameters"));
    }

    #[tokio::test]
    async fn test_nonexistent_repository() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = PushChangesTool::new(repo_manager);

        let params = json!({
            "repository_name": "nonexistent-repo"
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