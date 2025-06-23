//! Dummy embedding provider for when no real embedding model is configured

use sagitta_embed::{provider::EmbeddingProvider, EmbeddingModelType, Result};
use std::fmt;

/// A dummy embedding provider that returns zero vectors
/// Used when no real embedding model is configured to allow the system to function
#[derive(Clone)]
pub struct DummyEmbeddingProvider {
    dimension: usize,
}

impl DummyEmbeddingProvider {
    /// Create a new dummy embedding provider with the specified dimension
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

impl fmt::Debug for DummyEmbeddingProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DummyEmbeddingProvider")
            .field("dimension", &self.dimension)
            .finish()
    }
}

impl EmbeddingProvider for DummyEmbeddingProvider {
    /// Returns the dimension of embeddings this provider generates
    fn dimension(&self) -> usize {
        self.dimension
    }

    /// Returns the model type - default for dummy provider
    fn model_type(&self) -> EmbeddingModelType {
        EmbeddingModelType::Default
    }

    /// Generate embeddings for the given texts - returns zero vectors
    fn embed_batch<'a>(&self, texts: &[&'a str]) -> Result<Vec<Vec<f32>>> {
        log::warn!("Using dummy embedding provider - returning zero vectors for {} texts", texts.len());
        
        // Return zero vectors of the appropriate dimension
        let embeddings = texts
            .iter()
            .map(|_| vec![0.0f32; self.dimension])
            .collect();
        
        Ok(embeddings)
    }
}