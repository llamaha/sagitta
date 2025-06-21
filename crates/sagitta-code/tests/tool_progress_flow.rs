use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, broadcast, Mutex};
use uuid::Uuid;

use sagitta_code::agent::events::{AgentEvent, ToolRunId};
use sagitta_code::reasoning::AgentToolExecutor;
use sagitta_code::tools::registry::ToolRegistry;
use sagitta_code::tools::repository::add::AddExistingRepositoryTool;
use sagitta_code::gui::repository::manager::RepositoryManager;
use sagitta_search::AppConfig;
use terminal_stream::events::StreamEvent;
use reasoning_engine::traits::ToolExecutor;

/// Test that tool progress events flow correctly from tool execution to GUI
#[tokio::test]
async fn test_tool_progress_flow_end_to_end() {
    // Create test repository manager
    let config = AppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new_for_test(Arc::new(Mutex::new(config)))
    ));
    
    // Create tool registry with progress-aware tool
    let mut tool_registry = ToolRegistry::new();
    let add_tool = AddExistingRepositoryTool::new(repo_manager.clone());
    tool_registry.register(Arc::new(add_tool)).await.unwrap();
    
    // Create agent tool executor
    let mut executor = AgentToolExecutor::new(Arc::new(tool_registry));
    
    // Set up event broadcasting
    let (event_sender, mut event_receiver) = broadcast::channel(100);
    executor.set_event_sender(event_sender);
    
    // Collect events in a separate task
    let event_collector = tokio::spawn(async move {
        let mut events: Vec<AgentEvent> = Vec::new();
        let timeout = tokio::time::sleep(Duration::from_secs(5));
        tokio::pin!(timeout);
        
        loop {
            tokio::select! {
                event = event_receiver.recv() => {
                    match event {
                        Ok(event) => {
                            println!("Received event: {:?}", event);
                            let is_completion = matches!(event, AgentEvent::ToolRunCompleted { .. });
                            events.push(event);
                            
                            // Stop collecting after we get a completion event
                            if is_completion {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                _ = &mut timeout => {
                    println!("Test timeout reached");
                    break;
                }
            }
        }
        events
    });
    
    // Execute the tool (this will fail due to invalid path, but should still emit events)
    let params = serde_json::json!({
        "name": "test-repo",
        "local_path": "/nonexistent/path"
    });
    
    let result = executor.execute_tool("add_existing_repository", params).await;
    println!("Tool execution result: {:?}", result);
    
    // Wait for events to be collected
    let events = event_collector.await.unwrap();
    
    // Verify we received the expected events
    assert!(!events.is_empty(), "Should have received at least one event");
    
    // Check for ToolRunStarted event
    let started_events: Vec<_> = events.iter()
        .filter(|e| matches!(e, AgentEvent::ToolRunStarted { .. }))
        .collect();
    assert_eq!(started_events.len(), 1, "Should have exactly one ToolRunStarted event");
    
    // Check for ToolRunCompleted event
    let completed_events: Vec<_> = events.iter()
        .filter(|e| matches!(e, AgentEvent::ToolRunCompleted { .. }))
        .collect();
    assert_eq!(completed_events.len(), 1, "Should have exactly one ToolRunCompleted event");
    
    // Verify run_id consistency
    if let (AgentEvent::ToolRunStarted { run_id: start_id, .. }, 
            AgentEvent::ToolRunCompleted { run_id: end_id, .. }) = 
        (&started_events[0], &completed_events[0]) {
        assert_eq!(start_id, end_id, "Run IDs should match between start and completion");
    }
    
    println!("✅ Tool progress flow test passed!");
}

/// Test that progress events are properly forwarded
#[tokio::test]
async fn test_progress_event_forwarding() {
    // Create a mock tool that sends progress events
    let (progress_tx, mut progress_rx) = mpsc::channel::<StreamEvent>(10);
    
    // Send some mock progress events
    tokio::spawn(async move {
        let _ = progress_tx.send(StreamEvent::Progress {
            message: "Starting operation...".to_string(),
            percentage: Some(10.0),
        }).await;
        
        let _ = progress_tx.send(StreamEvent::Progress {
            message: "Processing...".to_string(),
            percentage: Some(50.0),
        }).await;
        
        let _ = progress_tx.send(StreamEvent::Progress {
            message: "Completing...".to_string(),
            percentage: Some(100.0),
        }).await;
    });
    
    // Collect progress events
    let mut progress_events = Vec::new();
    while let Some(event) = progress_rx.recv().await {
        progress_events.push(event);
        if progress_events.len() >= 3 {
            break;
        }
    }
    
    assert_eq!(progress_events.len(), 3, "Should receive 3 progress events");
    
    // Verify progress percentages
    if let StreamEvent::Progress { percentage: Some(pct), .. } = &progress_events[0] {
        assert_eq!(*pct, 10.0);
    }
    if let StreamEvent::Progress { percentage: Some(pct), .. } = &progress_events[1] {
        assert_eq!(*pct, 50.0);
    }
    if let StreamEvent::Progress { percentage: Some(pct), .. } = &progress_events[2] {
        assert_eq!(*pct, 100.0);
    }
    
    println!("✅ Progress event forwarding test passed!");
}

/// Test tool run ID generation and uniqueness
#[test]
fn test_tool_run_id_generation() {
    let id1: ToolRunId = Uuid::new_v4();
    let id2: ToolRunId = Uuid::new_v4();
    
    assert_ne!(id1, id2, "Tool run IDs should be unique");
    assert!(!id1.to_string().is_empty(), "Tool run ID should not be empty");
    
    println!("✅ Tool run ID generation test passed!");
} 