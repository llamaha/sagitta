#![allow(dead_code)] // Allow dead code for now

//! # vectordb_lib
//!
//! This crate provides the core functionality for semantic code search,
//! powering the `vectordb-cli` tool. It enables indexing codebases
//! and performing semantic searches using vector embeddings.
//!
//! ## Core Components
//!
//! *   **[`vectordb::embedding_logic::EmbeddingHandler`]:** Manages the loading and usage of ONNX embedding models.
//! *   **[`syntax`]:** Handles parsing source code into meaningful chunks using tree-sitter.
//! *   **[`vectordb::search`]:** Provides structures and traits related to search results.
//! *   **[`config`]:** Manages configuration loading (including ONNX paths and repository settings).
//! *   **[`vectordb::error`]:** Defines the error types used throughout the library.
//!
//! ## Example Usage (Conceptual)
//! 
//! ```rust,ignore
//! use vectordb_lib::config::AppConfig;
//! use vectordb_lib::vectordb::embedding_logic::EmbeddingHandler;
//! use vectordb_lib::vectordb::embedding::EmbeddingModelType;
//! use std::path::PathBuf;
//! 
//! // Load configuration
//! let config = AppConfig::load_or_default()?;
//! 
//! // Initialize embedding handler
//! let handler = EmbeddingHandler::new(
//!     EmbeddingModelType::Onnx, 
//!     config.onnx_model_path.map(PathBuf::from),
//!     config.onnx_tokenizer_path.map(PathBuf::from),
//! )?;
//! 
//! // Indexing and querying would involve using the handler 
//! // and potentially functions from the `syntax` module, 
//! // interacting with a Qdrant client directly.
//! // (Note: High-level index/query functions are primarily in the `cli` module for binary use)
//! 
//! ## Runtime Dependencies
//! 
//! Users of this library need to ensure that the required ONNX Runtime shared libraries
//! (`.so` on Linux, `.dylib` on macOS, `.dll` on Windows) are available at runtime.
//! This can typically be achieved by:
//! 1. Installing ONNX Runtime system-wide (see [https://onnxruntime.ai/docs/install/](https://onnxruntime.ai/docs/install/)).
//! 2. Placing the shared libraries alongside the final executable.
//! 3. Setting the appropriate environment variable (`LD_LIBRARY_PATH` on Linux, `DYLD_LIBRARY_PATH` on macOS) to point to the directory containing the shared libraries.
//! 
//! The `vectordb-cli` binary includes a build script (`build.rs`) that handles this automatically
//! for the binary itself, but this does not apply when using `vectordb_lib` as a dependency.
//! 
//! # Ok::<(), vectordb_lib::VectorDBError>(())
//! ```

// Top-level library modules
// pub mod commands;
// pub mod config; // Keep config if it's truly top-level
// pub mod errors;
// pub mod llm;
// pub mod provider;
pub mod syntax; // Keep syntax module
pub mod vectordb;
pub mod cli; // Keep cli module
pub mod config; // Keep one config module declaration
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
/// The main application configuration struct. See [config::AppConfig].
pub use config::AppConfig;
/// Loads the application configuration. See [config::load_config].
pub use config::load_config;
/// The handler for managing embedding models. See [vectordb::embedding_logic::EmbeddingHandler].
pub use vectordb::embedding_logic::EmbeddingHandler;

// Consider if hnsw types like HNSWConfig should be public if users need to configure it deeply
// pub use vectordb::hnsw::{HNSWConfig, HNSWStats};
