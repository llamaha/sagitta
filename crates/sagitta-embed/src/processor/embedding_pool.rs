//! Enhanced embedding pool implementation that uses trait-based providers.

use crate::error::{Result, SagittaEmbedError};
use crate::processor::{EmbeddingProcessor, ProcessedChunk, EmbeddedChunk, ProcessingConfig, ChunkMetadata};
use crate::provider::{EmbeddingProvider, create_embedding_provider};
use crate::config::EmbeddingConfig;
use std::sync::Arc;
use tokio::sync::{Semaphore, Mutex, mpsc};
use std::time::Instant;

/// Pool of embedding model instances with controlled GPU memory usage and optimized CPU threading.
/// This version uses trait-based providers to support different model types.
#[derive(Debug)]
pub struct EmbeddingPool {
    /// Configuration for the pool
    config: ProcessingConfig,
    /// Embedding configuration
    embedding_config: EmbeddingConfig,
    /// Semaphore to control concurrent access to embedding models
    semaphore: Arc<Semaphore>,
    /// Pool of embedding provider instances
    providers: Arc<Mutex<Vec<Arc<dyn EmbeddingProvider>>>>,
    /// Embedding dimension (cached for performance)
    dimension: usize,
    /// Number of CPU worker threads for GPU coordination
    cpu_worker_threads: usize,
}

impl EmbeddingPool {
    /// Create a new embedding pool with the given configurations.
    pub fn new(config: ProcessingConfig, embedding_config: EmbeddingConfig) -> Result<Self> {
        // Validate embedding config
        embedding_config.validate()?;
        
        // Get dimension from config
        let dimension = embedding_config.get_embedding_dimension();

        // Create the pool
        let semaphore = Arc::new(Semaphore::new(config.max_embedding_sessions));
        let providers = Arc::new(Mutex::new(Vec::new()));

        // Calculate optimal number of CPU worker threads
        let cpu_worker_threads = config.effective_cpu_worker_threads();

        log::info!("EmbeddingPool: Configured with {} sessions and {} CPU worker threads", 
                   config.max_embedding_sessions, cpu_worker_threads);

        Ok(Self {
            config,
            embedding_config,
            semaphore,
            providers,
            dimension,
            cpu_worker_threads,
        })
    }

    /// Create a new embedding pool with default processing config.
    pub fn with_embedding_config(embedding_config: EmbeddingConfig) -> Result<Self> {
        Self::new(ProcessingConfig::default(), embedding_config)
    }

    /// Create a new embedding pool that properly uses the max_sessions from the EmbeddingConfig.
    pub fn with_configured_sessions(embedding_config: EmbeddingConfig) -> Result<Self> {
        let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
        Self::new(processing_config, embedding_config)
    }

    /// Simple async function to embed raw text strings.
    pub async fn embed_texts_async(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Convert texts to ProcessedChunks
        let chunks: Vec<ProcessedChunk> = texts.iter().enumerate().map(|(i, text)| {
            ProcessedChunk {
                content: text.to_string(),
                metadata: ChunkMetadata {
                    file_path: std::path::PathBuf::from("text_input"),
                    start_line: 0,
                    end_line: 0,
                    language: "text".to_string(),
                    file_extension: "txt".to_string(),
                    element_type: "text".to_string(),
                    context: None,
                },
                id: format!("text_{i}"),
            }
        }).collect();

        // Process chunks and extract embeddings
        let embedded_chunks = self.process_chunks(chunks).await?;
        let embeddings = embedded_chunks.into_iter()
            .map(|chunk| chunk.embedding)
            .collect();

        Ok(embeddings)
    }

    /// Process multiple chunks in parallel using worker threads.
    async fn process_chunks_parallel(
        &self, 
        chunks: Vec<ProcessedChunk>,
        progress_reporter: Arc<dyn crate::processor::ProgressReporter>
    ) -> Result<Vec<EmbeddedChunk>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        let total_chunks = chunks.len();
        let chunk_size = self.config.embedding_batch_size;
        let total_batches = total_chunks.div_ceil(chunk_size);
        let start_time = Instant::now();

        log::info!("Processing {} chunks in {} batches using {} CPU workers", 
                   total_chunks, total_batches, self.cpu_worker_threads);

        // Create work queue
        let (batch_sender, batch_receiver) = mpsc::channel::<Vec<ProcessedChunk>>(total_batches);
        let (result_sender, mut result_receiver) = mpsc::channel::<Result<Vec<EmbeddedChunk>>>(total_batches);

        // Send batches to work queue
        for batch in chunks.chunks(chunk_size) {
            if let Err(_) = batch_sender.send(batch.to_vec()).await {
                return Err(SagittaEmbedError::thread_safety("Failed to queue batch for processing".to_string()));
            }
        }
        drop(batch_sender); // Close the queue

