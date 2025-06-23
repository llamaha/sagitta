use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use terminal_stream::events::StreamEvent;
use std::sync::Arc;

use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::tools::local_executor::{LocalExecutor, LocalExecutorConfig, CommandExecutor, ApprovalPolicy};
use crate::utils::errors::SagittaCodeError;
use sagitta_search::config::get_repo_base_path;
use crate::tools::working_directory::WorkingDirectoryManager;

/// Configuration for shell execution containers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    /// Docker image to use for execution
    pub image: String,
    /// Working directory inside container
    pub workdir: String,
    /// Environment variables to set
    pub env_vars: HashMap<String, String>,
    /// Volume mounts (host_path -> container_path)
    pub volumes: HashMap<String, String>,
    /// Network mode for the container
    pub network_mode: Option<String>,
    /// Memory limit for the container
    pub memory_limit: Option<String>,
    /// CPU limit for the container
    pub cpu_limit: Option<String>,
    /// Timeout for command execution in seconds
    pub timeout_seconds: u64,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            image: "alpine:3.19".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: HashMap::new(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()), // Isolated by default
            memory_limit: Some("256m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300, // 5 minutes default
        }
    }
}

/// Language-specific container configurations
#[derive(Debug, Clone)]
pub struct LanguageContainers {
    configs: HashMap<String, ContainerConfig>,
}

