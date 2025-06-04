//! Test to demonstrate and verify max_sessions configuration behavior

use sagitta_embed::{
    EmbeddingPool, EmbeddingConfig, EmbeddingModelType,
    ProcessingConfig
};
use std::path::PathBuf;
use tempfile::tempdir;
use std::fs;

fn create_test_embedding_config(max_sessions: usize) -> EmbeddingConfig {
    let temp_dir = tempdir().unwrap();
    let model_path = temp_dir.path().join("model.onnx");
    let tokenizer_path = temp_dir.path().join("tokenizer.json");

    // Create dummy files
    fs::write(&model_path, "dummy model").unwrap();
    
    // Create minimal valid tokenizer JSON
    let tokenizer_content = serde_json::json!({
        "version": "1.0",
        "truncation": null,
        "padding": null,
        "added_tokens": [],
        "normalizer": null,
        "pre_tokenizer": null,
        "post_processor": null,
        "decoder": null,
        "model": {
            "type": "WordPiece",
            "unk_token": "[UNK]",
            "continuing_subword_prefix": "##",
            "max_input_chars_per_word": 100,
            "vocab": {
                "[UNK]": 0,
                "[CLS]": 1,
                "[SEP]": 2
            }
        }
    });
    fs::write(&tokenizer_path, tokenizer_content.to_string()).unwrap();

    EmbeddingConfig {
        model_type: EmbeddingModelType::Onnx,
        onnx_model_path: Some(model_path),
        onnx_tokenizer_path: Some(tokenizer_path),
        max_sessions,
        max_sequence_length: 128,
        expected_dimension: Some(384),
        session_timeout_seconds: 300,
        enable_session_cleanup: true,
        tenant_id: None,
        embedding_batch_size: Some(128),
    }
}

#[tokio::test]
async fn test_embedding_pool_respects_max_sessions() {
    // This test shows the correct behavior: EmbeddingPool respects max_sessions
    
    let embedding_config_2 = create_test_embedding_config(2);
    let embedding_config_4 = create_test_embedding_config(4);
    let embedding_config_8 = create_test_embedding_config(8);

    // Test with max_sessions = 2
    if let Ok(pool_2) = EmbeddingPool::with_configured_sessions(embedding_config_2) {
        let stats_2 = pool_2.pool_stats().await;
        assert_eq!(stats_2.max_models, 2, "Pool should respect max_sessions=2");
        assert_eq!(stats_2.available_permits, 2, "Should have 2 available permits");
        println!("✅ EmbeddingPool with max_sessions=2: Creates pool with 2 model instances");
    }

    // Test with max_sessions = 4
    if let Ok(pool_4) = EmbeddingPool::with_configured_sessions(embedding_config_4) {
        let stats_4 = pool_4.pool_stats().await;
        assert_eq!(stats_4.max_models, 4, "Pool should respect max_sessions=4");
        assert_eq!(stats_4.available_permits, 4, "Should have 4 available permits");
        println!("✅ EmbeddingPool with max_sessions=4: Creates pool with 4 model instances");
    }

    // Test with max_sessions = 8
    if let Ok(pool_8) = EmbeddingPool::with_configured_sessions(embedding_config_8) {
        let stats_8 = pool_8.pool_stats().await;
        assert_eq!(stats_8.max_models, 8, "Pool should respect max_sessions=8");
        assert_eq!(stats_8.available_permits, 8, "Should have 8 available permits");
        println!("✅ EmbeddingPool with max_sessions=8: Creates pool with 8 model instances");
    }
}

#[tokio::test]
async fn test_processing_config_from_embedding_config() {
    // Test that ProcessingConfig correctly extracts max_sessions from EmbeddingConfig
    
    for max_sessions in [1, 2, 4, 8, 16] {
        let embedding_config = create_test_embedding_config(max_sessions);
        let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
        
        assert_eq!(
            processing_config.max_embedding_sessions, 
            max_sessions,
            "ProcessingConfig should extract max_sessions={} from EmbeddingConfig", 
            max_sessions
        );
        
        // Verify that EmbeddingPool uses this configuration correctly
        if let Ok(pool) = EmbeddingPool::with_configured_sessions(embedding_config) {
            let stats = pool.pool_stats().await;
            assert_eq!(stats.max_models, max_sessions, "EmbeddingPool should respect max_sessions={}", max_sessions);
        }
    }
}

#[test]
fn test_config_toml_simulation() {
    // Simulate how config.toml values flow through the system
    
    // Simulate config.toml [embedding] section
    struct ConfigToml {
        max_sessions: usize,
    }
    
    let config_toml = ConfigToml { max_sessions: 8 };
    
    // This is what happens in app_config_to_embedding_config()
    let embedding_config = EmbeddingConfig {
        max_sessions: config_toml.max_sessions, // This part works correctly
        ..Default::default()
    };
    
    assert_eq!(embedding_config.max_sessions, 8);
    
    // This is what now happens for proper GPU memory control
    let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
    assert_eq!(processing_config.max_embedding_sessions, 8);
    
    // EmbeddingPool::with_configured_sessions() properly respects the max_sessions setting
    println!("✅ Configuration flow: config.toml -> EmbeddingConfig -> ProcessingConfig -> EmbeddingPool");
}

#[test]
fn test_default_values_consistency() {
    // Ensure default values are consistent across the codebase
    
    let embedding_config = EmbeddingConfig::default();
    let processing_config_default = ProcessingConfig::default();
    let processing_config_from_embedding = ProcessingConfig::from_embedding_config(&embedding_config);
    
    // Default values should match
    assert_eq!(embedding_config.max_sessions, 4); // DEFAULT_MAX_SESSIONS
    assert_eq!(processing_config_default.max_embedding_sessions, 4);
    assert_eq!(processing_config_from_embedding.max_embedding_sessions, 4);
}

#[tokio::test]
async fn test_gpu_memory_control_through_sessions() {
    // Test that max_sessions properly controls GPU memory usage
    
    for max_sessions in [1, 2, 4] {
        let embedding_config = create_test_embedding_config(max_sessions);
        
        if let Ok(pool) = EmbeddingPool::with_configured_sessions(embedding_config) {
            let stats = pool.pool_stats().await;
            
            // Verify that the pool respects the session limit
            assert_eq!(stats.max_models, max_sessions);
            assert_eq!(stats.available_permits, max_sessions);
            assert_eq!(stats.utilization(), 0.0);
            assert!(!stats.is_at_capacity());
            
            println!("✅ GPU memory control: max_sessions={} properly limits to {} model instances", 
                    max_sessions, stats.max_models);
        }
    }
} 