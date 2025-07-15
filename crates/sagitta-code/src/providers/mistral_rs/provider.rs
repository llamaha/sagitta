use std::sync::Arc;
use async_trait::async_trait;

use crate::providers::{Provider, ProviderType, ProviderConfig};
use crate::llm::client::LlmClient;
use crate::utils::errors::SagittaCodeError;
use crate::providers::claude_code::mcp_integration::McpIntegration;
use super::{MistralRsClient, MistralRsConfig};

pub struct MistralRsProvider;

impl MistralRsProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MistralRsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for MistralRsProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::MistralRs
    }

    fn display_name(&self) -> &str {
        "Mistral.rs"
    }



    fn supported_features(&self) -> Vec<String> {
        vec![
            "streaming".to_string(),
            "tools".to_string(),
            "mcp_tools".to_string(),
            "temperature_control".to_string(),
            "max_tokens".to_string(),
            "top_k".to_string(),
            "min_p".to_string(),
        ]
    }

    fn requires_api_key(&self) -> bool {
        false // Mistral.rs typically runs locally without requiring an API key
    }

    fn is_available(&self) -> bool {
        true // Always available if the server is running
    }

    fn validate_config(&self, config: &ProviderConfig) -> Result<(), SagittaCodeError> {
        let mistral_config: MistralRsConfig = config.try_into()
            .map_err(|e| SagittaCodeError::ConfigError(format!("Invalid Mistral.rs config: {}", e)))?;
        
        mistral_config.validate()
            .map_err(|e| SagittaCodeError::ConfigError(e))
    }

    fn create_client(
        &self,
        config: &ProviderConfig,
        mcp_integration: Arc<McpIntegration>,
    ) -> Result<Box<dyn LlmClient>, SagittaCodeError> {
        let mistral_config: MistralRsConfig = config.try_into()
            .map_err(|e| SagittaCodeError::ConfigError(format!("Invalid Mistral.rs config: {}", e)))?;

        let client = MistralRsClient::new(mistral_config, mcp_integration.clone())?;
        
        // Configure MCP integration for Mistral.rs 
        // Note: This logs MCP availability - in production this would configure Mistral.rs
        let mcp_integration_clone = mcp_integration.clone();
        tokio::spawn(async move {
            if let Err(e) = MistralRsClient::configure_mcp_for_mistral_rs(mcp_integration_clone).await {
                log::error!("Failed to configure MCP for Mistral.rs: {}", e);
            }
        });
        
        Ok(Box::new(client))
    }

    fn default_config(&self) -> ProviderConfig {
        let mistral_config = MistralRsConfig::default();
        let mut config = ProviderConfig::new(ProviderType::MistralRs);
        
        config.set_option("base_url", &mistral_config.base_url).unwrap();
        config.set_option("model", &mistral_config.model).unwrap();
        config.set_option("stream", &mistral_config.stream).unwrap();
        config.set_option("max_tokens", &mistral_config.max_tokens).unwrap();
        config.set_option("temperature", &mistral_config.temperature).unwrap();
        config.set_option("top_k", &mistral_config.top_k).unwrap();
        config.set_option("min_p", &mistral_config.min_p).unwrap();
        config.set_option("api_key", &mistral_config.api_key).unwrap();
        
        config
    }
    
    fn config_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "base_url": {
                    "type": "string",
                    "description": "Base URL for the Mistral.rs API",
                    "default": "http://localhost:1234"
                },
                "model": {
                    "type": "string", 
                    "description": "Model name to use",
                    "default": "mistral-7b-instruct"
                },
                "stream": {
                    "type": "boolean",
                    "description": "Whether to use streaming responses",
                    "default": true
                },
                "max_tokens": {
                    "type": "integer",
                    "description": "Maximum number of tokens to generate",
                    "default": null
                },
                "temperature": {
                    "type": "number",
                    "description": "Sampling temperature",
                    "default": null
                },
                "top_k": {
                    "type": "integer",
                    "description": "Top-k sampling parameter",
                    "default": null
                },
                "min_p": {
                    "type": "number",
                    "description": "Min-p sampling parameter",
                    "default": null
                },
                "api_key": {
                    "type": "string",
                    "description": "Optional API key",
                    "default": null
                }
            },
            "required": ["base_url", "model"]
        })
    }


}

// Convert from ProviderConfig to MistralRsConfig
impl TryFrom<&ProviderConfig> for MistralRsConfig {
    type Error = SagittaCodeError;

