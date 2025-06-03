# Sagitta-Embed

A high-performance, thread-safe embedding generation library for the Sagitta ecosystem. This crate provides a unified interface for generating text embeddings using various models, with optimized support for ONNX models and session pooling.

## Features

- **Multiple Model Support**: ONNX models with extensible architecture for future model types
- **Thread-Safe Operations**: Concurrent embedding generation with session pooling
- **High Performance**: Optimized ONNX runtime with CUDA support
- **Batch Processing**: Efficient batch embedding generation
- **Session Management**: Automatic session pooling and lifecycle management
- **Error Handling**: Comprehensive error types with detailed context
- **Configuration-Driven**: Flexible configuration system

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
sagitta-embed = { path = "../sagitta-embed" }
```

### Basic Usage

```rust
use sagitta_embed::{EmbeddingHandler, EmbeddingConfig};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure for ONNX model
    let model_path = Path::new("path/to/model.onnx");
    let tokenizer_path = Path::new("path/to/tokenizer.json");
    
    let config = EmbeddingConfig::new_onnx(model_path, tokenizer_path);
    
    // Create embedding handler
    let handler = EmbeddingHandler::new(&config)?;
    
    // Generate embeddings
    let texts = vec!["Hello world", "How are you?"];
    let embeddings = handler.embed_batch(&texts)?;
    
    println!("Generated {} embeddings", embeddings.len());
    for (i, embedding) in embeddings.iter().enumerate() {
        println!("Text: '{}' -> Embedding dimension: {}", texts[i], embedding.len());
    }
    
    Ok(())
}
```

### Advanced Configuration

```rust
use sagitta_embed::{EmbeddingHandler, EmbeddingConfig, EmbeddingModelType};

let config = EmbeddingConfig {
    model_type: EmbeddingModelType::Onnx,
    onnx_model_path: Some("path/to/model.onnx".into()),
    onnx_tokenizer_path: Some("path/to/tokenizer.json".into()),
    max_sessions: 4,
    enable_cuda: true,
    max_batch_size: 32,
    normalize_embeddings: true,
    cache_size: 1000,
};

let handler = EmbeddingHandler::new(&config)?;
```

## Architecture

### Core Components

- **`EmbeddingHandler`**: Main interface for embedding generation
- **`EmbeddingProvider`**: Trait for different embedding model implementations
- **`OnnxEmbeddingModel`**: ONNX-specific implementation with session pooling
- **`SessionPool`**: Thread-safe session management for ONNX models
- **`EmbeddingConfig`**: Configuration structure for all embedding settings

### Model Types

Currently supported:
- **ONNX Models**: Optimized for performance with session pooling
- **Default**: Fallback implementation

Planned:
- **OpenAI API**: Remote embedding generation
- **HuggingFace**: Local transformer models
- **Custom**: User-defined embedding functions

### Session Pooling

The ONNX provider uses an intelligent session pool that:
- Maintains multiple ONNX runtime sessions for concurrent processing
- Automatically scales based on demand
- Handles session lifecycle and cleanup
- Provides thread-safe access to sessions
- Supports CUDA acceleration when available

## Configuration Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `model_type` | `EmbeddingModelType` | `Onnx` | Type of embedding model to use |
| `onnx_model_path` | `Option<PathBuf>` | `None` | Path to ONNX model file |
| `onnx_tokenizer_path` | `Option<PathBuf>` | `None` | Path to tokenizer JSON file |
| `max_sessions` | `usize` | `2` | Maximum number of ONNX sessions in pool |
| `enable_cuda` | `bool` | `false` | Enable CUDA acceleration |
| `max_batch_size` | `usize` | `16` | Maximum batch size for processing |
| `normalize_embeddings` | `bool` | `true` | Whether to normalize output embeddings |
| `cache_size` | `usize` | `0` | Size of embedding cache (0 = disabled) |

## Error Handling

The crate provides comprehensive error handling through the `SagittaEmbedError` enum:

```rust
use sagitta_embed::{Result, SagittaEmbedError};

match handler.embed_batch(&texts) {
    Ok(embeddings) => println!("Success: {} embeddings", embeddings.len()),
    Err(SagittaEmbedError::ModelNotFound(path)) => {
        eprintln!("Model file not found: {}", path);
    },
    Err(SagittaEmbedError::TokenizerError(msg)) => {
        eprintln!("Tokenizer error: {}", msg);
    },
    Err(SagittaEmbedError::OnnxError(msg)) => {
        eprintln!("ONNX runtime error: {}", msg);
    },
    Err(e) => eprintln!("Other error: {}", e),
}
```

## Performance Considerations

### Batch Processing
- Use batch processing for multiple texts to maximize throughput
- Optimal batch sizes depend on model size and available memory
- The library automatically handles batching within configured limits

### Session Pooling
- Configure `max_sessions` based on your concurrency needs
- More sessions = higher memory usage but better concurrency
- Monitor session utilization in high-load scenarios

### CUDA Acceleration
- Enable CUDA for significant performance improvements on compatible hardware
- Ensure CUDA toolkit and compatible ONNX runtime are installed
- CUDA sessions require more memory but provide faster inference

### Memory Management
- Sessions are automatically cleaned up when the handler is dropped
- Consider embedding cache for frequently used texts
- Monitor memory usage with large models or high session counts

## Examples

### Concurrent Processing

```rust
use sagitta_embed::{EmbeddingHandler, EmbeddingConfig};
use std::sync::Arc;
use tokio::task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = EmbeddingConfig::new_onnx("model.onnx", "tokenizer.json");
    let handler = Arc::new(EmbeddingHandler::new(&config)?);
    
    let mut handles = vec![];
    
    for i in 0..10 {
        let handler_clone = handler.clone();
        let handle = task::spawn(async move {
            let texts = vec![format!("Text batch {}", i)];
            handler_clone.embed_batch(&texts)
        });
        handles.push(handle);
    }
    
    for handle in handles {
        let result = handle.await??;
        println!("Batch completed: {} embeddings", result.len());
    }
    
    Ok(())
}
```

### Custom Error Handling

```rust
use sagitta_embed::{EmbeddingHandler, EmbeddingConfig, SagittaEmbedError};

fn handle_embedding_error(error: SagittaEmbedError) {
    match error {
        SagittaEmbedError::ModelNotFound(path) => {
            eprintln!("Please check that the model file exists: {}", path);
        },
        SagittaEmbedError::InsufficientMemory => {
            eprintln!("Not enough memory. Try reducing batch size or max_sessions.");
        },
        SagittaEmbedError::TokenizerError(msg) => {
            eprintln!("Tokenizer issue: {}. Check tokenizer file format.", msg);
        },
        _ => eprintln!("Unexpected error: {}", error),
    }
}
```

## Integration with Sagitta Search

This crate is designed to integrate seamlessly with the broader Sagitta ecosystem:

```rust
// In sagitta-search
use sagitta_embed::{EmbeddingHandler, EmbeddingConfig};

let embedding_config = EmbeddingConfig::new_onnx(
    &app_config.embedding.model_path,
    &app_config.embedding.tokenizer_path,
);

let embedding_handler = Arc::new(EmbeddingHandler::new(&embedding_config)?);

// Use in search operations
let query_embedding = embedding_handler.embed_batch(&[query])?;
```

## Development

### Building

```bash
cargo build --release
```

### Testing

```bash
cargo test
```

### Running Examples

```bash
cargo run --example basic_usage
cargo run --example concurrent_processing
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Changelog

### v0.1.0
- Initial release with ONNX model support
- Session pooling implementation
- Thread-safe embedding generation
- Comprehensive error handling
- Integration with sagitta-search ecosystem 