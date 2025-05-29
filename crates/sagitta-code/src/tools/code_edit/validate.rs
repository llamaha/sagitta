use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use std::path::PathBuf;
use std::fs;
use std::process::Command;
use std::io::Write;
use tempfile::NamedTempFile;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::FredAgentError;

/// Parameters for validating code edits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateParams {
    /// The repository containing the file
    pub repository_name: String,
    
    /// The path to the file within the repository
    pub file_path: String,
    
    /// Content to validate
    pub content: String,
    
    /// Optional semantic element to target (e.g., "function:my_func", "class:MyClass")
    pub element: Option<String>,
    
    /// Optional line start (1-indexed, inclusive)
    pub line_start: Option<usize>,
    
    /// Optional line end (1-indexed, inclusive)
    pub line_end: Option<usize>,
}

/// Tool for validating code edits before applying them
#[derive(Debug)]
pub struct ValidateTool {
    /// Repository manager for accessing repositories
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl ValidateTool {
    /// Create a new validate tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
    
    /// Validate a code edit
    async fn validate_edit(&self, params: &ValidateParams) -> Result<String, FredAgentError> {
        // Get repository information
        let repo_manager = self.repo_manager.lock().await;
        
        // Find repository by name
        let repositories = repo_manager.list_repositories().await
            .map_err(|e| FredAgentError::ToolError(format!("Failed to list repositories: {}", e)))?;
        
        let repo_config = repositories.iter()
            .find(|r| r.name == params.repository_name)
            .ok_or_else(|| FredAgentError::ToolError(format!("Repository '{}' not found", params.repository_name)))?;
        
        // Construct the full file path
        let full_path = PathBuf::from(&repo_config.local_path).join(&params.file_path);
        
        if !full_path.exists() {
            return Err(FredAgentError::ToolError(format!("File '{}' not found", full_path.display())));
        }
        
        // Create a temporary file for the content
        let mut temp_file = NamedTempFile::new()
            .map_err(|e| FredAgentError::ToolError(format!("Failed to create temporary file: {}", e)))?;
        
        // Write the content to the temporary file
        temp_file.write_all(params.content.as_bytes())
            .map_err(|e| FredAgentError::ToolError(format!("Failed to write to temporary file: {}", e)))?;
        
        // Get the path of the temporary file
        let temp_file_path = temp_file.path().to_str()
            .ok_or_else(|| FredAgentError::ToolError("Failed to get temporary file path".to_string()))?;
        
        // Build the sagitta-cli command for validation
        let mut validate_command = Command::new("sagitta-cli");
        validate_command.arg("edit")
            .arg("validate")
            .arg("--file").arg(&full_path)
            .arg("--content-file").arg(temp_file_path);
        
        // Add element or line numbers based on what was provided
        if let Some(element) = &params.element {
            validate_command.arg("--element").arg(element);
        } else if let (Some(line_start), Some(line_end)) = (params.line_start, params.line_end) {
            validate_command.arg("--line-start").arg(line_start.to_string())
                .arg("--line-end").arg(line_end.to_string());
        } else {
            return Err(FredAgentError::ToolError("Either element or both line_start and line_end must be provided".to_string()));
        }
        
        // Execute the command
        let validation_output = validate_command.output()
            .map_err(|e| FredAgentError::ToolError(format!("Failed to execute sagitta-cli: {}", e)))?;
        
        // Check validation result
        let output_text = String::from_utf8_lossy(&validation_output.stdout).to_string();
        let error_text = String::from_utf8_lossy(&validation_output.stderr).to_string();
        
        if validation_output.status.success() {
            Ok(format!("Validation successful: {}", output_text))
        } else {
            Err(FredAgentError::ToolError(format!("Validation failed: {}", error_text)))
        }
    }
}

#[async_trait]
impl Tool for ValidateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "validate_edit".to_string(),
            description: "Validate a code edit without applying it".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["repository_name", "file_path", "content"],
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "The repository containing the file"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file within the repository"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to validate"
                    },
                    "element": {
                        "type": "string",
                        "description": "Optional semantic element to target (e.g., 'function:my_func', 'class:MyClass')"
                    },
                    "line_start": {
                        "type": "integer",
                        "description": "Optional line start (1-indexed, inclusive)"
                    },
                    "line_end": {
                        "type": "integer",
                        "description": "Optional line end (1-indexed, inclusive)"
                    }
                }
            }),
            is_required: false,
            category: ToolCategory::CodeEdit,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, parameters: Value) -> Result<ToolResult, FredAgentError> {
        // Parse parameters
        let params: ValidateParams = serde_json::from_value(parameters)
            .map_err(|e| FredAgentError::ToolError(format!("Failed to parse validate parameters: {}", e)))?;
        
        // Validate the edit
        match self.validate_edit(&params).await {
            Ok(message) => {
                Ok(ToolResult::Success(serde_json::json!({
                    "message": message,
                    "repository_name": params.repository_name,
                    "file_path": params.file_path,
                    "element": params.element,
                    "line_start": params.line_start,
                    "line_end": params.line_end,
                    "valid": true
                })))
            },
            Err(e) => {
                Ok(ToolResult::Success(serde_json::json!({
                    "message": e.to_string(),
                    "repository_name": params.repository_name,
                    "file_path": params.file_path,
                    "element": params.element,
                    "line_start": params.line_start,
                    "line_end": params.line_end,
                    "valid": false
                })))
            }
        }
    }
} 