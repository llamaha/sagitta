//! Configuration types for the Sagitta embedding engine.

use crate::error::{Result, SagittaEmbedError};
use crate::model::EmbeddingModelType;
use crate::{DEFAULT_EMBEDDING_DIMENSION, DEFAULT_MAX_SESSIONS, DEFAULT_MAX_SEQUENCE_LENGTH, DEFAULT_SESSION_TIMEOUT_SECONDS, DEFAULT_ENABLE_SESSION_CLEANUP, DEFAULT_EMBEDDING_BATCH_SIZE};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for embedding models and processing.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingConfig {
    /// Type of embedding model to use
    pub model_type: EmbeddingModelType,
    /// Path to ONNX model file (required for ONNX models)
    pub onnx_model_path: Option<PathBuf>,
    /// Path to tokenizer file or directory (required for ONNX models)
    pub onnx_tokenizer_path: Option<PathBuf>,
    /// Maximum number of concurrent sessions for session pooling
    pub max_sessions: usize,
    /// Maximum sequence length for tokenization
    pub max_sequence_length: usize,
    /// Expected embedding dimension (for validation)
    pub expected_dimension: Option<usize>,
    /// Session timeout in seconds (0 = no timeout)
    pub session_timeout_seconds: u64,
    /// Enable session cleanup on idle
    pub enable_session_cleanup: bool,
    /// Optional tenant ID for multi-tenancy
    pub tenant_id: Option<String>,
    /// Batch size for embedding operations (number of texts processed together)
    pub embedding_batch_size: Option<usize>,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_type: EmbeddingModelType::Default,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            max_sessions: DEFAULT_MAX_SESSIONS,
            max_sequence_length: DEFAULT_MAX_SEQUENCE_LENGTH,
            expected_dimension: Some(DEFAULT_EMBEDDING_DIMENSION),
            session_timeout_seconds: DEFAULT_SESSION_TIMEOUT_SECONDS,
            enable_session_cleanup: DEFAULT_ENABLE_SESSION_CLEANUP,
            tenant_id: None,
            embedding_batch_size: Some(DEFAULT_EMBEDDING_BATCH_SIZE),
        }
    }
}

impl EmbeddingConfig {
    /// Create a new embedding configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new ONNX embedding configuration.
    pub fn new_onnx(model_path: PathBuf, tokenizer_path: PathBuf) -> Self {
        Self {
            model_type: EmbeddingModelType::Onnx,
            onnx_model_path: Some(model_path),
            onnx_tokenizer_path: Some(tokenizer_path),
            ..Default::default()
        }
    }

    /// Set the maximum number of concurrent sessions.
    pub fn with_max_sessions(mut self, max_sessions: usize) -> Self {
        self.max_sessions = max_sessions;
        self
    }

    /// Set the maximum sequence length.
    pub fn with_max_sequence_length(mut self, max_sequence_length: usize) -> Self {
        self.max_sequence_length = max_sequence_length;
        self
    }

    /// Set the expected embedding dimension.
    pub fn with_expected_dimension(mut self, dimension: usize) -> Self {
        self.expected_dimension = Some(dimension);
        self
    }

    /// Set the session timeout.
    pub fn with_session_timeout(mut self, timeout_seconds: u64) -> Self {
        self.session_timeout_seconds = timeout_seconds;
        self
    }

    /// Set the tenant ID.
    pub fn with_tenant_id(mut self, tenant_id: String) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    /// Set the embedding batch size.
    pub fn with_embedding_batch_size(mut self, batch_size: usize) -> Self {
        self.embedding_batch_size = Some(batch_size);
        self
    }

    /// Set the model type.
    pub fn with_model_type(mut self, model_type: EmbeddingModelType) -> Self {
        self.model_type = model_type;
        self
    }

    /// Get the embedding batch size, using default if not set.
    pub fn get_embedding_batch_size(&self) -> usize {
        self.embedding_batch_size.unwrap_or(DEFAULT_EMBEDDING_BATCH_SIZE)
    }

    /// Get the embedding dimension from configuration or default.
    pub fn get_embedding_dimension(&self) -> usize {
        self.expected_dimension.unwrap_or(DEFAULT_EMBEDDING_DIMENSION)
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        match self.model_type {
            EmbeddingModelType::Onnx => {
                if self.onnx_model_path.is_none() {
                    return Err(SagittaEmbedError::configuration("ONNX model path is required for ONNX models"));
                }
                if self.onnx_tokenizer_path.is_none() {
                    return Err(SagittaEmbedError::configuration("ONNX tokenizer path is required for ONNX models"));
                }
            },
            EmbeddingModelType::Default => {
                // Default model doesn't require additional validation
            }
        }

        if self.max_sessions == 0 {
            return Err(SagittaEmbedError::configuration("max_sessions must be greater than 0"));
        }

        if self.max_sequence_length == 0 {
            return Err(SagittaEmbedError::configuration("max_sequence_length must be greater than 0"));
        }

        Ok(())
    }

    /// Check if CUDA should be used (determined at compile time).
    pub fn should_use_cuda(&self) -> bool {
        cfg!(feature = "cuda")
    }
}

/// Configuration builder for more ergonomic configuration creation.
#[derive(Debug, Default)]
pub struct EmbeddingConfigBuilder {
    config: EmbeddingConfig,
}

