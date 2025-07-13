//! Configuration types for the Sagitta embedding engine.

use crate::error::{Result, SagittaEmbedError};
use crate::model::EmbeddingModelType;
use crate::{DEFAULT_EMBEDDING_DIMENSION, DEFAULT_SESSION_TIMEOUT_SECONDS, DEFAULT_ENABLE_SESSION_CLEANUP, DEFAULT_EMBEDDING_BATCH_SIZE};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Execution provider preference order for auto-selection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum ExecutionProvider {
    /// CUDA GPU execution provider
    Cuda,
    /// CPU execution provider
    Cpu,
    /// DirectML execution provider (Windows)
    DirectML,
    /// CoreML execution provider (macOS)
    CoreML,
    /// ROCm execution provider (AMD GPU)
    ROCm,
    /// Auto-detect optimal provider
    #[default]
    Auto,
}


/// Memory pool configuration for tensor reuse
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryPoolConfig {
    /// Enable memory pool for tensor reuse
    pub enable_pool: bool,
    /// Maximum number of tensors to keep in pool
    pub max_pool_size: usize,
    /// Maximum memory usage for pool in bytes (0 = unlimited)
    pub max_pool_memory_bytes: usize,
    /// Enable memory pressure detection and adaptation
    pub enable_memory_pressure_detection: bool,
    /// Memory pressure threshold (0.0-1.0, where 1.0 = 100% memory usage)
    pub memory_pressure_threshold: f32,
}

impl Default for MemoryPoolConfig {
    fn default() -> Self {
        Self {
            enable_pool: true,
            max_pool_size: 32,
            max_pool_memory_bytes: 512 * 1024 * 1024, // 512MB default
            enable_memory_pressure_detection: true,
            memory_pressure_threshold: 0.8, // 80% memory usage threshold
        }
    }
}

/// Advanced I/O binding configuration
#[derive(Debug, Clone, PartialEq)]
pub struct IOBindingConfig {
    /// Enable I/O binding for repeated inference (reduces memory copies)
    pub enable_io_binding: bool,
    /// Pre-allocate tensor buffers for repeated inference
    pub enable_pre_allocated_buffers: bool,
    /// Enable zero-copy memory operations between host/device
    pub enable_zero_copy: bool,
    /// Enable batch-optimized I/O binding for multiple texts
    pub enable_batch_optimization: bool,
    /// Number of pre-allocated input buffers
    pub pre_allocated_input_buffers: usize,
    /// Number of pre-allocated output buffers
    pub pre_allocated_output_buffers: usize,
}

impl Default for IOBindingConfig {
    fn default() -> Self {
        // Enable I/O binding by default only for CUDA builds
        #[cfg(feature = "cuda")]
        let enable_io_binding = true;
        #[cfg(not(feature = "cuda"))]
        let enable_io_binding = false;

        Self {
            enable_io_binding,
            enable_pre_allocated_buffers: enable_io_binding, // Only useful with I/O binding
            enable_zero_copy: enable_io_binding,             // Only useful with I/O binding
            enable_batch_optimization: enable_io_binding,    // Only useful with I/O binding
            pre_allocated_input_buffers: 4,
            pre_allocated_output_buffers: 4,
        }
    }
}

/// Dynamic batch size optimization configuration
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicBatchConfig {
    /// Enable dynamic batch size optimization
    pub enable_dynamic_batching: bool,
    /// Enable dynamic padding (pad to longest sequence in batch, not max)
    pub enable_dynamic_padding: bool,
    /// Minimum batch size
    pub min_batch_size: usize,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Target latency in milliseconds for batch size optimization
    pub target_latency_ms: u64,
    /// Memory usage prediction and adjustment
    pub enable_memory_prediction: bool,
    /// Throughput vs latency optimization mode (true = throughput, false = latency)
    pub optimize_for_throughput: bool,
}

impl Default for DynamicBatchConfig {
    fn default() -> Self {
        Self {
            enable_dynamic_batching: true,
            enable_dynamic_padding: false,  // Disabled - ONNX models require fixed sequence length
            min_batch_size: 1,
            max_batch_size: 32,
            target_latency_ms: 100, // 100ms target latency
            enable_memory_prediction: true,
            optimize_for_throughput: false, // Default to latency optimization
        }
    }
}

/// Configuration for embedding models and processing.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingConfig {
    /// Type of embedding model to use
    pub model_type: EmbeddingModelType,
    /// Path to ONNX model file (required for ONNX models)
    pub onnx_model_path: Option<PathBuf>,
    /// Path to tokenizer file or directory (required for ONNX models)
    pub onnx_tokenizer_path: Option<PathBuf>,
    /// Expected embedding dimension (for validation)
    pub expected_dimension: Option<usize>,
    /// Session timeout in seconds (0 = no timeout)
    pub session_timeout_seconds: u64,
    /// Enable session cleanup on idle
    pub enable_session_cleanup: bool,
    /// Batch size for embedding operations (number of texts processed together)
    pub embedding_batch_size: Option<usize>,
    
    // Phase 1: Performance optimization settings
    /// Number of threads for intra-op parallelism (within operators)
    pub intra_op_num_threads: Option<usize>,
    /// Number of threads for inter-op parallelism (between operators)
    pub inter_op_num_threads: Option<usize>,
    /// Enable parallel execution mode for models with branches
    pub enable_parallel_execution: bool,
    /// Graph optimization level (0=disable, 1=basic, 2=extended, 3=all)
    pub graph_optimization_level: u8,
    /// Enable memory pattern optimization (disable for dynamic batch sizes)
    pub enable_memory_pattern: bool,
    /// Enable deterministic compute (may reduce performance but ensures reproducibility)
    pub enable_deterministic_compute: bool,
    /// Enable profiling and specify output file path
    pub profiling_file_path: Option<PathBuf>,
    /// Memory limit for CUDA provider (in bytes, 0 = unlimited)
    pub cuda_memory_limit: Option<usize>,
    /// Enable CPU arena allocator
    pub enable_cpu_arena: bool,
    
    // Phase 2: Advanced optimization settings
    /// Advanced I/O binding configuration
    pub io_binding_config: IOBindingConfig,
    /// Memory pool configuration for tensor reuse
    pub memory_pool_config: MemoryPoolConfig,
    /// Execution provider preference order for auto-selection
    pub execution_providers: Vec<ExecutionProvider>,
    /// Enable execution provider auto-selection with fallback chains
    pub enable_provider_auto_selection: bool,
    /// Dynamic batch size optimization configuration
    pub dynamic_batch_config: DynamicBatchConfig,
    /// Enable CUDA memory stream optimization
    pub enable_cuda_memory_streams: bool,
    /// Hardware capability detection and optimization
    pub enable_hardware_detection: bool,
    
    // Phase 3: Execution Provider Optimization (Platform Specific)
    /// Advanced CUDA execution provider configuration
    pub cuda_config: CudaExecutionProviderConfig,
    /// Advanced CPU execution provider configuration
    pub cpu_config: CpuExecutionProviderConfig,
    
    // Phase 4: Profiling & Monitoring (Continuous Improvement)
    /// Performance profiling configuration
    pub profiling_config: ProfilingConfig,
    /// Performance monitoring and telemetry configuration
    pub monitoring_config: MonitoringConfig,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_type: EmbeddingModelType::Default,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            expected_dimension: Some(DEFAULT_EMBEDDING_DIMENSION),
            session_timeout_seconds: DEFAULT_SESSION_TIMEOUT_SECONDS,
            enable_session_cleanup: DEFAULT_ENABLE_SESSION_CLEANUP,
            embedding_batch_size: Some(DEFAULT_EMBEDDING_BATCH_SIZE),
            
            // Phase 1: Performance defaults - optimized for typical embedding workloads
            intra_op_num_threads: Some(num_cpus::get()), // Use all available CPU cores
            inter_op_num_threads: Some(1), // Single inter-op thread (FastEmbed style)
            enable_parallel_execution: false, // Most embedding models are sequential
            graph_optimization_level: 3, // Maximum optimization by default
            enable_memory_pattern: true, // Enable for fixed-size inputs (typical for embeddings)
            enable_deterministic_compute: false, // Prioritize performance over determinism
            profiling_file_path: None,
            cuda_memory_limit: None, // No limit by default
            enable_cpu_arena: true, // Enable arena allocator for better memory management
            
            // Phase 2: Advanced optimization defaults
            io_binding_config: IOBindingConfig::default(),
            memory_pool_config: MemoryPoolConfig::default(),
            execution_providers: vec![ExecutionProvider::Auto],
            enable_provider_auto_selection: true,
            dynamic_batch_config: DynamicBatchConfig::default(),
            enable_cuda_memory_streams: true,
            enable_hardware_detection: true,
            
            // Phase 3: Execution Provider Optimization (Platform Specific)
            cuda_config: CudaExecutionProviderConfig::default(),
            cpu_config: CpuExecutionProviderConfig::default(),
            
            // Phase 4: Profiling & Monitoring (Continuous Improvement)
            profiling_config: ProfilingConfig::default(),
            monitoring_config: MonitoringConfig::default(),
        }
    }
}

impl EmbeddingConfig {
    /// Create a new embedding configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new ONNX embedding configuration.
    pub fn new_onnx(model_path: PathBuf, tokenizer_path: PathBuf) -> Self {
        Self {
            model_type: EmbeddingModelType::Onnx,
            onnx_model_path: Some(model_path),
            onnx_tokenizer_path: Some(tokenizer_path),
            ..Default::default()
        }
    }


    /// Set the expected embedding dimension.
    pub fn with_expected_dimension(mut self, dimension: usize) -> Self {
        self.expected_dimension = Some(dimension);
        self
    }

    /// Set the session timeout.
    pub fn with_session_timeout(mut self, timeout_seconds: u64) -> Self {
        self.session_timeout_seconds = timeout_seconds;
        self
    }


