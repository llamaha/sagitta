use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tokio::sync::{Semaphore, Mutex};

use crate::error::Result;

/// A tool execution request with dependencies and constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRequest {
    /// Unique identifier for this execution request
    pub id: Uuid,
    /// Name of the tool to execute
    pub tool_name: String,
    /// Parameters for tool execution
    pub parameters: serde_json::Value,
    /// Tools that must complete before this tool can start
    pub dependencies: Vec<String>,
    /// Resources required by this tool
    pub required_resources: Vec<ResourceRequirement>,
    /// Priority level (0.0 to 1.0, higher is more important)
    pub priority: f32,
    /// Maximum execution time for this tool
    pub timeout: Option<Duration>,
    /// Whether this tool is critical for overall success
    pub is_critical: bool,
    /// Retry configuration for this specific tool
    pub retry_config: Option<RetryConfig>,
    /// Metadata for this execution
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Resource requirement for tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirement {
    /// Type of resource (e.g., "api_quota", "file_handle", "network_connection")
    pub resource_type: String,
    /// Amount of resource needed
    pub amount: u32,
    /// Whether this resource is exclusive (only one tool can use it)
    pub exclusive: bool,
    /// Maximum time to wait for resource allocation
    pub allocation_timeout: Option<Duration>,
}

/// Retry configuration for tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Base delay between retries
    pub base_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f32,
    /// Whether to retry on specific error types only
    pub retry_on_errors: Option<Vec<String>>,
    /// Enable alternative tool suggestions on failure
    pub enable_alternatives: bool,
    /// Enable parameter variation on retry
    pub enable_parameter_variation: bool,
}

/// Enhanced error recovery strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStrategy {
    /// Strategy type
    pub strategy_type: RecoveryStrategyType,
    /// Alternative tool to try
    pub alternative_tool: Option<String>,
    /// Modified parameters to try
    pub modified_parameters: Option<serde_json::Value>,
    /// Simplified approach with reduced requirements
    pub simplified_approach: Option<SimplifiedApproach>,
    /// Human-readable description of the strategy
    pub description: String,
    /// Confidence in this strategy (0.0 to 1.0)
    pub confidence: f32,
}

/// Types of recovery strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecoveryStrategyType {
    /// Retry with same parameters
    BasicRetry,
    /// Try alternative tool
    AlternativeTool,
    /// Modify parameters
    ParameterVariation,
    /// Use simpler approach
    SimplifiedApproach,
    /// Break down into smaller steps
    Decomposition,
    /// Use manual/shell commands
    ManualFallback,
    /// Skip non-critical operation
    GracefulSkip,
}

/// Simplified approach configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplifiedApproach {
    /// Reduced functionality parameters
    pub reduced_parameters: serde_json::Value,
    /// Description of what functionality is being reduced
    pub reduction_description: String,
    /// Whether this maintains core functionality
    pub maintains_core_functionality: bool,
}

/// Recovery suggestions for common failure scenarios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverySuggestions {
    /// Suggested recovery strategies
    pub strategies: Vec<RecoveryStrategy>,
    /// Analysis of the failure
    pub failure_analysis: FailureAnalysis,
    /// Recommended next steps for the user
    pub user_recommendations: Vec<String>,
    /// Whether manual intervention is recommended
    pub requires_manual_intervention: bool,
}

/// Analysis of tool failure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureAnalysis {
    /// Category of failure
    pub failure_category: FailureCategory,
    /// Root cause analysis
    pub root_cause: String,
    /// Whether this is a recoverable failure
    pub is_recoverable: bool,
    /// Likelihood of success with retry (0.0 to 1.0)
    pub retry_success_probability: f32,
    /// Alternative approaches available
    pub alternatives_available: bool,
}

/// Categories of tool failures
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FailureCategory {
    /// Network/connectivity issues
    NetworkError,
    /// Authentication/permission problems
    AuthenticationError,
    /// Invalid parameters
    ParameterError,
    /// Resource exhaustion
    ResourceError,
    /// Tool configuration issues
    ConfigurationError,
    /// External dependency failure
    DependencyError,
    /// Timeout
    TimeoutError,
    /// Unknown error
    UnknownError,
}

/// Validation outcome for tool execution results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationOutcome {
    /// Result is validated and consistent
    Validated,
    /// Result needs verification but can proceed
    NeedsVerification { reason: String },
    /// Result is inconsistent and should be retried
    Inconsistent { details: String },
    /// Verification itself failed
    VerificationFailed { error: String },
}

