use crate::vectordb::provider::EmbeddingProvider;
use anyhow::{Error, Result, anyhow};
use log::{debug};
use ndarray::{Array, Array2};
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::{Value};
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

        let (shape, data) = pooler_output.try_extract_raw_tensor::<f32>()?;

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
            .map_err(|e| anyhow!("ONNX session batch run failed: {}", e))?;

        let pooler_output_value = outputs.get("pooler_output")
            .ok_or_else(|| Error::msg("Model did not return 'pooler_output' in batch"))?;

        // Extract the raw tensor data for the batch
        let (shape, data) = pooler_output_value.try_extract_raw_tensor::<f32>()?;

        // Validate batch shape
        let expected_dim = self.dimension;
        if shape.len() != 2 || shape[0] as usize != batch_size || shape[1] as usize != expected_dim {
            return Err(Error::msg(format!(
                "Unexpected batch pooler output shape: got {:?}, expected [{}, {}]",
                shape, batch_size, expected_dim
            )));
        }

        // Process each embedding in the batch
        let embeddings: Result<Vec<Vec<f32>>> = data
            .chunks_exact(expected_dim) // Split the flat data into chunks of size expected_dim
            .map(|embedding_slice| {
                // Normalize each individual embedding slice
                let mut embedding = embedding_slice.to_vec();
                let norm = embedding.iter().map(|&x| x * x).sum::<f32>().sqrt();
                if norm > 1e-6 {
                    for x in &mut embedding {
                        *x /= norm;
                    }
                    // Optional: Verify norm is close to 1
                    // let norm_after = embedding.iter().map(|&x| x * x).sum::<f32>().sqrt();
                    // assert!((norm_after - 1.0).abs() < 1e-5);
                }
                Ok(embedding)
            })
            .collect(); // Collect into Result<Vec<Vec<f32>>>

        embeddings
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
