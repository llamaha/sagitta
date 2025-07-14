use thiserror::Error;

pub type Result<T> = std::result::Result<T, SagittaError>;

#[derive(Error, Debug)]
pub enum SagittaError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),
    
    #[error("Git error: {0}")]
    GitError(#[from] git2::Error),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("TOML error: {0}")]
    TomlError(#[from] toml::de::Error),
    
    #[error("TOML serialization error: {0}")]
    TomlSerError(#[from] toml::ser::Error),
    
    #[error("Anyhow error: {0}")]
    AnyhowError(#[from] anyhow::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}