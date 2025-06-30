use crate::mcp::types::{ReadFileParams, ReadFileResult, ErrorObject};
use crate::middleware::auth_middleware::AuthenticatedUser;
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use axum::Extension;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use std::path::Path;

/// Handler for reading file contents
pub async fn handle_read_file<C: QdrantClientTrait + Send + Sync + 'static>(
    params: ReadFileParams,
    _config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<ReadFileResult, ErrorObject> {
    // Check if file exists
    let path = Path::new(&params.file_path);
    if !path.exists() {
        return Err(ErrorObject {
            code: -32603,
            message: format!("File not found: {}", params.file_path),
            data: None,
        });
    }
    
    if !path.is_file() {
        return Err(ErrorObject {
            code: -32603,
            message: format!("Path is not a file: {}", params.file_path),
            data: None,
        });
    }
    
    // Get file metadata
    let metadata = match fs::metadata(&params.file_path).await {
        Ok(meta) => meta,
        Err(e) => {
            return Err(ErrorObject {
                code: -32603,
                message: format!("Failed to read file metadata: {}", e),
                data: None,
            });
        }
    };
    
    let file_size = metadata.len();
    
    // Read the file content
    let full_content = match fs::read_to_string(&params.file_path).await {
        Ok(content) => content,
        Err(e) => {
            return Err(ErrorObject {
                code: -32603,
                message: format!("Failed to read file: {}", e),
                data: None,
            });
        }
    };
    
    // Split into lines for line-based operations
    let lines: Vec<&str> = full_content.lines().collect();
    let total_lines = lines.len();
    
    // Determine which lines to return
    let (content, start_line, end_line) = if params.start_line.is_some() || params.end_line.is_some() {
        let start = params.start_line.unwrap_or(1);
        let end = params.end_line.unwrap_or(total_lines);
        
        // Validate line numbers (1-based)
        if start < 1 || start > total_lines {
            return Err(ErrorObject {
                code: -32603,
                message: format!("Invalid start_line: {}. File has {} lines", start, total_lines),
                data: None,
            });
        }
        
        if end < start || end > total_lines {
            return Err(ErrorObject {
                code: -32603,
                message: format!("Invalid end_line: {}. File has {} lines", end, total_lines),
                data: None,
            });
        }
        
        // Extract the requested lines (convert to 0-based indexing)
        let selected_lines = &lines[(start - 1)..end];
        let selected_content = selected_lines.join("\n");
        
        (selected_content, Some(start), Some(end))
    } else {
        // Return full content
        (full_content, None, None)
    };
    
    Ok(ReadFileResult {
        file_path: params.file_path,
        content,
        line_count: total_lines,
        file_size,
        start_line,
        end_line,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn create_mock_qdrant() -> Arc<qdrant_client::Qdrant> {
        Arc::new(qdrant_client::Qdrant::from_url("http://localhost:6334").build().unwrap())
    }
    
    #[tokio::test]
    async fn test_read_file_full_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        fs::write(&file_path, test_content).await.unwrap();
        
        let params = ReadFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            start_line: None,
            end_line: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_read_file(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.content, test_content);
        assert_eq!(result.line_count, 5);
        assert_eq!(result.file_size, test_content.len() as u64);
        assert!(result.start_line.is_none());
        assert!(result.end_line.is_none());
    }
    
    #[tokio::test]
    async fn test_read_file_line_range() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        fs::write(&file_path, test_content).await.unwrap();
        
        let params = ReadFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            start_line: Some(2),
            end_line: Some(4),
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_read_file(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.content, "Line 2\nLine 3\nLine 4");
        assert_eq!(result.line_count, 5);
        assert_eq!(result.start_line, Some(2));
        assert_eq!(result.end_line, Some(4));
    }
    
    #[tokio::test]
    async fn test_read_file_single_line() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let test_content = "Line 1\nLine 2\nLine 3";
        fs::write(&file_path, test_content).await.unwrap();
        
        let params = ReadFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            start_line: Some(2),
            end_line: Some(2),
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_read_file(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.content, "Line 2");
        assert_eq!(result.start_line, Some(2));
        assert_eq!(result.end_line, Some(2));
    }
    
    #[tokio::test]
    async fn test_read_file_not_found() {
        let params = ReadFileParams {
            file_path: "/nonexistent/file.txt".to_string(),
            start_line: None,
            end_line: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_read_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("File not found"));
    }
    
    #[tokio::test]
    async fn test_read_file_invalid_line_range() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let test_content = "Line 1\nLine 2\nLine 3";
        fs::write(&file_path, test_content).await.unwrap();
        
        let params = ReadFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            start_line: Some(5),
            end_line: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_read_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("Invalid start_line"));
    }
}