/// Status of tool execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// Execution is pending (waiting for dependencies or resources)
    Pending,
    /// Execution is currently running
    Running,
    /// Execution completed successfully
    Completed,
    /// Execution failed
    Failed,
    /// Execution was skipped due to failed dependencies
    Skipped,
    /// Execution was cancelled
    Cancelled,
    /// Execution timed out
    TimedOut,
}

/// Result of the overall orchestration
#[derive(Debug, Serialize, Deserialize)]
pub struct OrchestrationResult {
    /// Unique identifier for this orchestration
    pub orchestration_id: Uuid,
    /// Whether the overall orchestration was successful
    pub success: bool,
    /// Results from individual tool executions
    pub tool_results: HashMap<String, ToolExecutionResult>,
    /// Total execution time
    pub total_execution_time: Duration,
    /// Number of tools executed successfully
    pub successful_tools: u32,
    /// Number of tools that failed
    pub failed_tools: u32,
    /// Number of tools that were skipped due to dependencies
    pub skipped_tools: u32,
    /// Execution plan that was used
    pub execution_plan: ExecutionPlan,
    /// Any errors that occurred during orchestration
    pub orchestration_errors: Vec<String>,
    /// Performance metrics for this orchestration
    pub metrics: OrchestrationMetrics,
}

/// Result of individual tool execution
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    /// The original execution request
    pub request: ToolExecutionRequest,
    /// The tool result
    pub result: Option<crate::traits::ToolResult>,
    /// Execution status
    pub status: ExecutionStatus,
    /// Number of retry attempts made
    pub retry_attempts: u32,
    /// Time spent waiting for resources
    pub resource_wait_time: Duration,
    /// Actual execution time
    pub execution_time: Duration,
    /// Error message if execution failed
    pub error: Option<String>,
    /// Resources that were allocated for this execution
    pub allocated_resources: Vec<AllocatedResource>,
    /// Recovery suggestions if execution failed
    pub recovery_suggestions: Option<RecoverySuggestions>,
}

/// Execution plan for coordinating tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Unique identifier for this plan
    pub id: Uuid,
    /// Execution phases (tools that can run in parallel)
    pub phases: Vec<ExecutionPhase>,
    /// Total estimated execution time
    pub estimated_duration: Duration,
    /// Critical path through the execution
    pub critical_path: Vec<String>,
    /// Resource allocation plan
    pub resource_plan: ResourceAllocationPlan,
}

/// A phase of execution where tools can run in parallel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPhase {
    /// Phase number (0-based)
    pub phase_number: u32,
    /// Tools to execute in this phase
    pub tools: Vec<String>,
    /// Estimated duration for this phase
    pub estimated_duration: Duration,
    /// Resources required for this phase
    pub required_resources: HashMap<String, u32>,
}

/// Resource allocation plan for the entire orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocationPlan {
    /// Peak resource usage by type
    pub peak_usage: HashMap<String, u32>,
    /// Resource allocation timeline
    pub allocation_timeline: Vec<ResourceAllocation>,
    /// Potential resource conflicts
    pub conflicts: Vec<ResourceConflict>,
}

/// Resource allocation at a specific time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    /// Time offset from start of execution
    pub time_offset: Duration,
    /// Resources allocated at this time
    pub allocations: HashMap<String, u32>,
    /// Tools that will be using these resources
    pub tools: Vec<String>,
}

/// Resource conflict information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConflict {
    /// Type of resource in conflict
    pub resource_type: String,
    /// Tools competing for the resource
    pub competing_tools: Vec<String>,
    /// Severity of the conflict (0.0 to 1.0)
    pub severity: f32,
    /// Suggested resolution
    pub resolution: String,
}

/// Pool of available resources
#[derive(Debug)]
pub struct ResourcePool {
    /// Type of resource
    pub resource_type: String,
    /// Total available amount
    pub total_capacity: u32,
    /// Currently available amount
    pub available: u32,
    /// Semaphore for controlling access
    pub semaphore: Arc<Semaphore>,
    /// Queue of pending allocations
    pub pending_allocations: VecDeque<PendingAllocation>,
}

