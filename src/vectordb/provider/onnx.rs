use crate::vectordb::provider::EmbeddingProvider;
use anyhow::{Error, Result, anyhow};
use log::{debug};
use ndarray::{Array, Array2};
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::{DynValue, Value};
use ort::execution_providers::{CUDAExecutionProvider};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;

/// ONNX-based embedding provider
#[derive(Debug)]
pub struct OnnxEmbeddingProvider {
    /// The tokenizer for preprocessing input text
    tokenizer: Arc<Mutex<Tokenizer>>,
    /// Maximum sequence length for the model
    max_seq_length: usize,
    /// ONNX session for running inference
    session: Session,
    /// The actual dimension of the loaded model's embeddings
    dimension: usize,
}

impl OnnxEmbeddingProvider {
    /// Creates a new OnnxEmbeddingProvider from the given model and tokenizer paths
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        debug!(
            "Creating ONNX embedding provider with model: {}",
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
        ort::init()
            .with_name("vectordb-onnx")
            .with_execution_providers([cuda_provider.build()]) // Configure EPs here
            .commit()?;

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

        // Truncate or pad to the maximum sequence length
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

    /// Convert the ORT output tensor to a Vec<f32>
    fn extract_embedding(&self, pooler_output_value: &DynValue) -> Result<Vec<f32>> {
        let (shape, data) = pooler_output_value.try_extract_raw_tensor::<f32>()?;

        // Use self.dimension for validation
        let expected_dim = self.dimension;

        let embedding = match shape.len() {
            1 => {
                 if shape[0] as usize != expected_dim {
                     return Err(Error::msg(format!(
                         "Unexpected 1D pooler output shape: got {:?}, expected [{}]",
                         shape, expected_dim
                     )));
                 }
                 // Directly use the data slice since it's 1D
                 data.to_vec()
            }
            2 => {
                 let expected_shape = [1, expected_dim as i64];
                 if shape != expected_shape {
                     return Err(Error::msg(format!(
                         "Unexpected 2D pooler output shape: got {:?}, expected {:?}",
                         shape, expected_shape
                     )));
                 }
                 // Reconstruct ArrayView2 from shape and data
                 // We expect shape [1, dim], so we take the first row implicitly by taking the whole slice
                 // Ensure the length matches before creating the view to avoid panic
                 if data.len() != expected_dim {
                     return Err(Error::msg(format!(
                         "Data length {} mismatch for expected shape {:?}",
                         data.len(), expected_shape
                     )));
                 }
                 data.to_vec() // Data is already the correct slice for shape [1, dim]
            }
            _ => {
                return Err(Error::msg(format!(
                    "Pooler output has unexpected dimensionality: shape {:?}",
                    shape
                )));
            }
        };

        let mut normalized_embedding = embedding;

        // Normalize the embedding to unit length (L2 normalization)
        let norm: f32 = normalized_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut normalized_embedding {
                *x /= norm;
            }
        }

        Ok(normalized_embedding)
    }

    /// Normalize an embedding to unit length
    fn normalize_embedding(mut embedding: Vec<f32>) -> Vec<f32> {
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }
        embedding
    }
}

