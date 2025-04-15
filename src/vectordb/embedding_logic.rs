//!
//! Manages the configuration and instantiation of embedding models.
//! Currently focuses on ONNX models but designed to be extensible.

// use crate::vectordb::embedding::{EmbeddingModel, EmbeddingModelType}; // Remove unused EmbeddingModel
use crate::vectordb::embedding::{EmbeddingModelType};
use crate::vectordb::error::{Result, VectorDBError};
use crate::vectordb::provider::EmbeddingProvider;
use crate::vectordb::provider::onnx::OnnxEmbeddingModel;
use std::path::PathBuf;
use std::sync::Mutex;

/// Handles the configuration and creation of embedding models.
///
/// Stores the type of model and necessary paths (e.g., for ONNX models).
/// Use `create_embedding_model` to get an instance of the actual model.
#[derive(Debug)]
pub struct EmbeddingHandler {
    embedding_model_type: EmbeddingModelType,
    onnx_model_path: Option<PathBuf>,
    onnx_tokenizer_path: Option<PathBuf>,
    provider_cache: Mutex<Option<Box<dyn EmbeddingProvider>>>,
}

impl EmbeddingHandler {
    /// Creates a new `EmbeddingHandler`.
    ///
    /// For `EmbeddingModelType::Onnx`, paths to both the model and tokenizer
    /// must be provided and valid, otherwise a `ConfigurationError` or `FileNotFound`
    /// error is returned.
    pub fn new(
        embedding_model_type: EmbeddingModelType,
        onnx_model_path: Option<PathBuf>,
        onnx_tokenizer_path: Option<PathBuf>,
    ) -> Result<Self> {
        if embedding_model_type == EmbeddingModelType::Onnx {
            match (&onnx_model_path, &onnx_tokenizer_path) {
                (Some(model_p), Some(tok_p)) => {
                    if !model_p.exists() {
                        return Err(VectorDBError::FileNotFound(format!(
                            "ONNX model file not found: {}",
                            model_p.display()
                        )));
                    }
                    if !tok_p.exists() {
                        return Err(VectorDBError::FileNotFound(format!(
                            "ONNX tokenizer file not found: {}",
                            tok_p.display()
                        )));
                    }
                }
                _ => {
                    return Err(VectorDBError::ConfigurationError(
                        "ONNX model type requires both model and tokenizer paths.".to_string()
                    ));
                }
            }
        }
        // Add checks for other model types here if they are introduced

        Ok(Self {
            embedding_model_type,
            onnx_model_path,
            onnx_tokenizer_path,
            provider_cache: Mutex::new(None),
        })
    }

    /// Attempts to create an [`EmbeddingProvider`] instance based on the handler's configuration.
    ///
    /// Returns an error if the model cannot be created (e.g., required paths missing for ONNX).
    pub fn create_embedding_model(&self) -> Result<Box<dyn EmbeddingProvider>> {
        match self.embedding_model_type {
            EmbeddingModelType::Onnx => {
                let model_path = self.onnx_model_path.as_ref().ok_or_else(|| {
                    VectorDBError::EmbeddingError("ONNX model path not set in handler.".to_string())
                })?;
                let tokenizer_path = self.onnx_tokenizer_path.as_ref().ok_or_else(|| {
                    VectorDBError::EmbeddingError("ONNX tokenizer path not set in handler.".to_string())
                })?;
                let provider: Box<dyn EmbeddingProvider> = Box::new(OnnxEmbeddingModel::new(
                    model_path,
                    tokenizer_path,
                )?);
                Ok(provider)
            }
            EmbeddingModelType::Default => {
                // For default, potentially use a pre-configured or simpler model
                // Let's assume DefaultEmbeddingProvider exists and implements the trait
                // Ok(Box::new(DefaultEmbeddingProvider::new()?)) // Needs implementation
                 Err(VectorDBError::NotImplemented("Default embedding model provider not yet implemented".to_string()))
            }
        }
    }

