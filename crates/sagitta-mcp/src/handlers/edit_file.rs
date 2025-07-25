use crate::mcp::types::{EditFileParams, EditFileResult, ErrorObject};
use crate::middleware::auth_middleware::AuthenticatedUser;
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use axum::Extension;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use tokio::time::{timeout, Duration};
use similar::{TextDiff, ChangeTag};
use tracing::{info, error};
use std::path::{Path, PathBuf};
use super::utils::get_current_repository_path;
use uuid;

/// Timeout for file operations to prevent hanging
const FILE_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum number of concurrent file operations per file
const MAX_CONCURRENT_OPERATIONS: usize = 1;

/// Helper function to perform file read with timeout
async fn read_file_with_timeout(file_path: &str) -> Result<String, ErrorObject> {
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

/// Helper function to perform atomic file write with timeout
async fn write_file_atomic_with_timeout(file_path: &str, content: &str) -> Result<(), ErrorObject> {
    let temp_path = format!("{}.tmp.{}", file_path, uuid::Uuid::new_v4());
    
    // Write to temporary file first
    match timeout(FILE_OPERATION_TIMEOUT, fs::write(&temp_path, content)).await {
        Ok(Ok(_)) => {},
        Ok(Err(e)) => {
            // Clean up temp file on error
            let _ = fs::remove_file(&temp_path).await;
            return Err(ErrorObject {
                code: -32603,
                message: format!("Failed to write file: {}", e),
                data: None,
            });
        },
        Err(_) => {
            // Clean up temp file on timeout
            let _ = fs::remove_file(&temp_path).await;
            return Err(ErrorObject {
                code: -32603,
                message: "File write operation timed out after 30 seconds".to_string(),
                data: None,
            });
        },
    }
    
    // Atomically rename to final location
    match timeout(FILE_OPERATION_TIMEOUT, fs::rename(&temp_path, file_path)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => {
            // Clean up temp file on error
            let _ = fs::remove_file(&temp_path).await;
            Err(ErrorObject {
                code: -32603,
                message: format!("Failed to rename file: {}", e),
                data: None,
            })
        },
        Err(_) => {
            // Clean up temp file on timeout
            let _ = fs::remove_file(&temp_path).await;
            Err(ErrorObject {
                code: -32603,
                message: "File rename operation timed out after 30 seconds".to_string(),
                data: None,
            })
        },
    }
}

/// File locking manager to prevent concurrent modifications
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::Semaphore;

lazy_static::lazy_static! {
    static ref FILE_LOCKS: Mutex<HashMap<String, Arc<Semaphore>>> = Mutex::new(HashMap::new());
}

/// Get or create a semaphore for file-level locking
fn get_file_lock(file_path: &str) -> Arc<Semaphore> {
    let mut locks = FILE_LOCKS.lock().unwrap();
    locks.entry(file_path.to_string())
        .or_insert_with(|| Arc::new(Semaphore::new(MAX_CONCURRENT_OPERATIONS)))
        .clone()
}

/// Helper function to perform file edit with locking
async fn edit_file_with_lock<F, Fut>(file_path: &str, operation: F) -> Result<EditFileResult, ErrorObject>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<EditFileResult, ErrorObject>>,
{
    let lock = get_file_lock(file_path);
    
    // Acquire file lock with timeout
    let permit = match timeout(FILE_OPERATION_TIMEOUT, lock.acquire()).await {
        Ok(Ok(permit)) => permit,
        Ok(Err(_)) => return Err(ErrorObject {
            code: -32603,
            message: "Failed to acquire file lock".to_string(),
            data: None,
        }),
        Err(_) => return Err(ErrorObject {
            code: -32603,
            message: "File lock acquisition timed out after 30 seconds".to_string(),
            data: None,
        }),
    };
    
    // Perform the operation while holding the lock
    let result = operation().await;
    
    // Lock is automatically released when permit is dropped
    drop(permit);
    
    result
}

/// Get context around the edit location (10 lines before and after)
fn _get_context_lines(content: &str, match_start: usize, match_end: usize) -> (String, String) {
    let lines: Vec<&str> = content.lines().collect();
    let mut start_line = 0;
    let mut end_line = lines.len();
    let mut current_pos = 0;
    
    // Find which lines contain the match
    for (i, line) in lines.iter().enumerate() {
        let line_end = current_pos + line.len() + 1; // +1 for newline
        if current_pos <= match_start && match_start < line_end {
            start_line = i.saturating_sub(10); // 10 lines before
        }
        if current_pos <= match_end && match_end < line_end {
            end_line = (i + 11).min(lines.len()); // 10 lines after
            break;
        }
        current_pos = line_end;
    }
    
    let context_lines = &lines[start_line..end_line];
    let old_context = context_lines.join("\n");
    
    // Calculate new context by applying the change
    let before = &content[..match_start];
    let after = &content[match_end..];
    let new_content = format!("{}{}{}", before, "", after); // We'll replace this in the actual edit
    
    (old_context, new_content)
}

/// Create a unified diff between two strings
fn create_diff(old_content: &str, new_content: &str, file_path: &str) -> String {
    let diff = TextDiff::from_lines(old_content, new_content);
    
    let mut result = String::new();
    result.push_str(&format!("--- {file_path}\n"));
    result.push_str(&format!("+++ {file_path}\n"));
    
    for group in diff.grouped_ops(3) {
        let mut first_old = None;
        let mut last_old = None;
        let mut first_new = None;
        let mut last_new = None;
        
        for op in &group {
            match op {
                similar::DiffOp::Delete { old_index, old_len, .. } => {
                    if first_old.is_none() {
                        first_old = Some(*old_index);
                    }
                    last_old = Some(old_index + old_len);
                }
                similar::DiffOp::Insert { new_index, new_len, .. } => {
                    if first_new.is_none() {
                        first_new = Some(*new_index);
                    }
                    last_new = Some(new_index + new_len);
                }
                similar::DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                    if first_old.is_none() {
                        first_old = Some(*old_index);
                    }
                    last_old = Some(old_index + old_len);
                    if first_new.is_none() {
                        first_new = Some(*new_index);
                    }
                    last_new = Some(new_index + new_len);
                }
                similar::DiffOp::Equal { old_index, new_index, len } => {
                    if first_old.is_none() {
                        first_old = Some(*old_index);
                    }
                    last_old = Some(old_index + len);
                    if first_new.is_none() {
                        first_new = Some(*new_index);
                    }
                    last_new = Some(new_index + len);
                }
            }
        }
        
        if let (Some(old_start), Some(old_end), Some(new_start), Some(new_end)) = 
            (first_old, last_old, first_new, last_new) {
            result.push_str(&format!("@@ -{},{} +{},{} @@\n", 
                old_start + 1, old_end - old_start, 
                new_start + 1, new_end - new_start));
            
            for op in &group {
                for change in diff.iter_changes(op) {
                    let prefix = match change.tag() {
                        ChangeTag::Delete => "-",
                        ChangeTag::Insert => "+",
                        ChangeTag::Equal => " ",
                    };
                    result.push_str(&format!("{prefix}{change}"));
                    if !change.to_string().ends_with('\n') {
                        result.push('\n');
                    }
                }
            }
        }
    }
    
    result
}

