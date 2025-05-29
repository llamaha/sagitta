use reasoning_engine::{
    config::{ReasoningConfig, StreamingConfig, OrchestrationConfig, GraphConfig, DecisionConfig, BacktrackingConfig, ReflectionConfig, DebugConfig},
};
use std::time::Duration;

use crate::config::types::SagittaCodeConfig;

pub fn create_reasoning_config(agent_config: &SagittaCodeConfig) -> ReasoningConfig {
    ReasoningConfig {
        max_iterations: agent_config.gemini.max_reasoning_steps, // Corrected path
        confidence_threshold: 0.7, // Using the default value from reasoning_engine's ReasoningConfig
        step_timeout: Duration::from_secs(30), // Example, can be configured from agent_config if needed
        session_timeout: Duration::from_secs(300), // Example
        streaming: StreamingConfig {
            max_buffer_size: 1024 * 1024, // 1MB
            backpressure_threshold: 0.8,
            max_concurrent_streams: 10,
            chunk_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(30),
            enable_retry: true,
            max_retry_attempts: 3,
            retry_base_delay: Duration::from_millis(100),
            retry_max_delay: Duration::from_secs(10),
        },
        orchestration: OrchestrationConfig {
            max_parallel_tools: 5,
            global_timeout: Duration::from_secs(300),
            default_tool_timeout: Duration::from_secs(30),
            max_retry_attempts: 3, // Orchestration specific retries
            retry_base_delay: Duration::from_millis(100),
            retry_max_delay: Duration::from_secs(10),
            enable_dynamic_replanning: true,
            enable_deadlock_detection: true,
            resource_allocation_timeout: Duration::from_secs(30),
        },
        // Initialize other nested configs with defaults or from agent_config if applicable
        graph: GraphConfig::default(),
        decision: DecisionConfig::default(),
        backtracking: BacktrackingConfig::default(),
        reflection: ReflectionConfig::default(),
        debug: DebugConfig::default(),
    }
} 