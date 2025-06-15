use std::sync::Arc;
use sagitta_code::reasoning::AgentToolExecutor;
use sagitta_code::tools::registry::ToolRegistry;

/// Test that verifies the timeout configuration for different tools
#[tokio::test]
async fn test_timeout_configuration() {
    let registry = Arc::new(ToolRegistry::new());
    let executor = AgentToolExecutor::new(registry);
    
    // Test that sync operations get very long timeouts (effectively no timeout)
    assert_eq!(executor.get_timeout_for_tool("sync_repository"), u64::MAX);
    assert_eq!(executor.get_timeout_for_tool("add_existing_repository"), u64::MAX);
    
    // Test that other tools get the default timeout
    assert_eq!(executor.get_timeout_for_tool("shell_execution"), 1800); // 30 minutes
    assert_eq!(executor.get_timeout_for_tool("view_file"), 1800);
    assert_eq!(executor.get_timeout_for_tool("unknown_tool"), 1800);
}

/// Test that the default timeout was increased from 60 seconds
#[tokio::test]
async fn test_default_timeout_increased() {
    let registry = Arc::new(ToolRegistry::new());
    let executor = AgentToolExecutor::new(registry);
    
    // Verify the default timeout is now 30 minutes instead of 60 seconds
    assert_eq!(executor.default_timeout_seconds, 1800);
    assert_ne!(executor.default_timeout_seconds, 60); // Old value
} 