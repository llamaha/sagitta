/// Provider trait tests following TDD approach
/// 
/// These tests define the expected behavior of the Provider trait
/// before implementation, ensuring the contract is well-defined.

use crate::providers::*;
use std::sync::Arc;
use sagitta_code::llm::client::{Message, MessagePart, Role};
use uuid::Uuid;
use std::collections::HashMap;

/// Helper function to create a user message
pub fn create_user_message(text: impl Into<String>) -> Message {
    Message {
        id: Uuid::new_v4(),
        role: Role::User,
        parts: vec![MessagePart::Text { text: text.into() }],
        metadata: HashMap::new(),
    }
}

#[cfg(test)]
mod provider_trait_tests {
    use super::*;

    #[test]
    fn test_provider_trait_contract() {
        let provider = TestProvider::new(MockProviderType::ClaudeCode, "Test Provider");
        
        // Test basic trait methods
        assert_eq!(provider.provider_type(), MockProviderType::ClaudeCode);
        assert_eq!(provider.display_name(), "Test Provider");
        
        // Test validation with valid config
        let config = MockProviderConfig::mock_valid();
        assert!(provider.validate_config(&config).is_ok());
        
        // Test client creation with valid config
        let mcp = Some(Arc::new(MockMcpIntegration::new()));
        let client = provider.create_client(&config, mcp);
        assert!(client.is_ok());
        
        let client = client.unwrap();
        assert_eq!(client.client_type(), "claude-code");
    }
    
    #[test] 
    fn test_provider_error_handling() {
        let provider = TestProvider::new_with_failures(
            MockProviderType::TestProvider, 
            "Test Provider", 
            true, // fail validation
            true  // fail creation
        );
        
        // Test validation failure
        let config = MockProviderConfig::mock_valid();
        let validation_result = provider.validate_config(&config);
        assert!(validation_result.is_err());
        
        if let Err(e) = validation_result {
            assert!(matches!(e, sagitta_code::utils::errors::SagittaCodeError::ConfigError(_)));
        }
        
        // Test creation failure  
        let mcp = Some(Arc::new(MockMcpIntegration::new()));
        let creation_result = provider.create_client(&config, mcp);
        assert!(creation_result.is_err());
        
        if let Err(e) = creation_result {
            assert!(matches!(e, sagitta_code::utils::errors::SagittaCodeError::LlmError(_)));
        }
    }

    #[test]
    fn test_provider_with_invalid_config() {
        let provider = TestProvider::new(MockProviderType::MistralRs, "Mistral Provider");
        
        // Test validation with invalid config
        let invalid_config = MockProviderConfig::mock_invalid();
        let validation_result = provider.validate_config(&invalid_config);
        assert!(validation_result.is_err());
        
        // Test client creation with invalid config
        let mcp = Some(Arc::new(MockMcpIntegration::new()));
        let creation_result = provider.create_client(&invalid_config, mcp);
        assert!(creation_result.is_err());
    }

    #[test]
    fn test_provider_default_config() {
        let provider = TestProvider::new(MockProviderType::ClaudeCode, "Claude Provider");
        
        let default_config = provider.default_config();
        
        // Default config should be valid
        assert!(provider.validate_config(&default_config).is_ok());
        
        // Should be able to create client with default config
        let mcp = Some(Arc::new(MockMcpIntegration::new()));
        let client_result = provider.create_client(&default_config, mcp);
        assert!(client_result.is_ok());
    }

    #[test]
    fn test_provider_type_consistency() {
        let claude_provider = TestProvider::new(MockProviderType::ClaudeCode, "Claude");
        let mistral_provider = TestProvider::new(MockProviderType::MistralRs, "Mistral");
        
        // Each provider should return consistent type
        assert_eq!(claude_provider.provider_type(), MockProviderType::ClaudeCode);
        assert_eq!(mistral_provider.provider_type(), MockProviderType::MistralRs);
        
        // Display names should match expected
        assert_eq!(claude_provider.display_name(), "Claude");
        assert_eq!(mistral_provider.display_name(), "Mistral");
    }

