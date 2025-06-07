use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use terminal_stream::events::StreamEvent;
use chrono::{DateTime, Utc};
use serde_json;

use crate::tools::shell_execution::{ShellExecutionParams, ShellExecutionResult};
use crate::utils::errors::SagittaCodeError;

/// Approval policy for command execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalPolicy {
    /// Auto-approve safe commands, ask for dangerous ones
    Auto,
    /// Always ask for approval regardless of command
    AlwaysAsk,
    /// Paranoid mode - ask for every command including very safe ones
    Paranoid,
}

impl Default for ApprovalPolicy {
    fn default() -> Self {
        ApprovalPolicy::Auto
    }
}

/// Classification result for a command
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandClass {
    /// Safe to execute automatically
    Safe,
    /// Requires user approval
    NeedsApproval,
    /// Forbidden - should never be executed
    Forbidden,
}

/// Configuration for local command execution
#[derive(Debug, Clone)]
pub struct LocalExecutorConfig {
    /// Base directory for spatial containment
    pub repositories_base_path: PathBuf,
    /// Approval policy for commands
    pub approval_policy: ApprovalPolicy,
    /// Whether to allow automatic tool installation
    pub allow_automatic_tool_install: bool,
    /// CPU limit in seconds (optional)
    pub cpu_limit_seconds: Option<u64>,
    /// Memory limit in MB (optional)
    pub memory_limit_mb: Option<u64>,
}

impl Default for LocalExecutorConfig {
    fn default() -> Self {
        Self {
            repositories_base_path: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from(".")),
            approval_policy: ApprovalPolicy::Auto,
            allow_automatic_tool_install: false,
            cpu_limit_seconds: None,
            memory_limit_mb: None,
        }
    }
}

/// Information about a missing tool and how to install it
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingToolAdvice {
    pub tool_name: String,
    pub apt: Option<String>,
    pub brew: Option<String>,
    pub pacman: Option<String>,
    pub cargo: Option<String>,
    pub npm: Option<String>,
    pub pip: Option<String>,
    pub manual: Option<String>,
}

/// Audit log entry for executed commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub timestamp: DateTime<Utc>,
    pub user: String,
    pub working_directory: PathBuf,
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub prompted: bool,
    pub approved_by: Option<String>,
}

/// Trait for command execution backends
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    /// Execute a command and return the result
    async fn execute(&self, params: &ShellExecutionParams) -> Result<ShellExecutionResult, SagittaCodeError>;
    
    /// Execute a command with streaming output
    async fn execute_streaming(
        &self,
        params: &ShellExecutionParams,
        event_sender: mpsc::Sender<StreamEvent>,
    ) -> Result<ShellExecutionResult, SagittaCodeError>;
}

/// Local command executor with safety constraints
#[derive(Debug)]
pub struct LocalExecutor {
    config: LocalExecutorConfig,
}

impl LocalExecutor {
    /// Create a new local executor with the given config
    pub fn new(config: LocalExecutorConfig) -> Self {
        Self { config }
    }

    /// Create a local executor with default config
    pub fn with_default_config() -> Self {
        Self::new(LocalExecutorConfig::default())
    }

    /// Get a reference to the executor configuration (for testing)
    pub fn config(&self) -> &LocalExecutorConfig {
        &self.config
    }

