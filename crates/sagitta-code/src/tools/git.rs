use std::sync::Arc;
use std::path::PathBuf;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;

use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Parameters for creating a new git branch
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateBranchParams {
    /// Name for the new branch
    pub branch_name: String,
    /// Starting point for the branch (commit hash, branch name, or tag). If not provided, uses current HEAD
    pub start_point: Option<String>,
    /// Whether to switch to the new branch after creation
    #[serde(default)]
    pub checkout: bool,
}

/// Tool for creating git branches in the current directory
#[derive(Debug)]
pub struct GitCreateBranchTool {
    working_dir: PathBuf,
}

impl GitCreateBranchTool {
    /// Create a new git create branch tool
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for GitCreateBranchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "git_create_branch".to_string(),
            description: "Create a new git branch in the current working directory".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["branch_name", "start_point", "checkout"],
                "properties": {
                    "branch_name": {
                        "type": "string",
                        "description": "Name for the new branch"
                    },
                    "start_point": {
                        "type": ["string", "null"],
                        "description": "Starting point for the branch (commit hash, branch name, or tag). If not provided, uses current HEAD"
                    },
                    "checkout": {
                        "type": ["boolean", "null"],
                        "description": "Whether to switch to the new branch after creation (defaults to false if null)"
                    }
                }
            }),
            category: ToolCategory::Repository,
            is_required: false,
            metadata: std::collections::HashMap::new(),
        }
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let params: CreateBranchParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;

        // Validate branch name
        if params.branch_name.trim().is_empty() {
            return Ok(ToolResult::error("Branch name cannot be empty"));
        }

        // Check if we're in a git repository
        let git_check = Command::new("git")
            .arg("rev-parse")
            .arg("--git-dir")
            .current_dir(&self.working_dir)
            .output()
            .await;

        if git_check.is_err() || !git_check.unwrap().status.success() {
            return Ok(ToolResult::error("Not in a git repository"));
        }

        // Create the branch
        let mut cmd = Command::new("git");
        cmd.arg("branch")
            .arg(&params.branch_name)
            .current_dir(&self.working_dir);

        if let Some(start_point) = &params.start_point {
            cmd.arg(start_point);
        }

        let output = cmd.output().await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to execute git command: {}", e)))?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolResult::error(format!("Failed to create branch: {}", error_msg)));
        }

        // Checkout the branch if requested
        if params.checkout {
            let checkout_output = Command::new("git")
                .arg("checkout")
                .arg(&params.branch_name)
                .current_dir(&self.working_dir)
                .output()
                .await
                .map_err(|e| SagittaCodeError::ToolError(format!("Failed to execute git checkout: {}", e)))?;

            if !checkout_output.status.success() {
                let error_msg = String::from_utf8_lossy(&checkout_output.stderr);
                return Ok(ToolResult::error(format!("Created branch but failed to checkout: {}", error_msg)));
            }
        }

        let message = if params.checkout {
            format!("Successfully created and switched to branch '{}'", params.branch_name)
        } else {
            format!("Successfully created branch '{}'", params.branch_name)
        };

        Ok(ToolResult::success(serde_json::json!({
            "message": message,
            "branch_name": params.branch_name,
            "checked_out": params.checkout,
            "working_directory": self.working_dir.display().to_string()
        })))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Tool for listing git branches in the current directory
#[derive(Debug)]
pub struct GitListBranchesTool {
    working_dir: PathBuf,
}

impl GitListBranchesTool {
    /// Create a new git list branches tool
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for GitListBranchesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "git_list_branches".to_string(),
            description: "List all git branches in the current working directory".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["include_remote"],
                "properties": {
                    "include_remote": {
                        "type": ["boolean", "null"],
                        "description": "Whether to include remote branches (defaults to false if null)"
                    }
                }
            }),
            category: ToolCategory::Repository,
            is_required: false,
            metadata: std::collections::HashMap::new(),
        }
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        #[derive(Debug, Deserialize)]
        struct ListBranchesParams {
            #[serde(default)]
            include_remote: bool,
        }

        let params: ListBranchesParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;

        // Check if we're in a git repository
        let git_check = Command::new("git")
            .arg("rev-parse")
            .arg("--git-dir")
            .current_dir(&self.working_dir)
            .output()
            .await;

        if git_check.is_err() || !git_check.unwrap().status.success() {
            return Ok(ToolResult::error("Not in a git repository"));
        }

        // List branches
        let mut cmd = Command::new("git");
        cmd.arg("branch");
        
        if params.include_remote {
            cmd.arg("-a");
        }

        cmd.current_dir(&self.working_dir);

        let output = cmd.output().await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to execute git command: {}", e)))?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolResult::error(format!("Failed to list branches: {}", error_msg)));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let branches: Vec<String> = output_str
            .lines()
            .map(|line| line.trim_start_matches("* ").trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        let current_branch = branches.iter()
            .find(|branch| output_str.contains(&format!("* {}", branch)))
            .cloned();

        Ok(ToolResult::success(serde_json::json!({
            "branches": branches,
            "current_branch": current_branch,
            "working_directory": self.working_dir.display().to_string()
        })))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_git_create_branch_tool_creation() {
        let working_dir = PathBuf::from("/tmp");
        let tool = GitCreateBranchTool::new(working_dir);
        
        assert_eq!(tool.working_dir, PathBuf::from("/tmp"));
    }

    #[test]
    fn test_git_create_branch_tool_definition() {
        let working_dir = PathBuf::from("/tmp");
        let tool = GitCreateBranchTool::new(working_dir);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "git_create_branch");
        assert!(!definition.description.is_empty());
        assert!(definition.parameters.is_object());
        assert_eq!(definition.category, ToolCategory::Repository);
    }

    #[test]
    fn test_git_list_branches_tool_creation() {
        let working_dir = PathBuf::from("/tmp");
        let tool = GitListBranchesTool::new(working_dir);
        
        assert_eq!(tool.working_dir, PathBuf::from("/tmp"));
    }

    #[test]
    fn test_git_list_branches_tool_definition() {
        let working_dir = PathBuf::from("/tmp");
        let tool = GitListBranchesTool::new(working_dir);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "git_list_branches");
        assert!(!definition.description.is_empty());
        assert!(definition.parameters.is_object());
        assert_eq!(definition.category, ToolCategory::Repository);
    }
} 