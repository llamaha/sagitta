use crate::vectordb::provider::EmbeddingProvider;
use anyhow::{Error, Result};
use log::{debug, error};
use ndarray::{s, Array, Array2, Ix1, Ix2};
use ort::inputs;
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::DynValue;
use ort::execution_providers::{CUDAExecutionProvider, ExecutionProvider};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;

/// ONNX-based embedding provider
pub struct OnnxEmbeddingProvider {
    /// The tokenizer for preprocessing input text
    tokenizer: Tokenizer,
    /// Maximum sequence length for the model
    max_seq_length: usize,
    /// ONNX session for running inference
    session: Arc<Mutex<Session>>,
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

        // Create ONNX session
        debug!("Creating ONNX session with model path: {}", model_path.display());

        // --- Configure Execution Provider ---
        // Check if CUDA is available and intended
        // For now, unconditionally try to add CUDA if the feature is enabled.
        // The build flag 'ort/cuda' should ensure the necessary library is present.
        let cuda_provider_config = CUDAExecutionProvider::default(); // Create the config struct first

        // --- Check CUDA Provider Status ---
        // Call is_available() on the specific provider config struct
        match cuda_provider_config.is_available() {
            Ok(true) => debug!("CUDA Execution Provider reports available."),
            Ok(false) => debug!("CUDA Execution Provider reports *not* available. Ensure ONNX Runtime was built with CUDA support and CUDA drivers/runtime are correctly installed and found."),
            Err(e) => debug!("Error checking CUDA Execution Provider availability: {}", e),
        }

        // Now build the dispatchable provider for the session builder
        let cuda_provider_dispatch = cuda_provider_config.build();

        let session = Session::builder()?
            .with_execution_providers([cuda_provider_dispatch])? // Use the built dispatch provider
            .with_optimization_level(GraphOptimizationLevel::Level1)?
            // .with_intra_threads(num_cpus::get())? // Typically not needed when using GPU
            .commit_from_file(model_path)?;

        // --- ADDED: Determine dimension from model output ---
        let pooler_output_name = "pooler_output"; // Assuming this is the output we use
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

        Ok(Self {
            tokenizer,
            max_seq_length: 128, // TODO: Make this configurable or detect from model?
            session: Arc::new(Mutex::new(session)),
            dimension: output_dim,
        })
    }

    /// Tokenizes input text and prepares model inputs
    fn prepare_inputs(&self, text: &str) -> Result<(Vec<i64>, Vec<i64>)> {
        // Encode the text with the tokenizer
        let encoding = self
            .tokenizer
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
        let pooler_output_view = pooler_output_value.try_extract_tensor::<f32>()?;

        // Use self.dimension for validation
        let expected_dim = self.dimension;

        let embedding = match pooler_output_view.ndim() {
            1 => {
                let view1d = pooler_output_view.into_dimensionality::<Ix1>()?;
                if view1d.shape()[0] != expected_dim {
                    return Err(Error::msg(format!(
                        "Unexpected 1D pooler output shape: got {:?}, expected [{}]",
                        view1d.shape(), expected_dim
                    )));
                }
                view1d.to_vec()
            }
            2 => {
                let view2d = pooler_output_view.into_dimensionality::<Ix2>()?;
                let expected_shape = [1, expected_dim];
                if view2d.shape() != expected_shape {
                    return Err(Error::msg(format!(
                        "Unexpected 2D pooler output shape: got {:?}, expected {:?}",
                        view2d.shape(), expected_shape
                    )));
                }
                view2d.slice(s![0, ..]).to_vec()
            }
            _ => {
                return Err(Error::msg(format!(
                    "Pooler output has unexpected dimensionality: {:?}",
                    pooler_output_view.shape()
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
        // Prepare inputs
        let (input_ids, attention_mask) = self.prepare_inputs(text)?;

        // Lock the session for inference
        let session = self.session.lock().unwrap();

        // Create input tensors with proper shapes for ORT
        let input_ids_array = Array2::from_shape_vec((1, input_ids.len()), input_ids)?;
        let attention_mask_array =
            Array2::from_shape_vec((1, attention_mask.len()), attention_mask)?;

        // Use the inputs! macro. Assumes positional inputs "input_ids", "attention_mask".
        // The macro handles Array -> OrtOwnedTensor internally.
        let inputs = inputs![input_ids_array, attention_mask_array]?;

        // Run inference
        let outputs = session.run(inputs)?;

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

        // Lock the session for inference
        let session = self.session.lock().unwrap();

        // Create input tensors with shape [batch_size, sequence_length]
        let input_ids_array =
            Array::from_shape_vec((batch_size, self.max_seq_length), all_input_ids)?;
        let attention_mask_array =
            Array::from_shape_vec((batch_size, self.max_seq_length), all_attention_masks)?;

        // Use the inputs! macro. Assumes positional inputs "input_ids", "attention_mask".
        let inputs = inputs![input_ids_array, attention_mask_array]?;

        // Run inference
        let outputs = session.run(inputs)?;

        // Extract pooler output tensor view directly by name
        let pooler_output = outputs.get("pooler_output")
            .ok_or_else(|| Error::msg("Model did not return 'pooler_output'"))?;
        let pooler_view = pooler_output.try_extract_tensor::<f32>()?;

        // Check output shape: [batch_size, embedding_dim]
        let expected_dim = self.dimension;
        let output_shape = pooler_view.shape();
        if output_shape.len() != 2
            || output_shape[0] != batch_size
            || output_shape[1] != expected_dim
        {
            return Err(Error::msg(format!(
                "Unexpected pooler output shape: got {:?}, expected [{}, {}]",
                output_shape, batch_size, expected_dim
            )));
        }

        // Extract individual embeddings and normalize
        let mut embeddings = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let embedding_view = pooler_view.slice(s![i, ..]);
            let embedding_vec = embedding_view.to_vec();
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