    /// Sets or clears the ONNX model and tokenizer paths.
    ///
    /// If paths are provided, they are validated for existence.
    /// If any ONNX path is set, the handler's model type is automatically
    /// set to `EmbeddingModelType::Onnx`.
    pub fn set_onnx_paths(
        &mut self,
        model_path: Option<PathBuf>,
        tokenizer_path: Option<PathBuf>,
    ) -> Result<()> {
        if let Some(model_p) = &model_path {
            if !model_p.exists() {
                return Err(VectorDBError::EmbeddingError(format!(
                    "ONNX model file not found: {}",
                    model_p.display()
                )));
            }
        }
        if let Some(tokenizer_p) = &tokenizer_path {
            if !tokenizer_p.exists() {
                return Err(VectorDBError::EmbeddingError(format!(
                    "ONNX tokenizer file not found: {}",
                    tokenizer_p.display()
                )));
            }
        }

        // If paths are provided, ensure the type is Onnx or update it?
        // For now, assume if setting ONNX paths, the type is Onnx.
        if model_path.is_some() || tokenizer_path.is_some() {
            self.embedding_model_type = EmbeddingModelType::Onnx;
        }

        self.onnx_model_path = model_path;
        self.onnx_tokenizer_path = tokenizer_path;

        // Clear the cache since paths have changed
        self.provider_cache.lock().unwrap().take();

        Ok(())
    }

    /// Returns the configured embedding model type.
    pub fn embedding_model_type(&self) -> EmbeddingModelType {
        self.embedding_model_type
    }

    /// Returns the configured path to the ONNX model file, if set.
    pub fn onnx_model_path(&self) -> Option<&PathBuf> {
        self.onnx_model_path.as_ref()
    }

    /// Returns the configured path to the ONNX tokenizer file/directory, if set.
    pub fn onnx_tokenizer_path(&self) -> Option<&PathBuf> {
        self.onnx_tokenizer_path.as_ref()
    }

    /// Gets the embedding dimension using a cached or newly created provider.
    ///
    /// Returns an error if the provider cannot be created.
    pub fn dimension(&self) -> Result<usize> {
        let mut cache_guard = self.provider_cache.lock().unwrap();
        if cache_guard.is_none() {
            log::debug!("Provider cache miss for dimension. Creating provider...");
            let provider = self.create_embedding_model()?;
            cache_guard.replace(provider);
        }
        Ok(cache_guard.as_ref().unwrap().dimension())
    }

