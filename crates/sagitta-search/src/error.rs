use std::io;
use std::path::PathBuf;
use thiserror::Error;
// use syn;
#[cfg(feature = "ort")]
use ort;
// use serde::ser::Error as SerError;
// use serde::de::Error as DeError;

/// Result type for Sagitta operations
pub type Result<T> = std::result::Result<T, SagittaError>;

/// Errors that can occur in the Sagitta system
#[derive(Error, Debug)]
pub enum SagittaError {
    #[error("File not found: {0}")]
    /// Error indicating that a specified file was not found at the given path.
    FileNotFound(String),

    #[error("Failed to read file {path}: {source}")]
    /// Error indicating a failure to read from a file.
    FileReadError {
        /// The path to the file that could not be read.
        path: PathBuf, 
        /// The underlying I/O error that occurred.
        source: io::Error 
    },

    #[error("Failed to write file {path}: {source}")]
    /// Error indicating a failure to write to a file.
    FileWriteError { 
        /// The path to the file that could not be written.
        path: PathBuf, 
        /// The underlying I/O error that occurred.
        source: io::Error 
    },

    #[error("Failed to create directory {path}: {source}")]
    /// Error indicating a failure to create a directory.
    DirectoryCreationError { 
        /// The path to the directory that could not be created.
        path: PathBuf, 
        /// The underlying I/O error that occurred.
        source: io::Error 
    },

    #[error("Failed to access file metadata for {path}: {source}")]
    /// Error indicating a failure to access file metadata (e.g., size, modification time).
    MetadataError { 
        /// The path to the file whose metadata could not be accessed.
        path: PathBuf, 
        /// The underlying I/O error that occurred.
        source: io::Error 
    },

    #[error("Error serializing or deserializing data: {0}")]
    /// Error related to data serialization (e.g., to JSON, TOML) or deserialization.
    SerializationError(String),

    #[error("Error generating embedding: {0}")]
    /// Error occurring during the generation of text embeddings.
    EmbeddingError(String),

    #[error("Database error: {0}")]
    /// Generic error related to database operations (e.g., Qdrant, SQLite).
    DatabaseError(String),

    #[error("AST traversal error: {0}")]
    /// Error occurring during the traversal or processing of an Abstract Syntax Tree (AST).
    ASTTraversalError(String),

    #[error("Invalid parameter: {0}")]
    /// Error indicating that an invalid parameter was provided to a function or method.
    InvalidParameter(String),

    #[error("Invalid path: {0}")]
    /// Error indicating that a file or directory path is invalid.
    InvalidPath(String),

    #[error("Cache error: {0}")]
    /// Error related to caching operations (e.g., read/write failures, cache misses).
    CacheError(String),

    #[error("Parser error: {0}")]
    /// Error occurring during parsing of source code or other structured text.
    ParserError(String),

    #[error("Unsupported language: {0}")]
    /// Error indicating that a specified programming language is not supported.
    UnsupportedLanguage(String),

    #[error("HNSW index error: {0}")]
    /// Error related to HNSW (Hierarchical Navigable Small World) indexing or search.
    HNSWError(String),

    #[error("HNSW index dimension ({expected}) does not match query/data dimension ({found})")]
    /// Error indicating a mismatch between expected and actual embedding dimensions.
    DimensionMismatch { 
        /// The expected embedding dimension.
        expected: usize, 
        /// The found embedding dimension.
        found: usize 
    },