        // Wrap the receiver in Arc<Mutex> to share among workers
        let shared_batch_receiver = Arc::new(Mutex::new(batch_receiver));

        // Spawn worker tasks
        let mut worker_handles = Vec::new();
        for worker_id in 0..self.cpu_worker_threads {
            let batch_receiver = Arc::clone(&shared_batch_receiver);
            let sender = result_sender.clone();
            let semaphore = Arc::clone(&self.semaphore);
            let providers = Arc::clone(&self.providers);
            let embedding_config = self.embedding_config.clone();
            let max_sessions = self.config.max_embedding_sessions;

            let handle = tokio::spawn(async move {
                let mut batches_processed = 0;
                
                loop {
                    // Try to get work from the shared receiver
                    let batch = {
                        let mut receiver = batch_receiver.lock().await;
                        receiver.recv().await
                    };

                    match batch {
                        Some(batch) => {
                            log::debug!("Worker {} processing batch of {} chunks", worker_id, batch.len());
                            
                            // Process the batch
                            let result = Self::process_batch_worker(
                                batch,
                                &semaphore,
                                &providers,
                                &embedding_config,
                                max_sessions
                            ).await;
                            
                            if let Err(ref e) = result {
                                log::error!("Worker {worker_id} failed to process batch: {e}");
                            }
                            
                            // Always try to send result, even if it's an error
                            if sender.send(result).await.is_err() {
                                log::error!("Worker {worker_id} failed to send result");
                                break;
                            }
                            
                            batches_processed += 1;
                        }
                        None => {
                            // Channel closed, no more work
                            break;
                        }
                    }
                }
                
                log::debug!("Worker {worker_id} finished, processed {batches_processed} batches");
            });
            
            worker_handles.push(handle);
        }

        drop(result_sender); // Close the result channel

        // Collect results with improved error handling
        let mut all_embedded_chunks = Vec::with_capacity(total_chunks);
        let mut batches_received = 0;
        let mut chunks_processed = 0;
        let mut any_errors = Vec::new();

        while let Some(result) = result_receiver.recv().await {
            match result {
                Ok(embedded_batch) => {
                    chunks_processed += embedded_batch.len();
                    all_embedded_chunks.extend(embedded_batch);
                    batches_received += 1;
                    
                    // Report progress after each batch completion
                    let elapsed_secs = start_time.elapsed().as_secs_f64();
                    let chunks_per_second = if elapsed_secs > 0.0 { 
                        Some(chunks_processed as f64 / elapsed_secs) 
                    } else { 
                        None 
                    };
                    
                    progress_reporter.report(crate::processor::ProcessingProgress {
                        stage: crate::processor::ProcessingStage::GeneratingEmbeddings,
                        current_file: None,
                        files_completed: chunks_processed,
                        total_files: total_chunks,
                        files_per_second: chunks_per_second,
                        message: Some(format!("Processed batch {}/{} - {} chunks completed ({:.1} chunks/sec)", 
                                              batches_received, total_batches, chunks_processed,
                                              chunks_per_second.unwrap_or(0.0))),
                    }).await;
                    
                    log::debug!("Received batch {batches_received}/{total_batches} - {chunks_processed} chunks processed");
                }
                Err(e) => {
                    log::warn!("Batch processing failed (will continue with other batches): {e}");
                    any_errors.push(e);
                    batches_received += 1;
                }
            }
        }

        // Wait for all workers to complete gracefully
        for (i, handle) in worker_handles.into_iter().enumerate() {
            match handle.await {
                Ok(()) => {
                    log::debug!("Worker {i} completed successfully");
                }
                Err(e) if e.is_cancelled() => {
                    log::debug!("Worker {i} was cancelled (this is normal during shutdown)");
                }
                Err(e) => {
                    log::error!("Worker {i} task failed: {e}");
                    any_errors.push(SagittaEmbedError::thread_safety(format!("Worker {i} failed: {e}")));
                }
            }
        }

        // Check if we have any successful results
        if all_embedded_chunks.is_empty() && !any_errors.is_empty() {
            // All batches failed, return the first error
            return Err(any_errors.into_iter().next().unwrap());
        } else if !any_errors.is_empty() {
            // Some batches failed but we have some results
            log::warn!("Some batches failed ({} errors) but {} chunks were processed successfully", 
                      any_errors.len(), all_embedded_chunks.len());
        }

        log::info!("Completed processing {} chunks in {} batches using parallel workers (with {} errors)", 
                   all_embedded_chunks.len(), batches_received, any_errors.len());

