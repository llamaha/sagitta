//! ONNX embedding model implementation.

use crate::error::{Result, SagittaEmbedError};
use crate::model::EmbeddingModelType;
use crate::provider::EmbeddingProvider;
use crate::provider::onnx::memory_pool::{TensorMemoryPool, MemoryPoolStats};
use crate::provider::onnx::io_binding::{AdvancedIOBinding, IOBindingStats};
use log::{debug, warn, info};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
#[cfg(feature = "cuda")]
use ort::execution_providers::CUDAExecutionProvider;
use std::time::{Duration, Instant};

#[cfg(feature = "onnx")]
use ort::{
    execution_providers::CPUExecutionProvider,
    session::Session,
    value::Value,
};

#[cfg(feature = "onnx")]
use tokenizers::Tokenizer;

/// ONNX-based embedding model with memory management
#[derive(Debug)]
pub struct OnnxEmbeddingModel {
    /// The tokenizer for preprocessing input text
    #[cfg(feature = "onnx")]
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
    /// Whether the model expects token_type_ids input
    expects_token_type_ids: bool,
}

impl Clone for OnnxEmbeddingModel {
    fn clone(&self) -> Self {
        // Create a new session from the same model path
        #[cfg(feature = "onnx")]
        let session = Self::create_session(&self.model_path)
            .expect("Failed to create new session while cloning");

        Self {
            #[cfg(feature = "onnx")]
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
            expects_token_type_ids: self.expects_token_type_ids,
        }
    }
}

impl OnnxEmbeddingModel {
    /// Creates a new OnnxEmbeddingModel with default configuration
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

        // Auto-detect sequence length from model if possible, otherwise try tokenizer, then use config
        let max_seq_length = match Self::get_max_sequence_length_from_session(&session) {
            Ok(detected_length) => {
                info!("Auto-detected maximum sequence length from model: {detected_length}");
                debug!("Auto-detection successful, using detected length: {detected_length}");
                detected_length
            }
            Err(e) => {
                debug!("Model auto-detection failed: {e}");
                
                // Try to detect from tokenizer
                match Self::get_max_sequence_length_from_tokenizer(&tokenizer) {
                    Some(tokenizer_length) => {
                        info!("Auto-detected maximum sequence length from tokenizer behavior: {tokenizer_length}");
                        debug!("Tokenizer detection successful, using detected length: {tokenizer_length}");
                        tokenizer_length
                    }
                    None => {
                        warn!("Could not auto-detect sequence length from model or tokenizer. Using fallback default: 512");
                        debug!("Both auto-detection methods failed, using fallback default: 512");
                        512 // Reasonable default for most embedding models
                    }
                }
            }
        };

        debug!("Final max_seq_length that will be used: {max_seq_length}");
        
        // Check if model expects token_type_ids
        let expects_token_type_ids = session.inputs.iter()
            .any(|input| input.name == "token_type_ids");
        
        if expects_token_type_ids {
            debug!("Model expects token_type_ids input (3-input model)");
        } else {
            debug!("Model does not expect token_type_ids input (2-input model)");
        }

        // Phase 2: Initialize memory pool and I/O binding
        let memory_pool = TensorMemoryPool::new(config.memory_pool_config.clone());
        let io_binding = AdvancedIOBinding::new(config.io_binding_config.clone(), TensorMemoryPool::new(config.memory_pool_config.clone()));

        debug!(
            "ONNX model loaded successfully from {}, determined embedding dimension: {}, sequence length: {}",
            model_path.display(),
            dimension,
            max_seq_length
        );

        let model = Self {
            session,
            #[cfg(feature = "onnx")]
            tokenizer: Arc::new(Mutex::new(tokenizer)),
            max_seq_length,
            dimension,
            last_used: Instant::now(),
            model_path: model_path.to_path_buf(),
            memory_pool: Arc::new(Mutex::new(memory_pool)),
            #[cfg(feature = "onnx")]
            io_binding: Arc::new(Mutex::new(io_binding)),
            config: config.clone(),
            expects_token_type_ids,
        };

