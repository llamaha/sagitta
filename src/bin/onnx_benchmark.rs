use std::path::PathBuf;
use std::time::{Duration, Instant};
use anyhow::{Result, Error};
use clap::{Parser, ValueEnum};
use vectordb_cli::vectordb::provider::session_manager::{SessionManager, SessionConfig};
use vectordb_cli::vectordb::provider::tokenizer_cache::{TokenizerCache, TokenizerCacheConfig};
use vectordb_cli::vectordb::provider::batch_processor::{BatchProcessor, BatchProcessorConfig};
use vectordb_cli::vectordb::provider::onnx::{OptimizedOnnxEmbeddingProvider, OnnxEmbeddingProvider, ONNX_EMBEDDING_DIM};
use vectordb_cli::vectordb::provider::EmbeddingProvider;

/// Command line arguments
#[derive(Parser, Debug)]
#[clap(author, version, about = "Benchmark for ONNX embedding optimizations")]
struct Args {
    /// Path to ONNX model file
    #[clap(long, default_value = "onnx/all-minilm-l12-v2.onnx")]
    model_path: String,
    
    /// Path to tokenizer directory
    #[clap(long, default_value = "onnx/minilm_tokenizer")]
    tokenizer_path: String,
    
    /// Number of warmup iterations
    #[clap(long, default_value = "3")]
    warmup_iterations: usize,
    
    /// Number of benchmark iterations
    #[clap(long, default_value = "10")]
    bench_iterations: usize,
    
    /// Batch sizes to test
    #[clap(long, default_value = "1,4,8,16,32")]
    batch_sizes: String,
    
    /// Provider to benchmark
    #[clap(long, value_enum, default_value = "optimized")]
    provider: ProviderType,
    
    /// Text file with sample inputs (one per line)
    #[clap(long, default_value = "samples.txt")]
    samples_file: String,
    
    /// Whether to pre-warm the session pool
    #[clap(long, default_value = "true")]
    pre_warm: bool,
    
    /// Whether to use dynamic batching
    #[clap(long, default_value = "true")]
    dynamic_batching: bool,
    
    /// Output results in CSV format
    #[clap(long, default_value = "false")]
    csv: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum ProviderType {
    /// Basic ONNX provider
    Basic,
    /// Optimized ONNX provider
    Optimized,
}

/// Create sample texts with varying lengths
fn create_sample_texts() -> Vec<String> {
    // Short texts
    let short_texts = vec![
        "This is a short text".to_string(),
        "Another short text example".to_string(),
        "Hello, world!".to_string(),
        "ONNX Runtime is fast".to_string(),
    ];
    
    // Medium texts
    let medium_texts = vec![
        "This is a medium length text that should have more tokens than the shorter examples above. It contains multiple sentences to ensure adequate length.".to_string(),
        "Embedding models like MiniLM are designed to produce fixed-length vector representations of text that capture semantic meaning. These vectors can be used for similarity search.".to_string(),
        "The Rust programming language offers memory safety without a garbage collector, making it suitable for performance-critical applications like embedding generation.".to_string(),
    ];
    
    // Long texts
    let long_texts = vec![
        "This is a longer text that will require more tokens to process. It contains multiple sentences and paragraphs to ensure that batch processing logic can be properly tested. Batch processing is an important optimization when working with ONNX models and transformer-based architectures. By grouping multiple inputs together, we can better utilize the parallel processing capabilities of modern hardware. This should have significantly more tokens than the short and medium examples.".to_string(),
        "The ONNX Runtime provides an optimized inference engine for ONNX models. It includes various optimization capabilities such as operator fusion, memory planning, and parallelization across multiple compute resources. When working with embedding models, efficient batching and tokenization are critical for achieving good performance. The RunTime also supports hardware acceleration through CUDA, DirectML, and other platform-specific acceleration technologies. This longer text will require more tokens and serve as a good test case for batching efficiency.".to_string(),
    ];
    
    // Combine all texts
    let mut all_texts = Vec::new();
    all_texts.extend(short_texts);
    all_texts.extend(medium_texts);
    all_texts.extend(long_texts);
    
    // Create repeated sets to have sufficient data
    let mut result = Vec::new();
    for _ in 0..5 {
        result.extend(all_texts.clone());
    }
    
    result
}

/// Load sample texts from a file
fn load_sample_texts(path: &str) -> Result<Vec<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let lines: Vec<String> = content.lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect();
            
