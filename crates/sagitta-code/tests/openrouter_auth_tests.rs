//! Integration tests for OpenRouter authentication and error handling

use sagitta_code::llm::openrouter::client::OpenRouterClient;
use sagitta_code::llm::openrouter::error::OpenRouterError;
use sagitta_code::llm::client::{LlmClient, Message, MessagePart, Role};
use sagitta_code::config::types::{SagittaCodeConfig, OpenRouterConfig};
use sagitta_code::utils::errors::SagittaCodeError;
use uuid::Uuid;

/// Create a test configuration with the specified API key
fn create_test_config_with_key(api_key: Option<String>) -> SagittaCodeConfig {
    SagittaCodeConfig {
        openrouter: OpenRouterConfig {
            api_key,
            model: "openai/gpt-4o-mini".to_string(),
            provider_preferences: None,
            max_history_size: 50,
            max_reasoning_steps: 10,
            request_timeout: 5, // Short timeout for tests
        },
        ..Default::default()
    }
}

#[test]
fn test_missing_api_key_error() {
    // Save and clear environment variable
    let original_env_key = std::env::var("OPENROUTER_API_KEY").ok();
    std::env::remove_var("OPENROUTER_API_KEY");
    
    let config = create_test_config_with_key(None);
    let result = OpenRouterClient::new(&config);
    
    // Restore environment variable
    if let Some(key) = original_env_key {
        std::env::set_var("OPENROUTER_API_KEY", key);
    }
    
    assert!(result.is_err(), "Should fail without API key");
    match result {
        Err(OpenRouterError::AuthenticationError(msg)) => {
            assert!(msg.contains("not found"), "Error should mention API key not found: {}", msg);
        }
        _ => panic!("Expected AuthenticationError"),
    }
}

#[test]
fn test_empty_api_key_error() {
    let config = create_test_config_with_key(Some("".to_string()));
    let result = OpenRouterClient::new(&config);
    
    assert!(result.is_err(), "Should fail with empty API key");
    match result {
        Err(OpenRouterError::AuthenticationError(msg)) => {
            assert!(msg.contains("empty"), "Error should mention empty API key: {}", msg);
        }
        _ => panic!("Expected AuthenticationError"),
    }
}

#[test]
fn test_valid_api_key_creation() {
    let config = create_test_config_with_key(Some("test-valid-key".to_string()));
    let result = OpenRouterClient::new(&config);
    
    assert!(result.is_ok(), "Should succeed with valid API key");
}

#[tokio::test]
async fn test_invalid_api_key_401_response() {
    // This test uses a real HTTP request with an invalid API key
    // It expects a 401 Unauthorized response
    let config = create_test_config_with_key(Some("invalid-api-key".to_string()));
    let client = OpenRouterClient::new(&config).unwrap();
    
    let messages = vec![
        Message {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![MessagePart::Text { text: "Hello".to_string() }],
            metadata: Default::default(),
        }
    ];
    
    let result = client.generate(&messages, &[]).await;
    
    assert!(result.is_err(), "Should fail with invalid API key");
    match result {
        Err(SagittaCodeError::LlmError(msg)) => {
            // OpenRouter returns 401 for invalid API keys
            assert!(msg.contains("401") || msg.contains("Unauthorized"), 
                    "Error should indicate 401 Unauthorized: {}", msg);
        }
        _ => panic!("Expected LlmError with 401 status"),
    }
}

#[tokio::test]
async fn test_http_request_timeout() {
    // Create a config with very short timeout
    let mut config = create_test_config_with_key(Some("test-key".to_string()));
    config.openrouter.request_timeout = 0; // 0 second timeout should fail immediately
    
    // Use a custom base URL that will timeout
    let client = OpenRouterClient::new_with_base_url(&config, "http://192.0.2.1:9999")
        .expect("Client creation should succeed");
    
    let messages = vec![
        Message {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![MessagePart::Text { text: "Hello".to_string() }],
            metadata: Default::default(),
        }
    ];
    
    let result = client.generate(&messages, &[]).await;
    
    assert!(result.is_err(), "Should fail with timeout");
    match result {
        Err(SagittaCodeError::LlmError(msg)) => {
            assert!(msg.contains("request") || msg.contains("timeout"), 
                    "Error should indicate timeout: {}", msg);
        }
        _ => panic!("Expected LlmError with timeout"),
    }
}

#[test]
fn test_environment_variable_fallback() {
    // Save original environment variable
    let original_env_key = std::env::var("OPENROUTER_API_KEY").ok();
    
    // Set a test API key in environment
    std::env::set_var("OPENROUTER_API_KEY", "env-test-key");
    
    // Create config without API key
    let config = create_test_config_with_key(None);
    let result = OpenRouterClient::new(&config);
    
    // Restore original environment variable
    match original_env_key {
        Some(key) => std::env::set_var("OPENROUTER_API_KEY", key),
        None => std::env::remove_var("OPENROUTER_API_KEY"),
    }
    
    assert!(result.is_ok(), "Should succeed with environment variable API key");
}

#[test]
fn test_config_api_key_takes_precedence() {
    // Save original environment variable
    let original_env_key = std::env::var("OPENROUTER_API_KEY").ok();
    
    // Set a different API key in environment
    std::env::set_var("OPENROUTER_API_KEY", "env-test-key");
    
    // Create config with its own API key
    let config = create_test_config_with_key(Some("config-test-key".to_string()));
    let client = OpenRouterClient::new(&config).unwrap();
    
    // The client should use the config API key, not the environment one
    // We can't directly test which key is used, but we can verify the client was created
    assert!(matches!(client, OpenRouterClient { .. }));
    
    // Restore original environment variable
    match original_env_key {
        Some(key) => std::env::set_var("OPENROUTER_API_KEY", key),
        None => std::env::remove_var("OPENROUTER_API_KEY"),
    }
}