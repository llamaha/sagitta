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

    /// Memory management errors
    #[error("Memory error: {message}")]
    Memory { message: String },

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

    /// Create a memory management error
    pub fn memory<S: Into<String>>(message: S) -> Self {
        Self::Memory {
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

    #[test]
    fn test_all_error_variants() {
        // Test all error creation methods
        let config_err = SagittaEmbedError::configuration("config message");
        assert!(matches!(config_err, SagittaEmbedError::Configuration { .. }));

        let model_err = SagittaEmbedError::model("model message");
        assert!(matches!(model_err, SagittaEmbedError::Model { .. }));

        let provider_err = SagittaEmbedError::provider("provider message");
        assert!(matches!(provider_err, SagittaEmbedError::Provider { .. }));

        let onnx_err = SagittaEmbedError::onnx_runtime("onnx message");
        assert!(matches!(onnx_err, SagittaEmbedError::OnnxRuntime { .. }));

        let tokenization_err = SagittaEmbedError::tokenization("tokenizer message");
        assert!(matches!(tokenization_err, SagittaEmbedError::Tokenization { .. }));

        let fs_err = SagittaEmbedError::file_system("filesystem message");
        assert!(matches!(fs_err, SagittaEmbedError::FileSystem { .. }));

        let input_err = SagittaEmbedError::invalid_input("invalid input");
        assert!(matches!(input_err, SagittaEmbedError::InvalidInput { .. }));

        let not_impl_err = SagittaEmbedError::not_implemented("not implemented");
        assert!(matches!(not_impl_err, SagittaEmbedError::NotImplemented { .. }));

        let session_err = SagittaEmbedError::session_pool("session pool error");
        assert!(matches!(session_err, SagittaEmbedError::SessionPool { .. }));

        let invalid_model_path = SagittaEmbedError::invalid_model_path("/invalid/model.onnx");
        assert!(matches!(invalid_model_path, SagittaEmbedError::InvalidModelPath { .. }));

        let invalid_tokenizer_path = SagittaEmbedError::invalid_tokenizer_path("/invalid/tokenizer.json");
        assert!(matches!(invalid_tokenizer_path, SagittaEmbedError::InvalidTokenizerPath { .. }));

        let embedding_err = SagittaEmbedError::embedding_generation("embedding failed");
        assert!(matches!(embedding_err, SagittaEmbedError::EmbeddingGeneration { .. }));

        let thread_err = SagittaEmbedError::thread_safety("thread safety error");
        assert!(matches!(thread_err, SagittaEmbedError::ThreadSafety { .. }));

        let memory_err = SagittaEmbedError::memory("memory error");
        assert!(matches!(memory_err, SagittaEmbedError::Memory { .. }));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let sagitta_err: SagittaEmbedError = io_err.into();
        assert!(matches!(sagitta_err, SagittaEmbedError::Io { .. }));
    }

    #[test]
    fn test_anyhow_error_conversion() {
        let anyhow_err = anyhow::anyhow!("test anyhow error");
        let sagitta_err: SagittaEmbedError = anyhow_err.into();
        assert!(matches!(sagitta_err, SagittaEmbedError::External { .. }));
    }

    #[test]
    fn test_tokenizer_error_conversion() {
        // Test that the conversion trait exists and works
        // We'll create a simple test that validates the conversion function compiles
        let _test_fn = |err: tokenizers::Error| -> SagittaEmbedError {
            err.into()
        };
    }

    #[cfg(feature = "onnx")]
    #[test]
    fn test_ort_error_conversion() {
        // This test only works if we can create an ort::Error
        // We'll simulate it by testing the conversion logic
        use ort::Error as OrtError;
        
        // Create a mock ORT error using the correct API
        let mock_ort_err = OrtError::new("test ort error");
        let sagitta_err: SagittaEmbedError = mock_ort_err.into();
        assert!(matches!(sagitta_err, SagittaEmbedError::OnnxRuntime { .. }));
        assert!(sagitta_err.to_string().contains("ORT error"));
    }

    #[test]
    fn test_error_display_messages() {
        assert_eq!(
            SagittaEmbedError::configuration("config").to_string(),
            "Configuration error: config"
        );
        assert_eq!(
            SagittaEmbedError::model("model").to_string(),
            "Model error: model"
        );
        assert_eq!(
            SagittaEmbedError::provider("provider").to_string(),
            "Provider error: provider"
        );
        assert_eq!(
            SagittaEmbedError::onnx_runtime("onnx").to_string(),
            "ONNX runtime error: onnx"
        );
        assert_eq!(
            SagittaEmbedError::tokenization("token").to_string(),
            "Tokenization error: token"
        );
        assert_eq!(
            SagittaEmbedError::file_system("fs").to_string(),
            "File system error: fs"
        );
        assert_eq!(
            SagittaEmbedError::invalid_input("input").to_string(),
            "Invalid input: input"
        );
        assert_eq!(
            SagittaEmbedError::feature_not_enabled("feature").to_string(),
            "Feature 'feature' is not enabled"
        );
        assert_eq!(
            SagittaEmbedError::not_implemented("impl").to_string(),
            "Not implemented: impl"
        );
        assert_eq!(
            SagittaEmbedError::session_pool("session").to_string(),
            "Session pool error: session"
        );
        assert_eq!(
            SagittaEmbedError::embedding_generation("embed").to_string(),
            "Embedding generation failed: embed"
        );
        assert_eq!(
            SagittaEmbedError::thread_safety("thread").to_string(),
            "Thread safety error: thread"
        );
        assert_eq!(
            SagittaEmbedError::memory("memory").to_string(),
            "Memory error: memory"
        );
    }

    #[test]
    fn test_pathbuf_conversions() {
        let path_str = "/test/path/model.onnx";
        let path_buf = PathBuf::from(path_str);
        
        let file_not_found = SagittaEmbedError::file_not_found(path_str);
        assert!(file_not_found.to_string().contains(path_str));
        
        let invalid_model = SagittaEmbedError::invalid_model_path(&path_buf);
        assert!(invalid_model.to_string().contains(path_str));
        
        let invalid_tokenizer = SagittaEmbedError::invalid_tokenizer_path(path_buf);
        assert!(invalid_tokenizer.to_string().contains(path_str));
    }
} 