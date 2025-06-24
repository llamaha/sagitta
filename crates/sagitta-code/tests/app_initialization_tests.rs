//! Tests for application initialization, especially handling missing components

use sagitta_code::config::types::SagittaCodeConfig;
use sagitta_search::config::AppConfig;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a minimal test configuration
fn create_minimal_config() -> (SagittaCodeConfig, AppConfig) {
    let temp_dir = TempDir::new().unwrap();
    
    let code_config = SagittaCodeConfig {
        openrouter: sagitta_code::config::types::OpenRouterConfig {
            api_key: Some("test-key".to_string()),
            model: "openai/gpt-4o-mini".to_string(),
            provider_preferences: None,
            max_history_size: 10,
            max_reasoning_steps: 5,
            request_timeout: 30,
        },
        ui: sagitta_code::config::types::UiConfig {
            ..Default::default()
        },
        ..Default::default()
    };
    
    let search_config = AppConfig {
        qdrant_url: "http://localhost:6334".to_string(),
        onnx_model_path: None, // No embedding model configured
        onnx_tokenizer_path: None,
        embed_model: None,
        ..Default::default()
    };
    
    (code_config, search_config)
}

#[tokio::test]
async fn test_openrouter_client_creation_with_config() {
    use sagitta_code::llm::openrouter::client::OpenRouterClient;
    
    let (code_config, _search_config) = create_minimal_config();
    
    // Test that OpenRouter client can be created with valid config
    let client_result = OpenRouterClient::new(&code_config);
    assert!(client_result.is_ok(), "OpenRouter client should be created successfully with API key");
}

#[tokio::test]
async fn test_openrouter_client_without_api_key() {
    use sagitta_code::llm::openrouter::client::OpenRouterClient;
    use sagitta_code::config::types::SagittaCodeConfig;
    
    // Save and clear environment variable
    let original_env_key = std::env::var("OPENROUTER_API_KEY").ok();
    std::env::remove_var("OPENROUTER_API_KEY");
    
    let mut code_config = SagittaCodeConfig::default();
    code_config.openrouter.api_key = None; // No API key
    
    let result = OpenRouterClient::new(&code_config);
    
    // Restore environment variable
    if let Some(key) = original_env_key {
        std::env::set_var("OPENROUTER_API_KEY", key);
    }
    
    assert!(result.is_err(), "OpenRouter client creation should fail without API key");
    if let Err(e) = result {
        assert!(e.to_string().contains("not found"), 
                "Error should mention missing API key: {}", e);
    }
}

#[test]
fn test_repository_base_path_default() {
    use sagitta_code::config::types::SagittaCodeConfig;
    
    let config = SagittaCodeConfig::default();
    
    let base_path = config.repositories_base_path();
    assert!(base_path.to_string_lossy().contains("sagitta"), 
            "Default repository base path should contain 'sagitta'");
}

#[tokio::test]
async fn test_repository_manager_with_invalid_qdrant() {
    use sagitta_code::gui::repository::manager::RepositoryManager;
    use sagitta_search::config::AppConfig;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    
    let config = Arc::new(Mutex::new(AppConfig {
        qdrant_url: "http://localhost:9999".to_string(), // Non-existent Qdrant instance
        ..Default::default()
    }));
    
    // Create repository manager - it should handle missing Qdrant gracefully
    let repo_manager = Arc::new(Mutex::new(RepositoryManager::new(config)));
    let mut manager_lock = repo_manager.lock().await;
    
    // Initialize should complete even if Qdrant is not available
    let init_result = manager_lock.initialize().await;
    assert!(init_result.is_ok(), "Repository manager should initialize even without Qdrant");
}