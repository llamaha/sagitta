#![warn(missing_docs)] // Enforce documentation for all public items

//! # sagitta-search
//! `sagitta-search` is the core library powering the `sagitta-cli` application.
//! It provides semantic code search functionality, including:
//!
//! - **Indexing**: Parse and index codebases using tree-sitter for syntax analysis
//! - **Embedding**: Generate embeddings using ONNX Runtime with sentence-transformer models
//! - **Search**: Perform semantic similarity search using Qdrant vector database
//! - **Configuration**: Manage application settings and repository configurations
//! - **Caching**: Cache embeddings and other data for improved performance
//!
//! ## Features
//!
//! - **Multi-language support**: Python, Rust, JavaScript, TypeScript, and more
//! - **GPU acceleration**: Optional CUDA support for faster embedding generation
//! - **Parallel processing**: Efficient multi-threaded indexing and search
//! - **Flexible configuration**: TOML-based configuration with sensible defaults
//! - **Repository management**: Git integration for tracking code changes
//!
//! ## Usage
//!
//! This library is primarily designed for internal use by the Sagitta ecosystem.
//! While primarily designed for use by `sagitta-cli`, components of this library can be
//! used independently for custom semantic search applications.
//!
//! ## Example
//!
//! ```rust,no_run
//! use sagitta_search::{AppConfig, EmbeddingPool, app_config_to_embedding_config};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Load configuration
//! let config = AppConfig::default();
//!
//! // Convert to embedding config and initialize embedding pool
//! let embedding_config = app_config_to_embedding_config(&config);
//! let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Note on ONNX Runtime
//!
//! This library requires ONNX Runtime for embedding generation. When building
//! applications that use this library, ensure you have the appropriate ONNX Runtime
//! libraries installed on your system. The `ort` crate will attempt to download
//! the ONNX Runtime shared libraries, unlike the bundled `sagitta-cli` executable.

// crates/sagitta-search/src/lib.rs

// Public modules
/// Configuration management for the application.
pub mod config;
/// Embedding generation and handling.
// pub mod embedding; // Migrated to sagitta-embed crate
/// Defines the core error types and Result alias.
pub mod error;
/// Trait defining the interface for a Qdrant client, enabling mocking.
pub mod qdrant_client_trait;
/// Low-level Qdrant client operations.
pub mod qdrant_ops;
/// Code parsing and chunking logic for different languages.
pub mod syntax;
/// Shared constants used across the library.
pub mod constants;
/// Code editing and validation logic.
pub mod edit;
/// Implementation of search functionality.
pub mod search_impl;
/// Caching mechanisms, primarily for embeddings.
pub mod cache; // Added cache module
/// Core logic for indexing repositories and files.
pub mod indexing; // Added indexing module
/// Logic for adding new repositories.
pub mod repo_add;
/// Helper functions for repository operations (Git, Qdrant, Indexing).
pub mod repo_helpers;
/// Extracts code snippets with context.
pub mod snippet_extractor;
/// Code tokenization logic.
pub mod tokenizer;
/// Manages vocabulary for sparse vectors.
pub mod vocabulary;
/// Filesystem utilities.
pub mod fs_utils; // Added fs_utils module
/// Repository synchronization logic.
pub mod sync; // Added sync module
/// Repository synchronization progress logic.
pub mod sync_progress;

#[cfg(test)]
/// Utilities specific to testing within the core library.
pub mod test_utils;

pub use config::{AppConfig, IndexingConfig, RepositoryConfig, EmbeddingEngineConfig, load_config, save_config, get_config_path_or_default, get_managed_repos_from_config};
// Re-export from sagitta-embed crate
pub use sagitta_embed::{EmbeddingModel, EmbeddingModelType, EmbeddingPool, EmbeddingProcessor};
// Re-export EmbeddingConfig for convenience
pub use sagitta_embed::config::EmbeddingConfig;
pub use error::{SagittaError, Result};
pub use qdrant_client_trait::QdrantClientTrait;
pub use constants::*;
pub use cache::EmbeddingCache; // Added cache re-export
pub use snippet_extractor::extract_snippet;

