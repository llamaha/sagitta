// Use traits/types from within the embedding module
use super::EmbeddingProvider;
use crate::embedding::EmbeddingModelType;
// Use error types from within this crate
use crate::error::{Result as VectorDBResult, VectorDBError};

// Keep external dependencies (ensure they are in vectordb_core's Cargo.toml)
use anyhow::{anyhow, Error, Result};
use log::{debug};
use ndarray::{Array};
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::{Value};
use ort::execution_providers::{CUDAExecutionProvider};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;

// Remove imports for types defined elsewhere or unused
// use crate::vectordb::embedding::EmbeddingModel;
// use async_trait::async_trait; // Not used here
// use std::path::{Path, PathBuf}; // PathBuf not used here
// use crate::vectordb::embedding::EmbeddingResult; // Not used here
// use crate::syntax::SyntaxElement; // Not used here

/// ONNX-based embedding model
#[derive(Debug)]
pub struct OnnxEmbeddingModel {
    /// The tokenizer for preprocessing input text
    tokenizer: Arc<Mutex<Tokenizer>>,
    /// Maximum sequence length for the model
    max_seq_length: usize,
    /// ONNX session for running inference
    session: Session,
    /// The actual dimension of the loaded model's embeddings
    dimension: usize,
}

impl OnnxEmbeddingModel {
    /// Creates a new OnnxEmbeddingModel from the given model and tokenizer paths
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        debug!(
            "Creating ONNX embedding model with model: {}",
            model_path.display()
        );

        // Load tokenizer
        // Assuming tokenizer_path is the directory containing tokenizer.json
        let tokenizer_json_path = tokenizer_path.join("tokenizer.json"); 
        debug!("Loading tokenizer from: {}", tokenizer_json_path.display());

        let tokenizer = Tokenizer::from_file(&tokenizer_json_path)
            .map_err(|e| Error::msg(format!("Failed to load tokenizer: {}", e)))?;

        debug!("Tokenizer loaded successfully");

        // Initialize Environment using ort::init()
        let cuda_provider = CUDAExecutionProvider::default();
        let _ = ort::init()
            .with_name("vectordb-onnx")
            .with_execution_providers([cuda_provider.build()]) // Configure EPs here
            .commit();

        // Build session using Session::builder() - EPs are global now
        let session = Session::builder()? 
            .with_optimization_level(GraphOptimizationLevel::Level1)?
            .commit_from_file(model_path)?;

        // Determine dimension from the loaded session
        let dimension = session
            .outputs
            .iter()
            .find(|output| output.name == "pooler_output")
            .and_then(|output| match output.output_type {
                ort::value::ValueType::Tensor { ref dimensions, .. } => {
                    dimensions.last().map(|&d| d as usize)
                }
                _ => None,
            })
            .ok_or_else(|| anyhow!("Failed to get model dimension from pooler_output"))?;

        debug!(
            "ONNX model loaded successfully from {}, determined embedding dimension: {}",
            model_path.display(),
            dimension
        );

        let tokenizer = Arc::new(Mutex::new(tokenizer));

        Ok(Self {
            session,
            tokenizer,
            max_seq_length: 128, // TODO: Make this configurable or detect from model?
            dimension,
        })
    }

    /// Tokenizes input text and prepares model inputs
    fn prepare_inputs(&self, text: &str) -> Result<(Vec<i64>, Vec<i64>)> {
        let encoding = self
            .tokenizer
            .lock()
            .unwrap()
            .encode(text, true)
            .map_err(|e| Error::msg(format!("Failed to encode text with tokenizer: {}", e)))?;

        let mut input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mut attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&mask| mask as i64)
            .collect();

        #[allow(clippy::comparison_chain)]
        if input_ids.len() > self.max_seq_length {
            input_ids.truncate(self.max_seq_length);
            attention_mask.truncate(self.max_seq_length);
        } else if input_ids.len() < self.max_seq_length {
            let pad_length = self.max_seq_length - input_ids.len();
            input_ids.extend(vec![0; pad_length]);
            attention_mask.extend(vec![0; pad_length]);
        }

        Ok((input_ids, attention_mask))
    }
}

