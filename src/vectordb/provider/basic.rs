use anyhow::Result;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use crate::vectordb::provider::EmbeddingProvider;

/// Dimension of the basic embeddings (position-weighted token hashes)
pub const BASIC_EMBEDDING_DIM: usize = 384;

/// Simple embedding provider using token hashes with position weighting
pub struct BasicEmbeddingProvider {
    /// Cached trigram hashes for common tokens
    trigram_cache: HashMap<String, u64>,
}

impl BasicEmbeddingProvider {
    /// Create a new BasicEmbeddingProvider
    pub fn new() -> Self {
        Self {
            trigram_cache: HashMap::new(),
        }
    }
    
    /// Extract n-grams from a string
    fn extract_ngrams(&self, text: &str, n: usize) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() < n {
            return vec![text.to_string()];
        }
        
        let mut ngrams = Vec::with_capacity(chars.len() - n + 1);
        for i in 0..=(chars.len() - n) {
            let ngram: String = chars[i..(i + n)].iter().collect();
            ngrams.push(ngram);
        }
        
        ngrams
    }
    
    /// Hash a string to a u64 value
    fn hash_string(&mut self, s: &str) -> u64 {
        if let Some(&hash) = self.trigram_cache.get(s) {
            return hash;
        }
        
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        let hash = hasher.finish();
        
        // Cache the hash for future use
        if s.len() == 3 {
            self.trigram_cache.insert(s.to_string(), hash);
        }
        
        hash
    }
}

impl EmbeddingProvider for BasicEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut provider = self.clone();
        
        // Normalize the text
        let text = text.to_lowercase();
        
        // Extract character trigrams
        let trigrams = provider.extract_ngrams(&text, 3);
        
        // Initialize embedding vector
        let mut embedding = vec![0.0; BASIC_EMBEDDING_DIM];
        
        // Generate embedding based on trigram hashes with position weighting
        for (i, trigram) in trigrams.iter().enumerate() {
            let hash = provider.hash_string(trigram);
            let position_weight = 1.0 - (i as f32 / trigrams.len() as f32) * 0.5; // Weight ranges from 0.5 to 1.0
            
            // Distribute the weighted hash across multiple dimensions
            for j in 0..3 {
                let index = ((hash >> (j * 16)) % BASIC_EMBEDDING_DIM as u64) as usize;
                embedding[index] += position_weight;
            }
        }
        
        // Normalize the embedding to unit length
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
        "Basic-Trigram"
    }
    
    fn description(&self) -> &'static str {
        "Basic embedding using character trigrams with position weighting"
    }
}

impl Clone for BasicEmbeddingProvider {
    fn clone(&self) -> Self {
        Self {
            trigram_cache: self.trigram_cache.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectordb::provider::tests::test_provider_basics;
    
    #[test]
    fn test_basic_provider() {
        let provider = BasicEmbeddingProvider::new();
        test_provider_basics(&provider);
    }
    
    #[test]
    fn test_deterministic_embeddings() {
        let provider = BasicEmbeddingProvider::new();
        let text = "fn main() { println!(\"Hello, world!\"); }";
        
        let embedding1 = provider.embed(text).unwrap();
        let embedding2 = provider.embed(text).unwrap();
        
        // Embeddings for the same text should be identical
        assert_eq!(embedding1, embedding2);
    }
    
    #[test]
    fn test_similar_texts() {
        let provider = BasicEmbeddingProvider::new();
        let text1 = "fn calculate_sum(a: i32, b: i32) -> i32 { a + b }";
        let text2 = "fn calculate_sum(a: i32, b: i32) -> i32 { return a + b; }";
        let text3 = "struct Point { x: i32, y: i32 }";
        
        let embedding1 = provider.embed(text1).unwrap();
        let embedding2 = provider.embed(text2).unwrap();
        let embedding3 = provider.embed(text3).unwrap();
        
        // Calculate cosine similarity
        fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
            let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
            // Vectors should already be normalized to length 1, so dot product = cosine similarity
            dot_product
        }
        
        // Similar texts should have high similarity
        let sim_1_2 = cosine_similarity(&embedding1, &embedding2);
        // Different texts should have lower similarity
        let sim_1_3 = cosine_similarity(&embedding1, &embedding3);
        
        assert!(sim_1_2 > 0.8, "Similar texts should have high similarity: {}", sim_1_2);
        assert!(sim_1_3 < 0.8, "Different texts should have lower similarity: {}", sim_1_3);
    }
} 