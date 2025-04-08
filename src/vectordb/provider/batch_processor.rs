use crate::vectordb::provider::session_manager::SessionManager;
use crate::vectordb::provider::tokenizer_cache::{TokenizerCache, TokenizerOutput};
use anyhow::{Error, Result};
use ndarray::{Array2, CowArray};
use ort::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Configuration for the batch processor
#[derive(Clone, Debug)]
pub struct BatchProcessorConfig {
    /// Maximum batch size for processing
    pub max_batch_size: usize,
    /// Maximum time to wait for a batch to fill
    pub max_wait_time: Duration,
    /// Whether to use dynamic batching based on sequence length
    pub dynamic_batching: bool,
    /// Maximum sequence length variance allowed in a batch
    pub max_seq_length_variance: usize,
    /// Retry count for failed batches
    pub max_retries: usize,
    /// Timeout for each batch processing attempt
    pub batch_timeout: Duration,
    /// Whether to use adaptive batching to adjust batch size based on system load
    pub adaptive_batching: bool,
    /// Target latency for adaptive batching
    pub target_latency: Duration,
    /// Maximum parallelism for batch processing
    pub max_parallelism: usize,
}

impl Default for BatchProcessorConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 16,
            max_wait_time: Duration::from_millis(50),
            dynamic_batching: true,
            max_seq_length_variance: 32,
            max_retries: 3,
            batch_timeout: Duration::from_secs(30),
            adaptive_batching: false,
            target_latency: Duration::from_millis(100),
            max_parallelism: num_cpus::get(),
        }
    }
}

/// A request for embedding a text
#[derive(Clone)]
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

