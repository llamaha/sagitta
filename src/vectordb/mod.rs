//!
//! Core vector database operations, including embedding handling, indexing, and search structures.

pub mod error;
pub mod embedding;
pub mod provider;
pub mod search;
pub mod snippet_extractor;
pub mod embedding_logic;
pub mod indexing;
mod cache;

// Re-export key types/structs needed elsewhere
pub use crate::vectordb::error::{Result, VectorDBError};

// Optional: Re-export key types if desired at the vectordb level
// pub use db::VectorDB;
// pub use error::VectorDBError;
