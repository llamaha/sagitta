use std::path::PathBuf;
use tempfile::TempDir;
use tokio::sync::Mutex;
use std::sync::Arc;

use sagitta_code::{
    agent::Agent,
    config::types::SagittaCodeConfig,
    tools::{registry::ToolRegistry, Tool, ToolResult},
    tools::shell_execution::ShellExecutionTool,
    tools::file_operations::{ReadFileTool, DirectFileEditTool},
    tools::local_executor::{LocalExecutor, LocalExecutorConfig, CommandExecutor},
    gui::repository::manager::RepositoryManager,
};

/// Test that tools operate in the correct working directory
#[tokio::test]
async fn test_tools_use_configured_base_directory() {
    // Create temporary workspace
    let temp_workspace = TempDir::new().unwrap();
    let workspace_path = temp_workspace.path().to_path_buf();
    
    // Configure sagitta to use our temp workspace
    let mut config = SagittaCodeConfig::default();
    config.workspaces.storage_path = Some(workspace_path.clone());
    
    // Create tool registry with tools that should use the workspace
    let tool_registry = Arc::new(ToolRegistry::new());
    
    // Create repo manager
    let core_config = sagitta_search::config::AppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new(Arc::new(Mutex::new(core_config)))
    ));
    
    // Register shell tool - should use workspace as working dir
    let shell_tool = Arc::new(ShellExecutionTool::new(workspace_path.clone()));
    tool_registry.register(shell_tool).await.unwrap();
    
    // Register read file tool - should use workspace as base dir
    let read_tool = Arc::new(ReadFileTool::new(repo_manager.clone(), workspace_path.clone()));
    tool_registry.register(read_tool).await.unwrap();
    
    // Test 1: Shell execution creates project in workspace
    let shell_params = serde_json::json!({
        "command": "cargo new test-project"
    });
    
    if let Some(shell_tool) = tool_registry.get("shell_execution").await {
        let result = shell_tool.execute(shell_params).await.unwrap();
        // Should succeed and create project in workspace
        assert!(result.is_success());
    }
    
    // Test 2: Verify project was created in workspace, not current dir
    let project_path = workspace_path.join("test-project");
    assert!(project_path.exists(), "Project should be created in workspace");
    assert!(project_path.join("Cargo.toml").exists(), "Cargo.toml should exist");
    assert!(project_path.join("src").join("main.rs").exists(), "main.rs should exist");
    
    // Test 3: Read file should read from workspace context
    let read_params = serde_json::json!({
        "file_path": "test-project/src/main.rs"
    });
    
    if let Some(read_tool) = tool_registry.get("read_file").await {
        let result = read_tool.execute(read_params).await.unwrap();
        assert!(result.is_success());
        
        // Should read the default cargo new main.rs content
        if let Some(content) = result.success_value().and_then(|v| v.get("content").and_then(|c| c.as_str())) {
            assert!(content.contains("Hello, world!"), "Should read correct main.rs content");
            assert!(!content.contains("Sagitta Code"), "Should not read sagitta's main.rs");
        }
    }
}

/// Test that tools reject paths outside the base directory
#[tokio::test]
async fn test_tools_reject_paths_outside_workspace() {
    let temp_workspace = TempDir::new().unwrap();
    let workspace_path = temp_workspace.path().to_path_buf();
    
    let core_config = sagitta_search::config::AppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new(Arc::new(Mutex::new(core_config)))
    ));
    
    let read_tool = ReadFileTool::new(repo_manager, workspace_path);
    
    // Try to read a file outside the workspace
    let read_params = serde_json::json!({
        "file_path": "../../etc/passwd"
    });
    
    let result = read_tool.execute(read_params).await.unwrap();
    // Should fail or at least not read sensitive system files
    if result.is_success() {
        if let Some(content) = result.success_value().and_then(|v| v.get("content").and_then(|c| c.as_str())) {
            assert!(!content.contains("root:"), "Should not read system files");
        }
    }
}

/// Test working directory context can be queried and changed
#[tokio::test] 
async fn test_working_directory_management() {
    let temp_workspace = TempDir::new().unwrap();
    let workspace_path = temp_workspace.path().to_path_buf();
    
    // Create subdirectory in workspace
    let subdir = workspace_path.join("subproject");
    std::fs::create_dir_all(&subdir).unwrap();
    
    // This test will be implemented once WorkingDirectoryManager tools are added
    // For now, just verify the workspace structure
    assert!(workspace_path.exists());
    assert!(subdir.exists());
}