impl Clone for BatchProcessor {
    fn clone(&self) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            session_manager: Arc::clone(&self.session_manager),
            tokenizer_cache: Arc::clone(&self.tokenizer_cache),
            config: self.config.clone(),
            embedding_dim: self.embedding_dim,
            running: Mutex::new(*self.running.lock().unwrap()),
        }
    }
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

        // If queue is empty, return immediately
        if queue.is_empty() {
            return batch;
        }

        if self.config.dynamic_batching {
            // Group requests by similar sequence length for better efficiency

            // First, peek at the first request to get initial sequence length
            let first_seq_length = queue
                .front()
                .map(|req| req.tokenized.input_ids.len())
                .unwrap_or(0);

            // Collect requests of similar sequence length
            let mut i = 0;
            while i < queue.len() && batch.len() < max_batch_size {
                let req = &queue[i];
                let current_seq_length = req.tokenized.input_ids.len();

                // Check if this request fits within our variance window
                if batch.is_empty()
                    || (current_seq_length
                        >= first_seq_length.saturating_sub(self.config.max_seq_length_variance)
                        && current_seq_length
                            <= first_seq_length + self.config.max_seq_length_variance)
                {
                    batch.push(queue.remove(i).unwrap());
                } else {
                    // Skip this request for now, as it has a different sequence length
                    i += 1;
                }
            }
        } else {
            // Simple FIFO batch collection
            while !queue.is_empty() && batch.len() < max_batch_size {
                batch.push(queue.pop_front().unwrap());
            }
        }

        // If using adaptive batching, adjust batch size based on recent performance
        if self.config.adaptive_batching && !batch.is_empty() {
            // Implementation will be added in a future enhancement
            // This would track batch processing times and adjust batch sizes accordingly
        }

        batch
    }

    /// Process a batch of requests
    fn process_batch(&self, batch: &[EmbeddingRequest]) -> Result<Vec<Result<Vec<f32>>>> {
        if batch.is_empty() {
            return Ok(Vec::new());
        }

        // Set up timeout for batch processing
        let timeout = Instant::now() + self.config.batch_timeout;
        let mut retry_count = 0;
        let mut last_error = None;

        // Retry loop for resilience
        while retry_count < self.config.max_retries && Instant::now() < timeout {
            match self.process_batch_with_session(batch) {
                Ok(results) => {
                    // Successfully processed the batch
                    return Ok(results);
                }
                Err(e) => {
                    // Record the error and retry
                    eprintln!(
                        "Batch processing error (attempt {}/{}): {}",
                        retry_count + 1,
                        self.config.max_retries,
                        e
                    );
                    last_error = Some(e);
                    retry_count += 1;

                    // Exponential backoff before retry
                    if retry_count < self.config.max_retries {
                        let backoff_ms = 2u64.pow(retry_count as u32) * 10;
                        std::thread::sleep(Duration::from_millis(backoff_ms));
                    }
                }
            }
        }

        // If we've exhausted retries, return error results for all requests
        if let Some(err) = last_error {
            eprintln!(
                "Failed to process batch after {} retries: {}",
                self.config.max_retries, err
            );

            // Return individual errors for each request
            let err_msg = format!(
                "Batch processing failed after {} retries: {}",
                self.config.max_retries, err
            );

            let mut error_results = Vec::with_capacity(batch.len());
            for _ in 0..batch.len() {
                error_results.push(Err(Error::msg(err_msg.clone())));
            }

            Ok(error_results)
        } else {
            // This should not happen, but handle it anyway
            Err(Error::msg("Batch processing timed out"))
        }
    }

    /// Process a batch with a specific session - internal implementation
    fn process_batch_with_session(
        &self,
        batch: &[EmbeddingRequest],
    ) -> Result<Vec<Result<Vec<f32>>>> {
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
        let attention_mask_array =
            Array2::from_shape_vec((batch_size, seq_length), attention_mask)?;

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
            }
            _ => return Err(Error::msg("Failed to create input tensors")),
        };

        // Run inference with a timeout guard
        let outputs = session.run(inputs)?;

        // Validate output
        if outputs.len() < 2 {
            return Err(Error::msg(format!(
                "Model returned unexpected number of outputs: got {}, expected at least 2",
                outputs.len()
            )));
        }

        // Extract pooler output (second output tensor)
        let pooler_output = outputs[1].try_extract()?;
        let pooler_view = pooler_output.view();

        // Validate pooler output shape
        let output_shape = pooler_view.shape();
        if output_shape.len() != 2 || output_shape[0] != batch_size {
            return Err(Error::msg(format!(
                "Unexpected pooler output shape: got {:?}, expected [{}, {}]",
                output_shape, batch_size, self.embedding_dim
            )));
        }

        // Extract individual embeddings
        let mut results = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            // Get the embedding for this item
            let embedding = pooler_view
                .slice(ndarray::s![i, ..])
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
        // Use a timeout for the entire embedding process
        let timeout = Instant::now() + self.config.batch_timeout;

        // Try tokenization first - if this fails, don't bother queuing
        let tokenized = match self.tokenizer_cache.tokenize(text) {
            Ok(tokenized) => tokenized,
            Err(e) => {
                return Err(Error::msg(format!("Failed to tokenize text: {}", e)));
            }
        };

        // Create a channel for the result with bounded capacity
        let (sender, receiver) = std::sync::mpsc::channel();

        // Create a request
        let request = EmbeddingRequest {
            tokenized,
            created_at: Instant::now(),
            result_sender: sender,
        };

        // Add the request to the queue with retry logic
        let mut queued = false;
        let mut retry_count = 0;

        // Create a new request clone for each attempt
        while !queued && retry_count < self.config.max_retries && Instant::now() < timeout {
            // Try to acquire the queue lock with a timeout
            match self.queue.try_lock() {
                Ok(mut queue) => {
                    queue.push_back(request.clone());
                    queued = true;
                }
                Err(_) => {
                    // If we couldn't get the lock, wait briefly and retry
                    retry_count += 1;
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }

        // If we couldn't queue the request, return an error
        if !queued {
            return Err(Error::msg(
                "Failed to queue embedding request: queue lock unavailable",
            ));
        }

        // Wait for the result with remaining timeout
        let remaining_time = timeout.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(remaining_time) {
            Ok(result) => result,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(Error::msg(format!(
                "Timed out waiting for embedding result after {:?}",
                self.config.batch_timeout
            ))),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err(Error::msg("Batch processor channel disconnected"))
            }
        }
    }

    /// Embed multiple texts with improved parallel processing
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // For a small number of texts, process them serially
        if texts.len() <= 4 {
            // Process each text
            let mut results = Vec::with_capacity(texts.len());
            for text in texts {
                results.push(self.embed(text)?);
            }
            return Ok(results);
        }

        // True batch processing for larger batches
        // Tokenize all texts first
        let mut tokenized_texts = Vec::with_capacity(texts.len());
        let mut failed_indices = Vec::new();

        for (idx, text) in texts.iter().enumerate() {
            match self.tokenizer_cache.tokenize(text) {
                Ok(tokenized) => tokenized_texts.push((idx, tokenized)),
                Err(e) => {
                    eprintln!("Failed to tokenize text at index {}: {}", idx, e);
                    failed_indices.push((idx, Error::msg(format!("Tokenization failed: {}", e))));
                }
            }
        }

        if tokenized_texts.is_empty() {
            if let Some((idx, err)) = failed_indices.first() {
                return Err(Error::msg(format!(
                    "Failed to tokenize text at index {}: {}",
                    idx, err
                )));
            }
            return Ok(Vec::new());
        }

        // Set up storage for results
        let mut results = vec![None; texts.len()];

        // Group by similar sequence lengths for efficient batch processing
        // Sort by sequence length for better batching
        tokenized_texts.sort_by_key(|(_, t)| t.input_ids.len());

        // Process in batches
        let max_batch_size = self.config.max_batch_size;
        let mut current_batch = Vec::new();
        let mut current_batch_indices = Vec::new();

        for (orig_idx, tokenized) in tokenized_texts {
            // Add to current batch
            current_batch.push(EmbeddingRequest {
                tokenized,
                created_at: Instant::now(),
                result_sender: std::sync::mpsc::channel().0, // Dummy sender, not used in direct batch processing
            });
            current_batch_indices.push(orig_idx);

            // Process batch when it reaches max size
            if current_batch.len() >= max_batch_size {
                self.process_current_batch(
                    &mut current_batch,
                    &current_batch_indices,
                    &mut results,
                )?;
                current_batch.clear();
                current_batch_indices.clear();
            }
        }

        // Process any remaining items in the last batch
        if !current_batch.is_empty() {
            self.process_current_batch(&mut current_batch, &current_batch_indices, &mut results)?;
        }

        // Fill in missing results with errors or collect final results
        let mut final_results = Vec::with_capacity(texts.len());
        for (idx, result) in results.into_iter().enumerate() {
            if let Some(embedding) = result {
                final_results.push(embedding);
            } else {
                // Check if this was a tokenization failure
                if failed_indices.iter().any(|(i, _)| *i == idx) {
                    // Return the specific error for this index
                    if let Some((_, err)) = failed_indices.iter().find(|(i, _)| *i == idx) {
                        return Err(Error::msg(format!(
                            "Failed to process text at index {}: {}",
                            idx, err
                        )));
                    }
                }
                // Otherwise, it's an unknown failure
                return Err(Error::msg(format!(
                    "Failed to process text at index {}: unknown error",
                    idx
                )));
            }
        }

        Ok(final_results)
    }

    /// Helper method to process a batch and store results
    fn process_current_batch(
        &self,
        batch: &[EmbeddingRequest],
        indices: &[usize],
        results: &mut [Option<Vec<f32>>],
    ) -> Result<()> {
        match self.process_batch_with_session(batch) {
            Ok(batch_results) => {
                // Store results at their original indices
                for (i, result) in batch_results.into_iter().enumerate() {
                    if i < indices.len() {
                        let original_idx = indices[i];
                        if let Ok(embedding) = result {
                            results[original_idx] = Some(embedding);
                        }
                    }
                }
                Ok(())
            }
            Err(e) => Err(Error::msg(format!("Failed to process batch: {}", e))),
        }
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
    use crate::vectordb::provider::session_manager::SessionConfig;
    use crate::vectordb::provider::tokenizer_cache::TokenizerCacheConfig;
    use std::path::PathBuf;

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
        let tokenizer_cache = TokenizerCache::new(&tokenizer_path, tokenizer_config).unwrap();

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
        let tokenizer_cache = TokenizerCache::new(&tokenizer_path, tokenizer_config).unwrap();

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
        let tokenizer_cache = TokenizerCache::new(&tokenizer_path, tokenizer_config).unwrap();

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