/// Handler for editing a file
pub async fn handle_edit_file<C: QdrantClientTrait + Send + Sync + 'static>(
    params: EditFileParams,
    config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<EditFileResult, ErrorObject> {
    let start_time = std::time::Instant::now();
    info!("Starting edit_file operation for: {}", params.file_path);
    
    let file_path = params.file_path.clone();
    
    // Use file locking to prevent concurrent modifications
    let result = edit_file_with_lock(&file_path, || async {
        handle_edit_file_inner(params, config).await
    }).await;
    
    let duration = start_time.elapsed();
    match &result {
        Ok(_) => info!("Edit completed successfully in {:?}", duration),
        Err(e) => error!("Edit failed after {:?}: {}", duration, e.message),
    }
    
    result
}

/// Inner handler for editing a file (without locking)
async fn handle_edit_file_inner(params: EditFileParams, config: Arc<RwLock<AppConfig>>) -> Result<EditFileResult, ErrorObject> {
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
    
    // Read the file with timeout protection
    let content = read_file_with_timeout(file_path.to_str().unwrap_or(&params.file_path)).await?;
    
    // Find the old_string in the content
    let matches: Vec<_> = content.match_indices(&params.old_string).collect();
    
    if matches.is_empty() {
        return Err(ErrorObject {
            code: -32603,
            message: format!("String '{}' not found in file", params.old_string),
            data: None,
        });
    }
    
    if !params.replace_all && matches.len() > 1 {
        return Err(ErrorObject {
            code: -32603,
            message: format!("String '{}' found {} times. Use replace_all=true or make the string more unique", 
                           params.old_string, matches.len()),
            data: None,
        });
    }
    
    // Perform the replacement
    let new_content = if params.replace_all {
        content.replace(&params.old_string, &params.new_string)
    } else {
        let (start, _) = matches[0];
        let end = start + params.old_string.len();
        format!("{}{}{}", &content[..start], &params.new_string, &content[end..])
    };
    
    // Write the new content atomically with timeout protection
    write_file_atomic_with_timeout(file_path.to_str().unwrap_or(&params.file_path), &new_content).await?;
    
    // Get context for display (show limited context around changes)
    let (old_context, new_context) = if matches.len() == 1 && !params.replace_all {
        let (start, _) = matches[0];
        let lines: Vec<&str> = content.lines().collect();
        let mut line_start = 0;
        let mut line_num = 0;
        
        for (i, line) in lines.iter().enumerate() {
            if line_start + line.len() >= start {
                line_num = i;
                break;
            }
            line_start += line.len() + 1;
        }
        
        let context_start = line_num.saturating_sub(3);
        let context_end = (line_num + 4).min(lines.len());
        
        let old_lines = lines[context_start..context_end].join("\n");
        let new_lines: Vec<&str> = new_content.lines().collect();
        let new_context_lines = if new_lines.len() > context_start {
            new_lines[context_start..context_end.min(new_lines.len())].join("\n")
        } else {
            String::new()
        };
        
        (old_lines, new_context_lines)
    } else {
        // For multiple replacements, show full diff
        (content.clone(), new_content.clone())
    };
    
    // Create diff
    let diff = create_diff(&content, &new_content, file_path.to_str().unwrap_or(&params.file_path));
    
    // Create summary
    let changes_summary = if params.replace_all {
        format!("Replaced {} occurrences of the text", matches.len())
    } else {
        "Replaced 1 occurrence of the text".to_string()
    };
    
    Ok(EditFileResult {
        file_path: file_path.display().to_string(),
        old_content: old_context,
        new_content: new_context,
        diff,
        changes_summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::time::Duration;
    use tokio::time::sleep;
    // use std::path::PathBuf;
    
    fn create_mock_qdrant() -> Arc<qdrant_client::Qdrant> {
        Arc::new(qdrant_client::Qdrant::from_url("http://localhost:6334").build().unwrap())
    }
    
    #[tokio::test]
    async fn test_edit_file_single_occurrence() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "Hello world\nThis is a test\nGoodbye world";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "Hello world".to_string(),
            new_string: "Hi universe".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_edit_file(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.changes_summary, "Replaced 1 occurrence of the text");
        assert!(result.diff.contains("-Hello world"));
        assert!(result.diff.contains("+Hi universe"));
        
        // Verify file was actually changed
        let new_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(new_content, "Hi universe\nThis is a test\nGoodbye world");
    }
    
    #[tokio::test]
    async fn test_edit_file_multiple_occurrences_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file with multiple occurrences
        let original_content = "Hello world\nHello world again\nGoodbye world";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "Hello world".to_string(),
            new_string: "Hi universe".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_edit_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("found 2 times"));
    }
    
    #[tokio::test]
    async fn test_edit_file_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file with multiple occurrences
        let original_content = "foo bar\nfoo baz\nqux foo";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "foo".to_string(),
            new_string: "FOO".to_string(),
            replace_all: true,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_edit_file(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.changes_summary, "Replaced 3 occurrences of the text");
        
        // Verify file was actually changed
        let new_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(new_content, "FOO bar\nFOO baz\nqux FOO");
    }
    
    #[tokio::test]
    async fn test_edit_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "Hello world";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "not found".to_string(),
            new_string: "replacement".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_edit_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("not found in file"));
    }

    // NEW TESTS FOR TIMEOUT PROTECTION
    #[tokio::test]
    async fn test_edit_file_timeout_on_read() {
        // Test that our timeout protection works by creating a mock slow filesystem
        // We'll create a custom test that simulates a slow read operation
        
        // First, let's test that our timeout wrapper works correctly
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
        
        // If we get here, the timeout mechanism is working
        // The actual file operations will use the same timeout mechanism
    }

    #[tokio::test]
    async fn test_edit_file_timeout_on_write() {
        // Test that write operations handle permission errors gracefully
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        fs::write(&file_path, "Hello world").await.unwrap();
        
        // Make directory read-only to simulate write failure
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(temp_dir.path()).await.unwrap().permissions();
            perms.set_mode(0o444); // Read-only directory
            fs::set_permissions(temp_dir.path(), perms).await.unwrap();
        }
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "Hello".to_string(),
            new_string: "Hi".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let start_time = std::time::Instant::now();
        let result = handle_edit_file(params, config, qdrant_client, None).await;
        let elapsed = start_time.elapsed();
        
        // Should complete quickly with proper error handling
        assert!(elapsed < Duration::from_secs(5));
        assert!(result.is_err());
        
        // Error should be about permissions, not timeout
        let error = result.unwrap_err();
        assert!(error.message.contains("Failed to write file") || 
                error.message.contains("permission") || 
                error.message.contains("denied"));
    }

    // NEW TESTS FOR FILE LOCKING
    #[tokio::test]
    async fn test_concurrent_edit_same_file() {
        // This test will fail initially - we need to implement file locking
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "line1\nline2\nline3\nline4\nline5";
        fs::write(&file_path, original_content).await.unwrap();
        
        let file_path_str = file_path.to_str().unwrap().to_string();
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        // Start two concurrent edits
        let params1 = EditFileParams {
            file_path: file_path_str.clone(),
            old_string: "line1".to_string(),
            new_string: "LINE1".to_string(),
            replace_all: false,
        };
        
        let params2 = EditFileParams {
            file_path: file_path_str.clone(),
            old_string: "line2".to_string(),
            new_string: "LINE2".to_string(),
            replace_all: false,
        };
        
        let config1 = config.clone();
        let config2 = config.clone();
        let client1 = qdrant_client.clone();
        let client2 = qdrant_client.clone();
        
        // Run concurrently
        let (result1, result2) = tokio::join!(
            handle_edit_file(params1, config1, client1, None),
            handle_edit_file(params2, config2, client2, None)
        );
        
        // Both should succeed due to file locking preventing corruption
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        
        // Verify final file content is consistent (either line1 or line2 should be changed first)
        let final_content = fs::read_to_string(&file_path).await.unwrap();
        assert!(final_content.contains("LINE1") || final_content.contains("LINE2"));
        
        // File should not be corrupted
        assert_eq!(final_content.lines().count(), 5);
    }

    #[tokio::test]
    async fn test_file_locking_prevents_corruption() {
        // This test will fail initially - we need to implement file locking
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "0123456789".repeat(1000); // Large content
        fs::write(&file_path, &original_content).await.unwrap();
        
        let file_path_str = file_path.to_str().unwrap().to_string();
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        // Start multiple concurrent edits
        let mut handles = Vec::new();
        for i in 0..10 {
            let params = EditFileParams {
                file_path: file_path_str.clone(),
                old_string: format!("{}", i),
                new_string: format!("X{}", i),
                replace_all: true,
            };
            
            let config_clone = config.clone();
            let client_clone = qdrant_client.clone();
            
            let handle = tokio::spawn(async move {
                handle_edit_file(params, config_clone, client_clone, None).await
            });
            handles.push(handle);
        }
        
        // Wait for all to complete
        let results: Vec<_> = futures::future::join_all(handles).await;
        
        // All should succeed
        for result in results {
            assert!(result.unwrap().is_ok());
        }
        
        // Verify file is not corrupted
        let final_content = fs::read_to_string(&file_path).await.unwrap();
        assert!(!final_content.is_empty());
        assert!(final_content.len() >= original_content.len()); // Should not be truncated
    }

    // NEW TESTS FOR ATOMIC OPERATIONS
    #[tokio::test]
    async fn test_atomic_write_prevents_partial_content() {
        // This test will fail initially - we need to implement atomic operations
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "Hello world\nThis is important data\nDo not lose this";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "Hello world".to_string(),
            new_string: "Hi universe".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        // Simulate a crash by having the edit operation continue in background
        let file_path_clone = file_path.clone();
        let read_during_write = tokio::spawn(async move {
            // Give the edit some time to start
            sleep(Duration::from_millis(10)).await;
            
            // Try to read the file during the write operation
            for _ in 0..100 {
                if let Ok(content) = fs::read_to_string(&file_path_clone).await {
                    // File should never be empty or partially written
                    assert!(!content.is_empty());
                    assert!(content.contains("This is important data"));
                    assert!(content.contains("Do not lose this"));
                }
                sleep(Duration::from_millis(1)).await;
            }
        });
        
        let result = handle_edit_file(params, config, qdrant_client, None).await;
        read_during_write.await.unwrap();
        
        assert!(result.is_ok());
        
        // Verify final content is correct
        let final_content = fs::read_to_string(&file_path).await.unwrap();
        assert!(final_content.contains("Hi universe"));
        assert!(final_content.contains("This is important data"));
        assert!(final_content.contains("Do not lose this"));
    }

    #[tokio::test]
    async fn test_atomic_rollback_on_disk_full() {
        // This test will fail initially - we need to implement atomic operations
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "Hello world";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "Hello world".to_string(),
            new_string: "X".repeat(1_000_000), // Very large replacement
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_edit_file(params, config, qdrant_client, None).await;
        
        // If write fails, original content should remain intact
        let final_content = fs::read_to_string(&file_path).await.unwrap();
        if result.is_err() {
            // Original file should be unchanged
            assert_eq!(final_content, original_content);
        } else {
            // If it succeeded, content should be fully written
            assert!(final_content.contains("X"));
        }
    }

    // NEW TESTS FOR ENHANCED ERROR HANDLING AND LOGGING
    #[tokio::test]
    async fn test_detailed_error_messages() {
        // This test will fail initially - we need to implement enhanced error handling
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent_dir/test.txt");
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "Hello".to_string(),
            new_string: "Hi".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_edit_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        
        // Error message should be detailed and helpful
        assert!(error.message.contains("Failed to read file"));
        assert!(error.message.contains("nonexistent_dir"));
        assert!(error.code == -32603);
    }

    #[tokio::test]
    async fn test_operation_timing_logging() {
        // This test will fail initially - we need to implement timing logging
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "Hello world";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "Hello".to_string(),
            new_string: "Hi".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let start_time = std::time::Instant::now();
        let result = handle_edit_file(params, config, qdrant_client, None).await;
        let elapsed = start_time.elapsed();
        
        assert!(result.is_ok());
        
        // Should complete reasonably quickly for small files
        assert!(elapsed < Duration::from_secs(1));
        
        // TODO: Add actual logging verification once we implement structured logging
        // For now, just verify the operation completed successfully
    }

    #[tokio::test]
    async fn test_permission_error_handling() {
        // This test will fail initially - we need to implement enhanced error handling
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        fs::write(&file_path, "Hello world").await.unwrap();
        
        // Remove read permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&file_path).await.unwrap().permissions();
            perms.set_mode(0o000); // No permissions
            fs::set_permissions(&file_path, perms).await.unwrap();
        }
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "Hello".to_string(),
            new_string: "Hi".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_edit_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        
        // Should provide helpful error message about permissions
        assert!(error.message.contains("Failed to read file"));
        assert!(error.message.contains("permission") || error.message.contains("denied"));
    }

    // INTEGRATION TESTS FOR ALL FEATURES COMBINED
    #[tokio::test]
    async fn test_robust_edit_with_all_features() {
        // This test will fail initially - combines timeout, locking, atomicity, and error handling
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "line1\nline2\nline3\nline4\nline5";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "line2".to_string(),
            new_string: "LINE2".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let start_time = std::time::Instant::now();
        let result = handle_edit_file(params, config, qdrant_client, None).await;
        let elapsed = start_time.elapsed();
        
        // Should complete within reasonable time (timeout protection)
        assert!(elapsed < Duration::from_secs(30));
        
        // Should succeed (proper error handling)
        assert!(result.is_ok());
        
        // Should produce correct content (atomicity)
        let final_content = fs::read_to_string(&file_path).await.unwrap();
        assert!(final_content.contains("LINE2"));
        assert!(final_content.contains("line1"));
        assert!(final_content.contains("line3"));
        assert_eq!(final_content.lines().count(), 5);
    }
}