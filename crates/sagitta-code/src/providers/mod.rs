//! Provider abstraction layer for different LLM backends
//! 
//! This module provides a clean abstraction over different LLM providers,
//! allowing Sagitta Code to support multiple backends like Claude Code,
//! Mistral.rs, and future providers.

pub mod types;
pub mod manager;
pub mod factory;
pub mod claude_code;
pub mod mistral_rs;

// Re-export commonly used types
pub use types::{ProviderType, ProviderConfig};
pub use manager::{ProviderManager, ProviderState};
pub use factory::ProviderFactory;
pub use claude_code::ClaudeCodeProvider;
pub use mistral_rs::MistralRsProvider;

use crate::llm::client::LlmClient;
use crate::utils::errors::SagittaCodeError;

/// Core trait that all providers must implement
pub trait Provider: Send + Sync {
    /// Returns the type of this provider
    fn provider_type(&self) -> ProviderType;
    
    /// Returns the human-readable display name for this provider
    fn display_name(&self) -> &str;
    
    /// Creates a new LLM client instance for this provider
    fn create_client(
        &self, 
        config: &ProviderConfig,
        mcp_integration: std::sync::Arc<crate::providers::claude_code::mcp_integration::McpIntegration>
    ) -> Result<Box<dyn LlmClient>, SagittaCodeError>;
    
    /// Validates that the provided configuration is valid for this provider
    fn validate_config(&self, config: &ProviderConfig) -> Result<(), SagittaCodeError>;
    
    /// Returns the default configuration for this provider
    fn default_config(&self) -> ProviderConfig;
    
    /// Returns the features supported by this provider
    fn supported_features(&self) -> Vec<String>;
    
    /// Returns whether this provider requires an API key
    fn requires_api_key(&self) -> bool;
    
    /// Returns whether this provider is currently available/accessible
    fn is_available(&self) -> bool;
    
    /// Returns the JSON schema for this provider's configuration
    fn config_schema(&self) -> serde_json::Value;
}