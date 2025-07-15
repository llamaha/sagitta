//! Provider type definitions and configuration structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Enumeration of all supported provider types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProviderType {
    /// Claude Code provider (original implementation)
    ClaudeCode,
    /// Mistral.rs provider (OpenAI-compatible API)
    MistralRs,
    // Future providers (commented out for now):
    // Gemini,
    // LlamaCpp,
    // OpenRouter,
}

impl ProviderType {
    /// Returns the human-readable name for this provider type
    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderType::ClaudeCode => "Claude Code",
            ProviderType::MistralRs => "Mistral.rs",
        }
    }
    
    /// Returns a unique identifier string for this provider type
    pub fn id(&self) -> &'static str {
        match self {
            ProviderType::ClaudeCode => "claude-code",
            ProviderType::MistralRs => "mistral-rs",
        }
    }
    
    /// Returns all available provider types
    pub fn all() -> Vec<ProviderType> {
        vec![
            ProviderType::ClaudeCode,
            ProviderType::MistralRs,
        ]
    }
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl Default for ProviderType {
    fn default() -> Self {
        ProviderType::ClaudeCode
    }
}

/// Configuration for a specific provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// The type of provider this configuration is for
    pub provider_type: ProviderType,
    /// Whether this provider is enabled
    pub enabled: bool,
    /// Provider-specific configuration options
    pub options: HashMap<String, serde_json::Value>,
}

impl ProviderConfig {
    /// Creates a new provider configuration
    pub fn new(provider_type: ProviderType) -> Self {
        Self {
            provider_type,
            enabled: true,
            options: HashMap::new(),
        }
    }
    
    /// Creates a default configuration for the specified provider type
    pub fn default_for_provider(provider_type: ProviderType) -> Self {
        match provider_type {
            ProviderType::ClaudeCode => ClaudeCodeConfig::default().into(),
            ProviderType::MistralRs => MistralRsConfig::default().into(),
        }
    }
    
    /// Sets a configuration option
    pub fn set_option<T: Serialize>(&mut self, key: &str, value: T) -> Result<(), serde_json::Error> {
        self.options.insert(key.to_string(), serde_json::to_value(value)?);
        Ok(())
    }
    
    /// Gets a configuration option
    pub fn get_option<T>(&self, key: &str) -> Result<Option<T>, serde_json::Error> 
    where
        T: for<'de> Deserialize<'de>,
    {
        match self.options.get(key) {
            Some(value) => Ok(Some(serde_json::from_value(value.clone())?)),
            None => Ok(None),
        }
    }
    
    /// Gets a required configuration option, returning an error if missing
    pub fn get_required_option<T>(&self, key: &str) -> Result<T, ConfigError> 
    where
        T: for<'de> Deserialize<'de>,
    {
        self.get_option(key)
            .map_err(|e| ConfigError::InvalidValue(format!("Failed to parse option '{}': {}", key, e)))?
            .ok_or_else(|| ConfigError::MissingValue(format!("Required option '{}' is missing", key)))
    }
}

/// Configuration errors specific to providers
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing required configuration value: {0}")]
    MissingValue(String),
    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),
    #[error("Provider is disabled: {0}")]
    ProviderDisabled(String),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Claude Code specific configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCodeConfig {
    /// Path to the Claude CLI binary (optional, uses system PATH if not specified)
    pub binary_path: Option<String>,
    /// Additional arguments to pass to Claude CLI
    pub additional_args: Vec<String>,
    /// Timeout for Claude CLI operations in seconds
    pub timeout_seconds: u64,
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            binary_path: None,
            additional_args: vec![],
            timeout_seconds: 300, // 5 minutes
        }
    }
}

/// Mistral.rs specific configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MistralRsConfig {
    /// Base URL for the Mistral.rs server
    pub base_url: String,
    /// Optional API token for authentication
    pub api_token: Option<String>,
    /// Timeout for HTTP requests in seconds
    pub timeout_seconds: u64,
    /// Model name to use (optional, server default if not specified)
    pub model: Option<String>,
}

impl Default for MistralRsConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:1234".to_string(),
            api_token: None,
            timeout_seconds: 120, // 2 minutes
            model: None,
        }
    }
}

/// Converts a ClaudeCodeConfig to a generic ProviderConfig
impl From<ClaudeCodeConfig> for ProviderConfig {
    fn from(config: ClaudeCodeConfig) -> Self {
        let mut provider_config = ProviderConfig::new(ProviderType::ClaudeCode);
        // Only set binary_path if it's Some
        if let Some(ref binary_path) = config.binary_path {
            provider_config.set_option("binary_path", binary_path).unwrap();
        }
        provider_config.set_option("additional_args", &config.additional_args).unwrap();
        provider_config.set_option("timeout_seconds", config.timeout_seconds).unwrap();
        provider_config
    }
}