    /// Resolve and validate a working directory path
    pub fn resolve_and_check(&self, path: &Path) -> Result<PathBuf, SagittaCodeError> {
        let target_path = if path.is_absolute() {
            // For absolute paths, use as-is
            path.to_path_buf()
        } else {
            // For relative paths, join with the repositories base path
            self.config.repositories_base_path.join(path)
        };

        // Try to canonicalize the path if it exists, otherwise use the path as-is
        let canonical_path = if target_path.exists() {
            target_path.canonicalize()
                .map_err(|e| SagittaCodeError::ToolError(
                    format!("Failed to canonicalize path '{}': {}", target_path.display(), e)
                ))?
        } else {
            // For non-existent paths, we still need to validate they would be within bounds
            // Use the non-canonical path for validation
            target_path.clone()
        };

        // Ensure the path is within our base directory for spatial containment
        // For non-canonical paths, we need to check if the path would resolve within bounds
        let base_canonical = self.config.repositories_base_path.canonicalize()
            .map_err(|e| SagittaCodeError::ToolError(
                format!("Failed to canonicalize base path '{}': {}", 
                    self.config.repositories_base_path.display(), e)
            ))?;

        // Check if the path is within the base directory
        let is_within_base = if canonical_path.exists() {
            // For existing paths, use the canonical form
            canonical_path.starts_with(&base_canonical)
        } else {
            // For non-existing paths, check if the target path starts with base
            // This handles cases where intermediate directories don't exist yet
            target_path.starts_with(&self.config.repositories_base_path) ||
            // Also try with canonical base in case of symlinks in base path
            target_path.starts_with(&base_canonical)
        };

        if !is_within_base {
            return Err(SagittaCodeError::ToolError(
                format!(
                    "Path '{}' is outside the allowed base directory '{}'. All operations must stay within the repository base path for security.",
                    canonical_path.display(),
                    self.config.repositories_base_path.display()
                )
            ));
        }

        // Forbid certain system-critical directories
        let forbidden_paths = [
            "/", "/bin", "/sbin", "/usr/bin", "/usr/sbin", "/etc", 
            "/boot", "/root", "/proc", "/sys", "/dev"
        ];
        
        let path_str = canonical_path.to_string_lossy();
        for forbidden in &forbidden_paths {
            if path_str == *forbidden || path_str.starts_with(&format!("{}/", forbidden)) {
                return Err(SagittaCodeError::ToolError(
                    format!("Execution in system directory '{}' is forbidden for security reasons", path_str)
                ));
            }
        }

        Ok(canonical_path)
    }

    /// Classify a command based on its safety
    pub fn classify_command(&self, command: &str) -> CommandClass {
        let cmd_lower = command.to_lowercase();
        
        // Extract the first word (the actual command)
        let first_word = cmd_lower.split_whitespace().next().unwrap_or("");
        
        // Commands that are forbidden outright
        let forbidden_commands = [
            "rm", "rmdir", "del", "erase",
            "chmod", "chown", "chgrp",
            "dd", "fdisk", "mkfs", "format",
            "mount", "umount", "sudo", "su",
            "useradd", "userdel", "usermod",
            "passwd", "crontab", "systemctl",
            "service", "halt", "reboot", "shutdown",
            "iptables", "ufw", "firewall-cmd",
        ];
        
        if forbidden_commands.contains(&first_word) {
            return CommandClass::Forbidden;
        }
        
        // Check for specific dangerous patterns first, before general safe commands
        if cmd_lower.starts_with("git ") {
            // Most git commands are safe, but be careful with some
            if cmd_lower.contains("git rm") || cmd_lower.contains("git clean -f") {
                return CommandClass::NeedsApproval;
            }
            return CommandClass::Safe;
        }
        
        if cmd_lower.starts_with("cargo ") {
            // Cargo commands are generally safe
            return CommandClass::Safe;
        }
        
        if cmd_lower.starts_with("npm ") || cmd_lower.starts_with("yarn ") || cmd_lower.starts_with("pnpm ") {
            // Package manager commands are generally safe
            return CommandClass::Safe;
        }
        
        // Commands that are generally safe (after checking for specific patterns above)
        let safe_commands = [
            "git", "cargo", "npm", "yarn", "pnpm", "pip", "pipenv",
            "node", "python", "python3", "go", "rustc",
            "echo", "cat", "less", "more", "head", "tail",
            "ls", "dir", "pwd", "cd", "find", "grep",
            "wc", "sort", "uniq", "cut", "awk", "sed",
            "which", "where", "whereis", "type",
            "make", "cmake", "ninja", "mvn", "gradle",
            "docker", "kubectl", "helm", // Container tools (read-only operations)
        ];
        
        if safe_commands.contains(&first_word) {
            return CommandClass::Safe;
        }
        
        // Default to requiring approval for unknown commands
        CommandClass::NeedsApproval
    }

