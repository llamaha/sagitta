/// Provider factory tests
/// 
/// Tests for the provider factory pattern that will be responsible
/// for creating and managing provider instances.

use crate::providers::*;
use std::sync::Arc;

/// Mock provider factory for testing
#[derive(Default)]
pub struct MockProviderFactory {
    registered_providers: std::collections::HashMap<MockProviderType, Box<dyn MockProvider>>,
}

impl MockProviderFactory {
    pub fn new() -> Self {
        Self {
            registered_providers: std::collections::HashMap::new(),
        }
    }

    pub fn register_provider(&mut self, provider: Box<dyn MockProvider>) {
        let provider_type = provider.provider_type();
        self.registered_providers.insert(provider_type, provider);
    }

    pub fn create_provider(&self, provider_type: MockProviderType) -> Option<&dyn MockProvider> {
        self.registered_providers.get(&provider_type).map(|p| p.as_ref())
    }

    pub fn list_available_providers(&self) -> Vec<MockProviderType> {
        self.registered_providers.keys().cloned().collect()
    }

    pub fn is_provider_available(&self, provider_type: MockProviderType) -> bool {
        self.registered_providers.contains_key(&provider_type)
    }
}

#[cfg(test)]
mod factory_tests {
    use super::*;

    #[test]
    fn test_factory_creation() {
        let factory = MockProviderFactory::new();
        assert_eq!(factory.list_available_providers().len(), 0);
    }

    #[test]
    fn test_provider_registration() {
        let mut factory = MockProviderFactory::new();
        
        // Register Claude Code provider
        let claude_provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Claude Code"
        ));
        factory.register_provider(claude_provider);
        
        // Register Mistral.rs provider
        let mistral_provider = Box::new(TestProvider::new(
            MockProviderType::MistralRs,
            "Mistral.rs"
        ));
        factory.register_provider(mistral_provider);
        
        // Verify registration
        let available_providers = factory.list_available_providers();
        assert_eq!(available_providers.len(), 2);
        assert!(available_providers.contains(&MockProviderType::ClaudeCode));
        assert!(available_providers.contains(&MockProviderType::MistralRs));
    }

    #[test]
    fn test_provider_retrieval() {
        let mut factory = MockProviderFactory::new();
        
        let provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Claude Code"
        ));
        factory.register_provider(provider);
        
        // Test successful retrieval
        let retrieved_provider = factory.create_provider(MockProviderType::ClaudeCode);
        assert!(retrieved_provider.is_some());
        
        let provider = retrieved_provider.unwrap();
        assert_eq!(provider.provider_type(), MockProviderType::ClaudeCode);
        assert_eq!(provider.display_name(), "Claude Code");
        
        // Test retrieval of non-existent provider
        let non_existent = factory.create_provider(MockProviderType::MistralRs);
        assert!(non_existent.is_none());
    }

    #[test]
    fn test_provider_availability_check() {
        let mut factory = MockProviderFactory::new();
        
        // Initially no providers available
        assert!(!factory.is_provider_available(MockProviderType::ClaudeCode));
        assert!(!factory.is_provider_available(MockProviderType::MistralRs));
        
        // Register Claude Code provider
        let provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Claude Code"
        ));
        factory.register_provider(provider);
        
        // Now Claude Code should be available
        assert!(factory.is_provider_available(MockProviderType::ClaudeCode));
        assert!(!factory.is_provider_available(MockProviderType::MistralRs));
    }

    #[test]
    fn test_provider_override() {
        let mut factory = MockProviderFactory::new();
        
        // Register initial provider
        let initial_provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Initial Claude"
        ));
        factory.register_provider(initial_provider);
        
        // Verify initial registration
        let provider = factory.create_provider(MockProviderType::ClaudeCode).unwrap();
        assert_eq!(provider.display_name(), "Initial Claude");
        
        // Override with new provider
        let override_provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Override Claude"
        ));
        factory.register_provider(override_provider);
        
        // Verify override
        let provider = factory.create_provider(MockProviderType::ClaudeCode).unwrap();
        assert_eq!(provider.display_name(), "Override Claude");
        
        // Should still have only one provider of this type
        assert_eq!(factory.list_available_providers().len(), 1);
    }

    #[tokio::test]
    async fn test_factory_created_provider_functionality() {
        let mut factory = MockProviderFactory::new();
        
        let responses = vec![
            MockResponse::text("Factory test response"),
        ];
        
        let provider = Box::new(TestProvider::with_responses(
            MockProviderType::ClaudeCode,
            "Test Provider",
            responses
        ));
        factory.register_provider(provider);
        
        // Get provider from factory
        let provider = factory.create_provider(MockProviderType::ClaudeCode).unwrap();
        
        // Create client through factory provider
        let config = MockProviderConfig::mock_valid();
        let mcp = Some(Arc::new(MockMcpIntegration::new()));
        let client = provider.create_client(&config, mcp).unwrap();
        
        // Test functionality  
        use sagitta_code::llm::client::{Message, MessagePart, Role};
        use uuid::Uuid;
        use std::collections::HashMap;
        
        let messages = vec![Message {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![MessagePart::Text { text: "Factory test".to_string() }],
            metadata: HashMap::new(),
        }];
        let response = client.generate(&messages, &[]).await;
        
        assert!(response.is_ok());
        let response = response.unwrap();
        
        if let Some(sagitta_code::llm::client::MessagePart::Text { text }) = response.message.parts.first() {
            assert_eq!(text, "Factory test response");
        } else {
            panic!("Expected text part in response");
        }
    }

    #[test]
    fn test_factory_with_different_provider_types() {
        let mut factory = MockProviderFactory::new();
        
        // Register multiple different provider types
        let claude_provider = Box::new(TestProvider::new(
            MockProviderType::ClaudeCode,
            "Claude Code"
        ));
        let mistral_provider = Box::new(TestProvider::new(
            MockProviderType::MistralRs,
            "Mistral.rs"
        ));
        let test_provider = Box::new(TestProvider::new(
            MockProviderType::TestProvider,
            "Test Provider"
        ));
        
        factory.register_provider(claude_provider);
        factory.register_provider(mistral_provider);
        factory.register_provider(test_provider);
        
        // Verify all are available
        assert_eq!(factory.list_available_providers().len(), 3);
        
        // Verify each can be retrieved correctly
        let claude = factory.create_provider(MockProviderType::ClaudeCode).unwrap();
        assert_eq!(claude.display_name(), "Claude Code");
        
        let mistral = factory.create_provider(MockProviderType::MistralRs).unwrap();
        assert_eq!(mistral.display_name(), "Mistral.rs");
        
        let test = factory.create_provider(MockProviderType::TestProvider).unwrap();
        assert_eq!(test.display_name(), "Test Provider");
    }

    #[test]
    fn test_factory_error_handling() {
        let mut factory = MockProviderFactory::new();
        
        // Register provider that fails validation and creation
        let failing_provider = Box::new(TestProvider::new_with_failures(
            MockProviderType::TestProvider,
            "Failing Provider",
            true, // fail validation
            true  // fail creation
        ));
        factory.register_provider(failing_provider);
        
        let provider = factory.create_provider(MockProviderType::TestProvider).unwrap();
        
        // Test that factory still returns provider even if it has failures
        assert_eq!(provider.display_name(), "Failing Provider");
        
        // Test that provider failures are handled correctly
        let config = MockProviderConfig::mock_valid();
        assert!(provider.validate_config(&config).is_err());
        
        let mcp = Some(Arc::new(MockMcpIntegration::new()));
        assert!(provider.create_client(&config, mcp).is_err());
    }
}