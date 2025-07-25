// Minimal stub types for tools - actual tools provided via MCP from sagitta-mcp

use std::sync::Arc;
use serde::{Serialize, Deserialize};
use tokio::sync::mpsc;

pub mod mcp_bridge;

/// Tool registry that bridges MCP tools
pub struct ToolRegistry;

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry
    }
    
    pub async fn get_definitions(&self) -> Vec<crate::llm::client::ToolDefinition> {
        // Return MCP tool definitions for non-Claude providers
        let definitions = mcp_bridge::get_mcp_tool_definitions();
        log::info!("ToolRegistry::get_definitions returning {} MCP tool definitions", definitions.len());
        definitions
    }
}

/// Stub tool executor  
pub struct ToolExecutor;

impl ToolExecutor {
    pub async fn execute_tool(&self, _tool_name: &str, _parameters: serde_json::Value) -> Result<crate::agent::events::ToolResult, crate::utils::errors::SagittaCodeError> {
        // Tools are executed via MCP, not internally
        Err(crate::utils::errors::SagittaCodeError::Unknown("Tools are provided via MCP".to_string()))
    }
}

/// Stub tool executor internal (for backwards compatibility)
#[derive(Clone)]
pub struct SagittaCodeToolExecutorInternal;

impl SagittaCodeToolExecutorInternal {
    pub fn new(_registry: Arc<ToolRegistry>, _state_manager: Arc<crate::agent::state::manager::StateManager>) -> (Self, mpsc::Receiver<crate::agent::events::ToolExecutionEvent>) {
        let (_tx, rx) = mpsc::channel(100);
        (SagittaCodeToolExecutorInternal, rx)
    }
    
    pub async fn execute_tool(&self, _tool_name: &str, _parameters: serde_json::Value) -> Result<crate::agent::events::ToolResult, crate::utils::errors::SagittaCodeError> {
        // Tools are executed via MCP, not internally
        Err(crate::utils::errors::SagittaCodeError::Unknown("Tools are provided via MCP".to_string()))
    }
}

// ToolResult is now defined in agent/events.rs, use type alias
pub use crate::agent::events::ToolResult;

/// Tool definition type alias
pub type ToolDefinition = crate::llm::client::ToolDefinition;

/// Tool category for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCategory {
    FileOperation,
    CodeAnalysis,
    RepositoryManagement,
    Other,
}

/// Stub working directory manager
pub struct WorkingDirectoryManager {
    base_dir: std::path::PathBuf,
}

impl WorkingDirectoryManager {
    pub fn new(path: std::path::PathBuf) -> Result<Self, std::io::Error> {
        Ok(WorkingDirectoryManager { base_dir: path })
    }
    
    pub fn set_repository_context(&self, repo_path: Option<&std::path::Path>) -> Result<(), std::io::Error> {
        if let Some(path) = repo_path {
            // Actually change the working directory
            std::env::set_current_dir(path)?;
            
            // Write the repository path to the state file for MCP tools
            self.write_repository_state_file(path)?;
            
            log::info!("WorkingDirectoryManager: Changed directory to: {}", path.display());
        }
        Ok(())
    }
    
    pub fn get_base_directory(&self) -> &std::path::Path {
        &self.base_dir
    }
    
    pub fn change_directory(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        // Actually change the working directory
        std::env::set_current_dir(path)?;
        
        // Write the repository path to the state file for MCP tools
        self.write_repository_state_file(path)?;
        
        log::info!("WorkingDirectoryManager: Changed directory to: {}", path.display());
        Ok(())
    }
    
    /// Write the current repository path to the state file for MCP tools
    fn write_repository_state_file(&self, repo_path: &std::path::Path) -> Result<(), std::io::Error> {
        // Create the state file path
        let mut state_path = dirs::config_dir()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Could not find config directory"))?;
        state_path.push("sagitta-code");
        
        // Create the directory if it doesn't exist
        std::fs::create_dir_all(&state_path)?;
        
        state_path.push("current_repository.txt");
        
        // Write the repository path to the state file
        std::fs::write(&state_path, repo_path.to_string_lossy().as_bytes())?;
        
        log::debug!("WorkingDirectoryManager: Wrote repository path to state file: {}", state_path.display());
        Ok(())
    }
}