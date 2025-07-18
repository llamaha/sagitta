//! Provider factory for creating and managing provider instances

use super::{ProviderType, ProviderManager, Provider, ClaudeCodeProvider, ProviderConfig};
use super::openai_compatible::OpenAICompatibleProvider;
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

            ProviderType::OpenAICompatible => {
                Ok(Box::new(OpenAICompatibleProvider::new()))
            },

            ProviderType::ClaudeCodeRouter => {
                // TODO: Implement ClaudeCodeRouterProvider
                Err(SagittaCodeError::ConfigError("Claude Code Router provider not yet implemented".to_string()))
            },

            ProviderType::MistralRs => {
                // TODO: Implement MistralRs provider
                Err(SagittaCodeError::ConfigError("MistralRs provider not yet implemented".to_string()))
            },
        }
    }
    
    /// Registers all available providers with the manager
    fn register_all_providers(&self, manager: &mut ProviderManager) -> Result<(), SagittaCodeError> {
        // Register Claude Code provider
        let claude_provider = self.create_provider(ProviderType::ClaudeCode)?;
        manager.register_provider(claude_provider);
        

        
        // Register OpenAI Compatible provider
        let openai_provider = self.create_provider(ProviderType::OpenAICompatible)?;
        manager.register_provider(openai_provider);
        
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
    fn test_create_manager_with_all_providers() {
        let factory = create_test_factory();
        let manager = factory.create_manager().unwrap();
        
        let provider_types = manager.get_provider_types();
        assert!(provider_types.contains(&ProviderType::ClaudeCode));

        assert!(provider_types.contains(&ProviderType::OpenAICompatible));
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
        invalid_config.provider_type = ProviderType::OpenAICompatible;
        assert!(provider.validate_config(&invalid_config).is_err());
    }
    

    
    #[test]
    fn test_create_openai_compatible_provider() {
        let factory = create_test_factory();
        let provider = factory.create_provider(ProviderType::OpenAICompatible).unwrap();
        
        assert_eq!(provider.provider_type(), ProviderType::OpenAICompatible);
        assert_eq!(provider.display_name(), "OpenAI Compatible");
        assert!(!provider.requires_api_key());
    }
    
    #[test]
    fn test_openai_compatible_config_validation() {
        let factory = create_test_factory();
        let provider = factory.create_provider(ProviderType::OpenAICompatible).unwrap();
        
        let config = provider.default_config();
        match provider.validate_config(&config) {
            Ok(_) => {},
            Err(e) => panic!("OpenAI Compatible validation failed: {:?}", e),
        }
        
        // Test missing base URL
        let mut invalid_config = ProviderConfig::new(ProviderType::OpenAICompatible);
        assert!(provider.validate_config(&invalid_config).is_err());
        
        // Test invalid timeout
        let mut zero_timeout_config = config.clone();
        zero_timeout_config.set_option("timeout_seconds", 0u64).unwrap();
        assert!(provider.validate_config(&zero_timeout_config).is_err());
    }
}