    fn try_from(config: &ProviderConfig) -> Result<Self, Self::Error> {
        let base_url: String = config.get_option("base_url")
            .map_err(|e| SagittaCodeError::ConfigError(format!("Failed to parse base_url: {}", e)))?
            .ok_or_else(|| SagittaCodeError::ConfigError("base_url is required".to_string()))?;
        
        let model: String = config.get_option("model")
            .map_err(|e| SagittaCodeError::ConfigError(format!("Failed to parse model: {}", e)))?
            .ok_or_else(|| SagittaCodeError::ConfigError("model is required".to_string()))?;

        Ok(MistralRsConfig {
            base_url,
            api_key: config.get_option("api_key")
                .map_err(|e| SagittaCodeError::ConfigError(format!("Failed to parse api_key: {}", e)))?.flatten(),
            model,
            max_tokens: config.get_option("max_tokens")
                .map_err(|e| SagittaCodeError::ConfigError(format!("Failed to parse max_tokens: {}", e)))?.flatten(),
            temperature: config.get_option("temperature")
                .map_err(|e| SagittaCodeError::ConfigError(format!("Failed to parse temperature: {}", e)))?.flatten(),
            stream: config.get_option("stream")
                .map_err(|e| SagittaCodeError::ConfigError(format!("Failed to parse stream: {}", e)))?
                .unwrap_or(true),
            top_k: config.get_option("top_k")
                .map_err(|e| SagittaCodeError::ConfigError(format!("Failed to parse top_k: {}", e)))?.flatten(),
            min_p: config.get_option("min_p")
                .map_err(|e| SagittaCodeError::ConfigError(format!("Failed to parse min_p: {}", e)))?.flatten(),
        })
    }
}

// Convert from ProviderConfig (owned) to MistralRsConfig
impl TryFrom<ProviderConfig> for MistralRsConfig {
    type Error = SagittaCodeError;

    fn try_from(config: ProviderConfig) -> Result<Self, Self::Error> {
        (&config).try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderType;

    #[test]
    fn test_provider_properties() {
        let provider = MistralRsProvider::new();
        
        assert_eq!(provider.provider_type(), ProviderType::MistralRs);
        assert_eq!(provider.display_name(), "Mistral.rs");
        assert!(!provider.requires_api_key());
        assert!(provider.is_available());
        
        let features = provider.supported_features();
        assert!(features.contains(&"streaming".to_string()));
        assert!(features.contains(&"tools".to_string()));
        assert!(features.contains(&"temperature_control".to_string()));
    }

    #[test]
    fn test_default_config() {
        let provider = MistralRsProvider::new();
        let config = provider.default_config();
        
        assert_eq!(config.provider_type, ProviderType::MistralRs);
        let base_url: Option<String> = config.get_option("base_url").unwrap_or(None);
        assert_eq!(base_url, Some("http://localhost:1234".to_string()));
        let model: Option<String> = config.get_option("model").unwrap_or(None);
        assert_eq!(model, Some("default".to_string()));
        let stream: Option<bool> = config.get_option("stream").unwrap_or(None);
        assert_eq!(stream, Some(true));
    }

    #[test]
    fn test_config_conversion() {
        let mut config = ProviderConfig::new(ProviderType::MistralRs);
        config.set_option("base_url", "http://localhost:8080").unwrap();
        config.set_option("model", "test-model").unwrap();
        config.set_option("temperature", 0.7f32).unwrap();
        config.set_option("max_tokens", 1000u32).unwrap();

        let mistral_config: Result<MistralRsConfig, _> = (&config).try_into();
        assert!(mistral_config.is_ok());
        
        let mistral_config = mistral_config.unwrap();
        assert_eq!(mistral_config.base_url, "http://localhost:8080");
        assert_eq!(mistral_config.model, "test-model");
        assert_eq!(mistral_config.temperature, Some(0.7));
        assert_eq!(mistral_config.max_tokens, Some(1000));
    }

    #[tokio::test]
    async fn test_config_validation() {
        let provider = MistralRsProvider::new();
        
        // Valid config
        let valid_config = provider.default_config();
        assert!(provider.validate_config(&valid_config).is_ok());
        
        // Invalid config (missing base_url)
        let mut invalid_config = ProviderConfig::new(ProviderType::MistralRs);
        invalid_config.set_option("model", "test").unwrap();
        assert!(provider.validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_config_schema() {
        let provider = MistralRsProvider::new();
        let schema = provider.config_schema();
        
        assert!(schema.is_object());
        let properties = schema.get("properties").unwrap();
        assert!(properties.get("base_url").is_some());
        assert!(properties.get("model").is_some());
        assert!(properties.get("temperature").is_some());
        assert!(properties.get("max_tokens").is_some());
    }
}