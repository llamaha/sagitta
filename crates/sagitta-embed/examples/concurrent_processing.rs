use sagitta_embed::{EmbeddingPool, EmbeddingConfig, EmbeddingProcessor, EmbeddingModelType};
use sagitta_embed::processor::{ProcessedChunk, ChunkMetadata};
use std::sync::Arc;
use std::time::Instant;
use std::path::PathBuf;
use tokio::task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Sagitta-Embed Concurrent Processing Example");
    println!("===========================================");
    
    // Note: This example requires actual model files to run
    let model_path = "path/to/your/model.onnx";
    let tokenizer_path = "path/to/your/tokenizer.json";
    
    // Check if model files exist
    if !std::path::Path::new(model_path).exists() || !std::path::Path::new(tokenizer_path).exists() {
        println!("⚠️  Model files not found. This example requires:");
        println!("   - ONNX model file: {}", model_path);
        println!("   - Tokenizer file: {}", tokenizer_path);
        println!();
        println!("To run this example:");
        println!("1. Download a compatible ONNX embedding model");
        println!("2. Update the paths in this example");
        println!("3. Run: cargo run --example concurrent_processing");
        return Ok(());
    }
    
    // Configure with multiple sessions for concurrency
    let config = EmbeddingConfig::new_onnx(
        PathBuf::from(model_path),
        PathBuf::from(tokenizer_path)
    )
    // max_sessions removed - using automatic session management
    .with_embedding_batch_size(128); // Add the new field
    
    // Set the ONNX paths
    let config = EmbeddingConfig {
        onnx_model_path: Some(model_path.into()),
        onnx_tokenizer_path: Some(tokenizer_path.into()),
        enable_session_cleanup: true,
        ..config
    };
    
    println!("Creating embedding pool with automatic session management...");
    let pool = Arc::new(EmbeddingPool::with_configured_sessions(config)?);
    
    // Generate sample data for concurrent processing
    let batch_count = 10;
    let texts_per_batch = 5;
    
    println!("Starting {} concurrent batches with {} texts each...", batch_count, texts_per_batch);
    
    let start_time = Instant::now();
    let mut handles = vec![];
    
    // Spawn concurrent tasks
    for batch_id in 0..batch_count {
        let pool_clone = pool.clone();
        
        let handle = task::spawn(async move {
            let batch_start = Instant::now();
            
            // Generate texts for this batch
            let texts: Vec<String> = (0..texts_per_batch)
                .map(|i| format!("Batch {} text {}: This is sample text for concurrent embedding generation.", batch_id, i))
                .collect();
            
            // Convert texts to ProcessedChunks
            let chunks: Vec<ProcessedChunk> = texts.iter().enumerate().map(|(i, text)| {
                ProcessedChunk {
                    content: text.clone(),
                    metadata: ChunkMetadata {
                        file_path: PathBuf::from(format!("batch_{}.txt", batch_id)),
                        start_line: i,
                        end_line: i,
                        language: "text".to_string(),
                        file_extension: "txt".to_string(),
                        element_type: "text".to_string(),
                        context: Some(format!("batch_{}", batch_id)),
                    },
                    id: format!("batch_{}_{}", batch_id, i),
                }
            }).collect();
            
            // Generate embeddings using the process_chunks method
            let result = pool_clone.process_chunks(chunks).await;
            
            let batch_duration = batch_start.elapsed();
            
            match result {
                Ok(embedded_chunks) => {
                    println!("✅ Batch {} completed in {:?} - {} embeddings generated", 
                            batch_id, batch_duration, embedded_chunks.len());
                    Ok((batch_id, embedded_chunks.len(), batch_duration))
                },
                Err(e) => {
                    println!("❌ Batch {} failed: {}", batch_id, e);
                    Err(e)
                }
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete and collect results
    let mut successful_batches = 0;
    let mut total_embeddings = 0;
    let mut batch_durations = vec![];
    
    for handle in handles {
        match handle.await {
            Ok(Ok((batch_id, embedding_count, duration))) => {
                successful_batches += 1;
                total_embeddings += embedding_count;
                batch_durations.push(duration);
            },
            Ok(Err(e)) => {
                println!("Batch processing error: {}", e);
            },
            Err(e) => {
                println!("Task join error: {}", e);
            }
        }
    }
    
    let total_duration = start_time.elapsed();
    
    // Print summary
    println!();
    println!("Concurrent Processing Summary");
    println!("============================");
    println!("Total duration: {:?}", total_duration);
    println!("Successful batches: {}/{}", successful_batches, batch_count);
    println!("Total embeddings generated: {}", total_embeddings);
    
    if !batch_durations.is_empty() {
        let avg_batch_duration = batch_durations.iter().sum::<std::time::Duration>() / batch_durations.len() as u32;
        let min_batch_duration = batch_durations.iter().min().unwrap();
        let max_batch_duration = batch_durations.iter().max().unwrap();
        
        println!("Batch duration stats:");
        println!("  Average: {:?}", avg_batch_duration);
        println!("  Min: {:?}", min_batch_duration);
        println!("  Max: {:?}", max_batch_duration);
        
        let embeddings_per_second = total_embeddings as f64 / total_duration.as_secs_f64();
        println!("Throughput: {:.2} embeddings/second", embeddings_per_second);
    }
    
    // Demonstrate sequential vs concurrent comparison
    println!();
    println!("Sequential Processing Comparison");
    println!("===============================");
    
    let sequential_start = Instant::now();
    let mut sequential_embeddings = 0;
    
    for batch_id in 0..3 { // Just do a few batches for comparison
        let texts: Vec<String> = (0..texts_per_batch)
            .map(|i| format!("Sequential batch {} text {}: Sample text for comparison.", batch_id, i))
            .collect();
        
        let chunks: Vec<ProcessedChunk> = texts.iter().enumerate().map(|(i, text)| {
            ProcessedChunk {
                content: text.clone(),
                metadata: ChunkMetadata {
                    file_path: PathBuf::from(format!("sequential_{}.txt", batch_id)),
                    start_line: i,
                    end_line: i,
                    language: "text".to_string(),
                    file_extension: "txt".to_string(),
                    element_type: "text".to_string(),
                    context: Some(format!("sequential_{}", batch_id)),
                },
                id: format!("sequential_{}_{}", batch_id, i),
            }
        }).collect();
        
        let embedded_chunks = pool.process_chunks(chunks).await?;
        sequential_embeddings += embedded_chunks.len();
    }
    
    let sequential_duration = sequential_start.elapsed();
    let sequential_rate = sequential_embeddings as f64 / sequential_duration.as_secs_f64();
    
    println!("Sequential: {} embeddings in {:?} ({:.2} embeddings/second)", 
             sequential_embeddings, sequential_duration, sequential_rate);
    
    if !batch_durations.is_empty() {
        let concurrent_rate = total_embeddings as f64 / total_duration.as_secs_f64();
        let speedup = concurrent_rate / sequential_rate;
        println!("Concurrent speedup: {:.2}x faster", speedup);
    }
    
    Ok(())
} 