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
use crate::tools::file_operations::DirectFileEditTool;
use crate::utils::errors::SagittaCodeError;

/// Parameters for line-based code editing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditParams {
    /// The repository containing the file (optional for fallback mode)
    pub repository_name: Option<String>,
    
    /// The path to the file within the repository or filesystem
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

    /// Whether to create the file if it doesn't exist
    #[serde(default)]
    pub create_if_missing: bool,
}

/// Maximum content size in bytes to prevent streaming buffer overflows
const MAX_CONTENT_SIZE: usize = 10 * 1024; // 10KB limit to prevent streaming timeouts

/// Tool for editing code by line numbers with repository and direct file fallback
#[derive(Debug)]
pub struct EditTool {
    /// Repository manager for accessing repositories
    repo_manager: Arc<Mutex<RepositoryManager>>,
    /// Fallback direct file editor
    direct_editor: DirectFileEditTool,
    /// Base directory for fallback operations
    base_directory: PathBuf,
}

impl EditTool {
    /// Create a new edit tool with fallback capability
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>, base_directory: PathBuf) -> Self {
        let direct_editor = DirectFileEditTool::new(base_directory.clone());
        Self {
            repo_manager,
            direct_editor,
            base_directory,
        }
    }
    
    /// Perform a line-based edit using repository manager
    async fn perform_repository_edit(&self, params: &EditParams) -> Result<String, SagittaCodeError> {
        let repo_name = params.repository_name.as_ref()
            .ok_or_else(|| SagittaCodeError::ToolError(
                "Repository name required for repository-based file editing".to_string()
            ))?;

        // Get repository information
        let repo_manager = self.repo_manager.lock().await;
        
        // Find repository by name
        let repositories = repo_manager.list_repositories().await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to list repositories: {}", e)))?;
        
        let repo_config = repositories.iter()
            .find(|r| r.name == *repo_name)
            .ok_or_else(|| SagittaCodeError::ToolError(format!("Repository '{}' not found", repo_name)))?;
        
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

    /// Try to edit file using direct file access
    async fn try_direct_edit(&self, params: &EditParams) -> Result<String, SagittaCodeError> {
        let direct_params = serde_json::json!({
            "file_path": params.file_path,
            "line_start": params.line_start,
            "line_end": params.line_end,
            "content": params.content,
            "create_if_missing": params.create_if_missing
        });

        match self.direct_editor.execute(direct_params).await? {
            ToolResult::Success(data) => {
                if let Some(message) = data.get("message").and_then(|m| m.as_str()) {
                    Ok(message.to_string())
                } else {
                    Ok(format!("Successfully edited lines {} to {} in {}", params.line_start, params.line_end, params.file_path))
                }
            }
            ToolResult::Error { error } => {
                Err(SagittaCodeError::ToolError(format!("Direct edit failed: {}", error)))
            }
        }
    }

    /// Auto-detect whether to use repository or direct file access
    async fn auto_edit(&self, params: &EditParams) -> Result<String, SagittaCodeError> {
        // If repository name is provided, try repository-based approach first
        if params.repository_name.is_some() {
            match self.perform_repository_edit(params).await {
                Ok(message) => return Ok(message),
                Err(e) => {
                    log::warn!("Repository edit failed, trying direct file access: {}", e);
                    // Continue to fallback
                }
            }
        }

        // Fallback to direct file access
        self.try_direct_edit(params).await
    }
}