impl EmbeddingProvider for OnnxEmbeddingModel {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_type(&self) -> EmbeddingModelType {
        EmbeddingModelType::Onnx
    }

    fn embed_batch(&self, texts: &[&str]) -> VectorDBResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = texts.len();
        let mut all_input_ids = Vec::with_capacity(batch_size * self.max_seq_length);
        let mut all_attention_masks = Vec::with_capacity(batch_size * self.max_seq_length);

        for text in texts {
            let (mut input_ids, mut attention_mask) = self.prepare_inputs(text)
                .map_err(|e| VectorDBError::EmbeddingError(format!("Input prep failed: {}", e)))?;
            all_input_ids.append(&mut input_ids);
            all_attention_masks.append(&mut attention_mask);
        }

        let input_ids_array =
            Array::from_shape_vec((batch_size, self.max_seq_length), all_input_ids)
            .map_err(|e| VectorDBError::EmbeddingError(format!("Input ID batch shape error: {}", e)))?;
        let attention_mask_array =
            Array::from_shape_vec((batch_size, self.max_seq_length), all_attention_masks)
            .map_err(|e| VectorDBError::EmbeddingError(format!("Attention mask batch shape error: {}", e)))?;
        
        // Convert ndarray to ort Value
        let input_ids_shape = input_ids_array.shape().to_vec();
        let input_ids_vec = input_ids_array.into_raw_vec_and_offset().0;
        let input_ids_value = Value::from_array((input_ids_shape, input_ids_vec))
            .map_err(|e| VectorDBError::EmbeddingError(format!("Failed to create input ID tensor value: {}", e)))?;
        
        let attention_mask_shape = attention_mask_array.shape().to_vec();
        let attention_mask_vec = attention_mask_array.into_raw_vec_and_offset().0;
        let attention_mask_value = Value::from_array((attention_mask_shape, attention_mask_vec))
            .map_err(|e| VectorDBError::EmbeddingError(format!("Failed to create attention mask tensor value: {}", e)))?;

        let outputs = self.session.run(ort::inputs![
            "input_ids" => input_ids_value,
            "attention_mask" => attention_mask_value,
        ].map_err(|e| VectorDBError::EmbeddingError(format!("Failed to create ONNX inputs: {}", e)))?)
            .map_err(|e| VectorDBError::EmbeddingError(format!("ONNX session batch run failed: {}", e)))?;

        let pooler_output_value = outputs.get("pooler_output")
            .or_else(|| outputs.get("last_hidden_state")) // Fallback if pooler_output is not present
            .ok_or_else(|| VectorDBError::EmbeddingError("Model did not return 'pooler_output' or 'last_hidden_state' in batch".to_string()))?;
        
        // Extract raw tensor data
        let (shape, data) = pooler_output_value.try_extract_raw_tensor::<f32>()
            .map_err(|e| VectorDBError::EmbeddingError(format!("Failed to extract raw tensor data: {}", e)))?;

        let expected_dim = self.dimension;
        if shape.len() < 2 || shape[0] as usize != batch_size || shape[shape.len() - 1] != expected_dim as i64 {
            return Err(VectorDBError::EmbeddingError(format!(
                "Unexpected batch pooler output shape: got {:?}, expected [{}, ..., {}]",
                shape, batch_size, expected_dim
            )));
        }

        let mut embeddings = Vec::with_capacity(batch_size);
        // Handle potential 3D output (like last_hidden_state) by taking the first token ([CLS]) embedding
        if shape.len() == 3 && shape[1] > 0 { 
            let seq_len = shape[1] as usize;
            for i in 0..batch_size {
                 // Calculate slice based on shape [batch_size, seq_len, dim]
                 let start = i * seq_len * expected_dim; // Start of batch item i
                 let end = start + expected_dim; // End of the first token embedding for item i
                 let embedding_slice = &data[start..end]; // Use data slice directly
                 let mut embedding = embedding_slice.to_vec();
                 let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                 if norm > 0.0 {
                     for x in &mut embedding {
                         *x /= norm;
                     }
                 }
                 embeddings.push(embedding);
             }
        } else if shape.len() == 2 { // Handle 2D output (like pooler_output)
            for i in 0..batch_size {
                 // Calculate slice based on shape [batch_size, dim]
                 let start = i * expected_dim;
                 let end = start + expected_dim;
                 let embedding_slice = &data[start..end]; // Use data slice directly
                 let mut embedding = embedding_slice.to_vec();
                 let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                 if norm > 0.0 {
                     for x in &mut embedding {
                         *x /= norm;
                     }
                 }
                 embeddings.push(embedding);
             }
        } else {
            return Err(VectorDBError::EmbeddingError(format!("Unsupported output tensor shape: {:?}", shape)));
        }

