//! # Sagitta Embedding Engine
//!
//! A high-performance, modular embedding engine supporting multiple providers
//! and optimized for code search and semantic analysis.
//!
//! ## Features
//!
//! - **Multiple Providers**: Support for ONNX and other embedding providers
//! - **High Performance**: Optimized for concurrent access with session pooling
//! - **CUDA Support**: Optional GPU acceleration for ONNX models
//! - **Modular Design**: Clean separation of concerns with pluggable providers
//! - **Type Safety**: Comprehensive error handling and type safety
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use sagitta_embed::{EmbeddingHandler, EmbeddingConfig, EmbeddingModelType};
//! use std::path::PathBuf;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = EmbeddingConfig::new_onnx(
//!     PathBuf::from("model.onnx"),
//!     PathBuf::from("tokenizer.json")
//! );
//!
//! let handler = EmbeddingHandler::new(&config)?;
//! let embeddings = handler.embed(&["Hello world", "Rust programming"])?;
//! println!("Generated {} embeddings", embeddings.len());
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod config;
pub mod model;
pub mod provider;
pub mod handler;
pub mod utils;
pub mod processor;

// Re-export main types for convenience
pub use handler::EmbeddingHandler;
pub use model::{EmbeddingModel, EmbeddingModelType};
pub use provider::EmbeddingProvider;
pub use config::EmbeddingConfig;
pub use error::{SagittaEmbedError, Result};

// Re-export processor types for the new decoupled architecture
pub use processor::{
    FileProcessor, EmbeddingProcessor, ProcessedChunk, EmbeddedChunk,
    ChunkMetadata, ProcessingConfig, EmbeddingPool, DefaultFileProcessor, PoolStats
};

/// Current version of the sagitta-embed crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default embedding dimension for ONNX models
pub const DEFAULT_EMBEDDING_DIMENSION: usize = 384;

/// Default maximum number of concurrent sessions
pub const DEFAULT_MAX_SESSIONS: usize = 4;

/// Default maximum sequence length for tokenization
pub const DEFAULT_MAX_SEQUENCE_LENGTH: usize = 128; 