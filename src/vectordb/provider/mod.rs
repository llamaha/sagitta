use anyhow::Result;
use std::path::Path;

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
pub mod session_manager;
pub mod tokenizer_cache;
pub mod batch_processor;

// Re-export provider implementations
pub use basic::BasicEmbeddingProvider;
pub use onnx::OnnxEmbeddingProvider;
pub use onnx::OptimizedOnnxEmbeddingProvider;

/// A struct to hold all the components needed for optimized embeddings
pub struct OptimizedEmbeddingComponents {
    /// The session manager for pooling ONNX sessions
    pub session_manager: std::sync::Arc<session_manager::SessionManager>,
    /// The tokenizer cache for caching tokenization results
    pub tokenizer_cache: std::sync::Arc<tokenizer_cache::TokenizerCache>,
    /// The batch processor for efficient batch processing
    pub batch_processor: std::sync::Arc<batch_processor::BatchProcessor>,
    /// The optimized embedding provider
    pub provider: OptimizedOnnxEmbeddingProvider,
}

/// Create all components needed for optimized ONNX embeddings
pub fn create_optimized_components(
    model_path: &Path,
    tokenizer_path: &Path,
    embedding_dim: usize,
) -> Result<OptimizedEmbeddingComponents> {
    // Create session manager with default config
    let session_config = session_manager::SessionConfig::default();
    let session_manager = session_manager::SessionManager::new(model_path, session_config)?;
    
    // Create tokenizer cache with default config
    let tokenizer_config = tokenizer_cache::TokenizerCacheConfig::default();
    let tokenizer_cache = tokenizer_cache::TokenizerCache::new(tokenizer_path, tokenizer_config)?;
    
    // Create batch processor with default config
    let batch_config = batch_processor::BatchProcessorConfig::default();
    let batch_processor = batch_processor::BatchProcessor::new(
        session_manager.clone(),
        tokenizer_cache.clone(),
        batch_config,
        embedding_dim,
    );
    
    // Create optimized provider
    let provider = OptimizedOnnxEmbeddingProvider::new_with_defaults(model_path, tokenizer_path)?;
    
    Ok(OptimizedEmbeddingComponents {
        session_manager,
        tokenizer_cache,
        batch_processor,
        provider,
    })
}

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
    
    #[test]
    fn test_create_optimized_components() {
        // Skip if model/tokenizer aren't available
        let model_path = std::path::PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = std::path::PathBuf::from("onnx/minilm_tokenizer.json");
        
        if !model_path.exists() || !tokenizer_path.parent().unwrap().exists() {
            println!("Skipping test_create_optimized_components because model/tokenizer files aren't available");
            return;
        }
        
        // Create components
        let components = create_optimized_components(
            &model_path,
            tokenizer_path.parent().unwrap(),
            onnx::ONNX_EMBEDDING_DIM,
        );
        
        assert!(components.is_ok());
        
        // Test the provider
        let components = components.unwrap();
        test_provider_basics(&components.provider);
    }
} 