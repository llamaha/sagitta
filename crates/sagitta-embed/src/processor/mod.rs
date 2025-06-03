//! Processing pipeline components for decoupled file processing and embedding.

use crate::error::Result;
use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

pub mod file_processor;
pub mod embedding_pool;

// Re-export main types
pub use file_processor::DefaultFileProcessor;
pub use embedding_pool::{EmbeddingPool, PoolStats};

/// Metadata associated with a processed chunk of content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// The file path this chunk originated from
    pub file_path: PathBuf,
    /// Start line number in the original file
    pub start_line: usize,
    /// End line number in the original file
    pub end_line: usize,
    /// Programming language detected for this chunk
    pub language: String,
    /// File extension
    pub file_extension: String,
    /// Type of code element (function, class, etc.)
    pub element_type: String,
    /// Additional context or repository information
    pub context: Option<String>,
}

/// A chunk of content that has been processed and is ready for embedding.
#[derive(Debug, Clone)]
pub struct ProcessedChunk {
    /// The text content to be embedded
    pub content: String,
    /// Metadata about this chunk
    pub metadata: ChunkMetadata,
    /// Unique identifier for tracking
    pub id: String,
}

/// A chunk that has been embedded with vector representation.
#[derive(Debug, Clone)]
pub struct EmbeddedChunk {
    /// The original processed chunk
    pub chunk: ProcessedChunk,
    /// The dense embedding vector
    pub embedding: Vec<f32>,
    /// Processing timestamp
    pub processed_at: std::time::Instant,
}

/// Configuration for the processing pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Number of concurrent file processing workers (defaults to CPU core count)
    pub file_processing_concurrency: usize,
    /// Maximum number of embedding model instances (defaults to 4)
    pub max_embedding_sessions: usize,
    /// Size of the processing queue buffer
    pub processing_queue_size: usize,
    /// Maximum batch size for embedding processing
    pub embedding_batch_size: usize,
    /// Maximum file size to process (in bytes)
    pub max_file_size_bytes: u64,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            file_processing_concurrency: num_cpus::get(),
            max_embedding_sessions: 4,
            processing_queue_size: 1000,
            embedding_batch_size: 128,
            max_file_size_bytes: 5 * 1024 * 1024, // 5MB
        }
    }
}

impl ProcessingConfig {
    /// Create a ProcessingConfig that properly uses the max_sessions from an EmbeddingConfig.
    /// This ensures that the GPU memory control (max_embedding_sessions) respects the user's
    /// configuration in config.toml rather than using a hardcoded default.
    pub fn from_embedding_config(embedding_config: &crate::config::EmbeddingConfig) -> Self {
        Self {
            file_processing_concurrency: num_cpus::get(),
            max_embedding_sessions: embedding_config.max_sessions, // Use configured value, not default
            processing_queue_size: 1000,
            embedding_batch_size: 128,
            max_file_size_bytes: 5 * 1024 * 1024, // 5MB
        }
    }
}

/// Trait for processing files into chunks ready for embedding.
/// This is CPU-intensive work (I/O, parsing, tokenization) that can scale independently.
#[async_trait::async_trait]
pub trait FileProcessor: Send + Sync {
    /// Process a single file and return chunks ready for embedding.
    async fn process_file(&self, file_path: &PathBuf) -> Result<Vec<ProcessedChunk>>;
    
    /// Process multiple files concurrently.
    async fn process_files(&self, file_paths: &[PathBuf]) -> Result<Vec<ProcessedChunk>>;
    
    /// Process multiple files concurrently with progress reporting.
    async fn process_files_with_progress(
        &self, 
        file_paths: &[PathBuf], 
        progress_reporter: Arc<dyn ProgressReporter>
    ) -> Result<Vec<ProcessedChunk>>;
    
    /// Get the current configuration.
    fn config(&self) -> &ProcessingConfig;
}

/// Trait for processing chunks into embeddings.
/// This is GPU-intensive work that should be carefully controlled for memory usage.
#[async_trait::async_trait]
pub trait EmbeddingProcessor: Send + Sync {
    /// Process a batch of chunks into embeddings.
    async fn process_chunks(&self, chunks: Vec<ProcessedChunk>) -> Result<Vec<EmbeddedChunk>>;
    
    /// Process a batch of chunks into embeddings with progress reporting.
    async fn process_chunks_with_progress(
        &self,
        chunks: Vec<ProcessedChunk>,
        progress_reporter: Arc<dyn ProgressReporter>
    ) -> Result<Vec<EmbeddedChunk>>;
    
    /// Get the embedding dimension.
    fn dimension(&self) -> usize;
    
    /// Get the maximum recommended batch size.
    fn max_batch_size(&self) -> usize;
}

/// Progress information for file processing operations
#[derive(Debug, Clone)]
pub struct ProcessingProgress {
    /// Current stage of processing
    pub stage: ProcessingStage,
    /// Current file being processed (if applicable)
    pub current_file: Option<PathBuf>,
    /// Number of files completed so far
    pub files_completed: usize,
    /// Total number of files to process
    pub total_files: usize,
    /// Files processed per second (if available)
    pub files_per_second: Option<f64>,
    /// Optional message providing additional context
    pub message: Option<String>,
}

/// Different stages of the processing pipeline
#[derive(Debug, Clone)]
pub enum ProcessingStage {
    /// Starting the processing operation
    Starting,
    /// Processing individual files
    ProcessingFiles,
    /// Generating embeddings from processed chunks
    GeneratingEmbeddings,
    /// Processing completed successfully
    Completed,
    /// An error occurred during processing
    Error { message: String },
}

/// Trait for reporting processing progress
#[async_trait::async_trait]
pub trait ProgressReporter: Send + Sync {
    /// Called to report progress updates
    async fn report(&self, progress: ProcessingProgress);
}

/// No-op progress reporter that ignores all progress updates
#[derive(Debug, Clone)]
pub struct NoOpProgressReporter;

#[async_trait::async_trait]
impl ProgressReporter for NoOpProgressReporter {
    async fn report(&self, _progress: ProcessingProgress) {
        // Does nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EmbeddingConfig;
    use crate::model::EmbeddingModelType;

    #[test]
    fn test_processing_config_default() {
        let config = ProcessingConfig::default();
        assert_eq!(config.file_processing_concurrency, num_cpus::get());
        assert_eq!(config.max_embedding_sessions, 4); // Default value
        assert_eq!(config.processing_queue_size, 1000);
        assert_eq!(config.embedding_batch_size, 128);
        assert_eq!(config.max_file_size_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn test_processing_config_from_embedding_config() {
        let embedding_config = EmbeddingConfig {
            model_type: EmbeddingModelType::Onnx,
            max_sessions: 8, // Custom value that should be respected
            ..Default::default()
        };

        let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
        
        // Verify that max_embedding_sessions uses the configured value, not the default
        assert_eq!(processing_config.max_embedding_sessions, 8);
        assert_eq!(processing_config.file_processing_concurrency, num_cpus::get());
        assert_eq!(processing_config.processing_queue_size, 1000);
        assert_eq!(processing_config.embedding_batch_size, 128);
        assert_eq!(processing_config.max_file_size_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn test_processing_config_respects_config_toml_values() {
        // Test various max_sessions values that could come from config.toml
        for max_sessions in [1, 2, 4, 8, 16] {
            let embedding_config = EmbeddingConfig {
                max_sessions,
                ..Default::default()
            };

            let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
            assert_eq!(processing_config.max_embedding_sessions, max_sessions,
                "ProcessingConfig should respect max_sessions value {} from config.toml", max_sessions);
        }
    }
} 