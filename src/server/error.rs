#[cfg(feature = "server")]
use thiserror::Error;
#[cfg(feature = "server")]
use tonic::Status;
use std::io;
use std::net;

/// Server-specific error type
#[derive(Error, Debug)]
pub enum ServerError {
    #[error("gRPC service error: {0}")]
    ServiceError(String),
    
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Authentication failed: {0}")]
    Authentication(String),
    
    #[error("Authorization failed: {0}")]
    Authorization(String),
    
    #[error("Qdrant client error: {0}")]
    QdrantError(String),
    
    #[error("Internal server error: {0}")]
    Internal(String),
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Repository error: {0}")]
    RepositoryError(String),
    
    #[error("Embedding error: {0}")]
    EmbeddingError(String),
    
    #[error("Reflection error: {0}")]
    Reflection(String),
    
    #[error("Transport error: {0}")]
    Transport(tonic::transport::Error),
}

/// Server-specific result type
pub type Result<T> = std::result::Result<T, ServerError>;

#[cfg(feature = "server")]
impl From<ServerError> for Status {
    fn from(err: ServerError) -> Self {
        match err {
            ServerError::ServiceError(msg) => Status::internal(msg),
            ServerError::Io(e) => Status::internal(format!("I/O error: {}", e)),
            ServerError::Configuration(msg) => Status::failed_precondition(msg),
            ServerError::Authentication(msg) => Status::unauthenticated(msg),
            ServerError::Authorization(msg) => Status::permission_denied(msg),
            ServerError::QdrantError(msg) => Status::internal(format!("Database error: {}", msg)),
            ServerError::Internal(msg) => Status::internal(msg),
            ServerError::InvalidRequest(msg) => Status::invalid_argument(msg),
            ServerError::RepositoryError(msg) => Status::failed_precondition(msg),
            ServerError::EmbeddingError(msg) => Status::internal(format!("Embedding error: {}", msg)),
            ServerError::Reflection(msg) => Status::internal(msg),
            ServerError::Transport(e) => Status::internal(format!("Transport error: {}", e)),
        }
    }
}

/// Convert generic errors to ServerError
impl From<anyhow::Error> for ServerError {
    fn from(err: anyhow::Error) -> Self {
        ServerError::Internal(format!("{}", err))
    }
}

/// Convert Qdrant client errors to ServerError
impl From<qdrant_client::QdrantError> for ServerError {
    fn from(err: qdrant_client::QdrantError) -> Self {
        ServerError::QdrantError(format!("{}", err))
    }
}

#[cfg(feature = "server")]
impl From<tonic::transport::Error> for ServerError {
    fn from(err: tonic::transport::Error) -> Self {
        ServerError::Transport(err)
    }
}

/// Convert Rust vector database errors to ServerError
impl From<crate::vectordb::error::VectorDBError> for ServerError {
    fn from(err: crate::vectordb::error::VectorDBError) -> Self {
        match err {
            crate::vectordb::error::VectorDBError::QdrantError(e) => 
                ServerError::QdrantError(format!("{}", e)),
            crate::vectordb::error::VectorDBError::EmbeddingError(e) => 
                ServerError::EmbeddingError(format!("{}", e)),
            _ => ServerError::Internal(format!("{}", err)),
        }
    }
}

#[cfg(feature = "server")]
impl From<tonic_reflection::server::Error> for ServerError {
    fn from(err: tonic_reflection::server::Error) -> Self {
        ServerError::Reflection(err.to_string()) // Or a more specific variant if desired
    }
}

// Add From trait implementation for AddrParseError
impl From<net::AddrParseError> for ServerError {
    fn from(err: net::AddrParseError) -> Self {
        ServerError::Configuration(format!("Invalid socket address: {}", err))
    }
}