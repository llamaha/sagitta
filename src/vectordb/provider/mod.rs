use anyhow::Result;
use std::fmt::Debug;

/// Trait for embedding providers that convert text into vector representations
pub trait EmbeddingProvider: Debug + Send + Sync {
    /// Generate an embedding for the given text
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embeddings for multiple texts (batch processing)
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Default implementation calls embed() for each text
        texts.iter().map(|text| self.embed(text)).collect()
    }

    /// Get the dimension of the embeddings produced by this provider
    fn dimension(&self) -> usize;
}

// Module exports
pub mod onnx;
// pub mod fast; // Removed

// Re-export provider implementations
pub use onnx::OnnxEmbeddingProvider;
// pub use fast::FastEmbeddingProvider; // Removed

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to test provider implementations
    pub fn test_provider_basics<P: EmbeddingProvider>(provider: &P) {
        // Test embedding a single text
        let text = "fn main() { println!(\"Hello, world!\"); }";
        let embedding = provider.embed(text).unwrap();

        // Check normalization (roughly L2 normalized to 1.0)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "Embedding should be normalized");

        // Test batch embedding
        let texts = vec!["fn main() {}", "struct Point { x: i32, y: i32 }"];
        let embeddings = provider.embed_batch(&texts).unwrap();

        // Check batch results
        assert_eq!(embeddings.len(), 2);

        // Embeddings for different texts should be different
        assert_ne!(embeddings[0], embeddings[1]);
    }

    // Helper to normalize a vector - useful for testing provider implementations
    #[allow(dead_code)] // Allow dead code for now, might be used in tests later
    fn normalize(v: &mut [f32]) {
        let norm = v.iter().map(|&x| x * x).sum::<f32>().sqrt();
        if norm > 1e-6 {
            for x in v {
                *x /= norm;
            }
            // Re-enable the assertion to verify provider normalization
            let norm_after = v.iter().map(|&x| x * x).sum::<f32>().sqrt();
            assert!((norm_after - 1.0).abs() < 0.01, "Embedding should be normalized");
        }
    }
}