/// Converts a MistralRsConfig to a generic ProviderConfig
impl From<MistralRsConfig> for ProviderConfig {
    fn from(config: MistralRsConfig) -> Self {
        let mut provider_config = ProviderConfig::new(ProviderType::MistralRs);
        provider_config.set_option("base_url", &config.base_url).unwrap();
        // Only set api_token if it's Some
        if let Some(ref api_token) = config.api_token {
            provider_config.set_option("api_token", api_token).unwrap();
        }
        provider_config.set_option("timeout_seconds", config.timeout_seconds).unwrap();
        // Only set model if it's Some
        if let Some(ref model) = config.model {
            provider_config.set_option("model", model).unwrap();
        }
        provider_config
    }
}

/// Converts a generic ProviderConfig to a ClaudeCodeConfig
impl TryFrom<&ProviderConfig> for ClaudeCodeConfig {
    type Error = ConfigError;
    
    fn try_from(config: &ProviderConfig) -> Result<Self, Self::Error> {
        if config.provider_type != ProviderType::ClaudeCode {
            return Err(ConfigError::InvalidValue(
                format!("Expected ClaudeCode provider, got {:?}", config.provider_type)
            ));
        }
        
        Ok(ClaudeCodeConfig {
            binary_path: config.get_option("binary_path")?,
            additional_args: config.get_option("additional_args")?.unwrap_or_default(),
            timeout_seconds: config.get_option("timeout_seconds")?.unwrap_or(300),
        })
    }
}

/// Converts an owned ProviderConfig to a ClaudeCodeConfig
impl TryFrom<ProviderConfig> for ClaudeCodeConfig {
    type Error = ConfigError;
    
    fn try_from(config: ProviderConfig) -> Result<Self, Self::Error> {
        ClaudeCodeConfig::try_from(&config)
    }
}

/// Converts a generic ProviderConfig to a MistralRsConfig
impl TryFrom<&ProviderConfig> for MistralRsConfig {
    type Error = ConfigError;
    
    fn try_from(config: &ProviderConfig) -> Result<Self, Self::Error> {
        if config.provider_type != ProviderType::MistralRs {
            return Err(ConfigError::InvalidValue(
                format!("Expected MistralRs provider, got {:?}", config.provider_type)
            ));
        }
        
        Ok(MistralRsConfig {
            base_url: config.get_required_option("base_url")?,
            api_token: config.get_option("api_token")?,
            timeout_seconds: config.get_option("timeout_seconds")?.unwrap_or(120),
            model: config.get_option("model")?,
        })
    }
}

/// Converts an owned ProviderConfig to a MistralRsConfig
impl TryFrom<ProviderConfig> for MistralRsConfig {
    type Error = ConfigError;
    
    fn try_from(config: ProviderConfig) -> Result<Self, Self::Error> {
        MistralRsConfig::try_from(&config)
    }
}

/// Converts legacy ClaudeCodeConfig from config module to ProviderConfig
impl TryFrom<crate::config::types::ClaudeCodeConfig> for ProviderConfig {
    type Error = ConfigError;
    
    fn try_from(legacy_config: crate::config::types::ClaudeCodeConfig) -> Result<Self, Self::Error> {
        let mut provider_config = ProviderConfig::new(ProviderType::ClaudeCode);
        
        // Map legacy fields to provider config options
        provider_config.set_option("binary_path", &legacy_config.claude_path)?;
        provider_config.set_option("model", &legacy_config.model)?;
        provider_config.set_option("fallback_model", &legacy_config.fallback_model)?;
        provider_config.set_option("max_output_tokens", legacy_config.max_output_tokens)?;
        provider_config.set_option("debug", legacy_config.debug)?;
        provider_config.set_option("verbose", legacy_config.verbose)?;
        provider_config.set_option("timeout", legacy_config.timeout)?;
        provider_config.set_option("max_turns", legacy_config.max_turns)?;
        provider_config.set_option("output_format", &legacy_config.output_format)?;
        provider_config.set_option("input_format", &legacy_config.input_format)?;
        provider_config.set_option("dangerously_skip_permissions", legacy_config.dangerously_skip_permissions)?;
        provider_config.set_option("allowed_tools", &legacy_config.allowed_tools)?;
        provider_config.set_option("disallowed_tools", &legacy_config.disallowed_tools)?;
        provider_config.set_option("additional_directories", &legacy_config.additional_directories)?;
        provider_config.set_option("mcp_config", &legacy_config.mcp_config)?;
        provider_config.set_option("auto_ide", legacy_config.auto_ide)?;
        
        Ok(provider_config)
    }
}