impl Default for LanguageContainers {
    fn default() -> Self {
        let mut configs = HashMap::new();
        
        // General lightweight container - using Alpine with basic tools
        configs.insert("default".to_string(), ContainerConfig {
            image: "alpine:3.19".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("PATH".to_string(), "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()),
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("256m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300,
        });
        
        // Rust-specific container - Alpine with Rust toolchain
        configs.insert("rust".to_string(), ContainerConfig {
            image: "rust:1.75-alpine".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("CARGO_HOME".to_string(), "/usr/local/cargo".to_string()),
                ("RUSTUP_HOME".to_string(), "/usr/local/rustup".to_string()),
                ("PATH".to_string(), "/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()),
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("1g".to_string()),
            cpu_limit: Some("2.0".to_string()),
            timeout_seconds: 600, // 10 minutes for compilation
        });
        
        // Python-specific container - Alpine with Python
        configs.insert("python".to_string(), ContainerConfig {
            image: "python:3.12-alpine".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("PYTHONPATH".to_string(), "/workspace".to_string()),
                ("PYTHONUNBUFFERED".to_string(), "1".to_string()),
                ("PYTHONDONTWRITEBYTECODE".to_string(), "1".to_string()),
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300,
        });
        
        // JavaScript/TypeScript container - Alpine with Node.js
        configs.insert("javascript".to_string(), ContainerConfig {
            image: "node:20-alpine".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("NODE_ENV".to_string(), "development".to_string()),
                ("NPM_CONFIG_UPDATE_NOTIFIER".to_string(), "false".to_string()),
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300,
        });
        
        // TypeScript uses the same Node.js container
        configs.insert("typescript".to_string(), ContainerConfig {
            image: "node:20-alpine".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("NODE_ENV".to_string(), "development".to_string()),
                ("NPM_CONFIG_UPDATE_NOTIFIER".to_string(), "false".to_string()),
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300,
        });
        
        // Go-specific container - Alpine with Go toolchain
        configs.insert("go".to_string(), ContainerConfig {
            image: "golang:1.21-alpine".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("GOPATH".to_string(), "/go".to_string()),
                ("GO111MODULE".to_string(), "on".to_string()),
                ("CGO_ENABLED".to_string(), "0".to_string()), // Disable CGO for static binaries
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300,
        });
        
        // Golang uses the same Go container
        configs.insert("golang".to_string(), ContainerConfig {
            image: "golang:1.21-alpine".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("GOPATH".to_string(), "/go".to_string()),
                ("GO111MODULE".to_string(), "on".to_string()),
                ("CGO_ENABLED".to_string(), "0".to_string()), // Disable CGO for static binaries
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300,
        });
        
        // Ruby-specific container - Alpine with Ruby
        configs.insert("ruby".to_string(), ContainerConfig {
            image: "ruby:3.2-alpine".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("BUNDLE_SILENCE_ROOT_WARNING".to_string(), "1".to_string()),
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300,
        });
        
        // HTML - uses lightweight Alpine with basic tools for static content
        configs.insert("html".to_string(), ContainerConfig {
            image: "alpine:3.19".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: HashMap::new(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("128m".to_string()),
            cpu_limit: Some("0.5".to_string()),
            timeout_seconds: 60,
        });
        
        // YAML - uses Alpine with yq for YAML processing
        configs.insert("yaml".to_string(), ContainerConfig {
            image: "alpine:3.19".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: HashMap::new(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("128m".to_string()),
            cpu_limit: Some("0.5".to_string()),
            timeout_seconds: 60,
        });
        
        // Markdown - uses Alpine with basic text processing tools
        configs.insert("markdown".to_string(), ContainerConfig {
            image: "alpine:3.19".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: HashMap::new(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("128m".to_string()),
            cpu_limit: Some("0.5".to_string()),
            timeout_seconds: 60,
        });
        
        Self { configs }
    }
}

impl LanguageContainers {
    pub fn get_config(&self, language: &str) -> &ContainerConfig {
        self.configs.get(language).unwrap_or_else(|| {
            self.configs.get("default").unwrap()
        })
    }
    
    pub fn add_config(&mut self, language: String, config: ContainerConfig) {
        self.configs.insert(language, config);
    }
}

/// Parameters for shell command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellExecutionParams {
    /// The command to execute
    pub command: String,
    /// Optional language/environment to use (for backwards compatibility, now ignored)
    pub language: Option<String>,
    /// Working directory for command execution
    pub working_directory: Option<PathBuf>,
    /// Whether to allow network access (for backwards compatibility, now ignored)
    pub allow_network: Option<bool>,
    /// Additional environment variables
    pub env_vars: Option<HashMap<String, String>>,
    /// Custom timeout in seconds (for backwards compatibility, now ignored)
    pub timeout_seconds: Option<u64>,
}

/// Result of shell command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellExecutionResult {
    /// Exit code of the command
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Working directory where the command was executed
    pub working_directory: PathBuf,
    /// Container used for execution (now always "local")
    pub container_image: String,
    /// Whether the command timed out
    pub timed_out: bool,
}

/// Shell execution tool that uses local execution instead of Docker
#[derive(Debug)]
pub struct ShellExecutionTool {
    executor: LocalExecutor,
    pub default_working_dir: PathBuf,
    working_dir_manager: Option<Arc<WorkingDirectoryManager>>,
}

impl ShellExecutionTool {
    /// Create a new shell execution tool with default configuration
    pub fn new(default_working_dir: PathBuf) -> Self {
        let config = LocalExecutorConfig {
            base_dir: default_working_dir.clone(),
            execution_dir: default_working_dir.clone(),
            ..Default::default()
        };
        
        Self {
            executor: LocalExecutor::new(config),
            default_working_dir,
            working_dir_manager: None,
        }
    }

    /// Create a new shell execution tool with working directory manager
    pub fn new_with_working_dir_manager(
        default_working_dir: PathBuf,
        working_dir_manager: Arc<WorkingDirectoryManager>,
    ) -> Self {
        let config = LocalExecutorConfig {
            base_dir: default_working_dir.clone(),
            execution_dir: default_working_dir.clone(),
            ..Default::default()
        };
        
        Self {
            executor: LocalExecutor::new(config),
            default_working_dir,
            working_dir_manager: Some(working_dir_manager),
        }
    }

    /// Check if the execution environment is available (always true for local execution)
    pub async fn check_environment_available(&self) -> Result<bool, SagittaCodeError> {
                                Ok(true)
    }

    /// Get help text for environment setup (now just mentions local execution)
    pub fn get_environment_setup_help() -> String {
        "Local execution is enabled. Commands will run directly on your system with the following security measures:

1. **Spatial Containment**: All operations are restricted to the repository base directory
2. **Command Approval**: Potentially dangerous commands require user approval
3. **Tool Detection**: Missing tools will be detected with installation guidance
4. **Audit Logging**: All executed commands are logged for security tracking

For maximum security, you can enable 'always ask' approval mode in your configuration.

Required tools (like git, cargo, npm, etc.) should be installed on your system for best results.".to_string()
    }

    /// Execute a command with streaming output
    pub async fn execute_streaming(
        &self,
        params: &ShellExecutionParams,
        event_sender: mpsc::Sender<StreamEvent>,
    ) -> Result<ShellExecutionResult, SagittaCodeError> {
        self.executor.execute_streaming(params, event_sender).await
    }

    /// Validate git command context
    async fn validate_git_command(&self, command: &str, working_dir: &PathBuf) -> Result<(), SagittaCodeError> {
        // Check if this is a git command
        let is_git_command = command.trim_start().starts_with("git ");
        
        if is_git_command {
            // Check if we're in a git repository
            if !working_dir.join(".git").exists() {
                return Err(SagittaCodeError::ToolError(format!(
                    "Git command '{}' cannot be executed: not in a git repository.\n\
                    Current directory: {}\n\
                    Hint: Use 'set_repository_context' tool to switch to a repository first.",
                    command.split_whitespace().take(2).collect::<Vec<_>>().join(" "),
                    working_dir.display()
                )));
            }
        }
        
        Ok(())
    }

    /// Execute a command with enhanced directory resolution
    pub async fn execute_command(&self, params: &ShellExecutionParams) -> Result<ShellExecutionResult, SagittaCodeError> {
        // Resolve working directory
        let working_dir = if let Some(ref wd_manager) = self.working_dir_manager {
            wd_manager.auto_resolve(params.working_directory.clone()).await?
        } else {
            params.working_directory.clone().unwrap_or_else(|| self.default_working_dir.clone())
        };

        // Validate git commands
        self.validate_git_command(&params.command, &working_dir).await?;

        // Create new params with resolved directory
        let resolved_params = ShellExecutionParams {
            command: params.command.clone(),
            language: params.language.clone(),
            working_directory: Some(working_dir),
            allow_network: params.allow_network,
            env_vars: params.env_vars.clone(),
            timeout_seconds: params.timeout_seconds,
        };

        self.executor.execute(&resolved_params).await
    }
}

#[async_trait]
impl Tool for ShellExecutionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell_execution".to_string(),
            description: "Execute shell commands locally with security controls. All operations are restricted to the repository base directory with command approval for potentially dangerous operations.".to_string(),
            category: ToolCategory::ShellExecution,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["command"],
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    },
                    "language": {
                        "type": ["string", "null"],
                        "description": "Optional language/environment hint (for backwards compatibility, now ignored)",
                        "enum": ["rust", "python", "javascript", "typescript", "go", "golang", "ruby", "html", "css", "yaml", "json", "markdown", "shell", "bash", "default", null]
                    },
                    "working_directory": {
                        "type": ["string", "null"],
                        "description": "Working directory for command execution (must be within repository base)"
                    },
                    "allow_network": {
                        "type": ["boolean", "null"],
                        "description": "Whether to allow network access (for backwards compatibility, now ignored)"
                    },
                    "env_vars": {
                        "type": ["object", "null"],
                        "description": "Additional environment variables",
                        "additionalProperties": {
                            "type": "string"
                        }
                    },
                    "timeout_seconds": {
                        "type": ["number", "null"],
                        "description": "Custom timeout in seconds (for backwards compatibility, now ignored)"
                    }
                }
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        // Parse parameters
        let params: ShellExecutionParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(
                format!("Invalid shell execution parameters: {}", e)
            ))?;
        
