use tokio::sync::oneshot;
use anyhow::{anyhow, Result};
// Tool registry removed - tools now via MCP
use tempfile::NamedTempFile;
use std::io::Write;
use serde_json::json;
use sagitta_search::config::AppConfig;

/// Manages an internal MCP server for Claude CLI integration
pub struct McpIntegration {
    // Tool registry removed - tools provided by sagitta-mcp
    server_handle: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    config_file: Option<NamedTempFile>,
}

impl McpIntegration {
    pub fn new() -> Self {
        Self {
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
pub async fn run_internal_mcp_server(_tool_registry: Option<()>) -> Result<()> {
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

// Tests disabled - tools have been removed and are now provided via MCP
#[cfg(test)]
#[cfg(feature = "disabled_tests_tools_removed")]
mod tests {
    // Tests removed as they depend on tool registry which is no longer used
    // Tools are now provided via MCP from sagitta-mcp
}