    #[error("IO error: {0}")]
    /// A generic I/O error, often wrapping `std::io::Error`.
    IOError(#[from] io::Error),

    #[error("Code analysis error: {0}")]
    /// Error occurring during static analysis or parsing of code.
    CodeAnalysisError(String),

    #[error("General error: {0}")]
    /// A general-purpose error type for miscellaneous errors.
    GeneralError(String),

    #[error("Directory not found: {0}")]
    /// Error indicating that a specified directory was not found.
    DirectoryNotFound(String),

    #[error("Repository error: {0}")]
    /// Error related to Git repository operations (e.g., clone, pull, checkout).
    RepositoryError(String),

    #[error("Repository not found: {0}")]
    /// Error indicating that a configured repository could not be found.
    RepositoryNotFound(String),

    #[error("Error deserializing data: {0}")]
    /// Error occurring during data deserialization (distinct from `SerializationError` if needed for clarity).
    DeserializationError(String),

    #[error("Search error: {0}")]
    /// Error occurring during a search or query operation.
    SearchError(String),

    #[error("Other error: {0}")]
    /// A catch-all error type for unclassified errors, often converted from `anyhow::Error`.
    Other(String),

    #[error("Configuration error: {0}")]
    /// Error related to application configuration (e.g., missing settings, invalid values).
    ConfigurationError(String),

    #[error("Indexing error: {0}")]
    /// Error occurring during the indexing process.
    IndexingError(String),

    /// Directory is not present in the index.
    #[error("Directory '{0}' is not present in the index")]
    DirectoryNotIndexed(String),

    /// Search index not found or not built.
    #[error("Search index not found or not built")]
    IndexNotFound,

    /// Operation cancelled by user.
    #[error("Operation cancelled by user")]
    OperationCancelled,

    /// Mutex lock error.
    #[error("Mutex lock error: {0}")]
    MutexLockError(String),

    /// Error originating from the Qdrant client
    #[error("Qdrant client error: {0}")]
    QdrantError(Box<qdrant_client::QdrantError>),

    /// Custom error during a Qdrant operation (e.g., unexpected response)
    #[error("Qdrant operation error: {0}")]
    QdrantOperationError(String),

    /// Error related to Git operations
    #[error("Git error: {0}")]
    GitError(Box<git2::Error>),

    /// Error when a required feature is not yet implemented
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    #[cfg(feature = "ort")]
    /// ONNX Runtime session error.
    #[error("ONNX Runtime session error: {0}")]
    OrtSession(#[from] ort::Error),

    #[cfg(feature = "ort")]
    /// ONNX Runtime initialization error.
    #[error("ONNX Runtime initialization error: {0}")]
    OrtInitialization(ort::Error),

    /// New variant for when a feature is required but not enabled
    #[error("Required feature not enabled: {0}")]
    FeatureNotEnabled(String),

    /// Git error with a custom message.
    #[error("Git error: {0}")]
    GitMessageError(String),

    /// Configuration error with a custom message.
    #[error("Config error: {0}")]
    ConfigError(String),
}

// Manual From implementation for QdrantError
impl From<qdrant_client::QdrantError> for SagittaError {
    fn from(err: qdrant_client::QdrantError) -> Self {
        SagittaError::QdrantError(Box::new(err))
    }
}

// Manual From implementation for git2::Error
impl From<git2::Error> for SagittaError {
    fn from(err: git2::Error) -> Self {
        SagittaError::GitError(Box::new(err))
    }
}

// Custom conversion from anyhow::Error to SagittaError
// Tries to downcast to preserve the original SagittaError type if possible.
impl From<anyhow::Error> for SagittaError {
    fn from(err: anyhow::Error) -> Self {
        // First, try to downcast to SagittaError to preserve the original error type
        if let Some(sagitta_err) = err.downcast_ref::<SagittaError>() {
            return sagitta_err.clone();
        }
        
        // Check if the error is a specific type we want to handle differently
        if let Some(io_err) = err.downcast_ref::<io::Error>() {
            SagittaError::IOError(io_err.kind().into())
        } else {
            SagittaError::Other(err.to_string())
        }
    }
}

// Add conversion from SagittaEmbedError to SagittaError
impl From<sagitta_embed::error::SagittaEmbedError> for SagittaError {
    fn from(err: sagitta_embed::error::SagittaEmbedError) -> Self {
        SagittaError::EmbeddingError(err.to_string())
    }
}

// Add Clone implementation for SagittaError to support parallel processing
impl Clone for SagittaError {
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
            Self::QdrantError(e) => Self::Other(format!("QdrantError (cloned): {e}")),
            Self::QdrantOperationError(s) => Self::QdrantOperationError(s.clone()),
            Self::GitError(e) => Self::Other(format!("GitError (cloned): {e}")),
            Self::NotImplemented(s) => Self::NotImplemented(s.clone()),
            #[cfg(feature = "ort")]
            Self::OrtSession(e) => Self::Other(format!("OrtSession Error (cloned): {e}")),
            #[cfg(feature = "ort")]
            Self::OrtInitialization(e) => Self::Other(format!("OrtInitialization Error (cloned): {e}")),
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
        let err = SagittaError::FileNotFound("missing.txt".to_string());
        assert_eq!(err.to_string(), "File not found: missing.txt");
    }

    #[test]
    fn test_display_file_read_error() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "cannot read");
        let err = SagittaError::FileReadError { path: PathBuf::from("secret.txt"), source: io_err };
        assert_eq!(err.to_string(), "Failed to read file secret.txt: cannot read");
    }

    #[test]
    fn test_display_file_write_error() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "cannot write");
        let err = SagittaError::FileWriteError { path: PathBuf::from("output.log"), source: io_err };
        assert_eq!(err.to_string(), "Failed to write file output.log: cannot write");
    }

    #[test]
    fn test_display_directory_creation_error() {
        let io_err = io::Error::new(io::ErrorKind::AlreadyExists, "dir exists");
        let err = SagittaError::DirectoryCreationError{ path: PathBuf::from("my_dir"), source: io_err };
        assert_eq!(err.to_string(), "Failed to create directory my_dir: dir exists");
    }

    #[test]
    fn test_display_metadata_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "no metadata");
        let err = SagittaError::MetadataError{ path: PathBuf::from("no_file"), source: io_err };
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
        let err = SagittaError::SerializationError(serialization_err.to_string());
        // Check the Display output using contains, as the exact serde_json error might vary slightly
        assert!(err.to_string().contains("cannot serialize Unserializable"));
        // Also check the error type prefix is correct
        assert!(err.to_string().starts_with("Error serializing or deserializing data:"));
    }

    #[test]
    fn test_display_deserialization_error() {
        let err = SagittaError::DeserializationError("Failed to deserialize JSON data".to_string());
        assert_eq!(err.to_string(), "Error deserializing data: Failed to deserialize JSON data");
    }

    #[test]
    fn test_display_embedding_error() {
        let err = SagittaError::EmbeddingError("Embedding generation failed".to_string());
        assert_eq!(err.to_string(), "Error generating embedding: Embedding generation failed");
    }

    #[test]
    fn test_display_database_error() {
        let err = SagittaError::DatabaseError("DB connection failed".to_string());
        assert_eq!(err.to_string(), "Database error: DB connection failed");
    }

    #[test]
    fn test_display_ast_traversal_error() {
        let err = SagittaError::ASTTraversalError("Could not traverse node".to_string());
        assert_eq!(err.to_string(), "AST traversal error: Could not traverse node");
    }

    #[test]
    fn test_display_invalid_parameter() {
        let err = SagittaError::InvalidParameter("Negative count provided".to_string());
        assert_eq!(err.to_string(), "Invalid parameter: Negative count provided");
    }

    #[test]
    fn test_display_invalid_path() {
        let err = SagittaError::InvalidPath("Path contains invalid chars".to_string());
        assert_eq!(err.to_string(), "Invalid path: Path contains invalid chars");
    }

    #[test]
    fn test_display_cache_error() {
        let err = SagittaError::CacheError("Cache miss or invalidation".to_string());
        assert_eq!(err.to_string(), "Cache error: Cache miss or invalidation");
    }

    #[test]
    fn test_display_parser_error() {
        let err = SagittaError::ParserError("Syntax error in code".to_string());
        assert_eq!(err.to_string(), "Parser error: Syntax error in code");
    }

    #[test]
    fn test_display_unsupported_language() {
        let err = SagittaError::UnsupportedLanguage("Language 'Brainfuck' not supported".to_string());
        assert_eq!(err.to_string(), "Unsupported language: Language 'Brainfuck' not supported");
    }

    #[test]
    fn test_display_hnsw_error() {
        // Often wraps other errors, e.g., ONNX errors might end up here via From<anyhow::Error>
        let err = SagittaError::HNSWError("HNSW search failed internally".to_string());
        assert_eq!(err.to_string(), "HNSW index error: HNSW search failed internally");
    }

    #[test]
    fn test_display_dimension_mismatch() {
        let err = SagittaError::DimensionMismatch { expected: 768, found: 384 };
        assert_eq!(err.to_string(), "HNSW index dimension (768) does not match query/data dimension (384)");
    }

    #[test]
    fn test_display_io_error() {
        let io_err = io::Error::new(io::ErrorKind::TimedOut, "connection timed out");
        // Uses From<io::Error>
        let err = SagittaError::from(io_err); // Or SagittaError::IOError(io_err)
        assert_eq!(err.to_string(), "IO error: connection timed out");
    }

    #[test]
    fn test_display_code_analysis_error() {
        let err = SagittaError::CodeAnalysisError("Failed to analyze symbols".to_string());
        assert_eq!(err.to_string(), "Code analysis error: Failed to analyze symbols");
    }

    #[test]
    fn test_display_general_error() {
        let err = SagittaError::GeneralError("An unexpected issue occurred".to_string());
        assert_eq!(err.to_string(), "General error: An unexpected issue occurred");
    }

    #[test]
    fn test_display_directory_not_found() {
        let err = SagittaError::DirectoryNotFound("/non/existent/path".to_string());
        assert_eq!(err.to_string(), "Directory not found: /non/existent/path");
    }

    #[test]
    fn test_display_repository_error() {
        let err = SagittaError::RepositoryError("Git operation failed".to_string());
        assert_eq!(err.to_string(), "Repository error: Git operation failed");
    }

    #[test]
    fn test_display_repository_not_found() {
        let err = SagittaError::RepositoryNotFound("Repo at path not found".to_string());
        assert_eq!(err.to_string(), "Repository not found: Repo at path not found");
    }

    #[test]
    fn test_display_search_error() {
        let err = SagittaError::SearchError("Search query was invalid".to_string());
        assert_eq!(err.to_string(), "Search error: Search query was invalid");
    }

    #[test]
    fn test_display_other_error() {
        let err = SagittaError::Other("Some other specific error".to_string());
        assert_eq!(err.to_string(), "Other error: Some other specific error");
    }

    #[test]
    fn test_display_configuration_error() {
        let err = SagittaError::ConfigurationError("Missing API key".to_string());
        assert_eq!(err.to_string(), "Configuration error: Missing API key");
    }

    #[test]
    fn test_display_indexing_error() {
        let err = SagittaError::IndexingError("Failed to add document to index".to_string());
        assert_eq!(err.to_string(), "Indexing error: Failed to add document to index");
    }

    #[test]
    fn test_display_directory_not_indexed() {
        let err = SagittaError::DirectoryNotIndexed("src/utils".to_string());
        assert_eq!(err.to_string(), "Directory 'src/utils' is not present in the index");
    }

    #[test]
    fn test_display_index_not_found() {
        let err = SagittaError::IndexNotFound;
        assert_eq!(err.to_string(), "Search index not found or not built");
    }

    #[test]
    fn test_display_operation_cancelled() {
        let err = SagittaError::OperationCancelled;
        assert_eq!(err.to_string(), "Operation cancelled by user");
    }

    #[test]
    fn test_display_mutex_lock_error() {
        let err = SagittaError::MutexLockError("Failed to acquire lock".to_string());
        assert_eq!(err.to_string(), "Mutex lock error: Failed to acquire lock");
    }

    // Example test for Clone - testing one variant is usually enough
    #[test]
    fn test_error_cloning() {
        let original = SagittaError::FileNotFound("clone_test.txt".to_string());
        let cloned = original.clone();
        assert_eq!(original.to_string(), cloned.to_string());
        // Ensure it's a deep clone if necessary (though for String it doesn't matter much)
        if let SagittaError::FileNotFound(s1) = original {
            if let SagittaError::FileNotFound(s2) = cloned {
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
        let original_err = SagittaError::HNSWError("Test HNSW error".to_string());
        let anyhow_err = anyhow::Error::new(original_err.clone()); // Clone the original error
        let sagitta_err: SagittaError = anyhow_err.into();
        // Check if the downcasted error is the correct variant
        assert!(
            matches!(sagitta_err, SagittaError::HNSWError(_)),
            "Expected HNSWError, got {:?}", // Add debug print on failure
            sagitta_err
        );
        // Optionally, check if the content is preserved
        if let SagittaError::HNSWError(msg) = sagitta_err {
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
        let original_err = SagittaError::HNSWError("Test HNSW error".to_string());
        let anyhow_err = anyhow::Error::new(original_err.clone());
        let sagitta_err: SagittaError = anyhow_err.into();
        
        // Assert that the downcasted error is the correct variant
        assert!(
            matches!(sagitta_err, SagittaError::HNSWError(_)), 
            "Expected HNSWError, got {:?}", 
            sagitta_err
        );
        
        // Additionally, check if the content matches
        if let SagittaError::HNSWError(msg) = sagitta_err {
            assert_eq!(msg, "Test HNSW error");
        } else {
            // This branch shouldn't be reached if the matches! assertion passed,
            // but it's good practice for completeness or if matches! is removed.
            panic!("Error variant was not HNSWError after checking with matches!");
        }
    }
}