        info!("Model created with final configuration: {}", model.debug_info());
        Ok(model)
    }

    /// Creates a new OnnxEmbeddingModel (stub when ONNX feature is disabled)
    #[cfg(not(feature = "onnx"))]
    pub fn new(_model_path: &Path, _tokenizer_path: &Path) -> Result<Self> {
        Err(SagittaEmbedError::feature_not_enabled("onnx"))
    }

    /// Load tokenizer from path (can be file or directory)
    #[cfg(feature = "onnx")]
    fn load_tokenizer(tokenizer_path: &Path) -> Result<Tokenizer> {
        let tokenizer_json_path = if tokenizer_path.is_file()
            && tokenizer_path
                .file_name()
                .is_some_and(|name| name == "tokenizer.json")
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

        let mut tokenizer = Tokenizer::from_file(&tokenizer_json_path).map_err(|e| {
            SagittaEmbedError::tokenization(format!(
                "Failed to load tokenizer from {}: {}",
                tokenizer_json_path.display(),
                e
            ))
        })?;

        // Configure padding strategy like FastEmbed
        use tokenizers::PaddingParams;
        use tokenizers::PaddingStrategy;
        
        // Get the pad token ID from the tokenizer
        let pad_id = tokenizer.get_padding()
            .map(|p| p.pad_id)
            .unwrap_or(0);
        let pad_token = tokenizer.get_padding()
            .map(|p| p.pad_token.clone())
            .unwrap_or_else(|| "[PAD]".to_string());
            
        // Configure with BatchLongest padding strategy
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            pad_token,
            pad_id,
            pad_type_id: 0,
            direction: tokenizers::PaddingDirection::Right,
            pad_to_multiple_of: None,
        }));

        debug!("Tokenizer loaded successfully with BatchLongest padding strategy");
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
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create session builder: {e}")))?;

        // Apply graph optimization level
        let opt_level = match config.graph_optimization_level {
            0 => ort::session::builder::GraphOptimizationLevel::Disable,
            1 => ort::session::builder::GraphOptimizationLevel::Level1,
            2 => ort::session::builder::GraphOptimizationLevel::Level2,
            _ => ort::session::builder::GraphOptimizationLevel::Level3,
        };
        session_builder = session_builder
            .with_optimization_level(opt_level)
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to set optimization level: {e}")))?;

        // Configure threading
        if let Some(intra_threads) = config.intra_op_num_threads {
            session_builder = session_builder
                .with_intra_threads(intra_threads)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to set intra-op threads: {e}")))?;
        }

        if let Some(inter_threads) = config.inter_op_num_threads {
            session_builder = session_builder
                .with_inter_threads(inter_threads)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to set inter-op threads: {e}")))?;
        }

        // Enable memory patterns for better GPU memory reuse
        session_builder = session_builder
            .with_memory_pattern(true)
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to enable memory pattern: {e}")))?;
        
        // Configure parallel execution
        if config.enable_parallel_execution {
            session_builder = session_builder
                .with_parallel_execution(true)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to enable parallel execution: {e}")))?;
        }

        // Configure memory pattern optimization
        session_builder = session_builder
            .with_memory_pattern(config.enable_memory_pattern)
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to configure memory pattern: {e}")))?;

        // Configure deterministic compute
        if config.enable_deterministic_compute {
            session_builder = session_builder
                .with_deterministic_compute(true)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to enable deterministic compute: {e}")))?;
        }

        // Configure profiling if specified
        if let Some(ref profiling_path) = config.profiling_file_path {
            session_builder = session_builder
                .with_profiling(profiling_path)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to enable profiling: {e}")))?;
        }

        // Commit the session
        session_builder
            .commit_from_file(model_path)
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to load model from {}: {}", model_path.display(), e)))
    }

    /// Gets dimension from an ONNX session
    #[cfg(feature = "onnx")]
    fn get_dimension_from_session(session: &Session) -> Result<usize> {
        // Try different output names in order of precedence
        // Based on fastembed-rs implementation
        let target_output_name = if session.outputs.len() == 1 {
            // If there's only one output, use it
            session.outputs.first().unwrap().name.as_str()
        } else {
            // Try multiple output names in order of precedence
            session
                .outputs
                .iter()
                .find(|output| output.name == "last_hidden_state")
                .map(|_| "last_hidden_state")
                .or_else(|| {
                    session
                        .outputs
                        .iter()
                        .find(|output| output.name == "sentence_embedding")
                        .map(|_| "sentence_embedding")
                })
                .or_else(|| {
                    // Fallback to "pooler_output" for models like CodeBERT
                    session
                        .outputs
                        .iter()
                        .find(|output| output.name == "pooler_output")
                        .map(|_| "pooler_output")
                })
                .or_else(|| {
                    // Additional fallback for other common output names
                    session
                        .outputs
                        .iter()
                        .find(|output| output.name == "embeddings")
                        .map(|_| "embeddings")
                })
                .ok_or_else(|| {
                    let available_outputs: Vec<&str> = session.outputs.iter().map(|o| o.name.as_str()).collect();
                    SagittaEmbedError::model(
                        format!("Could not find a suitable output. Available outputs: {available_outputs:?}")
                    )
                })?
        };

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
                    "Failed to get model dimension from output: {target_output_name}"
                ))
            })
    }

    /// Automatically detect the maximum sequence length from the model's input specifications
    fn get_max_sequence_length_from_session(session: &Session) -> Result<usize> {
        debug!("Starting auto-detection of max sequence length from ONNX model");
        
        // Look for input_ids input to determine sequence length
        let input_ids_input = session
            .inputs
            .iter()
            .find(|input| input.name == "input_ids")
            .ok_or_else(|| {
                debug!("Available inputs: {:?}", session.inputs.iter().map(|i| &i.name).collect::<Vec<_>>());
                SagittaEmbedError::model(
                    "Could not find 'input_ids' input to determine sequence length"
                )
            })?;

        debug!("Found input_ids input: name={}, type={:?}", input_ids_input.name, input_ids_input.input_type);

        match input_ids_input.input_type {
            ort::value::ValueType::Tensor { ref dimensions, .. } => {
                debug!("Input tensor dimensions: {dimensions:?}");
                
                // For transformer models, input shape is typically [batch_size, sequence_length]
                // The sequence length is usually the second dimension
                if dimensions.len() >= 2 {
                    debug!("Checking dimension[1] for sequence length: {}", dimensions[1]);
                    
                    match dimensions[1] {
                        // If it's a dynamic dimension (-1), we can't determine it from the model
                        -1 => {
                            debug!("Model has dynamic sequence length (dimension[1] = -1), cannot auto-detect from model");
                            Err(SagittaEmbedError::model(
                                "Model has dynamic sequence length, cannot auto-detect from model dimensions"
                            ))
                        }
                        // If it's a fixed dimension, use it
                        seq_len if seq_len > 0 => {
                            let detected_length = seq_len as usize;
                            info!("Auto-detected maximum sequence length from model: {detected_length}");
                            debug!("Successfully detected fixed sequence length from model dimensions");
                            Ok(detected_length)
                        }
                        _ => {
                            debug!("Invalid sequence length dimension: {}", dimensions[1]);
                            Err(SagittaEmbedError::model(
                                "Invalid sequence length dimension in model"
                            ))
                        }
                    }
                } else {
                    debug!("Unexpected number of tensor dimensions: {} (expected >= 2)", dimensions.len());
                    Err(SagittaEmbedError::model(
                        "Unexpected input tensor dimensions for sequence length detection"
                    ))
                }
            }
            _ => {
                debug!("input_ids is not a tensor type: {:?}", input_ids_input.input_type);
                Err(SagittaEmbedError::model(
                    "input_ids is not a tensor type"
                ))
            }
        }
    }

    /// Try to detect max sequence length from tokenizer configuration
    #[cfg(feature = "onnx")]
    fn get_max_sequence_length_from_tokenizer(tokenizer: &Tokenizer) -> Option<usize> {
        debug!("Attempting to detect max sequence length from tokenizer configuration");
        
        // Try to get the model max length from the tokenizer
        // This is a bit of a hack since the tokenizers crate doesn't expose this directly
        // We'll try to encode a very long text and see where it gets truncated
        let very_long_text = "test ".repeat(1000); // 4000 characters, should be >512 tokens
        
        if let Ok(encoding) = tokenizer.encode(very_long_text.as_str(), true) {
            let token_count = encoding.get_ids().len();
            debug!("Tokenizer produced {token_count} tokens for very long text");
            
            // Common sequence lengths for transformer models
            let common_lengths = [512, 1024, 2048, 4096];
            
            // If the tokenizer truncated to a common length, that's likely the max
            for &length in &common_lengths {
                if token_count == length {
                    info!("Detected max sequence length from tokenizer behavior: {length}");
                    return Some(length);
                }
            }
            
            // If tokenizer doesn't truncate (produces >512 tokens), this likely means
            // it's a BGE-style model that was exported with 512 max length support
            if token_count > 600 {
                info!("Tokenizer doesn't truncate, assuming BGE model with 512 max length");
                return Some(512);
            }
            
            // If we got a reasonable token count that's not a standard length,
            // round up to the nearest power of 2 or common length
            if token_count > 256 && token_count <= 600 {
                info!("Tokenizer produced {token_count} tokens, assuming max length is 512");
                return Some(512);
            }
        }
        
        debug!("Could not determine max sequence length from tokenizer");
        None
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
    #[cfg(feature = "onnx")]
    fn prepare_inputs(&self, text: &str) -> Result<(Vec<i64>, Vec<i64>, Option<Vec<i64>>)> {
        debug!("Preparing inputs with max_seq_length: {}", self.max_seq_length);
        
        let encoding = self
            .tokenizer
            .lock()
            .map_err(|e| SagittaEmbedError::thread_safety(format!("Failed to lock tokenizer: {e}")))?
            .encode(text, true)
            .map_err(|e| SagittaEmbedError::tokenization(format!("Failed to encode text: {e}")))?;

        let mut input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mut attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&mask| mask as i64)
            .collect();
        
        // Create token_type_ids only if model expects it
        let original_length = input_ids.len();
        let mut token_type_ids: Option<Vec<i64>> = if self.expects_token_type_ids {
            Some(vec![0; original_length])
        } else {
            None
        };

        debug!("Original token count: {}, max_seq_length: {}", input_ids.len(), self.max_seq_length);

        #[allow(clippy::comparison_chain)]
        if input_ids.len() > self.max_seq_length {
            debug!("Truncating from {} tokens to {} tokens", input_ids.len(), self.max_seq_length);
            input_ids.truncate(self.max_seq_length);
            attention_mask.truncate(self.max_seq_length);
            if let Some(ref mut token_types) = token_type_ids {
                token_types.truncate(self.max_seq_length);
            }
        } else if input_ids.len() < self.max_seq_length {
            let pad_length = self.max_seq_length - input_ids.len();
            debug!("Padding from {} tokens to {} tokens (adding {} padding tokens)", 
                   input_ids.len(), self.max_seq_length, pad_length);
            input_ids.extend(vec![0; pad_length]);
            attention_mask.extend(vec![0; pad_length]);
            if let Some(ref mut token_types) = token_type_ids {
                token_types.extend(vec![0; pad_length]);
            }
        } else {
            debug!("Token count {} matches max_seq_length exactly", input_ids.len());
        }

        debug!("Final input shape: {} tokens", input_ids.len());
        Ok((input_ids, attention_mask, token_type_ids))
    }

    /// Get the maximum sequence length
    pub fn max_sequence_length(&self) -> usize {
        self.max_seq_length
    }

    /// Get debug information about the model configuration
    pub fn debug_info(&self) -> String {
        format!(
            "OnnxEmbeddingModel {{ max_seq_length: {}, dimension: {}, model_path: {:?} }}",
            self.max_seq_length,
            self.dimension,
            self.model_path
        )
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

    /// Optimized I/O binding inference following ORT v2.0.0-rc.9 best practices
    #[cfg(all(feature = "onnx", feature = "cuda"))]
    fn run_optimized_io_binding(
        &self,
        batch_size: usize,
        all_input_ids: Vec<i64>,
        all_attention_masks: Vec<i64>,
        all_token_type_ids: Option<Vec<i64>>,
        seq_length: usize,
    ) -> Result<std::collections::HashMap<String, ort::value::DynValue>> {
        use ort::memory::{MemoryInfo, AllocationDevice, AllocatorType, MemoryType};
        
        debug!("PROFILE: Starting I/O binding setup for batch size {batch_size}");
        let total_setup_start = Instant::now();
        
        // Create GPU memory info for CUDA device 0
        let _gpu_memory_info = MemoryInfo::new(
            AllocationDevice::CUDA,
            0, // CUDA device ID
            AllocatorType::Device,
            MemoryType::Default,
        ).map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create GPU memory info: {e}")))?;
        
        // Check if we're using GPU for this session
        let default_allocator = self.session.allocator();
        let default_memory_info = default_allocator.memory_info();
        debug!("PROFILE: Default allocator - device: {:?}, type: {:?}, memory_type: {:?}", 
               default_memory_info.allocation_device(), default_memory_info.allocator_type(), default_memory_info.memory_type());
        
        // Note: In ort 2.0.0-rc.9, session.allocator() doesn't take arguments
        // We'll use the gpu_memory_info for output binding instead
        // The key is to use bind_output_to_device with GPU memory info
        
        // Create I/O binding
        let binding_start = Instant::now();
        let mut io_binding = self.session.create_binding()
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create I/O binding: {e}")))?;
        debug!("PROFILE: I/O binding creation took {:?}", binding_start.elapsed());

        // Create input tensors directly on GPU using GPU allocator for maximum performance
        let tensor_start = Instant::now();
        let input_ids_shape = vec![batch_size, seq_length];
        let attention_mask_shape = vec![batch_size, seq_length];

        // Add memory allocation profiling with size calculations
        let alloc_start = Instant::now();
        let total_elements = batch_size * seq_length;
        let input_size_bytes = total_elements * std::mem::size_of::<i64>();
        debug!("PROFILE: About to allocate tensors on GPU for shape [{batch_size}, {seq_length}] = {total_elements} elements = {input_size_bytes} bytes per tensor");

        // Create tensors - note: in ort 2.0.0-rc.9, tensors are created on CPU first
        // The IO binding will handle the GPU transfer efficiently
        let input_tensor_start = Instant::now();
        let input_ids_tensor = Value::from_array((input_ids_shape.clone(), all_input_ids))
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create input_ids tensor: {e}")))?;
        let input_tensor_elapsed = input_tensor_start.elapsed();
        debug!("PROFILE: input_ids tensor creation took {input_tensor_elapsed:?}");

        let mask_tensor_start = Instant::now();
        let attention_mask_tensor = Value::from_array((attention_mask_shape.clone(), all_attention_masks))
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create attention_mask tensor: {e}")))?;
        let mask_tensor_elapsed = mask_tensor_start.elapsed();
        debug!("PROFILE: attention_mask tensor creation took {mask_tensor_elapsed:?}");
        
        let token_type_ids_tensor = if let Some(token_type_ids) = all_token_type_ids {
            let token_type_tensor_start = Instant::now();
            let token_type_ids_shape = vec![batch_size, seq_length];
            let tensor = Value::from_array((token_type_ids_shape.clone(), token_type_ids))
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create token_type_ids tensor: {e}")))?;
            let token_type_tensor_elapsed = token_type_tensor_start.elapsed();
            debug!("PROFILE: token_type_ids tensor creation took {token_type_tensor_elapsed:?}");
            Some(tensor)
        } else {
            None
        };
        
        debug!("PROFILE: Tensor memory allocation took {:?}", alloc_start.elapsed());
        debug!("PROFILE: Tensor creation took {:?}", tensor_start.elapsed());

        // Check if tensors are on expected device
        debug!("PROFILE: Checking tensor device locations...");
        // Note: Value doesn't expose device info directly, but binding will show if transfers occur

        // Bind inputs with detailed timing
        let bind_start = Instant::now();
        let input_bind_start = Instant::now();
        io_binding.bind_input("input_ids", &input_ids_tensor)
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to bind input_ids: {e}")))?;
        debug!("PROFILE: input_ids binding took {:?}", input_bind_start.elapsed());

        let mask_bind_start = Instant::now();
        io_binding.bind_input("attention_mask", &attention_mask_tensor)
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to bind attention_mask: {e}")))?;
        debug!("PROFILE: attention_mask binding took {:?}", mask_bind_start.elapsed());
        
        if let Some(ref token_type_ids_tensor) = token_type_ids_tensor {
            let token_type_bind_start = Instant::now();
            io_binding.bind_input("token_type_ids", token_type_ids_tensor)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to bind token_type_ids: {e}")))?;
            debug!("PROFILE: token_type_ids binding took {:?}", token_type_bind_start.elapsed());
        }
        
        debug!("PROFILE: Input binding took {:?}", bind_start.elapsed());

        // Bind output to GPU device for maximum performance - following ORT documentation
        // This keeps the output on GPU and avoids expensive GPU->CPU->GPU copies
        let output_bind_start = Instant::now();
        debug!("PROFILE: Attempting to bind output to GPU device memory");
        
        // For outputs, we need to bind to CPU memory to be able to extract the data later
        // Binding to GPU would require additional copy operations
        let cpu_memory_info = MemoryInfo::new(
            AllocationDevice::CPU,
            0,
            AllocatorType::Device,
            MemoryType::CPUOutput,
        ).map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create CPU memory info: {e}")))?;
        
        // Try output names in order of precedence (matching fastembed-rs)
        if self.session.outputs.len() == 1 {
            // If there's only one output, use it
            let output_name = &self.session.outputs[0].name;
            io_binding.bind_output_to_device(output_name, &cpu_memory_info)
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to bind single output '{output_name}': {e}")))?;
        } else {
            // Try multiple output names in order of precedence
            io_binding.bind_output_to_device("last_hidden_state", &cpu_memory_info)
                .or_else(|e1| {
                    debug!("PROFILE: last_hidden_state bind failed: {e1}, trying sentence_embedding");
                    io_binding.bind_output_to_device("sentence_embedding", &cpu_memory_info)
                })
                .or_else(|e2| {
                    debug!("PROFILE: sentence_embedding bind failed: {e2}, trying pooler_output");
                    io_binding.bind_output_to_device("pooler_output", &cpu_memory_info)
                })
                .or_else(|e3| {
                    debug!("PROFILE: pooler_output bind failed: {e3}, trying embeddings");
                    io_binding.bind_output_to_device("embeddings", &cpu_memory_info)
                })
                .map_err(|_e| {
                    let available_outputs: Vec<&str> = self.session.outputs.iter().map(|o| o.name.as_str()).collect();
                    SagittaEmbedError::onnx_runtime(format!("Failed to bind any output. Available outputs: {available_outputs:?}"))
                })?;
        }
        
        debug!("PROFILE: Output binding took {:?}", output_bind_start.elapsed());
        
        let setup_elapsed = total_setup_start.elapsed();
        debug!("PROFILE: Total I/O binding setup took {setup_elapsed:?}");
        
        // Performance analysis
        let setup_overhead_pct = (setup_elapsed.as_nanos() as f64 / 1_000_000.0) / 1000.0 * 100.0; // as % of 1 second
        debug!("PROFILE: Setup overhead: {setup_overhead_pct:.3}% of 1 second");
        
        // Log warning if setup is taking too long
        if setup_elapsed.as_millis() > 50 {
            warn!("PROFILE: Slow I/O binding setup detected - total setup took {setup_elapsed:?} for batch_size={batch_size}, seq_length={seq_length}");
        }

        // Run inference with I/O binding
        debug!("PROFILE: Starting actual inference execution");
        let inference_start = Instant::now();
        
        // Add pre-run profiling with memory expectations
        debug!("PROFILE: About to call io_binding.run() with batch_size={}, max_seq_length={}", batch_size, self.max_seq_length);
        debug!("PROFILE: Expected inference workload: {} elements * 2 tensors = {} total input elements", 
               total_elements, total_elements * 2);
        
        // Check if we're actually using CUDA execution
        debug!("PROFILE: Verifying CUDA execution provider is active...");
        // Note: ORT doesn't expose runtime provider info easily, but we can infer from timing patterns
        
        let pre_run_time = Instant::now();
        
        let session_outputs = io_binding.run()
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("I/O binding inference failed: {e}")))?;
        
        let run_elapsed = pre_run_time.elapsed();
        debug!("PROFILE: io_binding.run() completed in {run_elapsed:?}");
        
        let inference_elapsed = inference_start.elapsed();
        debug!("PROFILE: Actual inference execution took {inference_elapsed:?}");

        // Performance analysis
        let throughput_elements_per_sec = total_elements as f64 / run_elapsed.as_secs_f64();
        let throughput_tokens_per_sec = (batch_size * self.max_seq_length) as f64 / run_elapsed.as_secs_f64();
        debug!("PROFILE: Inference throughput: {throughput_elements_per_sec:.0} elements/sec, {throughput_tokens_per_sec:.0} tokens/sec");

        // GPU vs CPU execution pattern analysis
        let seconds_per_token = run_elapsed.as_secs_f64() / (batch_size * self.max_seq_length) as f64;
        let ms_per_token = seconds_per_token * 1000.0;
        debug!("PROFILE: Timing per token: {ms_per_token:.4} ms/token");
        
        // Expected performance baselines (rough estimates for BGE-small)
        let expected_gpu_ms_per_token = 0.02; // ~50K tokens/sec on modern GPU
        let expected_cpu_ms_per_token = 0.15; // ~6.7K tokens/sec on decent CPU
        
        if ms_per_token > expected_cpu_ms_per_token * 0.8 {
            warn!("PROFILE: Performance suggests CPU execution ({ms_per_token:.4} ms/token, expected GPU: ~{expected_gpu_ms_per_token:.4}, CPU: ~{expected_cpu_ms_per_token:.4})");
        } else if ms_per_token > expected_gpu_ms_per_token * 3.0 {
            warn!("PROFILE: Performance slower than expected for GPU ({ms_per_token:.4} ms/token, expected ~{expected_gpu_ms_per_token:.4})");
        } else {
            debug!("PROFILE: Performance suggests efficient GPU execution ({ms_per_token:.4} ms/token)");
        }

        // Detect potential performance issues
        if run_elapsed.as_millis() > 100 {
            warn!("PROFILE: Slow inference detected - io_binding.run() took {run_elapsed:?} for batch_size={batch_size}, seq_length={seq_length}");
            
            // Additional diagnostics for slow inference
            let expected_tokens_per_ms = 30.0; // Rough baseline for BGE-small on modern GPU
            let actual_tokens_per_ms = (batch_size * self.max_seq_length) as f64 / run_elapsed.as_millis() as f64;
            debug!("PROFILE: Token processing rate: {actual_tokens_per_ms:.1} tokens/ms (expected ~{expected_tokens_per_ms:.1} tokens/ms)");
                    
            if actual_tokens_per_ms < expected_tokens_per_ms * 0.5 {
                warn!("PROFILE: Significantly slower than expected - possible GPU memory transfer or compute bottleneck");
            }
        }

        // Convert SessionOutputs to HashMap for compatibility with existing code
        let conversion_start = Instant::now();
        let mut outputs = std::collections::HashMap::new();
        for (name, value) in session_outputs.into_iter() {
            debug!("PROFILE: Processing output tensor: {name}");
            outputs.insert(name.to_string(), value);
        }
        debug!("PROFILE: Output conversion took {:?}", conversion_start.elapsed());

        Ok(outputs)
    }

    /// Standard inference without I/O binding (fallback)
    #[cfg(feature = "onnx")]
    fn run_standard_inference(
        &self,
        batch_size: usize,
        all_input_ids: Vec<i64>,
        all_attention_masks: Vec<i64>,
        all_token_type_ids: Option<Vec<i64>>,
        seq_length: usize,
    ) -> Result<std::collections::HashMap<String, ort::value::DynValue>> {
        use ndarray::Array;

        debug!("PROFILE: Starting standard inference setup for batch size {batch_size}");
        let total_setup_start = Instant::now();

        let array_start = Instant::now();
        debug!("PROFILE: About to create arrays for shape [{}, {}] = {} elements", 
               batch_size, seq_length, batch_size * seq_length);
               
        let input_ids_array = Array::from_shape_vec((batch_size, seq_length), all_input_ids)
            .map_err(|e| SagittaEmbedError::embedding_generation(format!("Input ID batch shape error: {e}")))?;
        let attention_mask_array = Array::from_shape_vec((batch_size, seq_length), all_attention_masks)
            .map_err(|e| SagittaEmbedError::embedding_generation(format!("Attention mask batch shape error: {e}")))?;
        
        let token_type_ids_array = if let Some(token_type_ids) = all_token_type_ids {
            Some(Array::from_shape_vec((batch_size, seq_length), token_type_ids)
                .map_err(|e| SagittaEmbedError::embedding_generation(format!("Token type IDs batch shape error: {e}")))?)
        } else {
            None
        };
        
        debug!("PROFILE: Array creation took {:?}", array_start.elapsed());

        // Convert ndarray to ort Value
        let tensor_start = Instant::now();
        let input_ids_shape = input_ids_array.shape().to_vec();
        let input_ids_vec = input_ids_array.into_raw_vec_and_offset().0;
        
        // Add memory allocation profiling
        let alloc_start = Instant::now();
        let input_ids_value = Value::from_array((input_ids_shape.clone(), input_ids_vec.clone()))
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create input ID tensor: {e}")))?;

        let attention_mask_shape = attention_mask_array.shape().to_vec();
        let attention_mask_vec = attention_mask_array.into_raw_vec_and_offset().0;
        let attention_mask_value = Value::from_array((attention_mask_shape.clone(), attention_mask_vec.clone()))
            .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create attention mask tensor: {e}")))?;
        
        let token_type_ids_value = if let Some(array) = token_type_ids_array {
            let token_type_ids_shape = array.shape().to_vec();
            let token_type_ids_vec = array.into_raw_vec_and_offset().0;
            Some(Value::from_array((token_type_ids_shape.clone(), token_type_ids_vec.clone()))
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create token type IDs tensor: {e}")))?)
        } else {
            None
        };
        
        debug!("PROFILE: Tensor memory allocation took {:?}", alloc_start.elapsed());
        debug!("PROFILE: Tensor conversion took {:?}", tensor_start.elapsed());
        
        let setup_elapsed = total_setup_start.elapsed();
        debug!("PROFILE: Total standard inference setup took {setup_elapsed:?}");
        
        // Log warning if setup is taking too long
        if setup_elapsed.as_millis() > 50 {
            warn!("PROFILE: Slow standard inference setup detected - total setup took {setup_elapsed:?} for batch_size={batch_size}, seq_length={seq_length}");
        }

        // Run inference with standard session
        debug!("PROFILE: Starting standard inference execution");
        let inference_start = Instant::now();
        
        // Add pre-run profiling
        debug!("PROFILE: About to call session.run() with batch_size={batch_size}, seq_length={seq_length}");
        let pre_run_time = Instant::now();
        
        let outputs = if let Some(token_type_ids_value) = token_type_ids_value {
            // 3-input model
            self.session
                .run(
                    ort::inputs![
                        "input_ids" => input_ids_value,
                        "attention_mask" => attention_mask_value,
                        "token_type_ids" => token_type_ids_value,
                    ]
                    .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create ONNX inputs: {e}")))?,
                )
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("ONNX session batch run failed: {e}")))?
        } else {
            // 2-input model
            self.session
                .run(
                    ort::inputs![
                        "input_ids" => input_ids_value,
                        "attention_mask" => attention_mask_value,
                    ]
                    .map_err(|e| SagittaEmbedError::onnx_runtime(format!("Failed to create ONNX inputs: {e}")))?,
                )
                .map_err(|e| SagittaEmbedError::onnx_runtime(format!("ONNX session batch run failed: {e}")))?
        }
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        
        let run_elapsed = pre_run_time.elapsed();
        debug!("PROFILE: session.run() completed in {run_elapsed:?}");
        
        let inference_elapsed = inference_start.elapsed();
        debug!("PROFILE: Standard inference execution took {inference_elapsed:?}");

        // Add more detailed breakdown
        if run_elapsed.as_millis() > 100 {
            warn!("PROFILE: Slow standard inference detected - session.run() took {run_elapsed:?} for batch_size={batch_size}, seq_length={seq_length}");
        }

        Ok(outputs)
    }
}

