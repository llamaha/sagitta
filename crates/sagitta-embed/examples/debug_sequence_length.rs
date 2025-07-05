use sagitta_embed::config::EmbeddingConfig;
use sagitta_embed::provider::onnx::OnnxEmbeddingModel;
use sagitta_embed::provider::EmbeddingProvider;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with debug level
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    println!("=== Debugging Sequence Length Detection ===");

    // Get model paths from environment or use defaults
    let model_path = std::env::var("SAGITTA_ONNX_MODEL")
        .unwrap_or_else(|_| "bge_small_onnx/model.onnx".to_string());
    let tokenizer_path = std::env::var("SAGITTA_ONNX_TOKENIZER")
        .unwrap_or_else(|_| "bge_small_onnx".to_string());

    println!("Model path: {model_path}");
    println!("Tokenizer path: {tokenizer_path}");

    // Create configuration - sequence length will be auto-detected
    let config = EmbeddingConfig::default();

    // Also test with default config (sequence length auto-detected in both cases)
    let default_config = EmbeddingConfig::default();

    // Load the model
    println!("\n--- Loading Model with Auto-Detection ---");
    let model = OnnxEmbeddingModel::new_with_config(
        &PathBuf::from(model_path.clone()),
        &PathBuf::from(tokenizer_path.clone()),
        &config,
    )?;

    println!("\n--- Model Information ---");
    println!("Model debug info: {}", model.debug_info());
    println!("Model max_sequence_length(): {}", model.max_sequence_length());
    println!("Model dimension(): {}", model.dimension());

    // Test with default config
    println!("\n--- Loading Another Model Instance ---");
    let default_model = OnnxEmbeddingModel::new_with_config(
        &PathBuf::from(model_path),
        &PathBuf::from(tokenizer_path),
        &default_config,
    )?;

    println!("\n--- Second Model Information ---");
    println!("Model debug info: {}", default_model.debug_info());
    println!("Model max_sequence_length(): {}", default_model.max_sequence_length());
    println!("Model dimension(): {}", default_model.dimension());

    // Test with a sample text to see tokenization behavior
    println!("\n--- Testing Tokenization ---");
    let test_text = "This is a test sentence to see how tokenization works with the current sequence length setting.";
    println!("Test text: {test_text}");

    let embeddings = model.embed_batch(&[test_text])?;
    println!("Generated embedding with dimension: {}", embeddings[0].len());

    // Test with a longer text that might exceed 128 tokens
    println!("\n--- Testing with Longer Text ---");
    let long_text = "This is a much longer test sentence that contains many more words and should definitely exceed 128 tokens when tokenized. ".repeat(10);
    println!("Long text length: {} characters", long_text.len());

    let long_embeddings = model.embed_batch(&[&long_text])?;
    println!("Generated embedding for long text with dimension: {}", long_embeddings[0].len());

    println!("\n=== Debug Complete ===");
    Ok(())
} 