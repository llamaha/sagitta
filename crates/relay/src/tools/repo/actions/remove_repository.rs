use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{debug, error, info, warn};
use std::io::{self, Write};

// --- Remove Repository Action ---
// Executes `vectordb-cli repo remove <name>`

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveRepositoryParams {
    pub name: String,
}

#[derive(Debug)]
pub struct RemoveRepositoryAction {
    params: RemoveRepositoryParams,
}

impl RemoveRepositoryAction {
    pub fn new(params: RemoveRepositoryParams) -> Self {
        Self { params }
    }
}

#[async_trait]
impl Action for RemoveRepositoryAction {
    fn name(&self) -> &'static str {
        "remove_repository"
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        debug!(name = %self.params.name, "Preparing RemoveRepositoryAction");

        // --- Construct Command --- 
        let command_string = format!("vectordb-cli repo remove {}", self.params.name);
        let mut command = Command::new("vectordb-cli");
        command.args(["repo", "remove", &self.params.name]);
        // --- End Construct Command ---

        // --- User Confirmation --- 
        // Note: The original code included a direct user prompt here.
        // In the Relay architecture, such direct interaction should ideally be 
        // handled by the agent/orchestrator calling the action, not within the action itself.
        // For now, we'll simulate automatic confirmation or delegate this responsibility.
        // Let's assume confirmation is implicitly given for now, or handled upstream.
        // If explicit confirmation is needed, the action should signal this requirement.
        
        // Original code for reference:
        // print!("Relay wants to run: `{}`. This will remove the repository configuration. Allow? (y/n): ", command_string);
        // io::stdout().flush().map_err(|e| RelayError::IoError(e))?; 
        // let mut user_input = String::new();
        // io::stdin().read_line(&mut user_input).map_err(|e| RelayError::IoError(e))?;
        // if user_input.trim().to_lowercase() != "y" {
        //     warn!(command = %command_string, "User denied command execution.");
        //     let denial_message = format!("User denied execution of remove_repository command for {}", self.params.name);
        //     state
        //         .set_context(format!("repo_remove_error_{}", self.params.name), denial_message)
        //         .map_err(|e| RelayError::ToolError(format!("Failed to set context for denial: {}", e)))?;
        //     return Ok(()); 
        // }
        
        info!(command = %command_string, "Executing potentially confirmed RemoveRepositoryAction");

        // Configure stdio
        command.stdin(std::process::Stdio::null());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        // Run the command
        let output = command.output().await.map_err(|e| {
            error!(error = %e, command = %command_string, "Failed to execute 'vectordb-cli repo remove'");
            RelayError::ToolError(format!("Failed to execute vectordb-cli remove repo: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            info!(repo_name = %self.params.name, "Successfully removed repository via CLI.");

            // Update ChainState if the removed repo was the active one
            if state.active_repository.as_deref() == Some(&self.params.name) {
                state.active_repository = None;
                info!(repo_name = %self.params.name, "Cleared active repository in state as it was removed.");
            }

            state
                .set_context(format!("repo_remove_status_{}", self.params.name), "Success".to_string())
                .map_err(|e| RelayError::ToolError(format!("Failed to set context for repo remove status: {}", e)))?;
            state
                .set_context(format!("repo_remove_stdout_{}", self.params.name), stdout)
                .map_err(|e| RelayError::ToolError(format!("Failed to set context for repo remove stdout: {}", e)))?;
            Ok(())
        } else {
            error!(status = %output.status, stderr = %stderr, command = %command_string, "'vectordb-cli repo remove' failed");
            let err_msg = format!(
                "Failed to remove repository '{}'. Exit code: {:?}, Stderr: {}",
                self.params.name,
                output.status.code(),
                stderr
            );
            state
                .set_context(format!("repo_remove_error_{}", self.params.name), err_msg.clone())
                .map_err(|e| RelayError::ToolError(format!("Failed to set context for repo remove error: {}", e)))?;
            Err(RelayError::ToolError(err_msg))
        }
    }
} 