    #[tokio::test]
    async fn test_provider_created_client_functionality() {
        let responses = vec![
            MockResponse::text("Hello from provider"),
            MockResponse::with_tools("Tool call response", vec![
                ("tool1".to_string(), "test_tool".to_string(), serde_json::json!({"param": "value"}))
            ]),
        ];
        
        let provider = TestProvider::with_responses(
            MockProviderType::ClaudeCode, 
            "Test Provider",
            responses
        );
        
        let config = MockProviderConfig::mock_valid();
        let mcp = Some(Arc::new(MockMcpIntegration::new()));
        let client = provider.create_client(&config, mcp).unwrap();
        
        // Test basic generation
        let messages = vec![create_user_message("Test message")];
        let response = client.generate(&messages, &[]).await;
        
        assert!(response.is_ok());
        let response = response.unwrap();
        
        // Verify response content
        if let Some(sagitta_code::llm::client::MessagePart::Text { text }) = response.message.parts.first() {
            assert_eq!(text, "Hello from provider");
        } else {
            panic!("Expected text part in response");
        }
        
        // Test second call with tool calls
        let response2 = client.generate(&messages, &[]).await;
        assert!(response2.is_ok());
        let response2 = response2.unwrap();
        assert!(!response2.tool_calls.is_empty());
    }

    #[tokio::test]
    async fn test_provider_client_streaming() {
        let provider = TestProvider::new(MockProviderType::MistralRs, "Mistral Provider");
        let config = MockProviderConfig::mock_valid();
        let mcp = Some(Arc::new(MockMcpIntegration::new()));
        let client = provider.create_client(&config, mcp).unwrap();
        
        let messages = vec![create_user_message("Stream test")];
        let stream_result = client.generate_stream(&messages, &[]).await;
        
        assert!(stream_result.is_ok());
        let mut stream = stream_result.unwrap();
        
        // Collect stream chunks
        use futures_util::StreamExt;
        let chunks: Vec<_> = stream.collect().await;
        
        // Should have at least one chunk plus final chunk
        assert!(!chunks.is_empty());
        
        // Last chunk should be final
        if let Some(Ok(last_chunk)) = chunks.last() {
            assert!(last_chunk.is_final);
        }
    }

    #[test]
    fn test_provider_types_display_names() {
        assert_eq!(MockProviderType::ClaudeCode.display_name(), "Claude Code");
        assert_eq!(MockProviderType::MistralRs.display_name(), "Mistral.rs");
        assert_eq!(MockProviderType::TestProvider.display_name(), "Test Provider");
    }

    #[test]
    fn test_provider_config_variants() {
        // Test valid config
        let valid_config = MockProviderConfig::mock_valid();
        if let MockProviderConfig::Valid { api_key } = valid_config {
            assert_eq!(api_key, Some("test-key".to_string()));
        } else {
            panic!("Expected valid config variant");
        }
        
        // Test invalid config
        let invalid_config = MockProviderConfig::mock_invalid();
        if let MockProviderConfig::Invalid { error } = invalid_config {
            assert_eq!(error, "Invalid configuration");
        } else {
            panic!("Expected invalid config variant");
        }
    }

    #[tokio::test]
    async fn test_mock_client_call_tracking() {
        let provider = TestProvider::new(MockProviderType::ClaudeCode, "Test Provider");
        let config = MockProviderConfig::mock_valid();
        let mcp = Some(Arc::new(MockMcpIntegration::new()));
        let client = provider.create_client(&config, mcp).unwrap();
        
        // Downcast to MockLlmClient to access tracking methods
        let mock_client = client.as_any().downcast_ref::<MockLlmClient>().unwrap();
        
        // Make some calls
        let messages1 = vec![create_user_message("First message")];
        let messages2 = vec![create_user_message("Second message")];
        
        let _ = client.generate(&messages1, &[]).await;
        let _ = client.generate(&messages2, &[]).await;
        
        // Verify calls were tracked
        let calls = mock_client.get_calls().await;
        assert_eq!(calls.len(), 2);
        
        // Verify call content
        assert_eq!(calls[0].0.len(), 1); // First call had 1 message
        assert_eq!(calls[1].0.len(), 1); // Second call had 1 message
        
        // Reset and verify
        mock_client.reset().await;
        let calls_after_reset = mock_client.get_calls().await;
        assert_eq!(calls_after_reset.len(), 0);
    }
}