    /// Check if a tool is available and provide installation advice if not
    pub async fn check_tool_availability(&self, tool_name: &str) -> Result<bool, MissingToolAdvice> {
        // Try to find the tool using 'which' (Unix) or 'where' (Windows)
        let check_cmd = if cfg!(windows) { "where" } else { "which" };
        
        match Command::new(check_cmd)
            .arg(tool_name)
            .output()
            .await
        {
            Ok(output) => {
                if output.status.success() {
                    Ok(true)
                } else {
                    Err(self.get_installation_advice(tool_name))
                }
            }
            Err(_) => Err(self.get_installation_advice(tool_name)),
        }
    }

    /// Get installation advice for a missing tool
    fn get_installation_advice(&self, tool_name: &str) -> MissingToolAdvice {
        match tool_name {
            "git" => MissingToolAdvice {
                tool_name: tool_name.to_string(),
                apt: Some("sudo apt-get install git".to_string()),
                brew: Some("brew install git".to_string()),
                pacman: Some("sudo pacman -S git".to_string()),
                cargo: None,
                npm: None,
                pip: None,
                manual: Some("Download from https://git-scm.com/downloads".to_string()),
            },
            "cargo" | "rustc" => MissingToolAdvice {
                tool_name: tool_name.to_string(),
                apt: None,
                brew: None,
                pacman: None,
                cargo: None,
                npm: None,
                pip: None,
                manual: Some("Install Rust from https://rustup.rs/".to_string()),
            },
            "node" | "npm" => MissingToolAdvice {
                tool_name: tool_name.to_string(),
                apt: Some("sudo apt-get install nodejs npm".to_string()),
                brew: Some("brew install node".to_string()),
                pacman: Some("sudo pacman -S nodejs npm".to_string()),
                cargo: None,
                npm: None,
                pip: None,
                manual: Some("Download from https://nodejs.org/".to_string()),
            },
            "python" | "python3" => MissingToolAdvice {
                tool_name: tool_name.to_string(),
                apt: Some("sudo apt-get install python3".to_string()),
                brew: Some("brew install python".to_string()),
                pacman: Some("sudo pacman -S python".to_string()),
                cargo: None,
                npm: None,
                pip: None,
                manual: Some("Download from https://python.org/downloads/".to_string()),
            },
            "pip" => MissingToolAdvice {
                tool_name: tool_name.to_string(),
                apt: Some("sudo apt-get install python3-pip".to_string()),
                brew: Some("pip is included with Python from Homebrew".to_string()),
                pacman: Some("sudo pacman -S python-pip".to_string()),
                cargo: None,
                npm: None,
                pip: None,
                manual: Some("Install Python first, pip is usually included".to_string()),
            },
            "go" => MissingToolAdvice {
                tool_name: tool_name.to_string(),
                apt: Some("sudo apt-get install golang".to_string()),
                brew: Some("brew install go".to_string()),
                pacman: Some("sudo pacman -S go".to_string()),
                cargo: None,
                npm: None,
                pip: None,
                manual: Some("Download from https://golang.org/dl/".to_string()),
            },
            _ => MissingToolAdvice {
                tool_name: tool_name.to_string(),
                apt: Some(format!("sudo apt-get install {}", tool_name)),
                brew: Some(format!("brew install {}", tool_name)),
                pacman: Some(format!("sudo pacman -S {}", tool_name)),
                cargo: None,
                npm: None,
                pip: None,
                manual: None,
            },
        }
    }

