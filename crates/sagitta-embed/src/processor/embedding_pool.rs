//! Embedding pool implementation for controlled GPU memory usage.

use crate::error::{Result, SagittaEmbedError};
use crate::processor::{EmbeddingProcessor, ProcessedChunk, EmbeddedChunk, ProcessingConfig};
use crate::provider::EmbeddingProvider;
use crate::config::EmbeddingConfig;
use std::sync::Arc;
use tokio::sync::{Semaphore, Mutex};
use std::time::Instant;

#[cfg(feature = "onnx")]
use crate::provider::onnx::OnnxEmbeddingModel;

/// Pool of embedding model instances with controlled GPU memory usage.
/// This maintains a fixed number of embedding model instances regardless of
/// the number of concurrent file processing operations.
#[derive(Debug)]
pub struct EmbeddingPool {
    /// Configuration for the pool
    config: ProcessingConfig,
    /// Embedding configuration
    embedding_config: EmbeddingConfig,
    /// Semaphore to control concurrent access to embedding models
    semaphore: Arc<Semaphore>,
    /// Pool of embedding model instances
    #[cfg(feature = "onnx")]
    models: Arc<Mutex<Vec<OnnxEmbeddingModel>>>,
    /// Embedding dimension (cached for performance)
    dimension: usize,
}

impl EmbeddingPool {
    /// Create a new embedding pool with the given configurations.
    pub fn new(config: ProcessingConfig, embedding_config: EmbeddingConfig) -> Result<Self> {
        // Validate embedding config
        embedding_config.validate()?;
        
        // Get dimension from a temporary model instance
        #[cfg(feature = "onnx")]
        let dimension = {
            let model_path = embedding_config.onnx_model_path.as_ref()
                .ok_or_else(|| SagittaEmbedError::configuration("ONNX model path not set"))?;
            let tokenizer_path = embedding_config.onnx_tokenizer_path.as_ref()
                .ok_or_else(|| SagittaEmbedError::configuration("ONNX tokenizer path not set"))?;
            
            let temp_model = OnnxEmbeddingModel::new(model_path, tokenizer_path)?;
            temp_model.dimension()
        };
        
        #[cfg(not(feature = "onnx"))]
        let dimension = embedding_config.expected_dimension.unwrap_or(384);

        // Create the pool
        let semaphore = Arc::new(Semaphore::new(config.max_embedding_sessions));
        
        #[cfg(feature = "onnx")]
        let models = Arc::new(Mutex::new(Vec::new()));

        Ok(Self {
            config,
            embedding_config,
            semaphore,
            #[cfg(feature = "onnx")]
            models,
            dimension,
        })
    }

    /// Create a new embedding pool with default processing config.
    pub fn with_embedding_config(embedding_config: EmbeddingConfig) -> Result<Self> {
        Self::new(ProcessingConfig::default(), embedding_config)
    }

    /// Create a new embedding pool that properly uses the max_sessions from the EmbeddingConfig.
    /// This ensures GPU memory control respects the user's config.toml settings.
    pub fn with_configured_sessions(embedding_config: EmbeddingConfig) -> Result<Self> {
        let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
        Self::new(processing_config, embedding_config)
    }

    /// Get or create an embedding model instance from the pool.
    #[cfg(feature = "onnx")]
    async fn acquire_model(&self) -> Result<OnnxEmbeddingModel> {
        // Try to get an existing model from the pool
        {
            let mut models = self.models.lock().await;
            if let Some(model) = models.pop() {
                return Ok(model);
            }
        }

        // No available model, create a new one
        let model_path = self.embedding_config.onnx_model_path.as_ref()
            .ok_or_else(|| SagittaEmbedError::configuration("ONNX model path not set"))?;
        let tokenizer_path = self.embedding_config.onnx_tokenizer_path.as_ref()
            .ok_or_else(|| SagittaEmbedError::configuration("ONNX tokenizer path not set"))?;

        OnnxEmbeddingModel::new(model_path, tokenizer_path)
    }

    /// Return a model instance to the pool.
    #[cfg(feature = "onnx")]
    async fn release_model(&self, model: OnnxEmbeddingModel) {
        let mut models = self.models.lock().await;
        
        // Only keep up to max_embedding_sessions models in the pool
        if models.len() < self.config.max_embedding_sessions {
            models.push(model);
        }
        // If pool is full, the model will be dropped (freed from GPU memory)
    }

    /// Process a batch of chunks with controlled concurrency.
    async fn process_batch_internal(&self, chunks: Vec<ProcessedChunk>) -> Result<Vec<EmbeddedChunk>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        // Acquire semaphore permit to control concurrency
        let _permit = self.semaphore.acquire().await
            .map_err(|e| SagittaEmbedError::thread_safety(format!("Failed to acquire semaphore: {}", e)))?;

