# Sagitta Embedding Engine

A high-performance, modular embedding engine supporting multiple providers
and optimized for code search and semantic analysis.

## Features

- **Multiple Providers**: Support for ONNX and other embedding providers
- **High Performance**: Optimized for concurrent access with session pooling
- **CUDA Support**: Optional GPU acceleration for ONNX models
- **Modular Design**: Clean separation of concerns with pluggable providers
- **Type Safety**: Comprehensive error handling and type safety
- **Optimized Processing**: CPU-intensive file processing scales independently from GPU-intensive embedding generation

## Architecture

### Processing Pipeline

The engine features a modern architecture that separates CPU-intensive file processing from GPU-intensive embedding generation:

- **File Processor**: Handles I/O, parsing, and chunking. Scales to all CPU cores without GPU memory concerns.
- **Embedding Pool**: Manages a controlled number of embedding model instances for optimal GPU memory usage.
- **Async Pipeline**: Provides natural backpressure and prevents blocking between processing phases.

### Benefits

- ✅ **CPU Optimization**: File processing utilizes all available CPU cores independently
- ✅ **GPU Memory Control**: Fixed number of embedding models regardless of file processing concurrency  
- ✅ **Better Resource Utilization**: Parallel file I/O/parsing with controlled embedding throughput
- ✅ **Improved Scalability**: Process large repositories without GPU OOM issues
- ✅ **Configurable Trade-offs**: Independent tuning of CPU vs GPU resource usage

## Quick Start

### Traditional Embedding API

```rust
use sagitta_embed::{EmbeddingHandler, EmbeddingConfig};
use std::path::PathBuf;

let config = EmbeddingConfig::new_onnx(
    PathBuf::from("model.onnx"),
    PathBuf::from("tokenizer.json")
);

let handler = EmbeddingHandler::new(&config)?;
let embeddings = handler.embed(&["Hello world", "Rust programming"])?;
println!("Generated {} embeddings", embeddings.len());
```

### Processing API

```rust
use sagitta_embed::{
    DefaultFileProcessor, EmbeddingPool, FileProcessor, EmbeddingProcessor,
    ProcessingConfig, EmbeddingConfig
};
use std::path::PathBuf;

// Configure embedding settings (e.g., from config.toml via app_config_to_embedding_config)
let embedding_config = EmbeddingConfig::new_onnx(
    PathBuf::from("model.onnx"),
    PathBuf::from("tokenizer.json")
).with_max_sessions(2); // ⭐ This controls GPU memory usage

// Create embedding pool with configured sessions
let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config.clone())?;

// Create file processor with config that respects max_sessions
let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
let file_processor = DefaultFileProcessor::new(processing_config);

// Process files (CPU-intensive, parallel)
let files = vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")];
let processed_chunks = file_processor.process_files(&files).await?;

// Generate embeddings (GPU-intensive, controlled)
let embedded_chunks = embedding_pool.process_chunks(processed_chunks).await?;

println!("Generated {} embeddings", embedded_chunks.len());
```

## Configuration

### Processing Configuration

```rust
use sagitta_embed::ProcessingConfig;

let config = ProcessingConfig {
    file_processing_concurrency: 16,   // CPU cores for file processing
    max_embedding_sessions: 2,         // GPU memory instances  
    processing_queue_size: 1000,       // Buffer size
    embedding_batch_size: 64,          // Batch size for embedding
    max_file_size_bytes: 5 * 1024 * 1024, // 5MB file limit
};
```

### Embedding Configuration

```rust
use sagitta_embed::EmbeddingConfig;

let config = EmbeddingConfig {
    model_type: EmbeddingModelType::Onnx,
    onnx_model_path: Some("model.onnx".into()),
    onnx_tokenizer_path: Some("tokenizer.json".into()),
    max_sessions: 4,
    max_sequence_length: 512,
    expected_dimension: Some(384),
    session_timeout_seconds: 300,
    enable_session_cleanup: true,
    tenant_id: None,
    embedding_batch_size: Some(128), // Configurable batch size
};
```

**Note**: CUDA acceleration is automatically enabled if the crate was compiled with CUDA support and compatible hardware is available. No runtime configuration needed.

## Performance Tuning

### CPU vs GPU Resource Balance

The architecture allows you to tune CPU and GPU usage independently:

- **High CPU, Low GPU Memory**: Increase `file_processing_concurrency`, keep `max_embedding_sessions` low
- **Balanced**: Use defaults (CPU cores for processing, 4 embedding sessions)  
- **GPU Heavy**: Increase `max_embedding_sessions` if you have abundant GPU memory

### Embedding Performance Settings

- **`max_sessions`**: Controls the number of parallel ONNX model instances and GPU memory usage. Higher values allow more parallel processing but use more GPU memory.
- **`embedding_batch_size`**: Controls the number of texts processed together by a single model instance. Higher values improve throughput per model but use more VRAM per model.
- **Interaction**: A single large operation will use up to `max_sessions` model instances in parallel, each processing `embedding_batch_size` texts at once.

### Batch Size Optimization

- **Small batches (32-64)**: Lower memory usage per model, higher overhead
- **Large batches (128-256)**: Better GPU utilization per model, higher memory usage per model
- **Recommended**: Start with 128, adjust based on your model and available VRAM

### File Processing Concurrency

- **Default**: Uses all CPU cores (`num_cpus::get()`)
- **Memory Limited**: Reduce if you have memory constraints
- **I/O Limited**: May not need all cores if disk/network is bottleneck

## Examples

See the `examples/` directory for complete examples:

- `processing.rs` - Modern processing architecture
- `concurrent_processing.rs` - Traditional concurrent embedding

## Features Flags

- `default = ["onnx"]` - ONNX provider enabled by default
- `onnx` - Enable ONNX Runtime support
- `cuda` - Enable CUDA acceleration (requires ONNX feature)

## Requirements

- Rust 1.70+
- ONNX Runtime (when using ONNX features)
- CUDA Toolkit (when using CUDA features)

## License

MIT License 