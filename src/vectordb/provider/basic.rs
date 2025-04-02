use anyhow::Result;
use crate::vectordb::provider::EmbeddingProvider;
use crate::vectordb::tokenizer::CodeTokenizer;

/// Dimension of the basic embedding vectors
pub const BASIC_EMBEDDING_DIM: usize = 384;
const NGRAM_SIZE: usize = 3;
const POSITION_WEIGHT: f32 = 0.3;

/// Basic embedding provider using token-based approach
pub struct BasicEmbeddingProvider {
    /// The tokenizer for processing text
    tokenizer: CodeTokenizer,
}

impl BasicEmbeddingProvider {
    /// Creates a new basic embedding provider
    pub fn new() -> Result<Self> {
        let tokenizer = CodeTokenizer::new()?;
        Ok(Self { tokenizer })
    }
    
    /// Generates n-grams from the text
    fn generate_ngrams(text: &str) -> Vec<String> {
        let mut ngrams = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        
        for i in 0..chars.len().saturating_sub(NGRAM_SIZE - 1) {
            let ngram: String = chars[i..i + NGRAM_SIZE].iter().collect();
            ngrams.push(ngram);
        }
        
        ngrams
    }
    
    /// Calculates position weight for tokens
    fn calculate_position_weight(position: usize, total: usize) -> f32 {
        let normalized_pos = position as f32 / total as f32;
        1.0 + (POSITION_WEIGHT * (1.0 - normalized_pos))
    }
}

impl EmbeddingProvider for BasicEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let tokens = self.tokenizer.tokenize(text)?;
        let mut embedding = vec![0.0; BASIC_EMBEDDING_DIM];
        
        // Process tokens with position weighting
        let total_tokens = tokens.len();
        for (pos, &token) in tokens.iter().enumerate() {
            let weight = Self::calculate_position_weight(pos, total_tokens);
            let idx = token as usize % BASIC_EMBEDDING_DIM;
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
            let idx = (hash as usize) % BASIC_EMBEDDING_DIM;
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
    
    fn embedding_dimension(&self) -> usize {
        BASIC_EMBEDDING_DIM
    }
    
    fn name(&self) -> &'static str {
        "Basic"
    }
    
    fn description(&self) -> &'static str {
        "Token-based embedding with n-gram features and position weighting"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectordb::provider::tests::test_provider_basics;
    
    #[test]
    fn test_basic_provider() {
        let provider = BasicEmbeddingProvider::new().unwrap();
        test_provider_basics(&provider);
    }
    
    #[test]
    fn test_ngram_generation() {
        let text = "hello";
        let ngrams = BasicEmbeddingProvider::generate_ngrams(text);
        assert!(!ngrams.is_empty());
        assert!(ngrams.contains(&"hel".to_string()));
        assert!(ngrams.contains(&"ell".to_string()));
        assert!(ngrams.contains(&"llo".to_string()));
    }
    
    #[test]
    fn test_position_weighting() {
        let weight_start = BasicEmbeddingProvider::calculate_position_weight(0, 10);
        let weight_end = BasicEmbeddingProvider::calculate_position_weight(9, 10);
        assert!(weight_start > weight_end);
    }
} 