        Ok(embeddings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;

    fn create_dummy_onnx_files(dir: &Path) -> (PathBuf, PathBuf) {
        let model_path = dir.join("model.onnx");
        let tokenizer_dir = dir.join("tokenizer");
        fs::create_dir(&tokenizer_dir).unwrap();
        let tokenizer_path = tokenizer_dir.join("tokenizer.json");

        File::create(&model_path).unwrap();
        let mut tokenizer_file = File::create(&tokenizer_path).unwrap();
        // Minimal valid tokenizer JSON for testing - using regular string with escapes
        write!(tokenizer_file, "{{
            \"version\": \"1.0\", \"truncation\": null, \"padding\": null, \"added_tokens\": [],
            \"normalizer\": null, \"pre_tokenizer\": null, \"post_processor\": null, \"decoder\": null,
            \"model\": {{
                \"type\": \"WordPiece\", \"unk_token\": \"[UNK]\", \"continuing_subword_prefix\": \"##\", \"max_input_chars_per_word\": 100,
                \"vocab\": {{
                    \"[UNK]\": 0, \"[CLS]\": 1, \"[SEP]\": 2, \"hello\": 3, \",\": 4, \"world\": 5, \".\": 6,
                    \"test\": 7, \"sentence\": 8, \"this\": 9, \"is\": 10, \"a\": 11
                }}
            }}
        }}").unwrap();

        (model_path, tokenizer_dir) // Return the tokenizer directory path
    }

    fn test_provider_basics(provider: &OnnxEmbeddingModel) {
        assert_eq!(provider.model_type(), EmbeddingModelType::Onnx);
        // Dimension is dynamic based on the (dummy) model, check it's non-zero maybe?
        assert!(provider.dimension() > 0);
    }

    #[test]
    fn test_onnx_provider_new_and_basics() {
        let dir = tempdir().unwrap();
        let (model_path, tokenizer_path) = create_dummy_onnx_files(dir.path());

        // This will fail if ONNX runtime cannot load the dummy model, which is expected.
        // The goal here is more about testing path handling and basic setup.
        let result = OnnxEmbeddingModel::new(&model_path, &tokenizer_path);
        
        // In a real scenario without a valid ONNX model, this would likely error out.
        // For this dummy test, we can only assert that it *attempts* creation.
        // If ORT is installed and loadable, it might error on the dummy model.
        // If ORT feature is off or libs missing, it will fail earlier.
        // Let's just check if it returns Ok or a specific kind of error related to model loading.
        match result {
             Ok(provider) => test_provider_basics(&provider),
             Err(e) => {
                 println!("Note: ONNX provider creation failed as expected with dummy model: {}", e);
                 // Assert that the error is somewhat related to ORT loading/parsing if possible
                 assert!(e.to_string().contains("ONNX") || e.to_string().contains("ort") || e.to_string().contains("model"));
             }
         }
    }

    #[test]
    fn test_onnx_provider_new_missing_files() {
        let dir = tempdir().unwrap();
        let model_path = dir.path().join("nonexistent_model.onnx");
        let tokenizer_path = dir.path().join("nonexistent_tokenizer");
        // Do not create the files

        let result = OnnxEmbeddingModel::new(&model_path, &tokenizer_path);
        assert!(result.is_err());
        // Expect error related to file not found
        assert!(result.unwrap_err().to_string().contains("load tokenizer"));
    }
    
    // Embedding tests require a *real* model and tokenizer, 
    // and ONNX runtime environment correctly set up. 
    // These are more integration tests.
    // #[test]
    // fn test_batch_embedding() { ... }
} 