use crate::llm::client::LlmClient;
use crate::providers::{Provider, ProviderConfig, ProviderType};
use crate::utils::errors::SagittaCodeError;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct OpenAICompatibleProvider {
    name: String,
}

impl OpenAICompatibleProvider {
    pub fn new() -> Self {
        Self {
            name: "OpenAI Compatible".to_string(),
        }
    }
}

impl Default for OpenAICompatibleProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for OpenAICompatibleProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::OpenAICompatible
    }
    
    fn display_name(&self) -> &str {
        &self.name
    }
    
    fn create_client(
        &self,
        config: &ProviderConfig,
        mcp_integration: Arc<crate::providers::claude_code::mcp_integration::McpIntegration>,
    ) -> Result<Box<dyn LlmClient>, SagittaCodeError> {
        // Extract configuration from ProviderConfig
        let base_url = config.get_required_option::<String>("base_url")
            .map_err(|e| SagittaCodeError::ConfigError(e.to_string()))?;
        
        let api_key = config.get_option::<String>("api_key")
            .map_err(|e| SagittaCodeError::ConfigError(e.to_string()))?;
        
        let model = config.get_option::<String>("model")
            .map_err(|e| SagittaCodeError::ConfigError(e.to_string()))?;
        
        let timeout_seconds = config.get_option::<u64>("timeout_seconds")
            .map_err(|e| SagittaCodeError::ConfigError(e.to_string()))?
            .unwrap_or(120);
        
        let max_retries = config.get_option::<u32>("max_retries")
            .map_err(|e| SagittaCodeError::ConfigError(e.to_string()))?
            .unwrap_or(3);
        
        let client = super::client::OpenAICompatibleClient::new(
            base_url,
            api_key,
            model,
            mcp_integration,
            timeout_seconds,
            max_retries,
        );
        
        Ok(Box::new(client))
    }
    
    fn validate_config(&self, config: &ProviderConfig) -> Result<(), SagittaCodeError> {
        // Validate that base_url is present
        let _base_url = config.get_required_option::<String>("base_url")
            .map_err(|e| SagittaCodeError::ConfigError(e.to_string()))?;
        
        // Validate timeout_seconds if present
        if let Some(timeout) = config.get_option::<u64>("timeout_seconds")
            .map_err(|e| SagittaCodeError::ConfigError(e.to_string()))? {
            if timeout == 0 {
                return Err(SagittaCodeError::ConfigError("timeout_seconds must be greater than 0".to_string()));
            }
        }
        
        // Validate max_retries if present
        if let Some(retries) = config.get_option::<u32>("max_retries")
            .map_err(|e| SagittaCodeError::ConfigError(e.to_string()))? {
            if retries > 10 {
                return Err(SagittaCodeError::ConfigError("max_retries should not exceed 10".to_string()));
            }
        }
        
        Ok(())
    }
    
    fn default_config(&self) -> ProviderConfig {
        let mut config = ProviderConfig::new(ProviderType::OpenAICompatible);
        config.set_option("base_url", "http://localhost:1234/v1").ok();
        config.set_option("timeout_seconds", 120u64).ok();
        config.set_option("max_retries", 3u32).ok();
        config
    }
    
    fn supported_features(&self) -> Vec<String> {
        vec![
            "streaming".to_string(),
            "tools".to_string(),
            "mcp_tools".to_string(),
            "temperature_control".to_string(),
            "max_tokens".to_string(),
            "top_p".to_string(),
            "frequency_penalty".to_string(),
            "presence_penalty".to_string(),
            "stop_sequences".to_string(),
        ]
    }
    
    fn requires_api_key(&self) -> bool {
        false // API key is optional
    }
    
    fn is_available(&self) -> bool {
        true // Always available as it's just an HTTP client
    }
    
    fn config_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "base_url": {
                    "type": "string",
                    "description": "Base URL for the OpenAI-compatible API",
                    "examples": ["http://localhost:1234/v1", "https://api.openrouter.ai/api/v1"]
                },
                "api_key": {
                    "type": "string",
                    "description": "Optional API key for authentication"
                },
                "model": {
                    "type": "string",
                    "description": "Optional model to use (uses server default if not specified)",
                    "examples": ["mistral-7b-instruct", "codellama-34b-instruct"]
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Request timeout in seconds",
                    "default": 120,
                    "minimum": 1
                },
                "max_retries": {
                    "type": "integer",
                    "description": "Maximum number of retries on failure",
                    "default": 3,
                    "minimum": 0,
                    "maximum": 10
                }
            },
            "required": ["base_url"],
            "additionalProperties": false
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_provider_creation() {
        let provider = OpenAICompatibleProvider::new();
        assert_eq!(provider.display_name(), "OpenAI Compatible");
        assert_eq!(provider.provider_type(), ProviderType::OpenAICompatible);
    }
    
    #[test]
    fn test_default_config() {
        let provider = OpenAICompatibleProvider::new();
        let config = provider.default_config();
        
        assert_eq!(config.provider_type, ProviderType::OpenAICompatible);
        assert_eq!(config.get_option::<String>("base_url").unwrap(), Some("http://localhost:1234/v1".to_string()));
        assert_eq!(config.get_option::<u64>("timeout_seconds").unwrap(), Some(120));
        assert_eq!(config.get_option::<u32>("max_retries").unwrap(), Some(3));
    }
    
    #[test]
    fn test_validate_config_valid() {
        let provider = OpenAICompatibleProvider::new();
        let mut config = ProviderConfig::new(ProviderType::OpenAICompatible);
        config.set_option("base_url", "http://localhost:1234/v1").unwrap();
        
        assert!(provider.validate_config(&config).is_ok());
    }
    
    #[test]
    fn test_validate_config_missing_base_url() {
        let provider = OpenAICompatibleProvider::new();
        let config = ProviderConfig::new(ProviderType::OpenAICompatible);
        
        assert!(provider.validate_config(&config).is_err());
    }
    
    #[test]
    fn test_validate_config_invalid_timeout() {
        let provider = OpenAICompatibleProvider::new();
        let mut config = ProviderConfig::new(ProviderType::OpenAICompatible);
        config.set_option("base_url", "http://localhost:1234/v1").unwrap();
        config.set_option("timeout_seconds", 0u64).unwrap();
        
        assert!(provider.validate_config(&config).is_err());
    }
}