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

// Public modules (essential for CLI/server usage)
pub mod cli;      // CLI commands and argument handling
pub mod config;   // Configuration management

// Only include server module when the server feature is enabled
#[cfg(feature = "server")]
pub mod server;   // gRPC server implementation

// Internal implementation modules, made public for testing
pub mod syntax;   // Code parsing with tree-sitter
mod vectordb;     // Vector database and embedding operations 
mod utils;        // Utility functions

// Re-export minimal set of items for CLI usage
pub use config::{AppConfig, load_config};
