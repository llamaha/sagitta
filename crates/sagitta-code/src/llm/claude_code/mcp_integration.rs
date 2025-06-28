use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Command, Child};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};
use anyhow::{anyhow, Result};
use crate::tools::registry::ToolRegistry;
use tempfile::NamedTempFile;
use std::io::Write;
use serde_json::json;
use sagitta_search::config::AppConfig;

/// Manages an internal MCP server for Claude CLI integration
pub struct McpIntegration {
    tool_registry: Arc<ToolRegistry>,
    server_handle: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    config_file: Option<NamedTempFile>,
}

impl McpIntegration {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            tool_registry,
            server_handle: None,
            shutdown_tx: None,
            config_file: None,
        }
    }
    
    /// Start the internal MCP server and return the config for Claude CLI
    pub async fn start(&mut self) -> Result<serde_json::Value> {
        // Create a temporary file for the MCP config
        let mut config_file = NamedTempFile::new()?;
        
        // Generate a unique server name
        let server_name = format!("sagitta-internal-{}", uuid::Uuid::new_v4());
        
        // Create the MCP config that points to our own binary
        let current_exe = std::env::current_exe()?;
        log::info!("MCP: Using executable path: {}", current_exe.display());
        
        let mcp_config = json!({
            "mcpServers": {
                server_name.clone(): {
                    "command": current_exe.to_string_lossy(),
                    "args": ["--mcp-internal"],
                    "env": {},
                    "stdin": "pipe",
                    "stdout": "pipe",
                    "stderr": "pipe"
                }
            }
        });
        
        // Write config to file
        config_file.write_all(serde_json::to_string_pretty(&mcp_config)?.as_bytes())?;
        config_file.flush()?;
        
        // Store the config file to keep it alive
        let config_path = config_file.path().to_string_lossy().to_string();
        self.config_file = Some(config_file);
        
        log::info!("MCP: Config file created at: {}", config_path);
        log::info!("MCP: Server name: {}", server_name);
        
        // Return the config for Claude CLI
        Ok(json!({
            "mcp_config_path": config_path,
            "server_name": server_name
        }))
    }
    
    /// Stop the internal MCP server
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        
        if let Some(handle) = self.server_handle.take() {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                handle
            ).await;
        }
        
        // Clean up config file
        self.config_file = None;
    }
}

/// Start an internal MCP server process
/// This is called when the binary is run with --mcp-internal flag
pub async fn run_internal_mcp_server(tool_registry: Arc<ToolRegistry>) -> Result<()> {
    log::info!("Starting internal MCP server (using sagitta-mcp Server)");
    
    // Load the sagitta-search config (same as sagitta-mcp does)
    let config_path = sagitta_search::get_config_path_or_default(None)?;
    let config: AppConfig = sagitta_search::load_config(Some(&config_path))
        .map_err(|e| anyhow!("Failed to load configuration: {}", e))?;
    
    // Create and run the same Server that sagitta-mcp uses
    let server = sagitta_mcp::server::Server::new(config).await?;
    
    // Run in stdio mode (just like sagitta-mcp does)
    server.run().await?;
    
    Ok(())
}

impl Drop for McpIntegration {
    fn drop(&mut self) {
        // Ensure cleanup happens
        if self.server_handle.is_some() || self.shutdown_tx.is_some() {
            log::warn!("McpIntegration dropped without calling stop()");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
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
}