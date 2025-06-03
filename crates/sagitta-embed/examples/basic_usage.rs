//! Basic usage example for the sagitta-embed crate.
//!
//! This example demonstrates how to use the embedding engine with ONNX models.
//! Note: This example requires actual ONNX model files to run successfully.

use sagitta_embed::{EmbeddingHandler, EmbeddingConfig, EmbeddingProvider};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Sagitta-Embed Basic Usage Example");
    println!("==================================");
    
    // Note: This example requires actual model files to run
    // For testing purposes, we'll use placeholder paths
    let model_path = Path::new("path/to/your/model.onnx");
    let tokenizer_path = Path::new("path/to/your/tokenizer.json");
    
    // Check if model files exist
    if !model_path.exists() || !tokenizer_path.exists() {
        println!("⚠️  Model files not found. This example requires:");
        println!("   - ONNX model file: {}", model_path.display());
        println!("   - Tokenizer file: {}", tokenizer_path.display());
        println!();
        println!("To run this example:");
        println!("1. Download a compatible ONNX embedding model");
        println!("2. Update the paths in this example");
        println!("3. Run: cargo run --example basic_usage");
        return Ok(());
    }
    
    // Configure for ONNX model
    let config = EmbeddingConfig::new_onnx(model_path, tokenizer_path);
    
    println!("Creating embedding handler...");
    let handler = EmbeddingHandler::new(&config)?;
    
    // Sample texts to embed
    let texts = vec![
        "Hello, world!",
        "This is a sample text for embedding generation.",
        "Sagitta-embed provides high-performance text embeddings.",
        "ONNX models enable fast inference with session pooling.",
    ];
    
    println!("Generating embeddings for {} texts...", texts.len());
    
    // Convert to string slices for the API
    let text_refs: Vec<&str> = texts.iter().map(|s| s.as_ref()).collect();
    
    // Generate embeddings using the embed method
    let embeddings = handler.embed(&text_refs)?;
    
    println!("✅ Successfully generated {} embeddings!", embeddings.len());
    println!();
    
    // Display results
    for (i, (text, embedding)) in texts.iter().zip(embeddings.iter()).enumerate() {
        println!("Text {}: \"{}\"", i + 1, text);
        println!("  Embedding dimension: {}", embedding.len());
        println!("  First 5 values: {:?}", &embedding[..5.min(embedding.len())]);
        println!();
    }
    
    // Demonstrate single text embedding
    println!("Generating embedding for single text...");
    let single_text = "Single text embedding example";
    let single_embedding = handler.embed(&[single_text])?;
    
    println!("✅ Single embedding generated!");
    println!("Text: \"{}\"", single_text);
    println!("Dimension: {}", single_embedding[0].len());
    
    Ok(())
} 