        Ok(all_embedded_chunks)
    }

    /// Static method to process a batch in a worker thread without self reference.
    async fn process_batch_worker(
        chunks: Vec<ProcessedChunk>,
        semaphore: &Arc<Semaphore>,
        providers: &Arc<Mutex<Vec<Arc<dyn EmbeddingProvider>>>>,
        embedding_config: &EmbeddingConfig,
        max_sessions: usize
    ) -> Result<Vec<EmbeddedChunk>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        // Acquire semaphore permit to control concurrency
        let _permit = semaphore.acquire().await
            .map_err(|e| SagittaEmbedError::thread_safety(format!("Failed to acquire semaphore: {e}")))?;

        // Acquire a provider from the pool
        let provider = Self::acquire_provider_static(providers, embedding_config).await?;
        
        // Extract text content for embedding
        let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        
        // Generate embeddings
        let embeddings = match provider.embed_batch(&text_refs) {
            Ok(embeddings) => embeddings,
            Err(e) => {
                // Return provider to pool before failing
                Self::release_provider_static(provider, providers, max_sessions).await;
                return Err(e);
            }
        };
        
        // Return provider to pool
        Self::release_provider_static(provider, providers, max_sessions).await;
        
        // Combine chunks with embeddings
        let processed_at = Instant::now();
        let embedded_chunks = chunks
            .into_iter()
            .zip(embeddings.into_iter())
            .map(|(chunk, embedding)| EmbeddedChunk {
                chunk,
                embedding,
                processed_at,
            })
            .collect();

        Ok(embedded_chunks)
    }

    /// Static method to acquire a provider from the pool.
    async fn acquire_provider_static(
        providers: &Arc<Mutex<Vec<Arc<dyn EmbeddingProvider>>>>,
        embedding_config: &EmbeddingConfig
    ) -> Result<Arc<dyn EmbeddingProvider>> {
        // Try to get an existing provider from the pool first
        {
            let mut providers_guard = providers.lock().await;
            if let Some(provider) = providers_guard.pop() {
                log::debug!("Worker: Reusing existing provider from pool (remaining: {})", providers_guard.len());
                return Ok(provider);
            }
        }

        // No provider available in pool, create a new one
        log::debug!("Worker: Creating new provider");
        log::debug!("Worker: Embedding config model type: {:?}", embedding_config.model_type);
        let provider = create_embedding_provider(embedding_config)?;
        log::debug!("Worker: Successfully created new provider");
        Ok(provider)
    }

    /// Static method to return a provider to the pool.
    async fn release_provider_static(
        provider: Arc<dyn EmbeddingProvider>,
        providers: &Arc<Mutex<Vec<Arc<dyn EmbeddingProvider>>>>,
        max_sessions: usize
    ) {
        let mut providers_guard = providers.lock().await;
        
        // Only keep up to max_sessions providers in the pool
        if providers_guard.len() < max_sessions {
            providers_guard.push(provider);
            log::debug!("Worker: Returned provider to pool (total in pool: {}/{})", providers_guard.len(), max_sessions);
        } else {
            log::debug!("Worker: Pool full, dropping provider");
            // If pool is full, the provider will be dropped
        }
    }

    /// Get current pool statistics.
    pub async fn pool_stats(&self) -> PoolStats {
        let providers = self.providers.lock().await;
        PoolStats {
            available_providers: providers.len(),
            max_providers: self.config.max_embedding_sessions,
            available_permits: self.semaphore.available_permits(),
            cpu_worker_threads: self.cpu_worker_threads,
        }
    }
}

#[async_trait::async_trait]
impl EmbeddingProcessor for EmbeddingPool {
    async fn process_chunks(&self, chunks: Vec<ProcessedChunk>) -> Result<Vec<EmbeddedChunk>> {
        self.process_chunks_with_progress(chunks, Arc::new(crate::processor::NoOpProgressReporter)).await
    }

    async fn process_chunks_with_progress(
        &self,
        chunks: Vec<ProcessedChunk>,
        progress_reporter: Arc<dyn crate::processor::ProgressReporter>
    ) -> Result<Vec<EmbeddedChunk>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        let total_chunks = chunks.len();
        let start_time = Instant::now();

        // Report starting embedding generation
        progress_reporter.report(crate::processor::ProcessingProgress {
            stage: crate::processor::ProcessingStage::GeneratingEmbeddings,
            current_file: None,
            files_completed: 0,
            total_files: total_chunks,
            files_per_second: None,
            message: Some(format!("Starting parallel embedding generation for {} chunks using {} CPU workers", 
                                  total_chunks, self.cpu_worker_threads)),
        }).await;

        // Use the optimized parallel processing with progress reporting
        let embedded_chunks = self.process_chunks_parallel(chunks, Arc::clone(&progress_reporter)).await?;

