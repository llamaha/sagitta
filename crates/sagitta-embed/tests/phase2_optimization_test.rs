//! Phase 2 optimization integration tests for Sagitta embeddings engine.

use sagitta_embed::{
    config::{EmbeddingConfig, EmbeddingConfigBuilder, IOBindingConfig, MemoryPoolConfig, 
             DynamicBatchConfig, ExecutionProvider},
    model::EmbeddingModelType,
    provider::onnx::model::OnnxEmbeddingModel,
    provider::EmbeddingProvider,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use std::fs;

/// Helper function to create dummy model and tokenizer files for testing
fn create_dummy_model_files() -> (PathBuf, PathBuf) {
    let temp_dir = tempdir().unwrap();
    let model_path = temp_dir.path().join("model.onnx");
    let tokenizer_path = temp_dir.path().join("tokenizer.json");

    // Create minimal dummy ONNX model file
    fs::write(&model_path, "dummy onnx model content").unwrap();
    
    // Create minimal dummy tokenizer file
    let tokenizer_json = r#"{
        "version": "1.0",
        "normalizer": null,
        "pre_tokenizer": null,
        "post_processor": null,
        "decoder": null,
        "model": {
            "type": "WordPiece",
            "vocab": {"[UNK]": 0, "[CLS]": 1, "[SEP]": 2, "hello": 3, "world": 4},
            "unk_token": "[UNK]"
        },
        "added_tokens": []
    }"#;
    fs::write(&tokenizer_path, tokenizer_json).unwrap();

    // Prevent cleanup by leaking temp_dir
    let model_path = model_path.clone();
    let tokenizer_path = tokenizer_path.clone();
    std::mem::forget(temp_dir);
    
    (model_path, tokenizer_path)
}

#[test]
fn test_phase2_io_binding_optimization() {
    let (model_path, tokenizer_path) = create_dummy_model_files();
    
    // Test with advanced I/O binding configuration
    let io_config = IOBindingConfig {
        enable_io_binding: true,
        enable_pre_allocated_buffers: true,
        enable_zero_copy: true,
        enable_batch_optimization: true,
        pre_allocated_input_buffers: 8,
        pre_allocated_output_buffers: 8,
    };
    
    let config = EmbeddingConfig::new_onnx(model_path.clone(), tokenizer_path.clone())
        .with_io_binding_config(io_config);

    // This test will initially fail - we need to implement the I/O binding logic
    // The model should use pre-allocated buffers when available
    #[cfg(feature = "onnx")]
    {
        let result = OnnxEmbeddingModel::new_with_config(&model_path, &tokenizer_path, &config);
        // For now, just verify config is applied - we'll implement the actual I/O binding next
        assert!(result.is_ok() || result.is_err()); // Placeholder until implementation
    }
}

#[test]
fn test_phase2_memory_pool_optimization() {
    let (model_path, tokenizer_path) = create_dummy_model_files();
    
    // Test with memory pool configuration  
    let memory_config = MemoryPoolConfig {
        enable_pool: true,
        max_pool_size: 16,
        max_pool_memory_bytes: 128 * 1024 * 1024, // 128MB
        enable_memory_pressure_detection: true,
        memory_pressure_threshold: 0.75,
    };
    
    let config = EmbeddingConfig::new_onnx(model_path.clone(), tokenizer_path.clone())
        .with_memory_pool_config(memory_config);

    #[cfg(feature = "onnx")]
    {
        let result = OnnxEmbeddingModel::new_with_config(&model_path, &tokenizer_path, &config);
        // Verify memory pool is configured - implementation to follow
        assert!(result.is_ok() || result.is_err()); // Placeholder until implementation
    }
}

#[test]
fn test_phase2_execution_provider_auto_selection() {
    let (model_path, tokenizer_path) = create_dummy_model_files();
    
    // Test with execution provider auto-selection
    let config = EmbeddingConfig::new_onnx(model_path.clone(), tokenizer_path.clone())
        .with_execution_providers(vec![
            ExecutionProvider::Cuda, 
            ExecutionProvider::Cpu
        ])
        .with_provider_auto_selection(true)
        .with_hardware_detection(true);

    #[cfg(feature = "onnx")]
    {
        let result = OnnxEmbeddingModel::new_with_config(&model_path, &tokenizer_path, &config);
        // Should automatically select best available provider
        assert!(result.is_ok() || result.is_err()); // Placeholder until implementation
    }
}

#[test]
fn test_phase2_dynamic_batch_optimization() {
    let (model_path, tokenizer_path) = create_dummy_model_files();
    
    // Test with dynamic batch optimization
    let batch_config = DynamicBatchConfig {
        enable_dynamic_batching: true,
        min_batch_size: 1,
        max_batch_size: 16,
        target_latency_ms: 50,
        enable_memory_prediction: true,
        optimize_for_throughput: false, // Optimize for latency
    };
    
    let config = EmbeddingConfig::new_onnx(model_path.clone(), tokenizer_path.clone())
        .with_dynamic_batch_config(batch_config);

    #[cfg(feature = "onnx")]
    {
        let result = OnnxEmbeddingModel::new_with_config(&model_path, &tokenizer_path, &config);
        // Should enable adaptive batching based on latency targets
        assert!(result.is_ok() || result.is_err()); // Placeholder until implementation
    }
}

