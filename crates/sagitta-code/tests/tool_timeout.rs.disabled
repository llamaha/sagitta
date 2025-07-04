use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use sagitta_code::{
    tools::{registry::ToolRegistry, types::{Tool, ToolDefinition, ToolResult, ToolCategory}},
    utils::errors::SagittaCodeError,
};
use async_trait::async_trait;
use serde_json::Value;

/// A test tool that sleeps for a specified duration
#[derive(Debug)]
struct SlowTool {
    sleep_duration: Duration,
}

impl SlowTool {
    fn new(sleep_duration: Duration) -> Self {
        Self { sleep_duration }
    }
}

#[async_trait]
impl Tool for SlowTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "slow_tool".to_string(),
            description: "A tool that sleeps for testing timeouts".to_string(),
            category: ToolCategory::Other,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, _parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        // Sleep for the specified duration
        tokio::time::sleep(self.sleep_duration).await;
        Ok(ToolResult::Success(serde_json::json!({
            "message": "Tool completed after sleep"
        })))
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Test that tool execution can be properly timed out
#[tokio::test]
async fn test_tool_execution_timeout() {
    let registry = ToolRegistry::new();
    
    // Register a tool that sleeps for 5 seconds
    let slow_tool = Arc::new(SlowTool::new(Duration::from_secs(5)));
    registry.register(slow_tool).await.unwrap();
    
    // Try to execute the tool with a 1 second timeout
    let tool = registry.get("slow_tool").await.unwrap();
    
    let result = timeout(
        Duration::from_secs(1),
        tool.execute(serde_json::json!({}))
    ).await;
    
    // Should timeout
    assert!(result.is_err(), "Tool execution should timeout");
}

/// Test that normal tools complete within timeout
#[tokio::test]
async fn test_normal_tool_execution_succeeds() {
    let registry = ToolRegistry::new();
    
    // Register a tool that completes quickly
    let quick_tool = Arc::new(SlowTool::new(Duration::from_millis(100)));
    registry.register(quick_tool).await.unwrap();
    
    // Execute with a generous timeout
    let tool = registry.get("slow_tool").await.unwrap();
    
    let result = timeout(
        Duration::from_secs(2),
        tool.execute(serde_json::json!({}))
    ).await;
    
    // Should complete successfully
    assert!(result.is_ok(), "Quick tool should complete within timeout");
    let tool_result = result.unwrap().unwrap();
    assert!(tool_result.is_success(), "Tool should succeed");
} 