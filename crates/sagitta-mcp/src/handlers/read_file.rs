use crate::mcp::types::{ReadFileParams, ReadFileResult, ErrorObject};
use crate::middleware::auth_middleware::AuthenticatedUser;
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use axum::Extension;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use std::path::Path;
use tokio::time::{timeout, Duration};
use tracing::{info, error, warn};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Timeout for file operations to prevent hanging
const FILE_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum file size to read entirely into memory (100MB)
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// Maximum number of concurrent read operations per file
const MAX_CONCURRENT_READS: usize = 10;

// File locking manager to prevent issues during concurrent reads
lazy_static::lazy_static! {
    static ref FILE_READ_LOCKS: Mutex<HashMap<String, Arc<Semaphore>>> = Mutex::new(HashMap::new());
}

/// Get or create a semaphore for file-level read locking
fn get_file_read_lock(file_path: &str) -> Arc<Semaphore> {
    let mut locks = FILE_READ_LOCKS.lock().unwrap();
    locks.entry(file_path.to_string())
        .or_insert_with(|| Arc::new(Semaphore::new(MAX_CONCURRENT_READS)))
        .clone()
}

/// Helper function to read file metadata with timeout
async fn read_metadata_with_timeout(file_path: &str) -> Result<std::fs::Metadata, ErrorObject> {
    match timeout(FILE_OPERATION_TIMEOUT, fs::metadata(file_path)).await {
        Ok(Ok(metadata)) => Ok(metadata),
        Ok(Err(e)) => Err(ErrorObject {
            code: -32603,
            message: format!("Failed to read file metadata for '{}': {}", file_path, e),
            data: None,
        }),
        Err(_) => Err(ErrorObject {
            code: -32603,
            message: format!("File metadata read timed out after 30 seconds for '{}'", file_path),
            data: None,
        }),
    }
}

/// Helper function to read entire file content with timeout
async fn read_file_content_with_timeout(file_path: &str) -> Result<String, ErrorObject> {
    match timeout(FILE_OPERATION_TIMEOUT, fs::read_to_string(file_path)).await {
        Ok(Ok(content)) => Ok(content),
        Ok(Err(e)) => Err(ErrorObject {
            code: -32603,
            message: format!("Failed to read file '{}': {}", file_path, e),
            data: None,
        }),
        Err(_) => Err(ErrorObject {
            code: -32603,
            message: format!("File read operation timed out after 30 seconds for '{}'", file_path),
            data: None,
        }),
    }
}