    /// Set the embedding batch size.
    pub fn with_embedding_batch_size(mut self, batch_size: usize) -> Self {
        self.embedding_batch_size = Some(batch_size);
        self
    }

    /// Set the model type.
    pub fn with_model_type(mut self, model_type: EmbeddingModelType) -> Self {
        self.model_type = model_type;
        self
    }

    /// Set the number of threads for intra-op parallelism.
    pub fn with_intra_op_threads(mut self, num_threads: usize) -> Self {
        self.intra_op_num_threads = Some(num_threads);
        self
    }

    /// Set the number of threads for inter-op parallelism.
    pub fn with_inter_op_threads(mut self, num_threads: usize) -> Self {
        self.inter_op_num_threads = Some(num_threads);
        self
    }

    /// Enable or disable parallel execution mode.
    pub fn with_parallel_execution(mut self, enable: bool) -> Self {
        self.enable_parallel_execution = enable;
        self
    }

    /// Set the graph optimization level (0=disable, 1=basic, 2=extended, 3=all).
    pub fn with_graph_optimization_level(mut self, level: u8) -> Self {
        self.graph_optimization_level = level.min(3); // Clamp to valid range
        self
    }

    /// Enable or disable memory pattern optimization.
    pub fn with_memory_pattern(mut self, enable: bool) -> Self {
        self.enable_memory_pattern = enable;
        self
    }

    /// Enable or disable deterministic compute.
    pub fn with_deterministic_compute(mut self, enable: bool) -> Self {
        self.enable_deterministic_compute = enable;
        self
    }

    /// Set the profiling output file path.
    pub fn with_profiling<P: Into<PathBuf>>(mut self, file_path: P) -> Self {
        self.profiling_file_path = Some(file_path.into());
        self
    }

    /// Enable or disable I/O binding.
    pub fn with_io_binding(mut self, enable: bool) -> Self {
        self.io_binding_config.enable_io_binding = enable;
        self
    }

    /// Set CUDA memory limit in bytes.
    pub fn with_cuda_memory_limit(mut self, limit_bytes: usize) -> Self {
        self.cuda_memory_limit = Some(limit_bytes);
        self
    }

    /// Enable or disable CPU arena allocator.
    pub fn with_cpu_arena(mut self, enable: bool) -> Self {
        self.enable_cpu_arena = enable;
        self
    }

    /// Get the embedding batch size, using default if not set.
    pub fn get_embedding_batch_size(&self) -> usize {
        self.embedding_batch_size.unwrap_or(DEFAULT_EMBEDDING_BATCH_SIZE)
    }

    /// Get the embedding dimension from configuration or default.
    pub fn get_embedding_dimension(&self) -> usize {
        self.expected_dimension.unwrap_or(DEFAULT_EMBEDDING_DIMENSION)
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        match self.model_type {
            EmbeddingModelType::Onnx => {
                if self.onnx_model_path.is_none() {
                    return Err(SagittaEmbedError::configuration("ONNX model path is required for ONNX models"));
                }
                if self.onnx_tokenizer_path.is_none() {
                    return Err(SagittaEmbedError::configuration("ONNX tokenizer path is required for ONNX models"));
                }
            },
            EmbeddingModelType::Default => {
                // Default model doesn't require additional validation
            }
        }

        Ok(())
    }

    /// Check if CUDA should be used (determined at compile time).
    pub fn should_use_cuda(&self) -> bool {
        cfg!(feature = "cuda")
    }

    // Phase 2: Advanced optimization configuration methods

    /// Configure I/O binding settings.
    pub fn with_io_binding_config(mut self, config: IOBindingConfig) -> Self {
        self.io_binding_config = config;
        self
    }

    /// Enable or disable pre-allocated tensor buffers.
    pub fn with_pre_allocated_buffers(mut self, enable: bool) -> Self {
        self.io_binding_config.enable_pre_allocated_buffers = enable;
        self
    }

    /// Set the number of pre-allocated input buffers.
    pub fn with_pre_allocated_input_buffers(mut self, count: usize) -> Self {
        self.io_binding_config.pre_allocated_input_buffers = count;
        self
    }

    /// Set the number of pre-allocated output buffers.
    pub fn with_pre_allocated_output_buffers(mut self, count: usize) -> Self {
        self.io_binding_config.pre_allocated_output_buffers = count;
        self
    }

    /// Enable or disable zero-copy memory operations.
    pub fn with_zero_copy(mut self, enable: bool) -> Self {
        self.io_binding_config.enable_zero_copy = enable;
        self
    }

    /// Enable or disable batch-optimized I/O binding.
    pub fn with_batch_optimization(mut self, enable: bool) -> Self {
        self.io_binding_config.enable_batch_optimization = enable;
        self
    }

    /// Configure memory pool settings.
    pub fn with_memory_pool_config(mut self, config: MemoryPoolConfig) -> Self {
        self.memory_pool_config = config;
        self
    }

    /// Enable or disable memory pool for tensor reuse.
    pub fn with_memory_pool(mut self, enable: bool) -> Self {
        self.memory_pool_config.enable_pool = enable;
        self
    }

    /// Set the maximum number of tensors to keep in pool.
    pub fn with_max_pool_size(mut self, size: usize) -> Self {
        self.memory_pool_config.max_pool_size = size;
        self
    }

    /// Set the maximum memory usage for pool in bytes.
    pub fn with_max_pool_memory(mut self, bytes: usize) -> Self {
        self.memory_pool_config.max_pool_memory_bytes = bytes;
        self
    }

    /// Enable or disable memory pressure detection.
    pub fn with_memory_pressure_detection(mut self, enable: bool) -> Self {
        self.memory_pool_config.enable_memory_pressure_detection = enable;
        self
    }

