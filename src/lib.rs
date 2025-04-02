// Module exports
pub mod vectordb;
pub mod cli;
pub mod utils;

/// Run a demo of the Phase 2 ONNX optimizations
#[cfg(feature = "onnx")]
pub fn run_phase2_demo() -> anyhow::Result<()> {
    use std::path::PathBuf;
    use std::time::Instant;
    use crate::vectordb::provider::session_manager::{SessionManager, SessionConfig};
    use crate::vectordb::provider::tokenizer_cache::{TokenizerCache, TokenizerCacheConfig};
    use crate::vectordb::provider::batch_processor::{BatchProcessor, BatchProcessorConfig};
    use crate::vectordb::provider::onnx::ONNX_EMBEDDING_DIM;
    
    println!("ONNX Optimization Demo - Phase 2");
    println!("=================================");
    
    // Check if ONNX model files are available
    let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
    let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer");
    
    if !model_path.exists() || !tokenizer_path.exists() {
        println!("Error: ONNX model files not found at:");
        println!("  - Model: {}", model_path.display());
        println!("  - Tokenizer: {}", tokenizer_path.display());
        println!("\nPlease download the required model files or adjust paths.");
        return Ok(());
    }
    
    println!("\n1. Creating session manager...");
    // Create session manager
    let session_config = SessionConfig::default();
    let session_manager = SessionManager::new(&model_path, session_config)?;
    
    println!("2. Creating tokenizer cache...");
    // Create tokenizer cache
    let tokenizer_config = TokenizerCacheConfig::default();
    let tokenizer_cache = TokenizerCache::new(&tokenizer_path, tokenizer_config)?;
    
    println!("3. Creating batch processor...");
    // Create batch processor
    let batch_config = BatchProcessorConfig::default();
    let batch_processor = BatchProcessor::new(
        session_manager.clone(),
        tokenizer_cache.clone(),
        batch_config,
        ONNX_EMBEDDING_DIM,
    );
    
    // Test sample embeddings
    let text = "fn main() { println!(\"Hello, optimization!\"); }";
    
    println!("\nTesting embedding performance...");
    let start = Instant::now();
    let embedding = batch_processor.embed(text)?;
    let single_duration = start.elapsed();
    
    // Verify embedding dimensions
    assert_eq!(embedding.len(), ONNX_EMBEDDING_DIM);
    println!("✓ Single embedding took {:?}", single_duration);
    
    // Test batch embedding performance
    let texts = vec![
        "fn add(a: i32, b: i32) -> i32 { a + b }",
        "struct Point { x: f32, y: f32 }",
        "impl Point { fn new(x: f32, y: f32) -> Self { Self { x, y } } }",
        "fn main() { let p = Point::new(1.0, 2.0); }",
    ];
    
    println!("\nTesting batch embedding performance...");
    let start = Instant::now();
    let embeddings = batch_processor.embed_batch(&texts)?;
    let batch_duration = start.elapsed();
    
    // Verify batch results
    assert_eq!(embeddings.len(), texts.len());
    assert_eq!(embeddings[0].len(), ONNX_EMBEDDING_DIM);
    println!("✓ Batch embedding ({} items) took {:?}", texts.len(), batch_duration);
    println!("  Average time per item: {:?}", batch_duration / texts.len() as u32);
    
    // Calculate the speedup from batching
    let single_avg = single_duration;
    let batch_avg = batch_duration / texts.len() as u32;
    let speedup = single_avg.as_secs_f32() / batch_avg.as_secs_f32();
    println!("  Speedup from batching: {:.2}x", speedup);
    
    // Test tokenizer cache performance
    println!("\nTesting tokenizer cache performance...");
    let cached_text = "function calculateSum(a, b) { return a + b; }";
    
    // First tokenization (uncached)
    let start = Instant::now();
    let _ = tokenizer_cache.tokenize(cached_text)?;
    let uncached_duration = start.elapsed();
    println!("✓ Uncached tokenization took {:?}", uncached_duration);
    
    // Second tokenization (should be cached)
    let start = Instant::now();
    let _ = tokenizer_cache.tokenize(cached_text)?;
    let cached_duration = start.elapsed();
    println!("✓ Cached tokenization took {:?}", cached_duration);
    
    // Calculate cache speedup
    let cache_speedup = uncached_duration.as_secs_f32() / cached_duration.as_secs_f32();
    println!("  Cache speedup: {:.2}x", cache_speedup);
    
    // Test session pooling
    println!("\nTesting session pooling...");
    
    // Create a new session from scratch
    let start = Instant::now();
    let session1 = session_manager.get_session()?;
    let new_session_duration = start.elapsed();
    println!("✓ Creating new session took {:?}", new_session_duration);
    
    // Return session to the pool
    session_manager.return_session(session1);
    
    // Get a session from the pool (should be reused)
    let start = Instant::now();
    let _session2 = session_manager.get_session()?;
    let pooled_session_duration = start.elapsed();
    println!("✓ Getting session from pool took {:?}", pooled_session_duration);
    
    // Calculate pooling speedup
    let pooling_speedup = new_session_duration.as_secs_f32() / pooled_session_duration.as_secs_f32();
    println!("  Session pooling speedup: {:.2}x", pooling_speedup);
    
    println!("\nSummary of Optimizations:");
    println!("-------------------------");
    println!("✓ Tokenizer caching: {:.2}x speedup", cache_speedup);
    println!("✓ Session pooling:   {:.2}x speedup", pooling_speedup);
    println!("✓ Batch processing:  {:.2}x speedup", speedup);
    
    Ok(())
} 