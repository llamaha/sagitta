#![allow(dead_code)] // Allow dead code for now

//! # vectordb_lib
//!
//! This crate provides the core functionality for `vectordb-cli`, enabling
//! semantic search over codebases using vector embeddings. It can be used
//! programmatically to index directories and perform searches.
//!
//! The main entry point is the [VectorDB] struct. (Note: VectorDB struct might not exist yet)

// Top-level library modules
// pub mod commands;
// pub mod config; // Keep config if it's truly top-level
// pub mod errors;
// pub mod llm;
// pub mod provider;
pub mod syntax; // Keep syntax module
pub mod vectordb;
pub mod cli; // Keep cli module
pub mod config; // Keep config module (appears twice, likely a merge artifact, keep one)
mod utils; // Keep utils private for now

// Re-export key public types/functions for easier use
/// Custom Result type used throughout the library. See [vectordb::error::Result].
pub use vectordb::error::Result;
/// Custom Error enum for library operations. See [vectordb::error::VectorDBError].
pub use vectordb::error::VectorDBError;
/// Represents a single search result item. See [vectordb::search::result::SearchResult].
pub use vectordb::search::result::SearchResult;
/// Specifies the type of embedding model used. See [vectordb::embedding::EmbeddingModelType].
pub use vectordb::embedding::EmbeddingModelType;

// Consider if hnsw types like HNSWConfig should be public if users need to configure it deeply
// pub use vectordb::hnsw::{HNSWConfig, HNSWStats};
