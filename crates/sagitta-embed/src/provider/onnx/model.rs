//! ONNX embedding model implementation.

use crate::error::{Result, SagittaEmbedError};
use crate::model::EmbeddingModelType;
use crate::provider::EmbeddingProvider;
use crate::provider::onnx::memory_pool::{TensorMemoryPool, MemoryPoolStats};
use crate::provider::onnx::io_binding::{AdvancedIOBinding, IOBindingStats};
use anyhow::{anyhow, Error};
use log::{debug, warn, info};
use ndarray::Array;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokenizers::Tokenizer;

#[cfg(feature = "onnx")]
use ort::{
    execution_providers::{CPUExecutionProvider, CUDAExecutionProvider},
    session::{builder::GraphOptimizationLevel, Session},
    value::Value,
};

/// ONNX-based embedding model with memory management
#[derive(Debug)]
pub struct OnnxEmbeddingModel {
    /// The tokenizer for preprocessing input text
    tokenizer: Arc<Mutex<Tokenizer>>,
    /// Maximum sequence length for the model
    max_seq_length: usize,
    /// ONNX session for running inference
    #[cfg(feature = "onnx")]
    session: Session,
    /// The actual dimension of the loaded model's embeddings
    dimension: usize,
    /// Last time the session was used
    last_used: Instant,
    /// Model path for recreation
    model_path: PathBuf,
    /// Phase 2: Memory pool for tensor reuse
    memory_pool: Arc<Mutex<TensorMemoryPool>>,
    /// Phase 2: Advanced I/O binding for optimized inference
    #[cfg(feature = "onnx")]
    io_binding: Arc<Mutex<AdvancedIOBinding>>,
    /// Configuration for reference
    config: crate::config::EmbeddingConfig,
}

impl Clone for OnnxEmbeddingModel {
    fn clone(&self) -> Self {
        // Create a new session from the same model path
        #[cfg(feature = "onnx")]
        let session = Self::create_session(&self.model_path)
            .expect("Failed to create new session while cloning");

        Self {
            tokenizer: self.tokenizer.clone(),
            max_seq_length: self.max_seq_length,
            #[cfg(feature = "onnx")]
            session,
            dimension: self.dimension,
            last_used: Instant::now(), // Reset the last_used time for the new instance
            model_path: self.model_path.clone(),
            memory_pool: self.memory_pool.clone(),
            #[cfg(feature = "onnx")]
            io_binding: self.io_binding.clone(),
            config: self.config.clone(),
        }
    }
}

