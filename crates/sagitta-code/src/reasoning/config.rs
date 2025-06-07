use reasoning_engine::{
    config::{ReasoningConfig, StreamingConfig, OrchestrationConfig, GraphConfig, DecisionConfig, BacktrackingConfig, ReflectionConfig, DebugConfig},
};
use std::time::Duration;

use crate::config::types::SagittaCodeConfig;

pub fn create_reasoning_config(agent_config: &SagittaCodeConfig) -> ReasoningConfig {
    ReasoningConfig {
        max_iterations: agent_config.gemini.max_reasoning_steps, // Respect user's configured setting
        confidence_threshold: 0.7,
        step_timeout: Duration::from_secs(30),
        session_timeout: Duration::from_secs(300), // 5 minute session timeout
        streaming: StreamingConfig {
            max_buffer_size: 1024 * 1024, // 1MB
            backpressure_threshold: 0.8,
            max_concurrent_streams: 10,
            chunk_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(30),
            enable_retry: true,
            max_retry_attempts: 2, // Reduced from 3 to 2 to prevent excessive retries
            retry_base_delay: Duration::from_millis(100),
            retry_max_delay: Duration::from_secs(5), // Reduced from 10 to 5 seconds
        },
        orchestration: OrchestrationConfig {
            max_parallel_tools: 3, // Reduced from 5 to 3 to limit complexity
            global_timeout: Duration::from_secs(300), // 5 minute global timeout
            default_tool_timeout: Duration::from_secs(120), // 2 minutes for individual tools
            max_retry_attempts: 2, // Reduced from 3 to 2
            retry_base_delay: Duration::from_millis(200), // Slightly increased
            retry_max_delay: Duration::from_secs(5), // Reduced from 10 to 5 seconds
            enable_dynamic_replanning: true,
            enable_deadlock_detection: true,
            resource_allocation_timeout: Duration::from_secs(30),
        },
        // Initialize other nested configs with conservative defaults
        graph: GraphConfig {
            max_depth: 5, // Limit reasoning depth
            enable_parallel_execution: false, // Conservative default
            max_parallel_nodes: 3, // Limit parallel execution
            node_timeout: Duration::from_secs(30),
            enable_cycle_detection: true,
            max_cycles: 3, // Limit cycles
        },
        decision: DecisionConfig {
            min_confidence: 0.6,
            max_options: 3, // Limit decision alternatives
            enable_caching: true,
            decision_timeout: Duration::from_secs(30),
            history_weight: 0.3,
            context_weight: 0.4,
            tool_availability_weight: 0.3,
        },
        backtracking: BacktrackingConfig {
            enable_auto_backtrack: true,
            max_backtrack_attempts: 3, // Limit backtracking
            backtrack_confidence_threshold: 0.5,
            max_checkpoints: 10, // Limit checkpoints
            checkpoint_interval: 5,
        },
        reflection: ReflectionConfig {
            enable_reflection: true,
            min_session_length: 3,
            max_patterns: 100, // Limit pattern tracking
            pattern_similarity_threshold: 0.8,
            enable_failure_learning: true,
            enable_success_learning: true,
        },
        debug: DebugConfig {
            enable_detailed_logging: true,
            enable_metrics: true,
            enable_state_snapshots: false, // Disable to reduce overhead
            snapshot_interval: 10,
            enable_tracing: true,
            log_level: "info".to_string(),
            enable_debug_assertions: true,
        },
    }
} 