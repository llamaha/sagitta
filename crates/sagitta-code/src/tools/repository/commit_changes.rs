use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Parameters for committing changes
#[derive(Debug, Deserialize, Serialize)]
pub struct CommitChangesParams {
    /// Name of the repository to commit in
    pub repository_name: String,
    /// Commit message
    pub message: String,
    /// Files to stage and commit (if empty, commits all changes)
    #[serde(default)]
    pub files: Vec<String>,
    /// Whether to stage all changes before committing
    #[serde(default = "default_stage_all")]
    pub stage_all: bool,
}

fn default_stage_all() -> bool {
    true
}

/// Tool for committing changes to repositories
#[derive(Debug)]
pub struct CommitChangesTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl CommitChangesTool {
    /// Create a new commit changes tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self { repo_manager }
    }

    /// Stage all changes in a repository
    fn stage_all_changes(&self, repo: &git2::Repository) -> Result<(), git2::Error> {
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;
        Ok(())
    }

    /// Stage a specific file in a repository
    fn stage_file(&self, repo: &git2::Repository, file_path: &str) -> Result<(), git2::Error> {
        let mut index = repo.index()?;
        index.add_path(std::path::Path::new(file_path))?;
        index.write()?;
        Ok(())
    }

    /// Commit staged changes
    fn commit_changes(&self, repo: &git2::Repository, message: &str) -> Result<String, git2::Error> {
        let signature = repo.signature()?;
        let tree_id = repo.index()?.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let parent_commit = repo.head()?.peel_to_commit()?;
        
        let commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent_commit],
        )?;
        
        Ok(commit_id.to_string())
    }
}

#[async_trait]
impl Tool for CommitChangesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "commit_changes".to_string(),
            description: "Commit changes to a git repository".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Name of the repository to commit in"
                    },
                    "message": {
                        "type": "string",
                        "description": "Commit message"
                    },
                    "files": {
                        "type": "array",
                        "items": {
                            "type": "string"
                        },
                        "description": "Files to stage and commit (if empty, commits all changes)"
                    },
                    "stage_all": {
                        "type": "boolean",
                        "description": "Whether to stage all changes before committing (default: true)"
                    }
                },
                "required": ["repository_name", "message"]
            }),
            category: ToolCategory::Repository,
            is_required: false,
            metadata: std::collections::HashMap::new(),
        }
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let params: CommitChangesParams = serde_json::from_value(parameters)
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

        // Use git2 directly to handle the commit
        let repo_path = &repo_config.local_path;
        let repo = git2::Repository::open(repo_path)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to open repository: {}", e)))?;

        // Stage files
        if params.stage_all && params.files.is_empty() {
            // Stage all changes
            if let Err(e) = self.stage_all_changes(&repo) {
                return Ok(ToolResult::Error { 
                    error: format!("Failed to stage changes in repository '{}': {}", params.repository_name, e)
                });
            }
        } else if !params.files.is_empty() {
            // Stage specific files
            for file_path in &params.files {
                if let Err(e) = self.stage_file(&repo, file_path) {
                    return Ok(ToolResult::Error { 
                        error: format!("Failed to stage file '{}' in repository '{}': {}", 
                                     file_path, params.repository_name, e)
                    });
                }
            }
        }

        // Commit the changes
        match self.commit_changes(&repo, &params.message) {
            Ok(commit_hash) => {
                let staged_info = if params.files.is_empty() && params.stage_all {
                    "all changes".to_string()
                } else if params.files.is_empty() {
                    "staged changes".to_string()
                } else {
                    format!("{} files", params.files.len())
                };

                let files_committed_value = if params.files.is_empty() { 
                    if params.stage_all { "all_changes".to_string() } else { "staged_changes".to_string() }
                } else { 
                    format!("{}", params.files.len())
                };

                Ok(ToolResult::Success(serde_json::json!({
                    "message": format!("Successfully committed {} in repository '{}'", 
                                     staged_info, params.repository_name),
                    "repository_name": params.repository_name,
                    "commit_hash": commit_hash,
                    "commit_message": params.message,
                    "files_committed": files_committed_value
                })))
            }
            Err(e) => {
                Ok(ToolResult::Error { 
                    error: format!("Failed to commit changes in repository '{}': {}", 
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
    fn test_commit_changes_tool_creation() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = CommitChangesTool::new(repo_manager);
        
        // Should be able to create the tool
        assert_eq!(std::mem::size_of_val(&tool), std::mem::size_of::<CommitChangesTool>());
    }

    #[test]
    fn test_tool_definition() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = CommitChangesTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "commit_changes");
        assert!(!definition.description.is_empty());
        assert!(definition.parameters.is_object());
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(!definition.is_required);
    }

    #[test]
    fn test_default_stage_all() {
        assert_eq!(default_stage_all(), true);
    }

    #[tokio::test]
    async fn test_parameter_validation() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = CommitChangesTool::new(repo_manager);

        // Test with missing required field
        let invalid_params = json!({
            "repository_name": "test-repo"
            // Missing message
        });

        let result = tool.execute(invalid_params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid parameters"));
    }

    #[tokio::test]
    async fn test_nonexistent_repository() {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        let tool = CommitChangesTool::new(repo_manager);

        let params = json!({
            "repository_name": "nonexistent-repo",
            "message": "Test commit"
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