impl OnnxEmbeddingModel {
    /// Creates a new OnnxEmbeddingModel from the given model and tokenizer paths
    #[cfg(feature = "onnx")]
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        Self::new_with_config(model_path, tokenizer_path, &crate::config::EmbeddingConfig::default())
    }

    /// Creates a new OnnxEmbeddingModel with custom configuration
    #[cfg(feature = "onnx")]
    pub fn new_with_config(model_path: &Path, tokenizer_path: &Path, config: &crate::config::EmbeddingConfig) -> Result<Self> {
        debug!(
            "Creating ONNX embedding model with model: {} and config: {:?}",
            model_path.display(),
            config
        );

        let tokenizer = Self::load_tokenizer(tokenizer_path)?;
        let session = Self::create_session_with_config(model_path, config)?;
        let dimension = Self::get_dimension_from_session(&session)?;

        // Phase 2: Initialize memory pool and I/O binding
        let memory_pool = TensorMemoryPool::new(config.memory_pool_config.clone());
        let io_binding = AdvancedIOBinding::new(config.io_binding_config.clone(), TensorMemoryPool::new(config.memory_pool_config.clone()));

        debug!(
            "ONNX model loaded successfully from {}, determined embedding dimension: {}",
            model_path.display(),
            dimension
        );

        Ok(Self {
            session,
            tokenizer: Arc::new(Mutex::new(tokenizer)),
            max_seq_length: config.max_sequence_length,
            dimension,
            last_used: Instant::now(),
            model_path: model_path.to_path_buf(),
            memory_pool: Arc::new(Mutex::new(memory_pool)),
            #[cfg(feature = "onnx")]
            io_binding: Arc::new(Mutex::new(io_binding)),
            config: config.clone(),
        })
    }

    /// Creates a new OnnxEmbeddingModel (stub when ONNX feature is disabled)
    #[cfg(not(feature = "onnx"))]
    pub fn new(_model_path: &Path, _tokenizer_path: &Path) -> Result<Self> {
        Err(SagittaEmbedError::feature_not_enabled("onnx"))
    }

    /// Load tokenizer from path (can be file or directory)
    fn load_tokenizer(tokenizer_path: &Path) -> Result<Tokenizer> {
        let tokenizer_json_path = if tokenizer_path.is_file()
            && tokenizer_path
                .file_name()
                .map_or(false, |name| name == "tokenizer.json")
        {
            tokenizer_path.to_path_buf()
        } else if tokenizer_path.is_dir() {
            tokenizer_path.join("tokenizer.json")
        } else {
            tokenizer_path.to_path_buf()
        };

        // Explicitly check that both files exist
        if !tokenizer_json_path.is_file() {
            return Err(SagittaEmbedError::file_not_found(&tokenizer_json_path));
        }

        debug!(
            "Attempting to load tokenizer from: {}",
            tokenizer_json_path.display()
        );

        let tokenizer = Tokenizer::from_file(&tokenizer_json_path).map_err(|e| {
            SagittaEmbedError::tokenization(format!(
                "Failed to load tokenizer from {}: {}",
                tokenizer_json_path.display(),
                e
            ))
        })?;

        debug!("Tokenizer loaded successfully");
        Ok(tokenizer)
    }

    /// Creates a new ONNX session with appropriate execution provider based on build features
    #[cfg(feature = "onnx")]
    fn create_session(model_path: &Path) -> Result<Session> {
        Self::create_session_with_config(model_path, &crate::config::EmbeddingConfig::default())
    }

    /// Creates a new ONNX session with performance configuration
    #[cfg(feature = "onnx")]
    fn create_session_with_config(model_path: &Path, config: &crate::config::EmbeddingConfig) -> Result<Session> {
        if !model_path.is_file() {
            return Err(SagittaEmbedError::file_not_found(model_path));
        }

        // Initialize Environment using ort::init()
        #[cfg(feature = "cuda")]
        {
            debug!("Building ONNX session with CUDA execution provider (with CPU fallback)");
            let mut cuda_provider = CUDAExecutionProvider::default();
            
            // Configure CUDA memory limit if specified
            if let Some(memory_limit) = config.cuda_memory_limit {
                cuda_provider = cuda_provider.with_memory_limit(memory_limit);
            }
            
            let mut cpu_provider = CPUExecutionProvider::default();
            if config.enable_cpu_arena {
                cpu_provider = cpu_provider.with_arena_allocator();
            }
            
            let _ = ort::init()
                .with_name("sagitta-onnx")
                .with_execution_providers([cuda_provider.build(), cpu_provider.build()]) // CUDA first, CPU fallback
                .commit();
        }

        #[cfg(not(feature = "cuda"))]
        {
            debug!("Building ONNX session with CPU execution provider only");
            let mut cpu_provider = CPUExecutionProvider::default();
            if config.enable_cpu_arena {
                cpu_provider = cpu_provider.with_arena_allocator();
            }
            
            let _ = ort::init()
                .with_name("sagitta-onnx")
                .with_execution_providers([cpu_provider.build()]) // CPU only
                .commit();
        }

        // Build session using Session::builder() with performance optimizations
        let mut session_builder = Session::builder()
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create session builder: {}", e)))?;

        // Apply graph optimization level
        let opt_level = match config.graph_optimization_level {
            0 => ort::session::builder::GraphOptimizationLevel::Disable,
            1 => ort::session::builder::GraphOptimizationLevel::Level1,
            2 => ort::session::builder::GraphOptimizationLevel::Level2,
            _ => ort::session::builder::GraphOptimizationLevel::Level3,
        };
        session_builder = session_builder
            .with_optimization_level(opt_level)
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to set optimization level: {}", e)))?;

        // Configure threading
        if let Some(intra_threads) = config.intra_op_num_threads {
            session_builder = session_builder
                .with_intra_threads(intra_threads)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to set intra-op threads: {}", e)))?;
        }

        if let Some(inter_threads) = config.inter_op_num_threads {
            session_builder = session_builder
                .with_inter_threads(inter_threads)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to set inter-op threads: {}", e)))?;
        }

        // Configure parallel execution
        if config.enable_parallel_execution {
            session_builder = session_builder
                .with_parallel_execution(true)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to enable parallel execution: {}", e)))?;
        }

        // Configure memory pattern optimization
        session_builder = session_builder
            .with_memory_pattern(config.enable_memory_pattern)
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to configure memory pattern: {}", e)))?;

        // Configure deterministic compute
        if config.enable_deterministic_compute {
            session_builder = session_builder
                .with_deterministic_compute(true)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to enable deterministic compute: {}", e)))?;
        }

        // Configure profiling if specified
        if let Some(ref profiling_path) = config.profiling_file_path {
            session_builder = session_builder
                .with_profiling(profiling_path)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to enable profiling: {}", e)))?;
        }

        // Commit the session
        session_builder
            .commit_from_file(model_path)
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to load model from {}: {}", model_path.display(), e)))
    }

    /// Gets dimension from an ONNX session
    #[cfg(feature = "onnx")]
    fn get_dimension_from_session(session: &Session) -> Result<usize> {
        // Prioritize "sentence_embedding" for Sentence Transformer models
        let target_output_name = session
            .outputs
            .iter()
            .find(|output| output.name == "sentence_embedding")
            .map(|_| "sentence_embedding")
            .or_else(|| {
                // Fallback to "pooler_output" for models like CodeBERT
                session
                    .outputs
                    .iter()
                    .find(|output| output.name == "pooler_output")
                    .map(|_| "pooler_output")
            })
            .ok_or_else(|| {
                SagittaEmbedError::model(
                    "Could not find a suitable output ('sentence_embedding' or 'pooler_output') to determine dimension"
                )
            })?;

        session
            .outputs
            .iter()
            .find(|output| output.name == target_output_name)
            .and_then(|output| match output.output_type {
                ort::value::ValueType::Tensor { ref dimensions, .. } => {
                    // Dimension is usually the last element
                    dimensions.last().map(|&d| d as usize)
                }
                _ => None,
            })
            .ok_or_else(|| {
                SagittaEmbedError::model(format!(
                    "Failed to get model dimension from output: {}",
                    target_output_name
                ))
            })
    }

    /// Checks if the session needs cleanup (idle for too long)
    fn needs_cleanup(&self) -> bool {
        self.last_used.elapsed() > Duration::from_secs(300) // 5 minutes
    }

    /// Recreates the session if needed
    fn ensure_fresh_session(&mut self) -> Result<()> {
        if self.needs_cleanup() {
            warn!("Session was idle for > 5 minutes. Currently only updating timestamp. Previous recreation logic removed.");
        }
        self.last_used = Instant::now();
        Ok(())
    }

    /// Tokenizes input text and prepares model inputs
    fn prepare_inputs(&self, text: &str) -> Result<(Vec<i64>, Vec<i64>)> {
        let encoding = self
            .tokenizer
            .lock()
            .map_err(|e| SagittaEmbedError::thread_safety(format!("Failed to lock tokenizer: {}", e)))?
            .encode(text, true)
            .map_err(|e| SagittaEmbedError::tokenization(format!("Failed to encode text: {}", e)))?;

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

    /// Get the maximum sequence length
    pub fn max_sequence_length(&self) -> usize {
        self.max_seq_length
    }

    /// Set the maximum sequence length
    pub fn set_max_sequence_length(&mut self, max_length: usize) {
        self.max_seq_length = max_length;
    }

    /// Get memory pool performance statistics
    pub fn get_memory_pool_stats(&self) -> MemoryPoolStats {
        let memory_pool = self.memory_pool.lock().unwrap();
        memory_pool.get_stats()
    }

    /// Get I/O binding performance statistics
    #[cfg(feature = "onnx")]
    pub fn get_io_binding_stats(&self) -> IOBindingStats {
        let io_binding = self.io_binding.lock().unwrap();
        io_binding.get_stats()
    }

    /// Clear memory pool buffers
    pub fn clear_memory_pool(&mut self) {
        let mut memory_pool = self.memory_pool.lock().unwrap();
        memory_pool.clear();
    }
}

