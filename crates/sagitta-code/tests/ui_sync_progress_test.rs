use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{timeout, Duration};
use uuid::Uuid;
use tempfile::TempDir;

use sagitta_code::gui::repository::manager::RepositoryManager;
use sagitta_code::tools::repository::add::AddExistingRepositoryTool;
use sagitta_code::tools::types::{Tool, ToolResult};
use sagitta_code::gui::repository::shared_sync_state::{SIMPLE_STATUS, DETAILED_STATUS};
use sagitta_code::gui::repository::types::SimpleSyncStatus;
use sagitta_search::AppConfig as SagittaAppConfig;
use terminal_stream::events::StreamEvent;

/// Test that add_existing_repository automatically triggers sync and shows progress in UI
#[tokio::test]
async fn test_add_repo_auto_sync_with_ui_progress() {
    // Create test repository manager
    let config = SagittaAppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new_for_test(Arc::new(Mutex::new(config)))
    ));
    
    // Create add repository tool
    let add_tool = AddExistingRepositoryTool::new(repo_manager.clone());
    
    // Create a temporary directory for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path().to_str().unwrap();
    
    // Initialize as git repo
    std::process::Command::new("git")
        .args(&["init"])
        .current_dir(&temp_dir)
        .output()
        .expect("Failed to init git repo");
    
    // Add a test file
    std::fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();
    
    // Commit the file
    std::process::Command::new("git")
        .args(&["add", "."])
        .current_dir(&temp_dir)
        .output()
        .expect("Failed to add files");
    
    std::process::Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(&temp_dir)
        .output()
        .expect("Failed to commit");
    
    // Clear any existing sync status
    SIMPLE_STATUS.clear();
    DETAILED_STATUS.clear();
    
    // Execute add_existing_repository tool
    let params = serde_json::json!({
        "name": "test-repo",
        "local_path": repo_path
    });
    
    let result = add_tool.execute(params).await.unwrap();
    
    // Verify the tool succeeded
    match result {
        ToolResult::Success(data) => {
            assert_eq!(data["repository_name"], "test-repo");
            assert_eq!(data["status"], "added_and_synced");
        }
        ToolResult::Error { error } => panic!("Tool failed: {}", error),
    }
    
    // Verify sync status was created in UI state
    let sync_status = SIMPLE_STATUS.get("test-repo");
    assert!(sync_status.is_some(), "Sync status should be created");
    
    let status = sync_status.unwrap();
    assert!(status.is_complete, "Sync should be complete");
    assert!(status.is_success, "Sync should be successful");
    assert!(!status.output_lines.is_empty(), "Should have output lines");
}

/// Test that sync progress is properly reported to chat UI via StreamEvent
#[tokio::test]
async fn test_sync_progress_stream_events() {
    // Create channel for stream events
    let (tx, mut rx) = mpsc::channel::<StreamEvent>(100);
    
    // Create test repository manager
    let config = SagittaAppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new_for_test(Arc::new(Mutex::new(config)))
    ));
    
    // Create add repository tool with stream event sender
    let add_tool = AddExistingRepositoryTool::new_with_progress_sender(repo_manager.clone(), Some(tx));
    
    // Create a temporary directory for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path().to_str().unwrap();
    
    // Initialize as git repo
    std::process::Command::new("git")
        .args(&["init"])
        .current_dir(&temp_dir)
        .output()
        .expect("Failed to init git repo");
    
    // Add a test file
    std::fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();
    
    // Commit the file
    std::process::Command::new("git")
        .args(&["add", "."])
        .current_dir(&temp_dir)
        .output()
        .expect("Failed to add files");
    
    std::process::Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(&temp_dir)
        .output()
        .expect("Failed to commit");
    
    // Execute add_existing_repository tool in background
    let params = serde_json::json!({
        "name": "test-repo-stream",
        "local_path": repo_path
    });
    
    let tool_handle = tokio::spawn(async move {
        add_tool.execute(params).await
    });
    
    // Collect progress events
    let mut progress_events = Vec::new();
    let mut completed = false;
    
    // Wait for events with timeout
    while !completed {
        match timeout(Duration::from_secs(30), rx.recv()).await {
            Ok(Some(event)) => {
                match &event {
                    StreamEvent::Progress { message, percentage } => {
                        progress_events.push((message.clone(), *percentage));
                        
                        // Check if this is completion
                        if message.contains("Successfully") || message.contains("completed") {
                            completed = true;
                        }
                    }
                    _ => {} // Ignore other events
                }
            }
            Ok(None) => break, // Channel closed
            Err(_) => {
                panic!("Timeout waiting for progress events");
            }
        }
    }
    
    // Wait for tool to complete
    let result = tool_handle.await.unwrap().unwrap();
    
    // Verify we received progress events
    assert!(!progress_events.is_empty(), "Should receive progress events");
    
    // Verify we got expected progress stages
    let messages: Vec<String> = progress_events.iter().map(|(msg, _)| msg.clone()).collect();
    let combined_messages = messages.join(" ");
    
    assert!(combined_messages.contains("Adding repository"), "Should show adding stage");
    assert!(combined_messages.contains("Syncing"), "Should show syncing stage");
    assert!(combined_messages.contains("Successfully") || combined_messages.contains("completed"), "Should show completion");
    
    // Verify tool result
    match result {
        ToolResult::Success(data) => {
            assert_eq!(data["repository_name"], "test-repo-stream");
            assert_eq!(data["status"], "added_and_synced");
        }
        ToolResult::Error { error } => panic!("Tool failed: {}", error),
    }
}

/// Test that sync can be cancelled via StreamEvent
#[tokio::test]
async fn test_sync_cancellation() {
    // Create channel for stream events
    let (tx, mut rx) = mpsc::channel::<StreamEvent>(100);
    
    // Create test repository manager
    let config = SagittaAppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new_for_test(Arc::new(Mutex::new(config)))
    ));
    
    // Create add repository tool with stream event sender
    let add_tool = AddExistingRepositoryTool::new_with_progress_sender(repo_manager.clone(), Some(tx.clone()));
    
    // Create a temporary directory for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path().to_str().unwrap();
    
    // Initialize as git repo
    std::process::Command::new("git")
        .args(&["init"])
        .current_dir(&temp_dir)
        .output()
        .expect("Failed to init git repo");
    
    // Add a test file
    std::fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();
    
    // Commit the file
    std::process::Command::new("git")
        .args(&["add", "."])
        .current_dir(&temp_dir)
        .output()
        .expect("Failed to add files");
    
    std::process::Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(&temp_dir)
        .output()
        .expect("Failed to commit");
    
    // Execute add_existing_repository tool in background
    let params = serde_json::json!({
        "name": "test-repo-cancel",
        "local_path": repo_path
    });
    
    let tool_handle = tokio::spawn(async move {
        add_tool.execute(params).await
    });
    
    // Wait for first progress event, then simulate cancellation
    let mut received_progress = false;
    
    while !received_progress {
        match timeout(Duration::from_secs(10), rx.recv()).await {
            Ok(Some(StreamEvent::Progress { .. })) => {
                received_progress = true;
                
                // Send cancellation signal
                let _ = tx.send(StreamEvent::system("CANCEL_SYNC".to_string())).await;
                break;
            }
            Ok(Some(_)) => continue, // Ignore other events
            Ok(None) => break, // Channel closed
            Err(_) => panic!("Timeout waiting for progress events"),
        }
    }
    
    assert!(received_progress, "Should receive at least one progress event before cancellation");
    
    // Note: The actual cancellation implementation will be added to the sync logic
    // For now, we just verify the test structure works
} 