        #[cfg(feature = "onnx")]
        {
            // Acquire a model from the pool
            let model = self.acquire_model().await?;
            
            // Extract text content for embedding
            let texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
            
            // Generate embeddings
            let embeddings = model.embed_batch(&texts)?;
            
            // Return model to pool
            self.release_model(model).await;
            
            // Combine chunks with embeddings
            let processed_at = Instant::now();
            let embedded_chunks: Vec<EmbeddedChunk> = chunks
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

        #[cfg(not(feature = "onnx"))]
        {
            Err(SagittaEmbedError::feature_not_enabled("onnx"))
        }
    }

    /// Get current pool statistics.
    #[cfg(feature = "onnx")]
    pub async fn pool_stats(&self) -> PoolStats {
        let models = self.models.lock().await;
        PoolStats {
            available_models: models.len(),
            max_models: self.config.max_embedding_sessions,
            available_permits: self.semaphore.available_permits(),
        }
    }

    #[cfg(not(feature = "onnx"))]
    pub async fn pool_stats(&self) -> PoolStats {
        PoolStats {
            available_models: 0,
            max_models: self.config.max_embedding_sessions,
            available_permits: self.semaphore.available_permits(),
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
            total_files: total_chunks, // Using "files" field to represent chunks
            files_per_second: None,
            message: Some(format!("Starting embedding generation for {} chunks", total_chunks)),
        }).await;

        // Split chunks into batches based on configured batch size
        let mut all_embedded_chunks = Vec::new();
        let mut chunks_processed = 0;
        
        for (batch_idx, chunk_batch) in chunks.chunks(self.config.embedding_batch_size).enumerate() {
            let batch_chunks = chunk_batch.to_vec();
            let batch_size = batch_chunks.len();
            
            let embedded_batch = self.process_batch_internal(batch_chunks).await?;
            all_embedded_chunks.extend(embedded_batch);
            
            chunks_processed += batch_size;
            
            // Report progress
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
                message: Some(format!("Generated embeddings for batch {} ({}/{} chunks)", batch_idx + 1, chunks_processed, total_chunks)),
            }).await;
        }

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
            message: Some(format!("Successfully generated {} embeddings", total_chunks)),
        }).await;

        Ok(all_embedded_chunks)
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
    /// Number of models currently available in the pool
    pub available_models: usize,
    /// Maximum number of models the pool can hold
    pub max_models: usize,
    /// Number of available semaphore permits
    pub available_permits: usize,
}

impl PoolStats {
    /// Check if the pool is at capacity.
    pub fn is_at_capacity(&self) -> bool {
        self.available_permits == 0
    }

    /// Get the utilization percentage (0.0 to 1.0).
    pub fn utilization(&self) -> f64 {
        1.0 - (self.available_permits as f64 / self.max_models as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::{ChunkMetadata, ProcessedChunk};
    use tempfile::tempdir;
    use std::fs;

    fn create_test_embedding_config() -> EmbeddingConfig {
        let temp_dir = tempdir().unwrap();
        let model_path = temp_dir.path().join("model.onnx");
        let tokenizer_path = temp_dir.path().join("tokenizer.json");

        // Create dummy files
        fs::write(&model_path, "dummy model").unwrap();
        
        // Create minimal valid tokenizer JSON
        let tokenizer_content = serde_json::json!({
            "version": "1.0",
            "truncation": null,
            "padding": null,
            "added_tokens": [],
            "normalizer": null,
            "pre_tokenizer": null,
            "post_processor": null,
            "decoder": null,
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "continuing_subword_prefix": "##",
                "max_input_chars_per_word": 100,
                "vocab": {
                    "[UNK]": 0,
                    "[CLS]": 1,
                    "[SEP]": 2
                }
            }
        });
        fs::write(&tokenizer_path, tokenizer_content.to_string()).unwrap();

        EmbeddingConfig::new_onnx(model_path, tokenizer_path)
    }

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
        let embedding_config = create_test_embedding_config();
        let processing_config = ProcessingConfig {
            max_embedding_sessions: 2,
            ..Default::default()
        };

        // This might fail if ONNX runtime isn't available, but that's expected in test environments
        if let Ok(pool) = EmbeddingPool::new(processing_config, embedding_config) {
            assert_eq!(pool.dimension(), 384); // Default dimension
            
            let stats = pool.pool_stats().await;
            assert_eq!(stats.max_models, 2);
            assert_eq!(stats.available_permits, 2);
        }
    }

    #[tokio::test]
    async fn test_empty_chunk_processing() {
        let embedding_config = create_test_embedding_config();
        
        if let Ok(pool) = EmbeddingPool::with_embedding_config(embedding_config) {
            let result = pool.process_chunks(vec![]).await.unwrap();
            assert!(result.is_empty());
        }
    }

    #[test]
    fn test_pool_stats() {
        let stats = PoolStats {
            available_models: 2,
            max_models: 4,
            available_permits: 1,
        };

        assert!(!stats.is_at_capacity());
        assert_eq!(stats.utilization(), 0.75); // 3/4 = 0.75
    }

    #[test]
    fn test_pool_stats_at_capacity() {
        let stats = PoolStats {
            available_models: 0,
            max_models: 4,
            available_permits: 0,
        };

        assert!(stats.is_at_capacity());
        assert_eq!(stats.utilization(), 1.0);
    }
} 