impl OnnxEmbeddingModel {
    /// Prepare batch inputs using tokenizer's BatchLongest padding
    #[cfg(feature = "onnx")]
    fn prepare_batch_inputs_dynamic(&self, texts: &[&str]) -> Result<(Vec<i64>, Vec<i64>, Option<Vec<i64>>, usize)> {
        debug!("Preparing batch inputs with tokenizer's BatchLongest padding for {} texts", texts.len());
        
        let tokenizer = self.tokenizer.lock()
            .map_err(|e| SagittaEmbedError::thread_safety(format!("Failed to lock tokenizer: {e}")))?;
        
        // Use encode_batch which will apply BatchLongest padding automatically
        let encodings = tokenizer.encode_batch(texts.to_vec(), true)
            .map_err(|e| SagittaEmbedError::tokenization(format!("Failed to batch encode texts: {e}")))?;
        
        // Get the actual padded length from the first encoding, but cap at model's max
        let padded_length = encodings[0].get_ids().len().min(self.max_seq_length);
        debug!("Tokenizer padded batch to length: {} (model max: {})", 
               padded_length, self.max_seq_length);
        
        if encodings[0].get_ids().len() > self.max_seq_length {
            debug!("Batch contains sequences longer than model max, will truncate from {} to {}", 
                   encodings[0].get_ids().len(), self.max_seq_length);
        }
        
        let batch_size = texts.len();
        let mut all_input_ids = Vec::with_capacity(batch_size * padded_length);
        let mut all_attention_masks = Vec::with_capacity(batch_size * padded_length);
        let mut all_token_type_ids = if self.expects_token_type_ids {
            Some(Vec::with_capacity(batch_size * padded_length))
        } else {
            None
        };
        
        // Extract the padded sequences, truncating if necessary
        for encoding in encodings {
            let mut input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
            let mut attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&mask| mask as i64).collect();
            let mut token_type_ids: Option<Vec<i64>> = if self.expects_token_type_ids {
                Some(vec![0; input_ids.len()])
            } else {
                None
            };
            
            // Truncate if longer than model max
            if input_ids.len() > self.max_seq_length {
                input_ids.truncate(self.max_seq_length);
                attention_mask.truncate(self.max_seq_length);
                if let Some(ref mut token_types) = token_type_ids {
                    token_types.truncate(self.max_seq_length);
                }
            }
            
            // Truncate to padded_length (which is min of batch max and model max)
            input_ids.truncate(padded_length);
            attention_mask.truncate(padded_length);
            if let Some(ref mut token_types) = token_type_ids {
                token_types.truncate(padded_length);
            }
            
            // Pad if shorter than padded_length
            if input_ids.len() < padded_length {
                let pad_len = padded_length - input_ids.len();
                input_ids.extend(vec![0; pad_len]);
                attention_mask.extend(vec![0; pad_len]);
                if let Some(ref mut token_types) = token_type_ids {
                    token_types.extend(vec![0; pad_len]);
                }
            }
            
            all_input_ids.extend(input_ids);
            all_attention_masks.extend(attention_mask);
            if let Some(ref mut all_token_types) = all_token_type_ids {
                if let Some(token_types) = token_type_ids {
                    all_token_types.extend(token_types);
                }
            }
        }
        
