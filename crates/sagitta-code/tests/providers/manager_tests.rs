/// Provider manager tests
/// 
/// Tests for the provider manager that handles provider switching,
/// state management, and coordination between different providers.

use crate::providers::*;
use std::sync::Arc;
use std::collections::HashMap;

/// Mock provider manager for testing
// Note: Cannot derive Debug due to dyn LlmClient trait
pub struct MockProviderManager {
    factory: MockProviderFactory,
    current_provider: Option<MockProviderType>,
    provider_configs: HashMap<MockProviderType, MockProviderConfig>,
    active_clients: HashMap<MockProviderType, Box<dyn sagitta_code::llm::client::LlmClient>>,
}

impl MockProviderManager {
    pub fn new() -> Self {
        Self {
            factory: MockProviderFactory::new(),
            current_provider: None,
            provider_configs: HashMap::new(),
            active_clients: HashMap::new(),
        }
    }

    pub fn register_provider(&mut self, provider: Box<dyn MockProvider>) {
        let provider_type = provider.provider_type();
        let default_config = provider.default_config();
        
        self.factory.register_provider(provider);
        self.provider_configs.insert(provider_type, default_config);
    }

    pub fn set_current_provider(&mut self, provider_type: MockProviderType) -> Result<(), String> {
        if !self.factory.is_provider_available(provider_type) {
            return Err(format!("Provider {:?} is not available", provider_type));
        }
        
        self.current_provider = Some(provider_type);
        Ok(())
    }

    pub fn get_current_provider(&self) -> Option<MockProviderType> {
        self.current_provider
    }

    pub fn get_current_provider_display_name(&self) -> Option<String> {
        self.current_provider
            .and_then(|pt| self.factory.create_provider(pt))
            .map(|p| p.display_name().to_string())
    }

    pub fn get_available_providers(&self) -> Vec<MockProviderType> {
        self.factory.list_available_providers()
    }

    pub fn set_provider_config(&mut self, provider_type: MockProviderType, config: MockProviderConfig) -> Result<(), String> {
        if !self.factory.is_provider_available(provider_type) {
            return Err(format!("Provider {:?} is not available", provider_type));
        }

        // Validate config with provider
        if let Some(provider) = self.factory.create_provider(provider_type) {
            provider.validate_config(&config).map_err(|e| format!("Config validation failed: {:?}", e))?;
        }

        self.provider_configs.insert(provider_type, config);
        Ok(())
    }

    pub fn get_provider_config(&self, provider_type: MockProviderType) -> Option<&MockProviderConfig> {
        self.provider_configs.get(&provider_type)
    }

    pub fn create_client_for_provider(&self, provider_type: MockProviderType) -> Result<Box<dyn sagitta_code::llm::client::LlmClient>, String> {
        let provider = self.factory.create_provider(provider_type)
            .ok_or_else(|| format!("Provider {:?} not found", provider_type))?;

        let config = self.provider_configs.get(&provider_type)
            .ok_or_else(|| format!("No config found for provider {:?}", provider_type))?;

        let mcp = Some(Arc::new(MockMcpIntegration::new()));
        provider.create_client(config, mcp)
            .map_err(|e| format!("Failed to create client: {:?}", e))
    }

    pub fn get_current_client(&self) -> Result<Box<dyn sagitta_code::llm::client::LlmClient>, String> {
        let current = self.current_provider
            .ok_or_else(|| "No current provider set".to_string())?;
        
        self.create_client_for_provider(current)
    }

    pub fn validate_provider_config(&self, provider_type: MockProviderType, config: &MockProviderConfig) -> Result<(), String> {
        let provider = self.factory.create_provider(provider_type)
            .ok_or_else(|| format!("Provider {:?} not found", provider_type))?;
        
        provider.validate_config(config)
            .map_err(|e| format!("Validation failed: {:?}", e))
    }

    pub fn is_provider_configured(&self, provider_type: MockProviderType) -> bool {
        self.provider_configs.contains_key(&provider_type)
    }

    pub fn reset(&mut self) {
        self.current_provider = None;
        self.active_clients.clear();
    }
}

use super::factory_tests::MockProviderFactory;

