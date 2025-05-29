use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::tools::shell_execution::{ShellExecutionTool, ShellExecutionParams, ShellExecutionResult, LanguageContainers};
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Test framework configurations for different languages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFrameworkConfig {
    /// Test command template (use {test_path} placeholder)
    pub test_command: String,
    /// File patterns for test files
    pub test_file_patterns: Vec<String>,
    /// Directory patterns for test directories
    pub test_dir_patterns: Vec<String>,
    /// Additional setup commands to run before tests
    pub setup_commands: Vec<String>,
    /// Environment variables needed for testing
    pub test_env_vars: HashMap<String, String>,
}

/// Language-specific test configurations
#[derive(Debug, Clone)]
pub struct LanguageTestConfigs {
    configs: HashMap<String, TestFrameworkConfig>,
}

impl Default for LanguageTestConfigs {
    fn default() -> Self {
        let mut configs = HashMap::new();
        
        // Rust test configuration
        configs.insert("rust".to_string(), TestFrameworkConfig {
            test_command: "cargo test {test_path}".to_string(),
            test_file_patterns: vec![
                "**/tests/**/*.rs".to_string(),
                "**/*_test.rs".to_string(),
                "**/*_tests.rs".to_string(),
            ],
            test_dir_patterns: vec![
                "tests/".to_string(),
                "src/".to_string(), // For inline tests
            ],
            setup_commands: vec![
                "cargo check".to_string(),
            ],
            test_env_vars: [
                ("RUST_BACKTRACE".to_string(), "1".to_string()),
                ("CARGO_TERM_COLOR".to_string(), "always".to_string()),
            ].into_iter().collect(),
        });
        
        // Python test configuration
        configs.insert("python".to_string(), TestFrameworkConfig {
            test_command: "python -m pytest {test_path} -v".to_string(),
            test_file_patterns: vec![
                "**/test_*.py".to_string(),
                "**/*_test.py".to_string(),
                "**/tests/**/*.py".to_string(),
            ],
            test_dir_patterns: vec![
                "tests/".to_string(),
                "test/".to_string(),
            ],
            setup_commands: vec![
                "pip install pytest".to_string(),
                "pip install -r requirements.txt || true".to_string(),
                "pip install -e . || true".to_string(),
            ],
            test_env_vars: [
                ("PYTHONPATH".to_string(), "/workspace".to_string()),
                ("PYTEST_CURRENT_TEST".to_string(), "1".to_string()),
            ].into_iter().collect(),
        });
        
        // JavaScript/Node.js test configuration
        configs.insert("javascript".to_string(), TestFrameworkConfig {
            test_command: "npm test {test_path}".to_string(),
            test_file_patterns: vec![
                "**/*.test.js".to_string(),
                "**/*.spec.js".to_string(),
                "**/test/**/*.js".to_string(),
                "**/tests/**/*.js".to_string(),
            ],
            test_dir_patterns: vec![
                "test/".to_string(),
                "tests/".to_string(),
                "__tests__/".to_string(),
            ],
            setup_commands: vec![
                "npm install || yarn install || true".to_string(),
            ],
            test_env_vars: [
                ("NODE_ENV".to_string(), "test".to_string()),
            ].into_iter().collect(),
        });
        
        // TypeScript test configuration
        configs.insert("typescript".to_string(), TestFrameworkConfig {
            test_command: "npm test {test_path}".to_string(),
            test_file_patterns: vec![
                "**/*.test.ts".to_string(),
                "**/*.spec.ts".to_string(),
                "**/test/**/*.ts".to_string(),
                "**/tests/**/*.ts".to_string(),
            ],
            test_dir_patterns: vec![
                "test/".to_string(),
                "tests/".to_string(),
                "__tests__/".to_string(),
            ],
            setup_commands: vec![
                "npm install || yarn install || true".to_string(),
                "npx tsc --noEmit || true".to_string(), // Type check
            ],
            test_env_vars: [
                ("NODE_ENV".to_string(), "test".to_string()),
            ].into_iter().collect(),
        });
        
        // Go test configuration
        configs.insert("go".to_string(), TestFrameworkConfig {
            test_command: "go test {test_path} -v".to_string(),
            test_file_patterns: vec![
                "**/*_test.go".to_string(),
            ],
            test_dir_patterns: vec![
                "./".to_string(), // Go tests are typically alongside source
            ],
            setup_commands: vec![
                "go mod download || true".to_string(),
                "go mod tidy || true".to_string(),
            ],
            test_env_vars: [
                ("GO111MODULE".to_string(), "on".to_string()),
            ].into_iter().collect(),
        });
        
        // Ruby test configuration
        configs.insert("ruby".to_string(), TestFrameworkConfig {
            test_command: "bundle exec rspec {test_path}".to_string(),
            test_file_patterns: vec![
                "**/spec/**/*_spec.rb".to_string(),
                "**/test/**/*_test.rb".to_string(),
            ],
            test_dir_patterns: vec![
                "spec/".to_string(),
                "test/".to_string(),
            ],
            setup_commands: vec![
                "bundle install || gem install rspec || true".to_string(),
            ],
            test_env_vars: [
                ("RAILS_ENV".to_string(), "test".to_string()),
            ].into_iter().collect(),
        });
        
        Self { configs }
    }
}

