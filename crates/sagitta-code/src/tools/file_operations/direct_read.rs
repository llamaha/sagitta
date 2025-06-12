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
        // If a relative path is given we first attempt to resolve it against the *current* process
        // working directory (which is updated by WorkingDirectoryManager when the user calls
        // `change_directory`).  This allows tools to work correctly after the agent has changed
        // directories.  If that file does not exist we fall back to resolving relative to the
        // workspace root (`base_directory`) to keep backwards-compatibility with existing tests.
        let absolute_path = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            // Try `cwd` first
            match std::env::current_dir() {
                Ok(cwd) => {
                    let candidate = cwd.join(file_path);
                    if candidate.exists() {
                        candidate
                    } else {
                        self.base_directory.join(file_path)
                    }
                }
                Err(_) => self.base_directory.join(file_path),
            }
        };
        
        // Validate that the path is within the base directory for security
        let canonical_base = self.base_directory.canonicalize()
            .map_err(|e| SagittaCodeError::ToolError(format!(
                "Failed to canonicalize base directory '{}': {}",
                self.base_directory.display(), e
            )))?;
        
        // Try to canonicalize the target path if it exists
        let canonical_path = if absolute_path.exists() {
            absolute_path.canonicalize()
                .map_err(|e| SagittaCodeError::ToolError(format!(
                    "Failed to canonicalize path '{}': {}",
                    absolute_path.display(), e
                )))?
        } else {
            // For non-existent paths, we still need to validate they would be within bounds
            absolute_path.clone()
        };
        
        // Check if the path is within the base directory
        let is_within_base = if canonical_path.exists() {
            // For existing paths, use the canonical form
            canonical_path.starts_with(&canonical_base)
        } else {
            // For non-existing paths, check if the target path starts with base
            absolute_path.starts_with(&self.base_directory) ||
            absolute_path.starts_with(&canonical_base)
        };
        
        if !is_within_base {
            return Err(SagittaCodeError::ToolError(format!(
                "Access denied: Path '{}' is outside the allowed workspace directory '{}'. All file operations must stay within the workspace for security.",
                absolute_path.display(),
                self.base_directory.display()
            )));
        }
        
        // Forbid certain system-critical directories even if they're somehow within base
        let forbidden_paths = [
            "/etc", "/bin", "/sbin", "/usr/bin", "/usr/sbin", 
            "/boot", "/root", "/proc", "/sys", "/dev"
        ];
        
        let path_str = absolute_path.to_string_lossy();
        for forbidden in &forbidden_paths {
            if path_str == *forbidden || path_str.starts_with(&format!("{}/", forbidden)) {
                return Err(SagittaCodeError::ToolError(format!(
                    "Access denied: Reading from system directory '{}' is forbidden for security reasons",
                    path_str
                )));
            }
        }
        
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

    #[tokio::test]
    async fn test_reject_paths_outside_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let tool = DirectFileReadTool::new(temp_dir.path().to_path_buf());
        
        // Try to read a file outside the workspace using relative path traversal
        let params = serde_json::json!({
            "file_path": "../../etc/passwd"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Access denied") || error_msg.contains("outside the allowed workspace"));
    }

    #[tokio::test]
    async fn test_reject_system_directories() {
        let temp_dir = TempDir::new().unwrap();
        let tool = DirectFileReadTool::new(temp_dir.path().to_path_buf());
        
        // Try to read from system directories using absolute paths
        let system_paths = ["/etc/passwd", "/bin/sh", "/usr/bin/ls"];
        
        for system_path in &system_paths {
            let params = serde_json::json!({
                "file_path": system_path
            });
            
            let result = tool.execute(params).await;
            assert!(result.is_err(), "Should reject access to {}", system_path);
            let error_msg = result.unwrap_err().to_string();
            assert!(
                error_msg.contains("Access denied") || 
                error_msg.contains("outside the allowed workspace") ||
                error_msg.contains("system directory"),
                "Error message should indicate access denial for {}: {}", system_path, error_msg
            );
        }
    }
} 