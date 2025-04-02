use anyhow::{Result, Error};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use ndarray::{Array2, CowArray};
use ort::Value;
use crate::vectordb::provider::session_manager::SessionManager;
use crate::vectordb::provider::tokenizer_cache::{TokenizerCache, TokenizerOutput};

/// Configuration for the batch processor
#[derive(Clone, Debug)]
pub struct BatchProcessorConfig {
    /// Maximum batch size for processing
    pub max_batch_size: usize,
    /// Maximum time to wait for a batch to fill
    pub max_wait_time: Duration,
    /// Whether to use dynamic batching based on sequence length
    pub dynamic_batching: bool,
}

impl Default for BatchProcessorConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 16,
            max_wait_time: Duration::from_millis(50),
            dynamic_batching: true,
        }
    }
}

/// A request for embedding a text
struct EmbeddingRequest {
    /// The tokenized input
    tokenized: TokenizerOutput,
    /// When the request was created
    created_at: Instant,
    /// Channel to send the result back to the caller
    result_sender: std::sync::mpsc::Sender<Result<Vec<f32>>>,
}

/// Batch processor for embedding multiple texts efficiently
pub struct BatchProcessor {
    /// Queue of pending embedding requests
    queue: Mutex<VecDeque<EmbeddingRequest>>,
    /// Session manager for ONNX runtime
    session_manager: Arc<SessionManager>,
    /// Tokenizer cache for processing inputs
    tokenizer_cache: Arc<TokenizerCache>,
    /// Configuration for the batch processor
    config: BatchProcessorConfig,
    /// Dimension of the embeddings
    embedding_dim: usize,
    /// Whether the processor is running
    running: Mutex<bool>,
}

impl BatchProcessor {
    /// Create a new batch processor
    pub fn new(
        session_manager: Arc<SessionManager>,
        tokenizer_cache: Arc<TokenizerCache>,
        config: BatchProcessorConfig,
        embedding_dim: usize,
    ) -> Arc<Self> {
        let processor = Arc::new(Self {
            queue: Mutex::new(VecDeque::new()),
            session_manager,
            tokenizer_cache,
            config,
            embedding_dim,
            running: Mutex::new(false),
        });
        
        // Start the background processing thread
        let processor_clone = Arc::clone(&processor);
        std::thread::spawn(move || {
            processor_clone.process_batches();
        });
        
        processor
    }
    
    /// Process batches in the background
    fn process_batches(&self) {
        // Set the running flag
        *self.running.lock().unwrap() = true;
        
        // Process batches until stopped
        while *self.running.lock().unwrap() {
            // Try to process a batch
            match self.process_next_batch() {
                Ok(processed) => {
                    if !processed {
                        // No batch to process, sleep for a bit
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
                Err(e) => {
                    eprintln!("Error processing batch: {}", e);
                    // Sleep to avoid spinning on errors
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }
    
    /// Process the next batch of requests
    fn process_next_batch(&self) -> Result<bool> {
        // Get a batch of requests
        let batch = self.get_next_batch();
        if batch.is_empty() {
            return Ok(false);
        }
        
        // Process the batch
        let results = self.process_batch(&batch)?;
        
        // Send results back to the callers
        for (request, result) in batch.into_iter().zip(results) {
            let _ = request.result_sender.send(result);
        }
        
        Ok(true)
    }
    
    /// Get the next batch of requests from the queue
    fn get_next_batch(&self) -> Vec<EmbeddingRequest> {
        let mut queue = self.queue.lock().unwrap();
        let mut batch = Vec::new();
        let max_batch_size = self.config.max_batch_size;
        
        // Get up to max_batch_size requests
        while !queue.is_empty() && batch.len() < max_batch_size {
            batch.push(queue.pop_front().unwrap());
        }
        
        batch
    }
    
    /// Process a batch of requests
    fn process_batch(&self, batch: &[EmbeddingRequest]) -> Result<Vec<Result<Vec<f32>>>> {
        if batch.is_empty() {
            return Ok(Vec::new());
        }
        
        // Get a session
        let session_guard = self.session_manager.get_session_guard()?;
        let session = session_guard.session();
        
        // Prepare input tensors
        let batch_size = batch.len();
        let seq_length = batch[0].tokenized.input_ids.len();
        
        // Create tensors for input_ids and attention_mask
        let mut input_ids: Vec<i64> = Vec::with_capacity(batch_size * seq_length);
        let mut attention_mask: Vec<i64> = Vec::with_capacity(batch_size * seq_length);
        
        // Collect inputs from all requests
        for request in batch {
            input_ids.extend(&request.tokenized.input_ids);
            attention_mask.extend(&request.tokenized.attention_mask);
        }
        
        // Create 2D arrays
        let input_ids_array = Array2::from_shape_vec((batch_size, seq_length), input_ids)?;
        let attention_mask_array = Array2::from_shape_vec((batch_size, seq_length), attention_mask)?;
        
        // Convert to dynamic arrays
        let input_ids_dyn = input_ids_array.into_dyn();
        let attention_mask_dyn = attention_mask_array.into_dyn();
        
        // Create CowArray (needed for ONNX runtime)
        let input_ids_cow = CowArray::from(&input_ids_dyn);
        let attention_mask_cow = CowArray::from(&attention_mask_dyn);
        
        // Create input values
        let input_ids_val = Value::from_array(session.allocator(), &input_ids_cow);
        let attention_mask_val = Value::from_array(session.allocator(), &attention_mask_cow);
        
        let inputs = match (input_ids_val, attention_mask_val) {
            (Ok(input_ids), Ok(attention_mask)) => {
                vec![input_ids, attention_mask]
            },
            _ => return Err(Error::msg("Failed to create input tensors")),
        };
        
        // Run inference
        let outputs = session.run(inputs)?;
        
        // Extract pooler output (second output tensor)
        let pooler_output = outputs[1].try_extract()?;
        let pooler_view = pooler_output.view();
        
        // Extract individual embeddings
        let mut results = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            // Get the embedding for this item
            let embedding = pooler_view.slice(ndarray::s![i, ..])
                .as_slice()
                .map(|slice| slice.to_vec())
                .unwrap_or_else(|| vec![0.0; self.embedding_dim]);
            
            // Normalize the embedding
            let embedding = Self::normalize_embedding(embedding);
            
            results.push(Ok(embedding));
        }
        
        Ok(results)
    }
    
    /// Normalize an embedding to unit length
    fn normalize_embedding(mut embedding: Vec<f32>) -> Vec<f32> {
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }
        embedding
    }
    
    /// Queue a text for embedding
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Tokenize the text
        let tokenized = self.tokenizer_cache.tokenize(text)?;
        
        // Create a channel for the result
        let (sender, receiver) = std::sync::mpsc::channel();
        
        // Create a request
        let request = EmbeddingRequest {
            tokenized,
            created_at: Instant::now(),
            result_sender: sender,
        };
        
        // Add the request to the queue
        {
            let mut queue = self.queue.lock().unwrap();
            queue.push_back(request);
        }
        
        // Wait for the result
        receiver.recv().unwrap_or_else(|_| {
            Err(Error::msg("Failed to receive embedding result"))
        })
    }
    
    /// Embed multiple texts
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Process each text
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text)?);
        }
        
        Ok(results)
    }
    
    /// Stop the batch processor
    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }
}