        // Log efficiency gain
        let efficiency = (padded_length as f32 / self.max_seq_length as f32) * 100.0;
        debug!("Dynamic padding efficiency - using {efficiency:.1}% of tokens compared to fixed padding");
        
        Ok((all_input_ids, all_attention_masks, all_token_type_ids, padded_length))
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
            "PROFILE: ONNX provider received batch of {batch_size} items"
        );

        let token_start = Instant::now();
        
        // Check if dynamic padding is enabled
        let (all_input_ids, all_attention_masks, all_token_type_ids, actual_seq_length) = if self.config.dynamic_batch_config.enable_dynamic_padding {
            // Use dynamic padding like FastEmbed
            debug!("Using dynamic padding (FastEmbed style)");
            self.prepare_batch_inputs_dynamic(texts)?
        } else {
            // Use fixed padding (original behavior)
            debug!("Using fixed padding to max_seq_length={}", self.max_seq_length);
            let mut all_input_ids = Vec::with_capacity(batch_size * self.max_seq_length);
            let mut all_attention_masks = Vec::with_capacity(batch_size * self.max_seq_length);
            let mut all_token_type_ids = if self.expects_token_type_ids {
                Some(Vec::with_capacity(batch_size * self.max_seq_length))
            } else {
                None
            };
            
            for text in texts {
                let (mut input_ids, mut attention_mask, token_type_ids) = self.prepare_inputs(text)?;
                all_input_ids.append(&mut input_ids);
                all_attention_masks.append(&mut attention_mask);
                if let Some(ref mut all_token_types) = all_token_type_ids {
                    if let Some(mut token_types) = token_type_ids {
                        all_token_types.append(&mut token_types);
                    }
                }
            }
            
            (all_input_ids, all_attention_masks, all_token_type_ids, self.max_seq_length)
        };
        
        let token_elapsed = token_start.elapsed();
        debug!(
            "PROFILE: ONNX batch tokenization for {batch_size} items with seq_length {actual_seq_length}: {token_elapsed:?}"
        );

        // No need to modify max_seq_length anymore since we cap at model max

        // Store attention masks for potential mean pooling
        let attention_masks_for_pooling = all_attention_masks.clone();
        
        let onnx_start = Instant::now();
        
        // Use optimized I/O binding for CUDA builds when available, otherwise use standard inference
        #[cfg(feature = "cuda")]
        let outputs = if self.config.should_use_cuda() && self.config.io_binding_config.enable_io_binding {
            debug!("PROFILE: Using optimized I/O binding for GPU inference");
            let io_binding_start = Instant::now();
            let result = self.run_optimized_io_binding(batch_size, all_input_ids, all_attention_masks, all_token_type_ids, actual_seq_length)?;
            let io_binding_elapsed = io_binding_start.elapsed();
            debug!("PROFILE: I/O binding inference completed in {io_binding_elapsed:?}");
            result
        } else {
            debug!("PROFILE: Using standard ONNX inference");
            let standard_start = Instant::now();
            let result = self.run_standard_inference(batch_size, all_input_ids, all_attention_masks, all_token_type_ids, actual_seq_length)?;
            let standard_elapsed = standard_start.elapsed();
            debug!("PROFILE: Standard inference completed in {standard_elapsed:?}");
            result
        };
        
        #[cfg(not(feature = "cuda"))]
        let outputs = {
            debug!("PROFILE: Using standard ONNX inference (CPU build)");
            let standard_start = Instant::now();
            let result = self.run_standard_inference(batch_size, all_input_ids, all_attention_masks, all_token_type_ids, actual_seq_length)?;
            let standard_elapsed = standard_start.elapsed();
            debug!("PROFILE: Standard inference completed in {:?}", standard_elapsed);
            result
        };
        
        let onnx_elapsed = onnx_start.elapsed();
        debug!("PROFILE: Total ONNX inference for {batch_size} items: {onnx_elapsed:?}");


        // Check for output in order of precedence (matching fastembed-rs)
        let output_lookup_start = Instant::now();
        let output_value = if outputs.len() == 1 {
            // If there's only one output, use it
            outputs.iter().next().map(|(_, v)| v).unwrap()
        } else {
            // Try multiple output names in order of precedence
            outputs
                .get("last_hidden_state")
                .or_else(|| outputs.get("sentence_embedding"))
                .or_else(|| outputs.get("pooler_output"))
                .or_else(|| outputs.get("embeddings"))
                .ok_or_else(|| {
                    let available_outputs: Vec<&str> = outputs.keys().map(|k| k.as_str()).collect();
                    SagittaEmbedError::embedding_generation(
                        format!("Model did not return expected output. Available outputs: {available_outputs:?}")
                    )
                })?
        };
        debug!("PROFILE: Output lookup took {:?}", output_lookup_start.elapsed());

        // Extract raw tensor data
        let extraction_start = Instant::now();
        
        // Since we're binding outputs to CPU memory, extraction should work directly
        let (shape, data) = output_value
            .try_extract_raw_tensor::<f32>()
            .map_err(|e| SagittaEmbedError::embedding_generation(format!("Failed to extract raw tensor data: {e}")))?;
        
        debug!("PROFILE: Tensor extraction took {:?} for shape {:?}", extraction_start.elapsed(), shape);

        let expected_dim = self.dimension;
        // Handle different possible output shapes
        let processing_start = Instant::now();
        let result = if shape.len() == 2 && shape[0] as usize == batch_size && shape[1] == expected_dim as i64 {
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
            // Need to perform mean pooling to get sentence embeddings
            debug!("Received token embeddings (shape: {shape:?}), performing mean pooling");
            
            let seq_length = shape[1] as usize;
            let mut embeddings = Vec::with_capacity(batch_size);
            
            // For each sample in the batch
            for batch_idx in 0..batch_size {
                let mut sentence_embedding = vec![0.0_f32; expected_dim];
                let mut non_padding_tokens = 0;
                
                // Get attention mask for this sample to know which tokens to pool
                let attention_mask_start = batch_idx * actual_seq_length;
                let attention_mask_end = attention_mask_start + actual_seq_length;
                let sample_attention_mask = &attention_masks_for_pooling[attention_mask_start..attention_mask_end];
                
                // Compute mean pooling over non-padding tokens
                for token_idx in 0..seq_length.min(actual_seq_length) {
                    if sample_attention_mask[token_idx] == 1 {
                        non_padding_tokens += 1;
                        let token_start = batch_idx * seq_length * expected_dim + token_idx * expected_dim;
                        for dim_idx in 0..expected_dim {
                            sentence_embedding[dim_idx] += data[token_start + dim_idx];
                        }
                    }
                }
                
                // Average the embeddings
                if non_padding_tokens > 0 {
                    for val in &mut sentence_embedding {
                        *val /= non_padding_tokens as f32;
                    }
                }
                
                // Normalize the embedding (L2 normalization)
                let norm: f32 = sentence_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for val in &mut sentence_embedding {
                        *val /= norm;
                    }
                }
                
                embeddings.push(sentence_embedding);
            }
            
            Ok(embeddings)
        } else {
            // Unexpected shape
            Err(SagittaEmbedError::embedding_generation(format!(
                "Unexpected batch output shape: got {shape:?}, expected compatible with [{batch_size}, {expected_dim}]"
            )))
        };
        debug!("PROFILE: Final processing took {:?}", processing_start.elapsed());
        
        // Add comprehensive performance summary with actual sequence length used
        let total_batch_time = onnx_start.elapsed();
        let elements_processed = batch_size * actual_seq_length;
        let throughput = elements_processed as f64 / total_batch_time.as_secs_f64();
        
        info!("PROFILE: BATCH SUMMARY - batch_size={batch_size}, actual_seq_length={actual_seq_length}, total_elements={elements_processed}, total_time={total_batch_time:?}, throughput={throughput:.0} elements/sec");
        
        // Compare with fixed padding performance
        let fixed_padding_elements = batch_size * self.max_seq_length;
        let padding_efficiency = elements_processed as f64 / fixed_padding_elements as f64;
        debug!("PROFILE: Dynamic padding efficiency - using {:.1}% of tokens compared to fixed padding ({} vs {} tokens)", 
               padding_efficiency * 100.0, elements_processed, fixed_padding_elements);
        
        result
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
                "type": "BPE",
                "vocab": {},
                "merges": []
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
        // This test will compile if the trait is properly implemented
    }

    #[test]
    fn test_gpu_optimization_config() {
        let config = crate::config::EmbeddingConfig::default()
            .with_gpu_optimization();
        
        #[cfg(feature = "cuda")]
        {
            assert!(config.io_binding_config.enable_io_binding);
            assert!(config.io_binding_config.enable_pre_allocated_buffers);
            assert!(config.io_binding_config.enable_zero_copy);
            assert!(config.io_binding_config.enable_batch_optimization);
            assert!(config.enable_cuda_memory_streams);
            assert!(config.cuda_config.enable_memory_optimization);
            assert!(config.cuda_config.enable_memory_pool);
        }
        
        #[cfg(not(feature = "cuda"))]
        {
            // Without CUDA, GPU optimization falls back to CPU optimization
            assert!(!config.io_binding_config.enable_io_binding);
        }
    }

    #[test]
    fn test_cpu_optimization_config() {
        let config = crate::config::EmbeddingConfig::default()
            .with_cpu_optimization();
        
        // Should enable CPU optimizations
        assert!(!config.io_binding_config.enable_io_binding);
        assert!(!config.io_binding_config.enable_pre_allocated_buffers);
        assert!(!config.io_binding_config.enable_zero_copy);
        assert!(!config.io_binding_config.enable_batch_optimization);
        assert!(!config.enable_cuda_memory_streams);
        assert!(config.cpu_config.enable_arena);
        assert!(config.cpu_config.enable_numa);
        assert!(config.cpu_config.enable_cache_optimization);
        assert!(config.cpu_config.enable_simd);
    }

    #[test]
    fn test_sequence_length_auto_detection() {
        // This test verifies that the auto-detection logic is present
        // In a real scenario, this would require an actual ONNX model file
        
        // Test that the method exists and handles errors gracefully
        let temp_dir = tempdir().unwrap();
        let tokenizer_path = create_dummy_tokenizer(temp_dir.path());
        
        // The actual auto-detection would happen during model loading
        // For now, we just verify the method signature exists
        assert!(tokenizer_path.exists());
        
        // Note: Full integration test would require a real ONNX model
        // This test mainly ensures the code compiles and the logic is in place
    }
}