impl LanguageTestConfigs {
    pub fn get_config(&self, language: &str) -> Option<&TestFrameworkConfig> {
        self.configs.get(language)
    }
    
    pub fn add_config(&mut self, language: String, config: TestFrameworkConfig) {
        self.configs.insert(language, config);
    }
    
    pub fn supported_languages(&self) -> Vec<&String> {
        self.configs.keys().collect()
    }
}

/// Parameters for test execution
#[derive(Debug, Serialize, Deserialize)]
pub struct TestExecutionParams {
    /// Language/framework to use for testing
    pub language: String,
    /// Specific test file or directory to run (optional, runs all tests if not specified)
    pub test_path: Option<String>,
    /// Working directory containing the project
    pub working_directory: Option<PathBuf>,
    /// Whether to run setup commands before tests
    pub run_setup: Option<bool>,
    /// Additional environment variables for testing
    pub env_vars: Option<HashMap<String, String>>,
    /// Custom timeout in seconds
    pub timeout_seconds: Option<u64>,
    /// Whether to allow network access during tests
    pub allow_network: Option<bool>,
    /// Custom test command (overrides default for language)
    pub custom_command: Option<String>,
}

/// Result of test execution
#[derive(Debug, Serialize, Deserialize)]
pub struct TestExecutionResult {
    /// Language/framework used
    pub language: String,
    /// Test command that was executed
    pub command: String,
    /// Exit code of the test command
    pub exit_code: i32,
    /// Standard output from tests
    pub stdout: String,
    /// Standard error from tests
    pub stderr: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Container used for execution
    pub container_image: String,
    /// Whether the command timed out
    pub timed_out: bool,
    /// Setup commands that were run
    pub setup_commands: Vec<String>,
    /// Whether tests passed (exit_code == 0)
    pub tests_passed: bool,
}

/// Test execution tool for running language-specific tests in containers
#[derive(Debug)]
pub struct TestExecutionTool {
    shell_tool: ShellExecutionTool,
    test_configs: LanguageTestConfigs,
}

impl TestExecutionTool {
    pub fn new(default_working_dir: PathBuf) -> Self {
        Self {
            shell_tool: ShellExecutionTool::new(default_working_dir),
            test_configs: LanguageTestConfigs::default(),
        }
    }
    
    pub fn with_language_containers(mut self, containers: LanguageContainers) -> Self {
        self.shell_tool = self.shell_tool.with_language_containers(containers);
        self
    }
    
    pub fn with_test_configs(mut self, configs: LanguageTestConfigs) -> Self {
        self.test_configs = configs;
        self
    }
    
