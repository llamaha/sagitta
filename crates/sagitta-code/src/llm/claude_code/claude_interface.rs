use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use crate::config::types::ClaudeCodeConfig;
use crate::utils::errors::SagittaCodeError;

/// Enhanced Claude CLI interface that supports all Claude binary options
pub struct ClaudeInterface {
    config: ClaudeCodeConfig,
}

impl ClaudeInterface {
    pub fn new(config: ClaudeCodeConfig) -> Self {
        Self { config }
    }

    /// Build a Command with all the configured Claude CLI options
    pub fn build_command(&self, prompt: Option<&str>) -> Result<Command, SagittaCodeError> {
        let mut cmd = Command::new(&self.config.claude_path);

        // Debug mode
        if self.config.debug {
            cmd.arg("--debug");
        }

        // Verbose mode
        if self.config.verbose {
            cmd.arg("--verbose");
        }

        // Print mode (for non-interactive output)
        cmd.arg("--print");

        // Output format
        cmd.arg("--output-format").arg(&self.config.output_format);

        // Input format  
        cmd.arg("--input-format").arg(&self.config.input_format);

        // Model selection
        cmd.arg("--model").arg(&self.config.model);

        // Fallback model
        if let Some(ref fallback) = self.config.fallback_model {
            cmd.arg("--fallback-model").arg(fallback);
        }

        // Dangerous skip permissions
        if self.config.dangerously_skip_permissions {
            cmd.arg("--dangerously-skip-permissions");
        }

        // Allowed tools
        if !self.config.allowed_tools.is_empty() {
            cmd.arg("--allowedTools").arg(self.config.allowed_tools.join(","));
        }

        // Disallowed tools
        if !self.config.disallowed_tools.is_empty() {
            cmd.arg("--disallowedTools").arg(self.config.disallowed_tools.join(","));
        }

        // Additional directories
        for dir in &self.config.additional_directories {
            cmd.arg("--add-dir").arg(dir);
        }

        // MCP configuration
        if let Some(ref mcp_config) = self.config.mcp_config {
            cmd.arg("--mcp-config").arg(mcp_config);
        }

        // IDE auto-connect
        if self.config.auto_ide {
            cmd.arg("--ide");
        }

        // Add the prompt if provided
        if let Some(prompt_text) = prompt {
            cmd.arg(prompt_text);
        }

        log::debug!("Claude command: {:?}", cmd);
        Ok(cmd)
    }

    /// Get the configured model information
    pub fn get_model_info(&self) -> Result<ClaudeModelInfo, SagittaCodeError> {
        use crate::llm::claude_code::models::ClaudeCodeModel;
        
        let model = ClaudeCodeModel::find_by_id(&self.config.model)
            .ok_or_else(|| SagittaCodeError::ConfigError(
                format!("Unknown Claude model: {}", self.config.model)
            ))?;

        Ok(ClaudeModelInfo {
            id: model.id.to_string(),
            name: model.name.to_string(),
            context_window: model.context_window,
            max_output_tokens: model.max_output_tokens,
            supports_thinking: model.supports_thinking,
            supports_images: model.supports_images,
            supports_prompt_cache: model.supports_prompt_cache,
        })
    }

