use crate::vectordb::provider::EmbeddingProvider;
use crate::vectordb::embedding::EmbeddingModelType;
use anyhow::{Error, Result};
use log::{debug};
use ndarray::{Array};
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::{Value};
use ort::execution_providers::{CUDAExecutionProvider};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;
use crate::vectordb::error::{Result as VectorDBResult, VectorDBError};

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

        // Determine dimension
        let pooler_output_name = "pooler_output"; 
        let output_dim = session.outputs.iter()
            .find(|meta| meta.name == pooler_output_name)
            .and_then(|meta| {
                match &meta.output_type {
                    ort::value::ValueType::Tensor { dimensions, .. } => {
                        // Assume dimensions.last() gives Option<&i64>
                        dimensions.last().map(|dim_ref| *dim_ref as usize)
                    }
                    _ => None,
                }
            })
            .ok_or_else(|| Error::msg(format!("Could not determine embedding dimension from model output '{}'", pooler_output_name)))?;

        debug!(
            "ONNX model loaded successfully from {}, determined embedding dimension: {}",
            model_path.display(),
            output_dim
        );

        let tokenizer = Arc::new(Mutex::new(tokenizer));

        Ok(Self {
            session,
            tokenizer,
            max_seq_length: 128, // TODO: Make this configurable or detect from model?
            dimension: output_dim,
        })
    }

    /// Tokenizes input text and prepares model inputs
    fn prepare_inputs(&self, text: &str) -> Result<(Vec<i64>, Vec<i64>)> {
        // Encode the text with the tokenizer
        let encoding = self
            .tokenizer
            .lock()
            .unwrap()
            .encode(text, true)
            .map_err(|e| Error::msg(format!("Failed to encode text with tokenizer: {}", e)))?;

        // Get input IDs and attention mask
        let mut input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mut attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&mask| mask as i64)
            .collect();

        // Pad or truncate the input IDs and attention mask
        #[allow(clippy::comparison_chain)] // Keep if/else if for readability
        if input_ids.len() > self.max_seq_length {
            // Truncate
            input_ids.truncate(self.max_seq_length);
            attention_mask.truncate(self.max_seq_length);
        } else if input_ids.len() < self.max_seq_length {
            // Pad
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

        // Prepare inputs for all texts in the batch
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

        // Use Value::from_array with shape and Vec
        let input_ids_shape = input_ids_array.shape().to_vec();
        let input_ids_vec = input_ids_array.into_raw_vec_and_offset().0;
        let input_ids_value = Value::from_array((input_ids_shape, input_ids_vec))
            .map_err(|e| VectorDBError::EmbeddingError(format!("Failed to create input ID tensor value: {}", e)))?;

        let attention_mask_shape = attention_mask_array.shape().to_vec();
        let attention_mask_vec = attention_mask_array.into_raw_vec_and_offset().0;
        let attention_mask_value = Value::from_array((attention_mask_shape, attention_mask_vec))
             .map_err(|e| VectorDBError::EmbeddingError(format!("Failed to create attention mask tensor value: {}", e)))?;

        // Use inputs! macro with Values
        let outputs = self.session.run(ort::inputs![input_ids_value, attention_mask_value,]?)
            .map_err(|e| VectorDBError::EmbeddingError(format!("ONNX session batch run failed: {}", e)))?;

        let pooler_output_value = outputs.get("pooler_output")
            .ok_or_else(|| VectorDBError::EmbeddingError("Model did not return 'pooler_output' in batch".to_string()))?;

        // Extract the raw tensor data for the batch
        let (shape, data) = pooler_output_value.try_extract_raw_tensor::<f32>()
            .map_err(|e| VectorDBError::EmbeddingError(format!("Failed to extract raw tensor data: {}", e)))?;

        // Validate batch shape
        let expected_dim = self.dimension;
        if shape.len() != 2 || shape[0] as usize != batch_size || shape[1] as usize != expected_dim {
            return Err(VectorDBError::EmbeddingError(format!(
                "Unexpected batch pooler output shape: got {:?}, expected [{}, {}]",
                shape, batch_size, expected_dim
            )));
        }

        // Process the batch output
        let mut embeddings = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let start = i * expected_dim;
            let end = start + expected_dim;
            let embedding_slice = &data[start..end];
            
            // Normalize each embedding
            let mut embedding = embedding_slice.to_vec();
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut embedding {
                    *x /= norm;
                }
            }
            embeddings.push(embedding);
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

    // Helper to create dummy model/tokenizer files for testing
    fn create_dummy_onnx_files(dir: &Path) -> (PathBuf, PathBuf) {
        let model_path = dir.join("model.onnx");
        let tokenizer_dir = dir.join("tokenizer");
        fs::create_dir(&tokenizer_dir).unwrap();
        let tokenizer_path = tokenizer_dir.join("tokenizer.json");

        // Create empty files (real tests would need valid ONNX/tokenizer files)
        File::create(&model_path).unwrap();
        let mut tokenizer_file = File::create(&tokenizer_path).unwrap();
        // Minimal valid tokenizer JSON (ensure correct quoting and structure)
        write!(tokenizer_file, "{{\"version\":\"1.0\",\"truncation\":null,\"padding\":null,\"added_tokens\":[],\"normalizer\":null,\"pre_tokenizer\":null,\"post_processor\":null,\"decoder\":null,\"model\":{{\"type\":\"WordPiece\",\"unk_token\":\"[UNK]\",\"continuing_subword_prefix\":\"##\",\"max_input_chars_per_word\":100,\"vocab\":{{\"[UNK]\":0,\"[CLS]\":1,\"[SEP]\":2,\"hello\":3,\",\":4,\"world\":5,\".\":6,\"test\":7,\"sentence\":8,\"this\":9,\"is\":10,\"a\":11}}}}}}").unwrap();

        (model_path, tokenizer_dir) // Return the tokenizer directory path
    }

    // Helper to test basic provider functionality
    fn test_provider_basics(provider: &OnnxEmbeddingModel) {
        assert_eq!(provider.model_type(), EmbeddingModelType::Onnx);
        // Dimension assertion might depend on the dummy model, skip for now or make dynamic
        // assert_eq!(provider.dimension(), 384); // Example dimension
    }

    #[test]
    #[ignore] // Ignore by default as it needs a real ONNX model and tokenizer
    fn test_onnx_provider_new_and_basics() {
        let dir = tempdir().unwrap();
        let (model_path, tokenizer_path) = create_dummy_onnx_files(dir.path());

        // Test successful creation
        let provider = OnnxEmbeddingModel::new(&model_path, &tokenizer_path);
        assert!(provider.is_ok());
        test_provider_basics(&provider.unwrap());
    }

    #[test]
    #[ignore] // Ignore by default
    fn test_batch_embedding() {
        let dir = tempdir().unwrap();
        let (model_path, tokenizer_path) = create_dummy_onnx_files(dir.path());

        let provider = OnnxEmbeddingModel::new(&model_path, &tokenizer_path).unwrap();
        let texts = vec!["Hello, world!", "This is a test sentence."];
        let embeddings = provider.embed_batch(&texts);

        assert!(embeddings.is_ok());
        let embeddings = embeddings.unwrap();
        assert_eq!(embeddings.len(), 2); // Should get two embeddings
        // Check dimensions (assuming dummy model outputs correctly)
        // assert_eq!(embeddings[0].len(), provider.dimension());
        // assert_eq!(embeddings[1].len(), provider.dimension());
    }
}
