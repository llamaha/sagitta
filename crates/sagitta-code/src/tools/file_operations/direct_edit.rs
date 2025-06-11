use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::fs;

use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Parameters for editing a file directly without repository management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectEditFileParams {
    /// The absolute or relative path to the file
    pub file_path: String,
    
    /// Starting line number for the edit (1-indexed, inclusive)
    pub line_start: usize,
    
    /// Ending line number for the edit (1-indexed, inclusive)
    pub line_end: usize,
    
    /// New content to replace the lines
    pub content: String,
    
    /// Whether to create the file if it doesn't exist
    #[serde(default)]
    pub create_if_missing: bool,
}

/// Maximum content size in bytes to prevent memory issues
const MAX_CONTENT_SIZE: usize = 50 * 1024; // 50KB limit

/// Tool for editing files directly from the filesystem without repository management
#[derive(Debug)]
pub struct DirectFileEditTool {
    /// Base directory for resolving relative paths
    base_directory: PathBuf,
}

impl DirectFileEditTool {
    /// Create a new direct file edit tool
    pub fn new(base_directory: PathBuf) -> Self {
        Self {
            base_directory,
        }
    }
    
    /// Edit file content directly on filesystem
    async fn edit_file_content(&self, params: &DirectEditFileParams) -> Result<String, SagittaCodeError> {
        // Validate content size
        if params.content.len() > MAX_CONTENT_SIZE {
            return Err(SagittaCodeError::ToolError(format!(
                "Content size ({} bytes) exceeds maximum allowed size ({} bytes). Consider breaking large edits into smaller chunks.",
                params.content.len(), MAX_CONTENT_SIZE
            )));
        }
        
        let file_path = Path::new(&params.file_path);
        
        // Resolve absolute path
        let absolute_path = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            self.base_directory.join(file_path)
        };
        
        // Handle file creation if it doesn't exist
        if !absolute_path.exists() {
            if params.create_if_missing {
                // Create parent directories if they don't exist
                if let Some(parent) = absolute_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| SagittaCodeError::ToolError(format!(
                            "Failed to create parent directories for {}: {}",
                            absolute_path.display(), e
                        )))?;
                }
                
                // Create empty file
                fs::write(&absolute_path, "")
                    .map_err(|e| SagittaCodeError::ToolError(format!(
                        "Failed to create file {}: {}",
                        absolute_path.display(), e
                    )))?;
            } else {
                return Err(SagittaCodeError::ToolError(format!(
                    "File not found: {}. Set 'create_if_missing' to true to create the file.",
                    absolute_path.display()
                )));
            }
        }
        
        if !absolute_path.is_file() {
            return Err(SagittaCodeError::ToolError(format!(
                "Path is not a file: {}. Please provide a path to a file, not a directory.",
                absolute_path.display()
            )));
        }
        
        // Read existing file content
        let existing_content = fs::read_to_string(&absolute_path)
            .map_err(|e| SagittaCodeError::ToolError(format!(
                "Failed to read file {}: {}. Check file permissions.",
                absolute_path.display(), e
            )))?;
        
        // Split into lines
        let lines: Vec<&str> = if existing_content.is_empty() {
            vec![]
        } else {
            existing_content.lines().collect()
        };
        
        // Validate line numbers for non-empty files
        if !lines.is_empty() {
            if params.line_start < 1 {
                return Err(SagittaCodeError::ToolError(
                    "Line start must be at least 1 (1-indexed)".to_string()
                ));
            }
            
            if params.line_start > lines.len() + 1 {
                return Err(SagittaCodeError::ToolError(format!(
                    "Invalid start line number: {}. File has {} lines. Use line {} to append to end.",
                    params.line_start, lines.len(), lines.len() + 1
                )));
            }
            
            if params.line_end < params.line_start {
                return Err(SagittaCodeError::ToolError(format!(
                    "Invalid end line number: {}. End line must be >= start line ({})",
                    params.line_end, params.line_start
                )));
            }
            
            if params.line_end > lines.len() {
                return Err(SagittaCodeError::ToolError(format!(
                    "Invalid end line number: {}. File has {} lines",
                    params.line_end, lines.len()
                )));
            }
        } else {
            // For empty files, only allow inserting at line 1
            if params.line_start != 1 || params.line_end != 1 {
                return Err(SagittaCodeError::ToolError(
                    "For empty files, use line_start=1 and line_end=1 to insert content".to_string()
                ));
            }
        }
        
        // Create new content
        let mut new_lines = Vec::new();
        
        if !lines.is_empty() {
            // Convert 1-indexed to 0-indexed
            let line_start_idx = params.line_start.saturating_sub(1);
            let line_end_idx = params.line_end.saturating_sub(1);
            
            // Add lines before the edit
            for i in 0..line_start_idx {
                if i < lines.len() {
                    new_lines.push(lines[i]);
                }
            }
            
            // Add the new content
            for line in params.content.lines() {
                new_lines.push(line);
            }
            
            // Add lines after the edit
            for i in (line_end_idx + 1)..lines.len() {
                new_lines.push(lines[i]);
            }
        } else {
            // For empty files, just add the new content
            for line in params.content.lines() {
                new_lines.push(line);
            }
        }
        
        // Join the lines and write to file
        let new_content = new_lines.join("\n");
        fs::write(&absolute_path, &new_content)
            .map_err(|e| SagittaCodeError::ToolError(format!(
                "Failed to write file {}: {}. Check file permissions and disk space.",
                absolute_path.display(), e
            )))?;
        
        Ok(format!(
            "Successfully edited lines {} to {} in {}",
            params.line_start, params.line_end, params.file_path
        ))
    }
}

