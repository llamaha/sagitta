use anyhow::{Result, Error};
use tokenizers::Tokenizer;
use std::sync::{Arc, Mutex};
use std::path::Path;
use std::time::{Duration, Instant};
use std::num::NonZeroUsize;
use lru::LruCache;

/// Tokenization result including input IDs and attention mask
#[derive(Clone, Debug)]
pub struct TokenizerOutput {
    /// Input IDs for the model
    pub input_ids: Vec<i64>,
    /// Attention mask for the model
    pub attention_mask: Vec<i64>,
    /// When this result was generated
    created_at: Instant,
}

/// Configuration for the tokenizer cache
#[derive(Clone, Debug)]
pub struct TokenizerCacheConfig {
    /// Maximum number of entries to keep in the cache
    pub max_cache_size: usize,
    /// Maximum length of text to cache (longer texts won't be cached)
    pub max_text_length: usize,
    /// Time after which cached entries expire
    pub cache_ttl: Duration,
    /// Maximum sequence length for the model
    pub max_seq_length: usize,
}

impl Default for TokenizerCacheConfig {
    fn default() -> Self {
        Self {
            max_cache_size: 1000,
            max_text_length: 200,
            cache_ttl: Duration::from_secs(300), // 5 minutes
            max_seq_length: 128, // Default for MiniLM
        }
    }
}

/// Cache for tokenization results to avoid repeated tokenization of the same text
pub struct TokenizerCache {
    /// The underlying tokenizer
    tokenizer: Tokenizer,
    /// Cache of tokenization results
    cache: Mutex<LruCache<String, TokenizerOutput>>,
    /// Configuration for the cache
    config: TokenizerCacheConfig,
}

impl TokenizerCache {
    /// Create a new tokenizer cache
    pub fn new(tokenizer_path: &Path, config: TokenizerCacheConfig) -> Result<Arc<Self>> {
        // Load the tokenizer from the file
        let tokenizer_json_path = tokenizer_path.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(tokenizer_json_path)
            .map_err(|e| Error::msg(format!("Failed to load tokenizer: {}", e)))?;
        
        // Create the LRU cache with NonZeroUsize capacity
        let capacity = NonZeroUsize::new(config.max_cache_size)
            .unwrap_or(NonZeroUsize::new(1).unwrap());
        
        Ok(Arc::new(Self {
            tokenizer,
            cache: Mutex::new(LruCache::new(capacity)),
            config,
        }))
    }
    
    /// Tokenize text, using the cache if available
    pub fn tokenize(&self, text: &str) -> Result<TokenizerOutput> {
        // Check if the text is too long to cache
        if text.len() > self.config.max_text_length {
            // Skip the cache and tokenize directly
            return self.tokenize_uncached(text);
        }
        
        // Try to get the result from the cache
        let cache_key = text.to_string();
        
        // Lock the cache for reading and potential writing
        let mut cache = self.cache.lock().unwrap();
        
        // Check if we have a cached result
        if let Some(cached) = cache.get(&cache_key) {
            // Check if the cache entry has expired
            if cached.created_at.elapsed() < self.config.cache_ttl {
                // Return a clone of the cached result
                return Ok(cached.clone());
            }
            // Entry has expired, remove it and continue
            cache.pop(&cache_key);
        }
        
        // No valid cached result, tokenize and cache the result
        let output = self.tokenize_uncached(text)?;
        
        // Cache the result
        cache.put(cache_key, output.clone());
        
        Ok(output)
    }
    
    /// Tokenize text without using the cache
    fn tokenize_uncached(&self, text: &str) -> Result<TokenizerOutput> {
        // Encode the text with the tokenizer
        let encoding = self.tokenizer
            .encode(text, true)
            .map_err(|e| Error::msg(format!("Failed to encode text with tokenizer: {}", e)))?;
        
        // Get input IDs and attention mask
        let mut input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mut attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&mask| mask as i64).collect();
        