// Re-export core functionalities
// pub use indexing::index_paths; // Removed - indexing mod not directly exposed
pub use search_impl::search_collection; // Correctly export search_collection
pub use qdrant_ops::delete_all_points;

// Re-export other necessary items if needed by CLI directly
pub use edit::{apply_edit, validate_edit, EditTarget, EngineEditOptions, EngineValidationIssue, EngineValidationSeverity};
pub use repo_helpers::{delete_repository_data, get_collection_name, ensure_repository_collection_exists};
pub use repo_add::{handle_repo_add, AddRepoArgs, AddRepoError}; // Assuming repo_add is needed by CLI
pub use sync::{sync_repository, SyncOptions, SyncResult}; // Added sync re-export

// Re-export qdrant types needed by mcp or other crates
pub use qdrant_client::qdrant::{PointStruct, Filter, Condition, FieldCondition, Match, Range, PointsSelector, Value, Vectors, Vector, NamedVectors, ScoredPoint, SearchPoints, QueryPoints, QueryResponse, CollectionInfo, CountPoints, CountResponse, PointsOperationResponse, UpsertPoints, DeletePoints, CreateCollection, DeleteCollection, HealthCheckReply, Distance, VectorParams, VectorsConfig, SparseVectorParams, SparseVectorConfig, vectors_config, point_id::PointIdOptions, PointId, VectorParamsMap, HnswConfigDiff, OptimizersConfigDiff, WalConfigDiff, QuantizationConfig, ScalarQuantization, ProductQuantization, BinaryQuantization, /*quantization_config::Quantizer,*/ CompressionRatio, ListCollectionsResponse, CollectionDescription, AliasDescription, /*CollectionAliases,*/ ListAliasesRequest, /*UpdateCollectionAliases,*/ AliasOperations, CreateAlias, RenameAlias, DeleteAlias};

use std::sync::Arc;
use async_trait::async_trait;

/// Basic addition function (example/placeholder).
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn test_app_config_to_embedding_config_mapping() {
        use crate::config::{AppConfig, EmbeddingEngineConfig, PerformanceConfig};
        
        // Create a test AppConfig with custom embedding settings
        let mut app_config = AppConfig::default();
        app_config.onnx_model_path = Some("/path/to/model.onnx".to_string());
        app_config.onnx_tokenizer_path = Some("/path/to/tokenizer.json".to_string());
        app_config.tenant_id = Some("test-tenant".to_string());
        app_config.performance.vector_dimension = 512;
        
        // Set custom embedding configuration
        app_config.embedding = EmbeddingEngineConfig {
            max_sessions: 8,
            max_sequence_length: 256,
            session_timeout_seconds: 600,
            enable_session_cleanup: false,
            embedding_batch_size: 64,
        };
        
        // Convert to EmbeddingConfig
        let embedding_config = app_config_to_embedding_config(&app_config);
        
        // Verify all fields are properly mapped
        assert_eq!(embedding_config.model_type, EmbeddingModelType::Onnx);
        assert_eq!(embedding_config.onnx_model_path, Some("/path/to/model.onnx".into()));
        assert_eq!(embedding_config.onnx_tokenizer_path, Some("/path/to/tokenizer.json".into()));
        assert_eq!(embedding_config.max_sessions, 8);
        assert_eq!(embedding_config.max_sequence_length, 256);
        assert_eq!(embedding_config.session_timeout_seconds, 600);
        assert_eq!(embedding_config.enable_session_cleanup, false);
        assert_eq!(embedding_config.tenant_id, Some("test-tenant".to_string()));
        assert_eq!(embedding_config.expected_dimension, Some(512));
        assert_eq!(embedding_config.embedding_batch_size, Some(64));
    }

    #[test]
    fn test_app_config_to_embedding_config_defaults() {
        // Test with default AppConfig
        let app_config = AppConfig::default();
        let embedding_config = app_config_to_embedding_config(&app_config);
        
        // Verify defaults are properly set
        assert_eq!(embedding_config.model_type, EmbeddingModelType::Default);
        assert_eq!(embedding_config.onnx_model_path, None);
        assert_eq!(embedding_config.onnx_tokenizer_path, None);
        assert_eq!(embedding_config.max_sessions, 4); // Default from EmbeddingEngineConfig
        assert_eq!(embedding_config.max_sequence_length, 128);
        assert_eq!(embedding_config.session_timeout_seconds, 300);
        assert_eq!(embedding_config.enable_session_cleanup, true);
        assert_eq!(embedding_config.tenant_id, None);
        assert_eq!(embedding_config.expected_dimension, Some(384)); // Default vector dimension
        assert_eq!(embedding_config.embedding_batch_size, Some(128)); // Default batch size
    }

    #[test]
    fn test_config_toml_to_decoupled_processing() {
        use sagitta_embed::processor::ProcessingConfig;
        
        // Simulate different max_sessions values that could be set in config.toml
        for max_sessions in [1, 2, 4, 8, 16] {
            let mut app_config = AppConfig::default();
            app_config.embedding.max_sessions = max_sessions;
            
            // Bridge to embedding config (this is what happens in practice)
            let embedding_config = app_config_to_embedding_config(&app_config);
            assert_eq!(embedding_config.max_sessions, max_sessions);
            
            // Create processing config for decoupled architecture
            let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
            
            // Verify that GPU memory control respects config.toml setting
            assert_eq!(processing_config.max_embedding_sessions, max_sessions,
                "max_embedding_sessions should respect config.toml [embedding].max_sessions = {}", 
                max_sessions);
        }
    }
}

