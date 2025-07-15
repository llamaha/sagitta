//! Provider factory for creating and managing provider instances

use super::{ProviderType, ProviderConfig, ProviderManager, Provider, ClaudeCodeProvider};
use super::mistral_rs::MistralRsProvider;
use crate::utils::errors::SagittaCodeError;

/// Factory for creating provider instances
pub struct ProviderFactory {}

impl ProviderFactory {
    /// Creates a new provider factory
    pub fn new() -> Self {
        Self {}
    }
    
    /// Creates a provider manager with all available providers registered
    pub fn create_manager(&self) -> Result<ProviderManager, SagittaCodeError> {
        let mut manager = ProviderManager::new();
        
        // Register all available providers
        self.register_all_providers(&mut manager)?;
        
        Ok(manager)
    }
    
    /// Creates a specific provider instance
    pub fn create_provider(&self, provider_type: ProviderType) -> Result<Box<dyn Provider>, SagittaCodeError> {
        match provider_type {
            ProviderType::ClaudeCode => {
                Ok(Box::new(ClaudeCodeProvider::new()))
            },
            ProviderType::MistralRs => {
                Ok(Box::new(MistralRsProvider::new()))
            },
        }
    }
    
    /// Registers all available providers with the manager
    fn register_all_providers(&self, manager: &mut ProviderManager) -> Result<(), SagittaCodeError> {
        // Register Claude Code provider
        let claude_provider = self.create_provider(ProviderType::ClaudeCode)?;
        manager.register_provider(claude_provider);
        
        // Register Mistral.rs provider
        let mistral_provider = self.create_provider(ProviderType::MistralRs)?;
        manager.register_provider(mistral_provider);
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn create_test_factory() -> ProviderFactory {
        ProviderFactory::new()
    }
    
    #[test]
    fn test_create_claude_code_provider() {
        let factory = create_test_factory();
        let provider = factory.create_provider(ProviderType::ClaudeCode).unwrap();
        
        assert_eq!(provider.provider_type(), ProviderType::ClaudeCode);
        assert_eq!(provider.display_name(), "Claude Code");
        assert!(!provider.requires_api_key());
    }
    
    #[test]
    fn test_create_mistral_rs_provider() {
        let factory = create_test_factory();
        let provider = factory.create_provider(ProviderType::MistralRs).unwrap();
        
        assert_eq!(provider.provider_type(), ProviderType::MistralRs);
        assert_eq!(provider.display_name(), "Mistral.rs");
        assert!(!provider.requires_api_key());
    }
    
    #[test]
    fn test_create_manager_with_all_providers() {
        let factory = create_test_factory();
        let manager = factory.create_manager().unwrap();
        
        let provider_types = manager.get_provider_types();
        assert!(provider_types.contains(&ProviderType::ClaudeCode));
        assert!(provider_types.contains(&ProviderType::MistralRs));
    }
    
    #[test]
    fn test_claude_code_config_validation() {
        let factory = create_test_factory();
        let provider = factory.create_provider(ProviderType::ClaudeCode).unwrap();
        
        let config = provider.default_config();
        match provider.validate_config(&config) {
            Ok(_) => {},
            Err(e) => panic!("Claude Code validation failed: {:?}", e),
        }
        
        // Test invalid provider type
        let mut invalid_config = config.clone();
        invalid_config.provider_type = ProviderType::MistralRs;
        assert!(provider.validate_config(&invalid_config).is_err());
    }
    
    #[test]
    fn test_mistral_rs_config_validation() {
        let factory = create_test_factory();
        let provider = factory.create_provider(ProviderType::MistralRs).unwrap();
        
        let config = provider.default_config();
        match provider.validate_config(&config) {
            Ok(_) => {},
            Err(e) => panic!("Mistral.rs validation failed: {:?}", e),
        }
        
        // Test invalid URL
        let mut invalid_config = config.clone();
        invalid_config.set_option("base_url", "invalid-url").unwrap();
        assert!(provider.validate_config(&invalid_config).is_err());
        
        // Test empty URL
        let mut empty_config = config.clone();
        empty_config.set_option("base_url", "").unwrap();
        assert!(provider.validate_config(&empty_config).is_err());
    }
}