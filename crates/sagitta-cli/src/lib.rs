#![allow(dead_code)] // Allow dead code for now

//! # sagitta_cli
//!
//! This crate provides the core functionality for the Sagitta semantic code search CLI 
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
//! sagitta-cli server --port 50051
//! ```
//!
//! Clients can then connect to the gRPC API to perform operations.
//! Use the `sagitta-client` crate for a full-featured Rust client.
//! 
//! ## Edit Feature
//! 
//! The library provides powerful code editing capabilities:
//! 
//! ```bash
//! # Example: Replace a class with semantic targeting
//! sagitta-cli edit apply --file src/my_app.py --element "class:Calculator" --content-file new_calculator.py
//! 
//! # Example: Validate before applying an edit
//! sagitta-cli edit validate --file src/my_app.py --element "function:process_data" --content-file new_function.py
//! ```
//! 
//! For library usage, see the [`edit`](edit) module documentation.

// Public CLI module
pub mod cli;
// Public Edit module (contains CLI subcommands)
pub mod edit;
pub mod progress;

// Re-export key components from the core library for convenience
pub use sagitta_search::config::{AppConfig, IndexingConfig, RepositoryConfig, load_config, save_config, get_config_path_or_default, get_managed_repos_from_config};
pub use sagitta_search::{EmbeddingHandler, EmbeddingModel, EmbeddingModelType};
pub use sagitta_search::error::{SagittaError, Result as CoreResult}; // Alias CoreResult to avoid clash
pub use sagitta_search::qdrant_client_trait::QdrantClientTrait;
pub use sagitta_search::constants::*;
pub use sagitta_search::cache::EmbeddingCache;
pub use sagitta_search::snippet_extractor::extract_snippet;
pub use sagitta_search::edit::{apply_edit, validate_edit, EditTarget, EngineEditOptions, EngineValidationIssue, EngineValidationSeverity};
pub use sagitta_search::repo_helpers::{delete_repository_data, get_collection_name, ensure_repository_collection_exists};
pub use sagitta_search::repo_add::{handle_repo_add, AddRepoArgs, AddRepoError};
pub use sagitta_search::sync::{sync_repository, SyncOptions, SyncResult};
pub use sagitta_search::fs_utils::{find_files_matching_pattern, read_file_range};

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
