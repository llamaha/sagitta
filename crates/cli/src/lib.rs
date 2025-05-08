#![allow(dead_code)] // Allow dead code for now

//! # vectordb_lib
//!
//! This crate provides the core functionality for the VectorDB semantic code search CLI 
//! and server. It enables indexing codebases and performing semantic searches using 
//! vector embeddings, with both CLI and gRPC server interfaces.
//!
//! ## Core Components
//!
//! *   **CLI**: Command-line interface for local operations and server management
//! *   **Server**: gRPC server implementation for remote access
//! *   **Config**: Configuration management for the application
//! *   **Edit**: Semantic code editing with validation
//!
//! ## Internal Components (not part of public API)
//!
//! *   Syntax parsing: Using tree-sitter to extract code semantics
//! *   Vector database: For storing and searching vector embeddings
//! *   Embedding model: For generating vector embeddings from code
//!
//! ## Server Usage
//!
//! The main way to use this library is by starting the server:
//!
//! ```bash
//! vectordb-cli server --port 50051
//! ```
//!
//! Clients can then connect to the gRPC API to perform operations.
//! Use the `vectordb-client` crate for a full-featured Rust client.
//! 
//! ## Edit Feature
//! 
//! The library provides powerful code editing capabilities:
//! 
//! ```bash
//! # Example: Replace a class with semantic targeting
//! vectordb-cli edit apply --file src/my_app.py --element "class:Calculator" --content-file new_calculator.py
//! 
//! # Example: Validate before applying an edit
//! vectordb-cli edit validate --file src/my_app.py --element "function:process_data" --content-file new_function.py
//! ```
//! 
//! For library usage, see the [`edit`](edit) module documentation.

// Public CLI module
pub mod cli;
// Public Edit module (contains CLI subcommands)
pub mod edit;

// Re-export key components from the core library for convenience
pub use vectordb_core::config::{AppConfig, IndexingConfig, RepositoryConfig, load_config, save_config, get_config_path_or_default, get_managed_repos_from_config};
pub use vectordb_core::embedding::{EmbeddingHandler, EmbeddingModel, EmbeddingModelType};
pub use vectordb_core::error::{VectorDBError, Result as CoreResult}; // Alias CoreResult to avoid clash
pub use vectordb_core::qdrant_client_trait::QdrantClientTrait;
pub use vectordb_core::constants::*;
pub use vectordb_core::cache::EmbeddingCache;
pub use vectordb_core::snippet_extractor::extract_snippet;
pub use vectordb_core::edit::{apply_edit, validate_edit, EditTarget, EngineEditOptions, EngineValidationIssue, EngineValidationSeverity};
pub use vectordb_core::repo_helpers::{delete_repository_data, switch_repository_branch, get_collection_name, ensure_repository_collection_exists};
pub use vectordb_core::repo_add::{handle_repo_add, AddRepoArgs, AddRepoError};
pub use vectordb_core::sync::{sync_repository, SyncOptions, SyncResult};
pub use vectordb_core::fs_utils::{find_files_matching_pattern, read_file_range};

// Expose necessary Qdrant types if needed by consumers
pub use qdrant_client::Qdrant;

// Example function (optional)
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