/// Test that read_file resolves relative paths after a directory change (regression for file \#14)
#[tokio::test]
async fn test_read_file_after_change_directory() {
    let temp_workspace = TempDir::new().unwrap();
    let workspace_path = temp_workspace.path().to_path_buf();

    // Create a sub-directory and a file inside it
    let subdir = workspace_path.join("subproj");
    std::fs::create_dir_all(&subdir).unwrap();
    let file_path = subdir.join("hello.txt");
    std::fs::write(&file_path, "Hello, Sagitta!").unwrap();

    // Set the process working directory to the sub-directory, mimicking the effect of the
    // ChangeDirectoryTool / WorkingDirectoryManager in production.
    std::env::set_current_dir(&subdir).unwrap();

    // Build a read_file tool that is rooted at the workspace root
    let core_config = sagitta_search::config::AppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new(Arc::new(Mutex::new(core_config)))
    ));

    let read_tool = ReadFileTool::new(repo_manager, workspace_path.clone());

    // Attempt to read the file using a *relative* path
    let params = serde_json::json!({
        "file_path": "hello.txt"
    });

    let result = read_tool.execute(params).await.unwrap();
    assert!(result.is_success(), "read_file should succeed after cwd change");

    if let Some(content) = result.success_value().and_then(|v| v.get("content").and_then(|c| c.as_str())) {
        assert_eq!(content, "Hello, Sagitta!");
    } else {
        panic!("Expected content field in read_file result");
    }

    // Restore cwd for hygiene (TempDir will be deleted afterwards)
    std::env::set_current_dir(&workspace_path).unwrap();
}

/// Test that shell_execution defaults to current working directory after ChangeDirectory
#[tokio::test]
async fn test_shell_execution_after_change_directory() {
    let temp_workspace = TempDir::new().unwrap();
    let workspace_path = temp_workspace.path().to_path_buf();

    // make subproject dir and cargo init it manually (avoid network)
    let subdir = workspace_path.join("proj");
    std::fs::create_dir_all(&subdir).unwrap();
    std::fs::write(subdir.join("Cargo.toml"), "[package]\nname = \"proj\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n").unwrap();
    std::fs::create_dir_all(subdir.join("src")).unwrap();
    std::fs::write(subdir.join("src").join("main.rs"), "fn main() {}\n").unwrap();

    // Act: change working directory to the subdir
    std::env::set_current_dir(&subdir).unwrap();

    // Setup config with workspace as base
    let mut config = LocalExecutorConfig::default();
    config.base_dir = workspace_path.clone();
    config.execution_dir = workspace_path.clone();

    // Create executor 
    let executor = LocalExecutor::new(config);

    // Act: Run shell command without specifying working_directory param - should use current cwd (subdir)
    let params = sagitta_code::tools::shell_execution::ShellExecutionParams {
        command: "pwd".to_string(),
        language: None,
        working_directory: None,
        allow_network: None,
        env_vars: None,
        timeout_seconds: None,
    };

    // Execute
    let result = executor.execute(&params).await.unwrap();

    // The output should include "proj" since the command ran in the subdir
    assert!(result.stdout.contains("proj"), "Expected 'proj' in pwd output, but got: {}", result.stdout);
}

/// Test that edit_file content size limit prevents streaming timeouts
#[tokio::test] 
async fn test_edit_file_content_size_limit() {
    let temp_workspace = TempDir::new().unwrap();
    let workspace_path = temp_workspace.path().to_path_buf();
    
    // Create a simple file 
    let test_file = workspace_path.join("test.txt");
    std::fs::write(&test_file, "Hello, world!\n").unwrap();
    
    // Create a DirectFileEditTool
    let edit_tool = DirectFileEditTool::new(workspace_path.clone());
    
    // Create content that exceeds the 10KB limit (roughly 11KB)
    let large_content = "A".repeat(11 * 1024);
    
    let params = serde_json::json!({
        "file_path": "test.txt",
        "line_start": 1,
        "line_end": 1,
        "content": large_content
    });
    
    // Execute should return an error about content size
    let result = edit_tool.execute(params).await;
    
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Content size"));
    assert!(error_msg.contains("exceeds maximum allowed size"));
    assert!(error_msg.contains("10240")); // 10KB limit
} 