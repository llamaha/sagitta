use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use std::path::PathBuf;
use std::fs;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Parameters for line-based code editing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditParams {
    /// The repository containing the file
    pub repository_name: String,
    
    /// The path to the file within the repository
    pub file_path: String,
    
    /// Starting line number for the edit (1-indexed, inclusive)
    pub line_start: usize,
    
    /// Ending line number for the edit (1-indexed, inclusive)
    pub line_end: usize,
    
    /// New content to replace the lines
    pub content: String,
    
    /// Whether to format the code after editing
    #[serde(default)]
    pub format: bool,
}

/// Tool for editing code by line numbers
#[derive(Debug)]
pub struct EditTool {
    /// Repository manager for accessing repositories
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl EditTool {
    /// Create a new edit tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
    
    /// Perform a line-based edit
    async fn perform_edit(&self, params: &EditParams) -> Result<String, SagittaCodeError> {
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
        
        // Read the file
        let file_content = fs::read_to_string(&full_path)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to read file: {}", e)))?;
        
        // Split into lines
        let lines: Vec<&str> = file_content.lines().collect();
        
        // Validate line numbers
        if params.line_start < 1 || params.line_start > lines.len() {
            return Err(SagittaCodeError::ToolError(format!(
                "Invalid start line number: {}. File has {} lines",
                params.line_start, lines.len()
            )));
        }
        
        if params.line_end < params.line_start || params.line_end > lines.len() {
            return Err(SagittaCodeError::ToolError(format!(
                "Invalid end line number: {}. File has {} lines, and start line is {}",
                params.line_end, lines.len(), params.line_start
            )));
        }
        
        // Convert 1-indexed to 0-indexed
        let line_start_idx = params.line_start - 1;
        let line_end_idx = params.line_end - 1;
        
        // Create a new content with the edit
        let mut new_lines = Vec::new();
        
        // Add lines before the edit
        for i in 0..line_start_idx {
            new_lines.push(lines[i]);
        }
        
        // Add the new content
        for line in params.content.lines() {
            new_lines.push(line);
        }
        
        // Add lines after the edit
        for i in (line_end_idx + 1)..lines.len() {
            new_lines.push(lines[i]);
        }
        
        // Join the lines
        let new_content = new_lines.join("\n");
        
        // Write the file
        fs::write(&full_path, &new_content)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to write file: {}", e)))?;
        
        // TODO: Format the code if requested
        
        Ok(format!("Successfully edited lines {} to {} in {}", params.line_start, params.line_end, params.file_path))
    }
}

#[async_trait]
impl Tool for EditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit_file".to_string(),
            description: "Edit a file by replacing specified lines with new content".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["repository_name", "file_path", "line_start", "line_end", "content"],
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "The repository containing the file to edit"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file within the repository"
                    },
                    "line_start": {
                        "type": "integer",
                        "description": "Starting line number for the edit (1-indexed, inclusive)"
                    },
                    "line_end": {
                        "type": "integer",
                        "description": "Ending line number for the edit (1-indexed, inclusive)"
                    },
                    "content": {
                        "type": "string",
                        "description": "New content to replace the lines"
                    },
                    "format": {
                        "type": "boolean",
                        "description": "Whether to format the code after editing",
                        "default": false
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
        let params: EditParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to parse edit parameters: {}", e)))?;
        
        // Perform the edit
        match self.perform_edit(&params).await {
            Ok(message) => {
                Ok(ToolResult::Success(serde_json::json!({
                    "message": message,
                    "repository_name": params.repository_name,
                    "file_path": params.file_path,
                    "line_start": params.line_start,
                    "line_end": params.line_end,
                    "success": true
                })))
            },
            Err(e) => {
                Err(e)
            }
        }
    }
} 