//! Configuration types for the reasoning engine

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Main configuration for the reasoning engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    /// Maximum number of reasoning iterations
    pub max_iterations: u32,
    
    /// Confidence threshold for decision making
    pub confidence_threshold: f32,
    
    /// Timeout for individual reasoning steps
    pub step_timeout: Duration,
    
    /// Timeout for entire reasoning session
    pub session_timeout: Duration,
    
    /// Streaming configuration
    pub streaming: StreamingConfig,
    
    /// Graph execution configuration
    pub graph: GraphConfig,
    
    /// Decision making configuration
    pub decision: DecisionConfig,
    
    /// Tool orchestration configuration
    pub orchestration: OrchestrationConfig,
    
    /// Backtracking configuration
    pub backtracking: BacktrackingConfig,
    
    /// Reflection configuration
    pub reflection: ReflectionConfig,
    
    /// Debug configuration
    pub debug: DebugConfig,
    
    /// Control whether to use analyze_input tool for initial planning
    pub enable_analyze_input: bool,
    
    /// Control whether to use intent analyzer for response analysis
    pub enable_analyze_intent: bool,
    
    /// Enable autonomous execution mode (TODO-based execution)
    pub autonomous_mode: bool,
}

impl Default for ReasoningConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            confidence_threshold: 0.7,
            step_timeout: Duration::from_secs(30),
            session_timeout: Duration::from_secs(300), // 5 minutes
            streaming: StreamingConfig::default(),
            graph: GraphConfig::default(),
            decision: DecisionConfig::default(),
            orchestration: OrchestrationConfig::default(),
            backtracking: BacktrackingConfig::default(),
            reflection: ReflectionConfig::default(),
            debug: DebugConfig::default(),
            enable_analyze_input: true,
            enable_analyze_intent: true,
            autonomous_mode: false,
        }
    }
}

/// Configuration for streaming infrastructure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// Maximum buffer size in bytes
    pub max_buffer_size: usize,
    
    /// Backpressure threshold (0.0 to 1.0)
    pub backpressure_threshold: f32,
    
    /// Maximum number of concurrent streams
    pub max_concurrent_streams: u32,
    
    /// Chunk processing timeout
    pub chunk_timeout: Duration,
    
    /// Stream idle timeout
    pub idle_timeout: Duration,
    
    /// Enable automatic retry on stream errors
    pub enable_retry: bool,
    
    /// Maximum retry attempts
    pub max_retry_attempts: u32,
    
    /// Base retry delay
    pub retry_base_delay: Duration,
    
    /// Maximum retry delay
    pub retry_max_delay: Duration,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            max_buffer_size: 1024 * 1024, // 1MB
            backpressure_threshold: 0.8,
            max_concurrent_streams: 10,
            chunk_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(30),
            enable_retry: true,
            max_retry_attempts: 3,
            retry_base_delay: Duration::from_millis(100),
            retry_max_delay: Duration::from_secs(10),
        }
    }
}

/// Configuration for graph execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    /// Maximum depth for graph traversal
    pub max_depth: u32,
    
    /// Enable parallel node execution
    pub enable_parallel_execution: bool,
    
    /// Maximum number of parallel nodes
    pub max_parallel_nodes: u32,
    
    /// Node execution timeout
    pub node_timeout: Duration,
    
    /// Enable cycle detection
    pub enable_cycle_detection: bool,
    
    /// Maximum cycles allowed before termination
    pub max_cycles: u32,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            max_depth: 100,
            enable_parallel_execution: false, // Start conservative
            max_parallel_nodes: 4,
            node_timeout: Duration::from_secs(10),
            enable_cycle_detection: true,
            max_cycles: 3,
        }
    }
}

/// Configuration for decision making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionConfig {
    /// Minimum confidence required for decisions
    pub min_confidence: f32,
    
    /// Maximum number of options to consider
    pub max_options: u32,
    
    /// Enable decision caching
    pub enable_caching: bool,
    
    /// Decision timeout
    pub decision_timeout: Duration,
    
    /// Weight for historical success patterns
    pub history_weight: f32,
    
    /// Weight for context similarity
    pub context_weight: f32,
    
    /// Weight for tool availability
    pub tool_availability_weight: f32,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.5,
            max_options: 10,
            enable_caching: true,
            decision_timeout: Duration::from_secs(5),
            history_weight: 0.3,
            context_weight: 0.4,
            tool_availability_weight: 0.3,
        }
    }
}