            if lines.is_empty() {
                println!("Warning: No samples found in file. Using generated samples.");
                Ok(create_sample_texts())
            } else {
                println!("Loaded {} sample texts from {}", lines.len(), path);
                Ok(lines)
            }
        },
        Err(e) => {
            println!("Warning: Failed to load samples file ({}): {}", path, e);
            println!("Using generated samples instead.");
            Ok(create_sample_texts())
        }
    }
}

/// Run benchmark with the basic ONNX provider
fn benchmark_basic(
    model_path: &str,
    tokenizer_path: &str,
    samples: &[String],
    batch_size: usize,
    iterations: usize,
) -> Result<Vec<Duration>> {
    // Create the provider
    let model_path = PathBuf::from(model_path);
    let tokenizer_path = PathBuf::from(tokenizer_path);
    let provider = OnnxEmbeddingProvider::new(&model_path, &tokenizer_path)?;
    
    // Prepare batches
    let mut results = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        // Select a random subset of samples for this iteration
        let batch_start = fastrand::usize(0..samples.len().saturating_sub(batch_size));
        let batch_texts: Vec<&str> = samples[batch_start..batch_start + batch_size]
            .iter()
            .map(|s| s.as_str())
            .collect();
        
        // Time the embedding generation
        let start = Instant::now();
        let embeddings = provider.embed_batch(&batch_texts)?;
        let duration = start.elapsed();
        
        // Verify the embeddings
        assert_eq!(embeddings.len(), batch_size);
        for embedding in &embeddings {
            assert_eq!(embedding.len(), ONNX_EMBEDDING_DIM);
        }
        
        results.push(duration);
    }
    
    Ok(results)
}

/// Run benchmark with the optimized ONNX provider
fn benchmark_optimized(
    model_path: &str,
    tokenizer_path: &str,
    samples: &[String],
    batch_size: usize,
    iterations: usize,
    pre_warm: bool,
    dynamic_batching: bool,
) -> Result<Vec<Duration>> {
    // Create session configuration
    let mut session_config = SessionConfig::default();
    session_config.warmup_iterations = if pre_warm { 3 } else { 0 };
    
    // Create tokenizer configuration
    let tokenizer_config = TokenizerCacheConfig::default();
    
    // Create batch processor configuration
    let mut batch_config = BatchProcessorConfig::default();
    batch_config.max_batch_size = batch_size;
    batch_config.dynamic_batching = dynamic_batching;
    
    // Create the provider
    let model_path = PathBuf::from(model_path);
    let tokenizer_path = PathBuf::from(tokenizer_path);
    let provider = OptimizedOnnxEmbeddingProvider::new(
        &model_path,
        &tokenizer_path,
        Some(session_config),
        Some(tokenizer_config),
        Some(batch_config),
    )?;
    
    // Pre-warm the session pool if enabled
    if pre_warm {
        println!("Pre-warming session pool...");
        // This is done automatically when creating the provider
    }
    
    // Prepare batches
    let mut results = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        // Select a random subset of samples for this iteration
        let batch_start = fastrand::usize(0..samples.len().saturating_sub(batch_size));
        let batch_texts: Vec<&str> = samples[batch_start..batch_start + batch_size]
            .iter()
            .map(|s| s.as_str())
            .collect();
        
        // Time the embedding generation
        let start = Instant::now();
        let embeddings = provider.embed_batch(&batch_texts)?;
        let duration = start.elapsed();
        
        // Verify the embeddings
        assert_eq!(embeddings.len(), batch_size);
        for embedding in &embeddings {
            assert_eq!(embedding.len(), ONNX_EMBEDDING_DIM);
        }
        
        results.push(duration);
    }
    
    Ok(results)
}

/// Format a duration as milliseconds with 2 decimal places
fn format_ms(duration: Duration) -> String {
    let ms = duration.as_secs_f64() * 1000.0;
    format!("{:.2}", ms)
}

/// Calculate statistics for a set of durations
fn calculate_stats(durations: &[Duration]) -> (Duration, Duration, Duration) {
    let mut sorted = durations.to_vec();
    sorted.sort();
    
    let total = sorted.iter().sum::<Duration>();
    let mean = total / durations.len() as u32;
    
    let median = if sorted.is_empty() {
        Duration::from_secs(0)
    } else if sorted.len() % 2 == 1 {
        sorted[sorted.len() / 2]
    } else {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2
    };
    
    (mean, median, total)
}

