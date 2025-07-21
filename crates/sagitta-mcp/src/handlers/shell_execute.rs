use crate::mcp::types::{ShellExecuteParams, ShellExecuteResult, ErrorObject};
use crate::middleware::auth_middleware::AuthenticatedUser;
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use axum::Extension;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use std::time::Instant;
use std::process::Stdio;
use std::path::PathBuf;

/// Get the appropriate shell command based on the OS
fn get_shell_command(command: &str) -> (String, Vec<String>) {
    if cfg!(target_os = "windows") {
        // Windows: Use cmd.exe
        ("cmd".to_string(), vec!["/C".to_string(), command.to_string()])
    } else {
        // Unix-like (Linux, macOS): Use sh
        ("sh".to_string(), vec!["-c".to_string(), command.to_string()])
    }
}

/// Get the current repository working directory from state file
async fn get_current_repository_path() -> Option<PathBuf> {
    // Try to read from a state file in the config directory
    let mut state_path = dirs::config_dir()?;
    state_path.push("sagitta-code");
    state_path.push("current_repository.txt");
    
    match tokio::fs::read_to_string(&state_path).await {
        Ok(content) => {
            let path_str = content.trim();
            if !path_str.is_empty() {
                let path = PathBuf::from(path_str);
                if path.exists() && path.is_dir() {
                    log::debug!("Read current repository path from state file: {}", path.display());
                    return Some(path);
                }
            }
        }
        Err(e) => {
            log::trace!("Could not read repository state file: {e}");
        }
    }
    
    // Fallback to environment variable
    if let Ok(repo_path) = std::env::var("SAGITTA_CURRENT_REPO_PATH") {
        let path = PathBuf::from(repo_path);
        if path.exists() && path.is_dir() {
            log::debug!("Using repository path from environment: {}", path.display());
            return Some(path);
        }
    }
    
    None
}

