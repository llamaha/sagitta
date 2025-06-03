//! Error types for the Sagitta embedding engine.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for the embedding engine.
pub type Result<T> = std::result::Result<T, SagittaEmbedError>;

/// Comprehensive error types for the Sagitta embedding engine.
#[derive(Error, Debug)]
pub enum SagittaEmbedError {
    /// Configuration-related errors
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Model loading and initialization errors
    #[error("Model error: {message}")]
    Model { message: String },

    /// Provider-specific errors
    #[error("Provider error: {message}")]
    Provider { message: String },

    /// ONNX runtime errors
    #[error("ONNX runtime error: {message}")]
    OnnxRuntime { message: String },

    /// Tokenization errors
    #[error("Tokenization error: {message}")]
    Tokenization { message: String },

    /// File system errors
    #[error("File system error: {message}")]
    FileSystem { message: String },

    /// Invalid input errors
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    /// Feature not enabled errors
    #[error("Feature '{feature}' is not enabled")]
    FeatureNotEnabled { feature: String },

    /// Not implemented errors
    #[error("Not implemented: {message}")]
    NotImplemented { message: String },

    /// Session pool errors
    #[error("Session pool error: {message}")]
    SessionPool { message: String },

    /// Dimension mismatch errors
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// File not found errors
    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    /// Invalid model path errors
    #[error("Invalid model path: {path}")]
    InvalidModelPath { path: PathBuf },

    /// Invalid tokenizer path errors
    #[error("Invalid tokenizer path: {path}")]
    InvalidTokenizerPath { path: PathBuf },

    /// Embedding generation errors
    #[error("Embedding generation failed: {message}")]
    EmbeddingGeneration { message: String },

    /// Thread safety errors
    #[error("Thread safety error: {message}")]
    ThreadSafety { message: String },

    /// Generic I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Anyhow errors for compatibility
    #[error("External error: {0}")]
    External(#[from] anyhow::Error),
}

impl SagittaEmbedError {
    /// Create a configuration error
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a model error
    pub fn model<S: Into<String>>(message: S) -> Self {
        Self::Model {
            message: message.into(),
        }
    }

    /// Create a provider error
    pub fn provider<S: Into<String>>(message: S) -> Self {
        Self::Provider {
            message: message.into(),
        }
    }

    /// Create an ONNX runtime error
    pub fn onnx_runtime<S: Into<String>>(message: S) -> Self {
        Self::OnnxRuntime {
            message: message.into(),
        }
    }

    /// Create a tokenization error
    pub fn tokenization<S: Into<String>>(message: S) -> Self {
        Self::Tokenization {
            message: message.into(),
        }
    }

    /// Create a file system error
    pub fn file_system<S: Into<String>>(message: S) -> Self {
        Self::FileSystem {
            message: message.into(),
        }
    }

    /// Create an invalid input error
    pub fn invalid_input<S: Into<String>>(message: S) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    /// Create a feature not enabled error
    pub fn feature_not_enabled<S: Into<String>>(feature: S) -> Self {
        Self::FeatureNotEnabled {
            feature: feature.into(),
        }
    }

    /// Create a not implemented error
    pub fn not_implemented<S: Into<String>>(message: S) -> Self {
        Self::NotImplemented {
            message: message.into(),
        }
    }

    /// Create a session pool error
    pub fn session_pool<S: Into<String>>(message: S) -> Self {
        Self::SessionPool {
            message: message.into(),
        }
    }

    /// Create a dimension mismatch error
    pub fn dimension_mismatch(expected: usize, actual: usize) -> Self {
        Self::DimensionMismatch { expected, actual }
    }

    /// Create a file not found error
    pub fn file_not_found<P: Into<PathBuf>>(path: P) -> Self {
        Self::FileNotFound { path: path.into() }
    }

    /// Create an invalid model path error
    pub fn invalid_model_path<P: Into<PathBuf>>(path: P) -> Self {
        Self::InvalidModelPath { path: path.into() }
    }

    /// Create an invalid tokenizer path error
    pub fn invalid_tokenizer_path<P: Into<PathBuf>>(path: P) -> Self {
        Self::InvalidTokenizerPath { path: path.into() }
    }

    /// Create an embedding generation error
    pub fn embedding_generation<S: Into<String>>(message: S) -> Self {
        Self::EmbeddingGeneration {
            message: message.into(),
        }
    }

    /// Create a thread safety error
    pub fn thread_safety<S: Into<String>>(message: S) -> Self {
        Self::ThreadSafety {
            message: message.into(),
        }
    }
}

// Conversion from ort errors
#[cfg(feature = "onnx")]
impl From<ort::Error> for SagittaEmbedError {
    fn from(error: ort::Error) -> Self {
        Self::onnx_runtime(format!("ORT error: {}", error))
    }
}

// Conversion from tokenizer errors
impl From<tokenizers::Error> for SagittaEmbedError {
    fn from(error: tokenizers::Error) -> Self {
        Self::tokenization(format!("Tokenizer error: {}", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_error_creation() {
        let err = SagittaEmbedError::configuration("test config error");
        assert!(matches!(err, SagittaEmbedError::Configuration { .. }));
        assert_eq!(err.to_string(), "Configuration error: test config error");
    }

    #[test]
    fn test_dimension_mismatch_error() {
        let err = SagittaEmbedError::dimension_mismatch(384, 512);
        assert!(matches!(err, SagittaEmbedError::DimensionMismatch { .. }));
        assert_eq!(
            err.to_string(),
            "Dimension mismatch: expected 384, got 512"
        );
    }

    #[test]
    fn test_file_not_found_error() {
        let path = PathBuf::from("/nonexistent/file.onnx");
        let err = SagittaEmbedError::file_not_found(&path);
        assert!(matches!(err, SagittaEmbedError::FileNotFound { .. }));
        assert!(err.to_string().contains("/nonexistent/file.onnx"));
    }

    #[test]
    fn test_feature_not_enabled_error() {
        let err = SagittaEmbedError::feature_not_enabled("cuda");
        assert!(matches!(err, SagittaEmbedError::FeatureNotEnabled { .. }));
        assert_eq!(err.to_string(), "Feature 'cuda' is not enabled");
    }
} 