/// Configuration for backtracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktrackingConfig {
    /// Enable automatic backtracking
    pub enable_auto_backtrack: bool,
    
    /// Maximum number of backtrack attempts
    pub max_backtrack_attempts: u32,
    
    /// Confidence threshold for triggering backtrack
    pub backtrack_confidence_threshold: f32,
    
    /// Maximum number of checkpoints to maintain
    pub max_checkpoints: u32,
    
    /// Checkpoint creation interval (in steps)
    pub checkpoint_interval: u32,
}

impl Default for BacktrackingConfig {
    fn default() -> Self {
        Self {
            enable_auto_backtrack: true,
            max_backtrack_attempts: 5,
            backtrack_confidence_threshold: 0.3,
            max_checkpoints: 20,
            checkpoint_interval: 5,
        }
    }
}

/// Configuration for reflection engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionConfig {
    /// Enable reflection after each session
    pub enable_reflection: bool,
    
    /// Minimum session length to trigger reflection
    pub min_session_length: u32,
    
    /// Maximum number of patterns to track
    pub max_patterns: u32,
    
    /// Pattern similarity threshold
    pub pattern_similarity_threshold: f32,
    
    /// Enable learning from failures
    pub enable_failure_learning: bool,
    
    /// Enable learning from successes
    pub enable_success_learning: bool,
}

impl Default for ReflectionConfig {
    fn default() -> Self {
        Self {
            enable_reflection: true,
            min_session_length: 3,
            max_patterns: 1000,
            pattern_similarity_threshold: 0.8,
            enable_failure_learning: true,
            enable_success_learning: true,
        }
    }
}

/// Configuration for debugging and observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    /// Enable detailed logging
    pub enable_detailed_logging: bool,
    
    /// Enable performance metrics
    pub enable_metrics: bool,
    
    /// Enable state snapshots
    pub enable_state_snapshots: bool,
    
    /// Snapshot interval (in steps)
    pub snapshot_interval: u32,
    
    /// Enable execution tracing
    pub enable_tracing: bool,
    
    /// Log level for reasoning engine
    pub log_level: String,
    
    /// Enable debug assertions
    pub enable_debug_assertions: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            enable_detailed_logging: true,
            enable_metrics: true,
            enable_state_snapshots: false, // Can be expensive
            snapshot_interval: 10,
            enable_tracing: true,
            log_level: "info".to_string(),
            enable_debug_assertions: cfg!(debug_assertions),
        }
    }
}

/// Configuration for tool orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationConfig {
    /// Maximum number of tools to execute in parallel
    pub max_parallel_tools: usize,
    /// Global timeout for orchestration
    pub global_timeout: Duration,
    /// Default timeout for individual tools
    pub default_tool_timeout: Duration,
    /// Maximum retry attempts for failed tools
    pub max_retry_attempts: u32,
    /// Base delay for exponential backoff
    pub retry_base_delay: Duration,
    /// Maximum delay for exponential backoff
    pub retry_max_delay: Duration,
    /// Enable dynamic replanning
    pub enable_dynamic_replanning: bool,
    /// Enable resource deadlock detection
    pub enable_deadlock_detection: bool,
    /// Resource allocation timeout
    pub resource_allocation_timeout: Duration,
}

impl Default for OrchestrationConfig {
    fn default() -> Self {
        Self {
            max_parallel_tools: 4,
            global_timeout: Duration::from_secs(300), // 5 minutes
            default_tool_timeout: Duration::from_secs(30),
            max_retry_attempts: 3,
            retry_base_delay: Duration::from_millis(100),
            retry_max_delay: Duration::from_secs(10),
            enable_dynamic_replanning: true,
            enable_deadlock_detection: true,
            resource_allocation_timeout: Duration::from_secs(30),
        }
    }
}