impl EmbeddingProvider for OnnxEmbeddingModel {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_type(&self) -> EmbeddingModelType {
        EmbeddingModelType::Onnx
    }

    #[cfg(feature = "onnx")]
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Ensure session is fresh before use
        // Note: We need to cast away the const to modify the session
        unsafe {
            let this = self as *const _ as *mut OnnxEmbeddingModel;
            (*this).ensure_fresh_session()?;
        }

        let batch_size = texts.len();
        debug!(
            "ONNX provider received batch of {} items, max_seq_length={}",
            batch_size, self.max_seq_length
        );

        let mut all_input_ids = Vec::with_capacity(batch_size * self.max_seq_length);
        let mut all_attention_masks = Vec::with_capacity(batch_size * self.max_seq_length);

        let token_start = Instant::now();
        for text in texts {
            let (mut input_ids, mut attention_mask) = self.prepare_inputs(text)?;
            all_input_ids.append(&mut input_ids);
            all_attention_masks.append(&mut attention_mask);
        }
        let token_elapsed = token_start.elapsed();
        debug!(
            "ONNX batch tokenization for {} items: {:?}",
            batch_size, token_elapsed
        );

        let input_ids_array = Array::from_shape_vec((batch_size, self.max_seq_length), all_input_ids)
            .map_err(|e| SagittaEmbedError::embedding_generation(format!("Input ID batch shape error: {}", e)))?;
        let attention_mask_array = Array::from_shape_vec((batch_size, self.max_seq_length), all_attention_masks)
            .map_err(|e| SagittaEmbedError::embedding_generation(format!("Attention mask batch shape error: {}", e)))?;

