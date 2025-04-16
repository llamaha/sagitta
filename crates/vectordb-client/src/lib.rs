//! # vectordb-client
//!
//! Client library for interacting with the VectorDB semantic code search service.
//! This crate provides a gRPC client for connecting to the VectorDB server and
//! performing operations such as indexing code, searching, and repository management.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use vectordb_client::VectorDBClient;
//! use std::error::Error;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn Error>> {
//!     // Create a client with default configuration (localhost:50051)
//!     let mut client = VectorDBClient::default().await?;
//!     
//!     // Get server info
//!     let server_info = client.get_server_info().await?;
//!     println!("Connected to server version: {}", server_info.version);
//!     
//!     // List collections
//!     let collections = client.list_collections().await?;
//!     println!("Available collections:");
//!     for collection in collections.collections {
//!         println!("  - {}", collection);
//!     }
//!     
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod config;
pub mod client;

pub use client::grpc::VectorDBClient;
pub use config::ClientConfig;
pub use error::{ClientError, Result};

// Re-export messages from the proto crate for convenience
pub use vectordb_proto::vectordb::{
    Empty, ServerInfo, StatusResponse, CreateCollectionRequest,
    CollectionRequest, ListCollectionsResponse, QueryRequest, 
    QueryResponse, IndexFilesRequest, IndexResponse,
    AddRepositoryRequest, RepositoryRequest, RemoveRepositoryRequest,
    SyncRepositoryRequest, UseBranchRequest, ListRepositoriesResponse,
    SearchResult,
}; 