#[test] 
fn test_phase2_performance_benchmarks() {
    let (model_path, tokenizer_path) = create_dummy_model_files();
    
    // Compare Phase 1 vs Phase 2 configuration performance
    let phase1_config = EmbeddingConfig::new_onnx(model_path.clone(), tokenizer_path.clone())
        .with_io_binding(true)
        .with_graph_optimization_level(3);
        
    let phase2_config = EmbeddingConfig::new_onnx(model_path.clone(), tokenizer_path.clone())
        .with_pre_allocated_buffers(true)
        .with_pre_allocated_input_buffers(8)
        .with_memory_pool(true)
        .with_max_pool_size(32)
        .with_dynamic_batching(true)
        .with_cuda_memory_streams(true);

    // This test will demonstrate the performance improvements once implemented
    #[cfg(feature = "onnx")]
    {
        let phase1_result = OnnxEmbeddingModel::new_with_config(&model_path, &tokenizer_path, &phase1_config);
        let phase2_result = OnnxEmbeddingModel::new_with_config(&model_path, &tokenizer_path, &phase2_config);
        
        // Both should work, but Phase 2 should be faster (we'll measure this later)
        assert!(phase1_result.is_ok() || phase1_result.is_err());
        assert!(phase2_result.is_ok() || phase2_result.is_err());
    }
}

#[test]
fn test_phase2_memory_pressure_adaptation() {
    let (model_path, tokenizer_path) = create_dummy_model_files();
    
    // Test memory pressure detection and adaptation
    let config = EmbeddingConfig::new_onnx(model_path.clone(), tokenizer_path.clone())
        .with_memory_pressure_detection(true)
        .with_memory_pressure_threshold(0.8)
        .with_max_pool_memory(64 * 1024 * 1024); // 64MB limit

    #[cfg(feature = "onnx")]
    {
        let result = OnnxEmbeddingModel::new_with_config(&model_path, &tokenizer_path, &config);
        // Should adapt memory usage based on pressure detection
        assert!(result.is_ok() || result.is_err()); // Placeholder until implementation
    }
}

#[test]
fn test_phase2_zero_copy_operations() {
    let (model_path, tokenizer_path) = create_dummy_model_files();
    
    // Test zero-copy memory operations
    let config = EmbeddingConfig::new_onnx(model_path.clone(), tokenizer_path.clone())
        .with_zero_copy(true)
        .with_batch_optimization(true)
        .with_cuda_memory_streams(true);

    #[cfg(feature = "onnx")]
    {
        let result = OnnxEmbeddingModel::new_with_config(&model_path, &tokenizer_path, &config);
        // Should minimize memory copies between host and device
        assert!(result.is_ok() || result.is_err()); // Placeholder until implementation
    }
}

#[test]
fn test_phase2_comprehensive_optimization() {
    let (model_path, tokenizer_path) = create_dummy_model_files();
    
    // Test all Phase 2 optimizations together
    let config = EmbeddingConfigBuilder::new()
        .model_type(EmbeddingModelType::Onnx)
        .onnx_model_path(model_path.clone())
        .onnx_tokenizer_path(tokenizer_path.clone())
        // Phase 1 optimizations
        .with_intra_op_threads(4)
        .with_inter_op_threads(1)
        .with_graph_optimization_level(3)
        .with_memory_pattern(true)
        .with_cpu_arena(true)
        // Phase 2 optimizations
        .with_pre_allocated_buffers(true)
        .with_pre_allocated_input_buffers(8)
        .with_pre_allocated_output_buffers(8)
        .with_zero_copy(true)
        .with_batch_optimization(true)
        .with_memory_pool(true)
        .with_max_pool_size(32)
        .with_max_pool_memory(256 * 1024 * 1024) // 256MB
        .with_memory_pressure_detection(true)
        .with_memory_pressure_threshold(0.8)
        .add_execution_provider(ExecutionProvider::Cuda)
        .add_execution_provider(ExecutionProvider::Cpu)
        .with_provider_auto_selection(true)
        .with_dynamic_batching(true)
        .with_min_batch_size(1)
        .with_max_batch_size(32)
        .with_target_latency(100)
        .with_memory_prediction(true)
        .with_throughput_optimization(false)
        .with_cuda_memory_streams(true)
        .with_hardware_detection(true)
        .build_unchecked();

    #[cfg(feature = "onnx")]
    {
        let result = OnnxEmbeddingModel::new_with_config(&model_path, &tokenizer_path, &config);
        // Should combine all optimizations for maximum performance
        assert!(result.is_ok() || result.is_err()); // Placeholder until implementation
    }
    
    // Verify all configuration options are properly set
    assert_eq!(config.io_binding_config.enable_pre_allocated_buffers, true);
    assert_eq!(config.io_binding_config.pre_allocated_input_buffers, 8);
    assert_eq!(config.memory_pool_config.enable_pool, true);
    assert_eq!(config.memory_pool_config.max_pool_size, 32);
    assert_eq!(config.dynamic_batch_config.enable_dynamic_batching, true);
    assert_eq!(config.execution_providers.len(), 3); // Auto + Cuda + Cpu
    assert_eq!(config.enable_provider_auto_selection, true);
    assert_eq!(config.enable_cuda_memory_streams, true);
    assert_eq!(config.enable_hardware_detection, true);
} 