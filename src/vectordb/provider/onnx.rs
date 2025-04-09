use crate::vectordb::provider::EmbeddingProvider;
use anyhow::{Error, Result};
use log::debug;
use ndarray::{s, Array, Array2, Ix1, Ix2};
use ort::inputs;
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::DynValue;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;

/// Dimension of the ONNX MiniLM embeddings
pub const ONNX_EMBEDDING_DIM: usize = 384;

/// ONNX-based embedding provider using MiniLM model
pub struct OnnxEmbeddingProvider {
    /// The tokenizer for preprocessing input text
    tokenizer: Tokenizer,
    /// Maximum sequence length for the model
    max_seq_length: usize,
    /// ONNX session for running inference
    session: Arc<Mutex<Session>>,
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

        let tokenizer = Tokenizer::from_file(tokenizer_json_path)
            .map_err(|e| Error::msg(format!("Failed to load tokenizer: {}", e)))?;

        debug!("Tokenizer loaded successfully");

        // Create ONNX session
        debug!("Creating ONNX session with model path: {}", model_path.display());
        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level1)?
            .with_intra_threads(num_cpus::get())?
            .commit_from_file(model_path)?;

        debug!(
            "ONNX model loaded successfully from {}",
            model_path.display()
        );

        Ok(Self {
            tokenizer,
            max_seq_length: 128, // Default for MiniLM
            session: Arc::new(Mutex::new(session)),
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
        // try_extract_tensor returns an ArrayViewD directly
        let pooler_output_view = pooler_output_value.try_extract_tensor::<f32>()?;

        // Check if view is 1D or 2D with batch size 1
        let embedding = match pooler_output_view.ndim() {
            1 => {
                 // Assume it's shape [embedding_dim]
                 let view1d = pooler_output_view.into_dimensionality::<Ix1>()?;
                 if view1d.shape()[0] != ONNX_EMBEDDING_DIM {
                     return Err(Error::msg(format!(
                         "Unexpected 1D pooler output shape: got {:?}, expected [{}]",
                         view1d.shape(), ONNX_EMBEDDING_DIM
                     )));
                 }
                 view1d.to_vec()
            }
            2 => {
                // Assume it's shape [1, embedding_dim]
                let view2d = pooler_output_view.into_dimensionality::<Ix2>()?;
                let expected_shape = [1, ONNX_EMBEDDING_DIM];
                if view2d.shape() != expected_shape {
                    return Err(Error::msg(format!(
                        "Unexpected 2D pooler output shape: got {:?}, expected {:?}",
                        view2d.shape(), expected_shape
                    )));
                }
                // Extract the first (and only) embedding row and convert to Vec<f32>
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
        let output_shape = pooler_view.shape();
        if output_shape.len() != 2
            || output_shape[0] != batch_size
            || output_shape[1] != ONNX_EMBEDDING_DIM
        {
            return Err(Error::msg(format!(
                "Unexpected pooler output shape: got {:?}, expected [{}, {}]",
                output_shape, batch_size, ONNX_EMBEDDING_DIM
            )));
        }

        // Extract individual embeddings and normalize
        let mut results = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let embedding_slice = pooler_view.slice(ndarray::s![i, ..]);
            let embedding = embedding_slice.to_vec();
            results.push(Self::normalize_embedding(embedding));
        }

        Ok(results)
    }

    fn dimension(&self) -> usize {
        ONNX_EMBEDDING_DIM
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
        assert_eq!(embeddings[0].len(), ONNX_EMBEDDING_DIM);
        assert_eq!(embeddings[1].len(), ONNX_EMBEDDING_DIM);

        // Check normalization
        for embedding in &embeddings {
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 0.01);
        }

        // Check that embeddings are different
        assert_ne!(embeddings[0], embeddings[1]);
    }
}
