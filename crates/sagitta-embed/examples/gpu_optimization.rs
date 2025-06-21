//! Example demonstrating automatic optimization for both CPU and GPU builds.
//!
//! This example shows how to configure sagitta-embed for optimal performance
//! using auto-detection that works out-of-the-box for both CPU and GPU builds.

use sagitta_embed::{
    config::EmbeddingConfig,
    provider::onnx::OnnxEmbeddingModel,
    provider::EmbeddingProvider,
};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Create auto-optimized configuration that works out-of-the-box
    // - For CUDA builds: Enables I/O binding and GPU optimizations
    // - For CPU builds: Enables CPU-specific optimizations
    let config = EmbeddingConfig::default()
        .with_auto_optimization()  // Automatically detects and optimizes for available features
        .with_max_batch_size(32);   // Only configuration users typically need to adjust

    println!("Configuration optimized for: {}", 
        if config.should_use_cuda() { "GPU (CUDA)" } else { "CPU" });
    
    println!("I/O binding enabled: {}", config.io_binding_config.enable_io_binding);

    // For demonstration purposes, we'll use dummy paths
    // In real usage, these would point to your actual model files
    let model_path = PathBuf::from("path/to/your/model.onnx");
    let tokenizer_path = PathBuf::from("path/to/your/tokenizer.json");

    // The configuration will automatically use the best settings for your build
    match OnnxEmbeddingModel::new_with_config(&model_path, &tokenizer_path, &config) {
        Ok(model) => {
            println!("Model loaded successfully with optimized configuration!");
            
            // Example texts for embedding
            let texts = vec![
                "This is a sample text for embedding".to_string(),
                "Another example sentence".to_string(),
                "Machine learning is fascinating".to_string(),
            ];

            // Generate embeddings using optimized configuration
            let text_refs: Vec<&str> = texts.iter().map(|s| &**s).collect();
            let embeddings = model.embed_batch(&text_refs)?;

            println!("Generated {} embeddings with dimension {}", 
                embeddings.len(), 
                embeddings.first().map(|e| e.len()).unwrap_or(0)
            );
        }
        Err(e) => {
            println!("Note: This example uses dummy paths. Error: {}", e);
            println!("In real usage, provide valid paths to your ONNX model and tokenizer files.");
        }
    }

    Ok(())
} 