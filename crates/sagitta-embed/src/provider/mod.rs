//! Embedding provider implementations and traits.

use crate::error::Result;
use crate::model::EmbeddingModelType;
use std::fmt::Debug;

/// A trait for embedding providers, defining a common interface for generating embeddings.
/// This allows for different underlying embedding model implementations (e.g., ONNX, SentenceTransformers).
// #[cfg_attr(test, mockall::automock)]
pub trait EmbeddingProvider: Send + Sync + Debug {
    /// Get the embedding dimension of the model.
    fn dimension(&self) -> usize;

    /// Get the type of the model.
    fn model_type(&self) -> EmbeddingModelType;

    /// Embed a batch of texts.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}

// Provider modules
#[cfg(feature = "onnx")]
pub mod onnx;
pub mod default;
pub mod factory;

// Re-export commonly used types
#[cfg(feature = "onnx")]
pub use onnx::{OnnxEmbeddingModel, OnnxSessionPool};
pub use default::DefaultEmbeddingProvider;
pub use factory::create_embedding_provider;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_provider_trait_object() {
        // Test that we can create trait objects
        let _provider: Option<Box<dyn EmbeddingProvider>> = None;
    }
} 