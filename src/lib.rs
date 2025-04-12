#![allow(dead_code)] // Keep for now during refactoring

///! # vectordb_lib
///!
///! This crate provides the core functionality for `vectordb-cli`, enabling
///! semantic search over codebases using vector embeddings. It can be used
///! programmatically to index directories and perform searches.
///!
///! The main entry point is the [VectorDB] struct.

// Main library modules
/// Contains the core vector database implementation, including indexing, searching, and data structures.
pub mod vectordb;
/// Utility functions used across the library. (Consider if this needs to be public)
mod utils;
/// Command-line interface specific logic, accessible within the crate.
#[doc(hidden)] // Hide from public library documentation
pub mod cli;

// Re-export key public types/functions for easier use
/// The main vector database instance. See [vectordb::db::VectorDB].
pub use vectordb::db::VectorDB;
/// Configuration required to initialize a [VectorDB]. See [vectordb::db::VectorDBConfig].
pub use vectordb::db::VectorDBConfig;
/// Custom Result type used throughout the library. See [vectordb::error::Result].
pub use vectordb::error::Result;
/// Custom Error enum for library operations. See [vectordb::error::VectorDBError].
pub use vectordb::error::VectorDBError;
/// Represents a single search result item. See [vectordb::search::result::SearchResult].
pub use vectordb::search::result::SearchResult;
/// Statistics about the database state. See [vectordb::db::DBStats].
pub use vectordb::db::DBStats;
/// Specifies the type of embedding model used. See [vectordb::embedding::EmbeddingModelType].
pub use vectordb::embedding::EmbeddingModelType;

// Consider if hnsw types like HNSWConfig should be public if users need to configure it deeply
// pub use vectordb::hnsw::{HNSWConfig, HNSWStats};
