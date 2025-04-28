use std::io;
use std::path::PathBuf;
use thiserror::Error;
// use syn;
#[cfg(feature = "ort")]
use ort;
// use serde::ser::Error as SerError;
// use serde::de::Error as DeError;

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
    SerializationError(String),

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

    #[error("HNSW index dimension ({expected}) does not match query/data dimension ({found})")]
    DimensionMismatch { expected: usize, found: usize },

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

    #[error("Indexing error: {0}")]
    IndexingError(String),

    #[error("Directory '{0}' is not present in the index")]
    DirectoryNotIndexed(String),

    #[error("Search index not found or not built")]
    IndexNotFound,

    #[error("Operation cancelled by user")]
    OperationCancelled,

    #[error("Mutex lock error: {0}")]
    MutexLockError(String),

    /// Error originating from the Qdrant client
    #[error("Qdrant client error: {0}")]
    QdrantError(#[from] qdrant_client::QdrantError),

    /// Custom error during a Qdrant operation (e.g., unexpected response)
    #[error("Qdrant operation error: {0}")]
    QdrantOperationError(String),

    /// Error related to Git operations
    #[error("Git error: {0}")]
    GitError(#[from] git2::Error),

    /// Error when a required feature is not yet implemented
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    #[cfg(feature = "ort")]
    #[error("ONNX Runtime session error: {0}")]
    OrtSession(#[from] ort::Error),

    #[cfg(feature = "ort")]
    #[error("ONNX Runtime initialization error: {0}")]
    OrtInitialization(ort::Error),

    /// New variant for when a feature is required but not enabled
    #[error("Required feature not enabled: {0}")]
    FeatureNotEnabled(String),

    #[error("Git error: {0}")]
    GitMessageError(String),

    #[error("Config error: {0}")]
    ConfigError(String),
}

// Custom conversion from anyhow::Error to VectorDBError
// Tries to downcast to preserve the original VectorDBError type if possible.
impl From<anyhow::Error> for VectorDBError {
    fn from(err: anyhow::Error) -> Self {
        // Attempt to downcast to the original VectorDBError
        if let Some(specific_err) = err.downcast_ref::<VectorDBError>() {
            specific_err.clone() // Clone the original error if downcast succeeds
        } else {
            // Fallback: If downcast fails, wrap the error message in Other
            // This preserves the error context but loses the specific variant type.
            VectorDBError::Other(format!("{:?}", err))
        }
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
            Self::SerializationError(s) => Self::SerializationError(s.clone()),
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
            Self::DimensionMismatch { expected, found } => Self::DimensionMismatch {
                expected: *expected,
                found: *found,
            },
            Self::IndexingError(s) => Self::IndexingError(s.clone()),
            Self::DirectoryNotIndexed(s) => Self::DirectoryNotIndexed(s.clone()),
            Self::IndexNotFound => Self::IndexNotFound,
            Self::OperationCancelled => Self::OperationCancelled,
            Self::MutexLockError(s) => Self::MutexLockError(s.clone()),
            Self::QdrantError(e) => Self::Other(format!("QdrantError (cloned): {}", e)),
            Self::QdrantOperationError(s) => Self::QdrantOperationError(s.clone()),
            Self::GitError(e) => Self::Other(format!("GitError (cloned): {}", e)),
            Self::NotImplemented(s) => Self::NotImplemented(s.clone()),
            #[cfg(feature = "ort")]
            Self::OrtSession(e) => Self::Other(format!("OrtSession Error (cloned): {}", e)),
            #[cfg(feature = "ort")]
            Self::OrtInitialization(e) => Self::Other(format!("OrtInitialization Error (cloned): {}", e)),
            Self::FeatureNotEnabled(s) => Self::FeatureNotEnabled(s.clone()),
            Self::GitMessageError(s) => Self::GitMessageError(s.clone()),
            Self::ConfigError(s) => Self::ConfigError(s.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::path::PathBuf;
    use serde_json; // Ensure serde_json is available in dev-dependencies

    #[test]
    fn test_display_file_not_found() {
        let err = VectorDBError::FileNotFound("missing.txt".to_string());
        assert_eq!(err.to_string(), "File not found: missing.txt");
    }

    #[test]
    fn test_display_file_read_error() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "cannot read");
        let err = VectorDBError::FileReadError { path: PathBuf::from("secret.txt"), source: io_err };
        assert_eq!(err.to_string(), "Failed to read file secret.txt: cannot read");
    }

    #[test]
    fn test_display_file_write_error() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "cannot write");
        let err = VectorDBError::FileWriteError { path: PathBuf::from("output.log"), source: io_err };
        assert_eq!(err.to_string(), "Failed to write file output.log: cannot write");
    }

    #[test]
    fn test_display_directory_creation_error() {
        let io_err = io::Error::new(io::ErrorKind::AlreadyExists, "dir exists");
        let err = VectorDBError::DirectoryCreationError{ path: PathBuf::from("my_dir"), source: io_err };
        assert_eq!(err.to_string(), "Failed to create directory my_dir: dir exists");
    }

    #[test]
    fn test_display_metadata_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "no metadata");
        let err = VectorDBError::MetadataError{ path: PathBuf::from("no_file"), source: io_err };
        assert_eq!(err.to_string(), "Failed to access file metadata for no_file: no metadata");
    }

    #[test]
    fn test_display_serialization_error() {
        // Create a dummy value that cannot be serialized easily by default serde_json
        #[derive(Debug)] // Add Debug for easier assertion messages if needed
        struct Unserializable;
        impl serde::Serialize for Unserializable {
             // Correct the return type to match the trait
             fn serialize<S>(&self, _serializer: S) -> std::result::Result<S::Ok, S::Error>
             where S: serde::Serializer {
                 // Use the serializer's error type
                 Err(serde::ser::Error::custom("cannot serialize Unserializable"))
             }
        }
        // This should now correctly generate a serde_json::Error
        let serialization_err = serde_json::to_string(&Unserializable {}).unwrap_err();
        let err = VectorDBError::SerializationError(serialization_err.to_string());
        // Check the Display output using contains, as the exact serde_json error might vary slightly
        assert!(err.to_string().contains("cannot serialize Unserializable"));
        // Also check the error type prefix is correct
        assert!(err.to_string().starts_with("Error serializing or deserializing data:"));
    }

    #[test]
    fn test_display_deserialization_error() {
        let err = VectorDBError::DeserializationError("Failed to deserialize JSON data".to_string());
        assert_eq!(err.to_string(), "Error deserializing data: Failed to deserialize JSON data");
    }

    #[test]
    fn test_display_embedding_error() {
        let err = VectorDBError::EmbeddingError("Embedding generation failed".to_string());
        assert_eq!(err.to_string(), "Error generating embedding: Embedding generation failed");
    }

    #[test]
    fn test_display_database_error() {
        let err = VectorDBError::DatabaseError("DB connection failed".to_string());
        assert_eq!(err.to_string(), "Database error: DB connection failed");
    }

    #[test]
    fn test_display_ast_traversal_error() {
        let err = VectorDBError::ASTTraversalError("Could not traverse node".to_string());
        assert_eq!(err.to_string(), "AST traversal error: Could not traverse node");
    }

    #[test]
    fn test_display_invalid_parameter() {
        let err = VectorDBError::InvalidParameter("Negative count provided".to_string());
        assert_eq!(err.to_string(), "Invalid parameter: Negative count provided");
    }

    #[test]
    fn test_display_invalid_path() {
        let err = VectorDBError::InvalidPath("Path contains invalid chars".to_string());
        assert_eq!(err.to_string(), "Invalid path: Path contains invalid chars");
    }

    #[test]
    fn test_display_cache_error() {
        let err = VectorDBError::CacheError("Cache miss or invalidation".to_string());
        assert_eq!(err.to_string(), "Cache error: Cache miss or invalidation");
    }

    #[test]
    fn test_display_parser_error() {
        let err = VectorDBError::ParserError("Syntax error in code".to_string());
        assert_eq!(err.to_string(), "Parser error: Syntax error in code");
    }

    #[test]
    fn test_display_unsupported_language() {
        let err = VectorDBError::UnsupportedLanguage("Language 'Brainfuck' not supported".to_string());
        assert_eq!(err.to_string(), "Unsupported language: Language 'Brainfuck' not supported");
    }

    #[test]
    fn test_display_hnsw_error() {
        // Often wraps other errors, e.g., ONNX errors might end up here via From<anyhow::Error>
        let err = VectorDBError::HNSWError("HNSW search failed internally".to_string());
        assert_eq!(err.to_string(), "HNSW index error: HNSW search failed internally");
    }

    #[test]
    fn test_display_dimension_mismatch() {
        let err = VectorDBError::DimensionMismatch { expected: 768, found: 384 };
        assert_eq!(err.to_string(), "HNSW index dimension (768) does not match query/data dimension (384)");
    }

    #[test]
    fn test_display_io_error() {
        let io_err = io::Error::new(io::ErrorKind::TimedOut, "connection timed out");
        // Uses From<io::Error>
        let err = VectorDBError::from(io_err); // Or VectorDBError::IOError(io_err)
        assert_eq!(err.to_string(), "IO error: connection timed out");
    }

    #[test]
    fn test_display_code_analysis_error() {
        let err = VectorDBError::CodeAnalysisError("Failed to analyze symbols".to_string());
        assert_eq!(err.to_string(), "Code analysis error: Failed to analyze symbols");
    }

    #[test]
    fn test_display_general_error() {
        let err = VectorDBError::GeneralError("An unexpected issue occurred".to_string());
        assert_eq!(err.to_string(), "General error: An unexpected issue occurred");
    }

    #[test]
    fn test_display_directory_not_found() {
        let err = VectorDBError::DirectoryNotFound("/non/existent/path".to_string());
        assert_eq!(err.to_string(), "Directory not found: /non/existent/path");
    }

    #[test]
    fn test_display_repository_error() {
        let err = VectorDBError::RepositoryError("Git operation failed".to_string());
        assert_eq!(err.to_string(), "Repository error: Git operation failed");
    }

    #[test]
    fn test_display_repository_not_found() {
        let err = VectorDBError::RepositoryNotFound("Repo at path not found".to_string());
        assert_eq!(err.to_string(), "Repository not found: Repo at path not found");
    }

    #[test]
    fn test_display_search_error() {
        let err = VectorDBError::SearchError("Search query was invalid".to_string());
        assert_eq!(err.to_string(), "Search error: Search query was invalid");
    }

    #[test]
    fn test_display_other_error() {
        let err = VectorDBError::Other("Some other specific error".to_string());
        assert_eq!(err.to_string(), "Other error: Some other specific error");
    }

    #[test]
    fn test_display_configuration_error() {
        let err = VectorDBError::ConfigurationError("Missing API key".to_string());
        assert_eq!(err.to_string(), "Configuration error: Missing API key");
    }

    #[test]
    fn test_display_indexing_error() {
        let err = VectorDBError::IndexingError("Failed to add document to index".to_string());
        assert_eq!(err.to_string(), "Indexing error: Failed to add document to index");
    }

    #[test]
    fn test_display_directory_not_indexed() {
        let err = VectorDBError::DirectoryNotIndexed("src/utils".to_string());
        assert_eq!(err.to_string(), "Directory 'src/utils' is not present in the index");
    }

    #[test]
    fn test_display_index_not_found() {
        let err = VectorDBError::IndexNotFound;
        assert_eq!(err.to_string(), "Search index not found or not built");
    }

    #[test]
    fn test_display_operation_cancelled() {
        let err = VectorDBError::OperationCancelled;
        assert_eq!(err.to_string(), "Operation cancelled by user");
    }

    #[test]
    fn test_display_mutex_lock_error() {
        let err = VectorDBError::MutexLockError("Failed to acquire lock".to_string());
        assert_eq!(err.to_string(), "Mutex lock error: Failed to acquire lock");
    }

    // Example test for Clone - testing one variant is usually enough
    #[test]
    fn test_error_cloning() {
        let original = VectorDBError::FileNotFound("clone_test.txt".to_string());
        let cloned = original.clone();
        assert_eq!(original.to_string(), cloned.to_string());
        // Ensure it's a deep clone if necessary (though for String it doesn't matter much)
        if let VectorDBError::FileNotFound(s1) = original {
            if let VectorDBError::FileNotFound(s2) = cloned {
                assert_eq!(s1, s2);
                assert_ne!(s1.as_ptr(), s2.as_ptr()); // Check they aren't the same string instance in memory
            } else {
                panic!("Cloned error is not FileNotFound");
            }
        } else {
            panic!("Original error is not FileNotFound");
        }
    }

    // Example test for From<anyhow::Error>
    #[test]
    fn test_from_anyhow_error() {
        let original_err = VectorDBError::HNSWError("Test HNSW error".to_string());
        let anyhow_err = anyhow::Error::new(original_err.clone()); // Clone the original error
        let vectordb_err: VectorDBError = anyhow_err.into();
        // Check if the downcasted error is the correct variant
        assert!(
            matches!(vectordb_err, VectorDBError::HNSWError(_)),
            "Expected HNSWError, got {:?}", // Add debug print on failure
            vectordb_err
        );
        // Optionally, check if the content is preserved
        if let VectorDBError::HNSWError(msg) = vectordb_err {
            assert_eq!(msg, "Test HNSW error");
        }
    }

    #[derive(Error, Debug, Clone)]
    #[error("Mock specific error: {0}")]
    struct MockError(String);

    impl PartialEq for MockError {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    #[test]
    fn test_from_anyhow_error_other() {
        let original_err = VectorDBError::HNSWError("Test HNSW error".to_string());
        let anyhow_err = anyhow::Error::new(original_err.clone());
        let vectordb_err: VectorDBError = anyhow_err.into();
        
        // Assert that the downcasted error is the correct variant
        assert!(
            matches!(vectordb_err, VectorDBError::HNSWError(_)), 
            "Expected HNSWError, got {:?}", 
            vectordb_err
        );
        
        // Additionally, check if the content matches
        if let VectorDBError::HNSWError(msg) = vectordb_err {
            assert_eq!(msg, "Test HNSW error");
        } else {
            // This branch shouldn't be reached if the matches! assertion passed,
            // but it's good practice for completeness or if matches! is removed.
            panic!("Error variant was not HNSWError after checking with matches!");
        }
    }
}