impl ReasoningConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_iterations == 0 {
            return Err("max_iterations must be greater than 0".to_string());
        }
        
        if !(0.0..=1.0).contains(&self.confidence_threshold) {
            return Err("confidence_threshold must be between 0.0 and 1.0".to_string());
        }
        
        if self.step_timeout.is_zero() {
            return Err("step_timeout must be greater than 0".to_string());
        }
        
        if self.session_timeout.is_zero() {
            return Err("session_timeout must be greater than 0".to_string());
        }
        
        self.streaming.validate()?;
        self.graph.validate()?;
        self.decision.validate()?;
        self.orchestration.validate()?;
        self.backtracking.validate()?;
        self.reflection.validate()?;
        
        Ok(())
    }
    
    /// Create a configuration optimized for development
    pub fn development() -> Self {
        let mut config = Self::default();
        config.debug.enable_detailed_logging = true;
        config.debug.enable_tracing = true;
        config.debug.enable_debug_assertions = true;
        config.debug.log_level = "debug".to_string();
        config.streaming.enable_retry = false; // Fail fast in development
        config.max_iterations = 20; // Lower for faster feedback
        config.enable_analyze_input = true;
        config.enable_analyze_intent = true;
        config.autonomous_mode = false;
        config
    }
    
    /// Create a configuration optimized for production
    pub fn production() -> Self {
        let mut config = Self::default();
        config.debug.enable_detailed_logging = false;
        config.debug.enable_debug_assertions = false;
        config.debug.log_level = "warn".to_string();
        config.streaming.enable_retry = true;
        config.streaming.max_retry_attempts = 5;
        config.max_iterations = 100; // Higher for complex tasks
        config.enable_analyze_input = true;
        config.enable_analyze_intent = true;
        config.autonomous_mode = true;
        config
    }
}

impl StreamingConfig {
    fn validate(&self) -> Result<(), String> {
        if self.max_buffer_size == 0 {
            return Err("max_buffer_size must be greater than 0".to_string());
        }
        
        if !(0.0..=1.0).contains(&self.backpressure_threshold) {
            return Err("backpressure_threshold must be between 0.0 and 1.0".to_string());
        }
        
        if self.max_concurrent_streams == 0 {
            return Err("max_concurrent_streams must be greater than 0".to_string());
        }
        
        Ok(())
    }
}

impl GraphConfig {
    fn validate(&self) -> Result<(), String> {
        if self.max_depth == 0 {
            return Err("max_depth must be greater than 0".to_string());
        }
        
        if self.max_parallel_nodes == 0 {
            return Err("max_parallel_nodes must be greater than 0".to_string());
        }
        
        Ok(())
    }
}

impl DecisionConfig {
    fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.min_confidence) {
            return Err("min_confidence must be between 0.0 and 1.0".to_string());
        }
        
        if self.max_options == 0 {
            return Err("max_options must be greater than 0".to_string());
        }
        
        let total_weight = self.history_weight + self.context_weight + self.tool_availability_weight;
        if (total_weight - 1.0).abs() > 0.01 {
            return Err("decision weights must sum to approximately 1.0".to_string());
        }
        
        Ok(())
    }
}

impl BacktrackingConfig {
    fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.backtrack_confidence_threshold) {
            return Err("backtrack_confidence_threshold must be between 0.0 and 1.0".to_string());
        }
        
        if self.max_checkpoints == 0 {
            return Err("max_checkpoints must be greater than 0".to_string());
        }
        
        Ok(())
    }
}

impl ReflectionConfig {
    fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.pattern_similarity_threshold) {
            return Err("pattern_similarity_threshold must be between 0.0 and 1.0".to_string());
        }
        
        if self.max_patterns == 0 {
            return Err("max_patterns must be greater than 0".to_string());
        }
        
        Ok(())
    }
}

impl OrchestrationConfig {
    fn validate(&self) -> Result<(), String> {
        if self.max_parallel_tools == 0 {
            return Err("max_parallel_tools must be greater than 0".to_string());
        }
        
        if self.global_timeout.is_zero() {
            return Err("global_timeout must be greater than 0".to_string());
        }
        
        if self.default_tool_timeout.is_zero() {
            return Err("default_tool_timeout must be greater than 0".to_string());
        }
        
        if self.resource_allocation_timeout.is_zero() {
            return Err("resource_allocation_timeout must be greater than 0".to_string());
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config_validation() {
        let config = ReasoningConfig::default();
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_development_config() {
        let config = ReasoningConfig::development();
        assert!(config.validate().is_ok());
        assert_eq!(config.debug.log_level, "debug");
        assert!(config.debug.enable_detailed_logging);
    }
    
    #[test]
    fn test_production_config() {
        let config = ReasoningConfig::production();
        assert!(config.validate().is_ok());
        assert_eq!(config.debug.log_level, "warn");
        assert!(!config.debug.enable_detailed_logging);
    }
    
    #[test]
    fn test_invalid_confidence_threshold() {
        let mut config = ReasoningConfig::default();
        config.confidence_threshold = 1.5;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_invalid_max_iterations() {
        let mut config = ReasoningConfig::default();
        config.max_iterations = 0;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_config_serialization() {
        let config = ReasoningConfig::default();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: ReasoningConfig = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(config.max_iterations, deserialized.max_iterations);
        assert_eq!(config.confidence_threshold, deserialized.confidence_threshold);
    }
} 