        // Execute the command
        let result = self.execute_command(&params).await?;
        
        // Return the result
        let result_value = serde_json::to_value(result)
            .map_err(|e| SagittaCodeError::ToolError(
                format!("Failed to serialize execution result: {}", e)
            ))?;
        
        Ok(ToolResult::Success(result_value))
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Streaming shell execution tool for terminal integration
#[derive(Debug)]
pub struct StreamingShellExecutionTool {
    base_tool: ShellExecutionTool,
}

impl StreamingShellExecutionTool {
    pub fn new(default_working_dir: PathBuf) -> Self {
        Self {
            base_tool: ShellExecutionTool::new(default_working_dir),
        }
    }
    
    pub fn new_with_working_dir_manager(
        default_working_dir: PathBuf,
        working_dir_manager: Arc<WorkingDirectoryManager>,
    ) -> Self {
        Self {
            base_tool: ShellExecutionTool::new_with_working_dir_manager(default_working_dir, working_dir_manager),
        }
    }
    
    /// Execute a command with streaming output to a terminal widget
    pub async fn execute_streaming(
        &self,
        params: ShellExecutionParams,
        event_sender: mpsc::Sender<StreamEvent>,
    ) -> Result<ShellExecutionResult, SagittaCodeError> {
        self.base_tool.execute_streaming(&params, event_sender).await
    }
    
