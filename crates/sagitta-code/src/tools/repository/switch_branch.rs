use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Parameters for switching branches or checking out git references
#[derive(Debug, Deserialize, Serialize)]
pub struct SwitchBranchParams {
    /// Name of the repository
    pub repository_name: String,
    /// Target branch name (optional if target_ref is provided)
    pub branch_name: Option<String>,
    /// Optional specific Git ref (tag, commit hash, branch name) to check out.
    /// If provided, this ref will be checked out instead of the branch name.
    /// This supports any valid git reference including tags, commits, and remote branches.
    pub target_ref: Option<String>,
    /// Force switch even with uncommitted changes
    #[serde(default)]
    pub force: bool,
    /// Enable automatic resync after switch (default: true)
    #[serde(default = "default_auto_resync")]
    pub auto_resync: bool,
}

fn default_auto_resync() -> bool {
    true
}

/// Tool for switching branches or checking out git references
#[derive(Debug)]
pub struct SwitchBranchTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl SwitchBranchTool {
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self { repo_manager }
    }
}

#[async_trait]
impl Tool for SwitchBranchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "switch_repository_branch".to_string(),
            description: "Switch to a different branch or Git reference in a repository with automatic resync. Supports branches, tags, commits, and remote references.".to_string(),
            category: ToolCategory::Repository,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Name of the repository"
                    },
                    "branch_name": {
                        "type": "string", 
                        "description": "Name of the branch to switch to. Either this or target_ref must be provided, but not both."
                    },
                    "target_ref": {
                        "type": "string",
                        "description": "Optional specific Git ref (tag, commit hash, branch name) to check out. If provided, this ref will be checked out instead of branch_name. Supports any valid git reference including tags, commits, and remote branches."
                    },
                    "force": {
                        "type": "boolean",
                        "description": "Force switch even with uncommitted changes",
                        "default": false
                    },
                    "auto_resync": {
                        "type": "boolean",
                        "description": "Enable automatic resync after switch",
                        "default": true
                    }
                },
                "required": ["repository_name"],
                "oneOf": [
                    {"required": ["branch_name"]},
                    {"required": ["target_ref"]}
                ]
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let params: SwitchBranchParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;
        
        // Validate parameters - either branch_name or target_ref must be provided
        let (target_ref_to_checkout, is_target_ref) = match (&params.branch_name, &params.target_ref) {
            (Some(branch_name), None) => (branch_name.clone(), false),
            (None, Some(target_ref)) => (target_ref.clone(), true),
            (Some(_), Some(_)) => {
                return Err(SagittaCodeError::ToolError(
                    "Cannot specify both branch_name and target_ref. Use one or the other.".to_string()
                ));
            }
            (None, None) => {
                return Err(SagittaCodeError::ToolError(
                    "Must specify either branch_name or target_ref.".to_string()
                ));
            }
        };
        
        let repo_manager = self.repo_manager.lock().await;
        
        // TODO: Add force support to the switch_branch method signature when it's available
        let result = repo_manager.switch_branch(
            &params.repository_name, 
            &target_ref_to_checkout,
            params.auto_resync
        ).await;
        
        match result {
            Ok(switch_result) => {
                let ref_type = if is_target_ref { "ref" } else { "branch" };
                
                Ok(ToolResult::Success(serde_json::json!({
                    "success": switch_result.success,
                    "previous_branch": switch_result.previous_branch,
                    "new_branch": switch_result.new_branch,
                    "ref_type": ref_type,
                    "target_ref": target_ref_to_checkout,
                    "sync_type": switch_result.sync_type,
                    "files_processed": switch_result.files_processed,
                    "message": format!("Successfully switched to {} '{}'", ref_type, target_ref_to_checkout)
                })))
            }
            Err(e) => {
                Err(SagittaCodeError::ToolError(format!("Failed to switch branch/ref: {}", e)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parameter_validation_branch_name() {
        let params = json!({
            "repository_name": "test-repo",
            "branch_name": "feature-branch"
        });
        
        let parsed: Result<SwitchBranchParams, _> = serde_json::from_value(params);
        assert!(parsed.is_ok());
        
        let params = parsed.unwrap();
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.branch_name, Some("feature-branch".to_string()));
        assert_eq!(params.target_ref, None);
        assert!(!params.force);
        assert!(params.auto_resync); // default is true
    }

    #[test]
    fn test_parameter_validation_target_ref() {
        let params = json!({
            "repository_name": "test-repo",
            "target_ref": "v1.0.0"
        });
        
        let parsed: Result<SwitchBranchParams, _> = serde_json::from_value(params);
        assert!(parsed.is_ok());
        
        let params = parsed.unwrap();
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.branch_name, None);
        assert_eq!(params.target_ref, Some("v1.0.0".to_string()));
        assert!(!params.force);
        assert!(params.auto_resync);
    }

    #[test]
    fn test_parameter_validation_with_options() {
        let params = json!({
            "repository_name": "test-repo",
            "branch_name": "feature-branch",
            "force": true,
            "auto_resync": false
        });
        
        let parsed: Result<SwitchBranchParams, _> = serde_json::from_value(params);
        assert!(parsed.is_ok());
        
        let params = parsed.unwrap();
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.branch_name, Some("feature-branch".to_string()));
        assert_eq!(params.target_ref, None);
        assert!(params.force);
        assert!(!params.auto_resync);
    }
} 