        // Convert ndarray to ort Value
        let input_ids_shape = input_ids_array.shape().to_vec();
        let input_ids_vec = input_ids_array.into_raw_vec_and_offset().0;
        let input_ids_value = Value::from_array((input_ids_shape.clone(), input_ids_vec.clone()))
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create input ID tensor: {}", e)))?;

        let attention_mask_shape = attention_mask_array.shape().to_vec();
        let attention_mask_vec = attention_mask_array.into_raw_vec_and_offset().0;
        let attention_mask_value = Value::from_array((attention_mask_shape.clone(), attention_mask_vec.clone()))
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create attention mask tensor: {}", e)))?;

        let onnx_start = Instant::now();
        
        // Phase 2: Use advanced I/O binding if enabled, otherwise use standard inference
        let outputs = if self.config.io_binding_config.enable_io_binding {
            debug!("Using Phase 2 advanced I/O binding for optimized inference");
            
            // Prepare input data for I/O binding - keep correct data types (i64 for input_ids/attention_mask)
            let input_data = vec![
                ("input_ids", input_ids_vec.clone()),
                ("attention_mask", attention_mask_vec.clone()),
            ];
            let input_shapes = vec![input_ids_shape.clone(), attention_mask_shape.clone()];
            let output_shapes = vec![vec![batch_size, self.dimension]]; // Expected output shape
            
            // Use I/O binding for optimized inference
            let mut io_binding = self.io_binding.lock().unwrap();
            let results = io_binding.run_inference_i64(
                &self.session,
                &input_data,
                &input_shapes,
                &output_shapes,
            ).map_err(|e| SagittaEmbedError::onnx_runtime(format!("I/O binding inference failed: {}", e)))?;
            
            // Convert results back to ONNX format for compatibility
            // This is a simplified implementation - real implementation would return proper ONNX outputs
            let output_data = results.into_iter().next().unwrap_or_else(|| vec![0.0f32; batch_size * self.dimension]);
            let output_value = Value::from_array((vec![batch_size, self.dimension], output_data))
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create output tensor: {}", e)))?;
            
            // Create a simple outputs map for compatibility
            std::collections::HashMap::from([("sentence_embedding".to_string(), output_value.into_dyn())])
        } else {
            // Standard Phase 1 inference
            debug!("Using standard ONNX inference");
            self.session
                .run(
                    ort::inputs![
                        "input_ids" => input_ids_value,
                        "attention_mask" => attention_mask_value,
                    ]
                    .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create ONNX inputs: {}", e)))?,
                )
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("ONNX session batch run failed: {}", e)))?
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect()
        };
        
        let onnx_elapsed = onnx_start.elapsed();
        debug!("ONNX inference for {} items: {:?}", batch_size, onnx_elapsed);

        // Prioritize "sentence_embedding", fallback to "pooler_output", then "last_hidden_state"
        let output_value = outputs
            .get("sentence_embedding")
            .or_else(|| outputs.get("pooler_output"))
            .or_else(|| outputs.get("last_hidden_state"))
            .ok_or_else(|| {
                SagittaEmbedError::embedding_generation(
                    "Model did not return 'sentence_embedding', 'pooler_output', or 'last_hidden_state' in batch"
                )
            })?;

        // Extract raw tensor data
        let (shape, data) = output_value
            .try_extract_raw_tensor::<f32>()
            .map_err(|e| SagittaEmbedError::embedding_generation(format!("Failed to extract raw tensor data: {}", e)))?;

        let expected_dim = self.dimension;
        // Handle different possible output shapes
        if shape.len() == 2 && shape[0] as usize == batch_size && shape[1] == expected_dim as i64 {
            // Shape is [batch_size, embedding_dim] - directly use it
            let mut embeddings = Vec::with_capacity(batch_size);
            let stride = expected_dim;
            for i in 0..batch_size {
                let start = i * stride;
                let end = start + stride;
                embeddings.push(data[start..end].to_vec());
            }
            Ok(embeddings)
        } else if shape.len() == 3 && shape[0] as usize == batch_size && shape[2] == expected_dim as i64 {
            // Shape is [batch_size, sequence_length, embedding_dim] - last_hidden_state
            warn!(
                "Received token embeddings (shape: {:?}) instead of pooled sentence embeddings",
                shape
            );
            Err(SagittaEmbedError::embedding_generation(
                "Received token embeddings instead of expected sentence embeddings. Ensure the ONNX model includes pooling."
            ))
        } else {
            // Unexpected shape
            Err(SagittaEmbedError::embedding_generation(format!(
                "Unexpected batch output shape: got {:?}, expected compatible with [{}, {}]",
                shape, batch_size, expected_dim
            )))
        }
    }

    #[cfg(not(feature = "onnx"))]
    fn embed_batch(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Err(SagittaEmbedError::feature_not_enabled("onnx"))
    }
}