    /// Discover test files in the working directory
    pub async fn discover_tests(
        &self,
        language: &str,
        working_dir: &PathBuf,
    ) -> Result<Vec<PathBuf>, SagittaCodeError> {
        let config = self.test_configs.get_config(language)
            .ok_or_else(|| SagittaCodeError::ToolError(
                format!("Unsupported language for testing: {}", language)
            ))?;
        
        let mut test_files = Vec::new();
        
        // Use shell execution to find test files
        for pattern in &config.test_file_patterns {
            let find_command = format!("find . -name '{}' -type f", pattern);
            
            let params = ShellExecutionParams {
                command: find_command,
                language: Some("default".to_string()),
                working_directory: Some(working_dir.clone()),
                allow_network: Some(false),
                env_vars: None,
                timeout_seconds: Some(30),
            };
            
            if let Ok(result) = self.shell_tool.execute(serde_json::to_value(params)?).await {
                if let ToolResult::Success(value) = result {
                    let exec_result: ShellExecutionResult = serde_json::from_value(value)?;
                    if exec_result.exit_code == 0 {
                        for line in exec_result.stdout.lines() {
                            let line = line.trim();
                            if !line.is_empty() && line.starts_with("./") {
                                test_files.push(PathBuf::from(line.strip_prefix("./").unwrap_or(line)));
                            }
                        }
                    }
                }
            }
        }
        
        Ok(test_files)
    }
    
    /// Execute setup commands for a language
    async fn run_setup_commands(
        &self,
        language: &str,
        working_dir: &PathBuf,
        env_vars: &HashMap<String, String>,
    ) -> Result<Vec<String>, SagittaCodeError> {
        let config = self.test_configs.get_config(language)
            .ok_or_else(|| SagittaCodeError::ToolError(
                format!("Unsupported language for testing: {}", language)
            ))?;
        
        let mut executed_commands = Vec::new();
        
        for setup_cmd in &config.setup_commands {
            let params = ShellExecutionParams {
                command: setup_cmd.clone(),
                language: Some(language.to_string()),
                working_directory: Some(working_dir.clone()),
                allow_network: Some(true), // Setup might need network for dependencies
                env_vars: Some(env_vars.clone()),
                timeout_seconds: Some(300), // 5 minutes for setup
            };
            
            executed_commands.push(setup_cmd.clone());
            
            // Execute setup command but don't fail if it fails (some are optional)
            let _result = self.shell_tool.execute(serde_json::to_value(params)?).await;
        }
        
        Ok(executed_commands)
    }
    
    /// Execute tests for a specific language
    async fn execute_tests(
        &self,
        params: &TestExecutionParams,
    ) -> Result<TestExecutionResult, SagittaCodeError> {
        let config = self.test_configs.get_config(&params.language)
            .ok_or_else(|| SagittaCodeError::ToolError(
                format!("Unsupported language for testing: {}", params.language)
            ))?;
        
        let working_dir = params.working_directory
            .as_ref()
            .unwrap_or(&self.shell_tool.default_working_dir);
        
        // Prepare environment variables
        let mut env_vars = config.test_env_vars.clone();
        if let Some(custom_env) = &params.env_vars {
            env_vars.extend(custom_env.clone());
        }
        
        // Run setup commands if requested
        let setup_commands = if params.run_setup.unwrap_or(true) {
            self.run_setup_commands(&params.language, working_dir, &env_vars).await?
        } else {
            Vec::new()
        };
        
        // Prepare test command
        let test_command = if let Some(custom_cmd) = &params.custom_command {
            custom_cmd.clone()
        } else {
            let test_path = params.test_path.as_deref().unwrap_or(".");
            config.test_command.replace("{test_path}", test_path)
        };
        
        // Execute the test command
        let shell_params = ShellExecutionParams {
            command: test_command.clone(),
            language: Some(params.language.clone()),
            working_directory: Some(working_dir.clone()),
            allow_network: params.allow_network,
            env_vars: Some(env_vars),
            timeout_seconds: params.timeout_seconds,
        };
        
        let result = self.shell_tool.execute(serde_json::to_value(shell_params)?).await?;
        
        if let ToolResult::Success(value) = result {
            let exec_result: ShellExecutionResult = serde_json::from_value(value)?;
            
            Ok(TestExecutionResult {
                language: params.language.clone(),
                command: test_command,
                exit_code: exec_result.exit_code,
                stdout: exec_result.stdout,
                stderr: exec_result.stderr,
                execution_time_ms: exec_result.execution_time_ms,
                container_image: exec_result.container_image,
                timed_out: exec_result.timed_out,
                setup_commands,
                tests_passed: exec_result.exit_code == 0,
            })
        } else {
            Err(SagittaCodeError::ToolError(
                "Failed to execute test command".to_string()
            ))
        }
    }
}