impl EmbeddingProvider for OnnxEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let (input_ids, attention_mask) = self.prepare_inputs(text)?;
        
        let input_ids_array = Array2::from_shape_vec((1, input_ids.len()), input_ids)
            .map_err(|e| anyhow!("Input ID shape error: {}", e))?; 
        let attention_mask_array =
            Array2::from_shape_vec((1, attention_mask.len()), attention_mask)
            .map_err(|e| anyhow!("Attention mask shape error: {}", e))?; 

        // Use Value::from_array with shape and Vec
        let input_ids_shape = input_ids_array.shape().to_vec();
        let input_ids_vec = input_ids_array.into_raw_vec_and_offset().0;
        let input_ids_value = Value::from_array((input_ids_shape, input_ids_vec))?;

        let attention_mask_shape = attention_mask_array.shape().to_vec();
        let attention_mask_vec = attention_mask_array.into_raw_vec_and_offset().0;
        let attention_mask_value = Value::from_array((attention_mask_shape, attention_mask_vec))?;

        // Use inputs! macro with Values
        let outputs = self.session.run(ort::inputs![input_ids_value, attention_mask_value]?)
             .map_err(|e| anyhow!("ONNX session run failed: {}", e))?; 

        // Extract pooler output (second output tensor)
        let pooler_output = outputs.get("pooler_output")
            .ok_or_else(|| Error::msg("Model did not return 'pooler_output'"))?;
        self.extract_embedding(pooler_output)
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = texts.len();
        let mut all_input_ids = Vec::with_capacity(batch_size * self.max_seq_length);
        let mut all_attention_masks = Vec::with_capacity(batch_size * self.max_seq_length);

        // Prepare inputs for all texts in the batch
        for text in texts {
            let (mut input_ids, mut attention_mask) = self.prepare_inputs(text)?;
            all_input_ids.append(&mut input_ids);
            all_attention_masks.append(&mut attention_mask);
        }

        let input_ids_array =
            Array::from_shape_vec((batch_size, self.max_seq_length), all_input_ids)
            .map_err(|e| anyhow!("Input ID batch shape error: {}", e))?;
        let attention_mask_array =
            Array::from_shape_vec((batch_size, self.max_seq_length), all_attention_masks)
            .map_err(|e| anyhow!("Attention mask batch shape error: {}", e))?;

        // Use Value::from_array with shape and Vec
        let input_ids_shape = input_ids_array.shape().to_vec();
        let input_ids_vec = input_ids_array.into_raw_vec_and_offset().0;
        let input_ids_value = Value::from_array((input_ids_shape, input_ids_vec))?;

        let attention_mask_shape = attention_mask_array.shape().to_vec();
        let attention_mask_vec = attention_mask_array.into_raw_vec_and_offset().0;
        let attention_mask_value = Value::from_array((attention_mask_shape, attention_mask_vec))?;

        // Use inputs! macro with Values
        let outputs = self.session.run(ort::inputs![input_ids_value, attention_mask_value]?)
             .map_err(|e| anyhow!("ONNX session run failed (batch): {}", e))?; 

        // Extract pooler output tensor view directly by name
        let pooler_output = outputs.get("pooler_output")
            .ok_or_else(|| Error::msg("Model did not return 'pooler_output'"))?;
        let (output_shape, output_data) = pooler_output.try_extract_raw_tensor::<f32>()?;

        // Check output shape: [batch_size, embedding_dim]
        let expected_dim = self.dimension;
        if output_shape.len() != 2
            || output_shape[0] as usize != batch_size
            || output_shape[1] as usize != expected_dim
        {
            return Err(Error::msg(format!(
                "Unexpected pooler output shape: got {:?}, expected [{}, {}]",
                output_shape, batch_size, expected_dim
            )));
        }

        // Extract individual embeddings and normalize
        let mut embeddings = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let start = i * expected_dim;
            let end = start + expected_dim;
            // Slice the raw data buffer
            if end > output_data.len() {
                 return Err(Error::msg(format!(
                    "Data slice index out of bounds for batch item {}: start {}, end {}, data len {}",
                    i, start, end, output_data.len()
                )));
            }
            let embedding_slice = &output_data[start..end];
            let embedding_vec = embedding_slice.to_vec();
            embeddings.push(Self::normalize_embedding(embedding_vec));
        }

        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectordb::provider::tests::test_provider_basics;
    use std::path::PathBuf;

    #[test]
    fn test_onnx_provider() {
        // Skip if model/tokenizer aren't available
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");

        if !model_path.exists() || !tokenizer_path.exists() {
            println!("Skipping test_onnx_provider because model/tokenizer files aren't available");
            return;
        }

        let provider = OnnxEmbeddingProvider::new(&model_path, &tokenizer_path);
        assert!(provider.is_ok());
        test_provider_basics(&provider.unwrap());
    }

    #[test]
    fn test_batch_embedding() {
        // Skip if model/tokenizer aren't available
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");

        if !model_path.exists() || !tokenizer_path.exists() {
            println!("Skipping test_batch_embedding because model/tokenizer files aren't available");
            return;
        }

        let provider = OnnxEmbeddingProvider::new(&model_path, &tokenizer_path).unwrap();
        let texts = vec!["Hello, world!", "This is a test sentence."];
        let embeddings = provider.embed_batch(&texts);

        assert!(embeddings.is_ok());
        let embeddings = embeddings.unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), provider.dimension);
        assert_eq!(embeddings[1].len(), provider.dimension);

        // Check normalization
        for embedding in &embeddings {
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 0.01);
        }

        // Check that embeddings are different
        assert_ne!(embeddings[0], embeddings[1]);
    }
}