/// Handler for shell command execution
pub async fn handle_shell_execute<C: QdrantClientTrait + Send + Sync + 'static>(
    params: ShellExecuteParams,
    _config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<ShellExecuteResult, ErrorObject> {
    let start_time = Instant::now();
    
    // Get the appropriate shell command for the OS
    let (shell, args) = get_shell_command(&params.command);
    
    // Create the command
    let mut cmd = Command::new(&shell);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null()); // Don't allow stdin to avoid hanging
    
    // Set working directory if specified, or use current repository path
    if let Some(ref dir) = params.working_directory {
        cmd.current_dir(dir);
    } else if let Some(repo_path) = get_current_repository_path().await {
        // Use the current repository path if no specific directory provided
        cmd.current_dir(&repo_path);
        log::info!("Using current repository as working directory: {}", repo_path.display());
    }
    
    // Set environment variables if specified
    if let Some(ref env_vars) = params.env {
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
    }
    
    // Execute with timeout
    let timeout_duration = Duration::from_millis(params.timeout_ms);
    let result = timeout(timeout_duration, cmd.output()).await;
    
    let execution_time_ms = start_time.elapsed().as_millis() as u64;
    
    match result {
        Ok(Ok(output)) => {
            // Command completed successfully
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);
            
            Ok(ShellExecuteResult {
                command: params.command,
                exit_code,
                stdout,
                stderr,
                execution_time_ms,
                timed_out: false,
            })
        }
        Ok(Err(e)) => {
            // Command failed to execute
            Err(ErrorObject {
                code: -32603,
                message: format!("Failed to execute command: {e}"),
                data: None,
            })
        }
        Err(_) => {
            // Command timed out
            Ok(ShellExecuteResult {
                command: params.command,
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Command timed out after {} ms", params.timeout_ms),
                execution_time_ms,
                timed_out: true,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn create_mock_qdrant() -> Arc<qdrant_client::Qdrant> {
        Arc::new(qdrant_client::Qdrant::from_url("http://localhost:6334").build().unwrap())
    }
    
    #[tokio::test]
    async fn test_shell_execute_simple_command() {
        let params = ShellExecuteParams {
            command: if cfg!(target_os = "windows") {
                "echo Hello World".to_string()
            } else {
                "echo 'Hello World'".to_string()
            },
            working_directory: None,
            timeout_ms: 5000,
            env: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_shell_execute(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("Hello World"));
        assert!(!result.timed_out);
    }
    
    #[tokio::test]
    async fn test_shell_execute_with_working_directory() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_str().unwrap().to_string();
        
        let params = ShellExecuteParams {
            command: if cfg!(target_os = "windows") {
                "cd".to_string()  // Print current directory on Windows
            } else {
                "pwd".to_string() // Print working directory on Unix
            },
            working_directory: Some(temp_path.clone()),
            timeout_ms: 5000,
            env: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_shell_execute(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.exit_code, 0);
        // Normalize paths for comparison
        let stdout = result.stdout.trim().replace('\\', "/");
        let expected = temp_path.replace('\\', "/");
        assert!(stdout.contains(&expected) || stdout == expected);
    }
    
    #[tokio::test]
    async fn test_shell_execute_with_repository_state_file() {
        use tokio::fs;
        
        // Clean up any existing state file from previous tests
        if let Some(mut state_path) = dirs::config_dir() {
            state_path.push("sagitta-code");
            state_path.push("current_repository.txt");
            let _ = fs::remove_file(&state_path).await;
        }
        
        // Create a temporary directory to act as the repository
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_str().unwrap().to_string();
        
        // Set environment variable instead of writing to real config directory
        std::env::set_var("SAGITTA_CURRENT_REPO_PATH", &repo_path);
        
        // Execute command without specifying working directory
        let params = ShellExecuteParams {
            command: if cfg!(target_os = "windows") {
                "cd".to_string()
            } else {
                "pwd".to_string()
            },
            working_directory: None, // Not specified
            timeout_ms: 5000,
            env: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_shell_execute(params, config, qdrant_client, None).await.unwrap();
        
        // Clean up environment variable
        std::env::remove_var("SAGITTA_CURRENT_REPO_PATH");
        
        assert_eq!(result.exit_code, 0);
        // Should use repository path from environment variable
        let stdout = result.stdout.trim().replace('\\', "/");
        let expected = repo_path.replace('\\', "/");
        assert!(stdout.contains(&expected) || stdout == expected);
    }
    
    #[tokio::test]
    async fn test_shell_execute_bug_reproduction() {
        use tokio::fs;
        
        // This test verifies that commands execute in the correct directory
        
        // Create a temporary directory to act as the repository
        let repo_dir = TempDir::new().unwrap();
        let repo_path = repo_dir.path().to_str().unwrap().to_string();
        
        // Create a test file in the repository
        let test_file_path = repo_dir.path().join("test_file.txt");
        fs::write(&test_file_path, "This is in the repository").await.unwrap();
        
        // Execute a command that lists files with explicit working directory
        let params = ShellExecuteParams {
            command: if cfg!(target_os = "windows") {
                "dir /b".to_string()
            } else {
                "ls".to_string()
            },
            working_directory: Some(repo_path.clone()), // Explicitly specify to avoid test interference
            timeout_ms: 5000,
            env: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_shell_execute(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.exit_code, 0);
        // Should see the test file if working directory is correctly set
        assert!(result.stdout.contains("test_file.txt"), 
            "Expected to find test_file.txt in output, but got: {}", result.stdout);
    }
    
    #[tokio::test]
    async fn test_shell_execute_with_error() {
        let params = ShellExecuteParams {
            command: if cfg!(target_os = "windows") {
                "cmd /c exit 1".to_string()
            } else {
                "sh -c 'exit 1'".to_string()
            },
            working_directory: None,
            timeout_ms: 5000,
            env: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_shell_execute(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.exit_code, 1);
        assert!(!result.timed_out);
    }
    
    #[tokio::test]
    async fn test_shell_execute_timeout() {
        let params = ShellExecuteParams {
            command: if cfg!(target_os = "windows") {
                "timeout /t 5 /nobreak".to_string()  // Windows sleep
            } else {
                "sleep 5".to_string()  // Unix sleep
            },
            working_directory: None,
            timeout_ms: 100, // Very short timeout
            env: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_shell_execute(params, config, qdrant_client, None).await.unwrap();
        
        assert!(result.timed_out);
        assert_eq!(result.exit_code, -1);
        assert!(result.stderr.contains("timed out"));
    }
    
    #[tokio::test]
    async fn test_shell_execute_with_env_vars() {
        let mut env = std::collections::HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());
        
        let params = ShellExecuteParams {
            command: if cfg!(target_os = "windows") {
                "echo %TEST_VAR%".to_string()
            } else {
                "echo $TEST_VAR".to_string()
            },
            working_directory: None,
            timeout_ms: 5000,
            env: Some(env),
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_shell_execute(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("test_value"));
    }
    
    #[tokio::test]
    async fn test_shell_execute_stderr_capture() {
        let params = ShellExecuteParams {
            command: if cfg!(target_os = "windows") {
                "cmd /c echo Error message 1>&2".to_string()
            } else {
                "echo 'Error message' >&2".to_string()
            },
            working_directory: None,
            timeout_ms: 5000,
            env: None,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_shell_execute(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.exit_code, 0);
        assert!(result.stderr.contains("Error message"));
    }
}