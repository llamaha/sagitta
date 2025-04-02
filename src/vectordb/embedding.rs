use anyhow::Result;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use crate::vectordb::provider::{
    EmbeddingProvider, 
    BasicEmbeddingProvider, 
    OnnxEmbeddingProvider
};

// Use the embedding dimensions from the providers
use crate::vectordb::provider::basic::BASIC_EMBEDDING_DIM;
use crate::vectordb::provider::onnx::ONNX_EMBEDDING_DIM;

// For backward compatibility 
pub const EMBEDDING_DIM: usize = BASIC_EMBEDDING_DIM;

/// The type of embedding model being used
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingModelType {
    /// Simple token-based embedding
    Basic,
    /// ONNX-based MiniLM embedding
    ONNX,
}

impl Default for EmbeddingModelType {
    fn default() -> Self {
        EmbeddingModelType::Basic
    }
}

/// The embedding model used to generate vector representations of text
pub struct EmbeddingModel {
    /// The underlying embedding provider
    provider: Box<dyn EmbeddingProvider>,
    /// The type of embedding model being used
    model_type: EmbeddingModelType,
}

impl Clone for EmbeddingModel {
    fn clone(&self) -> Self {
        // Create a new instance with the same model type
        // Needed since Box<dyn EmbeddingProvider> is not Clone
        match self.model_type {
            EmbeddingModelType::Basic => Self::new().unwrap(),
            EmbeddingModelType::ONNX => {
                // Just create a new basic model as fallback
                // since we can't access the private fields of OnnxEmbeddingProvider
                Self::new().unwrap()
            }
        }
    }
}

impl EmbeddingModel {
    /// Creates a new basic embedding model
    pub fn new() -> Result<Self> {
        let provider = BasicEmbeddingProvider::new()?;
        Ok(Self { 
            provider: Box::new(provider),
            model_type: EmbeddingModelType::Basic,
        })
    }
    
    /// Creates a new ONNX-based embedding model
    pub fn new_with_onnx(model_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        let provider = OnnxEmbeddingProvider::new(model_path, tokenizer_path)?;
        Ok(Self {
            provider: Box::new(provider),
            model_type: EmbeddingModelType::ONNX,
        })
    }
    
    /// Returns the current embedding model type
    pub fn model_type(&self) -> &EmbeddingModelType {
        &self.model_type
    }
    
    /// Returns the dimension of the generated embeddings
    pub fn embedding_dimension(&self) -> usize {
        self.provider.embedding_dimension()
    }
    
    /// Generate an embedding for the given text
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.provider.embed(text)
    }
    
    /// Generate embeddings for multiple texts
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.provider.embed_batch(texts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_embedding() {
        let model = EmbeddingModel::new().unwrap();
        let text = "fn main() { let x = 42; }";
        let embedding = model.embed(text).unwrap();
        
        assert_eq!(embedding.len(), EMBEDDING_DIM);
        assert!(!embedding.iter().all(|&x| x == 0.0));
        assert_eq!(*model.model_type(), EmbeddingModelType::Basic);
    }
    
    #[test]
    fn test_embedding_batch() {
        let model = EmbeddingModel::new().unwrap();
        let texts = vec!["fn main() {}", "struct Point { x: i32, y: i32 }"];
        let embeddings = model.embed_batch(&texts.iter().map(|s| *s).collect::<Vec<_>>()).unwrap();
        
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), EMBEDDING_DIM);
        assert_eq!(embeddings[1].len(), EMBEDDING_DIM);
        assert!(!embeddings[0].iter().all(|&x| x == 0.0));
        assert!(!embeddings[1].iter().all(|&x| x == 0.0));
    }
    
    #[test]
    fn test_onnx_embedding_fallback() {
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
        
        // Skip test if ONNX files don't exist
        if !model_path.exists() || !tokenizer_path.exists() {
            println!("Skipping ONNX fallback test because model files aren't available");
            return;
        }
        
        // Create ONNX model
        let onnx_model = EmbeddingModel::new_with_onnx(&model_path, &tokenizer_path);
        assert!(onnx_model.is_ok());
        
        let model = onnx_model.unwrap();
        assert_eq!(*model.model_type(), EmbeddingModelType::ONNX);
        
        // Test embedding
        let text = "fn main() { let x = 42; }";
        let embedding = model.embed(text).unwrap();
        
        assert_eq!(embedding.len(), ONNX_EMBEDDING_DIM);
        assert!(!embedding.iter().all(|&x| x == 0.0));
    }
    
    #[test]
    fn test_model_cloning() {
        let model = EmbeddingModel::new().unwrap();
        let cloned_model = model.clone();
        
        assert_eq!(*cloned_model.model_type(), *model.model_type());
        
        let text = "fn main() { let x = 42; }";
        let embedding1 = model.embed(text).unwrap();
        let embedding2 = cloned_model.embed(text).unwrap();
        
        // The embeddings should be identical for the same text
        assert_eq!(embedding1, embedding2);
    }
} 