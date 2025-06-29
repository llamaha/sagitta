//! Default embedding provider for testing and fallback scenarios.

use crate::error::Result;
use crate::model::EmbeddingModelType;
use crate::provider::EmbeddingProvider;
use std::fmt::Debug;

/// A simple default embedding provider that generates deterministic embeddings.
/// This is used when no real model is available (e.g., in tests or when downloads fail).
#[derive(Debug, Clone)]
pub struct DefaultEmbeddingProvider {
    dimension: usize,
}

impl DefaultEmbeddingProvider {
    /// Create a new default embedding provider with the specified dimension.
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    /// Create a default provider with standard dimension (384).
    pub fn new_standard() -> Self {
        Self::new(384)
    }
}

impl EmbeddingProvider for DefaultEmbeddingProvider {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_type(&self) -> EmbeddingModelType {
        EmbeddingModelType::Default
    }

    fn embed_batch<'a>(&self, texts: &[&'a str]) -> Result<Vec<Vec<f32>>> {
        // Generate deterministic embeddings based on text hash
        texts.iter().map(|text| {
            let mut embedding = vec![0.0f32; self.dimension];
            
            // Simple deterministic embedding based on text content
            let hash = text.bytes().fold(1u32, |acc, b| acc.wrapping_add(b as u32).wrapping_mul(31));
            let base_value = ((hash % 100) + 1) as f32 / 100.0; // Ensure non-zero base value
            
            for (i, val) in embedding.iter_mut().enumerate() {
                // Create some variation across dimensions
                let dimension_factor = ((i + 1) as f32) / (self.dimension as f32);
                *val = base_value * dimension_factor * (if i % 2 == 0 { 1.0 } else { -1.0 });
            }
            
            // Normalize the embedding
            let norm = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for val in embedding.iter_mut() {
                    *val /= norm;
                }
            }
            
            Ok(embedding)
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_provider() {
        let provider = DefaultEmbeddingProvider::new_standard();
        assert_eq!(provider.dimension(), 384);
        assert_eq!(provider.model_type(), EmbeddingModelType::Default);
        
        let texts = vec!["hello", "world"];
        let embeddings = provider.embed_batch(&texts).unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 384);
        assert_eq!(embeddings[1].len(), 384);
        
        // Check that embeddings are normalized
        for embedding in &embeddings {
            let norm = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 0.001, "Embedding should be normalized");
        }
        
        // Check that different texts produce different embeddings
        assert_ne!(embeddings[0], embeddings[1]);
    }
    
    #[test]
    fn test_deterministic_embeddings() {
        let provider = DefaultEmbeddingProvider::new_standard();
        
        let texts = vec!["test"];
        let embeddings1 = provider.embed_batch(&texts).unwrap();
        let embeddings2 = provider.embed_batch(&texts).unwrap();
        
        // Same text should produce same embedding
        assert_eq!(embeddings1[0], embeddings2[0]);
    }
}