    /// Set the memory pressure threshold (0.0-1.0).
    pub fn with_memory_pressure_threshold(mut self, threshold: f32) -> Self {
        self.memory_pool_config.memory_pressure_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set execution provider preference order.
    pub fn with_execution_providers(mut self, providers: Vec<ExecutionProvider>) -> Self {
        self.execution_providers = providers;
        self
    }

    /// Add an execution provider to the preference list.
    pub fn add_execution_provider(mut self, provider: ExecutionProvider) -> Self {
        self.execution_providers.push(provider);
        self
    }

    /// Enable or disable execution provider auto-selection.
    pub fn with_provider_auto_selection(mut self, enable: bool) -> Self {
        self.enable_provider_auto_selection = enable;
        self
    }

    /// Configure dynamic batch optimization settings.
    pub fn with_dynamic_batch_config(mut self, config: DynamicBatchConfig) -> Self {
        self.dynamic_batch_config = config;
        self
    }

    /// Enable or disable dynamic batch size optimization.
    pub fn with_dynamic_batching(mut self, enable: bool) -> Self {
        self.dynamic_batch_config.enable_dynamic_batching = enable;
        self
    }

    /// Set the minimum batch size for dynamic batching.
    pub fn with_min_batch_size(mut self, size: usize) -> Self {
        self.dynamic_batch_config.min_batch_size = size;
        self
    }

    /// Set the maximum batch size for dynamic batching.
    pub fn with_max_batch_size(mut self, size: usize) -> Self {
        self.dynamic_batch_config.max_batch_size = size;
        self
    }

    /// Set the target latency for batch size optimization.
    pub fn with_target_latency(mut self, latency_ms: u64) -> Self {
        self.dynamic_batch_config.target_latency_ms = latency_ms;
        self
    }

    /// Enable or disable memory usage prediction.
    pub fn with_memory_prediction(mut self, enable: bool) -> Self {
        self.dynamic_batch_config.enable_memory_prediction = enable;
        self
    }

    /// Set optimization mode (true = throughput, false = latency).
    pub fn with_throughput_optimization(mut self, optimize_for_throughput: bool) -> Self {
        self.dynamic_batch_config.optimize_for_throughput = optimize_for_throughput;
        self
    }

    /// Enable or disable CUDA memory stream optimization.
    pub fn with_cuda_memory_streams(mut self, enable: bool) -> Self {
        self.enable_cuda_memory_streams = enable;
        self
    }

    /// Enable or disable hardware capability detection.
    pub fn with_hardware_detection(mut self, enable: bool) -> Self {
        self.enable_hardware_detection = enable;
        self
    }

    // Phase 3: Execution Provider Optimization (Platform Specific)
    
    /// Configure advanced CUDA execution provider settings.
    pub fn with_cuda_config(mut self, config: CudaExecutionProviderConfig) -> Self {
        self.cuda_config = config;
        self
    }
    
    /// Enable CUDA with memory limit and optimization settings.
    pub fn with_cuda_optimized(mut self, device_id: i32, memory_limit_gb: Option<f32>) -> Self {
        self.cuda_config = CudaExecutionProviderConfig {
            enable: true,
            device_id,
            memory_limit: memory_limit_gb.map(|gb| (gb * 1_000_000_000.0) as usize),
            arena_extend_strategy: CudaArenaExtendStrategy::NextPowerOfTwo,
            enable_cuda_graph: true,
            enable_tensorrt: true,
            cudnn_conv_algo_search: CudaConvAlgorithmSearch::Exhaustive,
            enable_memory_pool: true,
            enable_memory_optimization: true,
        };
        self
    }
    
    /// Configure advanced CPU execution provider settings.
    pub fn with_cpu_config(mut self, config: CpuExecutionProviderConfig) -> Self {
        self.cpu_config = config;
        self
    }
    
    /// Enable CPU optimizations with NUMA awareness and cache optimization.
    pub fn with_cpu_optimized(mut self) -> Self {
        self.cpu_config = CpuExecutionProviderConfig {
            enable_arena: true,
            enable_numa: true,
            thread_affinity: None,
            enable_cache_optimization: true,
            enable_simd: true,
            enable_parallel_execution: true,
            optimization_level: CpuOptimizationLevel::Aggressive,
        };
        self
    }
    
    /// Set CPU thread affinity for NUMA optimization.
    pub fn with_cpu_thread_affinity(mut self, affinity: String) -> Self {
        self.cpu_config.thread_affinity = Some(affinity);
        self
    }
    
    // Phase 4: Profiling & Monitoring (Continuous Improvement)
    
    /// Configure performance profiling settings.
    pub fn with_profiling_config(mut self, config: ProfilingConfig) -> Self {
        self.profiling_config = config;
        self
    }
    
    /// Enable performance profiling with output file.
    pub fn with_profiling_enabled<P: Into<PathBuf>>(mut self, profile_file: P) -> Self {
        self.profiling_config = ProfilingConfig {
            enable_profiling: true,
            profile_file: Some(profile_file.into()),
            enable_custom_metrics: true,
            enable_session_stats: true,
            enable_memory_tracking: true,
            enable_throughput_monitoring: true,
            enable_latency_monitoring: true,
            sampling_rate: 1.0,
            enable_operator_profiling: true,
        };
        self
    }
    
    /// Configure performance monitoring and telemetry.
    pub fn with_monitoring_config(mut self, config: MonitoringConfig) -> Self {
        self.monitoring_config = config;
        self
    }
    
    /// Enable performance monitoring with custom thresholds.
    pub fn with_monitoring_enabled(mut self, max_latency_ms: f64, min_throughput: f64) -> Self {
        self.monitoring_config = MonitoringConfig {
            enable_monitoring: true,
            metrics_interval_ms: 1000,
            enable_bottleneck_detection: true,
            performance_thresholds: PerformanceThresholds {
                max_latency_ms,
                min_throughput_ops_per_sec: min_throughput,
                ..PerformanceThresholds::default()
            },
            enable_metrics_export: false,
            metrics_export_endpoint: None,
        };
        self
    }
    
    /// Enable metrics export to external monitoring system.
    pub fn with_metrics_export<S: Into<String>>(mut self, endpoint: S) -> Self {
        self.monitoring_config.enable_metrics_export = true;
        self.monitoring_config.metrics_export_endpoint = Some(endpoint.into());
        self
    }

    /// Build and validate the configuration.
    pub fn build(self) -> Result<EmbeddingConfig> {
        self.validate()?;
        Ok(self)
    }

    /// Build the configuration without validation.
    pub fn build_unchecked(self) -> EmbeddingConfig {
        self
    }

    /// Creates a GPU-optimized configuration with I/O binding enabled
    /// Only available when compiled with CUDA support
    #[cfg(feature = "cuda")]
    pub fn with_gpu_optimization(mut self) -> Self {
        self.io_binding_config.enable_io_binding = true;
        self.io_binding_config.enable_pre_allocated_buffers = true;
        self.io_binding_config.enable_zero_copy = true;
        self.io_binding_config.enable_batch_optimization = true;
        self.enable_cuda_memory_streams = true;
        self.cuda_config.enable_memory_optimization = true;
        self.cuda_config.enable_memory_pool = true;
        self
    }

    /// GPU optimization is not available in CPU-only builds
    /// Use `with_auto_optimization()` for automatic feature detection
    #[cfg(not(feature = "cuda"))]
    pub fn with_gpu_optimization(self) -> Self {
        log::warn!("GPU optimization requested but CUDA feature not enabled. Using CPU optimization instead.");
        self.with_cpu_optimization()
    }

    /// Creates a CPU-optimized configuration with I/O binding disabled
    pub fn with_cpu_optimization(mut self) -> Self {
        self.io_binding_config.enable_io_binding = false;
        self.io_binding_config.enable_pre_allocated_buffers = false;
        self.io_binding_config.enable_zero_copy = false;
        self.io_binding_config.enable_batch_optimization = false;
        self.enable_cuda_memory_streams = false;
        self.cpu_config.enable_arena = true;
        self.cpu_config.enable_numa = true;
        self.cpu_config.enable_cache_optimization = true;
        self.cpu_config.enable_simd = true;
        self.cpu_config.optimization_level = CpuOptimizationLevel::Aggressive;
        self
    }

    /// Creates an auto-optimized configuration based on compile-time features
    /// This is the recommended method for out-of-the-box usage
    pub fn with_auto_optimization(self) -> Self {
        #[cfg(feature = "cuda")]
        {
            self.with_gpu_optimization()
        }
        #[cfg(not(feature = "cuda"))]
        {
            self.with_cpu_optimization()
        }
    }
}

/// Phase 3: Advanced CUDA provider configuration
#[derive(Debug, Clone, PartialEq)]
pub struct CudaExecutionProviderConfig {
    /// Enable advanced CUDA configuration
    pub enable: bool,
    /// CUDA device ID to use (default: 0)
    pub device_id: i32,
    /// GPU memory limit in bytes (0 = unlimited)
    pub memory_limit: Option<usize>,
    /// Memory arena extend strategy
    pub arena_extend_strategy: CudaArenaExtendStrategy,
    /// Enable CUDA streams for copy operations
    pub enable_cuda_graph: bool,
    /// Enable TensorRT optimizations if available
    pub enable_tensorrt: bool,
    /// cuDNN convolution algorithm search strategy
    pub cudnn_conv_algo_search: CudaConvAlgorithmSearch,
    /// Enable memory pool optimization
    pub enable_memory_pool: bool,
    /// Enable CUDA memory optimization
    pub enable_memory_optimization: bool,
}

/// CUDA arena extend strategy
#[derive(Debug, Clone, PartialEq)]
pub enum CudaArenaExtendStrategy {
    /// Extend by next power of two
    NextPowerOfTwo,
    /// Extend by exact amount requested
    SameAsRequested,
}

/// cuDNN convolution algorithm search strategy
#[derive(Debug, Clone, PartialEq)]
pub enum CudaConvAlgorithmSearch {
    /// Exhaustive search (slowest startup, best performance)
    Exhaustive,
    /// Heuristic search (fast startup, good performance)
    Heuristic,
    /// Default algorithm (fastest startup, may be suboptimal)
    Default,
}

impl Default for CudaExecutionProviderConfig {
    fn default() -> Self {
        Self {
            enable: false, // Disabled by default, enable explicitly
            device_id: 0,
            memory_limit: None,
            arena_extend_strategy: CudaArenaExtendStrategy::NextPowerOfTwo,
            enable_cuda_graph: false,
            enable_tensorrt: false,
            cudnn_conv_algo_search: CudaConvAlgorithmSearch::Exhaustive,
            enable_memory_pool: true,
            enable_memory_optimization: true,
        }
    }
}

/// Phase 3: Advanced CPU provider configuration
#[derive(Debug, Clone, PartialEq)]
pub struct CpuExecutionProviderConfig {
    /// Enable CPU arena allocator
    pub enable_arena: bool,
    /// Enable NUMA awareness
    pub enable_numa: bool,
    /// CPU thread affinity settings
    pub thread_affinity: Option<String>,
    /// Enable cache optimization
    pub enable_cache_optimization: bool,
    /// Enable SIMD optimizations
    pub enable_simd: bool,
    /// Enable parallel execution within operators
    pub enable_parallel_execution: bool,
    /// CPU-specific optimization level
    pub optimization_level: CpuOptimizationLevel,
}

/// CPU optimization level
#[derive(Debug, Clone, PartialEq)]
pub enum CpuOptimizationLevel {
    /// Basic optimizations
    Basic,
    /// Standard optimizations (default)
    Standard,
    /// Aggressive optimizations (may use more memory)
    Aggressive,
}

impl Default for CpuExecutionProviderConfig {
    fn default() -> Self {
        Self {
            enable_arena: true,
            enable_numa: true,
            thread_affinity: None,
            enable_cache_optimization: true,
            enable_simd: true,
            enable_parallel_execution: false, // Conservative default
            optimization_level: CpuOptimizationLevel::Standard,
        }
    }
}

/// Phase 4: Performance profiling configuration
#[derive(Debug, Clone, PartialEq)]
pub struct ProfilingConfig {
    /// Enable ONNX Runtime profiling
    pub enable_profiling: bool,
    /// Profiling output file path
    pub profile_file: Option<PathBuf>,
    /// Enable custom metrics collection
    pub enable_custom_metrics: bool,
    /// Enable session pool statistics
    pub enable_session_stats: bool,
    /// Enable memory usage tracking
    pub enable_memory_tracking: bool,
    /// Enable throughput monitoring
    pub enable_throughput_monitoring: bool,
    /// Enable latency monitoring
    pub enable_latency_monitoring: bool,
    /// Profiling sampling rate (1.0 = all operations, 0.1 = 10% sampling)
    pub sampling_rate: f32,
    /// Enable detailed operator profiling
    pub enable_operator_profiling: bool,
}

impl Default for ProfilingConfig {
    fn default() -> Self {
        Self {
            enable_profiling: false,
            profile_file: None,
            enable_custom_metrics: false,
            enable_session_stats: true,
            enable_memory_tracking: true,
            enable_throughput_monitoring: true,
            enable_latency_monitoring: true,
            sampling_rate: 1.0,
            enable_operator_profiling: false,
        }
    }
}

/// Phase 4: Performance monitoring and telemetry
#[derive(Debug, Clone, PartialEq)]
pub struct MonitoringConfig {
    /// Enable performance monitoring
    pub enable_monitoring: bool,
    /// Metrics collection interval in milliseconds
    pub metrics_interval_ms: u64,
    /// Enable bottleneck detection
    pub enable_bottleneck_detection: bool,
    /// Performance alert thresholds
    pub performance_thresholds: PerformanceThresholds,
    /// Enable metrics export to external systems
    pub enable_metrics_export: bool,
    /// Metrics export endpoint (e.g., Prometheus, StatsD)
    pub metrics_export_endpoint: Option<String>,
}

/// Performance alert thresholds
#[derive(Debug, Clone, PartialEq)]
pub struct PerformanceThresholds {
    /// Maximum acceptable latency in milliseconds
    pub max_latency_ms: f64,
    /// Maximum acceptable memory usage in bytes
    pub max_memory_bytes: usize,
    /// Minimum acceptable throughput (operations per second)
    pub min_throughput_ops_per_sec: f64,
    /// Maximum CPU usage percentage (0.0-1.0)
    pub max_cpu_usage: f32,
    /// Maximum GPU memory usage percentage (0.0-1.0)
    pub max_gpu_memory_usage: f32,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_latency_ms: 100.0,      // 100ms max latency
            max_memory_bytes: 1_000_000_000, // 1GB max memory
            min_throughput_ops_per_sec: 10.0,   // 10 ops/sec minimum
            max_cpu_usage: 0.8,         // 80% max CPU
            max_gpu_memory_usage: 0.9,  // 90% max GPU memory
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_monitoring: false,
            metrics_interval_ms: 1000, // 1 second
            enable_bottleneck_detection: true,
            performance_thresholds: PerformanceThresholds::default(),
            enable_metrics_export: false,
            metrics_export_endpoint: None,
        }
    }
}

