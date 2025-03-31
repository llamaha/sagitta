use std::path::PathBuf;
use thiserror::Error;
use std::io;
use syn;
use anyhow;

/// Result type for VectorDB operations
pub type Result<T> = std::result::Result<T, VectorDBError>;

/// Errors that can occur in the VectorDB system
#[derive(Error, Debug)]
pub enum VectorDBError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    
    #[error("Failed to read file {path}: {source}")]
    FileReadError {
        path: PathBuf,
        source: io::Error,
    },
    
    #[error("Failed to write file {path}: {source}")]
    FileWriteError {
        path: PathBuf,
        source: io::Error,
    },
    
    #[error("Failed to create directory {path}: {source}")]
    DirectoryCreationError {
        path: PathBuf,
        source: io::Error,
    },
    
    #[error("Failed to access file metadata for {path}: {source}")]
    MetadataError {
        path: PathBuf,
        source: io::Error,
    },
    
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
    
    #[error("Syntax parsing error: {0}")]
    SyntaxParsingError(String),
    
    #[error("Code analysis error: {0}")]
    CodeAnalysisError(String),
    
    #[error("General error: {0}")]
    GeneralError(String),
}

/// Conversion from syn::Error
impl From<syn::Error> for VectorDBError {
    fn from(error: syn::Error) -> Self {
        VectorDBError::SyntaxParsingError(error.to_string())
    }
}

/// Conversion from anyhow::Error
impl From<anyhow::Error> for VectorDBError {
    fn from(error: anyhow::Error) -> Self {
        VectorDBError::HNSWError(error.to_string())
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