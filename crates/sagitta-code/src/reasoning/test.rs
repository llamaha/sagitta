use std::sync::Arc;
use sagitta_embed::provider::EmbeddingProvider;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;

/// Create a test embedding pool for testing
pub fn create_test_embedding_pool() -> Arc<dyn EmbeddingProvider> {
    Arc::new(MockEmbeddingProvider::new(384))
}

/// Create a test Qdrant client for testing
pub fn create_test_qdrant_client() -> Arc<dyn QdrantClientTrait> {
    // This would typically return a mock implementation
    // For now, return a placeholder that would need to be implemented
    unimplemented!("Test Qdrant client not implemented - tests will need to provide this")
}

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

    fn model_type(&self) -> sagitta_embed::EmbeddingModelType {
        sagitta_embed::EmbeddingModelType::Default
    }

    fn embed_batch(&self, texts: &[&str]) -> sagitta_embed::Result<Vec<Vec<f32>>> {
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