impl EmbeddingConfigBuilder {
    /// Create a new configuration builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the model type.
    pub fn model_type(mut self, model_type: EmbeddingModelType) -> Self {
        self.config.model_type = model_type;
        self
    }

    /// Set the ONNX model path.
    pub fn onnx_model_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.config.onnx_model_path = Some(path.into());
        self
    }

    /// Set the ONNX model path from a string.
    pub fn onnx_model<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.config.onnx_model_path = Some(path.into());
        self
    }

    /// Set the ONNX tokenizer path.
    pub fn onnx_tokenizer_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.config.onnx_tokenizer_path = Some(path.into());
        self
    }

    /// Set the ONNX tokenizer path from a string.
    pub fn onnx_tokenizer<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.config.onnx_tokenizer_path = Some(path.into());
        self
    }

    /// Set the maximum number of sessions.
    pub fn max_sessions(mut self, max_sessions: usize) -> Self {
        self.config.max_sessions = max_sessions;
        self
    }

    /// Set the maximum sequence length.
    pub fn max_sequence_length(mut self, max_sequence_length: usize) -> Self {
        self.config.max_sequence_length = max_sequence_length;
        self
    }

    /// Set the expected embedding dimension.
    pub fn expected_dimension(mut self, dimension: usize) -> Self {
        self.config.expected_dimension = Some(dimension);
        self
    }

    /// Set the tenant ID.
    pub fn tenant_id<S: Into<String>>(mut self, tenant_id: S) -> Self {
        self.config.tenant_id = Some(tenant_id.into());
        self
    }

    /// Set the embedding batch size
    pub fn embedding_batch_size(mut self, batch_size: usize) -> Self {
        self.config.embedding_batch_size = Some(batch_size);
        self
    }

    /// Build and validate the configuration.
    pub fn build(self) -> Result<EmbeddingConfig> {
        self.config.validate()?;
        Ok(self.config)
    }

    /// Build the configuration without validation.
    pub fn build_unchecked(self) -> EmbeddingConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_default_config() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.model_type, EmbeddingModelType::Default);
        assert_eq!(config.max_sessions, DEFAULT_MAX_SESSIONS);
        assert_eq!(config.max_sequence_length, DEFAULT_MAX_SEQUENCE_LENGTH);
    }

    #[test]
    fn test_config_builder() {
        let config = EmbeddingConfigBuilder::new()
            .model_type(EmbeddingModelType::Onnx)
            .max_sessions(8)
            .max_sequence_length(256)
            .expected_dimension(512)
            .tenant_id("test-tenant")
            .build_unchecked();

        assert_eq!(config.model_type, EmbeddingModelType::Onnx);
        assert_eq!(config.max_sessions, 8);
        assert_eq!(config.max_sequence_length, 256);
        assert_eq!(config.expected_dimension, Some(512));
        assert_eq!(config.tenant_id, Some("test-tenant".to_string()));
    }

    #[test]
    fn test_config_validation_missing_paths() {
        let config = EmbeddingConfig {
            model_type: EmbeddingModelType::Onnx,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("model path is required"));
    }

    #[test]
    fn test_config_validation_invalid_sessions() {
        let temp_dir = tempdir().unwrap();
        let model_path = temp_dir.path().join("model.onnx");
        let tokenizer_path = temp_dir.path().join("tokenizer.json");

        // Create dummy files
        fs::write(&model_path, "dummy model").unwrap();
        fs::write(&tokenizer_path, "dummy tokenizer").unwrap();

        let config = EmbeddingConfig {
            model_type: EmbeddingModelType::Onnx,
            onnx_model_path: Some(model_path),
            onnx_tokenizer_path: Some(tokenizer_path),
            max_sessions: 0, // This should cause the validation error
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("max_sessions must be greater than 0"));
    }

    #[test]
    fn test_config_validation_with_valid_files() {
        let temp_dir = tempdir().unwrap();
        let model_path = temp_dir.path().join("model.onnx");
        let tokenizer_path = temp_dir.path().join("tokenizer.json");

        // Create dummy files
        fs::write(&model_path, "dummy model").unwrap();
        fs::write(&tokenizer_path, "dummy tokenizer").unwrap();

        let config = EmbeddingConfig {
            model_type: EmbeddingModelType::Onnx,
            onnx_model_path: Some(model_path),
            onnx_tokenizer_path: Some(tokenizer_path),
            ..Default::default()
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_new_onnx_config() {
        let config = EmbeddingConfig::new_onnx(PathBuf::from("/path/to/model.onnx"), PathBuf::from("/path/to/tokenizer.json"));
        assert_eq!(config.model_type, EmbeddingModelType::Onnx);
        assert_eq!(config.onnx_model_path, Some(PathBuf::from("/path/to/model.onnx")));
        assert_eq!(config.onnx_tokenizer_path, Some(PathBuf::from("/path/to/tokenizer.json")));
    }

    #[test]
    fn test_fluent_interface() {
        let config = EmbeddingConfig::new()
            .with_model_type(EmbeddingModelType::Onnx)
            .with_max_sessions(16)
            .with_expected_dimension(512)
            .with_tenant_id("test-tenant".to_string());

        assert_eq!(config.model_type, EmbeddingModelType::Onnx);
        assert_eq!(config.max_sessions, 16);
        assert_eq!(config.expected_dimension, Some(512));
        assert_eq!(config.tenant_id, Some("test-tenant".to_string()));
    }
} 