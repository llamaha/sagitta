use std::sync::Arc;
use tokio;
use sagitta_code::tools::types::{Tool, ToolResult, ToolDefinition, ToolCategory};
use sagitta_code::utils::errors::SagittaCodeError;
use async_trait::async_trait;
use serde_json::Value;

/// Mock tool that simulates the "already exists" scenario
#[derive(Debug)]
struct MockAddRepositoryTool {
    should_return_already_exists: bool,
}

impl MockAddRepositoryTool {
    fn new(should_return_already_exists: bool) -> Self {
        Self { should_return_already_exists }
    }
}

#[async_trait]
impl Tool for MockAddRepositoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "add_repository".to_string(),
            description: "Mock add repository tool for testing".to_string(),
            category: ToolCategory::Repository,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "url": {"type": "string"}
                },
                "required": ["name", "url"]
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, _parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        if self.should_return_already_exists {
            // Simulate the "already exists" case - this should be treated as success
            Ok(ToolResult::Success(serde_json::json!({
                "success": true,
                "message": "Repository 'sidekiq' already exists and is available for use",
                "repository_name": "sidekiq"
            })))
        } else {
            // Simulate successful addition
            Ok(ToolResult::Success(serde_json::json!({
                "success": true,
                "message": "Successfully added repository 'sidekiq'",
                "repository_name": "sidekiq"
            })))
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[tokio::test]
async fn test_tool_result_conversion() {
    // Test that ToolResult::Success is properly converted to reasoning engine format
    let tool = MockAddRepositoryTool::new(false);
    let result = tool.execute(serde_json::json!({"name": "test", "url": "https://example.com"})).await.unwrap();
    
    match result {
        ToolResult::Success(data) => {
            assert!(data.get("success").and_then(|v| v.as_bool()).unwrap_or(false));
            println!("✓ ToolResult::Success correctly indicates success");
        }
        ToolResult::Error { .. } => {
            panic!("Expected success result");
        }
    }
}

#[tokio::test]
async fn test_already_exists_case() {
    // Test that "already exists" case returns success
    let tool = MockAddRepositoryTool::new(true);
    let result = tool.execute(serde_json::json!({"name": "sidekiq", "url": "https://github.com/sidekiq/sidekiq.git"})).await.unwrap();
    
    match result {
        ToolResult::Success(data) => {
            assert!(data.get("success").and_then(|v| v.as_bool()).unwrap_or(false));
            let message = data.get("message").and_then(|v| v.as_str()).unwrap_or("");
            assert!(message.contains("already exists"));
            println!("✓ 'Already exists' case correctly returns success: {}", message);
        }
        ToolResult::Error { .. } => {
            panic!("'Already exists' should be treated as success, not error");
        }
    }
}

#[tokio::test]
async fn test_reasoning_engine_tool_adapter() {
    use sagitta_code::reasoning::AgentToolExecutor;
    use sagitta_code::tools::registry::ToolRegistry;
    use reasoning_engine::traits::ToolExecutor as ReasoningToolExecutor;
    
    // Create a tool registry and add our mock tool
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(MockAddRepositoryTool::new(true))).await.unwrap();
    
    // Create the adapter
    let adapter = AgentToolExecutor::new(registry);
    
    // Execute the tool through the adapter
    let result = adapter.execute_tool("add_repository", serde_json::json!({
        "name": "sidekiq",
        "url": "https://github.com/sidekiq/sidekiq.git"
    })).await.unwrap();
    
    // Verify the reasoning engine ToolResult has success=true
    assert!(result.success, "Reasoning engine ToolResult should have success=true");
    assert!(result.error.is_none(), "Reasoning engine ToolResult should have no error");
    
    println!("✓ AgentToolExecutor correctly converts 'already exists' to success=true");
    println!("  - success: {}", result.success);
    println!("  - data: {}", result.data);
    println!("  - error: {:?}", result.error);
} 