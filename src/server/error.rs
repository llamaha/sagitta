use thiserror::Error;
use tonic::Status;
use std::io;

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
}

/// Server-specific result type
pub type Result<T> = std::result::Result<T, ServerError>;

/// Convert ServerError to tonic::Status for gRPC responses
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

/// Convert tonic transport errors to ServerError
impl From<tonic::transport::Error> for ServerError {
    fn from(err: tonic::transport::Error) -> Self {
        ServerError::ServiceError(format!("Transport error: {}", err))
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