impl Drop for BatchProcessor {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::vectordb::provider::session_manager::SessionConfig;
    use crate::vectordb::provider::tokenizer_cache::TokenizerCacheConfig;
    
    #[test]
    fn test_batch_processor_creation() {
        // Skip if model/tokenizer aren't available
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
        
        if !model_path.exists() || !tokenizer_path.exists() {
            println!("Skipping test_batch_processor_creation because model/tokenizer files aren't available");
            return;
        }
        
        // Create session manager
        let session_config = SessionConfig::default();
        let session_manager = SessionManager::new(&model_path, session_config).unwrap();
        
        // Create tokenizer cache
        let tokenizer_config = TokenizerCacheConfig::default();
        let tokenizer_cache = TokenizerCache::new(&tokenizer_path.parent().unwrap(), tokenizer_config).unwrap();
        
        // Create batch processor
        let batch_config = BatchProcessorConfig::default();
        let processor = BatchProcessor::new(
            session_manager,
            tokenizer_cache,
            batch_config,
            384, // MiniLM dimension
        );
        
        // Just check that we got a valid Arc back
        assert!(Arc::strong_count(&processor) >= 1);
    }
    
    #[test]
    fn test_embed_single() {
        // Skip if model/tokenizer aren't available
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
        
        if !model_path.exists() || !tokenizer_path.exists() {
            println!("Skipping test_embed_single because model/tokenizer files aren't available");
            return;
        }
        
        // Create session manager
        let session_config = SessionConfig::default();
        let session_manager = SessionManager::new(&model_path, session_config).unwrap();
        
        // Create tokenizer cache
        let tokenizer_config = TokenizerCacheConfig::default();
        let tokenizer_cache = TokenizerCache::new(&tokenizer_path.parent().unwrap(), tokenizer_config).unwrap();
        
        // Create batch processor
        let batch_config = BatchProcessorConfig::default();
        let processor = BatchProcessor::new(
            session_manager,
            tokenizer_cache,
            batch_config,
            384, // MiniLM dimension
        );
        
        // Embed a single text
        let text = "Hello, world!";
        let embedding = processor.embed(text);
        assert!(embedding.is_ok());
        
        let embedding = embedding.unwrap();
        assert_eq!(embedding.len(), 384);
        
        // Check normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }
    
    #[test]
    fn test_embed_batch() {
        // Skip if model/tokenizer aren't available
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
        
        if !model_path.exists() || !tokenizer_path.exists() {
            println!("Skipping test_embed_batch because model/tokenizer files aren't available");
            return;
        }
        
        // Create session manager
        let session_config = SessionConfig::default();
        let session_manager = SessionManager::new(&model_path, session_config).unwrap();
        
        // Create tokenizer cache
        let tokenizer_config = TokenizerCacheConfig::default();
        let tokenizer_cache = TokenizerCache::new(&tokenizer_path.parent().unwrap(), tokenizer_config).unwrap();
        
        // Create batch processor
        let batch_config = BatchProcessorConfig::default();
        let processor = BatchProcessor::new(
            session_manager,
            tokenizer_cache,
            batch_config,
            384, // MiniLM dimension
        );
        
        // Embed multiple texts
        let texts = vec!["Hello, world!", "How are you?", "I'm fine, thank you."];
        let embeddings = processor.embed_batch(&texts);
        assert!(embeddings.is_ok());
        
        let embeddings = embeddings.unwrap();
        assert_eq!(embeddings.len(), 3);
        
        // Check that all embeddings have the correct dimension
        for embedding in &embeddings {
            assert_eq!(embedding.len(), 384);
            
            // Check normalization
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 0.01);
        }
        
        // Check that the embeddings are different
        assert_ne!(embeddings[0], embeddings[1]);
        assert_ne!(embeddings[1], embeddings[2]);
        assert_ne!(embeddings[0], embeddings[2]);
    }
} 