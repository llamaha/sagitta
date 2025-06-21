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

/// Configuration for the processing pipeline with optimized threading support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Number of concurrent file processing workers (defaults to CPU core count)
    pub file_processing_concurrency: usize,
    /// Maximum number of embedding model instances (defaults to 4)
    pub max_embedding_sessions: usize,
    /// Number of CPU worker threads for GPU coordination (auto-calculated if 0)
    pub cpu_worker_threads: usize,
    /// Size of the processing queue buffer
    pub processing_queue_size: usize,
    /// Maximum batch size for embedding processing
    pub embedding_batch_size: usize,
    /// Maximum file size to process (in bytes)
    pub max_file_size_bytes: u64,
    /// File processing batch size for reducing coordination overhead
    pub file_batch_size: usize,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        let cpu_cores = num_cpus::get();
        Self {
            file_processing_concurrency: cpu_cores,
            max_embedding_sessions: 4,
            cpu_worker_threads: 0, // Auto-calculate
            processing_queue_size: 1000,
            embedding_batch_size: crate::DEFAULT_EMBEDDING_BATCH_SIZE,
            max_file_size_bytes: 5 * 1024 * 1024, // 5MB
            file_batch_size: std::cmp::max(1, cpu_cores / 4), // Reduce coordination overhead
        }
    }
}

impl ProcessingConfig {
    /// Create a ProcessingConfig from an EmbeddingConfig.
    /// Uses default values for session management since max_sessions has been removed.
    pub fn from_embedding_config(embedding_config: &crate::config::EmbeddingConfig) -> Self {
        let cpu_cores = num_cpus::get();
        // Use a reasonable default for GPU memory control
        let default_sessions = 4;
        
        Self {
            file_processing_concurrency: cpu_cores,
            max_embedding_sessions: default_sessions,
            cpu_worker_threads: Self::calculate_optimal_cpu_workers(default_sessions, cpu_cores),
            processing_queue_size: 1000,
            embedding_batch_size: embedding_config.get_embedding_batch_size(), // Use configured or default batch size
            max_file_size_bytes: 5 * 1024 * 1024, // 5MB
            file_batch_size: std::cmp::max(1, cpu_cores / 4),
        }
    }

    /// Calculate optimal number of CPU worker threads for GPU coordination.
    /// This balances CPU utilization with memory bandwidth and coordination overhead.
    pub fn calculate_optimal_cpu_workers(max_gpu_sessions: usize, cpu_cores: usize) -> usize {
        // Use more CPU threads than GPU sessions to handle coordination overhead
        // but don't exceed available CPU cores
        let optimal = std::cmp::max(
            max_gpu_sessions * 2, // At least 2 CPU threads per GPU session
            cpu_cores / 2 // But use at least half the CPU cores
        );
        
        // Cap at total CPU cores to avoid oversubscription
        std::cmp::min(optimal, cpu_cores)
    }

    /// Get the effective number of CPU worker threads, calculating if needed.
    pub fn effective_cpu_worker_threads(&self) -> usize {
        if self.cpu_worker_threads == 0 {
            Self::calculate_optimal_cpu_workers(self.max_embedding_sessions, num_cpus::get())
        } else {
            self.cpu_worker_threads
        }
    }

    /// Create a configuration optimized for high-throughput processing.
    pub fn high_throughput() -> Self {
        let cpu_cores = num_cpus::get();
        Self {
            file_processing_concurrency: cpu_cores,
            max_embedding_sessions: std::cmp::max(4, cpu_cores / 2), // More GPU sessions
            cpu_worker_threads: cpu_cores, // Use all CPU cores for coordination
            processing_queue_size: 2000, // Larger queue
            embedding_batch_size: 256, // Larger batches
            max_file_size_bytes: 10 * 1024 * 1024, // 10MB
            file_batch_size: std::cmp::max(1, cpu_cores / 2), // Larger file batches
        }
    }

    /// Create a configuration optimized for low-memory usage.
    pub fn low_memory() -> Self {
        Self {
            file_processing_concurrency: std::cmp::max(1, num_cpus::get() / 2),
            max_embedding_sessions: 2, // Fewer GPU sessions
            cpu_worker_threads: 4, // Fewer CPU workers
            processing_queue_size: 500, // Smaller queue
            embedding_batch_size: 64, // Smaller batches
            max_file_size_bytes: 2 * 1024 * 1024, // 2MB
            file_batch_size: 1, // Process files individually
        }
    }

    /// Validate the configuration and adjust if necessary.
    pub fn validate_and_adjust(&mut self) {
        let cpu_cores = num_cpus::get();
        
        // Ensure file processing concurrency doesn't exceed CPU cores
        if self.file_processing_concurrency > cpu_cores {
            log::warn!("File processing concurrency ({}) exceeds CPU cores ({}), adjusting", 
                       self.file_processing_concurrency, cpu_cores);
            self.file_processing_concurrency = cpu_cores;
        }

        // Ensure at least 1 embedding session
        if self.max_embedding_sessions == 0 {
            log::warn!("Max embedding sessions is 0, setting to 1");
            self.max_embedding_sessions = 1;
        }

        // Calculate CPU worker threads if not set
        if self.cpu_worker_threads == 0 {
            self.cpu_worker_threads = Self::calculate_optimal_cpu_workers(
                self.max_embedding_sessions, 
                cpu_cores
            );
        }

        // Ensure CPU worker threads don't exceed CPU cores
        if self.cpu_worker_threads > cpu_cores {
            log::warn!("CPU worker threads ({}) exceeds CPU cores ({}), adjusting", 
                       self.cpu_worker_threads, cpu_cores);
            self.cpu_worker_threads = cpu_cores;
        }

        // Ensure reasonable batch sizes
        if self.embedding_batch_size == 0 {
            self.embedding_batch_size = 128;
        }
        if self.file_batch_size == 0 {
            self.file_batch_size = 1;
        }

        log::info!("ProcessingConfig validated: {} CPU cores, {} file workers, {} GPU sessions, {} CPU workers",
                   cpu_cores, self.file_processing_concurrency, self.max_embedding_sessions, self.cpu_worker_threads);
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
#[derive(Debug, Clone, PartialEq)]
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
        assert_eq!(config.embedding_batch_size, 256);  // Default batch size is 256
        assert_eq!(config.max_file_size_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn test_processing_config_from_embedding_config() {
        let embedding_config = EmbeddingConfig {
            model_type: EmbeddingModelType::Onnx,
            ..Default::default()
        };

        let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
        
        // Verify that automatic session management is used
        assert_eq!(processing_config.max_embedding_sessions, 4); // Default for automatic management
        assert_eq!(processing_config.file_processing_concurrency, num_cpus::get());
        assert_eq!(processing_config.processing_queue_size, 1000);
        assert_eq!(processing_config.embedding_batch_size, 256);  // Default batch size is 256
        assert_eq!(processing_config.max_file_size_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn test_processing_config_respects_config_toml_values() {
        // Test automatic session management with default config
        let embedding_config = EmbeddingConfig::default();

        let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
        
        // Automatic session management uses 4 sessions by default
        assert_eq!(processing_config.max_embedding_sessions, 4,
            "ProcessingConfig should use automatic session management");
    }
} 