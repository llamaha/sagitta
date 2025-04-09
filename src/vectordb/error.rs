use std::io;
use std::path::PathBuf;
use thiserror::Error;
// use syn;
use anyhow;

/// Result type for VectorDB operations
pub type Result<T> = std::result::Result<T, VectorDBError>;

/// Errors that can occur in the VectorDB system
#[derive(Error, Debug)]
pub enum VectorDBError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Failed to read file {path}: {source}")]
    FileReadError { path: PathBuf, source: io::Error },

    #[error("Failed to write file {path}: {source}")]
    FileWriteError { path: PathBuf, source: io::Error },

    #[error("Failed to create directory {path}: {source}")]
    DirectoryCreationError { path: PathBuf, source: io::Error },

    #[error("Failed to access file metadata for {path}: {source}")]
    MetadataError { path: PathBuf, source: io::Error },

    #[error("Error serializing or deserializing data: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Error generating embedding: {0}")]
    EmbeddingError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("AST traversal error: {0}")]
    ASTTraversalError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Parser error: {0}")]
    ParserError(String),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("HNSW index error: {0}")]
    HNSWError(String),

    #[error("IO error: {0}")]
    IOError(#[from] io::Error),

    #[error("Code analysis error: {0}")]
    CodeAnalysisError(String),

    #[error("General error: {0}")]
    GeneralError(String),

    #[error("Directory not found: {0}")]
    DirectoryNotFound(String),

    #[error("Repository error: {0}")]
    RepositoryError(String),

    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("Error deserializing data: {0}")]
    DeserializationError(String),

    #[error("Search error: {0}")]
    SearchError(String),

    #[error("Other error: {0}")]
    Other(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

/// Conversion from anyhow::Error
impl From<anyhow::Error> for VectorDBError {
    fn from(error: anyhow::Error) -> Self {
        VectorDBError::HNSWError(error.to_string())
    }
}

// Add Clone implementation for VectorDBError to support parallel processing
impl Clone for VectorDBError {
    fn clone(&self) -> Self {
        match self {
            Self::FileNotFound(s) => Self::FileNotFound(s.clone()),
            Self::FileReadError { path, source } => Self::FileReadError {
                path: path.clone(),
                source: io::Error::new(source.kind(), source.to_string()),
            },
            Self::FileWriteError { path, source } => Self::FileWriteError {
                path: path.clone(),
                source: io::Error::new(source.kind(), source.to_string()),
            },
            Self::DirectoryCreationError { path, source } => Self::DirectoryCreationError {
                path: path.clone(),
                source: io::Error::new(source.kind(), source.to_string()),
            },
            Self::MetadataError { path, source } => Self::MetadataError {
                path: path.clone(),
                source: io::Error::new(source.kind(), source.to_string()),
            },
            // Create new serialization error with the string representation
            Self::SerializationError(e) => Self::SerializationError(
                serde_json::from_str::<serde_json::Value>(&format!("\"{}\"", e)).unwrap_err(),
            ),
            Self::EmbeddingError(s) => Self::EmbeddingError(s.clone()),
            Self::DatabaseError(s) => Self::DatabaseError(s.clone()),
            Self::ASTTraversalError(s) => Self::ASTTraversalError(s.clone()),
            Self::InvalidParameter(s) => Self::InvalidParameter(s.clone()),
            Self::InvalidPath(s) => Self::InvalidPath(s.clone()),
            Self::CacheError(s) => Self::CacheError(s.clone()),
            Self::ParserError(s) => Self::ParserError(s.clone()),
            Self::UnsupportedLanguage(s) => Self::UnsupportedLanguage(s.clone()),
            Self::HNSWError(s) => Self::HNSWError(s.clone()),
            Self::IOError(e) => Self::IOError(io::Error::new(e.kind(), e.to_string())),
            Self::CodeAnalysisError(s) => Self::CodeAnalysisError(s.clone()),
            Self::GeneralError(s) => Self::GeneralError(s.clone()),
            Self::DirectoryNotFound(s) => Self::DirectoryNotFound(s.clone()),
            Self::RepositoryError(s) => Self::RepositoryError(s.clone()),
            Self::RepositoryNotFound(s) => Self::RepositoryNotFound(s.clone()),
            Self::DeserializationError(s) => Self::DeserializationError(s.clone()),
            Self::SearchError(s) => Self::SearchError(s.clone()),
            Self::Other(s) => Self::Other(s.clone()),
            Self::ConfigurationError(s) => Self::ConfigurationError(s.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "test file not found");
        let db_error = VectorDBError::from(io_error);

        match db_error {
            VectorDBError::IOError(_) => assert!(true),
            _ => assert!(false, "Expected IOError conversion"),
        }

        // Test manual creation for specific error kinds
        let db_error = VectorDBError::FileNotFound("test file not found".to_string());

        match db_error {
            VectorDBError::FileNotFound(_) => assert!(true),
            _ => assert!(false, "Expected FileNotFound type"),
        }
    }

    #[test]
    fn test_error_display() {
        let error = VectorDBError::FileNotFound("test.txt".to_string());
        assert!(error.to_string().contains("test.txt"));
    }

    #[test]
    fn test_parser_error() {
        let error = VectorDBError::ParserError("Failed to parse file".to_string());
        let err_string = error.to_string();
        assert!(err_string.contains("Failed to parse file"));
    }
}
