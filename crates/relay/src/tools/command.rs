// src/tools/command.rs

use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use crate::utils::prompt_user_confirmation;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use std::io::{self, Write};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct RunCommandParams {
    pub command: String,
    pub cwd: Option<String>,
    pub timeout_secs: Option<u64>,
}

#[derive(Debug)]
pub struct RunCommandAction {
    params: RunCommandParams,
}

impl RunCommandAction {
    pub fn new(command: String, cwd: Option<String>, timeout_secs: Option<u64>) -> Self {
        Self { params: RunCommandParams { command, cwd, timeout_secs } }
    }
}

#[async_trait]
impl Action for RunCommandAction {
    fn name(&self) -> &'static str {
        "run_command"
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        let command_str = &self.params.command;
        let cwd = self.params.cwd.as_deref().or(state.current_directory.as_deref());
        debug!(command = %command_str, cwd = ?cwd, "Preparing RunCommandAction");

        // --- User Confirmation ---
        let prompt = format!("Relay wants to run: `{}` in directory {:?}. Allow? (y/n): ", 
                            command_str, cwd.unwrap_or("."));

        match prompt_user_confirmation(&prompt)? {
            true => {
                // User confirmed, proceed with execution
                info!(command = %command_str, cwd = ?cwd, "Executing confirmed RunCommandAction");
            }
            false => {
                // User denied
                warn!(command = %command_str, "User denied command execution.");
                let denial_message = format!("User denied execution of command: {}", command_str);
                state
                    .set_context(format!("command_error_{}", command_str), denial_message)
                    .map_err(|e| {
                        RelayError::ToolError(format!("Failed to set context for denial: {}", e))
                    })?;
                return Ok(()); // Denial is not an action error
            }
        }
        // --- End User Confirmation ---

        // Parse command line string into command and args
        let cmd_parts: Vec<&str> = command_str.split_whitespace().collect();
        if cmd_parts.is_empty() {
            return Err(RelayError::ToolError("Empty command string".to_string()));
        }

        // Build the command
        let mut command = Command::new(cmd_parts[0]);
        command.args(&cmd_parts[1..]);
        
        // Set working directory if specified
        if let Some(dir) = cwd {
            command.current_dir(dir);
        }
        
        // Configure stdin/stdout/stderr
        command.stdin(std::process::Stdio::null());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        
        // Start the command
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                error!(command = %command_str, error = %e, "Failed to spawn command");
                return Err(RelayError::IoError(e));
            }
        };

        let timeout_duration = self.params.timeout_secs.map(Duration::from_secs);
        
        // Asynchronously wait for the command to complete or timeout
        let output_result = if let Some(duration) = timeout_duration {
            match timeout(duration, child.wait_with_output()).await {
                Ok(result) => result.map_err(RelayError::IoError),
                Err(_) => Err(RelayError::ToolError("Command timed out".to_string()))
            }
        } else {
            // No timeout specified, wait indefinitely
            child.wait_with_output().await.map_err(RelayError::IoError)
        };

        match output_result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code();

                debug!(command = %command_str, ?exit_code, stdout_len=stdout.len(), stderr_len=stderr.len(), "Command finished");

                // Store results in context
                state
                    .set_context(format!("command_stdout_{}", command_str), stdout)
                    .map_err(|e| {
                        RelayError::ToolError(format!("Failed to set context for stdout: {}", e))
                    })?;
                state
                    .set_context(format!("command_stderr_{}", command_str), stderr.clone()) // Clone stderr for potential error message
                    .map_err(|e| {
                        RelayError::ToolError(format!("Failed to set context for stderr: {}", e))
                    })?;
                state
                    .set_context(format!("command_exit_code_{}", command_str), exit_code)
                    .map_err(|e| {
                        RelayError::ToolError(format!("Failed to set context for exit code: {}", e))
                    })?;

                if output.status.success() {
                    Ok(())
                } else {
                    warn!(command = %command_str, ?exit_code, "Command finished with non-zero exit code.");
                    // Even on failure, the action completed. The failure status is in the context.
                    // Optionally, return an error if that's preferred chain behavior:
                    // Err(RelayError::ToolError(format!(
                    //     "Command '{}' failed with exit code {:?}. Stderr: {}",
                    //     command_str, exit_code, stderr
                    // )))
                    Ok(())
                }
            }
            Err(e) => {
                error!(command = %command_str, error = %e, "Error waiting for command output or timeout");
                state
                    .set_context(format!("command_error_{}", command_str), e.to_string())
                    .map_err(|inner_e| {
                        RelayError::ToolError(format!(
                            "Failed to set context for command wait error: {}",
                            inner_e
                        ))
                    })?;
                Err(e) // Propagate the error (timeout or wait error)
            }
        }
    }
} 