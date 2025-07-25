use crate::mcp::types::{WriteFileParams, WriteFileResult, ErrorObject};
use crate::middleware::auth_middleware::AuthenticatedUser;
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use axum::Extension;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use std::path::{Path, PathBuf};
use super::utils::get_current_repository_path;

/// Handler for writing file contents
pub async fn handle_write_file<C: QdrantClientTrait + Send + Sync + 'static>(
    params: WriteFileParams,
    config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<WriteFileResult, ErrorObject> {
    // Handle repository context and relative paths
    let file_path = if let Some(repo_name) = &params.repository_name {
        // Use specified repository
        let config_guard = config.read().await;
        let repo_config = config_guard.repositories.iter()
            .find(|r| r.name == *repo_name)
            .ok_or_else(|| ErrorObject {
                code: -32603,
                message: format!("Repository '{}' not found", repo_name),
                data: None,
            })?;
        
        if Path::new(&params.file_path).is_absolute() {
            // Verify absolute path is within repository
            let absolute_path = PathBuf::from(&params.file_path);
            let canonical_base = repo_config.local_path.canonicalize()
                .map_err(|e| ErrorObject {
                    code: -32603,
                    message: format!("Failed to canonicalize repository path: {}", e),
                    data: None,
                })?;
            
            // For write_file, we may need to create the file first
            if absolute_path.exists() {
                let canonical_target = absolute_path.canonicalize()
                    .map_err(|e| ErrorObject {
                        code: -32603,
                        message: format!("Failed to canonicalize target path: {}", e),
                        data: None,
                    })?;
                
                if !canonical_target.starts_with(&canonical_base) {
                    return Err(ErrorObject {
                        code: -32603,
                        message: format!("File path '{}' is outside repository '{}'", params.file_path, repo_name),
                        data: None,
                    });
                }
            } else {
                // For new files, check parent directory
                if let Some(parent) = absolute_path.parent() {
                    if parent.exists() {
                        let canonical_parent = parent.canonicalize()
                            .map_err(|e| ErrorObject {
                                code: -32603,
                                message: format!("Failed to canonicalize parent path: {}", e),
                                data: None,
                            })?;
                        
                        if !canonical_parent.starts_with(&canonical_base) {
                            return Err(ErrorObject {
                                code: -32603,
                                message: format!("File path '{}' is outside repository '{}'", params.file_path, repo_name),
                                data: None,
                            });
                        }
                    }
                }
            }
            absolute_path
        } else {
            // Relative path within repository
            repo_config.local_path.join(&params.file_path)
        }
    } else if Path::new(&params.file_path).is_absolute() {
        PathBuf::from(&params.file_path)
    } else {
        // Try to get repository context
        if let Some(repo_path) = get_current_repository_path().await {
            repo_path.join(&params.file_path)
        } else {
            // Fallback to current directory if no repository context
            std::env::current_dir()
                .map_err(|e| ErrorObject {
                    code: -32603,
                    message: format!("Failed to get current directory: {e}"),
                    data: None,
                })?
                .join(&params.file_path)
        }
    };
    
    let path = file_path.as_path();
    
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
    
    if let Err(e) = fs::write(&file_path, content_bytes).await {
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
        file_path: file_path.display().to_string(),
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
            repository_name: None,
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
            repository_name: None,
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
            repository_name: None,
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
            repository_name: None,
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
            repository_name: None,
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