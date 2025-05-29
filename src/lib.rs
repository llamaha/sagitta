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
//! use sagitta_search::{AppConfig, EmbeddingHandler};
//!
//! // Load configuration
//! let config = AppConfig::default();
//!
//! // Initialize embedding handler
//! let embedding_handler = EmbeddingHandler::new(&config)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
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
pub mod embedding;
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

pub use config::{AppConfig, IndexingConfig, RepositoryConfig, load_config, save_config, get_config_path_or_default, get_managed_repos_from_config};
pub use embedding::{EmbeddingHandler, EmbeddingModel, EmbeddingModelType};
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
}

#[macro_use]
extern crate log;
