use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::tools::shell_execution::StreamingShellExecutionTool;
use crate::tools::working_directory::WorkingDirectoryManager;

/// Parameters for the Bash tool
#[derive(Debug, Deserialize, Serialize)]
pub struct BashParams {
    /// The command to execute
    pub command: String,
    /// Optional working directory (defaults to current directory)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
}

/// Bash tool - a Claude-compatible wrapper around StreamingShellExecutionTool
pub struct BashTool {
    shell_tool: StreamingShellExecutionTool,
}

impl BashTool {
    pub fn new(default_working_dir: std::path::PathBuf) -> Self {
        Self {
            shell_tool: StreamingShellExecutionTool::new(default_working_dir),
        }
    }
    
    pub fn new_with_working_dir_manager(
        default_working_dir: std::path::PathBuf,
        working_dir_manager: Arc<WorkingDirectoryManager>,
    ) -> Self {
        Self {
            shell_tool: StreamingShellExecutionTool::new_with_working_dir_manager(
                default_working_dir,
                working_dir_manager,
            ),
        }
    }
}

#[async_trait]
impl Tool for BashTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Bash".to_string(),  // Claude expects this exact name
            description: "Execute bash commands in a shell environment".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "working_directory": {
                        "type": "string",
                        "description": "Optional working directory for command execution"
                    }
                },
                "required": ["command"]
            }),
            category: ToolCategory::Development,
        }
    }
    
    async fn execute(&self, params: Value) -> Result<ToolResult, Box<dyn std::error::Error + Send + Sync>> {
        let bash_params: BashParams = serde_json::from_value(params)?;
        
        // Convert to shell execution params
        let shell_params = serde_json::json!({
            "command": bash_params.command,
            "working_directory": bash_params.working_directory,
        });
        
        // Delegate to the shell execution tool
        self.shell_tool.execute(shell_params).await
    }
    
    fn set_progress_sender(&mut self, _sender: Option<String>) {
        // Progress sender functionality removed
    }
}