#[async_trait]
impl Tool for DirectFileEditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "direct_edit_file".to_string(),
            description: "Edit a file by replacing specified lines with new content, without requiring repository registration".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["file_path", "line_start", "line_end", "content"],
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file (absolute or relative to working directory)"
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
                    "create_if_missing": {
                        "type": "boolean",
                        "description": "Whether to create the file if it doesn't exist"
                    }
                }
            }),
            is_required: false,
            category: ToolCategory::FileOperations,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        let params: DirectEditFileParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;
        
        match self.edit_file_content(&params).await {
            Ok(message) => Ok(ToolResult::Success(serde_json::json!({
                "message": message,
                "file_path": params.file_path,
                "line_start": params.line_start,
                "line_end": params.line_end,
                "success": true
            }))),
            Err(e) => Err(e)
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_direct_edit_file_tool_creation() {
        let temp_dir = TempDir::new().unwrap();
        let tool = DirectFileEditTool::new(temp_dir.path().to_path_buf());
        let definition = tool.definition();
        assert_eq!(definition.name, "direct_edit_file");
        assert_eq!(definition.category, ToolCategory::FileOperations);
    }

    #[tokio::test]
    async fn test_edit_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3").unwrap();
        
        let tool = DirectFileEditTool::new(temp_dir.path().to_path_buf());
        let params = serde_json::json!({
            "file_path": "test.txt",
            "line_start": 2,
            "line_end": 2,
            "content": "Modified Line 2"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                assert_eq!(data["success"], true);
                assert!(data["message"].as_str().unwrap().contains("Successfully edited"));
                
                // Verify file content
                let content = fs::read_to_string(&file_path).unwrap();
                assert_eq!(content, "Line 1\nModified Line 2\nLine 3");
            }
            _ => panic!("Expected ToolResult::Success"),
        }
    }

    #[tokio::test]
    async fn test_create_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let tool = DirectFileEditTool::new(temp_dir.path().to_path_buf());
        let params = serde_json::json!({
            "file_path": "new_file.txt",
            "line_start": 1,
            "line_end": 1,
            "content": "Hello, new file!",
            "create_if_missing": true
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(_) => {
                // Verify file was created
                let file_path = temp_dir.path().join("new_file.txt");
                assert!(file_path.exists());
                let content = fs::read_to_string(&file_path).unwrap();
                assert_eq!(content, "Hello, new file!");
            }
            _ => panic!("Expected ToolResult::Success"),
        }
    }

    #[tokio::test]
    async fn test_edit_multiple_lines() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5").unwrap();
        
        let tool = DirectFileEditTool::new(temp_dir.path().to_path_buf());
        let params = serde_json::json!({
            "file_path": "test.txt",
            "line_start": 2,
            "line_end": 4,
            "content": "New Line 2\nNew Line 3\nNew Line 4"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(_) => {
                // Verify file content
                let content = fs::read_to_string(&file_path).unwrap();
                assert_eq!(content, "Line 1\nNew Line 2\nNew Line 3\nNew Line 4\nLine 5");
            }
            _ => panic!("Expected ToolResult::Success"),
        }
    }

    #[tokio::test]
    async fn test_edit_nonexistent_file_without_create() {
        let temp_dir = TempDir::new().unwrap();
        let tool = DirectFileEditTool::new(temp_dir.path().to_path_buf());
        let params = serde_json::json!({
            "file_path": "nonexistent.txt",
            "line_start": 1,
            "line_end": 1,
            "content": "test",
            "create_if_missing": false
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("File not found"));
    }

    #[tokio::test]
    async fn test_invalid_line_numbers() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Line 1\nLine 2").unwrap();
        
        let tool = DirectFileEditTool::new(temp_dir.path().to_path_buf());
        let params = serde_json::json!({
            "file_path": "test.txt",
            "line_start": 0,
            "line_end": 1,
            "content": "test"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Line start must be at least 1"));
    }

    #[tokio::test]
    async fn test_absolute_path_edit() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Original content").unwrap();
        
        let tool = DirectFileEditTool::new(PathBuf::from("/tmp"));
        let params = serde_json::json!({
            "file_path": file_path.to_string_lossy(),
            "line_start": 1,
            "line_end": 1,
            "content": "Modified content"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(_) => {
                let content = fs::read_to_string(&file_path).unwrap();
                assert_eq!(content, "Modified content");
            }
            _ => panic!("Expected ToolResult::Success"),
        }
    }
} 