        // Truncate or pad to the maximum sequence length
        let max_seq_length = self.config.max_seq_length;
        if input_ids.len() > max_seq_length {
            // Truncate
            input_ids.truncate(max_seq_length);
            attention_mask.truncate(max_seq_length);
        } else if input_ids.len() < max_seq_length {
            // Pad
            let pad_length = max_seq_length - input_ids.len();
            input_ids.extend(vec![0; pad_length]);
            attention_mask.extend(vec![0; pad_length]);
        }
        
        Ok(TokenizerOutput {
            input_ids,
            attention_mask,
            created_at: Instant::now(),
        })
    }
    
    /// Clear the cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }
    
    /// Get the current cache hit rate (for monitoring)
    pub fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.lock().unwrap();
        (cache.len(), cache.cap().get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::thread;
    
    #[test]
    fn test_tokenizer_creation() {
        // Skip if tokenizer isn't available
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
        if !tokenizer_path.exists() {
            println!("Skipping test_tokenizer_creation because tokenizer file isn't available");
            return;
        }
        
        // Create a tokenizer cache with default config
        let config = TokenizerCacheConfig::default();
        let cache = TokenizerCache::new(&tokenizer_path.parent().unwrap(), config);
        assert!(cache.is_ok());
    }
    
    #[test]
    fn test_tokenization() {
        // Skip if tokenizer isn't available
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
        if !tokenizer_path.exists() {
            println!("Skipping test_tokenization because tokenizer file isn't available");
            return;
        }
        
        // Create a tokenizer cache with default config
        let config = TokenizerCacheConfig::default();
        let expected_seq_length = config.max_seq_length; // Store the value before moving config
        
        let cache = TokenizerCache::new(&tokenizer_path.parent().unwrap(), config).unwrap();
        
        // Tokenize some text
        let text = "Hello, world!";
        let output = cache.tokenize(text);
        assert!(output.is_ok());
        
        let output = output.unwrap();
        assert_eq!(output.input_ids.len(), expected_seq_length);
        assert_eq!(output.attention_mask.len(), expected_seq_length);
    }
    
    #[test]
    fn test_cache_hit() {
        // Skip if tokenizer isn't available
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
        if !tokenizer_path.exists() {
            println!("Skipping test_cache_hit because tokenizer file isn't available");
            return;
        }
        
        // Create a tokenizer cache with default config
        let config = TokenizerCacheConfig::default();
        let cache = TokenizerCache::new(&tokenizer_path.parent().unwrap(), config).unwrap();
        
        // Tokenize the same text twice
        let text = "Hello, world!";
        let output1 = cache.tokenize(text).unwrap();
        let output2 = cache.tokenize(text).unwrap();
        
        // The cache should have been hit for the second call
        assert_eq!(output1.input_ids, output2.input_ids);
        assert_eq!(output1.attention_mask, output2.attention_mask);
        
        // Check cache stats
        let (len, _) = cache.cache_stats();
        assert_eq!(len, 1);
    }
    
    #[test]
    fn test_cache_expiry() {
        // Skip if tokenizer isn't available
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
        if !tokenizer_path.exists() {
            println!("Skipping test_cache_expiry because tokenizer file isn't available");
            return;
        }
        
        // Create a tokenizer cache with a very short TTL
        let mut config = TokenizerCacheConfig::default();
        config.cache_ttl = Duration::from_millis(10);
        let cache = TokenizerCache::new(&tokenizer_path.parent().unwrap(), config).unwrap();
        
        // Tokenize some text
        let text = "Hello, world!";
        let _ = cache.tokenize(text).unwrap();
        
        // Wait for the cache entry to expire
        thread::sleep(Duration::from_millis(20));
        
        // Tokenize again - this should not be a hit
        let _ = cache.tokenize(text).unwrap();
        
        // Check cache stats - should still be 1 entry (the second one)
        let (len, _) = cache.cache_stats();
        assert_eq!(len, 1);
    }
} 