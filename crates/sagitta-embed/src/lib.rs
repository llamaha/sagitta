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
//! use sagitta_embed::{EmbeddingPool, EmbeddingConfig};
//! use std::path::PathBuf;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = EmbeddingConfig::new_onnx(
//!     PathBuf::from("model.onnx"),
//!     PathBuf::from("tokenizer.json")
//! );
//!
//! let pool = EmbeddingPool::with_configured_sessions(config)?;
//! let embeddings = pool.embed_texts_async(&["Hello world", "Rust programming"]).await?;
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

/// Default constants for configuration values
/// Default embedding dimension for validation
pub const DEFAULT_EMBEDDING_DIMENSION: usize = 384;

/// Default session timeout in seconds (5 minutes)
pub const DEFAULT_SESSION_TIMEOUT_SECONDS: u64 = 300;

/// Default value for enabling session cleanup
pub const DEFAULT_ENABLE_SESSION_CLEANUP: bool = true;

/// Default embedding batch size for processing multiple texts
pub const DEFAULT_EMBEDDING_BATCH_SIZE: usize = 8;  // Optimized default for release 