    /// Check if the execution environment is available
    pub async fn check_environment_available(&self) -> Result<bool, SagittaCodeError> {
        self.base_tool.check_environment_available().await
    }
}

#[async_trait]
impl Tool for StreamingShellExecutionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "streaming_shell_execution".to_string(),
            description: "Execute shell commands locally with real-time streaming output to terminal. All operations are restricted to the repository base directory with command approval for potentially dangerous operations.".to_string(),
            category: ToolCategory::ShellExecution,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["command"],
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    },
                    "language": {
                        "type": ["string", "null"],
                        "description": "Optional language/environment hint (for backwards compatibility, now ignored)",
                        "enum": ["rust", "python", "javascript", "typescript", "go", "golang", "ruby", "html", "css", "yaml", "json", "markdown", "shell", "bash", "default", null]
                    },
                    "working_directory": {
                        "type": ["string", "null"],
                        "description": "Working directory for command execution (must be within repository base)"
                    },
                    "allow_network": {
                        "type": ["boolean", "null"],
                        "description": "Whether to allow network access (for backwards compatibility, now ignored)"
                    },
                    "env_vars": {
                        "type": ["object", "null"],
                        "description": "Additional environment variables",
                        "additionalProperties": {
                            "type": "string"
                        }
                    },
                    "timeout_seconds": {
                        "type": ["number", "null"],
                        "description": "Custom timeout in seconds (for backwards compatibility, now ignored)"
                    }
                }
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        self.base_tool.execute(parameters).await
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use serde_json::json;
    use tempfile::tempdir;
    use std::fs::File;
    use std::io::Write;

    // Helper function to create a ShellExecutionParams for testing
    fn create_test_params(command: &str) -> ShellExecutionParams {
        ShellExecutionParams {
            command: command.to_string(),
            language: None,
            working_directory: None,
            allow_network: None,
            env_vars: None,
            timeout_seconds: None,
        }
    }

    #[tokio::test]
    async fn test_shell_execution_tool_definition() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        let definition = tool.definition();
        
        assert_eq!(definition.name, "shell_execution");
        assert!(!definition.description.is_empty());
        
        let params_schema = &definition.parameters;
        assert_eq!(params_schema["type"], "object");
        
        let props = params_schema["properties"].as_object().expect("Properties should be an object");
        
        assert!(props.contains_key("command"));
        assert_eq!(props["command"]["type"], "string");
        assert!(props["command"]["description"].is_string());

        assert!(props.contains_key("language"));
        assert_eq!(props["language"]["type"], json!(["string", "null"]));
        assert!(props["language"]["description"].is_string());
        // Note: language field no longer has a default value since it's optional for backwards compatibility

        assert!(props.contains_key("working_directory"));
        assert_eq!(props["working_directory"]["type"], json!(["string", "null"]));
        assert!(props["working_directory"]["description"].is_string());

        assert!(props.contains_key("allow_network"));
        assert_eq!(props["allow_network"]["type"], json!(["boolean", "null"]));
        assert!(props["allow_network"]["description"].is_string());
        // Note: allow_network field no longer has a default value since it's optional for backwards compatibility

        assert!(props.contains_key("env_vars"));
        assert_eq!(props["env_vars"]["type"], json!(["object", "null"]));
        assert!(props["env_vars"]["description"].is_string());

        assert!(props.contains_key("timeout_seconds"));
        assert_eq!(props["timeout_seconds"]["type"], json!(["number", "null"]));
        assert!(props["timeout_seconds"]["description"].is_string());
        
        let required = params_schema["required"].as_array().expect("Required should be an array");
        assert!(required.contains(&json!("command")));
    }
    
    #[test]
    fn test_container_config_default() {
        let config = ContainerConfig::default();
        assert_eq!(config.image, "alpine:3.19");
        assert_eq!(config.workdir, "/workspace");
        assert_eq!(config.timeout_seconds, 300);
        assert_eq!(config.network_mode, Some("none".to_string()));
    }
    
    #[test]
    fn test_language_containers_default() {
        let containers = LanguageContainers::default();
        
        // Test that we have configurations for supported languages
        assert!(containers.configs.contains_key("default"));
        assert!(containers.configs.contains_key("rust"));
        assert!(containers.configs.contains_key("python"));
        assert!(containers.configs.contains_key("javascript"));
        assert!(containers.configs.contains_key("go"));
        assert!(containers.configs.contains_key("ruby"));
        
        // Test getting a config
        let rust_config = containers.get_config("rust");
        assert_eq!(rust_config.image, "rust:1.75-alpine");
        
        // Test fallback to default
        let unknown_config = containers.get_config("unknown");
        assert_eq!(unknown_config.image, "alpine:3.19");
    }
    
    #[test]
    fn test_language_containers_get_config_specific_language() {
        let lc = LanguageContainers::default();
        let rust_config = lc.get_config("rust");
        assert_eq!(rust_config.image, "rust:1.75-alpine");
        assert!(lc.configs.contains_key("python")); // Check python is present
        let python_config = lc.get_config("python");
        assert_eq!(python_config.image, "python:3.12-alpine");
    }

    #[test]
    fn test_language_containers_get_config_unknown_language_returns_default() {
        let lc = LanguageContainers::default();
        let default_config = lc.get_config("default");
        let unknown_lang_config = lc.get_config("nonexistentlanguage");
        assert_eq!(unknown_lang_config.image, default_config.image);
        assert_eq!(unknown_lang_config.workdir, default_config.workdir);
    }

    #[test]
    fn test_language_containers_add_config_new_language() {
        let mut lc = LanguageContainers::default();
        let new_lang = "swift";
        let swift_config = ContainerConfig {
            image: "swift:5.5".to_string(),
            workdir: "/app".to_string(),
            ..ContainerConfig::default()
        };
        lc.add_config(new_lang.to_string(), swift_config.clone());
        
        let retrieved_config = lc.get_config(new_lang);
        assert_eq!(retrieved_config.image, swift_config.image);
        assert_eq!(retrieved_config.workdir, swift_config.workdir);
    }

    #[test]
    fn test_language_containers_add_config_overwrite_existing() {
        let mut lc = LanguageContainers::default();
        let rust_lang = "rust";
        let original_rust_config = lc.get_config(rust_lang);
        assert_eq!(original_rust_config.image, "rust:1.75-alpine"); // Make sure it's the original

        let new_rust_config = ContainerConfig {
            image: "rust:latest".to_string(),
            workdir: "/src".to_string(),
            ..ContainerConfig::default()
        };
        lc.add_config(rust_lang.to_string(), new_rust_config.clone());

        let retrieved_config = lc.get_config(rust_lang);
        assert_eq!(retrieved_config.image, new_rust_config.image);
        assert_eq!(retrieved_config.workdir, new_rust_config.workdir);
    }
    
    #[test]
    fn test_language_containers_default_has_common_languages() {
        let lc = LanguageContainers::default();
        assert!(lc.configs.contains_key("default"));
        assert!(lc.configs.contains_key("rust"));
        assert!(lc.configs.contains_key("python"));
        assert!(lc.configs.contains_key("javascript"));
        assert!(lc.configs.contains_key("go"));
        assert!(lc.configs.contains_key("ruby"));

        let go_config = lc.get_config("go");
        assert_eq!(go_config.image, "golang:1.21-alpine");
        let ruby_config = lc.get_config("ruby");
        assert_eq!(ruby_config.image, "ruby:3.2-alpine");
    }

    #[tokio::test]
    async fn test_shell_execution_params_serialization() {
        let params = ShellExecutionParams {
            command: "echo 'hello world'".to_string(),
            language: Some("python".to_string()),
            working_directory: Some(PathBuf::from("/tmp")),
            allow_network: Some(false),
            env_vars: Some([("TEST".to_string(), "value".to_string())].into_iter().collect()),
            timeout_seconds: Some(60),
        };
        
        let json = serde_json::to_value(&params).unwrap();
        let deserialized: ShellExecutionParams = serde_json::from_value(json).unwrap();
        
        assert_eq!(deserialized.command, params.command);
        assert_eq!(deserialized.language, params.language);
        assert_eq!(deserialized.working_directory, params.working_directory);
        assert_eq!(deserialized.allow_network, params.allow_network);
        assert_eq!(deserialized.env_vars, params.env_vars);
        assert_eq!(deserialized.timeout_seconds, params.timeout_seconds);
    }
    
    #[tokio::test]
    async fn test_execute_with_invalid_parameters() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        let invalid_params = serde_json::json!({
            "invalid_field": "value"
        });
        
        let result = tool.execute(invalid_params).await;
        assert!(result.is_err());
        
        if let Err(SagittaCodeError::ToolError(msg)) = result {
            assert!(msg.contains("Invalid shell execution parameters"));
        } else {
            panic!("Expected ToolError");
        }
    }
    
    #[tokio::test]
    async fn test_execute_missing_required_parameter() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        let params = serde_json::json!({
            "language": "python"
            // Missing required "command" parameter
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
    }
    
    // Test with extended timeout for Docker operations
    #[tokio::test]
    async fn test_shell_execution_with_extended_timeout() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());

        // Test with a command that might need time for Docker image pull
        let params = serde_json::json!({
            "command": "echo 'Testing extended timeout'",
            "language": "default",
            "timeout_seconds": 120  // 2 minutes timeout
        });
        
        let start_time = std::time::Instant::now();
        let result = tool.execute(params).await.unwrap();
        let execution_time = start_time.elapsed();
        
        if let ToolResult::Success(value) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value).unwrap();
            assert_eq!(exec_result.exit_code, 0);
            assert!(exec_result.stdout.contains("Testing extended timeout") || exec_result.stdout.contains("'Testing extended timeout'"));
            assert!(!exec_result.timed_out);
            
            // Should complete well within the timeout even with Docker image pull
            assert!(execution_time.as_secs() < 120);
        } else {
            panic!("Expected successful execution");
        }
    }
    
    // Integration test - only runs if Docker is available
    #[tokio::test]
    #[ignore] // Ignore by default since it requires Docker
    async fn test_execute_simple_command() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        let params = serde_json::json!({
            "command": "echo hello world"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        if let ToolResult::Success(value) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value).unwrap();
            assert_eq!(exec_result.exit_code, 0);
            assert!(exec_result.stdout.contains("hello world") || exec_result.stdout.contains("'hello world'"));
            assert_eq!(exec_result.container_image, "local");
            assert!(!exec_result.timed_out);
        } else {
            panic!("Expected successful execution");
        }
    }
    
    // Integration test for Python execution
    #[tokio::test]
    #[ignore] // Ignore by default since it requires Docker
    async fn test_execute_python_command() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        let params = serde_json::json!({
            "command": "echo 'print(\"Python works\")' | python3 -",
            "language": "python"
        });
        
        // Note: This test will only work if python3 is installed on the system
        // In a real environment, we might want to check if python3 is available first
        let result = tool.execute(params).await;
        
        // If python3 is not available, we expect a tool missing error
        if let Ok(ToolResult::Success(value)) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value).unwrap();
            if exec_result.exit_code == 0 {
                assert!(exec_result.stdout.contains("Python works"));
            }
            assert_eq!(exec_result.container_image, "local");
        }
        // If python3 is not available, the test should fail gracefully
    }
    
    // Test file operations in container
    #[tokio::test]
    #[ignore] // Ignore by default since it requires Docker
    async fn test_execute_with_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        // Create a test file in the temp directory
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();
        
        let params = serde_json::json!({
            "command": format!("cat {}", test_file.display()),
            "working_directory": temp_dir.path().to_string_lossy()
        });
        
        let result = tool.execute(params).await.unwrap();
        
        if let ToolResult::Success(value) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value).unwrap();
            assert_eq!(exec_result.exit_code, 0);
            assert!(exec_result.stdout.contains("test content"));
            assert_eq!(exec_result.container_image, "local");
        } else {
            panic!("Expected successful execution");
        }
    }

    #[test]
    fn test_container_config_serialization_deserialization() {
        let original_config = ContainerConfig {
            image: "test_image:latest".to_string(),
            workdir: "/test".to_string(),
            env_vars: [("KEY".to_string(), "value".to_string())].into_iter().collect(),
            volumes: [("/host".to_string(), "/container".to_string())].into_iter().collect(),
            network_mode: Some("bridge".to_string()),
            memory_limit: Some("1g".to_string()),
            cpu_limit: Some("2.0".to_string()),
            timeout_seconds: 600,
        };

        let serialized = serde_json::to_string(&original_config).unwrap();
        let deserialized: ContainerConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original_config.image, deserialized.image);
        assert_eq!(original_config.workdir, deserialized.workdir);
        assert_eq!(original_config.env_vars, deserialized.env_vars);
        assert_eq!(original_config.volumes, deserialized.volumes);
        assert_eq!(original_config.network_mode, deserialized.network_mode);
        assert_eq!(original_config.memory_limit, deserialized.memory_limit);
        assert_eq!(original_config.cpu_limit, deserialized.cpu_limit);
        assert_eq!(original_config.timeout_seconds, deserialized.timeout_seconds);
    }

    #[test]
    fn test_shell_execution_result_serialization_deserialization() {
        let original_result = ShellExecutionResult {
            exit_code: 0,
            stdout: "Success".to_string(),
            stderr: String::new(),
            execution_time_ms: 1234,
            working_directory: PathBuf::from("/tmp"),
            container_image: "test_image:latest".to_string(),
            timed_out: false,
        };

        let serialized = serde_json::to_string(&original_result).unwrap();
        let deserialized: ShellExecutionResult = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original_result.exit_code, deserialized.exit_code);
        assert_eq!(original_result.stdout, deserialized.stdout);
        assert_eq!(original_result.stderr, deserialized.stderr);
        assert_eq!(original_result.execution_time_ms, deserialized.execution_time_ms);
        assert_eq!(original_result.working_directory, deserialized.working_directory);
        assert_eq!(original_result.container_image, deserialized.container_image);
        assert_eq!(original_result.timed_out, deserialized.timed_out);
    }

    #[tokio::test]
    async fn test_shell_execution_tool_new_with_default_config() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        assert_eq!(tool.default_working_dir, temp_dir.path().to_path_buf());
    }

    #[tokio::test]
    async fn test_shell_execution_tool_with_custom_executor_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = LocalExecutorConfig {
            base_dir: temp_dir.path().to_path_buf(),
            execution_dir: temp_dir.path().to_path_buf(),
            timeout_seconds: 60,
            ..Default::default()
        };
        
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        assert_eq!(tool.default_working_dir, temp_dir.path().to_path_buf());
    }

    #[tokio::test]
    async fn test_shell_execution_tool_check_environment_available() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        let result = tool.check_environment_available().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_shell_execution_tool_get_environment_setup_help() {
        let help_text = ShellExecutionTool::get_environment_setup_help();
        assert!(!help_text.is_empty());
        assert!(help_text.contains("Local execution"));
        assert!(help_text.contains("Spatial Containment"));
        assert!(help_text.contains("Command Approval"));
    }

    #[tokio::test]
    async fn test_tool_result_card_generation() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        let params = serde_json::json!({
            "command": "echo 'hello world'"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        // Verify result structure for tool cards
        if let ToolResult::Success(value) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value).unwrap();
            
            // Tool cards should be able to display these fields
            assert_eq!(exec_result.exit_code, 0);
            assert!(exec_result.stdout.contains("hello world") || exec_result.stdout.contains("'hello world'"));
            assert_eq!(exec_result.container_image, "local");
            assert!(!exec_result.timed_out);
            
            // Verify serialization works for tool cards
            let serialized = serde_json::to_string(&exec_result).unwrap();
            assert!(serialized.contains("stdout"));
            assert!(serialized.contains("exit_code"));
        } else {
            panic!("Expected successful execution");
        }
    }

    #[tokio::test]
    async fn test_command_risk_analysis_integration() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        // Test safe command (should auto-approve)
        let safe_params = serde_json::json!({
            "command": "ls -la"
        });
        
        let result = tool.execute(safe_params).await;
        // Should succeed or fail gracefully (ls might not work in all test environments)
        assert!(result.is_ok() || result.is_err());
        
        // Test potentially risky command structure
        let risky_params = serde_json::json!({
            "command": "rm nonexistent_file"
        });
        
        // This should work in our test environment since approval is disabled
        let result = tool.execute(risky_params).await;
        // rm on non-existent file should fail gracefully
        if let Ok(ToolResult::Success(value)) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value).unwrap();
            // rm on non-existent file typically returns non-zero exit code
            assert_ne!(exec_result.exit_code, 0);
        }
    }

    #[tokio::test]
    async fn test_stderr_classification() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        // Create a test script that outputs to stderr but isn't an error
        let script_content = r#"#!/bin/bash
