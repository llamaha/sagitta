use std::sync::Arc;
use std::path::PathBuf;
use crate::providers::{Provider, ProviderType, ProviderConfig};
use crate::llm::client::LlmClient;
use super::mcp_integration::McpIntegration;
use crate::utils::errors::SagittaCodeError;
use crate::config::types::SagittaCodeConfig;
use crate::providers::types::ClaudeCodeConfig;
use super::client::ClaudeCodeClient;

/// Claude Code provider implementation
pub struct ClaudeCodeProvider;

impl ClaudeCodeProvider {
    pub fn new() -> Self {
        Self
    }
    
    /// Convert provider config to Claude Code specific config
    pub fn extract_claude_config(&self, config: &ProviderConfig) -> Result<crate::config::types::ClaudeCodeConfig, SagittaCodeError> {
        // Convert from provider config to the internal ClaudeCodeConfig type used by ClaudeCodeClient
        let provider_config: ClaudeCodeConfig = config.try_into()
            .map_err(|e: crate::providers::types::ConfigError| SagittaCodeError::ConfigError(e.to_string()))?;
        
        // Convert from provider ClaudeCodeConfig to internal ClaudeCodeConfig
        // The provider config has different fields, so we need to map them appropriately
        // Read model configuration from provider config options (set by initialization)
        let model = config.get_option::<String>("model")
            .unwrap_or_else(|_| None)
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
        let fallback_model = config.get_option::<String>("fallback_model")
            .unwrap_or_else(|_| None);
        let max_output_tokens = config.get_option::<u32>("max_output_tokens")
            .unwrap_or_else(|_| None)
            .unwrap_or(4096);
            
        Ok(crate::config::types::ClaudeCodeConfig {
            claude_path: provider_config.binary_path.unwrap_or_else(|| "claude".to_string()),
            model,
            fallback_model,
            max_output_tokens,
            debug: false,
            verbose: false,
            timeout: provider_config.timeout_seconds,
            max_turns: 0,
            output_format: "text".to_string(), // Default format
            input_format: "text".to_string(), // Default format
            dangerously_skip_permissions: false,
            allowed_tools: config.get_option::<Vec<String>>("allowed_tools")
                .unwrap_or_else(|_| None)
                .unwrap_or_default(),
            disallowed_tools: config.get_option::<Vec<String>>("disallowed_tools")
                .unwrap_or_else(|_| None)
                .unwrap_or_default(),
            additional_directories: vec![], // Provider config doesn't have this
            mcp_config: None, // Will be set up by client
            auto_ide: false, // Provider config doesn't have this
        })
    }
}

impl Provider for ClaudeCodeProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::ClaudeCode
    }

    fn display_name(&self) -> &str {
        "Claude Code"
    }

    fn create_client(
        &self,
        config: &ProviderConfig,
        mcp_integration: Arc<McpIntegration>
    ) -> Result<Box<dyn LlmClient>, SagittaCodeError> {
        // Convert the provider config to Claude Code config
        let claude_config = self.extract_claude_config(config)?;
        
        // Create a temporary SagittaCodeConfig for compatibility
        let sagitta_config = SagittaCodeConfig {
            claude_code: Some(claude_config),
            ..Default::default()
        };
        
        // Create the Claude Code client
        let mut client = ClaudeCodeClient::new(&sagitta_config)?;
        
        // Initialize with MCP integration - we'll need to modify ClaudeCodeClient
        // to accept the shared MCP integration instead of creating its own
        // For now, let the client manage its own MCP
        
        Ok(Box::new(client))
    }

    fn validate_config(&self, config: &ProviderConfig) -> Result<(), SagittaCodeError> {
        // Try to extract Claude Config to validate it
        let _claude_config = self.extract_claude_config(config)?;
        
        // Additional validation could be added here
        // For example, checking if the binary path exists, model is valid, etc.
        
        Ok(())
    }

    fn default_config(&self) -> ProviderConfig {
        // Create a default provider-level Claude Code config and convert it to provider config
        let claude_config = ClaudeCodeConfig::default();
        
        // Convert to ProviderConfig
        ProviderConfig::from(claude_config)
    }

    fn supported_features(&self) -> Vec<String> {
        vec![
            "streaming".to_string(),
            "mcp_tools".to_string(),
            "multi_turn".to_string(),
            "thinking_mode".to_string(),
            "system_prompts".to_string(),
        ]
    }

    fn requires_api_key(&self) -> bool {
        false // Claude Code uses binary, not API key
    }

    fn is_available(&self) -> bool {
        // Could check if claude binary is available
        // For now, assume it's available
        true
    }
    
    fn config_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "binary_path": {
                    "type": "string",
                    "description": "Path to the Claude Code binary",
                    "default": "claude"
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Timeout for Claude Code operations in seconds",
                    "default": 300
                }
            },
            "required": []
        })
    }
}