    /// Validate the Claude binary and configuration
    pub async fn validate(&self) -> Result<(), SagittaCodeError> {
        // Check if Claude binary exists and is executable
        let mut cmd = Command::new(&self.config.claude_path);
        cmd.arg("--version");

        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    log::info!("Claude binary found: {}", version.trim());
                    Ok(())
                } else {
                    let error = String::from_utf8_lossy(&output.stderr);
                    Err(SagittaCodeError::ConfigError(
                        format!("Claude binary error: {}", error)
                    ))
                }
            }
            Err(e) => Err(SagittaCodeError::ConfigError(
                format!("Failed to execute Claude binary at '{}': {}", self.config.claude_path, e)
            ))
        }
    }

    /// Check Claude configuration and get available models
    pub async fn get_available_models(&self) -> Result<Vec<String>, SagittaCodeError> {
        // For now, return the predefined models from our models.rs
        use crate::llm::claude_code::models::CLAUDE_CODE_MODELS;
        
        Ok(CLAUDE_CODE_MODELS.iter()
            .map(|m| m.id.to_string())
            .collect())
    }

    /// Get Claude configuration info
    pub async fn get_config_info(&self) -> Result<ClaudeConfigInfo, SagittaCodeError> {
        Ok(ClaudeConfigInfo {
            claude_path: self.config.claude_path.clone(),
            model: self.config.model.clone(),
            fallback_model: self.config.fallback_model.clone(),
            debug: self.config.debug,
            verbose: self.config.verbose,
            output_format: self.config.output_format.clone(),
            input_format: self.config.input_format.clone(),
            timeout: self.config.timeout,
            max_turns: self.config.max_turns,
            dangerously_skip_permissions: self.config.dangerously_skip_permissions,
            allowed_tools: self.config.allowed_tools.clone(),
            disallowed_tools: self.config.disallowed_tools.clone(),
            additional_directories: self.config.additional_directories.clone(),
            mcp_config: self.config.mcp_config.clone(),
            auto_ide: self.config.auto_ide,
        })
    }
}

/// Information about a Claude model
#[derive(Debug, Clone)]
pub struct ClaudeModelInfo {
    pub id: String,
    pub name: String,
    pub context_window: usize,
    pub max_output_tokens: usize,
    pub supports_thinking: bool,
    pub supports_images: bool,
    pub supports_prompt_cache: bool,
}

/// Complete Claude configuration information
#[derive(Debug, Clone)]
pub struct ClaudeConfigInfo {
    pub claude_path: String,
    pub model: String,
    pub fallback_model: Option<String>,
    pub debug: bool,
    pub verbose: bool,
    pub output_format: String,
    pub input_format: String,
    pub timeout: u64,
    pub max_turns: u32,
    pub dangerously_skip_permissions: bool,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub additional_directories: Vec<PathBuf>,
    pub mcp_config: Option<String>,
    pub auto_ide: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::ClaudeCodeConfig;

    #[test]
    fn test_claude_interface_creation() {
        let config = ClaudeCodeConfig::default();
        let interface = ClaudeInterface::new(config);
        
        // Should create successfully
        assert_eq!(interface.config.claude_path, "claude");
    }

    #[test]
    fn test_build_command_basic() {
        let config = ClaudeCodeConfig::default();
        let interface = ClaudeInterface::new(config);
        
        let cmd = interface.build_command(Some("Hello")).unwrap();
        let cmd_str = format!("{:?}", cmd);
        
        assert!(cmd_str.contains("claude"));
        assert!(cmd_str.contains("--print"));
        assert!(cmd_str.contains("--model"));
        assert!(cmd_str.contains("Hello"));
    }

    #[test]
    fn test_build_command_with_options() {
        let mut config = ClaudeCodeConfig::default();
        config.debug = true;
        config.verbose = true;
        config.dangerously_skip_permissions = true;
        config.allowed_tools = vec!["Bash".to_string(), "Edit".to_string()];
        config.fallback_model = Some("claude-haiku-3-20240307".to_string());
        
        let interface = ClaudeInterface::new(config);
        let cmd = interface.build_command(Some("Test")).unwrap();
        let cmd_str = format!("{:?}", cmd);
        
        assert!(cmd_str.contains("--debug"));
        assert!(cmd_str.contains("--verbose"));
        assert!(cmd_str.contains("--dangerously-skip-permissions"));
        assert!(cmd_str.contains("--allowedTools"));
        assert!(cmd_str.contains("--fallback-model"));
    }

    #[tokio::test]
    async fn test_get_model_info() {
        let config = ClaudeCodeConfig::default();
        let interface = ClaudeInterface::new(config);
        
        let model_info = interface.get_model_info().unwrap();
        assert_eq!(model_info.id, "claude-sonnet-4-20250514");
        assert!(model_info.context_window > 0);
        assert!(model_info.supports_thinking);
    }

    #[tokio::test]
    async fn test_get_available_models() {
        let config = ClaudeCodeConfig::default();
        let interface = ClaudeInterface::new(config);
        
        let models = interface.get_available_models().await.unwrap();
        assert!(!models.is_empty());
        assert!(models.contains(&"claude-sonnet-4-20250514".to_string()));
    }
}