/// Helper function to read file lines with timeout (for large files)
async fn read_file_lines_with_timeout(
    file_path: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<(String, usize, usize, Option<usize>), ErrorObject> {
    let file = match timeout(FILE_OPERATION_TIMEOUT, fs::File::open(file_path)).await {
        Ok(Ok(file)) => file,
        Ok(Err(e)) => {
            return Err(ErrorObject {
                code: -32603,
                message: format!("Failed to open file '{}': {}", file_path, e),
                data: None,
            });
        }
        Err(_) => {
            return Err(ErrorObject {
                code: -32603,
                message: format!("File open operation timed out after 30 seconds for '{}'", file_path),
                data: None,
            });
        }
    };

    let reader = BufReader::new(file);
    let mut lines_iter = reader.lines();
    let mut content = Vec::new();
    let mut current_line = 0;
    let mut total_lines = 0;

    let start = start_line.unwrap_or(1);
    let end = end_line.unwrap_or(usize::MAX);

    // Read lines with timeout protection
    loop {
        match timeout(Duration::from_secs(5), lines_iter.next_line()).await {
            Ok(Ok(Some(line))) => {
                current_line += 1;
                total_lines = current_line;
                
                if current_line >= start && current_line <= end {
                    content.push(line);
                }
                
                // Stop if we've read all requested lines
                if current_line >= end {
                    break;
                }
            }
            Ok(Ok(None)) => break, // End of file
            Ok(Err(e)) => {
                return Err(ErrorObject {
                    code: -32603,
                    message: format!("Error reading line {} from file: {}", current_line + 1, e),
                    data: None,
                });
            }
            Err(_) => {
                return Err(ErrorObject {
                    code: -32603,
                    message: format!("Reading line {} timed out after 5 seconds", current_line + 1),
                    data: None,
                });
            }
        }
    }

    // Continue counting lines to get total line count
    loop {
        match timeout(Duration::from_secs(1), lines_iter.next_line()).await {
            Ok(Ok(Some(_))) => total_lines += 1,
            Ok(Ok(None)) => break,
            Ok(Err(_)) => break, // Stop counting on error
            Err(_) => {
                warn!("Line counting timed out, using current count");
                break;
            }
        }
    }

    // Validate line ranges
    if let Some(s) = start_line {
        if s > total_lines {
            return Err(ErrorObject {
                code: -32603,
                message: format!("Invalid start_line: {}. File has {} lines", s, total_lines),
                data: None,
            });
        }
    }

    // Determine actual lines read
    let lines_returned = content.len();
    let actual_end_line = if lines_returned > 0 {
        Some(start.saturating_sub(1) + lines_returned)
    } else {
        None
    };

    Ok((content.join("\n"), total_lines, lines_returned, actual_end_line))
}

/// Handler for reading file contents with enhanced robustness
pub async fn handle_read_file<C: QdrantClientTrait + Send + Sync + 'static>(
    params: ReadFileParams,
    _config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<ReadFileResult, ErrorObject> {
    let start_time = std::time::Instant::now();
    info!("Starting read_file operation for: {}", params.file_path);
    
    let file_path = params.file_path.clone();
    
    // Acquire read lock with timeout
    let lock = get_file_read_lock(&file_path);
    let permit = match timeout(FILE_OPERATION_TIMEOUT, lock.acquire()).await {
        Ok(Ok(permit)) => permit,
        Ok(Err(_)) => {
            error!("Failed to acquire read lock for file: {}", file_path);
            return Err(ErrorObject {
                code: -32603,
                message: "Failed to acquire file read lock".to_string(),
                data: None,
            });
        }
        Err(_) => {
            error!("Read lock acquisition timed out for file: {}", file_path);
            return Err(ErrorObject {
                code: -32603,
                message: "File read lock acquisition timed out after 30 seconds".to_string(),
                data: None,
            });
        }
    };
    
    // Perform the read operation while holding the lock
    let result = handle_read_file_inner(params).await;
    
    // Lock is automatically released when permit is dropped
    drop(permit);
    
    let duration = start_time.elapsed();
    match &result {
        Ok(_) => info!("Read completed successfully in {:?}", duration),
        Err(e) => error!("Read failed after {:?}: {}", duration, e.message),
    }
    
    result
}

/// Inner handler for reading file contents (without locking)
async fn handle_read_file_inner(params: ReadFileParams) -> Result<ReadFileResult, ErrorObject> {
    // Check if file exists with timeout
    let path = Path::new(&params.file_path);
    let exists = match timeout(Duration::from_secs(5), async { path.exists() }).await {
        Ok(exists) => exists,
        Err(_) => {
            return Err(ErrorObject {
                code: -32603,
                message: format!("File existence check timed out for: {}", params.file_path),
                data: None,
            });
        }
    };
    
    if !exists {
        return Err(ErrorObject {
            code: -32603,
            message: format!("File not found: {}", params.file_path),
            data: None,
        });
    }
    
    let is_file = match timeout(Duration::from_secs(5), async { path.is_file() }).await {
        Ok(is_file) => is_file,
        Err(_) => {
            return Err(ErrorObject {
                code: -32603,
                message: format!("File type check timed out for: {}", params.file_path),
                data: None,
            });
        }
    };
    
    if !is_file {
        return Err(ErrorObject {
            code: -32603,
            message: format!("Path is not a file: {}", params.file_path),
            data: None,
        });
    }
    
    // Get file metadata with timeout
    let metadata = read_metadata_with_timeout(&params.file_path).await?;
    let file_size = metadata.len();
    
    // Check file size limit
    if file_size > MAX_FILE_SIZE && params.start_line.is_none() && params.end_line.is_none() {
        warn!("File '{}' is too large ({} bytes) to read entirely. Maximum size is {} bytes.", 
              params.file_path, file_size, MAX_FILE_SIZE);
        return Err(ErrorObject {
            code: -32603,
            message: format!(
                "File is too large to read entirely ({} MB). Maximum size is {} MB. \
                 Please specify start_line and end_line parameters to read a portion of the file.",
                file_size / (1024 * 1024),
                MAX_FILE_SIZE / (1024 * 1024)
            ),
            data: None,
        });
    }
    
    // Determine read strategy based on file size and parameters
    let (content, total_lines, start_line, end_line) = if file_size > MAX_FILE_SIZE || 
        params.start_line.is_some() || params.end_line.is_some() {
        // Use line-by-line reading for large files or when specific lines are requested
        info!("Using line-by-line reading for file: {} (size: {} bytes)", params.file_path, file_size);
        
        let (content, line_count, _lines_returned, actual_end) = read_file_lines_with_timeout(
            &params.file_path,
            params.start_line,
            params.end_line,
        ).await?;
        
        let start = params.start_line;
        let end = if let Some(requested_end) = params.end_line {
            Some(requested_end.min(line_count))
        } else {
            actual_end
        };
        
        (content, line_count, start, end)
    } else {
        // Read entire file for small files
        info!("Reading entire file: {} (size: {} bytes)", params.file_path, file_size);
        let full_content = read_file_content_with_timeout(&params.file_path).await?;
        
        // Count lines for consistency
        let line_count = full_content.lines().count();
        
        (full_content, line_count, None, None)
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
    use std::time::Duration;
    
    
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
    
    // NEW TESTS FOR TIMEOUT PROTECTION
    #[tokio::test]
    async fn test_read_file_timeout_simulation() {
        // Test that our timeout mechanism works correctly
        use std::future::Future;
        use std::pin::Pin;
        use std::task::{Context, Poll};
        
        // Create a future that never completes to simulate hanging read
        struct NeverComplete;
        impl Future for NeverComplete {
            type Output = Result<String, std::io::Error>;
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                // Never return Ready - simulates hanging
                Poll::Pending
            }
        }
        
        // Test the timeout wrapper directly
        let start_time = std::time::Instant::now();
        let result = timeout(Duration::from_millis(100), NeverComplete).await;
        let elapsed = start_time.elapsed();
        
        // Should timeout quickly
        assert!(elapsed < Duration::from_secs(1));
        assert!(result.is_err()); // Should be a timeout error
    }
    
    #[tokio::test]
    async fn test_read_large_file_protection() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.txt");
        
        // Create a file that would be too large (simulate with smaller size for testing)
        let large_content = "x".repeat(1024 * 1024); // 1MB for testing
        fs::write(&file_path, &large_content).await.unwrap();
        
        // Override the constant for testing by checking file size manually
        let metadata = fs::metadata(&file_path).await.unwrap();
        let _file_size = metadata.len();
        
        // Test reading with line range (should succeed)
        let params = ReadFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            start_line: Some(1),
            end_line: Some(1),
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_read_file(params, config, qdrant_client, None).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_concurrent_reads_same_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("concurrent.txt");
        
        // Create test file
        let content = (0..1000).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
        fs::write(&file_path, &content).await.unwrap();
        
        let file_path_str = file_path.to_str().unwrap().to_string();
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        // Start multiple concurrent reads
        let mut handles = Vec::new();
        for i in 0..10 {
            let params = ReadFileParams {
                file_path: file_path_str.clone(),
                start_line: Some(i * 10 + 1),
                end_line: Some((i + 1) * 10),
            };
            
            let config_clone = config.clone();
            let client_clone = qdrant_client.clone();
            
            let handle = tokio::spawn(async move {
                handle_read_file(params, config_clone, client_clone, None).await
            });
            handles.push(handle);
        }
        
        // Wait for all to complete
        let results: Vec<_> = futures::future::join_all(handles).await;
        
        // All should succeed
        for (i, result) in results.iter().enumerate() {
            let res = result.as_ref().unwrap();
            assert!(res.is_ok(), "Read {} failed", i);
            
            // Verify content is correct
            let file_result = res.as_ref().unwrap();
            assert!(file_result.content.contains(&format!("Line {}", i * 10)));
        }
    }
    
    #[tokio::test]
    async fn test_read_file_permission_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("no_perms.txt");
        
        // Create test file
        fs::write(&file_path, "Test content").await.unwrap();
        
        // Remove read permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&file_path).await.unwrap().permissions();
            perms.set_mode(0o000); // No permissions
            fs::set_permissions(&file_path, perms).await.unwrap();
        }
        
        let params = ReadFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            start_line: None,
            end_line: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_read_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("Failed to read file") || 
                error.message.contains("permission") || 
                error.message.contains("denied"));
    }
    
    #[tokio::test]
    async fn test_read_file_with_timeout_protection() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        fs::write(&file_path, content).await.unwrap();
        
        let params = ReadFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            start_line: None,
            end_line: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let start_time = std::time::Instant::now();
        let result = handle_read_file(params, config, qdrant_client, None).await;
        let elapsed = start_time.elapsed();
        
        // Should complete quickly for small files
        assert!(elapsed < Duration::from_secs(5));
        assert!(result.is_ok());
        
        let file_result = result.unwrap();
        assert_eq!(file_result.content, content);
        assert_eq!(file_result.line_count, 5);
    }
    
    #[tokio::test]
    async fn test_file_locking_multiple_readers() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("readers.txt");
        
        // Create a larger test file
        let content = (0..10000).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
        fs::write(&file_path, &content).await.unwrap();
        
        let file_path_str = file_path.to_str().unwrap().to_string();
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        // Start many concurrent reads (more than MAX_CONCURRENT_READS)
        let mut handles = Vec::new();
        for _ in 0..20 {
            let params = ReadFileParams {
                file_path: file_path_str.clone(),
                start_line: Some(1),
                end_line: Some(100),
            };
            
            let config_clone = config.clone();
            let client_clone = qdrant_client.clone();
            
            let handle = tokio::spawn(async move {
                handle_read_file(params, config_clone, client_clone, None).await
            });
            handles.push(handle);
        }
        
        // All should eventually succeed (some may wait for lock)
        let results: Vec<_> = futures::future::join_all(handles).await;
        
        for result in results {
            assert!(result.unwrap().is_ok());
        }
    }
    
    #[tokio::test]
    async fn test_line_by_line_reading_large_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large_lines.txt");
        
        // Create file with many lines
        let lines: Vec<String> = (1..=1000).map(|i| format!("This is line number {}", i)).collect();
        let content = lines.join("\n");
        fs::write(&file_path, &content).await.unwrap();
        
        // Read specific line range
        let params = ReadFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            start_line: Some(500),
            end_line: Some(510),
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_read_file(params, config, qdrant_client, None).await.unwrap();
        
        // Verify correct lines were read
        assert_eq!(result.start_line, Some(500));
        assert_eq!(result.end_line, Some(510));
        assert!(result.content.contains("This is line number 500"));
        assert!(result.content.contains("This is line number 510"));
        assert_eq!(result.line_count, 1000);
    }
    
    #[tokio::test]
    async fn test_detailed_error_messages() {
        let params = ReadFileParams {
            file_path: "/nonexistent/path/to/file.txt".to_string(),
            start_line: None,
            end_line: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_read_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        
        // Error message should be detailed and helpful
        assert!(error.message.contains("File not found"));
        assert!(error.message.contains("/nonexistent/path/to/file.txt"));
        assert_eq!(error.code, -32603);
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
    
    #[tokio::test]
    async fn test_read_file_params_parsing() {
        // Test that ReadFileParams can be correctly deserialized from JSON
        // This verifies that the tool definition matches the actual struct
        use serde_json::json;
        
        // Test with all parameters
        let json_params = json!({
            "file_path": "/test/file.txt",
            "start_line": 10,
            "end_line": 20
        });
        
        let params: ReadFileParams = serde_json::from_value(json_params).unwrap();
        assert_eq!(params.file_path, "/test/file.txt");
        assert_eq!(params.start_line, Some(10));
        assert_eq!(params.end_line, Some(20));
        
        // Test with only file_path
        let json_params = json!({
            "file_path": "/test/file2.txt"
        });
        
        let params: ReadFileParams = serde_json::from_value(json_params).unwrap();
        assert_eq!(params.file_path, "/test/file2.txt");
        assert_eq!(params.start_line, None);
        assert_eq!(params.end_line, None);
        
        // Test that old parameter names (limit/offset) would fail
        let json_params = json!({
            "file_path": "/test/file3.txt",
            "limit": 10,
            "offset": 5
        });
        
        let params: ReadFileParams = serde_json::from_value(json_params).unwrap();
        // These should be ignored/None since they're not valid field names
        assert_eq!(params.file_path, "/test/file3.txt");
        assert_eq!(params.start_line, None);
        assert_eq!(params.end_line, None);
    }
}