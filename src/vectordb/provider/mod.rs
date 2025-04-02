use anyhow::Result;

/// Trait for embedding providers that convert text into vector representations
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding for the given text
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
    
    /// Generate embeddings for multiple texts (batch processing)
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Default implementation calls embed() for each text
        texts.iter().map(|text| self.embed(text)).collect()
    }
    
    /// Return the dimension of the generated embeddings
    fn embedding_dimension(&self) -> usize;
    
    /// Return the name of the provider
    fn name(&self) -> &'static str;
    
    /// Return a description of the provider
    fn description(&self) -> &'static str;
}

// Module exports
pub mod basic;
pub mod onnx;

// Re-export provider implementations
pub use basic::BasicEmbeddingProvider;
pub use onnx::OnnxEmbeddingProvider;

#[cfg(test)]
mod tests {
    use super::*;
    
    // Helper function to test provider implementations
    pub fn test_provider_basics<P: EmbeddingProvider>(provider: &P) {
        // Test embedding a single text
        let text = "fn main() { println!(\"Hello, world!\"); }";
        let embedding = provider.embed(text).unwrap();
        
        // Check dimension
        assert_eq!(embedding.len(), provider.embedding_dimension());
        
        // Check normalization (roughly L2 normalized to 1.0)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "Embedding should be normalized");
        
        // Test batch embedding
        let texts = vec!["fn main() {}", "struct Point { x: i32, y: i32 }"];
        let embeddings = provider.embed_batch(&texts).unwrap();
        
        // Check batch results
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), provider.embedding_dimension());
        assert_eq!(embeddings[1].len(), provider.embedding_dimension());
        
        // Embeddings for different texts should be different
        assert_ne!(embeddings[0], embeddings[1]);
    }
} 