use thiserror::Error;
use std::io;

/// Client-specific error type
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("gRPC client error: {0}")]
    Transport(String),
    
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Authentication failed: {0}")]
    Authentication(String),
    
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Request failed: {0}")]
    RequestFailed(String),
    
    #[error("Server returned error: {0}")]
    ServerError(String),
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("Feature not available: {0}")]
    FeatureNotAvailable(String),
    
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    
    #[error("Service unavailable: {0}")]
    Unavailable(String),
}

/// Client-specific result type
pub type Result<T> = std::result::Result<T, ClientError>;

impl From<tonic::Status> for ClientError {
    fn from(status: tonic::Status) -> Self {
        match status.code() {
            tonic::Code::Unauthenticated => ClientError::Authentication(status.message().to_string()),
            tonic::Code::PermissionDenied => ClientError::Authentication(status.message().to_string()),
            tonic::Code::Unavailable => ClientError::Connection(status.message().to_string()),
            tonic::Code::Internal => ClientError::ServerError(status.message().to_string()),
            tonic::Code::InvalidArgument => ClientError::RequestFailed(status.message().to_string()),
            _ => ClientError::RequestFailed(format!("{}: {}", status.code(), status.message())),
        }
    }
}

impl From<tonic::transport::Error> for ClientError {
    fn from(err: tonic::transport::Error) -> Self {
        ClientError::Transport(format!("Transport error: {}", err))
    }
} 