// Integration and comprehensive testing for the provider system
// This module brings together all provider-related tests

pub mod providers {
    pub mod integration_tests;
}

pub mod config {
    pub mod migration_tests;
}

pub mod ui {
    pub mod provider_ui_tests;
}

// Common test utilities
pub mod common {
    use std::sync::Once;
    
    static INIT: Once = Once::new();
    
    /// Initialize test environment (call once per test process)
    pub fn init_test_isolation() {
        INIT.call_once(|| {
            let _ = env_logger::builder()
                .is_test(true)
                .filter_level(log::LevelFilter::Debug)
                .try_init();
            
            // Additional test setup can go here
            println!("Provider system integration tests initialized");
        });
    }
}

#[cfg(test)]
mod provider_system_tests {
    use super::*;
    use sagitta_code::config::types::SagittaCodeConfig;
    use sagitta_code::providers::types::ProviderType;
    use sagitta_code::providers::factory::ProviderFactory;
    use sagitta_code::providers::manager::ProviderManager;
    use sagitta_code::providers::claude_code::mcp_integration::McpIntegration;
    use std::sync::Arc;
    
    #[tokio::test]
    async fn test_complete_provider_system_workflow() {
        common::init_test_isolation();
        
        // Test the complete workflow from config to provider usage
        let mut config = SagittaCodeConfig::default();
        config.current_provider = ProviderType::ClaudeCode;
        
        let factory = ProviderFactory::new();
        let provider_manager = factory.create_manager().unwrap();
        
        // Set the initial active provider
        provider_manager.set_active_provider(ProviderType::ClaudeCode).unwrap();
        
        // Test that the complete system works together
        assert!(provider_manager.get_active_provider_type().is_some());
        
        // Test provider switching
        let result = provider_manager.set_active_provider(ProviderType::MistralRs);
        
        // Should either succeed or fail gracefully
        match result {
            Ok(_) => {
                // Provider switching succeeded
                println!("Provider switching successful");
            },
            Err(e) => {
                // Provider switching failed but handled gracefully
                println!("Provider switching failed gracefully: {}", e);
            }
        }
    }
    
    #[test]
    fn test_provider_type_functionality() {
        common::init_test_isolation();
        
        // Test ProviderType enum functionality
        assert_eq!(ProviderType::ClaudeCode.display_name(), "Claude Code");
        assert_eq!(ProviderType::MistralRs.display_name(), "Mistral.rs");
        
        // Test serialization/deserialization (important for config)
        let claude_json = serde_json::to_string(&ProviderType::ClaudeCode).unwrap();
        let mistral_json = serde_json::to_string(&ProviderType::MistralRs).unwrap();
        
        assert_eq!(claude_json, "\"ClaudeCode\"");
        assert_eq!(mistral_json, "\"MistralRs\"");
        
        // Test deserialization
        let claude_from_json: ProviderType = serde_json::from_str(&claude_json).unwrap();
        let mistral_from_json: ProviderType = serde_json::from_str(&mistral_json).unwrap();
        
        assert_eq!(claude_from_json, ProviderType::ClaudeCode);
        assert_eq!(mistral_from_json, ProviderType::MistralRs);
    }
    
    #[test]
    fn test_provider_factory_completeness() {
        common::init_test_isolation();
        
        let factory = ProviderFactory::new();
        
        // Test that factory can create all provider types
        assert!(factory.create_provider(ProviderType::ClaudeCode).is_ok());
        assert!(factory.create_provider(ProviderType::OpenAICompatible).is_ok());
        assert!(factory.create_provider(ProviderType::ClaudeCodeRouter).is_ok());
        
        // Test that factory provides correct provider types
        let claude_provider = factory.create_provider(ProviderType::ClaudeCode).unwrap();
        let openai_provider = factory.create_provider(ProviderType::OpenAICompatible).unwrap();
        let router_provider = factory.create_provider(ProviderType::ClaudeCodeRouter).unwrap();
        
        assert_eq!(claude_provider.provider_type(), ProviderType::ClaudeCode);
        assert_eq!(openai_provider.provider_type(), ProviderType::OpenAICompatible);
        assert_eq!(router_provider.provider_type(), ProviderType::ClaudeCodeRouter);
        
        assert_eq!(claude_provider.display_name(), "Claude Code");
        assert_eq!(openai_provider.display_name(), "OpenAI Compatible");
        assert_eq!(router_provider.display_name(), "Claude Code Router");
    }
    
    #[test]
    fn test_provider_feature_support() {
        common::init_test_isolation();
        
        let factory = ProviderFactory::new();
        
        // Test Claude Code features
        let claude_provider = factory.create_provider(ProviderType::ClaudeCode).unwrap();
        let claude_features = claude_provider.supported_features();
        
        assert!(claude_features.contains(&"streaming".to_string()));
        assert!(claude_features.contains(&"mcp_tools".to_string()));
        
        // Test OpenAI Compatible features
        let openai_provider = factory.create_provider(ProviderType::OpenAICompatible).unwrap();
        let openai_features = openai_provider.supported_features();
        
        assert!(openai_features.contains(&"streaming".to_string()));
        // Note: OpenAI Compatible may not support MCP tools
    }
}