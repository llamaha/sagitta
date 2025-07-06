//! Debug utility to inspect ONNX model specifications
//!
//! This utility helps debug model input/output dimensions and sequence length detection.

use sagitta_embed::{
    config::EmbeddingConfig,
    provider::onnx::OnnxEmbeddingModel,
    provider::EmbeddingProvider,
};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Get model paths from command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <model.onnx> <tokenizer_dir>", args[0]);
        std::process::exit(1);
    }

    let model_path = PathBuf::from(&args[1]);
    let tokenizer_path = PathBuf::from(&args[2]);

    println!("🔍 Inspecting ONNX model: {}", model_path.display());
    println!("🔍 Using tokenizer: {}", tokenizer_path.display());

    // Create a config with auto-optimization
    let config = EmbeddingConfig::default()
        .with_auto_optimization()
;

    println!("\n📋 Configuration before model loading:");
    println!("  - Configuration created with auto-optimization");
    println!("  - I/O binding enabled: {}", config.io_binding_config.enable_io_binding);
    println!("  - Should use CUDA: {}", config.should_use_cuda());

    // Load the model - this should trigger auto-detection
    match OnnxEmbeddingModel::new_with_config(&model_path, &tokenizer_path, &config) {
        Ok(model) => {
            println!("\n✅ Model loaded successfully!");
            println!("📊 Model Information:");
            println!("  - Detected embedding dimension: {}", model.dimension());
            println!("  - Detected max sequence length: {}", model.max_sequence_length());
            
            // Test with a sample text
            let test_text = "This is a test sentence to check the model.";
            println!("\n🧪 Testing inference with sample text:");
            println!("  Text: \"{test_text}\"");
            
            match model.embed_batch(&[test_text]) {
                Ok(embeddings) => {
                    if let Some(embedding) = embeddings.first() {
                        println!("  ✅ Inference successful!");
                        println!("  - Embedding dimension: {}", embedding.len());
                        println!("  - First 5 values: {:?}", &embedding[..5.min(embedding.len())]);
                    }
                }
                Err(e) => {
                    println!("  ❌ Inference failed: {e}");
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to load model: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
} 