//! Basic usage example for the sagitta-embed crate.
//!
//! This example demonstrates how to use the embedding engine with ONNX models.
//! Note: This example requires actual ONNX model files to run successfully.

use sagitta_embed::{EmbeddingPool, EmbeddingConfig, EmbeddingProcessor};
use sagitta_embed::processor::{ProcessedChunk, ChunkMetadata};
use std::path::{Path, PathBuf};

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
    let config = EmbeddingConfig::new_onnx(model_path.to_path_buf(), tokenizer_path.to_path_buf());
    
    println!("Creating embedding pool...");
    let pool = EmbeddingPool::with_configured_sessions(config)?;
    
    // Sample texts to embed
    let texts = vec![
        "Hello, world!",
        "This is a sample text for embedding generation.",
        "Sagitta-embed provides high-performance text embeddings.",
        "ONNX models enable fast inference with session pooling.",
    ];
    
    println!("Generating embeddings for {} texts...", texts.len());
    
    // Convert texts to ProcessedChunks
    let chunks: Vec<ProcessedChunk> = texts.iter().enumerate().map(|(i, text)| {
        ProcessedChunk {
            content: text.to_string(),
            metadata: ChunkMetadata {
                file_path: PathBuf::from("example.txt"),
                start_line: i,
                end_line: i,
                language: "text".to_string(),
                file_extension: "txt".to_string(),
                element_type: "text".to_string(),
                context: None,
            },
            id: format!("text_{}", i),
        }
    }).collect();
    
    // Generate embeddings using the process_chunks method
    let embedded_chunks = pool.process_chunks(chunks).await?;
    
    println!("✅ Successfully generated {} embeddings!", embedded_chunks.len());
    println!();
    
    // Display results
    for (i, embedded_chunk) in embedded_chunks.iter().enumerate() {
        println!("Text {}: \"{}\"", i + 1, embedded_chunk.chunk.content);
        println!("  Embedding dimension: {}", embedded_chunk.embedding.len());
        println!("  First 5 values: {:?}", &embedded_chunk.embedding[..5.min(embedded_chunk.embedding.len())]);
        println!();
    }
    
    // Demonstrate single text embedding
    println!("Generating embedding for single text...");
    let single_text = "Single text embedding example";
    let single_chunk = ProcessedChunk {
        content: single_text.to_string(),
        metadata: ChunkMetadata {
            file_path: PathBuf::from("single.txt"),
            start_line: 0,
            end_line: 0,
            language: "text".to_string(),
            file_extension: "txt".to_string(),
            element_type: "text".to_string(),
            context: None,
        },
        id: "single_0".to_string(),
    };
    
    let single_embedded = pool.process_chunks(vec![single_chunk]).await?;
    
    println!("✅ Single embedding generated!");
    println!("Text: \"{}\"", single_embedded[0].chunk.content);
    println!("Dimension: {}", single_embedded[0].embedding.len());
    
    Ok(())
} 