/// Resource that has been allocated to a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocatedResource {
    /// Allocation identifier
    pub allocation_id: Uuid,
    /// Type of resource
    pub resource_type: String,
    /// Amount allocated
    pub amount: u32,
    /// Time when allocated (using chrono for serialization)
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub allocated_at: chrono::DateTime<chrono::Utc>,
    /// Tool that owns this allocation
    pub owner_tool: String,
}

/// Pending resource allocation request
#[derive(Debug)]
pub struct PendingAllocation {
    /// Request identifier
    pub request_id: Uuid,
    /// Tool requesting the resource
    pub tool_name: String,
    /// Amount requested
    pub amount: u32,
    /// Priority of the request
    pub priority: f32,
    /// Time when request was made
    pub requested_at: std::time::Instant,
    /// Notification channel for allocation
    pub notify: tokio::sync::oneshot::Sender<Result<AllocatedResource>>,
}

/// Record of resource allocation for history tracking
#[derive(Debug, Clone)]
pub struct ResourceAllocationRecord {
    /// Tool that used the resource
    pub tool_name: String,
    /// Resource type
    pub resource_type: String,
    /// Amount used
    pub amount: u32,
    /// Duration of usage
    pub duration: Duration,
    /// Efficiency score (0.0 to 1.0)
    pub efficiency: f32,
    /// Timestamp
    pub timestamp: std::time::Instant,
}

/// Dependency graph for tool execution ordering
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// Nodes in the graph (tool names)
    pub nodes: HashSet<String>,
    /// Edges representing dependencies (from -> to)
    pub edges: HashMap<String, HashSet<String>>,
    /// Topological sort of the graph
    pub topological_order: Vec<String>,
    /// Critical path through the graph
    pub critical_path: Vec<String>,
}

/// Performance data for tool execution optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPerformanceData {
    /// Tool name
    pub tool_name: String,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: u64,
    /// Resource usage patterns
    pub resource_usage: HashMap<String, ResourceUsagePattern>,
    /// Success rate
    pub success_rate: f32,
    /// Number of executions recorded
    pub execution_count: u32,
}

/// Resource usage pattern for optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsagePattern {
    /// Average amount used
    pub avg_amount: f32,
    /// Peak amount used
    pub peak_amount: u32,
    /// Average duration of usage in milliseconds
    pub avg_duration_ms: u64,
    /// Efficiency score
    pub efficiency: f32,
}

/// Context for an active orchestration execution
#[derive(Debug)]
pub struct ExecutionContext {
    /// Orchestration identifier
    pub orchestration_id: Uuid,
    /// Execution plan being followed
    pub plan: ExecutionPlan,
    /// Current phase being executed
    pub current_phase: u32,
    /// Status of each tool
    pub tool_status: HashMap<String, ExecutionStatus>,
    /// Start time of orchestration
    pub start_time: Instant,
    /// Resource allocations for this execution
    pub resource_allocations: HashMap<String, Vec<AllocatedResource>>,
}

/// Metrics for orchestration performance tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationMetrics {
    /// Total orchestrations executed
    pub total_orchestrations: u64,
    /// Successful orchestrations
    pub successful_orchestrations: u64,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: u64,
    /// Average number of tools per orchestration
    pub avg_tools_per_orchestration: f32,
    /// Resource utilization efficiency
    pub resource_efficiency: f32,
    /// Parallel execution efficiency
    pub parallelization_efficiency: f32,
    /// Most common failure reasons
    pub failure_reasons: HashMap<String, u32>,
    /// Tool performance statistics
    pub tool_performance: HashMap<String, ToolPerformanceData>,
}

impl ToolExecutionRequest {
    pub fn new(tool_name: String, parameters: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            tool_name,
            parameters,
            dependencies: Vec::new(),
            required_resources: Vec::new(),
            priority: 0.5,
            timeout: None,
            is_critical: false,
            retry_config: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_dependency(mut self, dependency: String) -> Self {
        self.dependencies.push(dependency);
        self
    }

    pub fn with_resource(mut self, resource_type: String, amount: u32, exclusive: bool) -> Self {
        self.required_resources.push(ResourceRequirement {
            resource_type,
            amount,
            exclusive,
            allocation_timeout: None,
        });
        self
    }

    pub fn with_priority(mut self, priority: f32) -> Self {
        self.priority = priority.clamp(0.0, 1.0);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn as_critical(mut self) -> Self {
        self.is_critical = true;
        self
    }

    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }
} 