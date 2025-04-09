use crate::vectordb::error::VectorDBError;
use crate::vectordb::provider::{EmbeddingProvider, OnnxEmbeddingProvider};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// Use the embedding dimensions from the providers
// use crate::vectordb::provider::fast::FAST_EMBEDDING_DIM;

/// Supported embedding models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingModelType {
    /// Use the ONNX model for embeddings.
    Onnx,
    // No specific CodeBert type needed if we handle dimensions dynamically
}

impl std::fmt::Display for EmbeddingModelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingModelType::Onnx => write!(f, "ONNX"),
        }
    }
}

impl Default for EmbeddingModelType {
    fn default() -> Self {
        EmbeddingModelType::Onnx
    }
}

/// Model for generating embeddings from text
pub struct EmbeddingModel {
    provider: Box<dyn EmbeddingProvider + Send + Sync>,
    model_type: EmbeddingModelType,
    onnx_model_path: Option<PathBuf>,
    onnx_tokenizer_path: Option<PathBuf>,
}

impl Clone for EmbeddingModel {
    fn clone(&self) -> Self {
        // Re-create the provider using stored paths.
        match self.model_type {
            EmbeddingModelType::Onnx => {
                let model_path = self.onnx_model_path.as_ref()
                    .expect("Missing ONNX model path for cloning");
                let tokenizer_path = self.onnx_tokenizer_path.as_ref()
                    .expect("Missing ONNX tokenizer path for cloning");
                
                // Use expect here as cloning implies the original creation succeeded
                Self::new_onnx(model_path, tokenizer_path)
                    .expect("Failed to re-create ONNX model during clone")
            }
            // Add other types here if needed in the future
        }
    }
}

impl EmbeddingModel {
    /// Creates a new EmbeddingModel with the Fast provider
    /// This provider is much faster but less accurate than ONNX
    // pub fn new() -> Self {
    //     let provider = Box::new(FastTextProvider::new());
    //     Self {
    //         provider,
    //         model_type: EmbeddingModelType::Fast,
    //     }
    // }

    /// Creates a new EmbeddingModel with the ONNX provider
    /// This provider is more accurate but slower than Fast
    pub fn new_onnx(model_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        let provider = Box::new(OnnxEmbeddingProvider::new(model_path, tokenizer_path)?);
        Ok(Self {
            provider,
            model_type: EmbeddingModelType::Onnx,
            onnx_model_path: Some(model_path.to_path_buf()),
            onnx_tokenizer_path: Some(tokenizer_path.to_path_buf()),
        })
    }

    /// Convert text to an embedding vector
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, VectorDBError> {
        self.provider
            .embed(text)
            .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))
    }

    /// Convert multiple texts to embedding vectors
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, VectorDBError> {
        self.provider
            .embed_batch(texts)
            .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))
    }

    /// Get the dimension of the embeddings produced by this model
    pub fn dim(&self) -> usize {
        self.provider.dimension()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Removed test_fast_embedding
    // #[test]
    // fn test_fast_embedding() { ... }

    // Removed test_embedding_batch (it used the default FastText model)
    // #[test]
    // fn test_embedding_batch() { ... }

    // Keep test_onnx_embedding_fallback
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
        let expected_dim = model.dim(); // Get dimension from model

        // Test embedding
        let text = "fn main() { let x = 42; }";
        let embedding = model.embed(text).unwrap();

        assert_eq!(embedding.len(), expected_dim); // Check against model's dimension
        assert!(!embedding.iter().all(|&x| x == 0.0));

        // Test cloning
        let cloned_model = model.clone();
        assert_eq!(cloned_model.dim(), expected_dim);
        let cloned_embedding = cloned_model.embed(text).unwrap();
        assert_eq!(embedding, cloned_embedding);
    }

    // Removed test_model_cloning (it used the default FastText model)
    // #[test]
    // fn test_model_cloning() { ... }
}