#[macro_use]
extern crate log;

/// Helper function to convert AppConfig to EmbeddingConfig for the new sagitta-embed crate
pub fn app_config_to_embedding_config(app_config: &AppConfig) -> EmbeddingConfig {
    use sagitta_embed::config::EmbeddingConfig;
    use std::path::PathBuf;
    
    let mut embedding_config = EmbeddingConfig::new();
    
    // Set ONNX paths if available
    if let Some(ref model_path) = app_config.onnx_model_path {
        embedding_config.onnx_model_path = Some(PathBuf::from(model_path));
    }
    
    if let Some(ref tokenizer_path) = app_config.onnx_tokenizer_path {
        embedding_config.onnx_tokenizer_path = Some(PathBuf::from(tokenizer_path));
    }
    
    // Set model type based on available paths
    if app_config.onnx_model_path.is_some() && app_config.onnx_tokenizer_path.is_some() {
        embedding_config.model_type = EmbeddingModelType::Onnx;
    }
    
    // Map embedding engine configuration
    embedding_config.max_sessions = app_config.embedding.max_sessions;
    embedding_config.max_sequence_length = app_config.embedding.max_sequence_length;
    embedding_config.session_timeout_seconds = app_config.embedding.session_timeout_seconds;
    embedding_config.enable_session_cleanup = app_config.embedding.enable_session_cleanup;
    embedding_config.embedding_batch_size = Some(app_config.embedding.embedding_batch_size);
    
    // Map tenant ID if available
    if let Some(ref tenant_id) = app_config.tenant_id {
        embedding_config.tenant_id = Some(tenant_id.clone());
    }
    
    // Set expected dimension from performance config
    embedding_config.expected_dimension = Some(app_config.performance.vector_dimension as usize);
    
    embedding_config
}

/// Creates an EmbeddingPool that properly respects the max_sessions configuration.
/// This provides GPU memory control through session pooling.
/// 
/// # Arguments
/// * `app_config` - The application configuration containing embedding settings
/// 
/// # Returns
/// * `Result<Arc<EmbeddingPool>>` - A thread-safe embedding pool that respects max_sessions
/// 
/// # Example
/// ```rust,no_run
/// use sagitta_search::{create_embedding_pool, embed_text_with_pool, AppConfig};
/// 
/// # async fn example(config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
/// let pool = create_embedding_pool(config).await?;
/// let embeddings = embed_text_with_pool(&pool, &["Hello world"]).await?;
/// # Ok(())
/// # }
/// ```
pub async fn create_embedding_pool(app_config: &AppConfig) -> std::result::Result<Arc<EmbeddingPool>, SagittaError> {
    let embedding_config = app_config_to_embedding_config(app_config);
    let pool = EmbeddingPool::with_configured_sessions(embedding_config)
        .map_err(|e| SagittaError::EmbeddingError(e.to_string()))?;
    Ok(Arc::new(pool))
}

