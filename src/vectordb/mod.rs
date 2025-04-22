//!
//! Core vector database operations, including embedding handling, indexing, and search structures.

pub mod embedding;
pub mod provider;
pub mod embedding_logic;
// pub mod error; // Moved to vectordb_core
pub mod search;
pub mod snippet_extractor;
pub mod indexing;
mod cache;

// Add the new trait module
pub mod qdrant_client;

// Re-export key types/structs needed elsewhere
pub use vectordb_core::error::{Result as VectorDBResult, VectorDBError};

// Optional: Re-export key types if desired at the vectordb level
// pub use db::VectorDB;
// pub use error::VectorDBError;

// Re-export key components from submodules
pub use embedding::EmbeddingModel;