#[async_trait]
impl Tool for TestExecutionTool {
    fn definition(&self) -> ToolDefinition {
        let supported_langs = self.test_configs.supported_languages()
            .into_iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        
        ToolDefinition {
            name: "test_execution".to_string(),
            description: format!(
                "Execute language-specific tests in isolated Docker containers. Supports: {}",
                supported_langs
            ),
            category: ToolCategory::TestExecution,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "description": format!("Programming language/framework ({})", supported_langs),
                        "enum": self.test_configs.supported_languages()
                    },
                    "test_path": {
                        "type": "string",
                        "description": "Specific test file or directory to run (optional, runs all tests if not specified)"
                    },
                    "working_directory": {
                        "type": "string",
                        "description": "Working directory containing the project"
                    },
                    "run_setup": {
                        "type": "boolean",
                        "description": "Whether to run setup commands before tests (default: true)",
                        "default": true
                    },
                    "env_vars": {
                        "type": "object",
                        "description": "Additional environment variables for testing as JSON object"
                    },
                    "timeout_seconds": {
                        "type": "number",
                        "description": "Timeout for test execution in seconds"
                    },
                    "allow_network": {
                        "type": "boolean",
                        "description": "Whether to allow network access during tests (default: false)",
                        "default": false
                    },
                    "custom_command": {
                        "type": "string",
                        "description": "Custom test command (overrides default for language)"
                    }
                },
                "required": ["language"]
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        // Parse parameters
        let params: TestExecutionParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(
                format!("Invalid parameters: {}", e)
            ))?;
        
        // Execute tests
        let result = self.execute_tests(&params).await?;
        
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
    async fn test_test_execution_tool_definition() {
        let temp_dir = TempDir::new().unwrap();
        let tool = TestExecutionTool::new(temp_dir.path().to_path_buf());
        let definition = tool.definition();
        
        assert_eq!(definition.name, "test_execution");
        assert!(definition.description.contains("Execute language-specific tests"));
        
        // Check parameters structure
        let params = &definition.parameters;
        assert!(params.get("type").is_some());
        assert!(params.get("properties").is_some());
        assert!(params.get("required").is_some());
        
        // Check required parameters
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("language".to_string())));
        
        // Check properties
        let properties = params.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("language"));
    }
    
    #[tokio::test]
    async fn test_language_test_configs_default() {
        let configs = LanguageTestConfigs::default();
        
        // Test that we have configurations for supported languages
        assert!(configs.get_config("rust").is_some());
        assert!(configs.get_config("python").is_some());
        assert!(configs.get_config("javascript").is_some());
        assert!(configs.get_config("typescript").is_some());
        assert!(configs.get_config("go").is_some());
        assert!(configs.get_config("ruby").is_some());
        
        // Test unsupported language
        assert!(configs.get_config("unknown").is_none());
        
        // Test specific configuration
        let rust_config = configs.get_config("rust").unwrap();
        assert!(rust_config.test_command.contains("cargo test"));
        assert!(rust_config.test_file_patterns.iter().any(|p| p.contains("*_test.rs")));
    }
    
    #[tokio::test]
    async fn test_test_framework_config_serialization() {
        let config = TestFrameworkConfig {
            test_command: "cargo test {test_path}".to_string(),
            test_file_patterns: vec!["*_test.rs".to_string()],
            test_dir_patterns: vec!["tests/".to_string()],
            setup_commands: vec!["cargo check".to_string()],
            test_env_vars: [("RUST_BACKTRACE".to_string(), "1".to_string())].into_iter().collect(),
        };
        
        let json = serde_json::to_value(&config).unwrap();
        let deserialized: TestFrameworkConfig = serde_json::from_value(json).unwrap();
        
        assert_eq!(deserialized.test_command, config.test_command);
        assert_eq!(deserialized.test_file_patterns, config.test_file_patterns);
        assert_eq!(deserialized.test_dir_patterns, config.test_dir_patterns);
        assert_eq!(deserialized.setup_commands, config.setup_commands);
        assert_eq!(deserialized.test_env_vars, config.test_env_vars);
    }
    
    #[tokio::test]
    async fn test_test_execution_params_serialization() {
        let params = TestExecutionParams {
            language: "rust".to_string(),
            test_path: Some("tests/integration_test.rs".to_string()),
            working_directory: Some(PathBuf::from("/tmp")),
            run_setup: Some(true),
            env_vars: Some([("TEST_ENV".to_string(), "test".to_string())].into_iter().collect()),
            timeout_seconds: Some(300),
            allow_network: Some(false),
            custom_command: Some("cargo test --release".to_string()),
        };
        
        let json = serde_json::to_value(&params).unwrap();
        let deserialized: TestExecutionParams = serde_json::from_value(json).unwrap();
        
        assert_eq!(deserialized.language, params.language);
        assert_eq!(deserialized.test_path, params.test_path);
        assert_eq!(deserialized.working_directory, params.working_directory);
        assert_eq!(deserialized.run_setup, params.run_setup);
        assert_eq!(deserialized.env_vars, params.env_vars);
        assert_eq!(deserialized.timeout_seconds, params.timeout_seconds);
        assert_eq!(deserialized.allow_network, params.allow_network);
        assert_eq!(deserialized.custom_command, params.custom_command);
    }
    
    #[tokio::test]
    async fn test_execute_with_invalid_parameters() {
        let temp_dir = TempDir::new().unwrap();
        let tool = TestExecutionTool::new(temp_dir.path().to_path_buf());
        
        let invalid_params = serde_json::json!({
            "invalid_field": "value"
        });
        
        let result = tool.execute(invalid_params).await;
        assert!(result.is_err());
        
        if let Err(SagittaCodeError::ToolError(msg)) = result {
            assert!(msg.contains("Invalid parameters"));
        } else {
            panic!("Expected ToolError");
        }
    }
    
    #[tokio::test]
    async fn test_execute_with_unsupported_language() {
        let temp_dir = TempDir::new().unwrap();
        let tool = TestExecutionTool::new(temp_dir.path().to_path_buf());
        
        let params = serde_json::json!({
            "language": "unsupported_language"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        
        if let Err(SagittaCodeError::ToolError(msg)) = result {
            assert!(msg.contains("Unsupported language"));
        } else {
            panic!("Expected ToolError");
        }
    }
    
    #[tokio::test]
    async fn test_supported_languages() {
        let configs = LanguageTestConfigs::default();
        let languages = configs.supported_languages();
        
        assert!(languages.contains(&&"rust".to_string()));
        assert!(languages.contains(&&"python".to_string()));
        assert!(languages.contains(&&"javascript".to_string()));
        assert!(languages.contains(&&"typescript".to_string()));
        assert!(languages.contains(&&"go".to_string()));
        assert!(languages.contains(&&"ruby".to_string()));
    }
    
    // Integration test - only runs if Docker is available
    #[tokio::test]
    #[ignore] // Ignore by default since it requires Docker
    async fn test_discover_tests_integration() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create some test files
        fs::create_dir_all(temp_dir.path().join("tests")).unwrap();
        fs::write(temp_dir.path().join("tests/test_example.py"), "# test file").unwrap();
        fs::write(temp_dir.path().join("test_another.py"), "# another test").unwrap();
        
        let tool = TestExecutionTool::new(temp_dir.path().to_path_buf());
        
        // Skip test if Docker is not available
        if !tool.shell_tool.check_docker_available().await.unwrap() {
            return;
        }
        
        let test_files = tool.discover_tests("python", &temp_dir.path().to_path_buf()).await.unwrap();
        
        // Should find both test files
        assert!(test_files.len() >= 1);
        assert!(test_files.iter().any(|f| f.to_string_lossy().contains("test_example.py")));
    }
} 