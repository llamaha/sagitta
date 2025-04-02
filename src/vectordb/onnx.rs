use anyhow::{Result, Error};
use std::path::Path;
use tokenizers::Tokenizer;
use ndarray::s;
use std::sync::Arc;

pub const ONNX_EMBEDDING_DIM: usize = 384;

/// OnnxEmbeddingModel provides a wrapper for the ONNX-based MiniLM embedding model
/// Note: This is currently a mock implementation that will be replaced with actual ONNX integration
#[derive(Clone)]
pub struct OnnxEmbeddingModel {
    tokenizer: Tokenizer,
    max_seq_length: usize,
    // We only store paths, but don't actually load the model yet
    model_path: Arc<Path>,
    tokenizer_path: Arc<Path>,
}

impl OnnxEmbeddingModel {
    /// Creates a new OnnxEmbeddingModel from the given model and tokenizer paths
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        // Only load the tokenizer for now
        let tokenizer_json_path = tokenizer_path.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(tokenizer_json_path)
            .map_err(|e| Error::msg(format!("Failed to load tokenizer: {}", e)))?;

        Ok(Self {
            tokenizer,
            max_seq_length: 128, // Default for MiniLM
            model_path: Arc::from(model_path),
            tokenizer_path: Arc::from(tokenizer_path),
        })
    }

    /// Tokenizes input text and returns tokens
    fn tokenize(&self, text: &str) -> Result<Vec<String>> {
        // Encode the text with the tokenizer
        let encoding = self.tokenizer
            .encode(text, true)
            .map_err(|e| Error::msg(format!("Failed to encode text with tokenizer: {}", e)))?;
        
        // Just get tokens as strings for the mock implementation
        let tokens = encoding.get_tokens().to_vec();
        
        Ok(tokens)
    }

    /// Generates mock embeddings (this will be replaced with actual ONNX inference)
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Tokenize the input text
        let tokens = self.tokenize(text)?;
        
        // Create a mock embedding based on token hashing
        let mut embedding = vec![0.0; ONNX_EMBEDDING_DIM];
        
        // Process tokens with simple hashing
        for (i, token) in tokens.iter().enumerate() {
            let weight = 1.0 - (i as f32 * 0.02).min(0.5); // Position weighting
            
            // Simple hash function for the token
            let mut hash = 0u64;
            for c in token.chars() {
                hash = hash.wrapping_mul(31).wrapping_add(c as u64);
            }
            
            // Distribute across embedding dimensions
            let primary_idx = (hash % ONNX_EMBEDDING_DIM as u64) as usize;
            embedding[primary_idx] += weight;
            
            // Also add to a few additional dimensions to create more complex embeddings
            let secondary_idx = ((hash >> 8) % ONNX_EMBEDDING_DIM as u64) as usize;
            embedding[secondary_idx] += weight * 0.5;
            
            let tertiary_idx = ((hash >> 16) % ONNX_EMBEDDING_DIM as u64) as usize;
            embedding[tertiary_idx] += weight * 0.3;
        }
        
        // Normalize the embedding
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }
        
        Ok(embedding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_onnx_embedding_creation() {
        // Skip test if the model files aren't available
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
        
        if !model_path.exists() || !tokenizer_path.exists() {
            println!("Skipping ONNX test because model files aren't available");
            return;
        }
        
        let model = OnnxEmbeddingModel::new(&model_path, &tokenizer_path);
        assert!(model.is_ok());
    }
    
    #[test]
    fn test_onnx_embedding() {
        // Skip test if the model files aren't available
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
        
        if !model_path.exists() || !tokenizer_path.exists() {
            println!("Skipping ONNX test because model files aren't available");
            return;
        }
        
        let model = OnnxEmbeddingModel::new(&model_path, &tokenizer_path).unwrap();
        let text = "fn main() { let x = 42; }";
        let embedding = model.embed(text).unwrap();
        
        assert_eq!(embedding.len(), ONNX_EMBEDDING_DIM);
        assert!(!embedding.iter().all(|&x| x == 0.0));
        
        // Generate a second embedding for the same text - should be identical
        let embedding2 = model.embed(text).unwrap();
        assert_eq!(embedding, embedding2);
        
        // Generate embedding for different text - should be different
        let different_text = "struct Point { x: i32, y: i32 }";
        let different_embedding = model.embed(different_text).unwrap();
        assert_ne!(embedding, different_embedding);
    }
} 