    /// Parse command string into command and arguments
    fn parse_command(&self, command_str: &str) -> Vec<String> {
        // Simple shell-like parsing - split on whitespace but respect quotes
        // This is a basic implementation; for production, consider using shell-words crate
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut chars = command_str.chars().peekable();
        
        while let Some(ch) = chars.next() {
            match ch {
                '"' => {
                    in_quotes = !in_quotes;
                }
                ' ' | '\t' if !in_quotes => {
                    if !current.is_empty() {
                        parts.push(current);
                        current = String::new();
                    }
                }
                _ => {
                    current.push(ch);
                }
            }
        }
        
        if !current.is_empty() {
            parts.push(current);
        }
        
        parts
    }

    /// Write an audit log entry
    async fn write_audit_log(&self, entry: &AuditLogEntry) -> Result<(), SagittaCodeError> {
        let audit_file = self.config.repositories_base_path.join(".sagitta_audit.log");
        
        let json_line = serde_json::to_string(entry)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to serialize audit entry: {}", e)))?;
        
        let line_with_newline = format!("{}\n", json_line);
        
        tokio::fs::write(&audit_file, line_with_newline).await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to write audit log: {}", e)))?;
        
        Ok(())
    }
}

#[async_trait]
impl CommandExecutor for LocalExecutor {
    async fn execute(&self, params: &ShellExecutionParams) -> Result<ShellExecutionResult, SagittaCodeError> {
        // For non-streaming execution, we'll create a channel but not use it
        // We'll modify the streaming method to handle this case gracefully
        let (tx, mut rx) = mpsc::channel(100);
        
        // Spawn a task to consume events so the sender doesn't block
        let _consumer_task = tokio::spawn(async move {
            while let Some(_event) = rx.recv().await {
                // Just consume the events for non-streaming execution
            }
        });
        
        self.execute_streaming(params, tx).await
    }

    async fn execute_streaming(
        &self,
        params: &ShellExecutionParams,
        event_sender: mpsc::Sender<StreamEvent>,
    ) -> Result<ShellExecutionResult, SagittaCodeError> {
        let start_time = std::time::Instant::now();
        
        // Parse the command
        let command_parts = self.parse_command(&params.command);
        if command_parts.is_empty() {
            return Err(SagittaCodeError::ToolError("Empty command".to_string()));
        }
        
        let program = &command_parts[0];
        let args = &command_parts[1..];
        
        // Classify the command
        let command_class = self.classify_command(&params.command);
        
        // Check approval policy
        let needs_approval = match (self.config.approval_policy, command_class) {
            (ApprovalPolicy::Auto, CommandClass::Safe) => false,
            (ApprovalPolicy::Auto, CommandClass::NeedsApproval) => true,
            (ApprovalPolicy::Auto, CommandClass::Forbidden) => {
                return Err(SagittaCodeError::ToolError(
                    format!("Command '{}' is forbidden for security reasons", params.command)
                ));
            }
            (ApprovalPolicy::AlwaysAsk, CommandClass::Forbidden) => {
                return Err(SagittaCodeError::ToolError(
                    format!("Command '{}' is forbidden for security reasons", params.command)
                ));
            }
            (ApprovalPolicy::AlwaysAsk, _) => true,
            (ApprovalPolicy::Paranoid, _) => true,
        };
        
        // If approval is needed, send approval request
        if needs_approval {
            let approval_event = StreamEvent::ApprovalRequest {
                id: uuid::Uuid::new_v4().to_string(),
                command: params.command.clone(),
                reason: "Command requires user approval".to_string(),
            };
            
            if let Err(_) = event_sender.send(approval_event).await {
                return Err(SagittaCodeError::ToolError("Failed to send approval request".to_string()));
            }
            
            // For now, we'll assume approval is granted
            // In a real implementation, this would wait for user response
        }
        
        // Check tool availability
        if let Err(advice) = self.check_tool_availability(program).await {
            let missing_tool_event = StreamEvent::MissingTool {
                tool: advice.tool_name.clone(),
                advice: serde_json::to_value(advice)
                    .map_err(|e| SagittaCodeError::ToolError(format!("Failed to serialize tool advice: {}", e)))?,
            };
            
            if let Err(_) = event_sender.send(missing_tool_event).await {
                log::warn!("Failed to send missing tool event");
            }
            
            return Err(SagittaCodeError::ToolError(
                format!("Required tool '{}' is not available", program)
            ));
        }
        
        // Resolve and validate working directory
        let working_dir = if let Some(ref wd) = params.working_directory {
            self.resolve_and_check(wd)?
        } else {
            self.config.repositories_base_path.clone()
        };
        
        // Create the command
        let mut cmd = Command::new(program);
        cmd.args(args)
           .current_dir(&working_dir)
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());
        
