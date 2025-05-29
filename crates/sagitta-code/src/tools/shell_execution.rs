use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::FredAgentError;

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
            image: "megabytelabs/devcontainer:latest".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: HashMap::new(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()), // Isolated by default
            memory_limit: Some("512m".to_string()),
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
        
        // General devtools container
        configs.insert("default".to_string(), ContainerConfig::default());
        
        // Rust-specific container
        configs.insert("rust".to_string(), ContainerConfig {
            image: "rust:1.75".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("CARGO_HOME".to_string(), "/usr/local/cargo".to_string()),
                ("RUSTUP_HOME".to_string(), "/usr/local/rustup".to_string()),
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("1g".to_string()),
            cpu_limit: Some("2.0".to_string()),
            timeout_seconds: 600, // 10 minutes for compilation
        });
        
        // Python-specific container
        configs.insert("python".to_string(), ContainerConfig {
            image: "python:3.11".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("PYTHONPATH".to_string(), "/workspace".to_string()),
                ("PYTHONUNBUFFERED".to_string(), "1".to_string()),
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300,
        });
        
        // Node.js-specific container
        configs.insert("javascript".to_string(), ContainerConfig {
            image: "node:20".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("NODE_ENV".to_string(), "development".to_string()),
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300,
        });
        
        // Go-specific container
        configs.insert("go".to_string(), ContainerConfig {
            image: "golang:1.21".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: [
                ("GOPATH".to_string(), "/go".to_string()),
                ("GO111MODULE".to_string(), "on".to_string()),
            ].into_iter().collect(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300,
        });
        
        // Ruby-specific container
        configs.insert("ruby".to_string(), ContainerConfig {
            image: "ruby:3.1".to_string(),
            workdir: "/workspace".to_string(),
            env_vars: HashMap::new(),
            volumes: HashMap::new(),
            network_mode: Some("none".to_string()),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.0".to_string()),
            timeout_seconds: 300,
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
#[derive(Debug, Serialize, Deserialize)]
pub struct ShellExecutionParams {
    /// The command to execute
    pub command: String,
    /// Optional language/environment to use (determines container)
    pub language: Option<String>,
    /// Working directory to mount (defaults to current repository)
    pub working_directory: Option<PathBuf>,
    /// Whether to allow network access
    pub allow_network: Option<bool>,
    /// Additional environment variables
    pub env_vars: Option<HashMap<String, String>>,
    /// Custom timeout in seconds
    pub timeout_seconds: Option<u64>,
}

/// Result of shell command execution
#[derive(Debug, Serialize, Deserialize)]
pub struct ShellExecutionResult {
    /// Exit code of the command
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Container used for execution
    pub container_image: String,
    /// Whether the command timed out
    pub timed_out: bool,
}

/// Shell execution tool for running commands in isolated containers
#[derive(Debug)]
pub struct ShellExecutionTool {
    language_containers: LanguageContainers,
    pub default_working_dir: PathBuf,
}

impl ShellExecutionTool {
    pub fn new(default_working_dir: PathBuf) -> Self {
        Self {
            language_containers: LanguageContainers::default(),
            default_working_dir,
        }
    }
    
    pub fn with_language_containers(mut self, containers: LanguageContainers) -> Self {
        self.language_containers = containers;
        self
    }
    
    /// Check if Docker is available
    pub async fn check_docker_available(&self) -> Result<bool, FredAgentError> {
        let output = Command::new("docker")
            .arg("--version")
            .output()
            .await;
            
        match output {
            Ok(output) => Ok(output.status.success()),
            Err(_) => Ok(false),
        }
    }
    
    /// Execute a command in a container
    async fn execute_in_container(
        &self,
        params: &ShellExecutionParams,
        config: &ContainerConfig,
    ) -> Result<ShellExecutionResult, FredAgentError> {
        let start_time = std::time::Instant::now();
        
        // Prepare Docker command
        let mut docker_cmd = Command::new("docker");
        docker_cmd.arg("run")
            .arg("--rm")
            .arg("--interactive");
        
        // Set working directory
        docker_cmd.arg("--workdir").arg(&config.workdir);
        
        // Set memory limit
        if let Some(memory) = &config.memory_limit {
            docker_cmd.arg("--memory").arg(memory);
        }
        
        // Set CPU limit
        if let Some(cpu) = &config.cpu_limit {
            docker_cmd.arg("--cpus").arg(cpu);
        }
        
        // Set network mode
        if let Some(network) = &config.network_mode {
            if !params.allow_network.unwrap_or(false) {
                docker_cmd.arg("--network").arg(network);
            }
        }
        
        // Add environment variables
        for (key, value) in &config.env_vars {
            docker_cmd.arg("-e").arg(format!("{}={}", key, value));
        }
        
        // Add custom environment variables
        if let Some(env_vars) = &params.env_vars {
            for (key, value) in env_vars {
                docker_cmd.arg("-e").arg(format!("{}={}", key, value));
            }
        }
        
        // Add volume mounts
        let working_dir = params.working_directory
            .as_ref()
            .unwrap_or(&self.default_working_dir);
        
        docker_cmd.arg("-v").arg(format!("{}:{}", 
            working_dir.display(), 
            config.workdir
        ));
        
        // Add additional volume mounts
        for (host_path, container_path) in &config.volumes {
            docker_cmd.arg("-v").arg(format!("{}:{}", host_path, container_path));
        }
        
        // Add the container image
        docker_cmd.arg(&config.image);
        
        // Add the command to execute
        docker_cmd.arg("sh").arg("-c").arg(&params.command);
        
        // Set up stdio
        docker_cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        // Execute the command
        let mut child = docker_cmd.spawn()
            .map_err(|e| FredAgentError::ToolError(
                format!("Failed to spawn Docker command: {}", e)
            ))?;
        
        // Set up timeout
        let timeout_duration = std::time::Duration::from_secs(
            params.timeout_seconds.unwrap_or(config.timeout_seconds)
        );
        
        // Wait for completion or timeout
        let result = tokio::time::timeout(timeout_duration, async {
            child.wait_with_output().await
        }).await;
        
        let execution_time = start_time.elapsed();
        
        match result {
            Ok(Ok(output)) => {
                Ok(ShellExecutionResult {
                    exit_code: output.status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    execution_time_ms: execution_time.as_millis() as u64,
                    container_image: config.image.clone(),
                    timed_out: false,
                })
            }
            Ok(Err(e)) => {
                Err(FredAgentError::ToolError(
                    format!("Command execution failed: {}", e)
                ))
            }
            Err(_) => {
                // Timeout occurred, try to kill the process if it's still running
                // Note: child is moved into the timeout future, so we can't access it here
                // Docker containers should be cleaned up automatically when the process exits
                Ok(ShellExecutionResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: "Command timed out".to_string(),
                    execution_time_ms: execution_time.as_millis() as u64,
                    container_image: config.image.clone(),
                    timed_out: true,
                })
            }
        }
    }
}

#[async_trait]
impl Tool for ShellExecutionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell_execution".to_string(),
            description: "Execute shell commands in isolated Docker containers for safe code execution and testing".to_string(),
            category: ToolCategory::ShellExecution,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    },
                    "language": {
                        "type": "string",
                        "description": "Programming language/environment (rust, python, javascript, go, ruby, or default)",
                        "default": "default"
                    },
                    "working_directory": {
                        "type": "string",
                        "description": "Working directory to mount into container"
                    },
                    "allow_network": {
                        "type": "boolean",
                        "description": "Whether to allow network access (default: false for security)",
                        "default": false
                    },
                    "env_vars": {
                        "type": "object",
                        "description": "Additional environment variables as JSON object"
                    },
                    "timeout_seconds": {
                        "type": "number",
                        "description": "Timeout for command execution in seconds"
                    }
                },
                "required": ["command"]
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, FredAgentError> {
        // Check if Docker is available
        if !self.check_docker_available().await? {
            return Err(FredAgentError::ToolError(
                "Docker is not available. Please install Docker to use shell execution.".to_string()
            ));
        }
        
        // Parse parameters
        let params: ShellExecutionParams = serde_json::from_value(parameters)
            .map_err(|e| FredAgentError::ToolError(
                format!("Invalid parameters: {}", e)
            ))?;
        
        // Get container configuration
        let language = params.language.as_deref().unwrap_or("default");
        let config = self.language_containers.get_config(language);
        
        // Execute the command
        let result = self.execute_in_container(&params, config).await?;
        
        // Return the result
        Ok(ToolResult::Success(serde_json::to_value(result)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    
    #[tokio::test]
    async fn test_shell_execution_tool_definition() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        let definition = tool.definition();
        
        assert_eq!(definition.name, "shell_execution");
        assert!(!definition.description.is_empty());
        
        // Check parameters structure
        let params = &definition.parameters;
        assert!(params.get("type").is_some());
        assert!(params.get("properties").is_some());
        assert!(params.get("required").is_some());
        
        // Check required parameters
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("command".to_string())));
        
        // Check properties
        let properties = params.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("command"));
        assert!(properties.contains_key("language"));
    }
    
    #[tokio::test]
    async fn test_container_config_default() {
        let config = ContainerConfig::default();
        assert_eq!(config.image, "megabytelabs/devcontainer:latest");
        assert_eq!(config.workdir, "/workspace");
        assert_eq!(config.timeout_seconds, 300);
        assert_eq!(config.network_mode, Some("none".to_string()));
    }
    
    #[tokio::test]
    async fn test_language_containers_default() {
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
        assert_eq!(rust_config.image, "rust:1.75");
        
        // Test fallback to default
        let unknown_config = containers.get_config("unknown");
        assert_eq!(unknown_config.image, "megabytelabs/devcontainer:latest");
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
    async fn test_docker_availability_check() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        // This test will pass if Docker is installed, fail if not
        // In a real environment, we'd mock this
        let _is_available = tool.check_docker_available().await.unwrap();
        // We can't assert the result since it depends on the test environment
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
        
        if let Err(FredAgentError::ToolError(msg)) = result {
            assert!(msg.contains("Invalid parameters"));
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
    
    // Integration test - only runs if Docker is available
    #[tokio::test]
    #[ignore] // Ignore by default since it requires Docker
    async fn test_execute_simple_command() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        // Skip test if Docker is not available
        if !tool.check_docker_available().await.unwrap() {
            return;
        }
        
        let params = serde_json::json!({
            "command": "echo 'Hello, World!'",
            "language": "default"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        if let ToolResult::Success(value) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value).unwrap();
            assert_eq!(exec_result.exit_code, 0);
            assert!(exec_result.stdout.contains("Hello, World!"));
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
        
        // Skip test if Docker is not available
        if !tool.check_docker_available().await.unwrap() {
            return;
        }
        
        let params = serde_json::json!({
            "command": "python3 -c \"print('Python is working!')\"",
            "language": "python"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        if let ToolResult::Success(value) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value).unwrap();
            assert_eq!(exec_result.exit_code, 0);
            assert!(exec_result.stdout.contains("Python is working!"));
            assert_eq!(exec_result.container_image, "python:3.11");
        } else {
            panic!("Expected successful execution");
        }
    }
    
    // Test file operations in container
    #[tokio::test]
    #[ignore] // Ignore by default since it requires Docker
    async fn test_execute_with_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create a test file in the temp directory
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Hello from file!").unwrap();
        
        let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
        
        // Skip test if Docker is not available
        if !tool.check_docker_available().await.unwrap() {
            return;
        }
        
        let params = serde_json::json!({
            "command": "cat test.txt",
            "language": "default",
            "working_directory": temp_dir.path().to_string_lossy()
        });
        
        let result = tool.execute(params).await.unwrap();
        
        if let ToolResult::Success(value) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value).unwrap();
            assert_eq!(exec_result.exit_code, 0);
            assert!(exec_result.stdout.contains("Hello from file!"));
        } else {
            panic!("Expected successful execution");
        }
    }
} 