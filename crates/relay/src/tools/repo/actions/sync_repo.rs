use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

// --- Sync Repository Action ---
// Executes `vectordb-cli repo sync [name]` by executing the command.

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncRepositoryParams {
    pub name: Option<String>, // Repository name, optional (defaults to active/all)
}

#[derive(Debug)]
pub struct SyncRepositoryAction {
    params: SyncRepositoryParams,
}

impl SyncRepositoryAction {
    pub fn new(params: SyncRepositoryParams) -> Self {
        Self { params }
    }
}

#[async_trait]
impl Action for SyncRepositoryAction {
    fn name(&self) -> &'static str {
        "sync_repository"
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        // Determine the target repository: use specified name or fallback to active repo in state
        // CLONE the active repository name early to avoid borrow issues later.
        let active_repo_name = state.active_repository.clone();
        let repo_name_to_sync = self.params.name.as_deref()
            .or(active_repo_name.as_deref()); // Use the cloned value here
        
        debug!(repo_name = ?repo_name_to_sync, "Preparing SyncRepositoryAction");

        // --- Construct Command --- 
        let mut command = Command::new("vectordb-cli");
        command.args(["repo", "sync"]);
        let target_repo_display;
        if let Some(name) = repo_name_to_sync {
             command.arg(name);
             target_repo_display = name.to_string();
        } else {
            // If no name specified and no active repo, the CLI should sync all. 
            // The CLI handles the case where no repos exist.
            warn!("SyncRepositoryAction called without a specific name and no active repository set in state. CLI will attempt to sync all.");
            target_repo_display = "(all)".to_string();
        }
        let command_string = format!("{:?}", command); 
        // --- End Construct Command ---

        // --- User Confirmation (Removed for Relay) --- 
        // Similar to RemoveRepoAction, confirmation should be handled upstream.
        // Original code commented out below.

        // print!("Relay wants to run: `{}`. This may fetch updates and re-index. Allow? (y/n): ", command_string);
        // io::stdout().flush().map_err(|e| RelayError::IoError(e))?; 
        // let mut user_input = String::new();
        // io::stdin().read_line(&mut user_input).map_err(|e| RelayError::IoError(e))?;
        // if user_input.trim().to_lowercase() != "y" {
        //     warn!(command = %command_string, "User denied command execution.");
        //     let denial_message = format!("User denied execution of sync_repository command for {}", target_repo_display);
        //     let context_key = format!("repo_sync_error_{}", repo_name_to_sync.unwrap_or("all"));
        //     state
        //         .set_context(context_key, denial_message)
        //         .map_err(|e| RelayError::ToolError(format!("Failed to set context for denial: {}", e)))?;
        //     return Ok(()); 
        // }

        info!(command = %command_string, "Executing potentially confirmed SyncRepositoryAction");

        // Configure stdio
        command.stdin(std::process::Stdio::null());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        // --- Execute Command ---
        let output = command.output().await.map_err(|e| {
            error!(error = %e, command = %command_string, "Failed to execute 'vectordb-cli repo sync'");
            RelayError::ToolError(format!("Failed to execute vectordb-cli sync repo: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Use the potentially owned string for the key calculation BEFORE the mutable borrows.
        let context_repo_key = repo_name_to_sync.unwrap_or("all");

        if output.status.success() {
            info!(repo_name = %target_repo_display, stdout_len = stdout.len(), "Repository sync completed successfully via CLI.");
            // Mutable borrow of state starts here
            state
                .set_context(format!("repo_sync_status_{}", context_repo_key), "Success".to_string())
                .map_err(|e| RelayError::ToolError(format!("Failed to set context for repo sync status: {}", e)))?;
            state
                .set_context(format!("repo_sync_stdout_{}", context_repo_key), stdout)
                .map_err(|e| RelayError::ToolError(format!("Failed to set context for repo sync stdout: {}", e)))?;
            Ok(())
        } else {
            error!(status = %output.status, stderr = %stderr, command = %command_string, "'vectordb-cli repo sync' failed");
            let err_msg = format!(
                "Failed to sync repository '{}'. Exit code: {:?}, Stderr: {}",
                target_repo_display,
                output.status.code(),
                stderr
            );
             // Mutable borrow of state starts here
             state
                .set_context(format!("repo_sync_error_{}", context_repo_key), err_msg.clone())
                .map_err(|e| RelayError::ToolError(format!("Failed to set context for repo sync error: {}", e)))?;
            Err(RelayError::ToolError(err_msg))
        }
    }
} 