/// Creates an EmbeddingPool from an EmbeddingConfig that properly respects max_sessions.
/// This is a convenience function for cases where you already have an EmbeddingConfig.
/// 
/// # Arguments
/// * `embedding_config` - The embedding configuration
/// 
/// # Returns
/// * `Result<Arc<EmbeddingPool>>` - A thread-safe embedding pool that respects max_sessions
pub async fn create_embedding_pool_from_config(embedding_config: EmbeddingConfig) -> std::result::Result<Arc<EmbeddingPool>, SagittaError> {
    let pool = EmbeddingPool::with_configured_sessions(embedding_config)
        .map_err(|e| SagittaError::EmbeddingError(e.to_string()))?;
    Ok(Arc::new(pool))
}

/// Helper function to embed text using EmbeddingPool with the same interface as EmbeddingHandler.
/// This bridges the gap between the old EmbeddingHandler::embed() and new EmbeddingPool::process_chunks().
pub async fn embed_text_with_pool(pool: &EmbeddingPool, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
    use sagitta_embed::processor::{ProcessedChunk, ChunkMetadata};
    use std::path::PathBuf;
    
    // Convert texts to ProcessedChunks
    let chunks: Vec<ProcessedChunk> = texts.iter().enumerate().map(|(i, text)| {
        ProcessedChunk {
            content: text.to_string(),
            metadata: ChunkMetadata {
                file_path: PathBuf::from("text"),
                start_line: 0,
                end_line: 0,
                language: "text".to_string(),
                file_extension: "txt".to_string(),
                element_type: "text".to_string(),
                context: None,
            },
            id: format!("text_{}", i),
        }
    }).collect();
    
    // Process chunks
    let embedded_chunks = pool.process_chunks(chunks).await
        .map_err(|e| SagittaError::EmbeddingError(e.to_string()))?;
    
    // Extract embeddings
    let embeddings = embedded_chunks.into_iter()
        .map(|chunk| chunk.embedding)
        .collect();
    
    Ok(embeddings)
}

/// Helper function to embed a single text using EmbeddingPool.
pub async fn embed_single_text_with_pool(pool: &EmbeddingPool, text: &str) -> Result<Vec<f32>> {
    let embeddings = embed_text_with_pool(pool, &[text]).await?;
    embeddings.into_iter().next()
        .ok_or_else(|| SagittaError::EmbeddingError("No embedding generated".to_string()))
}

/// Adapter that implements EmbeddingProvider for EmbeddingPool.
/// This bridges the interface gap between the old EmbeddingHandler and new EmbeddingPool.
#[derive(Debug)]
pub struct EmbeddingPoolAdapter {
    pool: Arc<EmbeddingPool>,
}

impl EmbeddingPoolAdapter {
    pub fn new(pool: Arc<EmbeddingPool>) -> Self {
        Self { pool }
    }
}

impl sagitta_embed::provider::EmbeddingProvider for EmbeddingPoolAdapter {
    fn dimension(&self) -> usize {
        self.pool.dimension()
    }

    fn model_type(&self) -> sagitta_embed::EmbeddingModelType {
        sagitta_embed::EmbeddingModelType::Onnx // EmbeddingPool is ONNX-based
    }

    fn embed_batch(&self, texts: &[&str]) -> std::result::Result<Vec<Vec<f32>>, sagitta_embed::SagittaEmbedError> {
        // This is a blocking call, but EmbeddingPool is async
        // We need to use a runtime to handle this
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            embed_text_with_pool(&self.pool, texts).await
                .map_err(|e| sagitta_embed::SagittaEmbedError::provider(e.to_string()))
        })
    }
}