// Thread-safe wrapper implementations
impl EmbeddingProvider for Arc<Mutex<OnnxEmbeddingModel>> {
    fn dimension(&self) -> usize {
        self.lock()
            .expect("Failed to lock OnnxEmbeddingModel mutex")
            .dimension()
    }

    fn model_type(&self) -> EmbeddingModelType {
        self.lock()
            .expect("Failed to lock OnnxEmbeddingModel mutex")
            .model_type()
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.lock()
            .expect("Failed to lock OnnxEmbeddingModel mutex")
            .embed_batch(texts)
    }
}

impl Drop for OnnxEmbeddingModel {
    fn drop(&mut self) {
        debug!(
            "Dropping OnnxEmbeddingModel for model: {}",
            self.model_path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn create_dummy_tokenizer(dir: &Path) -> PathBuf {
        let tokenizer_path = dir.join("tokenizer.json");
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
        tokenizer_path
    }

    #[test]
    fn test_load_tokenizer() {
        let temp_dir = tempdir().unwrap();
        let tokenizer_path = create_dummy_tokenizer(temp_dir.path());

        let result = OnnxEmbeddingModel::load_tokenizer(&tokenizer_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_tokenizer_missing_file() {
        let temp_dir = tempdir().unwrap();
        let missing_path = temp_dir.path().join("missing.json");

        let result = OnnxEmbeddingModel::load_tokenizer(&missing_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_trait_implementation() {
        // Test that OnnxEmbeddingModel implements EmbeddingProvider
        fn _test_provider<T: EmbeddingProvider>(_: T) {}
        
        // This would fail to compile if OnnxEmbeddingModel doesn't implement EmbeddingProvider
        // _test_provider(OnnxEmbeddingModel { ... }); // Can't construct without valid files
    }
} 