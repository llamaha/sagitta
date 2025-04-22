use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{debug, error, info};

// --- Use Repo Action ---
// Corresponds to `vectordb-cli repo use` by executing the command.

#[derive(Debug, Serialize, Deserialize)]
pub struct UseRepoParams {
    pub name: String, // Made public
}

#[derive(Debug)]
pub struct UseRepoAction {
    params: UseRepoParams,
}

impl UseRepoAction {
    pub fn new(name: String) -> Self {
        Self { params: UseRepoParams { name } }
    }
}

#[async_trait]
impl Action for UseRepoAction {
    fn name(&self) -> &'static str {
        "use_repository" // Using the name expected by parser.rs
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        debug!(name = %self.params.name, "Preparing UseRepoAction");

        // --- Construct Command --- 
        let mut command = Command::new("vectordb-cli");
        command.args(["repo", "use", &self.params.name]);
        let command_string = format!("{:?}", command);
        // --- End Construct Command ---

        // --- Execute Command ---
        info!(command = %command_string, "Executing UseRepoAction");
        command.stdin(std::process::Stdio::null()); // Don't wait for stdin
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        let output = command.output().await.map_err(|e| {
            error!(error = %e, command = %command_string, "Failed to execute 'vectordb-cli repo use'");
            RelayError::ToolError(format!("Failed to execute vectordb-cli use repo: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            info!(repo_name = %self.params.name, stdout = %stdout, "Successfully set active repository via CLI.");
            
            // Update ChainState
            state.active_repository = Some(self.params.name.clone());
            info!(active_repository = ?state.active_repository, "Updated active repository in state.");

            // Note: Finding the repo path and setting state.current_directory requires reading config,
            // which might be better handled by the caller or a dedicated config-reading action.
            // For now, this action focuses only on calling the CLI command.

            state
                .set_context(format!("repo_use_status_{}", self.params.name), "Success".to_string())
                .map_err(|e| RelayError::ToolError(format!("Failed to set context for repo use status: {}", e)))?;
            state
                .set_context(format!("repo_use_stdout_{}", self.params.name), stdout)
                .map_err(|e| RelayError::ToolError(format!("Failed to set context for repo use stdout: {}", e)))?;
            Ok(())
        } else {
            error!(status = %output.status, stderr = %stderr, command = %command_string, "'vectordb-cli repo use' failed");
            let err_msg = format!(
                "Failed to use repository '{}'. Exit code: {:?}, Stderr: {}",
                self.params.name,
                output.status.code(),
                stderr
            );
            state
                .set_context(format!("repo_use_error_{}", self.params.name), err_msg.clone())
                .map_err(|e| RelayError::ToolError(format!("Failed to set context for repo use error: {}", e)))?;
            Err(RelayError::ToolError(err_msg))
        }
    }
} 