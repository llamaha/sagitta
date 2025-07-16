use std::process::{Command, Stdio, Child};
use std::io::{BufReader, BufRead};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::config::types::ClaudeCodeConfig;
use super::error::ClaudeCodeError;

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
        prompt: &str,
    ) -> Result<Child, ClaudeCodeError> {
        self.spawn_with_mcp(prompt, None).await
    }
    
    /// Spawn the claude process with optional MCP configuration
    pub async fn spawn_with_mcp(
        &self,
        prompt: &str,
        mcp_config_path: Option<&str>,
    ) -> Result<Child, ClaudeCodeError> {
        log::info!("CLAUDE_CODE: Spawning claude process");
        log::info!("CLAUDE_CODE: Binary path: {}", self.config.claude_path);
        log::info!("CLAUDE_CODE: Model: {}", self.config.model);
        log::debug!("CLAUDE_CODE: Prompt: {prompt}");
        
        let mut args = vec![
            "-p".to_string(),
            prompt.to_string(),
            "--verbose".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ];
        
        // Add max-turns if not unlimited (0)
        if self.config.max_turns > 0 {
            args.push("--max-turns".to_string());
            args.push(self.config.max_turns.to_string());
        }
        
        // Add model if specified
        if !self.config.model.is_empty() {
            args.push("--model".to_string());
            args.push(self.config.model.clone());
        }
        
        // Add MCP config if provided
        if let Some(mcp_path) = mcp_config_path {
            args.push("--mcp-config".to_string());
            args.push(mcp_path.to_string());
            log::info!("CLAUDE_CODE: Using MCP config: {mcp_path}");
            
            // Allow all MCP tools from our server
            // Claude CLI prefixes MCP tools with mcp__servername__toolname
            // Try multiple approaches based on GitHub issues
            args.push("--allowedTools".to_string());
            
            // Build a list of all MCP tool patterns
            // Based on the actual tools in sagitta-mcp/src/handlers/tool.rs
            let mcp_tools = vec![
                // Core tools
                "mcp__*__ping",
                // Repository management
                "mcp__*__repository_add",
                "mcp__*__repository_list",
                "mcp__*__repository_sync",
                "mcp__*__repository_switch_branch",
                "mcp__*__repository_list_branches",
                // Code search
                "mcp__*__semantic_code_search",
                "mcp__*__search_file",
                // Todo management
                "mcp__*__todo_read",
                "mcp__*__todo_write",
                // File operations
                "mcp__*__read_file",
                "mcp__*__write_file",
                "mcp__*__edit_file",
                "mcp__*__multi_edit_file",
                // Shell execution
                "mcp__*__shell_execute",
                // Wildcard to catch any we missed
                "mcp__*"
            ];
            
            args.push(mcp_tools.join(","));
            log::info!("CLAUDE_CODE: Allowing MCP tools: {}", mcp_tools.join(","));
            
            // Disable Claude native tools that are duplicated by our MCP tools
            args.push("--disallowedTools".to_string());
            let disallowed_tools = vec![
                "TodoRead",     // We have mcp__sagitta-mcp-stdio__todo_read
                "TodoWrite",    // We have mcp__sagitta-mcp-stdio__todo_write
                "Edit",         // We have mcp__sagitta-mcp-stdio__edit_file
                "MultiEdit",    // We have mcp__sagitta-mcp-stdio__multi_edit_file
                "Write",        // We have mcp__sagitta-mcp-stdio__write_file
                "Read",         // We have mcp__sagitta-mcp-stdio__read_file and mcp__*__view_file
                "Bash",         // We have mcp__sagitta-mcp-stdio__shell_execute
                "Glob",         // We have mcp__*__search_file_in_repository
                "Grep",         // We have mcp__*__search_code and mcp__*__query
                "LS",           // We have directory listing through MCP tools
            ];
            args.push(disallowed_tools.join(","));
            log::info!("CLAUDE_CODE: Disallowing native tools: {}", disallowed_tools.join(","));
        }
        
        log::trace!("CLAUDE_CODE: Full args: {args:?}");
        
        // Log the full command for debugging
        let full_command = format!("{} {}", self.config.claude_path, args.join(" "));
        log::debug!("CLAUDE_CODE: Full command: {full_command}");
        
        let mut cmd = Command::new(&self.config.claude_path);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("CLAUDE_CODE_MAX_OUTPUT_TOKENS", self.config.max_output_tokens.to_string())
            .env_remove("ANTHROPIC_API_KEY"); // Ensure we use Claude Max subscription
        
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
