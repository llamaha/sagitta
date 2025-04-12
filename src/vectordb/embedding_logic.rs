use crate::vectordb::embedding::{EmbeddingModel, EmbeddingModelType};
use crate::vectordb::error::{Result, VectorDBError};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct EmbeddingHandler {
    embedding_model_type: EmbeddingModelType,
    onnx_model_path: Option<PathBuf>,
    onnx_tokenizer_path: Option<PathBuf>,
}

impl EmbeddingHandler {
    /// Creates a new EmbeddingHandler, validating paths if provided.
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
        })
    }

    /// Attempts to create an EmbeddingModel instance based on the handler's configuration.
    pub fn create_embedding_model(&self) -> Result<EmbeddingModel> {
        match self.embedding_model_type {
            EmbeddingModelType::Onnx => {
                if let (Some(model_path), Some(tokenizer_path)) =
                    (&self.onnx_model_path, &self.onnx_tokenizer_path)
                {
                    EmbeddingModel::new_onnx(model_path, tokenizer_path)
                        .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))
                } else {
                    Err(VectorDBError::EmbeddingError(
                        "ONNX model paths not set in handler.".to_string(),
                    ))
                }
            }
            // Add cases for other embedding model types here
            // EmbeddingModelType::Other => { ... }
        }
    }

    /// Sets or clears the ONNX model and tokenizer paths.
    /// Performs validation checks if paths are provided.
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

        Ok(())
    }

    // --- Getters ---
    pub fn embedding_model_type(&self) -> EmbeddingModelType {
        self.embedding_model_type
    }

    pub fn onnx_model_path(&self) -> Option<&PathBuf> {
        self.onnx_model_path.as_ref()
    }

    pub fn onnx_tokenizer_path(&self) -> Option<&PathBuf> {
        self.onnx_tokenizer_path.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectordb::embedding::EmbeddingModelType;
    use std::fs::File;
    use tempfile::tempdir;

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

        let mut handler = EmbeddingHandler::new(EmbeddingModelType::Onnx, None, None).unwrap_or_else(|_| 
            // Provide dummy initial paths if needed for constructor, although set_onnx_paths should overwrite
            EmbeddingHandler { 
                embedding_model_type: EmbeddingModelType::Onnx, // Start as Onnx but paths are None 
                onnx_model_path: None, 
                onnx_tokenizer_path: None 
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
        };
        
        let result = handler.create_embedding_model();
        assert!(matches!(result, Err(VectorDBError::EmbeddingError(_))));
        if let Err(VectorDBError::EmbeddingError(msg)) = result {
            assert!(msg.contains("ONNX model paths not set in handler"));
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
        };
        
        let result = handler.create_embedding_model();
        assert!(matches!(result, Err(VectorDBError::EmbeddingError(_))));
        if let Err(VectorDBError::EmbeddingError(msg)) = result {
            assert!(msg.contains("ONNX model paths not set in handler"));
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
        };
        
        let result = handler.create_embedding_model();
        assert!(matches!(result, Err(VectorDBError::EmbeddingError(_))));
        if let Err(VectorDBError::EmbeddingError(msg)) = result {
            assert!(msg.contains("ONNX model paths not set in handler"));
        }
    }

    // Note: Testing the Ok case requires either:
    // 1. A real (or minimal mock) ONNX model and tokenizer available during tests.
    // 2. Mocking the `EmbeddingModel::new_onnx` function itself (e.g., using a mocking library like `mockall`).
    // For now, we only test the error paths related to missing configuration within the handler.
} 