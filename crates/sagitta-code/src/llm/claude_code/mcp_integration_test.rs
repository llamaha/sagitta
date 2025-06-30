#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::tools::registry::ToolRegistry;
    use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
    use std::sync::Arc;
    use serde_json::{json, Value};
    use async_trait::async_trait;
    use crate::utils::errors::SagittaCodeError;
    
    #[derive(Debug)]
    struct MockTool {
        name: String,
    }
    
    #[async_trait]
    impl Tool for MockTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: self.name.clone(),
                description: format!("Mock {} tool", self.name),
                parameters: json!({
                    "type": "object",
                    "properties": {}
                }),
                is_required: false,
                category: ToolCategory::Other,
                metadata: Default::default(),
            }
        }
        
        async fn execute(&self, _params: Value) -> Result<ToolResult, SagittaCodeError> {
            Ok(ToolResult::success(json!({
                "result": format!("{} executed", self.name)
            })))
        }
        
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    
    #[tokio::test]
    async fn test_mcp_integration_creates_config() {
        // Create a tool registry with some test tools
        let tool_registry = Arc::new(ToolRegistry::new());
        tool_registry.register(Arc::new(MockTool { name: "test_tool_1".to_string() })).await.unwrap();
        tool_registry.register(Arc::new(MockTool { name: "test_tool_2".to_string() })).await.unwrap();
        
        // Create MCP integration
        let mut mcp = McpIntegration::new(tool_registry);
        
        // Start the integration
        let config = mcp.start().await.unwrap();
        
        // Verify config has required fields
        assert!(config.get("mcp_config_path").is_some());
        assert!(config.get("server_name").is_some());
        
        // Read the config file
        let config_path = config["mcp_config_path"].as_str().unwrap();
        let config_content = std::fs::read_to_string(config_path).unwrap();
        let parsed_config: Value = serde_json::from_str(&config_content).unwrap();
        
        // Verify the config structure
        assert!(parsed_config.get("mcpServers").is_some());
        let servers = parsed_config["mcpServers"].as_object().unwrap();
        assert_eq!(servers.len(), 1);
        
        // Get the server config
        let server_name = config["server_name"].as_str().unwrap();
        let server_config = &servers[server_name];
        
        // Verify command points to current executable
        let current_exe = std::env::current_exe().unwrap();
        assert_eq!(server_config["command"].as_str().unwrap(), current_exe.to_string_lossy());
        
        // Verify args include --mcp-internal
        let args = server_config["args"].as_array().unwrap();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].as_str().unwrap(), "--mcp-internal");
        
        // Clean up
        mcp.stop().await;
    }
    
    // Test removed: MCP server functionality is now provided by sagitta-mcp crate
    // #[tokio::test]
    // async fn test_mcp_server_exposes_tools() {
    
    // Test removed: MCP tool execution is now tested in sagitta-mcp crate
    // #[tokio::test]
    // async fn test_mcp_tool_execution() {
}