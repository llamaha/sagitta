#![warn(missing_docs)] // Enforce documentation for all public items

//! `vectordb-core` is the core library powering the `vectordb-cli` application.
//! 
//! It provides the foundational components for:
//! - Configuration management (`config`)
//! - Generating embeddings using ONNX models (`embedding`)
//! - Interacting with the Qdrant vector database (`qdrant_ops`, `qdrant_client_trait`)
//! - Indexing codebases (`indexing`, `syntax`, `snippet_extractor`)
//! - Searching indexed code (`search_impl`)
//! - Managing Git repositories (`git_helpers`, `repo_helpers`, `repo_add`)
//! - Applying and validating code edits (`edit`)
//! - Error handling (`error`)
//! - Caching (`cache`)
//! 
//! ## Overview
//! 
//! This library orchestrates the process of converting source code into vector embeddings,
//! storing them in Qdrant, and performing semantic searches based on natural language queries.
//! It also includes utilities for managing the repositories being indexed and applying suggested
//! code modifications.
//! 
//! ## Usage
//! 
//! While primarily designed for use by `vectordb-cli`, components of this library can be 
//! used programmatically. See the [library README](./README.md) for a quickstart guide 
//! on dependencies (especially ONNX Runtime) and basic initialization.
//! 
//! Key modules to explore:
//! - `embedding`: For handling embedding models.
//! - `indexing`: For the indexing workflow.
//! - `search_impl`: For performing searches.
//! - `qdrant_ops`: For direct Qdrant operations.
//! - `config`: For loading and managing configuration.
//! 
//! ## Features
//! 
//! - `onnx`: (Default) Enables the ONNX embedding provider.
//! - `ort/cuda`: Enables CUDA acceleration for ONNX (Linux, requires toolkit).
//! - `ort/coreml`: Enables Core ML acceleration for ONNX (macOS, requires code modification).
//! - `ort/metal`: Enables Metal acceleration for ONNX (macOS, requires code modification).
//! 
//! **Note:** Using this library directly requires careful handling of runtime dependencies like
//! the ONNX Runtime shared libraries, unlike the bundled `vectordb-cli` executable.

// crates/vectordb-core/src/lib.rs

// Public modules
pub mod config;
pub mod embedding;
pub mod error;
pub mod qdrant_client_trait;
pub mod qdrant_ops;
pub mod syntax;
pub mod constants;
pub mod edit;
pub mod search_impl;
pub mod git_helpers;
pub mod cache; // Added cache module
pub mod indexing; // Added indexing module
pub mod repo_add;
pub mod repo_helpers;
pub mod snippet_extractor;
pub mod tokenizer;
pub mod vocabulary;

pub use config::{AppConfig, IndexingConfig, RepositoryConfig, load_config, save_config, get_config_path_or_default, get_managed_repos_from_config};
pub use embedding::{EmbeddingHandler, EmbeddingModel, EmbeddingModelType};
pub use error::{VectorDBError, Result};
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
pub use repo_helpers::{delete_repository_data, sync_repository_branch, switch_repository_branch, get_collection_name, ensure_repository_collection_exists};
pub use repo_add::{handle_repo_add, AddRepoArgs, AddRepoError}; // Assuming repo_add is needed by CLI

// Keep the basic add function and test for now
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