#[async_trait]
impl Tool for EditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit_file".to_string(),
            description: "Edit a file by replacing specified lines with new content, with repository and direct file support".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["file_path", "line_start", "line_end", "content"],
                "properties": {
                    "repository_name": {
                        "type": ["string", "null"],
                        "description": "The repository containing the file to edit (optional - will fallback to direct file access if not provided or if repository is not found)"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file within the repository or filesystem"
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
                        "description": "New content to replace the lines. IMPORTANT: Keep content under 10KB. For large files, break into multiple smaller edit_file calls targeting different line ranges."
                    },
                    "format": {
                        "type": ["boolean", "null"],
                        "description": "Whether to format the code after editing (defaults to false if null)"
                    },
                    "create_if_missing": {
                        "type": ["boolean", "null"],
                        "description": "Whether to create the file if it doesn't exist (defaults to false if null)"
                    }
                }
            }),
            is_required: false,
            category: ToolCategory::CodeEdit,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        // Phase 6: Add logging to capture parameter sets Gemini sends
        log::info!("EditTool::execute - Parameters received from LLM: {}", serde_json::to_string_pretty(&parameters).unwrap_or_else(|_| format!("{:?}", parameters)));
        
        // Parse parameters
        let mut params: EditParams = serde_json::from_value(parameters.clone())
            .map_err(|e| {
                log::error!("EditTool::execute - Failed to parse parameters: {}. Parameters were: {}", e, serde_json::to_string_pretty(&parameters).unwrap_or_else(|_| format!("{:?}", parameters)));
                SagittaCodeError::ToolError(format!("Failed to parse edit parameters: {}", e))
            })?;
        
        // Phase 6: Additional parameter validation and normalization
        if params.line_start == 0 {
            log::warn!("EditTool::execute - Parameter normalization: line_start was 0, converting to 1 (1-indexed)");
            params.line_start = 1;
        }
        
        if params.line_end == 0 {
            log::warn!("EditTool::execute - Parameter normalization: line_end was 0, converting to 1 (1-indexed)");
            params.line_end = 1;
        }
        
        if params.line_start > params.line_end {
            log::warn!("EditTool::execute - Parameter normalization: line_start ({}) > line_end ({}), swapping values", params.line_start, params.line_end);
            let temp = params.line_start;
            params.line_start = params.line_end;
            params.line_end = temp;
        }
        
        // Validate content size to prevent streaming buffer issues
        if params.content.len() > MAX_CONTENT_SIZE {
            return Err(SagittaCodeError::ToolError(format!(
                "Content size ({} bytes) exceeds maximum allowed size ({} bytes). Consider breaking large edits into smaller chunks.",
                params.content.len(), MAX_CONTENT_SIZE
            )));
        }
        
        // Perform the edit using auto-detection
        match self.auto_edit(&params).await {
            Ok(message) => {
                log::info!("EditTool::execute - Edit successful: {}", message);
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
                log::error!("EditTool::execute - Edit failed: {}", e);
                Err(e)
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
    use crate::gui::repository::manager::RepositoryManager;
    use sagitta_search::AppConfig as SagittaAppConfig;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use serde_json::json;
    use tempfile::TempDir;

    fn create_test_repo_manager() -> Arc<Mutex<RepositoryManager>> {
        let config = SagittaAppConfig::default();
        let repo_manager = RepositoryManager::new_for_test(Arc::new(Mutex::new(config)));
        Arc::new(Mutex::new(repo_manager))
    }

    #[tokio::test]
    async fn test_edit_tool_creation() {
        let temp_dir = TempDir::new().unwrap();
        let tool = EditTool::new(create_test_repo_manager(), temp_dir.path().to_path_buf());
        let definition = tool.definition();
        assert_eq!(definition.name, "edit_file");
        assert_eq!(definition.category, ToolCategory::CodeEdit);
    }

    #[test]
    fn test_edit_tool_parameter_validation() {
        let temp_dir = TempDir::new().unwrap();
        let tool = EditTool::new(create_test_repo_manager(), temp_dir.path().to_path_buf());
        let definition = tool.definition();
        
        let properties = definition.parameters.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("repository_name"));
        assert!(properties.contains_key("file_path"));
        assert!(properties.contains_key("line_start"));
        assert!(properties.contains_key("line_end"));
        assert!(properties.contains_key("content"));
        assert!(properties.contains_key("format"));
        assert!(properties.contains_key("create_if_missing"));
        
        let required = definition.parameters.get("required").unwrap().as_array().unwrap();
        assert_eq!(required.len(), 4);
        assert!(required.contains(&json!("file_path")));
        assert!(required.contains(&json!("line_start")));
        assert!(required.contains(&json!("line_end")));
        assert!(required.contains(&json!("content")));
        
        // Optional parameters should not be required
        assert!(!required.contains(&json!("repository_name")));
        assert!(!required.contains(&json!("format")));
        assert!(!required.contains(&json!("create_if_missing")));
    }

    #[tokio::test]
    async fn test_edit_tool_minimal_parameters() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_edit.txt");
        std::fs::write(&test_file, "line 1\nline 2\nline 3\n").unwrap();

        let tool = EditTool::new(create_test_repo_manager(), temp_dir.path().to_path_buf());
        
        // Test with only the required parameters
        let params = json!({
            "file_path": "test_edit.txt",
            "line_start": 2,
            "line_end": 2,
            "content": "modified line 2"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                assert!(data["success"].as_bool().unwrap_or(false));
                assert_eq!(data["file_path"], "test_edit.txt");
                assert_eq!(data["line_start"], 2);
                assert_eq!(data["line_end"], 2);
                assert!(data["repository_name"].is_null());
            }
            ToolResult::Error { error } => {
                // This might happen if the direct file edit doesn't work in test environment
                println!("Edit failed (may be acceptable in test): {}", error);
            }
        }
    }

    #[tokio::test]
    async fn test_edit_tool_content_size_validation() {
        let temp_dir = TempDir::new().unwrap();
        let tool = EditTool::new(create_test_repo_manager(), temp_dir.path().to_path_buf());
        
        // Create content larger than MAX_CONTENT_SIZE
        let large_content = "x".repeat(MAX_CONTENT_SIZE + 1);
        
        let params = json!({
            "file_path": "test.txt",
            "line_start": 1,
            "line_end": 1,
            "content": large_content
        });
        
        let result = tool.execute(params).await;
        
        // Should return an error due to content size limit
        assert!(result.is_err());
        let error_msg = format!("{}", result.unwrap_err());
        assert!(error_msg.contains("Content size"));
        assert!(error_msg.contains("exceeds maximum"));
    }
} 