#[cfg(test)]
mod manager_tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = MockProviderManager::new();
        assert!(manager.get_current_provider().is_none());
        assert_eq!(manager.get_available_providers().len(), 0);
    }

    #[test]
    fn test_provider_registration_and_availability() {
        let mut manager = MockProviderManager::new();
        
        // Initially no providers
        assert_eq!(manager.get_available_providers().len(), 0);
        
        // Register Claude Code provider
        let claude_provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Claude Code"
        ));
        manager.register_provider(claude_provider);
        
        // Should now be available
        let available = manager.get_available_providers();
        assert_eq!(available.len(), 1);
        assert!(available.contains(&MockProviderType::ClaudeCode));
        
        // Should have default config
        assert!(manager.is_provider_configured(MockProviderType::ClaudeCode));
    }

    #[test]
    fn test_current_provider_management() {
        let mut manager = MockProviderManager::new();
        
        // Register provider
        let provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Claude Code"
        ));
        manager.register_provider(provider);
        
        // Set current provider
        let result = manager.set_current_provider(MockProviderType::ClaudeCode);
        assert!(result.is_ok());
        assert_eq!(manager.get_current_provider(), Some(MockProviderType::ClaudeCode));
        assert_eq!(manager.get_current_provider_display_name(), Some("Claude Code".to_string()));
        
        // Try to set non-existent provider
        let result = manager.set_current_provider(MockProviderType::MistralRs);
        assert!(result.is_err());
        
        // Current provider should remain unchanged
        assert_eq!(manager.get_current_provider(), Some(MockProviderType::ClaudeCode));
    }

    #[test]
    fn test_provider_config_management() {
        let mut manager = MockProviderManager::new();
        
        let provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Claude Code"
        ));
        manager.register_provider(provider);
        
        // Set valid config
        let valid_config = MockProviderConfig::mock_valid();
        let result = manager.set_provider_config(MockProviderType::ClaudeCode, valid_config.clone());
        assert!(result.is_ok());
        
        // Retrieve config
        let retrieved_config = manager.get_provider_config(MockProviderType::ClaudeCode);
        assert!(retrieved_config.is_some());
        
        // Set invalid config
        let invalid_config = MockProviderConfig::mock_invalid();
        let result = manager.set_provider_config(MockProviderType::ClaudeCode, invalid_config);
        assert!(result.is_err());
        
        // Config should remain valid
        let config = manager.get_provider_config(MockProviderType::ClaudeCode).unwrap();
        if let MockProviderConfig::Valid { .. } = config {
            // Good
        } else {
            panic!("Config should still be valid");
        }
    }

    #[tokio::test]
    async fn test_client_creation() {
        let mut manager = MockProviderManager::new();
        
        let responses = vec![MockResponse::text("Manager test response")];
        let provider = Box::new(TestProvider::with_responses(
            MockProviderType::ClaudeCode,
            "Claude Code",
            responses
        ));
        manager.register_provider(provider);
        
        // Set as current provider
        manager.set_current_provider(MockProviderType::ClaudeCode).unwrap();
        
        // Create client
        let client_result = manager.get_current_client();
        assert!(client_result.is_ok());
        
        let client = client_result.unwrap();
        assert_eq!(client.client_type(), "claude-code");
        
        // Test client functionality
        use sagitta_code::llm::client::{Message, MessagePart, Role};
        use uuid::Uuid;
        use std::collections::HashMap;
        
        let messages = vec![Message {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![MessagePart::Text { text: "Manager test".to_string() }],
            metadata: HashMap::new(),
        }];
        
        let response = client.generate(&messages, &[]).await;
        assert!(response.is_ok());
        
        let response = response.unwrap();
        if let Some(MessagePart::Text { text }) = response.message.parts.first() {
            assert_eq!(text, "Manager test response");
        }
    }

    #[test]
    fn test_client_creation_without_current_provider() {
        let manager = MockProviderManager::new();
        
        let result = manager.get_current_client();
        assert!(result.is_err());
        match result {
            Err(error_msg) => assert!(error_msg.contains("No current provider set")),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    }

    #[test]
    fn test_provider_switching() {
        let mut manager = MockProviderManager::new();
        
        // Register multiple providers
        let claude_provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Claude Code"
        ));
        let mistral_provider = Box::new(TestProvider::new(
            MockProviderType::MistralRs,
            "Mistral.rs"
        ));
        
        manager.register_provider(claude_provider);
        manager.register_provider(mistral_provider);
        
        // Start with Claude
        manager.set_current_provider(MockProviderType::ClaudeCode).unwrap();
        assert_eq!(manager.get_current_provider(), Some(MockProviderType::ClaudeCode));
        assert_eq!(manager.get_current_provider_display_name(), Some("Claude Code".to_string()));
        
        // Switch to Mistral
        manager.set_current_provider(MockProviderType::MistralRs).unwrap();
        assert_eq!(manager.get_current_provider(), Some(MockProviderType::MistralRs));
        assert_eq!(manager.get_current_provider_display_name(), Some("Mistral.rs".to_string()));
        
        // Switch back to Claude
        manager.set_current_provider(MockProviderType::ClaudeCode).unwrap();
        assert_eq!(manager.get_current_provider(), Some(MockProviderType::ClaudeCode));
    }

    #[test]
    fn test_config_validation() {
        let mut manager = MockProviderManager::new();
        
        let provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Claude Code"
        ));
        manager.register_provider(provider);
        
        // Valid config
        let valid_config = MockProviderConfig::mock_valid();
        let result = manager.validate_provider_config(MockProviderType::ClaudeCode, &valid_config);
        assert!(result.is_ok());
        
        // Invalid config
        let invalid_config = MockProviderConfig::mock_invalid();
        let result = manager.validate_provider_config(MockProviderType::ClaudeCode, &invalid_config);
        assert!(result.is_err());
        
        // Non-existent provider
        let result = manager.validate_provider_config(MockProviderType::MistralRs, &valid_config);
        assert!(result.is_err());
        let error_msg = format!("{}", result.unwrap_err());
        assert!(error_msg.contains("not found"));
    }

    #[test]
    fn test_manager_reset() {
        let mut manager = MockProviderManager::new();
        
        let provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Claude Code"
        ));
        manager.register_provider(provider);
        manager.set_current_provider(MockProviderType::ClaudeCode).unwrap();
        
        // Verify state before reset
        assert!(manager.get_current_provider().is_some());
        
        // Reset
        manager.reset();
        
        // Verify state after reset
        assert!(manager.get_current_provider().is_none());
        
        // Provider should still be registered
        assert!(manager.get_available_providers().contains(&MockProviderType::ClaudeCode));
    }

    #[test]
    fn test_error_handling_for_invalid_operations() {
        let mut manager = MockProviderManager::new();
        
        // Try to set config for non-existent provider
        let config = MockProviderConfig::mock_valid();
        let result = manager.set_provider_config(MockProviderType::ClaudeCode, config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not available"));
        
        // Try to create client for non-existent provider
        let result = manager.create_client_for_provider(MockProviderType::ClaudeCode);
        assert!(result.is_err());
        match result {
            Err(error_msg) => assert!(error_msg.contains("not found")),
            Ok(_) => panic!("Expected error but got Ok"),
        }
        
        // Try to get config for non-existent provider
        let config = manager.get_provider_config(MockProviderType::ClaudeCode);
        assert!(config.is_none());
    }

    #[test]
    fn test_multiple_providers_with_different_configs() {
        let mut manager = MockProviderManager::new();
        
        // Register providers
        let claude_provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Claude Code"
        ));
        let mistral_provider = Box::new(TestProvider::new(
            MockProviderType::MistralRs,
            "Mistral.rs"
        ));
        
        manager.register_provider(claude_provider);
        manager.register_provider(mistral_provider);
        
        // Set different configs
        let claude_config = MockProviderConfig::Valid { api_key: Some("claude-key".to_string()) };
        let mistral_config = MockProviderConfig::Valid { api_key: Some("mistral-key".to_string()) };
        
        manager.set_provider_config(MockProviderType::ClaudeCode, claude_config).unwrap();
        manager.set_provider_config(MockProviderType::MistralRs, mistral_config).unwrap();
        
        // Verify configs are independent
        let claude_retrieved = manager.get_provider_config(MockProviderType::ClaudeCode).unwrap();
        let mistral_retrieved = manager.get_provider_config(MockProviderType::MistralRs).unwrap();
        
        if let (MockProviderConfig::Valid { api_key: claude_key }, MockProviderConfig::Valid { api_key: mistral_key }) = (claude_retrieved, mistral_retrieved) {
            assert_eq!(claude_key, &Some("claude-key".to_string()));
            assert_eq!(mistral_key, &Some("mistral-key".to_string()));
        } else {
            panic!("Expected valid configs for both providers");
        }
    }
}