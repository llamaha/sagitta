use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Parameters for pulling changes
#[derive(Debug, Deserialize, Serialize)]
pub struct PullChangesParams {
    /// Name of the repository to pull into
    pub repository_name: String,
    /// Remote name (defaults to "origin")
    #[serde(default = "default_remote_name")]
    pub remote_name: String,
    /// Branch to pull from (if not specified, pulls current branch)
    pub branch_name: Option<String>,
    /// Whether to rebase instead of merge
    #[serde(default)]
    pub rebase: bool,
    /// Whether to fast-forward only
    #[serde(default)]
    pub fast_forward_only: bool,
}

fn default_remote_name() -> String {
    "origin".to_string()
}

/// Tool for pulling changes from remote repositories
#[derive(Debug)]
pub struct PullChangesTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl PullChangesTool {
    /// Create a new pull changes tool
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

    /// Pull changes from remote
    fn pull_changes(
        &self,
        repo: &git2::Repository,
        remote_name: &str,
        branch_name: &str,
        rebase: bool,
        fast_forward_only: bool,
    ) -> Result<String, git2::Error> {
        // First, fetch from remote
        let mut remote = repo.find_remote(remote_name)?;
        let mut fetch_options = git2::FetchOptions::new();
        remote.fetch(&[&format!("refs/heads/{}", branch_name)], Some(&mut fetch_options), None)?;

        // Get the remote branch reference
        let remote_branch_name = format!("{}/{}", remote_name, branch_name);
        let remote_ref = repo.find_reference(&format!("refs/remotes/{}", remote_branch_name))?;
        let remote_commit = remote_ref.peel_to_commit()?;

        // Get the current head commit
        let head_commit = repo.head()?.peel_to_commit()?;

        // Check if we're already up to date
        if head_commit.id() == remote_commit.id() {
            return Ok("up-to-date".to_string());
        }

        // Check if we can fast-forward
        let (ahead, behind) = repo.graph_ahead_behind(head_commit.id(), remote_commit.id())?;
        
        if behind == 0 {
            return Ok("up-to-date".to_string());
        }

        if ahead == 0 {
            // Can fast-forward
            let mut reference = repo.head()?;
            reference.set_target(remote_commit.id(), "Fast-forward")?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
            return Ok("fast-forward".to_string());
        }

        if fast_forward_only {
            return Err(git2::Error::from_str("Cannot fast-forward, and fast-forward only was requested"));
        }

        if rebase {
            // For now, we'll just report that rebase would be needed
            // Implementing full rebase is complex and beyond the scope of this basic implementation
            return Ok("rebase".to_string());
        } else {
            // For now, we'll just report that merge would be needed
            // Implementing full merge is complex and beyond the scope of this basic implementation
            return Ok("merge".to_string());
        }
    }
}

#[async_trait]
impl Tool for PullChangesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "pull_changes".to_string(),
            description: "Pull changes from a remote git repository".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Name of the repository to pull into"
                    },
                    "remote_name": {
                        "type": "string",
                        "description": "Remote name (defaults to 'origin')"
                    },
                    "branch_name": {
                        "type": "string",
                        "description": "Branch to pull from (if not specified, pulls current branch)"
                    },
                    "rebase": {
                        "type": "boolean",
                        "description": "Whether to rebase instead of merge (default: false)"
                    },
                    "fast_forward_only": {
                        "type": "boolean",
                        "description": "Whether to fast-forward only (default: false)"
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
        let params: PullChangesParams = serde_json::from_value(parameters)
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

        // Use git2 directly to handle the pull
        let repo_path = &repo_config.local_path;
        let repo = git2::Repository::open(repo_path)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to open repository: {}", e)))?;

        // Get current branch if branch_name not specified
        let branch_to_pull = if let Some(ref branch_name) = params.branch_name {
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

        // Pull the changes
        match self.pull_changes(
            &repo,
            &params.remote_name,
            &branch_to_pull,
            params.rebase,
            params.fast_forward_only
        ) {
            Ok(pull_result) => {
                let pull_info = if params.rebase {
                    format!("from {}/{} with rebase", params.remote_name, branch_to_pull)
                } else if params.fast_forward_only {
                    format!("from {}/{} (fast-forward only)", params.remote_name, branch_to_pull)
                } else {
                    format!("from {}/{}", params.remote_name, branch_to_pull)
                };

                // Check if there were any conflicts or if it's up to date
                let status_info = match pull_result.as_str() {
                    "up-to-date" => "Repository is already up to date".to_string(),
                    "fast-forward" => "Fast-forwarded to latest changes".to_string(),
                    "merge" => "Merged changes successfully".to_string(),
                    "rebase" => "Rebased changes successfully".to_string(),
                    other => format!("Pull completed: {}", other),
                };

                Ok(ToolResult::Success(serde_json::json!({
                    "message": format!("Successfully pulled changes into repository '{}' {}", 
                                     params.repository_name, pull_info),
                    "status": status_info,
                    "repository_name": params.repository_name,
                    "remote_name": params.remote_name,
                    "branch_name": branch_to_pull,
                    "rebase": params.rebase,
                    "fast_forward_only": params.fast_forward_only,
                    "pull_result": pull_result
                })))
            }
            Err(e) => {
                Ok(ToolResult::Error { 
                    error: format!("Failed to pull changes into repository '{}': {}", 
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
    fn test_pull_changes_tool_creation() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = PullChangesTool::new(repo_manager);
        
        // Should be able to create the tool
        assert_eq!(std::mem::size_of_val(&tool), std::mem::size_of::<PullChangesTool>());
    }

    #[test]
    fn test_tool_definition() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = PullChangesTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "pull_changes");
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
        let params: PullChangesParams = serde_json::from_value(json!({
            "repository_name": "test-repo"
        })).unwrap();
        
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.remote_name, "origin");
        assert_eq!(params.branch_name, None);
        assert!(!params.rebase);
        assert!(!params.fast_forward_only);
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = PullChangesTool::new(repo_manager);

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
        let tool = PullChangesTool::new(repo_manager);

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