fn main() -> Result<()> {
    // Parse arguments
    let args = Args::parse();
    
    // Load sample texts
    let samples = load_sample_texts(&args.samples_file)?;
    println!("Using {} sample texts for benchmarking", samples.len());
    
    // Parse batch sizes
    let batch_sizes: Vec<usize> = args.batch_sizes
        .split(',')
        .map(|s| s.trim().parse::<usize>())
        .collect::<std::result::Result<Vec<_>, _>>()?;
    
    // Print configuration
    println!("\nBenchmark Configuration:");
    println!("------------------------");
    println!("Provider:         {}", match args.provider {
        ProviderType::Basic => "Basic ONNX",
        ProviderType::Optimized => "Optimized ONNX",
    });
    println!("Model:            {}", args.model_path);
    println!("Tokenizer:        {}", args.tokenizer_path);
    println!("Warmup iterations: {}", args.warmup_iterations);
    println!("Bench iterations:  {}", args.bench_iterations);
    println!("Batch sizes:       {}", args.batch_sizes);
    println!("Pre-warm pool:     {}", args.pre_warm);
    println!("Dynamic batching:  {}", args.dynamic_batching);
    
    if args.csv {
        // CSV header
        println!("\nprovider,batch_size,mean_ms,median_ms,total_ms,throughput");
    } else {
        println!("\nResults:");
        println!("--------");
    }
    
    // Run benchmarks for each batch size
    for batch_size in batch_sizes {
        if batch_size > samples.len() {
            println!("Warning: Batch size {} exceeds number of samples {}. Skipping.", 
                    batch_size, samples.len());
            continue;
        }
        
        // Run warmup iterations first
        println!("Running {} warmup iterations with batch size {}...", args.warmup_iterations, batch_size);
        let warmup_result = match args.provider {
            ProviderType::Basic => {
                benchmark_basic(
                    &args.model_path,
                    &args.tokenizer_path,
                    &samples,
                    batch_size,
                    args.warmup_iterations,
                )
            },
            ProviderType::Optimized => {
                benchmark_optimized(
                    &args.model_path,
                    &args.tokenizer_path,
                    &samples,
                    batch_size,
                    args.warmup_iterations,
                    args.pre_warm,
                    args.dynamic_batching,
                )
            }
        };
        
        if warmup_result.is_err() {
            println!("Error during warmup: {:?}", warmup_result.err());
            continue;
        }
        
        // Run actual benchmark
        println!("Running {} benchmark iterations with batch size {}...", args.bench_iterations, batch_size);
        let bench_result = match args.provider {
            ProviderType::Basic => {
                benchmark_basic(
                    &args.model_path,
                    &args.tokenizer_path,
                    &samples,
                    batch_size,
                    args.bench_iterations,
                )
            },
            ProviderType::Optimized => {
                benchmark_optimized(
                    &args.model_path,
                    &args.tokenizer_path,
                    &samples,
                    batch_size,
                    args.bench_iterations,
                    args.pre_warm,
                    args.dynamic_batching,
                )
            }
        };
        
        match bench_result {
            Ok(durations) => {
                // Calculate statistics
                let (mean, median, total) = calculate_stats(&durations);
                let throughput = batch_size as f64 * args.bench_iterations as f64 / total.as_secs_f64();
                
                // Output results
                if args.csv {
                    let provider_name = match args.provider {
                        ProviderType::Basic => "basic",
                        ProviderType::Optimized => "optimized",
                    };
                    println!("{},{},{},{},{},{:.2}",
                            provider_name,
                            batch_size,
                            format_ms(mean),
                            format_ms(median),
                            format_ms(total),
                            throughput);
                } else {
                    println!("Batch size {}:", batch_size);
                    println!("  Mean time:   {} ms", format_ms(mean));
                    println!("  Median time: {} ms", format_ms(median));
                    println!("  Total time:  {} ms", format_ms(total));
                    println!("  Throughput:  {:.2} samples/sec", throughput);
                }
            },
            Err(e) => {
                println!("Error benchmarking batch size {}: {}", batch_size, e);
            }
        }
    }
    
    Ok(())
} 