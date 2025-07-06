// Minimal stub types for tools - actual tools provided via MCP from sagitta-mcp

use std::sync::Arc;
use serde::{Serialize, Deserialize};
use tokio::sync::mpsc;

/// Stub tool registry - no actual tools
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
        vec![] // No tools - they come from MCP
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
    
    pub fn set_repository_context(&self, _repo_path: Option<&std::path::Path>) -> Result<(), std::io::Error> {
        Ok(())
    }
    
    pub fn get_base_directory(&self) -> &std::path::Path {
        &self.base_dir
    }
    
    pub fn change_directory(&self, _path: &std::path::Path) -> Result<(), std::io::Error> {
        Ok(())
    }
}