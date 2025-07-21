//! Claude Code Router provider - wraps ClaudeCodeProvider for router functionality

use crate::providers::{Provider, ProviderType, ProviderConfig, claude_code::ClaudeCodeProvider};
use crate::llm::client::LlmClient;
use crate::utils::errors::SagittaCodeError;
use std::sync::Arc;

/// Claude Code Router provider - uses the same implementation as ClaudeCode
/// but identifies as ClaudeCodeRouter for configuration purposes
pub struct ClaudeCodeRouterProvider {
    inner: ClaudeCodeProvider,
}

impl ClaudeCodeRouterProvider {
    pub fn new() -> Self {
        Self {
            inner: ClaudeCodeProvider::new(),
        }
    }
}

impl Provider for ClaudeCodeRouterProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::ClaudeCodeRouter
    }
    
    fn display_name(&self) -> &str {
        "Claude Code Router"
    }
    
    fn create_client(
        &self, 
        config: &ProviderConfig,
        mcp_integration: Arc<super::claude_code::mcp_integration::McpIntegration>
    ) -> Result<Box<dyn LlmClient>, SagittaCodeError> {
        // Create a modified config with ClaudeCode provider type for compatibility
        let mut router_config = config.clone();
        router_config.provider_type = ProviderType::ClaudeCode;
        router_config.set_option("use_claude_code_router", true)?;
        
        // Delegate to the inner provider
        self.inner.create_client(&router_config, mcp_integration)
    }
    
    fn validate_config(&self, config: &ProviderConfig) -> Result<(), SagittaCodeError> {
        // Ensure the config is for ClaudeCodeRouter
        if config.provider_type != ProviderType::ClaudeCodeRouter {
            return Err(SagittaCodeError::ConfigError(
                format!("Invalid provider type for ClaudeCodeRouterProvider: expected ClaudeCodeRouter, got {:?}", config.provider_type)
            ));
        }
        
        // Otherwise delegate validation to the inner provider
        // But first we need to temporarily change the provider type for validation
        let mut temp_config = config.clone();
        temp_config.provider_type = ProviderType::ClaudeCode;
        self.inner.validate_config(&temp_config)
    }
    
    fn default_config(&self) -> ProviderConfig {
        let mut config = self.inner.default_config();
        config.provider_type = ProviderType::ClaudeCodeRouter;
        // Ensure the router flag is set by default
        let _ = config.set_option("use_claude_code_router", true);
        config
    }
    
    fn supported_features(&self) -> Vec<String> {
        self.inner.supported_features()
    }
    
    fn requires_api_key(&self) -> bool {
        self.inner.requires_api_key()
    }
    
    fn is_available(&self) -> bool {
        self.inner.is_available()
    }
    
    fn config_schema(&self) -> serde_json::Value {
        self.inner.config_schema()
    }
}