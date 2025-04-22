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

// Public modules that can be used when importing the crate
pub mod cli;
// pub mod client; // Removed - likely refers to workspace crate
// pub mod config; // Moved to vectordb-core
pub mod edit;
pub mod git;
pub mod syntax;
pub mod utils;
pub mod vectordb;

// Re-export minimal set of items for CLI usage
// pub use config::{AppConfig, load_config}; // Moved to vectordb-core re-export

// Use the re-exports from vectordb_core (with underscore)
pub use vectordb_core::{AppConfig, load_config};