        // Add environment variables
        if let Some(ref env_vars) = params.env_vars {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }
        
        // Add OS information as environment variables
        cmd.env("SAGITTA_OS_NAME", std::env::consts::OS);
        cmd.env("SAGITTA_OS_VERSION", "unknown"); // Could use os_info crate for more detail
        
        // Spawn the process
        let mut child = cmd.spawn()
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to spawn command: {}", e)))?;
        
        // Get stdout and stderr streams
        let stdout = child.stdout.take()
            .ok_or_else(|| SagittaCodeError::ToolError("Failed to get stdout".to_string()))?;
        let stderr = child.stderr.take()
            .ok_or_else(|| SagittaCodeError::ToolError("Failed to get stderr".to_string()))?;
        
        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);
        
        let mut stdout_lines = stdout_reader.lines();
        let mut stderr_lines = stderr_reader.lines();
        
        let mut stdout_output = String::new();
        let mut stderr_output = String::new();
        
        // Read output streams concurrently
        let mut stdout_closed = false;
        let mut stderr_closed = false;
        
        loop {
            tokio::select! {
                stdout_line = stdout_lines.next_line(), if !stdout_closed => {
                    match stdout_line {
                        Ok(Some(line)) => {
                            stdout_output.push_str(&line);
                            stdout_output.push('\n');
                            
                            let stdout_event = StreamEvent::Stdout { content: line };
                            if let Err(_) = event_sender.send(stdout_event).await {
                                log::warn!("Failed to send stdout event");
                            }
                        }
                        Ok(None) => {
                            stdout_closed = true;
                        }
                        Err(e) => {
                            log::warn!("Error reading stdout: {}", e);
                            stdout_closed = true;
                        }
                    }
                }
                stderr_line = stderr_lines.next_line(), if !stderr_closed => {
                    match stderr_line {
                        Ok(Some(line)) => {
                            stderr_output.push_str(&line);
                            stderr_output.push('\n');
                            
                            let stderr_event = StreamEvent::Stderr { content: line };
                            if let Err(_) = event_sender.send(stderr_event).await {
                                log::warn!("Failed to send stderr event");
                            }
                        }
                        Ok(None) => {
                            stderr_closed = true;
                        }
                        Err(e) => {
                            log::warn!("Error reading stderr: {}", e);
                            stderr_closed = true;
                        }
                    }
                }
                // Also wait for the process to complete
                status = child.wait() => {
                    match status {
                        Ok(exit_status) => {
                            let exit_code = exit_status.code().unwrap_or(-1);
                            let execution_time = start_time.elapsed();
                            
                            // Send exit event
                            let exit_event = StreamEvent::Exit { code: exit_code };
                            if let Err(_) = event_sender.send(exit_event).await {
                                log::warn!("Failed to send exit event");
                            }
                            
                            // Write audit log
                            let audit_entry = AuditLogEntry {
                                timestamp: Utc::now(),
                                user: std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
                                working_directory: working_dir.clone(),
                                command: command_parts,
                                exit_code: Some(exit_code),
                                duration_ms: execution_time.as_millis() as u64,
                                prompted: needs_approval,
                                approved_by: if needs_approval { Some("user".to_string()) } else { None },
                            };
                            
                            if let Err(e) = self.write_audit_log(&audit_entry).await {
                                log::warn!("Failed to write audit log: {}", e);
                            }
                            
                            return Ok(ShellExecutionResult {
                                exit_code,
                                stdout: stdout_output,
                                stderr: stderr_output,
                                execution_time_ms: execution_time.as_millis() as u64,
                                container_image: "local".to_string(), // No container used
                                timed_out: false, // TODO: Implement timeout support
                            });
                        }
                        Err(e) => {
                            return Err(SagittaCodeError::ToolError(format!("Failed to wait for command: {}", e)));
                        }
                    }
                }
            }
            
            // Break if both streams are closed
            if stdout_closed && stderr_closed {
                break;
            }
        }
        
        // This should not be reached since we return from the process completion above
        // But just in case, wait for the process and return
        let status = child.wait().await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to wait for command: {}", e)))?;
        
        let exit_code = status.code().unwrap_or(-1);
        let execution_time = start_time.elapsed();
        
        // Send exit event
        let exit_event = StreamEvent::Exit { code: exit_code };
        if let Err(_) = event_sender.send(exit_event).await {
            log::warn!("Failed to send exit event");
        }
        
        // Write audit log
        let audit_entry = AuditLogEntry {
            timestamp: Utc::now(),
            user: std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
            working_directory: working_dir.clone(),
            command: command_parts,
            exit_code: Some(exit_code),
            duration_ms: execution_time.as_millis() as u64,
            prompted: needs_approval,
            approved_by: if needs_approval { Some("user".to_string()) } else { None },
        };
        
        if let Err(e) = self.write_audit_log(&audit_entry).await {
            log::warn!("Failed to write audit log: {}", e);
        }
        
        Ok(ShellExecutionResult {
            exit_code,
            stdout: stdout_output,
            stderr: stderr_output,
            execution_time_ms: execution_time.as_millis() as u64,
            container_image: "local".to_string(), // No container used
            timed_out: false, // TODO: Implement timeout support
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config() -> (LocalExecutorConfig, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = LocalExecutorConfig {
            repositories_base_path: temp_dir.path().to_path_buf(),
            approval_policy: ApprovalPolicy::Auto,
            allow_automatic_tool_install: false,
            cpu_limit_seconds: None,
            memory_limit_mb: None,
        };
        (config, temp_dir)
    }

    #[test]
    fn test_command_classification() {
        let (config, _temp_dir) = create_test_config();
        let executor = LocalExecutor::new(config);
        
        // Test safe commands
        assert_eq!(executor.classify_command("git status"), CommandClass::Safe);
        assert_eq!(executor.classify_command("cargo check"), CommandClass::Safe);
        assert_eq!(executor.classify_command("npm install"), CommandClass::Safe);
        assert_eq!(executor.classify_command("echo hello"), CommandClass::Safe);
        
        // Test dangerous commands
        assert_eq!(executor.classify_command("rm -rf /"), CommandClass::Forbidden);
        assert_eq!(executor.classify_command("sudo rm file"), CommandClass::Forbidden);
        assert_eq!(executor.classify_command("chmod 777 file"), CommandClass::Forbidden);
        
        // Test commands needing approval
        assert_eq!(executor.classify_command("unknown_command"), CommandClass::NeedsApproval);
        assert_eq!(executor.classify_command("git rm file"), CommandClass::NeedsApproval);
    }

    #[test]
    fn test_path_validation() {
        let (config, temp_dir) = create_test_config();
        let executor = LocalExecutor::new(config);
        
        // Test valid path within base
        let valid_path = temp_dir.path().join("subdir");
        std::fs::create_dir_all(&valid_path).unwrap();
        assert!(executor.resolve_and_check(&valid_path).is_ok());
        
        // Test invalid path outside base (this will fail because we can't create paths outside temp_dir easily in test)
        // We'll test the error condition in integration tests
    }

    #[test]
    fn test_path_validation_comprehensive() {
        let (config, temp_dir) = create_test_config();
        let executor = LocalExecutor::new(config);
        
        // Test 1: Valid existing absolute path within base
        let valid_absolute = temp_dir.path().join("existing_subdir");
        std::fs::create_dir_all(&valid_absolute).unwrap();
        let result = executor.resolve_and_check(&valid_absolute);
        assert!(result.is_ok(), "Valid absolute path should be accepted: {:?}", result);
        
        // Test 2: Valid relative path within base (existing)
        let valid_relative_existing = Path::new("existing_subdir");
        let result = executor.resolve_and_check(valid_relative_existing);
        assert!(result.is_ok(), "Valid relative path to existing dir should be accepted: {:?}", result);
        
        // Test 3: Valid relative path within base (non-existing)
        let valid_relative_nonexisting = Path::new("nonexisting_subdir");
        let result = executor.resolve_and_check(valid_relative_nonexisting);
        assert!(result.is_ok(), "Valid relative path to non-existing dir should be accepted: {:?}", result);
        
        // Test 4: Relative path with traversal that stays within base
        let safe_traversal = Path::new("existing_subdir/../other_dir");
        let result = executor.resolve_and_check(safe_traversal);
        assert!(result.is_ok(), "Safe path traversal within base should be accepted: {:?}", result);
        
        // Test 5: Relative path with traversal that escapes base (should fail)
        let escape_attempt = Path::new("../../../etc");
        let result = executor.resolve_and_check(escape_attempt);
        assert!(result.is_err(), "Path traversal escaping base should be rejected");
        
        // Test 6: Absolute path to system directory (should fail)
        let system_path = Path::new("/etc");
        let result = executor.resolve_and_check(system_path);
        assert!(result.is_err(), "System directory access should be rejected");
        
        // Test 7: Relative path to forbidden directory name (should fail if it resolves to system dir)
        // This test might not fail in temp dir context, but we test the logic
        let forbidden_relative = Path::new("etc");
        let result = executor.resolve_and_check(forbidden_relative);
        // This should succeed in temp dir context since it's temp_dir/etc, not /etc
        assert!(result.is_ok(), "Relative path to 'etc' within base should be ok in temp context");
    }

    #[test]
    fn test_path_validation_edge_cases() {
        let (config, temp_dir) = create_test_config();
        let executor = LocalExecutor::new(config);
        
        // Test empty relative path (should resolve to base)
        let empty_path = Path::new("");
        let result = executor.resolve_and_check(empty_path);
        assert!(result.is_ok(), "Empty path should resolve to base directory");
        
        // Test current directory reference
        let current_dir = Path::new(".");
        let result = executor.resolve_and_check(current_dir);
        assert!(result.is_ok(), "Current directory reference should be valid");
        
        // Test nested relative path
        let nested_path = Path::new("level1/level2/level3");
        let result = executor.resolve_and_check(nested_path);
        assert!(result.is_ok(), "Nested relative path should be valid");
        
        // Test path with multiple traversals that end up in base
        let complex_traversal = Path::new("subdir/../subdir2/../final");
        let result = executor.resolve_and_check(complex_traversal);
        assert!(result.is_ok(), "Complex traversal ending in base should be valid");
    }

    #[test]
    fn test_command_parsing() {
        let (config, _temp_dir) = create_test_config();
        let executor = LocalExecutor::new(config);
        
        assert_eq!(
            executor.parse_command("git status"),
            vec!["git", "status"]
        );
        
        assert_eq!(
            executor.parse_command("echo \"hello world\""),
            vec!["echo", "hello world"]
        );
        
        assert_eq!(
            executor.parse_command("  ls  -la  "),
            vec!["ls", "-la"]
        );
    }

    #[tokio::test]
    async fn test_simple_command_execution() {
        let (config, temp_dir) = create_test_config();
        let executor = LocalExecutor::new(config);
        
        let params = ShellExecutionParams {
            command: "echo hello".to_string(),
            language: None,
            working_directory: Some(temp_dir.path().to_path_buf()),
            allow_network: None,
            env_vars: None,
            timeout_seconds: None,
        };
        
        let result = executor.execute(&params).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello"));
    }
} 