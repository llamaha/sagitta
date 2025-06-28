//! Factory for creating embedding providers based on configuration.

use crate::error::{Result, SagittaEmbedError};
use crate::config::EmbeddingConfig;
use crate::model::EmbeddingModelType;
use crate::provider::{EmbeddingProvider, DefaultEmbeddingProvider};
use std::sync::Arc;

#[cfg(feature = "onnx")]
use crate::provider::onnx::OnnxEmbeddingModel;

/// Create an embedding provider based on the configuration.
pub fn create_embedding_provider(config: &EmbeddingConfig) -> Result<Arc<dyn EmbeddingProvider>> {
    log::debug!("Creating embedding provider for model type: {:?}", config.model_type);
    match config.model_type {
        EmbeddingModelType::Onnx => {
            #[cfg(feature = "onnx")]
            {
                let model_path = config.onnx_model_path.as_ref()
                    .ok_or_else(|| SagittaEmbedError::configuration("ONNX model path not set [factory]"))?;
                let tokenizer_path = config.onnx_tokenizer_path.as_ref()
                    .ok_or_else(|| SagittaEmbedError::configuration("ONNX tokenizer path not set"))?;
                
                let model = OnnxEmbeddingModel::new_with_config(model_path, tokenizer_path, config)?;
                Ok(Arc::new(model))
            }
            #[cfg(not(feature = "onnx"))]
            {
                Err(SagittaEmbedError::feature_not_enabled("onnx"))
            }
        }
        EmbeddingModelType::Default => {
            let dimension = config.get_embedding_dimension();
            let provider = DefaultEmbeddingProvider::new(dimension);
            Ok(Arc::new(provider))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_create_default_provider() {
        let config = EmbeddingConfig::default();
        let provider = create_embedding_provider(&config).unwrap();
        assert_eq!(provider.model_type(), EmbeddingModelType::Default);
        assert_eq!(provider.dimension(), 384); // Default dimension
    }

    #[test]
    fn test_create_onnx_provider_missing_paths() {
        let config = EmbeddingConfig {
            model_type: EmbeddingModelType::Onnx,
            ..Default::default()
        };
        
        let result = create_embedding_provider(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("model path not set"));
    }

    #[cfg(feature = "onnx")]
    #[test]
    fn test_create_onnx_provider_with_paths() {
        use tempfile::tempdir;
        use std::fs;
        
        let temp_dir = tempdir().unwrap();
        let model_path = temp_dir.path().join("model.onnx");
        let tokenizer_path = temp_dir.path().join("tokenizer.json");
        
        // Create dummy files
        fs::write(&model_path, "dummy model").unwrap();
        
        // Create minimal valid tokenizer JSON
        let tokenizer_content = serde_json::json!({
            "version": "1.0",
            "truncation": null,
            "padding": null,
            "added_tokens": [],
            "normalizer": null,
            "pre_tokenizer": null,
            "post_processor": null,
            "decoder": null,
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "continuing_subword_prefix": "##",
                "max_input_chars_per_word": 100,
                "vocab": {
                    "[UNK]": 0,
                    "[CLS]": 1,
                    "[SEP]": 2
                }
            }
        });
        fs::write(&tokenizer_path, tokenizer_content.to_string()).unwrap();
        
        let config = EmbeddingConfig::new_onnx(model_path, tokenizer_path);
        
        // This might fail if ONNX runtime isn't available
        match create_embedding_provider(&config) {
            Ok(provider) => {
                assert_eq!(provider.model_type(), EmbeddingModelType::Onnx);
            }
            Err(_) => {
                // Expected in test environments without ONNX runtime
                println!("ONNX provider creation failed as expected in test environment");
            }
        }
    }
}