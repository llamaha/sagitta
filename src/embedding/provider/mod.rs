#![allow(unused_variables)]
#![allow(dead_code)]

// Use types from within this crate's embedding module
use crate::embedding::EmbeddingModelType;
// Use error type from this crate
use crate::error::Result;

// Keep dependencies needed by this file (check Cargo.toml)
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::path::PathBuf;

// === New EmbeddingProvider Trait ===
/// A trait for embedding providers, defining a common interface for generating embeddings.
/// This allows for different underlying embedding model implementations (e.g., ONNX, SentenceTransformers).
#[cfg_attr(test, mockall::automock)]
pub trait EmbeddingProvider: Send + Sync + Debug {
    /// Get the embedding dimension of the model.
    fn dimension(&self) -> usize;

    /// Get the type of the model.
    fn model_type(&self) -> EmbeddingModelType;

    /// Embed a batch of texts.
    fn embed_batch<'a>(&self, texts: &[&'a str]) -> Result<Vec<Vec<f32>>>;
}

// === Provider Modules ===
/// Provides an ONNX-based implementation of the `EmbeddingProvider` trait.
pub mod onnx;
/// Provides a session pool for ONNX models to manage concurrent access.
pub mod session_pool;

// Re-export commonly used types
pub use onnx::OnnxEmbeddingModel;
pub use session_pool::OnnxSessionPool;

// === Config Structs ===
/// Configuration for an embedding model provider.
/// This struct holds information like the model type and paths to model files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingModelConfig {
    /// The type of the embedding model (e.g., ONNX, Default).
    pub model_type: EmbeddingModelType,
    /// Optional path to the ONNX model file.
    pub onnx_model_path: Option<PathBuf>,
    /// Optional path to the ONNX tokenizer configuration file or directory.
    pub onnx_tokenizer_path: Option<PathBuf>,
}

impl EmbeddingModelConfig {
    /// Creates a new `EmbeddingModelConfig` for the given `model_type`.
    /// ONNX paths are initialized to `None` and can be set later.
    pub fn new(model_type: EmbeddingModelType) -> Self {
        Self {
            model_type,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_embedding_model_config_new() {
        let config = EmbeddingModelConfig::new(EmbeddingModelType::Onnx);
        assert_eq!(config.model_type, EmbeddingModelType::Onnx);
        assert!(config.onnx_model_path.is_none());
        assert!(config.onnx_tokenizer_path.is_none());
    }

    #[test]
    fn test_embedding_model_config_with_paths() {
        let mut config = EmbeddingModelConfig::new(EmbeddingModelType::Default);
        let model_path = PathBuf::from("/path/to/model.onnx");
        let tokenizer_path = PathBuf::from("/path/to/tokenizer.json");
        config.onnx_model_path = Some(model_path.clone());
        config.onnx_tokenizer_path = Some(tokenizer_path.clone());
        assert_eq!(config.model_type, EmbeddingModelType::Default);
        assert_eq!(config.onnx_model_path, Some(model_path));
        assert_eq!(config.onnx_tokenizer_path, Some(tokenizer_path));
    }
} 