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

// Public modules that can be used when importing the crate
pub mod config;
pub mod syntax;
pub mod utils;
pub mod vectordb;

// Making cli and git public to allow tests to access them
pub mod cli;
pub mod git;

// Only include server module when the "server" feature is enabled
#[cfg(feature = "server")]
pub mod server;

// Re-export minimal set of items for CLI usage
pub use config::{AppConfig, load_config};
