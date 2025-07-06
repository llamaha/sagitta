use crate::mcp::types::{WriteFileParams, WriteFileResult, ErrorObject};
use crate::middleware::auth_middleware::AuthenticatedUser;
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use axum::Extension;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use std::path::Path;

/// Handler for writing file contents
pub async fn handle_write_file<C: QdrantClientTrait + Send + Sync + 'static>(
    params: WriteFileParams,
    _config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<WriteFileResult, ErrorObject> {
    let path = Path::new(&params.file_path);
    
    // Check if file already exists
    let file_exists = path.exists();
    
    // Create parent directories if requested
    if params.create_parents {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent).await {
                    return Err(ErrorObject {
                        code: -32603,
                        message: format!("Failed to create parent directories: {e}"),
                        data: None,
                    });
                }
            }
        }
    }
    
    // Write the content
    let content_bytes = params.content.as_bytes();
    let bytes_written = content_bytes.len() as u64;
    
    if let Err(e) = fs::write(&params.file_path, content_bytes).await {
        return Err(ErrorObject {
            code: -32603,
            message: format!("Failed to write file: {e}"),
            data: None,
        });
    }
    
    // Truncate content for response if it's too large (> 1KB)
    let display_content = if params.content.len() > 1024 {
        format!("{}... (truncated, {} bytes total)", 
                &params.content[..1024], 
                bytes_written)
    } else {
        params.content.clone()
    };
    
    Ok(WriteFileResult {
        file_path: params.file_path,
        content: display_content,
        bytes_written,
        created: !file_exists,
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
    async fn test_write_file_new() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");
        
        let content = "Hello, World!";
        let params = WriteFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            content: content.to_string(),
            create_parents: true,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_write_file(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.content, content);
        assert_eq!(result.bytes_written, content.len() as u64);
        assert!(result.created);
        
        // Verify file was actually written
        let read_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_content, content);
    }
    
    #[tokio::test]
    async fn test_write_file_overwrite() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("existing.txt");
        
        // Create existing file
        fs::write(&file_path, "Old content").await.unwrap();
        
        let new_content = "New content";
        let params = WriteFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            content: new_content.to_string(),
            create_parents: true,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_write_file(params, config, qdrant_client, None).await.unwrap();
        
        assert!(!result.created);
        assert_eq!(result.bytes_written, new_content.len() as u64);
        
        // Verify file was overwritten
        let read_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_content, new_content);
    }
    
    #[tokio::test]
    async fn test_write_file_with_parent_creation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("subdir").join("nested").join("file.txt");
        
        let content = "Nested file content";
        let params = WriteFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            content: content.to_string(),
            create_parents: true,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_write_file(params, config, qdrant_client, None).await.unwrap();
        
        assert!(result.created);
        assert!(file_path.exists());
        
        // Verify file was written
        let read_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_content, content);
    }
    
    #[tokio::test]
    async fn test_write_file_without_parent_creation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent").join("file.txt");
        
        let params = WriteFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            content: "Content".to_string(),
            create_parents: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_write_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("Failed to write file"));
    }
    
    #[tokio::test]
    async fn test_write_file_large_content_truncation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.txt");
        
        // Create content larger than 1KB
        let large_content = "x".repeat(2000);
        let params = WriteFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            content: large_content.clone(),
            create_parents: true,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_write_file(params, config, qdrant_client, None).await.unwrap();
        
        // Check that display content is truncated
        assert!(result.content.contains("... (truncated"));
        assert!(result.content.len() < large_content.len());
        assert_eq!(result.bytes_written, 2000);
        
        // But full content was written to file
        let read_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_content.len(), 2000);
    }
}