        // Report completion
        let elapsed_secs = start_time.elapsed().as_secs_f64();
        let chunks_per_second = if elapsed_secs > 0.0 { 
            Some(total_chunks as f64 / elapsed_secs) 
        } else { 
            None 
        };

        progress_reporter.report(crate::processor::ProcessingProgress {
            stage: crate::processor::ProcessingStage::Completed,
            current_file: None,
            files_completed: total_chunks,
            total_files: total_chunks,
            files_per_second: chunks_per_second,
            message: Some(format!("Successfully generated {} embeddings using {} CPU workers and {} sessions ({:.1} chunks/sec)", 
                                  embedded_chunks.len(), self.cpu_worker_threads, self.config.max_embedding_sessions,
                                  chunks_per_second.unwrap_or(0.0))),
        }).await;

        Ok(embedded_chunks)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn max_batch_size(&self) -> usize {
        self.config.embedding_batch_size
    }
}

/// Statistics about the embedding pool state.
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Number of providers currently available in the pool
    pub available_providers: usize,
    /// Maximum number of providers the pool can hold
    pub max_providers: usize,
    /// Number of available semaphore permits
    pub available_permits: usize,
    /// Number of CPU worker threads for coordination
    pub cpu_worker_threads: usize,
}

impl PoolStats {
    /// Check if the pool is at capacity.
    pub fn is_at_capacity(&self) -> bool {
        self.available_permits == 0
    }

    /// Get the utilization percentage (0.0 to 1.0).
    pub fn utilization(&self) -> f64 {
        1.0 - (self.available_permits as f64 / self.max_providers as f64)
    }

    /// Get CPU worker thread utilization info.
    pub fn cpu_worker_info(&self) -> String {
        format!("{} CPU workers for {} sessions", self.cpu_worker_threads, self.max_providers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::{ChunkMetadata, ProcessedChunk};

    fn create_test_chunk(content: &str, id: &str) -> ProcessedChunk {
        ProcessedChunk {
            content: content.to_string(),
            metadata: ChunkMetadata {
                file_path: "test.rs".into(),
                start_line: 1,
                end_line: 10,
                language: "rust".to_string(),
                file_extension: "rs".to_string(),
                element_type: "function".to_string(),
                context: None,
            },
            id: id.to_string(),
        }
    }

    #[tokio::test]
    async fn test_pool_creation() {
        let embedding_config = EmbeddingConfig::default();
        let processing_config = ProcessingConfig {
            max_embedding_sessions: 2,
            ..Default::default()
        };

        let pool = EmbeddingPool::new(processing_config, embedding_config).unwrap();
        assert_eq!(pool.dimension(), 384); // Default dimension
        
        let stats = pool.pool_stats().await;
        assert_eq!(stats.max_providers, 2);
        assert_eq!(stats.available_permits, 2);
    }

    #[tokio::test]
    async fn test_empty_chunk_processing() {
        let embedding_config = EmbeddingConfig::default();
        let pool = EmbeddingPool::with_embedding_config(embedding_config).unwrap();
        
        let result = pool.process_chunks(vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_default_provider_processing() {
        let embedding_config = EmbeddingConfig::default(); // Uses Default model type
        let pool = EmbeddingPool::with_embedding_config(embedding_config).unwrap();
        
        let chunks = vec![
            create_test_chunk("Hello, world!", "chunk_1"),
            create_test_chunk("Test content", "chunk_2"),
        ];
        
        let result = pool.process_chunks(chunks).await.unwrap();
        assert_eq!(result.len(), 2);
        
        // Verify embeddings have correct dimension
        for embedded_chunk in &result {
            assert_eq!(embedded_chunk.embedding.len(), 384);
        }
    }

    #[tokio::test]
    async fn test_embed_texts_async() {
        let embedding_config = EmbeddingConfig::default();
        let pool = EmbeddingPool::with_embedding_config(embedding_config).unwrap();
        
        let texts = vec!["Hello", "World"];
        let embeddings = pool.embed_texts_async(&texts).await.unwrap();
        
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 384);
        assert_eq!(embeddings[1].len(), 384);
    }

    #[test]
    fn test_pool_stats() {
        let stats = PoolStats {
            available_providers: 2,
            max_providers: 4,
            available_permits: 1,
            cpu_worker_threads: 4,
        };

        assert!(!stats.is_at_capacity());
        assert_eq!(stats.utilization(), 0.75); // 3/4 = 0.75
    }

    #[test]
    fn test_pool_stats_at_capacity() {
        let stats = PoolStats {
            available_providers: 0,
            max_providers: 4,
            available_permits: 0,
            cpu_worker_threads: 4,
        };

        assert!(stats.is_at_capacity());
        assert_eq!(stats.utilization(), 1.0);
    }
}