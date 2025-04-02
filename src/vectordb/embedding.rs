use anyhow::Result;
use crate::vectordb::tokenizer::CodeTokenizer;
use std::path::{Path, PathBuf};
use crate::vectordb::onnx::{OnnxEmbeddingModel, ONNX_EMBEDDING_DIM};
use serde::{Serialize, Deserialize};

pub const EMBEDDING_DIM: usize = 384;
const NGRAM_SIZE: usize = 3;
const POSITION_WEIGHT: f32 = 0.3;

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
#[derive(Clone)]
pub struct EmbeddingModel {
    /// The tokenizer for the basic embedding approach
    tokenizer: CodeTokenizer,
    /// Optional ONNX model for more sophisticated embeddings
    onnx_model: Option<OnnxEmbeddingModel>,
    /// The type of embedding model being used
    model_type: EmbeddingModelType,
}

impl EmbeddingModel {
    /// Creates a new basic embedding model
    pub fn new() -> Result<Self> {
        let tokenizer = CodeTokenizer::new()?;
        Ok(Self { 
            tokenizer,
            onnx_model: None,
            model_type: EmbeddingModelType::Basic,
        })
    }
    
    /// Creates a new ONNX-based embedding model
    pub fn new_with_onnx(model_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        let tokenizer = CodeTokenizer::new()?;
        let onnx_model = OnnxEmbeddingModel::new(model_path, tokenizer_path)?;
        
        Ok(Self {
            tokenizer,
            onnx_model: Some(onnx_model),
            model_type: EmbeddingModelType::ONNX,
        })
    }
    
    /// Returns the current embedding model type
    pub fn model_type(&self) -> &EmbeddingModelType {
        &self.model_type
    }

    fn generate_ngrams(text: &str) -> Vec<String> {
        let mut ngrams = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        
        for i in 0..chars.len().saturating_sub(NGRAM_SIZE - 1) {
            let ngram: String = chars[i..i + NGRAM_SIZE].iter().collect();
            ngrams.push(ngram);
        }
        
        ngrams
    }

    fn calculate_position_weight(position: usize, total: usize) -> f32 {
        let normalized_pos = position as f32 / total as f32;
        1.0 + (POSITION_WEIGHT * (1.0 - normalized_pos))
    }
    
    /// Generate an embedding for the given text
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        match (&self.model_type, &self.onnx_model) {
            // Use ONNX model if available and specified
            (EmbeddingModelType::ONNX, Some(onnx_model)) => {
                onnx_model.embed(text)
            },
            // Fallback to basic embedding if ONNX is not available
            _ => {
                self.basic_embed(text)
            }
        }
    }
    
    /// Generate a basic embedding using the simpler token-based approach
    fn basic_embed(&self, text: &str) -> Result<Vec<f32>> {
        let tokens = self.tokenizer.tokenize(text)?;
        let mut embedding = vec![0.0; EMBEDDING_DIM];
        
        // Process tokens with position weighting
        let total_tokens = tokens.len();
        for (pos, &token) in tokens.iter().enumerate() {
            let weight = Self::calculate_position_weight(pos, total_tokens);
            let idx = token as usize % EMBEDDING_DIM;
            embedding[idx] += weight;
        }
        
        // Process character n-grams
        let ngrams = Self::generate_ngrams(text);
        for ngram in ngrams {
            // Use a simple hash function for n-grams
            let mut hash: u64 = 0;
            for c in ngram.chars() {
                hash = hash.wrapping_mul(31).wrapping_add(c as u64);
            }
            let idx = (hash as usize) % EMBEDDING_DIM;
            embedding[idx] += 0.5; // Lower weight for n-grams
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

    #[test]
    fn test_embedding() {
        let model = EmbeddingModel::new().unwrap();
        let text = "fn main() { let x = 42; }";
        let embedding = model.embed(text).unwrap();
        
        assert_eq!(embedding.len(), EMBEDDING_DIM);
        assert!(!embedding.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_ngram_generation() {
        let text = "hello";
        let ngrams = EmbeddingModel::generate_ngrams(text);
        assert!(!ngrams.is_empty());
        assert!(ngrams.contains(&"hel".to_string()));
        assert!(ngrams.contains(&"ell".to_string()));
        assert!(ngrams.contains(&"llo".to_string()));
    }

    #[test]
    fn test_position_weighting() {
        let weight_start = EmbeddingModel::calculate_position_weight(0, 10);
        let weight_end = EmbeddingModel::calculate_position_weight(9, 10);
        assert!(weight_start > weight_end);
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
} 