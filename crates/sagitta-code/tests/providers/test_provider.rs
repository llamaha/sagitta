/// Test provider implementation for testing the provider framework
/// 
/// This is a concrete implementation of the MockProvider trait used in tests
/// to verify the provider abstraction works correctly.

use std::sync::Arc;
use sagitta_code::llm::client::{LlmClient, Message, ToolDefinition};
use sagitta_code::utils::errors::SagittaCodeError;
use crate::providers::*;

/// Concrete test provider implementation
#[derive(Debug)]
pub struct TestProvider {
    provider_type: MockProviderType,
    display_name: String,
}

impl TestProvider {
    pub fn new(provider_type: MockProviderType, display_name: impl Into<String>) -> Self {
        Self {
            provider_type,
            display_name: display_name.into(),
        }
    }
}

impl MockProvider for TestProvider {
    fn provider_type(&self) -> MockProviderType {
        self.provider_type
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn validate_config(&self, config: &MockProviderConfig) -> Result<(), SagittaCodeError> {
        match config {
            MockProviderConfig::Valid { .. } => Ok(()),
            MockProviderConfig::Invalid { error } => {
                Err(SagittaCodeError::ConfigError(error.clone()))
            }
        }
    }

    fn create_client(
        &self,
        config: &MockProviderConfig,
        mcp_integration: Option<Arc<MockMcpIntegration>>,
    ) -> Result<Box<dyn LlmClient>, SagittaCodeError> {
        // Validate config first
        self.validate_config(config)?;

        match config {
            MockProviderConfig::Valid { .. } => {
                // Create a mock client with some default responses
                let responses = vec![
                    MockResponse::text("Test response from provider"),
                    MockResponse::with_tools("Tool response", vec![]),
                ];
                
                let client = MockLlmClient::new(responses, self.provider_type);
                Ok(Box::new(client))
            }
            MockProviderConfig::Invalid { error } => {
                Err(SagittaCodeError::LlmError(format!("Cannot create client with invalid config: {}", error)))
            }
        }
    }

    fn default_config(&self) -> MockProviderConfig {
        MockProviderConfig::Valid {
            api_key: Some("test-api-key".to_string()),
        }
    }

    fn supported_features(&self) -> Vec<String> {
        vec![
            "text_generation".to_string(),
            "tool_calling".to_string(),
            "streaming".to_string(),
        ]
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    fn is_available(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = TestProvider::new(MockProviderType::ClaudeCode, "Test Claude");
        assert_eq!(provider.provider_type(), MockProviderType::ClaudeCode);
        assert_eq!(provider.display_name(), "Test Claude");
    }

    #[test]
    fn test_provider_config_validation() {
        let provider = TestProvider::new(MockProviderType::ClaudeCode, "Test");
        
        // Valid config should pass
        let valid_config = MockProviderConfig::mock_valid();
        assert!(provider.validate_config(&valid_config).is_ok());
        
        // Invalid config should fail
        let invalid_config = MockProviderConfig::mock_invalid();
        assert!(provider.validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_client_creation() {
        let provider = TestProvider::new(MockProviderType::ClaudeCode, "Test");
        let config = MockProviderConfig::mock_valid();
        let mcp = Some(Arc::new(MockMcpIntegration::new()));
        
        let client = provider.create_client(&config, mcp);
        assert!(client.is_ok());
        
        let client = client.unwrap();
        assert_eq!(client.client_type(), "mock");
    }

    #[test]
    fn test_default_config() {
        let provider = TestProvider::new(MockProviderType::ClaudeCode, "Test");
        let default_config = provider.default_config();
        
        // Default config should be valid
        assert!(provider.validate_config(&default_config).is_ok());
    }

    #[test]
    fn test_provider_features() {
        let provider = TestProvider::new(MockProviderType::ClaudeCode, "Test");
        let features = provider.supported_features();
        
        assert!(features.contains(&"text_generation".to_string()));
        assert!(features.contains(&"tool_calling".to_string()));
        assert!(features.contains(&"streaming".to_string()));
    }

    #[test]
    fn test_provider_requirements() {
        let provider = TestProvider::new(MockProviderType::ClaudeCode, "Test");
        
        assert!(provider.requires_api_key());
        assert!(provider.is_available());
    }
}