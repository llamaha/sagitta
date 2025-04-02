use anyhow::Result;
use std::path::Path;
use serde::{Serialize, Deserialize};
use crate::vectordb::provider::{
    EmbeddingProvider, 
    BasicEmbeddingProvider, 
    OnnxEmbeddingProvider
};

// Use the embedding dimensions from the providers
use crate::vectordb::provider::basic::BASIC_EMBEDDING_DIM;

// For backward compatibility 
pub const EMBEDDING_DIM: usize = BASIC_EMBEDDING_DIM;

/// The type of embedding model to use
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum EmbeddingModelType {
    /// Simple token-based embeddings (default)
    Basic,
    /// ONNX model-based embeddings
    Onnx,
}

impl Default for EmbeddingModelType {
    fn default() -> Self {
        EmbeddingModelType::Basic
    }
}

/// Model for generating embeddings from text
pub struct EmbeddingModel {
    provider: Box<dyn EmbeddingProvider + Send + Sync>,
    model_type: EmbeddingModelType,
}

impl Clone for EmbeddingModel {
    fn clone(&self) -> Self {
        // When cloning, create a new instance based on the current model type
        match self.model_type {
            EmbeddingModelType::Basic => {
                Self::new()
            },
            EmbeddingModelType::Onnx => {
                // For ONNX models, we can't clone directly
                // Log a warning and return a basic model
                eprintln!("Warning: ONNX model cannot be cloned. Creating a new basic model instead.");
                Self::new()
            }
        }
    }
}

impl EmbeddingModel {
    /// Creates a new EmbeddingModel with the Basic provider
    pub fn new() -> Self {
        let provider = Box::new(BasicEmbeddingProvider::new());
        
        Self {
            provider,
            model_type: EmbeddingModelType::Basic,
        }
    }
    
    /// Creates a new EmbeddingModel with the ONNX provider
    pub fn new_onnx(model_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        let provider = Box::new(OnnxEmbeddingProvider::new(model_path, tokenizer_path)?);
        
        Ok(Self {
            provider,
            model_type: EmbeddingModelType::Onnx,
        })
    }
    
    /// Get the model type
    pub fn model_type(&self) -> &EmbeddingModelType {
        &self.model_type
    }
    
    /// Get the embedding dimension
    pub fn embedding_dimension(&self) -> usize {
        self.provider.embedding_dimension()
    }
    
    /// Convert text to an embedding vector
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.provider.embed(text)
    }
    
    /// Convert multiple texts to embedding vectors
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.provider.embed_batch(texts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_embedding() {
        let model = EmbeddingModel::new();
        let text = "fn main() { let x = 42; }";
        let embedding = model.embed(text).unwrap();
        
        assert_eq!(embedding.len(), BASIC_EMBEDDING_DIM);
        assert!(!embedding.iter().all(|&x| x == 0.0));
        
        // Test that embeddings are deterministic
        let embedding2 = model.embed(text).unwrap();
        assert_eq!(embedding, embedding2);
    }
    
    #[test]
    fn test_embedding_batch() {
        let model = EmbeddingModel::new();
        let texts = vec!["fn main() {}", "struct Point { x: i32, y: i32 }"];
        let embeddings = model.embed_batch(&texts.iter().map(|s| *s).collect::<Vec<_>>()).unwrap();
        
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), BASIC_EMBEDDING_DIM);
        assert_eq!(embeddings[1].len(), BASIC_EMBEDDING_DIM);
        
        // Different texts should have different embeddings
        assert_ne!(embeddings[0], embeddings[1]);
    }
    
    #[test]
    fn test_onnx_embedding_fallback() {
        let model_path = Path::new("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = Path::new("onnx/minilm_tokenizer.json");
        
        // Skip test if ONNX files don't exist
        if !model_path.exists() || !tokenizer_path.exists() {
            println!("Skipping ONNX test because model files aren't available");
            return;
        }
        
        // Create ONNX model
        let onnx_model = EmbeddingModel::new_onnx(model_path, tokenizer_path);
        assert!(onnx_model.is_ok());
        
        let model = onnx_model.unwrap();
        assert_eq!(*model.model_type(), EmbeddingModelType::Onnx);
        
        // Test embedding
        let text = "fn main() { let x = 42; }";
        let embedding = model.embed(text).unwrap();
        
        assert_eq!(embedding.len(), EMBEDDING_DIM);
        assert!(!embedding.iter().all(|&x| x == 0.0));
    }
    
    #[test]
    fn test_model_cloning() {
        let model = EmbeddingModel::new();
        let cloned_model = model.clone();
        
        // Both models should produce the same embeddings
        let text = "fn main() {}";
        let embedding1 = model.embed(text).unwrap();
        let embedding2 = cloned_model.embed(text).unwrap();
        
        assert_eq!(embedding1, embedding2);
    }
} 