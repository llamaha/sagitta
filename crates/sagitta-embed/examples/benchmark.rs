//! Benchmark example for the sagitta-embed crate.
//!
//! This example demonstrates performance characteristics of the embedding engine
//! with different configurations and provides throughput measurements.

use sagitta_embed::{EmbeddingPool, EmbeddingConfig};
use std::path::PathBuf;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Sagitta-Embed Performance Benchmark");
    println!("===================================");
    
    // Note: This example requires actual model files to run
    let model_path = "path/to/your/model.onnx";
    let tokenizer_path = "path/to/your/tokenizer.json";
    
    // Check if model files exist
    if !std::path::Path::new(model_path).exists() || !std::path::Path::new(tokenizer_path).exists() {
        println!("⚠️  Model files not found. This benchmark requires:");
        println!("   - ONNX model file: {model_path}");
        println!("   - Tokenizer file: {tokenizer_path}");
        println!();
        println!("To run this benchmark:");
        println!("1. Download a compatible ONNX embedding model");
        println!("2. Update the paths in this example");
        println!("3. Run: cargo run --example benchmark --release");
        return Ok(());
    }
    
    // Generate test data
    let test_texts: Vec<String> = (0..1000).map(|i| {
        format!("This is test text number {i} for benchmarking the embedding generation performance. \
                 It contains enough content to be representative of real-world usage patterns. \
                 The text includes various programming concepts like functions, variables, and algorithms.")
    }).collect();
    
    let text_refs: Vec<&str> = test_texts.iter().map(|s| s.as_ref()).collect();
    
    println!("Generated {} test texts for benchmarking", test_texts.len());
    println!();
    
    // Benchmark different configurations
    let configs = vec![
        ("Low Memory (2 sessions)", create_config(model_path, tokenizer_path, 2, 64)),
        ("Balanced (4 sessions)", create_config(model_path, tokenizer_path, 4, 128)),
        ("High Throughput (8 sessions)", create_config(model_path, tokenizer_path, 8, 256)),
    ];
    
    for (name, config) in configs {
        println!("Benchmarking: {name}");
        println!("Configuration:");
        println!("  - Session management: automatic");
        println!("  - Batch size: {}", config.get_embedding_batch_size());
        
        match run_benchmark(&config, &text_refs).await {
            Ok((duration, throughput)) => {
                println!("  ✅ Results:");
                println!("     - Duration: {duration:?}");
                println!("     - Throughput: {throughput:.2} embeddings/second");
                println!("     - Latency per embedding: {:.2}ms", 1000.0 / throughput);
            },
            Err(e) => {
                println!("  ❌ Failed: {e}");
            }
        }
        println!();
    }
    
    // Test the simple embed_texts_async API
    println!("Testing simple embed_texts_async API");
    println!("===================================");
    
    let simple_config = create_config(model_path, tokenizer_path, 4, 128);
    let pool = EmbeddingPool::with_configured_sessions(simple_config)?;
    
    let simple_texts = vec![
        "Hello, world!",
        "This is a test of the simple API",
        "Embedding generation with sagitta-embed",
    ];
    
    let start = Instant::now();
    let embeddings = pool.embed_texts_async(&simple_texts).await?;
    let duration = start.elapsed();
    
    println!("✅ Simple API test completed:");
    println!("   - Texts: {}", simple_texts.len());
    println!("   - Duration: {duration:?}");
    println!("   - Embedding dimension: {}", embeddings[0].len());
    println!("   - First embedding preview: {:?}", &embeddings[0][..5]);
    
    Ok(())
}

fn create_config(model_path: &str, tokenizer_path: &str, sessions: usize, batch_size: usize) -> EmbeddingConfig {
    EmbeddingConfig::new_onnx(
        PathBuf::from(model_path),
        PathBuf::from(tokenizer_path)
    )
    // max_sessions removed - using automatic session management
    .with_embedding_batch_size(batch_size)
}

async fn run_benchmark(config: &EmbeddingConfig, texts: &[&str]) -> Result<(std::time::Duration, f64), Box<dyn std::error::Error>> {
    let pool = EmbeddingPool::with_configured_sessions(config.clone())?;
    
    // Warm up
    let warmup_texts = &texts[..10.min(texts.len())];
    let _ = pool.embed_texts_async(warmup_texts).await?;
    
    // Actual benchmark
    let start = Instant::now();
    let embeddings = pool.embed_texts_async(texts).await?;
    let duration = start.elapsed();
    
    let throughput = embeddings.len() as f64 / duration.as_secs_f64();
    
    Ok((duration, throughput))
} 