use std::sync::Arc;
use sagitta_embed::provider::EmbeddingProvider;
use sagitta_embed::EmbeddingModelType;
use sagitta_embed::{Result, SagittaEmbedError};

/// Mock embedding provider for testing
#[derive(Debug, Clone)]
pub struct MockEmbeddingProvider {
    dimension: usize,
}

impl MockEmbeddingProvider {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

impl EmbeddingProvider for MockEmbeddingProvider {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_type(&self) -> EmbeddingModelType {
        EmbeddingModelType::Default
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Return mock embeddings - just simple patterns for testing
        let embeddings = texts.iter()
            .map(|_| {
                (0..self.dimension)
                    .map(|i| (i as f32) / (self.dimension as f32)) // Simple pattern
                    .collect()
            })
            .collect();
        Ok(embeddings)
    }
}

/// ThreadSafe wrapper for MockEmbeddingProvider
#[derive(Debug, Clone)]
pub struct ThreadSafeMockProvider {
    inner: Arc<MockEmbeddingProvider>,
}

impl ThreadSafeMockProvider {
    pub fn new(dimension: usize) -> Self {
        Self {
            inner: Arc::new(MockEmbeddingProvider::new(dimension)),
        }
    }
}

impl EmbeddingProvider for ThreadSafeMockProvider {
    fn dimension(&self) -> usize {
        self.inner.dimension()
    }

    fn model_type(&self) -> EmbeddingModelType {
        self.inner.model_type()
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.inner.embed_batch(texts)
    }
} 