/// Configuration builder for more ergonomic configuration creation.
#[derive(Debug, Default)]
pub struct EmbeddingConfigBuilder {
    config: EmbeddingConfig,
}

impl EmbeddingConfigBuilder {
    /// Create a new configuration builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the model type.
    pub fn model_type(mut self, model_type: EmbeddingModelType) -> Self {
        self.config.model_type = model_type;
        self
    }

    /// Set the ONNX model path.
    pub fn onnx_model_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.config.onnx_model_path = Some(path.into());
        self
    }

    /// Set the ONNX model path from a string.
    pub fn onnx_model<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.config.onnx_model_path = Some(path.into());
        self
    }

    /// Set the ONNX tokenizer path.
    pub fn onnx_tokenizer_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.config.onnx_tokenizer_path = Some(path.into());
        self
    }

    /// Set the ONNX tokenizer path from a string.
    pub fn onnx_tokenizer<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.config.onnx_tokenizer_path = Some(path.into());
        self
    }


    /// Set the expected embedding dimension.
    pub fn expected_dimension(mut self, dimension: usize) -> Self {
        self.config.expected_dimension = Some(dimension);
        self
    }

    /// Set the session timeout.
    pub fn session_timeout(mut self, timeout_seconds: u64) -> Self {
        self.config.session_timeout_seconds = timeout_seconds;
        self
    }


    /// Set the embedding batch size
    pub fn embedding_batch_size(mut self, batch_size: usize) -> Self {
        self.config.embedding_batch_size = Some(batch_size);
        self
    }

    /// Set the number of threads for intra-op parallelism.
    pub fn with_intra_op_threads(mut self, num_threads: usize) -> Self {
        self.config.intra_op_num_threads = Some(num_threads);
        self
    }

    /// Set the number of threads for inter-op parallelism.
    pub fn with_inter_op_threads(mut self, num_threads: usize) -> Self {
        self.config.inter_op_num_threads = Some(num_threads);
        self
    }

    /// Enable or disable parallel execution mode.
    pub fn with_parallel_execution(mut self, enable: bool) -> Self {
        self.config.enable_parallel_execution = enable;
        self
    }

    /// Set the graph optimization level (0=disable, 1=basic, 2=extended, 3=all).
    pub fn with_graph_optimization_level(mut self, level: u8) -> Self {
        self.config.graph_optimization_level = level.min(3); // Clamp to valid range
        self
    }

    /// Enable or disable memory pattern optimization.
    pub fn with_memory_pattern(mut self, enable: bool) -> Self {
        self.config.enable_memory_pattern = enable;
        self
    }

    /// Enable or disable deterministic compute.
    pub fn with_deterministic_compute(mut self, enable: bool) -> Self {
        self.config.enable_deterministic_compute = enable;
        self
    }

    /// Set the profiling output file path.
    pub fn with_profiling<P: Into<PathBuf>>(mut self, file_path: P) -> Self {
        self.config.profiling_file_path = Some(file_path.into());
        self
    }

    /// Enable or disable I/O binding.
    pub fn with_io_binding(mut self, enable: bool) -> Self {
        self.config.io_binding_config.enable_io_binding = enable;
        self
    }

    /// Set CUDA memory limit in bytes.
    pub fn with_cuda_memory_limit(mut self, limit_bytes: usize) -> Self {
        self.config.cuda_memory_limit = Some(limit_bytes);
        self
    }

    /// Enable or disable CPU arena allocator.
    pub fn with_cpu_arena(mut self, enable: bool) -> Self {
        self.config.enable_cpu_arena = enable;
        self
    }

    // Phase 2: Advanced optimization configuration methods

    /// Configure I/O binding settings.
    pub fn with_io_binding_config(mut self, config: IOBindingConfig) -> Self {
        self.config.io_binding_config = config;
        self
    }

    /// Enable or disable pre-allocated tensor buffers.
    pub fn with_pre_allocated_buffers(mut self, enable: bool) -> Self {
        self.config.io_binding_config.enable_pre_allocated_buffers = enable;
        self
    }

    /// Set the number of pre-allocated input buffers.
    pub fn with_pre_allocated_input_buffers(mut self, count: usize) -> Self {
        self.config.io_binding_config.pre_allocated_input_buffers = count;
        self
    }

    /// Set the number of pre-allocated output buffers.
    pub fn with_pre_allocated_output_buffers(mut self, count: usize) -> Self {
        self.config.io_binding_config.pre_allocated_output_buffers = count;
        self
    }

    /// Enable or disable zero-copy memory operations.
    pub fn with_zero_copy(mut self, enable: bool) -> Self {
        self.config.io_binding_config.enable_zero_copy = enable;
        self
    }

    /// Enable or disable batch-optimized I/O binding.
    pub fn with_batch_optimization(mut self, enable: bool) -> Self {
        self.config.io_binding_config.enable_batch_optimization = enable;
        self
    }

    /// Configure memory pool settings.
    pub fn with_memory_pool_config(mut self, config: MemoryPoolConfig) -> Self {
        self.config.memory_pool_config = config;
        self
    }

    /// Enable or disable memory pool for tensor reuse.
    pub fn with_memory_pool(mut self, enable: bool) -> Self {
        self.config.memory_pool_config.enable_pool = enable;
        self
    }

    /// Set the maximum number of tensors to keep in pool.
    pub fn with_max_pool_size(mut self, size: usize) -> Self {
        self.config.memory_pool_config.max_pool_size = size;
        self
    }

    /// Set the maximum memory usage for pool in bytes.
    pub fn with_max_pool_memory(mut self, bytes: usize) -> Self {
        self.config.memory_pool_config.max_pool_memory_bytes = bytes;
        self
    }

    /// Enable or disable memory pressure detection.
    pub fn with_memory_pressure_detection(mut self, enable: bool) -> Self {
        self.config.memory_pool_config.enable_memory_pressure_detection = enable;
        self
    }

    /// Set the memory pressure threshold (0.0-1.0).
    pub fn with_memory_pressure_threshold(mut self, threshold: f32) -> Self {
        self.config.memory_pool_config.memory_pressure_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set execution provider preference order.
    pub fn with_execution_providers(mut self, providers: Vec<ExecutionProvider>) -> Self {
        self.config.execution_providers = providers;
        self
    }

    /// Add an execution provider to the preference list.
    pub fn add_execution_provider(mut self, provider: ExecutionProvider) -> Self {
        self.config.execution_providers.push(provider);
        self
    }

    /// Enable or disable execution provider auto-selection.
    pub fn with_provider_auto_selection(mut self, enable: bool) -> Self {
        self.config.enable_provider_auto_selection = enable;
        self
    }

    /// Configure dynamic batch optimization settings.
    pub fn with_dynamic_batch_config(mut self, config: DynamicBatchConfig) -> Self {
        self.config.dynamic_batch_config = config;
        self
    }

    /// Enable or disable dynamic batch size optimization.
    pub fn with_dynamic_batching(mut self, enable: bool) -> Self {
        self.config.dynamic_batch_config.enable_dynamic_batching = enable;
        self
    }

    /// Set the minimum batch size for dynamic batching.
    pub fn with_min_batch_size(mut self, size: usize) -> Self {
        self.config.dynamic_batch_config.min_batch_size = size;
        self
    }

    /// Set the maximum batch size for dynamic batching.
    pub fn with_max_batch_size(mut self, size: usize) -> Self {
        self.config.dynamic_batch_config.max_batch_size = size;
        self
    }

    /// Set the target latency for batch size optimization.
    pub fn with_target_latency(mut self, latency_ms: u64) -> Self {
        self.config.dynamic_batch_config.target_latency_ms = latency_ms;
        self
    }

    /// Enable or disable memory usage prediction.
    pub fn with_memory_prediction(mut self, enable: bool) -> Self {
        self.config.dynamic_batch_config.enable_memory_prediction = enable;
        self
    }

    /// Set optimization mode (true = throughput, false = latency).
    pub fn with_throughput_optimization(mut self, optimize_for_throughput: bool) -> Self {
        self.config.dynamic_batch_config.optimize_for_throughput = optimize_for_throughput;
        self
    }

    /// Enable or disable CUDA memory stream optimization.
    pub fn with_cuda_memory_streams(mut self, enable: bool) -> Self {
        self.config.enable_cuda_memory_streams = enable;
        self
    }

    /// Enable or disable hardware capability detection.
    pub fn with_hardware_detection(mut self, enable: bool) -> Self {
        self.config.enable_hardware_detection = enable;
        self
    }

    // Phase 3: Execution Provider Optimization (Platform Specific)
    
    /// Configure advanced CUDA execution provider settings.
    pub fn with_cuda_config(mut self, config: CudaExecutionProviderConfig) -> Self {
        self.config.cuda_config = config;
        self
    }
    
    /// Enable CUDA with memory limit and optimization settings.
    pub fn with_cuda_optimized(mut self, device_id: i32, memory_limit_gb: Option<f32>) -> Self {
        self.config.cuda_config = CudaExecutionProviderConfig {
            enable: true,
            device_id,
            memory_limit: memory_limit_gb.map(|gb| (gb * 1_000_000_000.0) as usize),
            arena_extend_strategy: CudaArenaExtendStrategy::NextPowerOfTwo,
            enable_cuda_graph: true,
            enable_tensorrt: true,
            cudnn_conv_algo_search: CudaConvAlgorithmSearch::Exhaustive,
            enable_memory_pool: true,
            enable_memory_optimization: true,
        };
        self
    }
    
    /// Configure advanced CPU execution provider settings.
    pub fn with_cpu_config(mut self, config: CpuExecutionProviderConfig) -> Self {
        self.config.cpu_config = config;
        self
    }
    
    /// Enable CPU optimizations with NUMA awareness and cache optimization.
    pub fn with_cpu_optimized(mut self) -> Self {
        self.config.cpu_config = CpuExecutionProviderConfig {
            enable_arena: true,
            enable_numa: true,
            thread_affinity: None,
            enable_cache_optimization: true,
            enable_simd: true,
            enable_parallel_execution: true,
            optimization_level: CpuOptimizationLevel::Aggressive,
        };
        self
    }
    
    /// Set CPU thread affinity for NUMA optimization.
    pub fn with_cpu_thread_affinity(mut self, affinity: String) -> Self {
        self.config.cpu_config.thread_affinity = Some(affinity);
        self
    }
    
    // Phase 4: Profiling & Monitoring (Continuous Improvement)
    
    /// Configure performance profiling settings.
    pub fn with_profiling_config(mut self, config: ProfilingConfig) -> Self {
        self.config.profiling_config = config;
        self
    }
    
    /// Enable performance profiling with output file.
    pub fn with_profiling_enabled<P: Into<PathBuf>>(mut self, profile_file: P) -> Self {
        self.config.profiling_config = ProfilingConfig {
            enable_profiling: true,
            profile_file: Some(profile_file.into()),
            enable_custom_metrics: true,
            enable_session_stats: true,
            enable_memory_tracking: true,
            enable_throughput_monitoring: true,
            enable_latency_monitoring: true,
            sampling_rate: 1.0,
            enable_operator_profiling: true,
        };
        self
    }
    
    /// Configure performance monitoring and telemetry.
    pub fn with_monitoring_config(mut self, config: MonitoringConfig) -> Self {
        self.config.monitoring_config = config;
        self
    }
    
    /// Enable performance monitoring with custom thresholds.
    pub fn with_monitoring_enabled(mut self, max_latency_ms: f64, min_throughput: f64) -> Self {
        self.config.monitoring_config = MonitoringConfig {
            enable_monitoring: true,
            metrics_interval_ms: 1000,
            enable_bottleneck_detection: true,
            performance_thresholds: PerformanceThresholds {
                max_latency_ms,
                min_throughput_ops_per_sec: min_throughput,
                ..PerformanceThresholds::default()
            },
            enable_metrics_export: false,
            metrics_export_endpoint: None,
        };
        self
    }
    
    /// Enable metrics export to external monitoring system.
    pub fn with_metrics_export<S: Into<String>>(mut self, endpoint: S) -> Self {
        self.config.monitoring_config.enable_metrics_export = true;
        self.config.monitoring_config.metrics_export_endpoint = Some(endpoint.into());
        self
    }

    /// Build and validate the configuration.
    pub fn build(self) -> Result<EmbeddingConfig> {
        self.config.validate()?;
        Ok(self.config)
    }

    /// Build the configuration without validation.
    pub fn build_unchecked(self) -> EmbeddingConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_default_config() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.model_type, EmbeddingModelType::Default);
        // max_sessions has been removed
    }

    #[test]
    fn test_config_builder() {
        let config = EmbeddingConfigBuilder::new()
            .model_type(EmbeddingModelType::Onnx)
            .expected_dimension(512)
            .build_unchecked();

        assert_eq!(config.model_type, EmbeddingModelType::Onnx);
        // max_sessions has been removed
        assert_eq!(config.expected_dimension, Some(512));
    }

    #[test]
    fn test_config_validation_missing_paths() {
        let config = EmbeddingConfig {
            model_type: EmbeddingModelType::Onnx,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("model path is required"));
    }

    // Removed test_config_validation_invalid_sessions as max_sessions no longer exists

    #[test]
    fn test_config_validation_with_valid_files() {
        let temp_dir = tempdir().unwrap();
        let model_path = temp_dir.path().join("model.onnx");
        let tokenizer_path = temp_dir.path().join("tokenizer.json");

        // Create dummy files
        fs::write(&model_path, "dummy model").unwrap();
        fs::write(&tokenizer_path, "dummy tokenizer").unwrap();

        let config = EmbeddingConfig {
            model_type: EmbeddingModelType::Onnx,
            onnx_model_path: Some(model_path),
            onnx_tokenizer_path: Some(tokenizer_path),
            ..Default::default()
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_new_onnx_config() {
        let config = EmbeddingConfig::new_onnx(PathBuf::from("/path/to/model.onnx"), PathBuf::from("/path/to/tokenizer.json"));
        assert_eq!(config.model_type, EmbeddingModelType::Onnx);
        assert_eq!(config.onnx_model_path, Some(PathBuf::from("/path/to/model.onnx")));
        assert_eq!(config.onnx_tokenizer_path, Some(PathBuf::from("/path/to/tokenizer.json")));
    }

    #[test]
    fn test_fluent_interface() {
        let config = EmbeddingConfig::new()
            .with_model_type(EmbeddingModelType::Onnx)
            .with_expected_dimension(512)
;

        assert_eq!(config.model_type, EmbeddingModelType::Onnx);
        // max_sessions has been removed
        assert_eq!(config.expected_dimension, Some(512));
    }

    #[test]
    fn test_performance_config_options() {
        let config = EmbeddingConfig::new()
            .with_intra_op_threads(2)
            .with_inter_op_threads(1)
            .with_parallel_execution(true)
            .with_graph_optimization_level(2)
            .with_memory_pattern(false)
            .with_deterministic_compute(true)
            .with_profiling("/tmp/profile.json")
            .with_io_binding(false)
            .with_cuda_memory_limit(1024 * 1024 * 1024) // 1GB
            .with_cpu_arena(false);

        assert_eq!(config.intra_op_num_threads, Some(2));
        assert_eq!(config.inter_op_num_threads, Some(1));
        assert!(config.enable_parallel_execution);
        assert_eq!(config.graph_optimization_level, 2);
        assert!(!config.enable_memory_pattern);
        assert!(config.enable_deterministic_compute);
        assert_eq!(config.profiling_file_path, Some(PathBuf::from("/tmp/profile.json")));
        assert!(!config.io_binding_config.enable_io_binding);
        assert_eq!(config.cuda_memory_limit, Some(1024 * 1024 * 1024));
        assert!(!config.enable_cpu_arena);
    }

    #[test]
    fn test_performance_config_builder() {
        let config = EmbeddingConfigBuilder::new()
            .model_type(EmbeddingModelType::Onnx)
            .onnx_model_path("/path/to/model.onnx")
            .onnx_tokenizer_path("/path/to/tokenizer.json")
            .with_intra_op_threads(4)
            .with_inter_op_threads(2)
            .with_graph_optimization_level(3)
            .with_memory_pattern(true)
            .with_cuda_memory_limit(2 * 1024 * 1024 * 1024) // 2GB
            .build_unchecked();

        assert_eq!(config.intra_op_num_threads, Some(4));
        assert_eq!(config.inter_op_num_threads, Some(2));
        assert_eq!(config.graph_optimization_level, 3);
        assert!(config.enable_memory_pattern);
        assert_eq!(config.cuda_memory_limit, Some(2 * 1024 * 1024 * 1024));
    }

    #[test]
    fn test_graph_optimization_level_clamping() {
        let config = EmbeddingConfig::new()
            .with_graph_optimization_level(10); // Should be clamped to 3

        assert_eq!(config.graph_optimization_level, 3);
    }

    #[test]
    fn test_default_performance_settings() {
        let config = EmbeddingConfig::default();
        
        // Verify sensible defaults are set
        assert!(config.intra_op_num_threads.is_some());
        assert!(config.inter_op_num_threads.is_some());
        assert!(!config.enable_parallel_execution);
        assert_eq!(config.graph_optimization_level, 3);
        assert!(config.enable_memory_pattern);
        assert!(!config.enable_deterministic_compute);
        
        // I/O binding should be enabled for CUDA builds, disabled for CPU-only builds
        #[cfg(feature = "cuda")]
        assert!(config.io_binding_config.enable_io_binding);
        #[cfg(not(feature = "cuda"))]
        assert_eq!(config.io_binding_config.enable_io_binding, false);
        
        assert!(config.enable_cpu_arena);
    }

    #[test]
    fn test_phase2_io_binding_config() {
        let io_config = IOBindingConfig {
            enable_io_binding: true,
            enable_pre_allocated_buffers: true,
            enable_zero_copy: false,
            enable_batch_optimization: true,
            pre_allocated_input_buffers: 8,
            pre_allocated_output_buffers: 8,
        };

        let config = EmbeddingConfig::new()
            .with_io_binding_config(io_config.clone())
            .with_pre_allocated_input_buffers(16)
            .with_zero_copy(true);

        assert!(config.io_binding_config.enable_io_binding);
        assert!(config.io_binding_config.enable_pre_allocated_buffers);
        assert!(config.io_binding_config.enable_zero_copy);
        assert!(config.io_binding_config.enable_batch_optimization);
        assert_eq!(config.io_binding_config.pre_allocated_input_buffers, 16);
        assert_eq!(config.io_binding_config.pre_allocated_output_buffers, 8);
    }

    #[test]
    fn test_phase2_memory_pool_config() {
        let config = EmbeddingConfig::new()
            .with_memory_pool(true)
            .with_max_pool_size(64)
            .with_max_pool_memory(1024 * 1024 * 1024) // 1GB
            .with_memory_pressure_detection(false)
            .with_memory_pressure_threshold(0.9);

        assert!(config.memory_pool_config.enable_pool);
        assert_eq!(config.memory_pool_config.max_pool_size, 64);
        assert_eq!(config.memory_pool_config.max_pool_memory_bytes, 1024 * 1024 * 1024);
        assert!(!config.memory_pool_config.enable_memory_pressure_detection);
        assert_eq!(config.memory_pool_config.memory_pressure_threshold, 0.9);
    }

    #[test]
    fn test_phase2_execution_providers() {
        let config = EmbeddingConfig::new()
            .with_execution_providers(vec![ExecutionProvider::Cuda, ExecutionProvider::Cpu])
            .add_execution_provider(ExecutionProvider::DirectML)
            .with_provider_auto_selection(false);

        assert_eq!(config.execution_providers.len(), 3);
        assert_eq!(config.execution_providers[0], ExecutionProvider::Cuda);
        assert_eq!(config.execution_providers[1], ExecutionProvider::Cpu);
        assert_eq!(config.execution_providers[2], ExecutionProvider::DirectML);
        assert!(!config.enable_provider_auto_selection);
    }

    #[test]
    fn test_phase2_dynamic_batch_config() {
        let config = EmbeddingConfig::new()
            .with_dynamic_batching(true)
            .with_min_batch_size(2)
            .with_max_batch_size(64)
            .with_target_latency(50)
            .with_memory_prediction(false)
            .with_throughput_optimization(true);

        assert!(config.dynamic_batch_config.enable_dynamic_batching);
        assert_eq!(config.dynamic_batch_config.min_batch_size, 2);
        assert_eq!(config.dynamic_batch_config.max_batch_size, 64);
        assert_eq!(config.dynamic_batch_config.target_latency_ms, 50);
        assert!(!config.dynamic_batch_config.enable_memory_prediction);
        assert!(config.dynamic_batch_config.optimize_for_throughput);
    }

    #[test]
    fn test_phase2_hardware_optimization() {
        let config = EmbeddingConfig::new()
            .with_cuda_memory_streams(false)
            .with_hardware_detection(false);

        assert!(!config.enable_cuda_memory_streams);
        assert!(!config.enable_hardware_detection);
    }

    #[test]
    fn test_phase2_config_builder() {
        let config = EmbeddingConfigBuilder::new()
            .model_type(EmbeddingModelType::Onnx)
            .with_pre_allocated_buffers(true)
            .with_pre_allocated_input_buffers(8)
            .with_memory_pool(true)
            .with_max_pool_size(32)
            .with_dynamic_batching(true)
            .with_min_batch_size(1)
            .with_max_batch_size(16)
            .add_execution_provider(ExecutionProvider::Cuda)
            .with_provider_auto_selection(true)
            .build_unchecked();

        assert!(config.io_binding_config.enable_pre_allocated_buffers);
        assert_eq!(config.io_binding_config.pre_allocated_input_buffers, 8);
        assert!(config.memory_pool_config.enable_pool);
        assert_eq!(config.memory_pool_config.max_pool_size, 32);
        assert!(config.dynamic_batch_config.enable_dynamic_batching);
        assert_eq!(config.dynamic_batch_config.min_batch_size, 1);
        assert_eq!(config.dynamic_batch_config.max_batch_size, 16);
        assert_eq!(config.execution_providers.len(), 2); // Auto + Cuda
        assert!(config.enable_provider_auto_selection);
    }

    #[test]
    fn test_memory_pressure_threshold_clamping() {
        let config = EmbeddingConfig::new()
            .with_memory_pressure_threshold(1.5); // Should be clamped to 1.0

        assert_eq!(config.memory_pool_config.memory_pressure_threshold, 1.0);

        let config2 = EmbeddingConfig::new()
            .with_memory_pressure_threshold(-0.1); // Should be clamped to 0.0

        assert_eq!(config2.memory_pool_config.memory_pressure_threshold, 0.0);
    }

    #[test]
    fn test_phase2_default_values() {
        let config = EmbeddingConfig::default();
        
        // I/O binding defaults - feature-gated for optimal out-of-the-box experience
        #[cfg(feature = "cuda")]
        {
            assert!(config.io_binding_config.enable_io_binding);
            assert!(config.io_binding_config.enable_pre_allocated_buffers);
            assert!(config.io_binding_config.enable_zero_copy);
            assert!(config.io_binding_config.enable_batch_optimization);
        }
        #[cfg(not(feature = "cuda"))]
        {
            assert_eq!(config.io_binding_config.enable_io_binding, false);
            assert_eq!(config.io_binding_config.enable_pre_allocated_buffers, false);
            assert_eq!(config.io_binding_config.enable_zero_copy, false);
            assert_eq!(config.io_binding_config.enable_batch_optimization, false);
        }
        
        assert_eq!(config.io_binding_config.pre_allocated_input_buffers, 4);
        assert_eq!(config.io_binding_config.pre_allocated_output_buffers, 4);

        // Memory pool defaults
        assert!(config.memory_pool_config.enable_pool);
        assert_eq!(config.memory_pool_config.max_pool_size, 32);
        assert_eq!(config.memory_pool_config.max_pool_memory_bytes, 512 * 1024 * 1024);
        assert!(config.memory_pool_config.enable_memory_pressure_detection);
        assert_eq!(config.memory_pool_config.memory_pressure_threshold, 0.8);

        // Execution provider defaults
        assert_eq!(config.execution_providers, vec![ExecutionProvider::Auto]);
        assert!(config.enable_provider_auto_selection);

        // Dynamic batch defaults
        assert!(config.dynamic_batch_config.enable_dynamic_batching);
        assert_eq!(config.dynamic_batch_config.min_batch_size, 1);
        assert_eq!(config.dynamic_batch_config.max_batch_size, 32);
        assert_eq!(config.dynamic_batch_config.target_latency_ms, 100);
        assert!(config.dynamic_batch_config.enable_memory_prediction);
        assert!(!config.dynamic_batch_config.optimize_for_throughput);

        // Hardware optimization defaults
        assert!(config.enable_cuda_memory_streams);
        assert!(config.enable_hardware_detection);
    }

    #[test]
    fn test_phase3_cuda_config() {
        let config = EmbeddingConfig::default()
            .with_cuda_optimized(1, Some(4.0)); // 4GB limit
        
        assert!(config.cuda_config.enable);
        assert_eq!(config.cuda_config.device_id, 1);
        assert_eq!(config.cuda_config.memory_limit, Some(4_000_000_000));
        assert_eq!(config.cuda_config.arena_extend_strategy, CudaArenaExtendStrategy::NextPowerOfTwo);
        assert!(config.cuda_config.enable_cuda_graph);
        assert!(config.cuda_config.enable_tensorrt);
        assert_eq!(config.cuda_config.cudnn_conv_algo_search, CudaConvAlgorithmSearch::Exhaustive);
        assert!(config.cuda_config.enable_memory_pool);
        assert!(config.cuda_config.enable_memory_optimization);
    }
    
    #[test]
    fn test_phase3_cuda_config_default() {
        let config = EmbeddingConfig::default();
        
        // CUDA should be disabled by default
        assert!(!config.cuda_config.enable);
        assert_eq!(config.cuda_config.device_id, 0);
        assert_eq!(config.cuda_config.memory_limit, None);
        assert_eq!(config.cuda_config.arena_extend_strategy, CudaArenaExtendStrategy::NextPowerOfTwo);
        assert!(!config.cuda_config.enable_cuda_graph);
        assert!(!config.cuda_config.enable_tensorrt);
        assert_eq!(config.cuda_config.cudnn_conv_algo_search, CudaConvAlgorithmSearch::Exhaustive);
        assert!(config.cuda_config.enable_memory_pool);
        assert!(config.cuda_config.enable_memory_optimization);
    }
    
    #[test]
    fn test_phase3_cuda_arena_strategies() {
        let mut config = EmbeddingConfig::default();
        
        // Test NextPowerOfTwo strategy
        config.cuda_config.arena_extend_strategy = CudaArenaExtendStrategy::NextPowerOfTwo;
        assert_eq!(config.cuda_config.arena_extend_strategy, CudaArenaExtendStrategy::NextPowerOfTwo);
        
        // Test SameAsRequested strategy
        config.cuda_config.arena_extend_strategy = CudaArenaExtendStrategy::SameAsRequested;
        assert_eq!(config.cuda_config.arena_extend_strategy, CudaArenaExtendStrategy::SameAsRequested);
    }
    
    #[test]
    fn test_phase3_cuda_conv_algorithms() {
        let mut config = EmbeddingConfig::default();
        
        // Test Exhaustive search
        config.cuda_config.cudnn_conv_algo_search = CudaConvAlgorithmSearch::Exhaustive;
        assert_eq!(config.cuda_config.cudnn_conv_algo_search, CudaConvAlgorithmSearch::Exhaustive);
        
        // Test Heuristic search
        config.cuda_config.cudnn_conv_algo_search = CudaConvAlgorithmSearch::Heuristic;
        assert_eq!(config.cuda_config.cudnn_conv_algo_search, CudaConvAlgorithmSearch::Heuristic);
        
        // Test Default algorithm
        config.cuda_config.cudnn_conv_algo_search = CudaConvAlgorithmSearch::Default;
        assert_eq!(config.cuda_config.cudnn_conv_algo_search, CudaConvAlgorithmSearch::Default);
    }
    
    #[test]
    fn test_phase3_cpu_config() {
        let config = EmbeddingConfig::default()
            .with_cpu_optimized()
            .with_cpu_thread_affinity("0-7".to_string());
        
        assert!(config.cpu_config.enable_arena);
        assert!(config.cpu_config.enable_numa);
        assert_eq!(config.cpu_config.thread_affinity, Some("0-7".to_string()));
        assert!(config.cpu_config.enable_cache_optimization);
        assert!(config.cpu_config.enable_simd);
        assert!(config.cpu_config.enable_parallel_execution);
        assert_eq!(config.cpu_config.optimization_level, CpuOptimizationLevel::Aggressive);
    }
    
    #[test]
    fn test_phase3_cpu_config_default() {
        let config = EmbeddingConfig::default();
        
        assert!(config.cpu_config.enable_arena);
        assert!(config.cpu_config.enable_numa);
        assert_eq!(config.cpu_config.thread_affinity, None);
        assert!(config.cpu_config.enable_cache_optimization);
        assert!(config.cpu_config.enable_simd);
        assert!(!config.cpu_config.enable_parallel_execution); // Conservative default
        assert_eq!(config.cpu_config.optimization_level, CpuOptimizationLevel::Standard);
    }
    
    #[test]
    fn test_phase3_cpu_optimization_levels() {
        let mut config = EmbeddingConfig::default();
        
        // Test Basic optimization
        config.cpu_config.optimization_level = CpuOptimizationLevel::Basic;
        assert_eq!(config.cpu_config.optimization_level, CpuOptimizationLevel::Basic);
        
        // Test Standard optimization
        config.cpu_config.optimization_level = CpuOptimizationLevel::Standard;
        assert_eq!(config.cpu_config.optimization_level, CpuOptimizationLevel::Standard);
        
        // Test Aggressive optimization
        config.cpu_config.optimization_level = CpuOptimizationLevel::Aggressive;
        assert_eq!(config.cpu_config.optimization_level, CpuOptimizationLevel::Aggressive);
    }
    
    #[test]
    fn test_phase4_profiling_config() {
        let config = EmbeddingConfig::default()
            .with_profiling_enabled("/tmp/profile.json");
        
        assert!(config.profiling_config.enable_profiling);
        assert_eq!(config.profiling_config.profile_file, Some(PathBuf::from("/tmp/profile.json")));
        assert!(config.profiling_config.enable_custom_metrics);
        assert!(config.profiling_config.enable_session_stats);
        assert!(config.profiling_config.enable_memory_tracking);
        assert!(config.profiling_config.enable_throughput_monitoring);
        assert!(config.profiling_config.enable_latency_monitoring);
        assert_eq!(config.profiling_config.sampling_rate, 1.0);
        assert!(config.profiling_config.enable_operator_profiling);
    }
    
    #[test]
    fn test_phase4_profiling_config_default() {
        let config = EmbeddingConfig::default();
        
        assert!(!config.profiling_config.enable_profiling);
        assert_eq!(config.profiling_config.profile_file, None);
        assert!(!config.profiling_config.enable_custom_metrics);
        assert!(config.profiling_config.enable_session_stats);
        assert!(config.profiling_config.enable_memory_tracking);
        assert!(config.profiling_config.enable_throughput_monitoring);
        assert!(config.profiling_config.enable_latency_monitoring);
        assert_eq!(config.profiling_config.sampling_rate, 1.0);
        assert!(!config.profiling_config.enable_operator_profiling);
    }
    
    #[test]
    fn test_phase4_monitoring_config() {
        let config = EmbeddingConfig::default()
            .with_monitoring_enabled(50.0, 20.0) // 50ms max latency, 20 ops/sec min throughput
            .with_metrics_export("http://prometheus:9090");
        
        assert!(config.monitoring_config.enable_monitoring);
        assert_eq!(config.monitoring_config.metrics_interval_ms, 1000);
        assert!(config.monitoring_config.enable_bottleneck_detection);
        assert_eq!(config.monitoring_config.performance_thresholds.max_latency_ms, 50.0);
        assert_eq!(config.monitoring_config.performance_thresholds.min_throughput_ops_per_sec, 20.0);
        assert!(config.monitoring_config.enable_metrics_export);
        assert_eq!(config.monitoring_config.metrics_export_endpoint, Some("http://prometheus:9090".to_string()));
    }
    
    #[test]
    fn test_phase4_monitoring_config_default() {
        let config = EmbeddingConfig::default();
        
        assert!(!config.monitoring_config.enable_monitoring);
        assert_eq!(config.monitoring_config.metrics_interval_ms, 1000);
        assert!(config.monitoring_config.enable_bottleneck_detection);
        assert_eq!(config.monitoring_config.performance_thresholds.max_latency_ms, 100.0);
        assert_eq!(config.monitoring_config.performance_thresholds.max_memory_bytes, 1_000_000_000);
        assert_eq!(config.monitoring_config.performance_thresholds.min_throughput_ops_per_sec, 10.0);
        assert_eq!(config.monitoring_config.performance_thresholds.max_cpu_usage, 0.8);
        assert_eq!(config.monitoring_config.performance_thresholds.max_gpu_memory_usage, 0.9);
        assert!(!config.monitoring_config.enable_metrics_export);
        assert_eq!(config.monitoring_config.metrics_export_endpoint, None);
    }
    
    #[test]
    fn test_phase3_phase4_config_builder() {
        let config = EmbeddingConfigBuilder::new()
            .onnx_model_path("/tmp/model.onnx")
            .onnx_tokenizer_path("/tmp/tokenizer.json")
            .with_cuda_optimized(0, Some(2.0))
            .with_cpu_optimized()
            .with_cpu_thread_affinity("0-3".to_string())
            .with_profiling_enabled("/tmp/profile.json")
            .with_monitoring_enabled(30.0, 15.0)
            .with_metrics_export("http://localhost:9090")
            .build_unchecked();
        
        // Test basic config
        // max_sessions has been removed
        
        // Test Phase 3 CUDA config
        assert!(config.cuda_config.enable);
        assert_eq!(config.cuda_config.device_id, 0);
        assert_eq!(config.cuda_config.memory_limit, Some(2_000_000_000));
        
        // Test Phase 3 CPU config
        assert!(config.cpu_config.enable_arena);
        assert!(config.cpu_config.enable_numa);
        assert_eq!(config.cpu_config.thread_affinity, Some("0-3".to_string()));
        assert_eq!(config.cpu_config.optimization_level, CpuOptimizationLevel::Aggressive);
        
        // Test Phase 4 profiling config
        assert!(config.profiling_config.enable_profiling);
        assert_eq!(config.profiling_config.profile_file, Some(PathBuf::from("/tmp/profile.json")));
        
        // Test Phase 4 monitoring config
        assert!(config.monitoring_config.enable_monitoring);
        assert_eq!(config.monitoring_config.performance_thresholds.max_latency_ms, 30.0);
        assert_eq!(config.monitoring_config.performance_thresholds.min_throughput_ops_per_sec, 15.0);
        assert!(config.monitoring_config.enable_metrics_export);
        assert_eq!(config.monitoring_config.metrics_export_endpoint, Some("http://localhost:9090".to_string()));
    }

    #[test]
    fn test_execution_provider_display_and_default() {
        assert_eq!(ExecutionProvider::default(), ExecutionProvider::Auto);
        
        // Test enum values exist
        let _cuda = ExecutionProvider::Cuda;
        let _cpu = ExecutionProvider::Cpu;
        let _directml = ExecutionProvider::DirectML;
        let _coreml = ExecutionProvider::CoreML;
        let _auto = ExecutionProvider::Auto;
    }

    #[test]
    fn test_config_validation_edge_cases() {
        // max_sessions validation removed

        // Test valid config passes validation
        let config = EmbeddingConfig::default();
        assert!(config.validate().is_ok());

        // Note: graph_optimization_level is automatically clamped to valid range [0-3]
        // so validation won't fail for invalid levels - they're just corrected
    }

    #[test]
    fn test_config_methods_comprehensive() {
        let config = EmbeddingConfig::default();
        
        // Test getter methods
        assert_eq!(config.get_embedding_batch_size(), 8); // Default batch size is 8
        assert_eq!(config.get_embedding_dimension(), 384); // Default dimension
        
        // Test should_use_cuda - this checks compile-time feature, not runtime config
        let cuda_enabled = cfg!(feature = "cuda");
        assert_eq!(config.should_use_cuda(), cuda_enabled);
    }

    #[test]
    fn test_memory_pool_config_methods() {
        let config = EmbeddingConfig::default()
            .with_memory_pool(true)
            .with_max_pool_size(100)
            .with_max_pool_memory(1024 * 1024)
            .with_memory_pressure_detection(true)
            .with_memory_pressure_threshold(0.8);
        
        assert!(config.memory_pool_config.enable_pool);
        assert_eq!(config.memory_pool_config.max_pool_size, 100);
        assert_eq!(config.memory_pool_config.max_pool_memory_bytes, 1024 * 1024);
        assert!(config.memory_pool_config.enable_memory_pressure_detection);
        assert_eq!(config.memory_pool_config.memory_pressure_threshold, 0.8);
    }

    #[test]
    fn test_io_binding_config_methods() {
        let config = EmbeddingConfig::default()
            .with_io_binding(true)
            .with_pre_allocated_buffers(true)
            .with_zero_copy(true)
            .with_batch_optimization(true)
            .with_pre_allocated_input_buffers(10)
            .with_pre_allocated_output_buffers(10);
        
        assert!(config.io_binding_config.enable_io_binding);
        assert!(config.io_binding_config.enable_pre_allocated_buffers);
        assert!(config.io_binding_config.enable_zero_copy);
        assert!(config.io_binding_config.enable_batch_optimization);
        assert_eq!(config.io_binding_config.pre_allocated_input_buffers, 10);
        assert_eq!(config.io_binding_config.pre_allocated_output_buffers, 10);
    }

    #[test]
    fn test_dynamic_batch_config_methods() {
        let config = EmbeddingConfig::default()
            .with_dynamic_batching(true)
            .with_min_batch_size(1)
            .with_max_batch_size(128)
            .with_target_latency(50)
            .with_memory_prediction(true)
            .with_throughput_optimization(true);
        
        assert!(config.dynamic_batch_config.enable_dynamic_batching);
        assert_eq!(config.dynamic_batch_config.min_batch_size, 1);
        assert_eq!(config.dynamic_batch_config.max_batch_size, 128);
        assert_eq!(config.dynamic_batch_config.target_latency_ms, 50);
        assert!(config.dynamic_batch_config.enable_memory_prediction);
        assert!(config.dynamic_batch_config.optimize_for_throughput);
    }

    #[test]
    fn test_execution_provider_methods() {
        let config = EmbeddingConfig::default()
            .with_execution_providers(vec![ExecutionProvider::Cuda, ExecutionProvider::Cpu])
            .add_execution_provider(ExecutionProvider::CoreML)
            .with_provider_auto_selection(true);
        
        assert_eq!(config.execution_providers.len(), 3);
        assert!(config.execution_providers.contains(&ExecutionProvider::Cuda));
        assert!(config.execution_providers.contains(&ExecutionProvider::Cpu));
        assert!(config.execution_providers.contains(&ExecutionProvider::CoreML));
        assert!(config.enable_provider_auto_selection);
    }

    #[test]
    fn test_hardware_optimization_methods() {
        let config = EmbeddingConfig::default()
            .with_cuda_memory_streams(true)
            .with_hardware_detection(true);
        
        assert!(config.enable_cuda_memory_streams);
        assert!(config.enable_hardware_detection);
    }

    #[test]
    fn test_builder_alias_methods() {
        let config = EmbeddingConfigBuilder::new()
            .onnx_model("/path/to/model.onnx")
            .onnx_tokenizer("/path/to/tokenizer.json")
            .build_unchecked();
        
        assert_eq!(config.onnx_model_path, Some(PathBuf::from("/path/to/model.onnx")));
        assert_eq!(config.onnx_tokenizer_path, Some(PathBuf::from("/path/to/tokenizer.json")));
    }

    #[test]
    fn test_performance_threshold_modifications() {
        let config = EmbeddingConfig::default()
            .with_monitoring_enabled(25.0, 30.0);
        
        assert_eq!(config.monitoring_config.performance_thresholds.max_latency_ms, 25.0);
        assert_eq!(config.monitoring_config.performance_thresholds.min_throughput_ops_per_sec, 30.0);
        
        // Verify other thresholds are still at defaults
        assert_eq!(config.monitoring_config.performance_thresholds.max_memory_bytes, 1_000_000_000);
        assert_eq!(config.monitoring_config.performance_thresholds.max_cpu_usage, 0.8);
        assert_eq!(config.monitoring_config.performance_thresholds.max_gpu_memory_usage, 0.9);
    }

    #[test]
    fn test_profiling_config_comprehensive() {
        let config = EmbeddingConfig::default()
            .with_profiling_enabled("/tmp/profile.json");
        
        assert!(config.profiling_config.enable_profiling);
        assert_eq!(config.profiling_config.profile_file, Some(PathBuf::from("/tmp/profile.json")));
        assert!(config.profiling_config.enable_custom_metrics);
        assert!(config.profiling_config.enable_session_stats);
        assert!(config.profiling_config.enable_memory_tracking);
        assert!(config.profiling_config.enable_throughput_monitoring);
        assert!(config.profiling_config.enable_latency_monitoring);
        assert_eq!(config.profiling_config.sampling_rate, 1.0);
        assert!(config.profiling_config.enable_operator_profiling);
    }

    #[test]
    fn test_cuda_config_comprehensive() {
        let config = EmbeddingConfig::default()
            .with_cuda_optimized(1, Some(4.0));
        
        assert!(config.cuda_config.enable);
        assert_eq!(config.cuda_config.device_id, 1);
        assert_eq!(config.cuda_config.memory_limit, Some(4_000_000_000));
        assert_eq!(config.cuda_config.arena_extend_strategy, CudaArenaExtendStrategy::NextPowerOfTwo);
        assert!(config.cuda_config.enable_cuda_graph);
        assert!(config.cuda_config.enable_tensorrt);
        assert_eq!(config.cuda_config.cudnn_conv_algo_search, CudaConvAlgorithmSearch::Exhaustive);
        assert!(config.cuda_config.enable_memory_pool);
        assert!(config.cuda_config.enable_memory_optimization);
    }

    #[test]
    fn test_config_struct_completeness() {
        // Test that all struct fields are accessible
        let config = EmbeddingConfig::default();
        
        // Basic fields
        let _ = config.model_type;
        let _ = config.onnx_model_path;
        let _ = config.onnx_tokenizer_path;
        // max_sessions has been removed
        let _ = config.expected_dimension;
        let _ = config.session_timeout_seconds;
        let _ = config.enable_session_cleanup;
        let _ = config.embedding_batch_size;
        
        // Performance fields
        let _ = config.intra_op_num_threads;
        let _ = config.inter_op_num_threads;
        let _ = config.enable_parallel_execution;
        let _ = config.graph_optimization_level;
        let _ = config.enable_memory_pattern;
        let _ = config.enable_deterministic_compute;
        let _ = config.profiling_file_path;
        let _ = config.cuda_memory_limit;
        let _ = config.enable_cpu_arena;
        
        // Advanced fields
        let _ = config.io_binding_config;
        let _ = config.memory_pool_config;
        let _ = config.execution_providers;
        let _ = config.enable_provider_auto_selection;
        let _ = config.dynamic_batch_config;
        let _ = config.enable_cuda_memory_streams;
        let _ = config.enable_hardware_detection;
        
        // Platform specific
        let _ = config.cuda_config;
        let _ = config.cpu_config;
        
        // Monitoring
        let _ = config.profiling_config;
        let _ = config.monitoring_config;
    }

    #[test]
    fn test_enum_completeness() {
        // Test all enum variants
        let _exec_providers = [
            ExecutionProvider::Cuda,
            ExecutionProvider::Cpu,
            ExecutionProvider::DirectML,
            ExecutionProvider::CoreML,
            ExecutionProvider::Auto,
        ];
        
        let _cuda_arena_strategies = [
            CudaArenaExtendStrategy::NextPowerOfTwo,
            CudaArenaExtendStrategy::SameAsRequested,
        ];
        
        let _cuda_conv_algorithms = [
            CudaConvAlgorithmSearch::Exhaustive,
            CudaConvAlgorithmSearch::Heuristic,
            CudaConvAlgorithmSearch::Default,
        ];
        
        let _cpu_optimization_levels = [
            CpuOptimizationLevel::Basic,
            CpuOptimizationLevel::Standard,
            CpuOptimizationLevel::Aggressive,
        ];
    }

    #[test]
    fn test_config_builder_pattern_comprehensive() {
        // Test builder pattern with all possible configurations
        let config = EmbeddingConfigBuilder::new()
            .model_type(EmbeddingModelType::Onnx)
            .onnx_model_path("/path/to/model.onnx")
            .onnx_tokenizer_path("/path/to/tokenizer.json")
            .expected_dimension(768)
            .embedding_batch_size(64)
            .with_intra_op_threads(8)
            .with_inter_op_threads(4)
            .with_parallel_execution(true)
            .with_graph_optimization_level(2)
            .with_memory_pattern(false)
            .with_deterministic_compute(true)
            .with_profiling("/tmp/profile.json")
            .with_io_binding(true)
            .with_cuda_memory_limit(1024 * 1024 * 1024)
            .with_cpu_arena(true)
            .build_unchecked();
        
        // Verify all settings were applied
        assert_eq!(config.model_type, EmbeddingModelType::Onnx);
        // max_sessions has been removed
        assert_eq!(config.expected_dimension, Some(768));
        assert_eq!(config.embedding_batch_size, Some(64));
        assert_eq!(config.intra_op_num_threads, Some(8));
        assert_eq!(config.inter_op_num_threads, Some(4));
        assert!(config.enable_parallel_execution);
        assert_eq!(config.graph_optimization_level, 2);
        assert!(!config.enable_memory_pattern);
        assert!(config.enable_deterministic_compute);
        assert_eq!(config.profiling_file_path, Some(PathBuf::from("/tmp/profile.json")));
        assert!(config.io_binding_config.enable_io_binding);
        assert_eq!(config.cuda_memory_limit, Some(1024 * 1024 * 1024));
        assert!(config.enable_cpu_arena);
    }

    #[test]
    fn test_default_impls_coverage() {
        // Test all Default implementations
        let _memory_pool = MemoryPoolConfig::default();
        let _io_binding = IOBindingConfig::default();
        let _dynamic_batch = DynamicBatchConfig::default();
        let _embedding_config = EmbeddingConfig::default();
        let _cuda_config = CudaExecutionProviderConfig::default();
        let _cpu_config = CpuExecutionProviderConfig::default();
        let _profiling_config = ProfilingConfig::default();
        let _monitoring_config = MonitoringConfig::default();
        let _performance_thresholds = PerformanceThresholds::default();
        let _execution_provider = ExecutionProvider::default();
    }

    #[test]
    fn test_auto_optimization_config() {
        let config = EmbeddingConfig::default().with_auto_optimization();
        
        // Auto-optimization should enable appropriate settings based on features
        #[cfg(feature = "cuda")]
        {
            // Should enable GPU optimizations
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
            // Should enable CPU optimizations
            assert_eq!(config.io_binding_config.enable_io_binding, false);
            assert_eq!(config.io_binding_config.enable_pre_allocated_buffers, false);
            assert_eq!(config.io_binding_config.enable_zero_copy, false);
            assert_eq!(config.io_binding_config.enable_batch_optimization, false);
            assert_eq!(config.enable_cuda_memory_streams, false);
            assert_eq!(config.cpu_config.enable_arena, true);
            assert_eq!(config.cpu_config.enable_numa, true);
            assert_eq!(config.cpu_config.enable_cache_optimization, true);
            assert_eq!(config.cpu_config.enable_simd, true);
        }
    }
} 