use std::process::{Command, Stdio, Child};
use std::io::{BufReader, BufRead};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use crate::config::types::ClaudeCodeConfig;
use super::error::ClaudeCodeError;
use super::message_converter::ClaudeMessage;

/// Manages the Claude process lifecycle
pub struct ClaudeProcess {
    child: Arc<Mutex<Option<Child>>>,
    config: ClaudeCodeConfig,
}

impl ClaudeProcess {
    pub fn new(config: ClaudeCodeConfig) -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
            config,
        }
    }
    
    /// Spawn the claude process with the given arguments
    pub async fn spawn(
        &self,
        system_prompt: &str,
        messages: &[ClaudeMessage],
        tools: &[String], // Tool names to disable
    ) -> Result<Child, ClaudeCodeError> {
        // Convert messages to JSON format like Roo-Code does
        let messages_json = serde_json::to_string(messages)?;
        
        log::info!("CLAUDE_CODE: Spawning claude process");
        log::info!("CLAUDE_CODE: Binary path: {}", self.config.claude_path);
        log::info!("CLAUDE_CODE: Model: {}", self.config.model);
        log::debug!("CLAUDE_CODE: Messages JSON: {}", messages_json);
        log::debug!("CLAUDE_CODE: System prompt: {}", system_prompt);
        
        let mut args = vec![
            "-p".to_string(),
            messages_json,
            "--system-prompt".to_string(),
            system_prompt.to_string(),
            "--verbose".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--max-turns".to_string(),
            "1".to_string(), // Let sagitta-code handle multi-turn
        ];
        
        // Add model if specified
        if !self.config.model.is_empty() {
            args.push("--model".to_string());
            args.push(self.config.model.clone());
        }
        
        // Disable tools if any
        if !tools.is_empty() {
            args.push("--disallowedTools".to_string());
            args.push(tools.join(","));
        }
        
        log::trace!("CLAUDE_CODE: Full args: {:?}", args);
        
        // Log the full command for debugging
        let full_command = format!("{} {}", self.config.claude_path, args.join(" "));
        log::debug!("CLAUDE_CODE: Full command: {}", full_command);
        
        let mut cmd = Command::new(&self.config.claude_path);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("CLAUDE_CODE_MAX_OUTPUT_TOKENS", self.config.max_output_tokens.to_string());
        
        match cmd.spawn() {
            Ok(child) => {
                log::debug!("CLAUDE_CODE: Process spawned successfully, PID: {:?}", child.id());
                Ok(child)
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Err(ClaudeCodeError::BinaryNotFound(self.config.claude_path.clone()))
                } else {
                    Err(ClaudeCodeError::ProcessError(e))
                }
            }
        }
    }
    
    /// Kill the process if it's still running
    pub async fn kill(&self) -> Result<(), ClaudeCodeError> {
        let mut child_guard = self.child.lock().await;
        if let Some(mut child) = child_guard.take() {
            log::debug!("CLAUDE_CODE: Killing process");
            child.kill().map_err(ClaudeCodeError::ProcessError)?;
        }
        Ok(())
    }
}

impl Drop for ClaudeProcess {
    fn drop(&mut self) {
        // Try to kill the process on drop
        if let Ok(mut child_guard) = self.child.try_lock() {
            if let Some(mut child) = child_guard.take() {
                let _ = child.kill();
            }
        }
    }
}

/// Read stderr for debugging
pub async fn read_stderr(child: &mut Child) -> String {
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        let lines: Vec<String> = reader.lines()
            .filter_map(|line| line.ok())
            .collect();
        return lines.join("\n");
    }
    String::new()
}