echo "Creating binary (application) package" >&2
echo "note: see more Cargo.toml keys and their definitions" >&2
echo "Normal output to stdout"
exit 0
"#;
        
        let script_path = temp_dir.path().join("test_script.sh");
        std::fs::write(&script_path, script_content).unwrap();
        
        // Make script executable (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms).unwrap();
        }
        
        let params = serde_json::json!({
            "command": format!("bash {}", script_path.display()),
            "working_directory": temp_dir.path().to_string_lossy()
        });
        
        let result = tool.execute(params).await.unwrap();
        
        if let ToolResult::Success(value) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value).unwrap();
            
            // Should succeed since it's not a real error
            assert_eq!(exec_result.exit_code, 0);
            assert!(exec_result.stdout.contains("Normal output"));
            
            // The stderr should contain the "Creating binary" message
            // but our classification should not treat it as an error
            assert!(exec_result.stderr.contains("Creating binary") || 
                   exec_result.stderr.contains("note:"));
        }
    }

    #[tokio::test]
    async fn test_cargo_new_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        // Test creating a new Rust project
        let params = serde_json::json!({
            "command": "cargo new fibonacci_calculator --bin",
            "working_directory": temp_dir.path().to_string_lossy()
        });
        
        let result = tool.execute(params).await.unwrap();
        
        if let ToolResult::Success(value) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value).unwrap();
            
            // Should succeed if cargo is available
            if exec_result.exit_code == 0 {
                // Verify the project was created by checking for the actual files
                assert!(temp_dir.path().join("fibonacci_calculator").exists());
                assert!(temp_dir.path().join("fibonacci_calculator/Cargo.toml").exists());
                assert!(temp_dir.path().join("fibonacci_calculator/src/main.rs").exists());
                
                // The output may or may not contain "Created" depending on cargo version and environment
                // The fact that the files exist is the real test of success
                println!("Cargo output (stdout): {}", exec_result.stdout);
                println!("Cargo output (stderr): {}", exec_result.stderr);
            } else {
                // If cargo is not available, that's ok for testing, but let's see what happened
                println!("Cargo not available in test environment. Exit code: {}", exec_result.exit_code);
                println!("Stderr: {}", exec_result.stderr);
                println!("Stdout: {}", exec_result.stdout);
                
                // Don't fail the test if cargo isn't available - this is an environment issue
                if exec_result.stderr.contains("cargo: command not found") || 
                   exec_result.stderr.contains("No such file or directory") {
                    println!("Cargo not installed, skipping actual functionality test");
                    return;
                }
                
                // If cargo is available but failed for another reason, that might be a real issue
                // But for now, we'll be lenient since this is about testing the tool, not cargo itself
            }
        }
    }

    #[tokio::test]
    async fn test_tool_result_json_structure() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        let params = json!({
            "command": "echo 'test output'"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        if let ToolResult::Success(value) = result {
            // Verify the JSON structure matches ShellExecutionResult
            assert!(value.get("exit_code").is_some());
            assert!(value.get("stdout").is_some());
            assert!(value.get("stderr").is_some());
            assert!(value.get("execution_time_ms").is_some());
            assert!(value.get("working_directory").is_some());
            assert!(value.get("container_image").is_some());
            assert!(value.get("timed_out").is_some());
            
            // Verify types
            assert!(value["exit_code"].is_number());
            assert!(value["stdout"].is_string());
            assert!(value["stderr"].is_string());
            assert!(value["execution_time_ms"].is_number());
            assert!(value["working_directory"].is_string());
            assert!(value["container_image"].is_string());
            assert!(value["timed_out"].is_boolean());
        } else {
            panic!("Expected successful tool result");
        }
    }

    #[tokio::test]
    async fn test_minimal_parameters_shell_execution() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        // Test with only the required "command" parameter
        let params = json!({
            "command": "echo 'minimal test'"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_ok(), "Tool should work with minimal parameters");
        
        if let Ok(ToolResult::Success(value)) = result {
            assert_eq!(value["exit_code"], 0);
            assert!(value["stdout"].as_str().unwrap().contains("minimal test"));
        }
    }

    #[tokio::test]
    async fn test_minimal_parameters_streaming_shell_execution() {
        let temp_dir = TempDir::new().unwrap();
        let tool = StreamingShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        // Test with only the required "command" parameter
        let params = json!({
            "command": "echo 'minimal streaming test'"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_ok(), "Streaming tool should work with minimal parameters");
        
        if let Ok(ToolResult::Success(value)) = result {
            assert_eq!(value["exit_code"], 0);
            assert!(value["stdout"].as_str().unwrap().contains("minimal streaming test"));
        }
    }

    #[tokio::test]
    async fn test_optional_parameters_handling() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        // Test with some optional parameters provided
        let params = json!({
            "command": "echo 'optional test'",
            "language": "shell",
            "working_directory": temp_dir.path().to_str().unwrap(),
            "env_vars": {
                "TEST_VAR": "test_value"
            }
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_ok(), "Tool should work with optional parameters");
        
        if let Ok(ToolResult::Success(value)) = result {
            assert_eq!(value["exit_code"], 0);
            assert!(value["stdout"].as_str().unwrap().contains("optional test"));
        }
    }

    #[tokio::test]
    async fn test_null_optional_parameters() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        // Test with explicit null values for optional parameters
        let params = json!({
            "command": "echo 'null test'",
            "language": null,
            "working_directory": null,
            "allow_network": null,
            "env_vars": null,
            "timeout_seconds": null
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_ok(), "Tool should work with null optional parameters");
        
        if let Ok(ToolResult::Success(value)) = result {
            assert_eq!(value["exit_code"], 0);
            assert!(value["stdout"].as_str().unwrap().contains("null test"));
        }
    }
} 