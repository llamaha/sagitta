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
use crate::utils::errors::SagittaCodeError;

/// Parameters for semantic code editing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticEditParams {
    /// The repository containing the file
    pub repository_name: String,
    
    /// The path to the file within the repository
    pub file_path: String,
    
    /// Semantic element to target (e.g., "function:my_func", "class:MyClass")
    pub element: String,
    
    /// New content to replace the element
    pub content: String,
    
    /// Whether to format the code after editing
    #[serde(default)]
    pub format: bool,
    
    /// Whether to update references to the edited element
    #[serde(default)]
    pub update_references: bool,
}

/// Tool for semantic editing of code elements
#[derive(Debug)]
pub struct SemanticEditTool {
    /// Repository manager for accessing repositories
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl SemanticEditTool {
    /// Create a new semantic edit tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
    
    /// Perform a semantic edit using sagitta-cli
    async fn perform_semantic_edit(&self, params: &SemanticEditParams) -> Result<String, SagittaCodeError> {
        // Get repository information
        let repo_manager = self.repo_manager.lock().await;
        
        // Find repository by name
        let repositories = repo_manager.list_repositories().await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to list repositories: {}", e)))?;
        
        let repo_config = repositories.iter()
            .find(|r| r.name == params.repository_name)
            .ok_or_else(|| SagittaCodeError::ToolError(format!("Repository '{}' not found", params.repository_name)))?;
        
        // Construct the full file path
        let full_path = PathBuf::from(&repo_config.local_path).join(&params.file_path);
        
        if !full_path.exists() {
            return Err(SagittaCodeError::ToolError(format!("File '{}' not found", full_path.display())));
        }
        
        // Create a temporary file for the content
        let mut temp_file = NamedTempFile::new()
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to create temporary file: {}", e)))?;
        
        // Write the content to the temporary file
        temp_file.write_all(params.content.as_bytes())
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to write to temporary file: {}", e)))?;
        
        // Get the path of the temporary file
        let temp_file_path = temp_file.path().to_str()
            .ok_or_else(|| SagittaCodeError::ToolError("Failed to get temporary file path".to_string()))?;
        
        // First validate the edit
        let validation_output = Command::new("sagitta-cli")
            .arg("edit")
            .arg("validate")
            .arg("--file").arg(&full_path)
            .arg("--element").arg(&params.element)
            .arg("--content-file").arg(temp_file_path)
            .output()
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to execute sagitta-cli: {}", e)))?;
        
        // Check if validation was successful
        if !validation_output.status.success() {
            let error_message = String::from_utf8_lossy(&validation_output.stderr).to_string();
            return Err(SagittaCodeError::ToolError(format!("Validation failed: {}", error_message)));
        }
        
        // Then apply the edit
        let mut apply_command = Command::new("sagitta-cli");
        apply_command.arg("edit")
            .arg("apply")
            .arg("--file").arg(&full_path)
            .arg("--element").arg(&params.element)
            .arg("--content-file").arg(temp_file_path);
        
        if params.format {
            apply_command.arg("--format");
        }
        
        if params.update_references {
            apply_command.arg("--update-references");
        }
        
        let apply_output = apply_command.output()
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to execute sagitta-cli: {}", e)))?;
        
        // Check if apply was successful
        if !apply_output.status.success() {
            let error_message = String::from_utf8_lossy(&apply_output.stderr).to_string();
            return Err(SagittaCodeError::ToolError(format!("Apply failed: {}", error_message)));
        }
        
        // Return success message
        Ok(format!("Successfully edited element '{}' in {}", params.element, params.file_path))
    }
}

#[async_trait]
impl Tool for SemanticEditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "semantic_edit".to_string(),
            description: "Edit a code element (function, class, etc.) semantically".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["repository_name", "file_path", "element", "content", "format", "update_references"],
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "The repository containing the file to edit"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file within the repository"
                    },
                    "element": {
                        "type": "string",
                        "description": "Semantic element to target (e.g., 'function:my_func', 'class:MyClass')"
                    },
                    "content": {
                        "type": "string",
                        "description": "New content to replace the element"
                    },
                    "format": {
                        "type": ["boolean", "null"],
                        "description": "Whether to format the code after editing (defaults to false if null)"
                    },
                    "update_references": {
                        "type": ["boolean", "null"],
                        "description": "Whether to update references to the edited element (defaults to false if null)"
                    }
                }
            }),
            is_required: false,
            category: ToolCategory::CodeEdit,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        // Parse parameters
        let params: SemanticEditParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to parse semantic edit parameters: {}", e)))?;
        
        // Perform the semantic edit
        match self.perform_semantic_edit(&params).await {
            Ok(message) => {
                Ok(ToolResult::Success(serde_json::json!({
                    "message": message,
                    "repository_name": params.repository_name,
                    "file_path": params.file_path,
                    "element": params.element,
                    "success": true
                })))
            },
            Err(e) => {
                Err(e)
            }
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
} 