    /// Embeds a batch of texts using a cached or newly created provider.
    ///
    /// Returns an error if the provider cannot be created or embedding fails.
    pub fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut cache_guard = self.provider_cache.lock().unwrap();
        if cache_guard.is_none() {
            log::debug!("Provider cache miss for embed. Creating provider...");
            let provider = self.create_embedding_model()?;
            cache_guard.replace(provider);
        }
        cache_guard.as_mut().unwrap().embed_batch(texts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectordb::embedding::EmbeddingModelType;
    use std::fs::File;
    use tempfile::tempdir;

    // Helper to create dummy files
    fn create_dummy_file(path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        File::create(path)?;
        Ok(())
    }

    #[test]
    fn test_embedding_handler_new_onnx_valid_paths() -> Result<()> {
        let dir = tempdir()?;
        let model_path = dir.path().join("model.onnx");
        let tokenizer_path = dir.path().join("tokenizer.json");
        File::create(&model_path)?;
        File::create(&tokenizer_path)?;

        let handler = EmbeddingHandler::new(
            EmbeddingModelType::Onnx,
            Some(model_path.clone()),
            Some(tokenizer_path.clone()),
        )?;

        assert_eq!(handler.embedding_model_type(), EmbeddingModelType::Onnx);
        assert_eq!(handler.onnx_model_path(), Some(&model_path));
        assert_eq!(handler.onnx_tokenizer_path(), Some(&tokenizer_path));
        Ok(())
    }

    #[test]
    fn test_embedding_handler_new_onnx_missing_paths() {
        let result = EmbeddingHandler::new(EmbeddingModelType::Onnx, None, None);
        assert!(matches!(
            result,
            Err(VectorDBError::ConfigurationError(_))
        ));
        if let Err(VectorDBError::ConfigurationError(msg)) = result {
             assert!(msg.contains("requires both model and tokenizer paths"));
        }
    }
    
    #[test]
    fn test_embedding_handler_new_onnx_missing_model_path() {
        let dir = tempdir().unwrap();
        let tokenizer_path = dir.path().join("tokenizer.json");
        File::create(&tokenizer_path).unwrap();
        
        let result = EmbeddingHandler::new(
            EmbeddingModelType::Onnx,
            None, // Missing model path
            Some(tokenizer_path),
        );
        assert!(matches!(
            result,
            Err(VectorDBError::ConfigurationError(_))
        ));
         if let Err(VectorDBError::ConfigurationError(msg)) = result {
             assert!(msg.contains("requires both model and tokenizer paths"));
        }
    }

    #[test]
    fn test_embedding_handler_new_onnx_missing_tokenizer_path() {
        let dir = tempdir().unwrap();
        let model_path = dir.path().join("model.onnx");
        File::create(&model_path).unwrap();

        let result = EmbeddingHandler::new(
            EmbeddingModelType::Onnx,
            Some(model_path), // Missing tokenizer path
            None,
        );
        assert!(matches!(
            result,
            Err(VectorDBError::ConfigurationError(_))
        ));
         if let Err(VectorDBError::ConfigurationError(msg)) = result {
             assert!(msg.contains("requires both model and tokenizer paths"));
        }
    }

    #[test]
    fn test_embedding_handler_new_onnx_invalid_model_path() {
        let dir = tempdir().unwrap();
        let model_path = dir.path().join("non_existent_model.onnx");
        let tokenizer_path = dir.path().join("tokenizer.json");
        File::create(&tokenizer_path).unwrap(); // Tokenizer exists

        let result = EmbeddingHandler::new(
            EmbeddingModelType::Onnx,
            Some(model_path.clone()),
            Some(tokenizer_path),
        );
        assert!(matches!(result, Err(VectorDBError::FileNotFound(_))));
        if let Err(VectorDBError::FileNotFound(msg)) = result {
            assert!(msg.contains("ONNX model file not found"));
            assert!(msg.contains("non_existent_model.onnx"));
        }
    }

    #[test]
    fn test_embedding_handler_new_onnx_invalid_tokenizer_path() {
        let dir = tempdir().unwrap();
        let model_path = dir.path().join("model.onnx");
        let tokenizer_path = dir.path().join("non_existent_tokenizer.json");
        File::create(&model_path).unwrap(); // Model exists

        let result = EmbeddingHandler::new(
            EmbeddingModelType::Onnx,
            Some(model_path),
            Some(tokenizer_path.clone()),
        );
        assert!(matches!(result, Err(VectorDBError::FileNotFound(_))));
        if let Err(VectorDBError::FileNotFound(msg)) = result {
            assert!(msg.contains("ONNX tokenizer file not found"));
            assert!(msg.contains("non_existent_tokenizer.json"));
        }
    }

    // --- Tests for set_onnx_paths ---

    #[test]
    fn test_set_onnx_paths_valid() -> Result<()> {
        let dir = tempdir()?;
        let model_path = dir.path().join("model_v1.onnx");
        let tokenizer_path = dir.path().join("tokenizer_v1.json");
        File::create(&model_path)?;
        File::create(&tokenizer_path)?;

        #[allow(clippy::unnecessary_lazy_evaluations)]
        let mut handler = EmbeddingHandler::new(EmbeddingModelType::Onnx, None, None).unwrap_or_else(|_|
            EmbeddingHandler { 
                embedding_model_type: EmbeddingModelType::Onnx,
                onnx_model_path: None,
                onnx_tokenizer_path: None,
                provider_cache: Mutex::new(None),
            }
        );
        // Assert initial state (or skip if constructor guarantees None)
        assert_eq!(handler.onnx_model_path(), None);
        assert_eq!(handler.onnx_tokenizer_path(), None);

        // Set valid paths
        handler.set_onnx_paths(Some(model_path.clone()), Some(tokenizer_path.clone()))?;

        assert_eq!(handler.embedding_model_type(), EmbeddingModelType::Onnx);
        assert_eq!(handler.onnx_model_path(), Some(&model_path));
        assert_eq!(handler.onnx_tokenizer_path(), Some(&tokenizer_path));

        Ok(())
    }

    #[test]
    fn test_set_onnx_paths_clear() -> Result<()> {
        let dir = tempdir()?;
        let model_path = dir.path().join("model.onnx");
        let tokenizer_path = dir.path().join("tokenizer.json");
        File::create(&model_path)?;
        File::create(&tokenizer_path)?;

        // Start with valid paths
        let mut handler = EmbeddingHandler::new(
            EmbeddingModelType::Onnx,
            Some(model_path.clone()),
            Some(tokenizer_path.clone()),
        )?;

        // Clear paths
        handler.set_onnx_paths(None, None)?;

        // Type should remain Onnx (as per current logic), paths should be None
        assert_eq!(handler.embedding_model_type(), EmbeddingModelType::Onnx);
        assert_eq!(handler.onnx_model_path(), None);
        assert_eq!(handler.onnx_tokenizer_path(), None);

        Ok(())
    }
    
    #[test]
    fn test_set_onnx_paths_invalid_model() {
        let dir = tempdir().unwrap();
        let invalid_model_path = dir.path().join("bad_model.onnx");
        let tokenizer_path = dir.path().join("good_tokenizer.json");
        File::create(&tokenizer_path).unwrap();
        
        let mut handler = EmbeddingHandler { // Create directly to avoid constructor issues
             embedding_model_type: EmbeddingModelType::Onnx,
             onnx_model_path: None,
             onnx_tokenizer_path: None,
             provider_cache: Mutex::new(None),
        };

        let result = handler.set_onnx_paths(Some(invalid_model_path.clone()), Some(tokenizer_path));
        
        assert!(matches!(result, Err(VectorDBError::EmbeddingError(_))));
        if let Err(VectorDBError::EmbeddingError(msg)) = result {
            assert!(msg.contains("ONNX model file not found"));
            assert!(msg.contains("bad_model.onnx"));
        }
        // Ensure original paths (None) were not changed on error
        assert_eq!(handler.onnx_model_path(), None);
        assert_eq!(handler.onnx_tokenizer_path(), None);
    }

    #[test]
    fn test_set_onnx_paths_invalid_tokenizer() {
        let dir = tempdir().unwrap();
        let model_path = dir.path().join("good_model.onnx");
        let invalid_tokenizer_path = dir.path().join("bad_tokenizer.json");
        File::create(&model_path).unwrap();

        let mut handler = EmbeddingHandler { 
             embedding_model_type: EmbeddingModelType::Onnx,
             onnx_model_path: None,
             onnx_tokenizer_path: None,
             provider_cache: Mutex::new(None),
        };

        let result = handler.set_onnx_paths(Some(model_path), Some(invalid_tokenizer_path.clone()));

        assert!(matches!(result, Err(VectorDBError::EmbeddingError(_))));
        if let Err(VectorDBError::EmbeddingError(msg)) = result {
             assert!(msg.contains("ONNX tokenizer file not found"));
             assert!(msg.contains("bad_tokenizer.json"));
        }
        // Ensure original paths (None) were not changed on error
        assert_eq!(handler.onnx_model_path(), None);
        assert_eq!(handler.onnx_tokenizer_path(), None);
    }

    // --- Tests for create_embedding_model ---

    #[test]
    fn test_create_embedding_model_onnx_paths_none() {
        // Create handler without setting paths (assuming constructor allows this or use default)
        let handler = EmbeddingHandler { 
            embedding_model_type: EmbeddingModelType::Onnx,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            provider_cache: Mutex::new(None),
        };
        
        let result = handler.create_embedding_model();
        assert!(matches!(result, Err(VectorDBError::EmbeddingError(_))));
        if let Err(VectorDBError::EmbeddingError(msg)) = result {
            assert!(msg.contains("ONNX model path not set in handler"));
        }
    }
    
    #[test]
    fn test_create_embedding_model_onnx_model_path_none() {
        let dir = tempdir().unwrap();
        let tokenizer_path = dir.path().join("tokenizer.json");
        File::create(&tokenizer_path).unwrap();
        
        let handler = EmbeddingHandler { 
            embedding_model_type: EmbeddingModelType::Onnx,
            onnx_model_path: None,
            onnx_tokenizer_path: Some(tokenizer_path),
            provider_cache: Mutex::new(None),
        };
        
        let result = handler.create_embedding_model();
        assert!(matches!(result, Err(VectorDBError::EmbeddingError(_))));
        if let Err(VectorDBError::EmbeddingError(msg)) = result {
            assert!(msg.contains("ONNX model path not set in handler"));
        }
    }

    #[test]
    fn test_create_embedding_model_onnx_tokenizer_path_none() {
        let dir = tempdir().unwrap();
        let model_path = dir.path().join("model.onnx");
        File::create(&model_path).unwrap();
        
        let handler = EmbeddingHandler { 
            embedding_model_type: EmbeddingModelType::Onnx,
            onnx_model_path: Some(model_path),
            onnx_tokenizer_path: None,
            provider_cache: Mutex::new(None),
        };
        
        let result = handler.create_embedding_model();
        assert!(matches!(result, Err(VectorDBError::EmbeddingError(_))));
        if let Err(VectorDBError::EmbeddingError(msg)) = result {
            assert!(msg.contains("ONNX tokenizer path not set in handler"));
        }
    }

    // Note: Testing the Ok case requires either:
    // 1. A real (or minimal mock) ONNX model and tokenizer available during tests.
    // 2. Mocking the `EmbeddingModel::new_onnx` function itself (e.g., using a mocking library like `mockall`).
    // For now, we only test the error paths related to missing configuration within the handler.

    #[test]
    fn test_embedding_handler_dimension_onnx_success() -> Result<()> {
        // This test requires actual ONNX model files or a mock provider.
        // For now, let's assume the model files exist at standard paths
        // and skip if they don't.
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json"); // Assumes tokenizer.json is in the same dir

        if !model_path.exists() || !tokenizer_path.exists() {
             println!("Skipping test_embedding_handler_dimension_onnx_success: ONNX files not found at expected paths.");
             return Ok(());
        }


        let handler = EmbeddingHandler::new(
            EmbeddingModelType::Onnx,
            Some(model_path.clone()),
            Some(tokenizer_path.clone()),
        )?;

        let dim = handler.dimension()?;
        // The dimension depends on the actual model, but MiniLM is typically 384
        assert_eq!(dim, 384, "Expected dimension for MiniLM L12 v2"); 
        
        Ok(())
    }

    #[test]
    fn test_embedding_handler_dimension_onnx_fail_missing_path() {
        // Test getting dimension when paths are missing
        let handler_no_paths = EmbeddingHandler {
            embedding_model_type: EmbeddingModelType::Onnx,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            provider_cache: Mutex::new(None),
        };
        let result = handler_no_paths.dimension();
        assert!(matches!(result, Err(VectorDBError::EmbeddingError(_))));
         if let Err(VectorDBError::EmbeddingError(msg)) = result {
            assert!(msg.contains("ONNX model path not set in handler"));
        }

        // Test with invalid (non-existent) paths provided during construction
        let dir = tempdir().unwrap();
        let invalid_model_path = dir.path().join("invalid_model.onnx");
        let invalid_tokenizer_path = dir.path().join("invalid_tokenizer.json");
        // We need to create the files for EmbeddingHandler::new to succeed,
        // but the underlying EmbeddingModel::new_onnx will fail.
        create_dummy_file(&invalid_model_path).unwrap();
        create_dummy_file(&invalid_tokenizer_path).unwrap();

        // Use a real path that points to invalid file contents for the provider to fail
        let handler_invalid_files = EmbeddingHandler::new(
             EmbeddingModelType::Onnx,
             Some(invalid_model_path),
             Some(invalid_tokenizer_path),
        ).expect("Handler creation should succeed with existing (but invalid) files");

        let result_invalid = handler_invalid_files.dimension();
        // Expect HNSWError because the underlying provider creation (anyhow::Error) gets converted
        assert!(matches!(result_invalid, Err(VectorDBError::HNSWError(_))), "Expected HNSWError for invalid ONNX model/tokenizer files, got {:?}", result_invalid);
        // Check if the error message indicates a lower-level failure (like ORT error)
         if let Err(VectorDBError::HNSWError(msg)) = result_invalid {
            // Check for keywords indicating model/tokenizer loading failure
            assert!(msg.contains("load") || msg.contains("invalid") || msg.contains("session") || msg.contains("tokenizer"), "Error message mismatch, expected load/invalid/session/tokenizer error: {}", msg);
        }
    }
} 