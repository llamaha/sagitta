use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::fs;

use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Parameters for reading a file directly without repository management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectReadFileParams {
    /// The absolute or relative path to the file
    pub file_path: String,
    
    /// Optional start line (1-indexed)
    pub start_line: Option<u32>,
    
    /// Optional end line (1-indexed, inclusive)
    pub end_line: Option<u32>,
}

/// Tool for reading files directly from the filesystem without repository management
#[derive(Debug)]
pub struct DirectFileReadTool {
    /// Base directory for resolving relative paths
    base_directory: PathBuf,
}

impl DirectFileReadTool {
    /// Create a new direct file read tool
    pub fn new(base_directory: PathBuf) -> Self {
        Self {
            base_directory,
        }
    }
    
    /// Read file content directly from filesystem
    async fn read_file_content(&self, params: &DirectReadFileParams) -> Result<String, SagittaCodeError> {
        let file_path = Path::new(&params.file_path);
        
        // Resolve absolute path
        let absolute_path = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            self.base_directory.join(file_path)
        };
        
        // Check if file exists
        if !absolute_path.exists() {
            return Err(SagittaCodeError::ToolError(format!(
                "File not found: {}. Please check the path and ensure the file exists.",
                absolute_path.display()
            )));
        }
        
        if !absolute_path.is_file() {
            return Err(SagittaCodeError::ToolError(format!(
                "Path is not a file: {}. Please provide a path to a file, not a directory.",
                absolute_path.display()
            )));
        }
        
        // Read file content
        let content = fs::read_to_string(&absolute_path)
            .map_err(|e| SagittaCodeError::ToolError(format!(
                "Failed to read file {}: {}. Check file permissions and encoding.",
                absolute_path.display(), e
            )))?;
        
        // Apply line range if specified
        if let (Some(start), Some(end)) = (params.start_line, params.end_line) {
            let lines: Vec<&str> = content.lines().collect();
            let start_idx = (start as usize).saturating_sub(1);
            let end_idx = (end as usize).min(lines.len());
            
            if start_idx >= lines.len() {
                return Err(SagittaCodeError::ToolError(format!(
                    "Start line {} is beyond file length ({}). File has {} lines.",
                    start, lines.len(), lines.len()
                )));
            }
            
            let selected_lines = &lines[start_idx..end_idx];
            Ok(selected_lines.join("\n"))
        } else if let Some(start) = params.start_line {
            let lines: Vec<&str> = content.lines().collect();
            let start_idx = (start as usize).saturating_sub(1);
            
            if start_idx >= lines.len() {
                return Err(SagittaCodeError::ToolError(format!(
                    "Start line {} is beyond file length ({}). File has {} lines.",
                    start, lines.len(), lines.len()
                )));
            }
            
            let selected_lines = &lines[start_idx..];
            Ok(selected_lines.join("\n"))
        } else {
            Ok(content)
        }
    }
}

#[async_trait]
impl Tool for DirectFileReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "direct_read_file".to_string(),
            description: "Read the contents of a file directly from the filesystem without requiring repository registration".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["file_path"],
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file (absolute or relative to working directory)"
                    },
                    "start_line": {
                        "type": "integer",
                        "description": "Optional start line (1-indexed)"
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Optional end line (1-indexed, inclusive)"
                    }
                }
            }),
            is_required: false,
            category: ToolCategory::FileOperations,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        let params: DirectReadFileParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;
        
        match self.read_file_content(&params).await {
            Ok(content) => Ok(ToolResult::Success(serde_json::json!({
                "file_path": params.file_path,
                "content": content,
                "size": content.len(),
                "message": format!("Successfully read file: {}", params.file_path)
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
    async fn test_direct_read_file_tool_creation() {
        let temp_dir = TempDir::new().unwrap();
        let tool = DirectFileReadTool::new(temp_dir.path().to_path_buf());
        let definition = tool.definition();
        assert_eq!(definition.name, "direct_read_file");
        assert_eq!(definition.category, ToolCategory::FileOperations);
    }

    #[tokio::test]
    async fn test_read_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!\nSecond line\nThird line").unwrap();
        
        let tool = DirectFileReadTool::new(temp_dir.path().to_path_buf());
        let params = serde_json::json!({
            "file_path": "test.txt"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                assert_eq!(data["file_path"], "test.txt");
                assert_eq!(data["content"], "Hello, World!\nSecond line\nThird line");
            }
            _ => panic!("Expected ToolResult::Success"),
        }
    }

    #[tokio::test]
    async fn test_read_file_with_line_range() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3\nLine 4").unwrap();
        
        let tool = DirectFileReadTool::new(temp_dir.path().to_path_buf());
        let params = serde_json::json!({
            "file_path": "test.txt",
            "start_line": 2,
            "end_line": 3
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                assert_eq!(data["content"], "Line 2\nLine 3");
            }
            _ => panic!("Expected ToolResult::Success"),
        }
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let tool = DirectFileReadTool::new(temp_dir.path().to_path_buf());
        let params = serde_json::json!({
            "file_path": "nonexistent.txt"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("File not found"));
    }

    #[tokio::test]
    async fn test_read_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Absolute path test").unwrap();
        
        let tool = DirectFileReadTool::new(PathBuf::from("/tmp"));
        let params = serde_json::json!({
            "file_path": file_path.to_string_lossy()
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                assert_eq!(data["content"], "Absolute path test");
            }
            _ => panic!("Expected ToolResult::Success"),
        }
    }
} 