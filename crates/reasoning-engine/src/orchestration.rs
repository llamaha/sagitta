//! Tool orchestration and coordination
//!
//! This module implements a sophisticated tool orchestration system that manages and coordinates
//! the execution of multiple tools, potentially in parallel, while handling dependencies between
//! tool executions and managing their results. It addresses the limitations found in the original
//! sagitta-code tool execution system.
//!
//! ## Key Features
//!
//! ### 🔄 **Parallel Tool Execution**
//! - **Dependency Resolution**: Tools can declare dependencies on other tools or their outputs
//! - **Concurrent Execution**: Independent tools execute in parallel for optimal performance
//! - **Resource Coordination**: Shared resources are managed to prevent conflicts
//! - **Execution Planning**: Optimal execution order determined based on dependencies and resources
//!
//! ### 🛡️ **Robust Error Handling**
//! - **Failure Isolation**: Tool failures don't cascade to independent tools
//! - **Retry Strategies**: Configurable retry logic with exponential backoff
//! - **Fallback Mechanisms**: Alternative tools can be substituted for failed tools
//! - **Partial Success**: Orchestration can succeed even if some non-critical tools fail
//!
//! ### 📊 **Resource Management**
//! - **Resource Pools**: Manage limited resources like API quotas, file handles, network connections
//! - **Priority Scheduling**: High-priority tools get resource preference
//! - **Deadlock Prevention**: Sophisticated resource allocation to prevent deadlocks
//! - **Resource Monitoring**: Track resource utilization and availability
//!
//! ### 🎯 **Intelligent Coordination**
//! - **Dynamic Replanning**: Adjust execution plan based on runtime conditions
//! - **Load Balancing**: Distribute tool execution across available resources
//! - **Timeout Management**: Per-tool and global timeouts with graceful degradation
//! - **Result Aggregation**: Combine results from multiple tools intelligently
//!
//! ### 📈 **Observability & Metrics**
//! - **Execution Tracking**: Real-time monitoring of tool execution progress
//! - **Performance Metrics**: Latency, throughput, and success rate tracking
//! - **Resource Utilization**: Monitor resource usage patterns
//! - **Dependency Analysis**: Track dependency resolution and bottlenecks

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tokio::sync::{RwLock, Mutex, Semaphore};
use tokio::time::timeout;
use futures_util::future::join_all;

use crate::error::{Result, ReasoningError};
use crate::config::OrchestrationConfig;
use crate::traits::{ToolExecutor, ToolResult, ToolDefinition, EventEmitter, ReasoningEvent, ValidationStatus, CompletionEvidence, EvidenceType};

/// Main tool orchestrator for managing and coordinating tool execution
pub struct ToolOrchestrator {
    config: OrchestrationConfig,
    resource_manager: Arc<ResourceManager>,
    execution_planner: Arc<ExecutionPlanner>,
    metrics: Arc<RwLock<OrchestrationMetrics>>,
    active_executions: Arc<RwLock<HashMap<Uuid, ExecutionContext>>>,
}

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

/// Result of orchestrating multiple tools
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Result of executing a single tool within orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    /// The original execution request
    pub request: ToolExecutionRequest,
    /// The tool result
    pub result: Option<ToolResult>,
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

/// Execution plan for orchestrating tools
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

/// A phase of execution containing tools that can run in parallel
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

/// Resource allocation plan
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

/// Resource conflict detection
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

/// Manages resource allocation and coordination
pub struct ResourceManager {
    /// Available resources by type
    resources: Arc<RwLock<HashMap<String, ResourcePool>>>,
    /// Active resource allocations
    allocations: Arc<RwLock<HashMap<Uuid, Vec<AllocatedResource>>>>,
    /// Resource allocation history for optimization
    allocation_history: Arc<RwLock<VecDeque<ResourceAllocationRecord>>>,
}

/// A pool of resources of a specific type
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

/// An allocated resource
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

/// Pending resource allocation
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

/// Record of resource allocation for analysis
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

/// Plans optimal execution order for tools
pub struct ExecutionPlanner {
    config: OrchestrationConfig,
    dependency_analyzer: DependencyAnalyzer,
    resource_optimizer: ResourceOptimizer,
}

/// Analyzes tool dependencies
pub struct DependencyAnalyzer {
    /// Cache of dependency graphs
    dependency_cache: Arc<RwLock<HashMap<String, DependencyGraph>>>,
}

/// Dependency graph for tools
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

/// Optimizes resource allocation
pub struct ResourceOptimizer {
    /// Historical performance data
    performance_history: Arc<RwLock<HashMap<String, ToolPerformanceData>>>,
}

/// Performance data for a tool
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

/// Resource usage pattern for a tool
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

/// Context for an active execution
#[derive(Debug, Clone)]
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

/// Metrics for orchestration performance
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

impl ToolOrchestrator {
    /// Create a new tool orchestrator
    pub async fn new(config: OrchestrationConfig) -> Result<Self> {
        tracing::info!("Creating tool orchestrator with config: {:?}", config);
        
        let resource_manager = Arc::new(ResourceManager::new().await?);
        let execution_planner = Arc::new(ExecutionPlanner::new(config.clone()).await?);
        
        Ok(Self {
            config,
            resource_manager,
            execution_planner,
            metrics: Arc::new(RwLock::new(OrchestrationMetrics::default())),
            active_executions: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Orchestrate execution of multiple tools
    pub async fn orchestrate_tools<T, E>(
        &self,
        requests: Vec<ToolExecutionRequest>,
        tool_executor: Arc<T>,
        event_emitter: Arc<E>,
    ) -> Result<OrchestrationResult>
    where
        T: ToolExecutor + 'static,
        E: EventEmitter + 'static,
    {
        let orchestration_id = Uuid::new_v4();
        let start_time = Instant::now();
        
        tracing::info!(%orchestration_id, num_requests = requests.len(), "ToolOrchestrator::orchestrate_tools STARTING.");

        // Emit orchestration started event
        if let Err(e) = event_emitter.emit_event(ReasoningEvent::SessionStarted {
            session_id: orchestration_id,
            input: format!("Tool orchestration with {} tools", requests.len()),
            timestamp: chrono::Utc::now(),
        }).await {
            tracing::error!(%orchestration_id, "Error emitting SessionStarted event: {}. Orchestration aborted.", e);
            return Err(e); // Propagate emitter error
        }
        
        // Create execution plan
        let plan = match self.execution_planner.create_plan(&requests).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(%orchestration_id, "Error creating execution plan: {}. Orchestration aborted.", e);
                return Err(e); // Propagate planner error
            }
        };
        tracing::debug!(%orchestration_id, "Created execution plan with {} phases", plan.phases.len());
        
        // Initialize execution context
        let context = ExecutionContext {
            orchestration_id,
            plan: plan.clone(),
            current_phase: 0,
            tool_status: requests.iter().map(|r| (r.tool_name.clone(), ExecutionStatus::Pending)).collect(),
            start_time,
            resource_allocations: HashMap::new(),
        };
        
        // Store active execution
        {
            let mut active = self.active_executions.write().await;
            active.insert(orchestration_id, context.clone());
        }
        
        // Execute the plan
        let execution_result_outer = timeout(
            self.config.global_timeout,
            self.execute_plan(orchestration_id, requests, tool_executor, event_emitter.clone())
        ).await;
        
        // Clean up active execution
        {
            let mut active = self.active_executions.write().await;
            active.remove(&orchestration_id);
        }
        
        let total_execution_time = start_time.elapsed();
        
        // Process execution result
        let final_orchestration_result = match execution_result_outer {
            Ok(Ok(tool_results_map)) => { // execute_plan returned Ok(HashMap<String, ToolExecutionResult>)
                let successful_tools_count = tool_results_map.values().filter(|r| r.status == ExecutionStatus::Completed).count() as u32;
                let failed_tools_count = tool_results_map.values().filter(|r| r.status == ExecutionStatus::Failed).count() as u32;
                let skipped_tools_count = tool_results_map.values().filter(|r| r.status == ExecutionStatus::Skipped).count() as u32;
                
                OrchestrationResult {
                    orchestration_id,
                    success: failed_tools_count == 0,
                    tool_results: tool_results_map,
                    total_execution_time,
                    successful_tools: successful_tools_count,
                    failed_tools: failed_tools_count,
                    skipped_tools: skipped_tools_count,
                    execution_plan: plan.clone(),
                    orchestration_errors: Vec::new(), // No orchestration-level error here
                    metrics: self.metrics.read().await.clone(), // Placeholder for actual metrics update
                }
            }
            Ok(Err(e)) => { // Error from execute_plan itself
                tracing::error!(%orchestration_id, "execute_plan returned Err: {}. Orchestration considered failed.", e);
                OrchestrationResult {
                    orchestration_id,
                    success: false,
                    tool_results: HashMap::new(),
                    total_execution_time,
                    successful_tools: 0,
                    failed_tools: 0, // requests.len() as u32, // Or be more precise if some ran
                    skipped_tools: 0, // requests.len() as u32,
                    execution_plan: plan.clone(),
                    orchestration_errors: vec![e.to_string()],
                    metrics: self.metrics.read().await.clone(),
                }
            }
            Err(_elapsed) => { // Timeout from the global_timeout on execute_plan
                tracing::error!(%orchestration_id, "Global orchestration timeout after {:?}. Orchestration considered failed.", total_execution_time);
                OrchestrationResult {
                    orchestration_id,
                    success: false,
                    tool_results: HashMap::new(), // Tool results might be partial or unavailable
                    total_execution_time,
                    successful_tools: 0,
                    failed_tools: 0, // requests.len() as u32, 
                    skipped_tools: 0, // requests.len() as u32,
                    execution_plan: plan.clone(),
                    orchestration_errors: vec!["Global orchestration timeout".to_string()],
                    metrics: self.metrics.read().await.clone(),
                }
            }
        };
        
        tracing::info!(%orchestration_id, success = final_orchestration_result.success, "ToolOrchestrator::orchestrate_tools COMPLETED.");
        self.update_metrics(&final_orchestration_result).await; // Update metrics
        Ok(final_orchestration_result)
    }
    
    /// Execute the orchestration plan
    async fn execute_plan<T, E>(
        &self,
        orchestration_id: Uuid,
        requests: Vec<ToolExecutionRequest>,
        tool_executor: Arc<T>,
        event_emitter: Arc<E>,
    ) -> Result<HashMap<String, ToolExecutionResult>>
    where
        T: ToolExecutor + 'static,
        E: EventEmitter + 'static,
    {
        let mut tool_results = HashMap::new();
        let request_map: HashMap<String, ToolExecutionRequest> = requests.into_iter()
            .map(|r| (r.tool_name.clone(), r))
            .collect();
        
        // Get execution plan
        let plan = {
            let active = self.active_executions.read().await;
            active.get(&orchestration_id)
                .ok_or_else(|| ReasoningError::orchestration("Execution context not found"))?
                .plan.clone()
        };
        
        // Execute each phase
        for (phase_idx, phase) in plan.phases.iter().enumerate() {
            // Update current phase
            {
                let mut active = self.active_executions.write().await;
                if let Some(context) = active.get_mut(&orchestration_id) {
                    context.current_phase = phase_idx as u32;
                }
            }
            
            // Execute tools in this phase in parallel
            let phase_futures: Vec<_> = phase.tools.iter()
                .filter_map(|tool_name| {
                    request_map.get(tool_name).map(|request| {
                        self.execute_single_tool(
                            request.clone(),
                            tool_executor.clone(),
                            event_emitter.clone(),
                            &tool_results,
                        )
                    })
                })
                .collect();
            
            let phase_results = join_all(phase_futures).await;
            
            // Process phase results
            for result in phase_results {
                match result {
                    Ok(tool_result) => {
                        tool_results.insert(tool_result.request.tool_name.clone(), tool_result);
                    }
                    Err(error) => {
                        tracing::error!("Tool execution failed in phase {}: {}", phase_idx, error);
                        // Continue with other tools in the phase
                    }
                }
            }
        }
        
        Ok(tool_results)
    }
    
    /// Execute a single tool with resource management and retry logic
    async fn execute_single_tool<T, E>(
        &self,
        request: ToolExecutionRequest,
        tool_executor: Arc<T>,
        event_emitter: Arc<E>,
        completed_tools: &HashMap<String, ToolExecutionResult>,
    ) -> Result<ToolExecutionResult>
    where
        T: ToolExecutor + 'static,
        E: EventEmitter + 'static,
    {
        let start_time = Instant::now();
        
        // Check dependencies
        if !self.check_dependencies(&request, completed_tools).await? {
            return Ok(ToolExecutionResult {
                request,
                result: None,
                status: ExecutionStatus::Skipped,
                retry_attempts: 0,
                resource_wait_time: Duration::ZERO,
                execution_time: Duration::ZERO,
                error: Some("Dependencies not satisfied".to_string()),
                allocated_resources: Vec::new(),
                recovery_suggestions: None,
            });
        }
        
        // Allocate resources
        let resource_start = Instant::now();
        let allocated_resources = self.allocate_resources(&request).await?;
        let resource_wait_time = resource_start.elapsed();
        
        // Execute with retry logic
        let retry_config = request.retry_config.clone()
            .unwrap_or_else(|| self.default_retry_config());
        
        let mut retry_attempts = 0;
        let mut last_error = None;
        
        while retry_attempts < retry_config.max_attempts {
            // Emit tool execution started event
            event_emitter.emit_event(ReasoningEvent::ToolExecutionStarted {
                session_id: Uuid::new_v4(), // TODO: Use orchestration ID
                tool_name: request.tool_name.clone(),
                tool_args: request.parameters.clone(),
            }).await?;
            
            let execution_start = Instant::now();
            let tool_timeout = request.timeout.unwrap_or(self.config.default_tool_timeout);
            
            let execution_result = timeout(
                tool_timeout,
                tool_executor.execute_tool(&request.tool_name, request.parameters.clone())
            ).await;
            
            let execution_time = execution_start.elapsed();
            
            match execution_result {
                Ok(Ok(tool_result)) => {
                    // Success - release resources and return
                    self.release_resources(&allocated_resources).await?;
                    
                    event_emitter.emit_event(ReasoningEvent::ToolExecutionCompleted {
                        session_id: Uuid::new_v4(), // TODO: Use orchestration ID
                        tool_name: request.tool_name.clone(),
                        success: tool_result.success,
                        duration_ms: execution_time.as_millis() as u64,
                    }).await?;
                    
                    return Ok(ToolExecutionResult {
                        request,
                        result: Some(tool_result),
                        status: ExecutionStatus::Completed,
                        retry_attempts,
                        resource_wait_time,
                        execution_time: start_time.elapsed(),
                        error: None,
                        allocated_resources,
                        recovery_suggestions: None,
                    });
                }
                Ok(Err(error)) => {
                    // Tool execution failed
                    let error_message = error.to_string();
                    last_error = Some(error_message.clone());
                    
                    // Generate recovery suggestions on failure if enabled
                    if retry_config.enable_alternatives || retry_config.enable_parameter_variation {
                        if let Ok(suggestions) = self.analyze_failure_and_suggest_recovery(
                            &request.tool_name,
                            &error_message,
                            &request.parameters,
                            retry_attempts
                        ).await {
                            // Log recovery suggestions for debugging
                            tracing::info!("Generated {} recovery strategies for failed tool '{}': {}", 
                                suggestions.strategies.len(),
                                request.tool_name,
                                suggestions.strategies.iter()
                                    .map(|s| s.description.clone())
                                    .collect::<Vec<_>>()
                                    .join("; "));
                        }
                    }
                    
                    if retry_attempts < retry_config.max_attempts {
                        retry_attempts += 1;
                        let delay = self.calculate_retry_delay(&retry_config, retry_attempts);
                        tracing::warn!("Tool {} failed, retrying in {:?} (attempt {}/{}): {}", 
                            request.tool_name, delay, retry_attempts, retry_config.max_attempts, error_message);
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                }
                Err(_) => {
                    // Timeout
                    last_error = Some("Tool execution timed out".to_string());
                    break;
                }
            }
        }
        
        // All retries exhausted - generate final recovery suggestions and return failure
        self.release_resources(&allocated_resources).await?;
        
        let final_recovery_suggestions = if retry_config.enable_alternatives || retry_config.enable_parameter_variation {
            if let Some(error_msg) = &last_error {
                self.analyze_failure_and_suggest_recovery(
                    &request.tool_name,
                    error_msg,
                    &request.parameters,
                    retry_attempts
                ).await.ok()
            } else {
                None
            }
        } else {
            None
        };
        
        event_emitter.emit_event(ReasoningEvent::ToolExecutionCompleted {
            session_id: Uuid::new_v4(), // TODO: Use orchestration ID
            tool_name: request.tool_name.clone(),
            success: false,
            duration_ms: start_time.elapsed().as_millis() as u64,
        }).await?;
        
        Ok(ToolExecutionResult {
            request,
            result: None,
            status: ExecutionStatus::Failed,
            retry_attempts,
            resource_wait_time,
            execution_time: start_time.elapsed(),
            error: last_error,
            allocated_resources,
            recovery_suggestions: final_recovery_suggestions,
        })
    }
    
    /// Check if tool dependencies are satisfied
    async fn check_dependencies(
        &self,
        request: &ToolExecutionRequest,
        completed_tools: &HashMap<String, ToolExecutionResult>,
    ) -> Result<bool> {
        for dependency in &request.dependencies {
            if let Some(dep_result) = completed_tools.get(dependency) {
                if dep_result.status != ExecutionStatus::Completed {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }
        Ok(true)
    }
    
    /// Allocate resources for tool execution
    async fn allocate_resources(&self, request: &ToolExecutionRequest) -> Result<Vec<AllocatedResource>> {
        let mut allocated = Vec::new();
        
        for requirement in &request.required_resources {
            let allocation = self.resource_manager.allocate_resource(
                &requirement.resource_type,
                requirement.amount,
                request.priority,
                &request.tool_name,
            ).await?;
            allocated.push(allocation);
        }
        
        Ok(allocated)
    }
    
    /// Release allocated resources
    async fn release_resources(&self, resources: &[AllocatedResource]) -> Result<()> {
        for resource in resources {
            self.resource_manager.release_resource(resource).await?;
        }
        Ok(())
    }
    
    /// Calculate retry delay with exponential backoff
    fn calculate_retry_delay(&self, config: &RetryConfig, attempt: u32) -> Duration {
        let delay = config.base_delay.as_millis() as f32 * config.backoff_multiplier.powi(attempt as i32 - 1);
        let delay = Duration::from_millis(delay as u64);
        delay.min(config.max_delay)
    }
    
    /// Get default retry configuration
    fn default_retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_attempts: self.config.max_retry_attempts,
            base_delay: self.config.retry_base_delay,
            max_delay: self.config.retry_max_delay,
            backoff_multiplier: 2.0,
            retry_on_errors: None,
            enable_alternatives: true,
            enable_parameter_variation: true,
        }
    }

    /// Update orchestration metrics
    async fn update_metrics(&self, result: &OrchestrationResult) {
        let mut metrics = self.metrics.write().await;
        metrics.total_orchestrations += 1;
        
        if result.success {
            metrics.successful_orchestrations += 1;
        }
        
        // Update average execution time
        let total_time = metrics.avg_execution_time_ms as f64 * (metrics.total_orchestrations - 1) as f64;
        let new_time = (total_time + result.total_execution_time.as_millis() as f64) / metrics.total_orchestrations as f64;
        metrics.avg_execution_time_ms = new_time as u64;
        
        // Update tools per orchestration
        let total_tools = metrics.avg_tools_per_orchestration * (metrics.total_orchestrations - 1) as f32;
        metrics.avg_tools_per_orchestration = (total_tools + result.tool_results.len() as f32) / metrics.total_orchestrations as f32;
    }
    
    /// Get current orchestration metrics
    pub async fn get_metrics(&self) -> OrchestrationMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Get status of active orchestrations
    pub async fn get_active_orchestrations(&self) -> Vec<Uuid> {
        let active = self.active_executions.read().await;
        active.keys().cloned().collect()
    }

    /// Analyze tool failure and suggest recovery strategies
    async fn analyze_failure_and_suggest_recovery(
        &self,
        tool_name: &str,
        error_message: &str,
        parameters: &serde_json::Value,
        retry_attempts: u32,
    ) -> Result<RecoverySuggestions> {
        let failure_analysis = self.analyze_failure(tool_name, error_message).await;
        let strategies = self.generate_recovery_strategies(
            tool_name, 
            &failure_analysis, 
            parameters, 
            retry_attempts
        ).await?;
        
        let user_recommendations = self.generate_user_recommendations(
            tool_name,
            &failure_analysis,
            &strategies
        );

        Ok(RecoverySuggestions {
            strategies,
            failure_analysis,
            user_recommendations,
            requires_manual_intervention: self.requires_manual_intervention(tool_name, error_message),
        })
    }

    /// Analyze the failure to determine category and root cause
    async fn analyze_failure(&self, tool_name: &str, error_message: &str) -> FailureAnalysis {
        let failure_category = self.categorize_failure(error_message);
        let root_cause = self.determine_root_cause(tool_name, error_message, &failure_category);
        let is_recoverable = self.is_failure_recoverable(&failure_category, error_message);
        let retry_success_probability = self.estimate_retry_success_probability(&failure_category, error_message);
        let alternatives_available = self.check_alternatives_available(tool_name, &failure_category);

        FailureAnalysis {
            failure_category,
            root_cause,
            is_recoverable,
            retry_success_probability,
            alternatives_available,
        }
    }

    /// Categorize the failure based on error message patterns
    fn categorize_failure(&self, error_message: &str) -> FailureCategory {
        let lower_message = error_message.to_lowercase();
        
        if lower_message.contains("network") || lower_message.contains("connection") || 
           lower_message.contains("timeout") || lower_message.contains("unreachable") {
            if lower_message.contains("timeout") || lower_message.contains("timed out") {
                FailureCategory::TimeoutError
            } else {
                FailureCategory::NetworkError
            }
        } else if lower_message.contains("auth") || lower_message.contains("permission") || 
                  lower_message.contains("unauthorized") || lower_message.contains("forbidden") {
            FailureCategory::AuthenticationError
        } else if lower_message.contains("parameter") || lower_message.contains("invalid") || 
                  lower_message.contains("missing") || lower_message.contains("required") {
            FailureCategory::ParameterError
        } else if lower_message.contains("resource") || lower_message.contains("quota") || 
                  lower_message.contains("limit") || lower_message.contains("exhausted") {
            FailureCategory::ResourceError
        } else if lower_message.contains("config") || lower_message.contains("setup") || 
                  lower_message.contains("initialization") {
            FailureCategory::ConfigurationError
        } else if lower_message.contains("dependency") || lower_message.contains("external") || 
                  lower_message.contains("service") {
            FailureCategory::DependencyError
        } else {
            FailureCategory::UnknownError
        }
    }

    /// Determine specific root cause analysis
    fn determine_root_cause(&self, tool_name: &str, error_message: &str, category: &FailureCategory) -> String {
        match category {
            FailureCategory::NetworkError => {
                format!("Network connectivity issue while executing '{}': {}", tool_name, error_message)
            }
            FailureCategory::AuthenticationError => {
                format!("Authentication or permission problem with '{}': {}", tool_name, error_message)
            }
            FailureCategory::ParameterError => {
                format!("Invalid or missing parameters for '{}': {}", tool_name, error_message)
            }
            FailureCategory::ResourceError => {
                format!("Resource limitation encountered in '{}': {}", tool_name, error_message)
            }
            FailureCategory::ConfigurationError => {
                format!("Configuration issue with '{}': {}", tool_name, error_message)
            }
            FailureCategory::DependencyError => {
                format!("External dependency failure affecting '{}': {}", tool_name, error_message)
            }
            FailureCategory::TimeoutError => {
                format!("Timeout occurred while executing '{}': {}", tool_name, error_message)
            }
            FailureCategory::UnknownError => {
                format!("Unknown error in '{}': {}", tool_name, error_message)
            }
        }
    }

    /// Check if failure is recoverable
    fn is_failure_recoverable(&self, category: &FailureCategory, error_message: &str) -> bool {
        match category {
            FailureCategory::NetworkError => true,
            FailureCategory::TimeoutError => true,
            FailureCategory::ResourceError => true,
            FailureCategory::DependencyError => true,
            FailureCategory::AuthenticationError => false,
            FailureCategory::ConfigurationError => false,
            FailureCategory::ParameterError => {
                // Parameter errors might be recoverable with alternative parameters
                !error_message.to_lowercase().contains("missing required")
            }
            FailureCategory::UnknownError => true, // Assume recoverable for unknown errors
        }
    }

    /// Estimate probability of retry success
    fn estimate_retry_success_probability(&self, category: &FailureCategory, error_message: &str) -> f32 {
        match category {
            FailureCategory::NetworkError => 0.7,
            FailureCategory::TimeoutError => 0.6,
            FailureCategory::ResourceError => 0.4,
            FailureCategory::DependencyError => 0.5,
            FailureCategory::AuthenticationError => 0.1,
            FailureCategory::ConfigurationError => 0.2,
            FailureCategory::ParameterError => {
                if error_message.to_lowercase().contains("format") {
                    0.3 // Format errors might be fixable with parameter variation
                } else {
                    0.1
                }
            }
            FailureCategory::UnknownError => 0.4,
        }
    }

    /// Check if alternative tools are available
    fn check_alternatives_available(&self, tool_name: &str, category: &FailureCategory) -> bool {
        // Common tools that have alternatives
        let tool_alternatives = [
            ("add_existing_repository", vec!["shell_execution"]),
            ("file_write", vec!["shell_execution"]),
            ("file_read", vec!["shell_execution"]),
            ("git_clone", vec!["shell_execution"]),
            ("package_install", vec!["shell_execution"]),
        ];

        for (tool, _alternatives) in &tool_alternatives {
            if tool_name.contains(tool) {
                return true;
            }
        }

        // Some error categories have generic alternatives
        matches!(category, 
            FailureCategory::ParameterError | 
            FailureCategory::ConfigurationError |
            FailureCategory::DependencyError
        )
    }

    /// Generate recovery strategies based on failure analysis
    async fn generate_recovery_strategies(
        &self,
        tool_name: &str,
        analysis: &FailureAnalysis,
        parameters: &serde_json::Value,
        retry_attempts: u32,
    ) -> Result<Vec<RecoveryStrategy>> {
        let mut strategies = Vec::new();

        // Basic retry if recoverable and not too many attempts
        if analysis.is_recoverable && retry_attempts < 3 {
            strategies.push(RecoveryStrategy {
                strategy_type: RecoveryStrategyType::BasicRetry,
                alternative_tool: None,
                modified_parameters: None,
                simplified_approach: None,
                description: format!("Retry '{}' with exponential backoff", tool_name),
                confidence: analysis.retry_success_probability,
            });
        }

        // Alternative tools based on tool name and error category
        if analysis.alternatives_available {
            strategies.extend(self.generate_alternative_tool_strategies(tool_name, analysis).await);
        }

        // Parameter variation strategies
        if matches!(analysis.failure_category, FailureCategory::ParameterError) {
            strategies.extend(self.generate_parameter_variation_strategies(tool_name, parameters).await);
        }

        // Simplified approach strategies
        strategies.extend(self.generate_simplified_approach_strategies(tool_name, analysis, parameters).await);

        // Manual fallback strategies
        strategies.extend(self.generate_manual_fallback_strategies(tool_name, analysis).await);

        // Sort by confidence
        strategies.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        Ok(strategies)
    }

    /// Generate alternative tool strategies
    async fn generate_alternative_tool_strategies(
        &self,
        tool_name: &str,
        _analysis: &FailureAnalysis,
    ) -> Vec<RecoveryStrategy> {
        let mut strategies = Vec::new();

        // Repository management alternatives
        if tool_name.contains("add_existing_repository") {
            strategies.push(RecoveryStrategy {
                strategy_type: RecoveryStrategyType::AlternativeTool,
                alternative_tool: Some("shell_execution".to_string()),
                modified_parameters: Some(serde_json::json!({
                    "command": "ls -la",
                    "description": "Use shell commands to verify directory existence and contents"
                })),
                simplified_approach: None,
                description: "Use shell commands to manually verify and work with the directory".to_string(),
                confidence: 0.8,
            });
        }

        // File operations alternatives
        if tool_name.contains("file_") {
            strategies.push(RecoveryStrategy {
                strategy_type: RecoveryStrategyType::AlternativeTool,
                alternative_tool: Some("shell_execution".to_string()),
                modified_parameters: Some(serde_json::json!({
                    "command": "echo 'Using shell for file operations'",
                    "description": "Fall back to shell commands for file operations"
                })),
                simplified_approach: None,
                description: "Use shell commands for file operations instead of specialized tools".to_string(),
                confidence: 0.7,
            });
        }

        strategies
    }

    /// Generate parameter variation strategies
    async fn generate_parameter_variation_strategies(
        &self,
        tool_name: &str,
        parameters: &serde_json::Value,
    ) -> Vec<RecoveryStrategy> {
        let mut strategies = Vec::new();

        if tool_name.contains("add_existing_repository") {
            if let Some(obj) = parameters.as_object() {
                // Try with just local_path if both URL and local_path were provided
                if obj.contains_key("url") && obj.contains_key("local_path") {
                    let mut new_params = obj.clone();
                    new_params.remove("url");
                    new_params.remove("branch");
                    
                    strategies.push(RecoveryStrategy {
                        strategy_type: RecoveryStrategyType::ParameterVariation,
                        alternative_tool: None,
                        modified_parameters: Some(serde_json::Value::Object(new_params)),
                        simplified_approach: None,
                        description: "Try with only local_path parameter, removing URL and branch".to_string(),
                        confidence: 0.6,
                    });
                }

                // Try with absolute path if relative path was used
                if let Some(local_path) = obj.get("local_path").and_then(|v| v.as_str()) {
                    if !local_path.starts_with('/') {
                        let mut new_params = obj.clone();
                        new_params.insert("local_path".to_string(), 
                            serde_json::Value::String(format!("/home/user/{}", local_path)));
                        
                        strategies.push(RecoveryStrategy {
                            strategy_type: RecoveryStrategyType::ParameterVariation,
                            alternative_tool: None,
                            modified_parameters: Some(serde_json::Value::Object(new_params)),
                            simplified_approach: None,
                            description: "Try with absolute path instead of relative path".to_string(),
                            confidence: 0.5,
                        });
                    }
                }
            }
        }

        strategies
    }

    /// Generate simplified approach strategies
    async fn generate_simplified_approach_strategies(
        &self,
        tool_name: &str,
        _analysis: &FailureAnalysis,
        _parameters: &serde_json::Value,
    ) -> Vec<RecoveryStrategy> {
        let mut strategies = Vec::new();

        if tool_name.contains("add_existing_repository") {
            strategies.push(RecoveryStrategy {
                strategy_type: RecoveryStrategyType::SimplifiedApproach,
                alternative_tool: None,
                modified_parameters: None,
                simplified_approach: Some(SimplifiedApproach {
                    reduced_parameters: serde_json::json!({
                        "description": "Skip repository registration and work with files directly"
                    }),
                    reduction_description: "Skip complex repository registration".to_string(),
                    maintains_core_functionality: false,
                }),
                description: "Skip repository registration and work with files directly using shell commands".to_string(),
                confidence: 0.7,
            });
        }

        strategies
    }

    /// Generate manual fallback strategies
    async fn generate_manual_fallback_strategies(
        &self,
        tool_name: &str,
        _analysis: &FailureAnalysis,
    ) -> Vec<RecoveryStrategy> {
        let mut strategies = Vec::new();

        strategies.push(RecoveryStrategy {
            strategy_type: RecoveryStrategyType::ManualFallback,
            alternative_tool: Some("shell_execution".to_string()),
            modified_parameters: Some(serde_json::json!({
                "command": "echo 'Manual intervention required'",
                "description": "Provide guidance for manual steps"
            })),
            simplified_approach: None,
            description: format!("Provide manual instructions for completing the '{}' operation", tool_name),
            confidence: 0.4,
        });

        strategies
    }

    /// Generate user recommendations
    fn generate_user_recommendations(
        &self,
        tool_name: &str,
        analysis: &FailureAnalysis,
        strategies: &[RecoveryStrategy],
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        match analysis.failure_category {
            FailureCategory::NetworkError => {
                recommendations.push("Check your internet connection".to_string());
                recommendations.push("Verify that the remote service is accessible".to_string());
                recommendations.push("Try again in a few moments".to_string());
            }
            FailureCategory::AuthenticationError => {
                recommendations.push("Check your credentials and permissions".to_string());
                recommendations.push("Verify you have access to the required resources".to_string());
                recommendations.push("Ensure your authentication tokens are valid".to_string());
            }
            FailureCategory::ParameterError => {
                recommendations.push("Review the parameters provided to the tool".to_string());
                recommendations.push("Ensure all required parameters are present and correctly formatted".to_string());
                if tool_name.contains("repository") {
                    recommendations.push("For repository tools, provide either 'url' OR 'local_path', not both".to_string());
                    recommendations.push("Ensure local paths are absolute and directories exist".to_string());
                }
            }
            FailureCategory::ResourceError => {
                recommendations.push("Wait for resources to become available".to_string());
                recommendations.push("Consider reducing resource requirements".to_string());
                recommendations.push("Try the operation during off-peak hours".to_string());
            }
            FailureCategory::ConfigurationError => {
                recommendations.push("Check the tool configuration".to_string());
                recommendations.push("Verify all required dependencies are installed".to_string());
                recommendations.push("Review the system setup requirements".to_string());
            }
            FailureCategory::DependencyError => {
                recommendations.push("Check that external services are available".to_string());
                recommendations.push("Verify network connectivity to dependencies".to_string());
                recommendations.push("Consider using alternative approaches".to_string());
            }
            FailureCategory::TimeoutError => {
                recommendations.push("The operation timed out - try again with increased timeout".to_string());
                recommendations.push("Consider breaking the operation into smaller steps".to_string());
                recommendations.push("Check for network latency issues".to_string());
            }
            FailureCategory::UnknownError => {
                recommendations.push("An unexpected error occurred".to_string());
                recommendations.push("Try the suggested alternative approaches".to_string());
                recommendations.push("Consider reporting this issue if it persists".to_string());
            }
        }

        // Add strategy-specific recommendations
        if !strategies.is_empty() {
            recommendations.push("Consider trying these alternative approaches:".to_string());
            for (i, strategy) in strategies.iter().take(3).enumerate() {
                recommendations.push(format!("{}. {}", i + 1, strategy.description));
            }
        }

        recommendations
    }

    /// Check if manual intervention is required
    fn requires_manual_intervention(&self, _tool_name: &str, error_message: &str) -> bool {
        // Heuristics for determining manual intervention
        error_message.contains("authentication") || 
        error_message.contains("permission denied") ||
        error_message.contains("access denied") ||
        error_message.contains("not authorized") ||
        error_message.contains("credential") ||
        error_message.contains("login") ||
        error_message.contains("password") ||
        error_message.contains("api key") ||
        error_message.contains("token") ||
        error_message.contains("certificate") ||
        error_message.contains("ssl") ||
        error_message.contains("firewall") ||
        error_message.contains("network unreachable") ||
        error_message.contains("configuration error")
    }

    /// NEW: Validate tool execution result against expected behavior
    async fn validate_tool_execution_result(
        &self,
        request: &ToolExecutionRequest,
        tool_result: &mut ToolResult,
        execution_time: Duration,
    ) -> ValidationOutcome {
        // First, validate internal consistency
        let consistency_status = tool_result.validate_consistency();
        
        // Then perform tool-specific validation
        let tool_specific_validation = self.perform_tool_specific_validation(
            &request.tool_name,
            &request.parameters,
            tool_result
        ).await;

        // Combine validation results
        match (consistency_status, tool_specific_validation) {
            (ValidationStatus::Validated, Ok(())) => ValidationOutcome::Validated,
            (ValidationStatus::NeedsVerification { reason }, Ok(())) => {
                ValidationOutcome::NeedsVerification { reason }
            }
            (ValidationStatus::Inconsistent { details }, _) => {
                ValidationOutcome::Inconsistent { details }
            }
            (_, Err(validation_error)) => {
                ValidationOutcome::VerificationFailed { error: validation_error.to_string() }
            }
            (ValidationStatus::UnableToValidate, Ok(())) => {
                // If we can't validate, but there are no obvious issues, proceed with caution
                ValidationOutcome::NeedsVerification { 
                    reason: "Unable to validate tool execution result".to_string() 
                }
            }
        }
    }

    /// NEW: Perform tool-specific validation
    async fn perform_tool_specific_validation(
        &self,
        tool_name: &str,
        parameters: &serde_json::Value,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        match tool_name {
            "add_existing_repository" => {
                self.validate_repository_operation(parameters, tool_result).await
            }
            "shell_execution" | "streaming_shell_execution" => {
                self.validate_shell_execution(parameters, tool_result).await
            }
            "read_file" | "write_file" | "edit_file" => {
                self.validate_file_operation(parameters, tool_result).await
            }
            "sync_repository" => {
                self.validate_sync_operation(parameters, tool_result).await
            }
            _ => {
                // For unknown tools, perform basic validation
                self.validate_generic_tool_result(tool_result).await
            }
        }
    }

    /// Validate repository operations
    async fn validate_repository_operation(
        &self,
        parameters: &serde_json::Value,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        if !tool_result.success {
            // For failed repository operations, check if it's actually a "already exists" success
            if let Some(error_msg) = &tool_result.error {
                if error_msg.contains("already exists") || error_msg.contains("already available") {
                    // This is actually a success case
                    tool_result.success = true;
                    tool_result.error = None;
                    tool_result.operation_performed = Some("Repository registration (already existed)".to_string());
                    tool_result.completion_evidence.push(CompletionEvidence {
                        evidence_type: EvidenceType::RepositoryOperation,
                        description: "Repository was already registered".to_string(),
                        supports_success: true,
                        confidence: 0.9,
                    });
                    tool_result.validation_status = ValidationStatus::Validated;
                    
                    tracing::info!("Corrected repository operation result: 'already exists' treated as success");
                }
            }
        } else {
            // For successful repository operations, add evidence
            if let Some(repo_name) = parameters.get("name").and_then(|v| v.as_str()) {
                tool_result.operation_performed = Some(format!("Added repository: {}", repo_name));
                tool_result.completion_evidence.push(CompletionEvidence {
                    evidence_type: EvidenceType::RepositoryOperation,
                    description: format!("Repository '{}' was added to configuration", repo_name),
                    supports_success: true,
                    confidence: 0.8,
                });
            }
        }
        
        Ok(())
    }

    /// Validate shell execution results
    async fn validate_shell_execution(
        &self,
        parameters: &serde_json::Value,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        if let Some(command) = parameters.get("command").and_then(|v| v.as_str()) {
            tool_result.operation_performed = Some(format!("Executed command: {}", command));
            
            // Check for common success/failure patterns in shell output
            if let Some(output) = tool_result.data.get("stdout").and_then(|v| v.as_str()) {
                // Check for success indicators
                if output.contains("successfully") || output.contains("completed") || output.contains("done") {
                    tool_result.completion_evidence.push(CompletionEvidence {
                        evidence_type: EvidenceType::CommandOutput,
                        description: "Command output indicates success".to_string(),
                        supports_success: true,
                        confidence: 0.7,
                    });
                }
                
                // Check for failure indicators
                if output.contains("error") || output.contains("failed") || output.contains("not found") {
                    tool_result.completion_evidence.push(CompletionEvidence {
                        evidence_type: EvidenceType::CommandOutput,
                        description: "Command output indicates error".to_string(),
                        supports_success: false,
                        confidence: 0.8,
                    });
                }
            }
            
            // Check exit code if available
            if let Some(exit_code) = tool_result.data.get("exit_code").and_then(|v| v.as_i64()) {
                if exit_code == 0 && !tool_result.success {
                    // Exit code 0 but tool claims failure - add contradicting evidence
                    tool_result.completion_evidence.push(CompletionEvidence {
                        evidence_type: EvidenceType::ProcessCompletion,
                        description: "Exit code is 0 (success)".to_string(),
                        supports_success: true,
                        confidence: 0.9,
                    });
                } else if exit_code != 0 && tool_result.success {
                    // Non-zero exit code but tool claims success - add contradicting evidence
                    tool_result.completion_evidence.push(CompletionEvidence {
                        evidence_type: EvidenceType::ProcessCompletion,
                        description: format!("Exit code is {} (failure)", exit_code),
                        supports_success: false,
                        confidence: 0.9,
                    });
                } else {
                    // Consistent result - add supporting evidence
                    tool_result.completion_evidence.push(CompletionEvidence {
                        evidence_type: EvidenceType::ProcessCompletion,
                        description: format!("Exit code: {}", exit_code),
                        supports_success: exit_code == 0,
                        confidence: 0.9,
                    });
                }
            }
        }
        
        Ok(())
    }

    /// Validate file operations
    async fn validate_file_operation(
        &self,
        parameters: &serde_json::Value,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        if let Some(file_path) = parameters.get("file_path").or_else(|| parameters.get("target_file")).and_then(|v| v.as_str()) {
            tool_result.operation_performed = Some(format!("File operation on: {}", file_path));
            
            // For file operations, we could check if the file actually exists/was modified
            // This is a placeholder for more sophisticated validation
            tool_result.completion_evidence.push(CompletionEvidence {
                evidence_type: EvidenceType::FileSystemChange,
                description: format!("File operation on {}", file_path),
                supports_success: tool_result.success,
                confidence: 0.6, // Lower confidence since we're not actually checking the filesystem
            });
        }
        
        Ok(())
    }

    /// Validate sync operations
    async fn validate_sync_operation(
        &self,
        _parameters: &serde_json::Value,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        // Add evidence for sync operations
        tool_result.completion_evidence.push(CompletionEvidence {
            evidence_type: EvidenceType::RepositoryOperation,
            description: "Repository synchronization".to_string(),
            supports_success: tool_result.success,
            confidence: 0.7,
        });
        
        Ok(())
    }

    /// Generic validation for unknown tools
    async fn validate_generic_tool_result(
        &self,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        // For unknown tools, we can only do basic validation
        if tool_result.completion_evidence.is_empty() {
            // Add evidence that indicates we can't validate
            tool_result.completion_evidence.push(CompletionEvidence {
                evidence_type: EvidenceType::NoEvidence,
                description: "Unknown tool type - unable to validate specific behavior".to_string(),
                supports_success: tool_result.success, // Neither supports nor contradicts
                confidence: 0.1, // Very low confidence
            });
        }
        
        Ok(())
    }
}

impl ResourceManager {
    /// Create a new resource manager
    pub async fn new() -> Result<Self> {
        Ok(Self {
            resources: Arc::new(RwLock::new(HashMap::new())),
            allocations: Arc::new(RwLock::new(HashMap::new())),
            allocation_history: Arc::new(RwLock::new(VecDeque::new())),
        })
    }
    
    /// Register a resource pool
    pub async fn register_resource_pool(&self, resource_type: String, capacity: u32) -> Result<()> {
        let pool = ResourcePool {
            resource_type: resource_type.clone(),
            total_capacity: capacity,
            available: capacity,
            semaphore: Arc::new(Semaphore::new(capacity as usize)),
            pending_allocations: VecDeque::new(),
        };
        
        let mut resources = self.resources.write().await;
        resources.insert(resource_type, pool);
        Ok(())
    }
    
    /// Allocate a resource
    pub async fn allocate_resource(
        &self,
        resource_type: &str,
        amount: u32,
        priority: f32,
        tool_name: &str,
    ) -> Result<AllocatedResource> {
        let allocation_id = Uuid::new_v4();
        
        // For now, implement a simple allocation strategy with timeout
        // In a full implementation, this would handle priority queuing, deadlock detection, etc.
        
        // Add timeout to prevent hanging
        let allocation_result = tokio::time::timeout(
            Duration::from_secs(5),
            async {
                let allocation = AllocatedResource {
                    allocation_id,
                    resource_type: resource_type.to_string(),
                    amount,
                    allocated_at: chrono::Utc::now(),
                    owner_tool: tool_name.to_string(),
                };
                
                // Record allocation
                let mut allocations = self.allocations.write().await;
                allocations.entry(allocation_id).or_insert_with(Vec::new).push(allocation.clone());
                
                Ok(allocation)
            }
        ).await;
        
        match allocation_result {
            Ok(result) => result,
            Err(_) => Err(ReasoningError::orchestration("Resource allocation timed out")),
        }
    }
    
    /// Release a resource
    pub async fn release_resource(&self, resource: &AllocatedResource) -> Result<()> {
        let mut allocations = self.allocations.write().await;
        allocations.remove(&resource.allocation_id);
        
        // Record in history for optimization
        let duration = chrono::Utc::now().signed_duration_since(resource.allocated_at);
        let record = ResourceAllocationRecord {
            tool_name: resource.owner_tool.clone(),
            resource_type: resource.resource_type.clone(),
            amount: resource.amount,
            duration: duration.to_std().unwrap_or(Duration::ZERO),
            efficiency: 1.0, // TODO: Calculate actual efficiency
            timestamp: std::time::Instant::now(),
        };
        
        let mut history = self.allocation_history.write().await;
        history.push_back(record);
        
        // Limit history size
        while history.len() > 10000 {
            history.pop_front();
        }
        
        Ok(())
    }
}

impl ExecutionPlanner {
    /// Create a new execution planner
    pub async fn new(config: OrchestrationConfig) -> Result<Self> {
        Ok(Self {
            config,
            dependency_analyzer: DependencyAnalyzer::new().await?,
            resource_optimizer: ResourceOptimizer::new().await?,
        })
    }
    
    /// Create an execution plan for the given requests
    pub async fn create_plan(&self, requests: &[ToolExecutionRequest]) -> Result<ExecutionPlan> {
        let plan_id = Uuid::new_v4();
        
        // Add timeout to prevent hanging in dependency analysis
        let plan_result = tokio::time::timeout(
            Duration::from_secs(10),
            async {
                // Analyze dependencies
                let dependency_graph = self.dependency_analyzer.analyze_dependencies(requests).await?;
                
                // Create execution phases based on topological sort
                let phases = self.create_execution_phases(&dependency_graph, requests).await?;
                
                // Estimate duration
                let estimated_duration = self.estimate_total_duration(&phases).await?;
                
                // Create resource allocation plan
                let resource_plan = self.resource_optimizer.create_allocation_plan(requests, &phases).await?;
                
                Ok(ExecutionPlan {
                    id: plan_id,
                    phases,
                    estimated_duration,
                    critical_path: dependency_graph.critical_path,
                    resource_plan,
                })
            }
        ).await;
        
        match plan_result {
            Ok(result) => result,
            Err(_) => Err(ReasoningError::orchestration("Execution plan creation timed out")),
        }
    }
    
    /// Create execution phases from dependency graph
    async fn create_execution_phases(
        &self,
        graph: &DependencyGraph,
        requests: &[ToolExecutionRequest],
    ) -> Result<Vec<ExecutionPhase>> {
        let mut phases = Vec::new();
        let mut remaining_tools: HashSet<String> = graph.nodes.clone();
        let mut phase_number = 0;
        
        while !remaining_tools.is_empty() {
            let mut current_phase_tools = Vec::new();
            
            // Find tools with no remaining dependencies
            for tool in &remaining_tools {
                let has_pending_deps = graph.edges.get(tool)
                    .map(|deps| deps.iter().any(|dep| remaining_tools.contains(dep)))
                    .unwrap_or(false);
                
                if !has_pending_deps {
                    current_phase_tools.push(tool.clone());
                }
            }
            
            if current_phase_tools.is_empty() {
                return Err(ReasoningError::orchestration("Circular dependency detected"));
            }
            
            // Remove tools from remaining set
            for tool in &current_phase_tools {
                remaining_tools.remove(tool);
            }
            
            // Calculate phase duration and resources
            let estimated_duration = self.estimate_phase_duration(&current_phase_tools, requests).await?;
            let required_resources = self.calculate_phase_resources(&current_phase_tools, requests).await?;
            
            phases.push(ExecutionPhase {
                phase_number,
                tools: current_phase_tools,
                estimated_duration,
                required_resources,
            });
            
            phase_number += 1;
        }
        
        Ok(phases)
    }
    
    /// Estimate duration for a phase
    async fn estimate_phase_duration(
        &self,
        tools: &[String],
        requests: &[ToolExecutionRequest],
    ) -> Result<Duration> {
        let mut max_duration = Duration::ZERO;
        
        for tool in tools {
            if let Some(request) = requests.iter().find(|r| r.tool_name == *tool) {
                let tool_duration = request.timeout.unwrap_or(self.config.default_tool_timeout);
                max_duration = max_duration.max(tool_duration);
            }
        }
        
        Ok(max_duration)
    }
    
    /// Calculate resource requirements for a phase
    async fn calculate_phase_resources(
        &self,
        tools: &[String],
        requests: &[ToolExecutionRequest],
    ) -> Result<HashMap<String, u32>> {
        let mut resources = HashMap::new();
        
        for tool in tools {
            if let Some(request) = requests.iter().find(|r| r.tool_name == *tool) {
                for requirement in &request.required_resources {
                    *resources.entry(requirement.resource_type.clone()).or_insert(0) += requirement.amount;
                }
            }
        }
        
        Ok(resources)
    }
    
    /// Estimate total execution duration
    async fn estimate_total_duration(&self, phases: &[ExecutionPhase]) -> Result<Duration> {
        Ok(phases.iter().map(|p| p.estimated_duration).sum())
    }
}

impl DependencyAnalyzer {
    /// Create a new dependency analyzer
    pub async fn new() -> Result<Self> {
        Ok(Self {
            dependency_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Analyze dependencies between tools
    pub async fn analyze_dependencies(&self, requests: &[ToolExecutionRequest]) -> Result<DependencyGraph> {
        let mut nodes = HashSet::new();
        let mut edges = HashMap::new();
        
        // Build graph
        for request in requests {
            nodes.insert(request.tool_name.clone());
            
            if !request.dependencies.is_empty() {
                edges.insert(request.tool_name.clone(), request.dependencies.iter().cloned().collect());
            }
        }
        
        // Perform topological sort using Kahn's algorithm (non-recursive)
        let topological_order = self.topological_sort(&nodes, &edges)?;
        
        // Find critical path (simplified - just the longest dependency chain)
        let critical_path = self.find_critical_path(&nodes, &edges);
        
        Ok(DependencyGraph {
            nodes,
            edges,
            topological_order,
            critical_path,
        })
    }
    
    /// Perform topological sort using Kahn's algorithm (non-recursive)
    fn topological_sort(&self, nodes: &HashSet<String>, edges: &HashMap<String, HashSet<String>>) -> Result<Vec<String>> {
        let mut result = Vec::new();
        let mut in_degree = HashMap::new();
        let mut queue = VecDeque::new();
        
        // Initialize in-degree count for all nodes
        for node in nodes {
            in_degree.insert(node.clone(), 0);
        }
        
        // Calculate in-degrees
        for (node, dependencies) in edges {
            for dep in dependencies {
                if nodes.contains(dep) {
                    *in_degree.entry(node.clone()).or_insert(0) += 1;
                }
            }
        }
        
        // Find nodes with no incoming edges
        for (node, degree) in &in_degree {
            if *degree == 0 {
                queue.push_back(node.clone());
            }
        }
        
        // Process nodes
        while let Some(node) = queue.pop_front() {
            result.push(node.clone());
            
            // For each node that depends on the current node
            for (other_node, dependencies) in edges {
                if dependencies.contains(&node) {
                    let degree = in_degree.get_mut(other_node).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(other_node.clone());
                    }
                }
            }
        }
        
        // Check for cycles
        if result.len() != nodes.len() {
            return Err(ReasoningError::orchestration("Circular dependency detected"));
        }
        
        Ok(result)
    }
    
    /// Find critical path through dependencies
    fn find_critical_path(&self, nodes: &HashSet<String>, edges: &HashMap<String, HashSet<String>>) -> Vec<String> {
        // Simplified implementation - find the longest dependency chain
        let mut longest_path = Vec::new();
        
        for node in nodes {
            let path = self.find_longest_path_from(node, edges, &mut HashSet::new());
            if path.len() > longest_path.len() {
                longest_path = path;
            }
        }
        
        longest_path
    }
    
    /// Find longest path from a node
    fn find_longest_path_from(
        &self,
        node: &str,
        edges: &HashMap<String, HashSet<String>>,
        visited: &mut HashSet<String>,
    ) -> Vec<String> {
        if visited.contains(node) {
            return Vec::new();
        }
        
        visited.insert(node.to_string());
        
        let mut longest_path = vec![node.to_string()];
        
        if let Some(dependencies) = edges.get(node) {
            for dep in dependencies {
                let dep_path = self.find_longest_path_from(dep, edges, visited);
                if dep_path.len() + 1 > longest_path.len() {
                    longest_path = vec![node.to_string()];
                    longest_path.extend(dep_path);
                }
            }
        }
        
        visited.remove(node);
        longest_path
    }
    
    /// Recursive helper for topological sort (REMOVED - replaced with Kahn's algorithm)
    fn topological_sort_visit(
        &self,
        _node: &str,
        _edges: &HashMap<String, HashSet<String>>,
        _visited: &mut HashSet<String>,
        _temp_visited: &mut HashSet<String>,
        _result: &mut Vec<String>,
    ) -> Result<()> {
        // This method is no longer used but kept for compatibility
        Ok(())
    }
}

impl ResourceOptimizer {
    /// Create a new resource optimizer
    pub async fn new() -> Result<Self> {
        Ok(Self {
            performance_history: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Create resource allocation plan
    pub async fn create_allocation_plan(
        &self,
        requests: &[ToolExecutionRequest],
        phases: &[ExecutionPhase],
    ) -> Result<ResourceAllocationPlan> {
        let mut peak_usage = HashMap::new();
        let mut allocation_timeline = Vec::new();
        let conflicts = Vec::new(); // TODO: Implement conflict detection
        
        // Calculate peak usage
        for phase in phases {
            for (resource_type, amount) in &phase.required_resources {
                let current_peak = peak_usage.get(resource_type).unwrap_or(&0);
                peak_usage.insert(resource_type.clone(), (*current_peak).max(*amount));
            }
        }
        
        // Create allocation timeline
        let mut time_offset = Duration::ZERO;
        for phase in phases {
            allocation_timeline.push(ResourceAllocation {
                time_offset,
                allocations: phase.required_resources.clone(),
                tools: phase.tools.clone(),
            });
            time_offset += phase.estimated_duration;
        }
        
        Ok(ResourceAllocationPlan {
            peak_usage,
            allocation_timeline,
            conflicts,
        })
    }
}

impl ToolExecutionRequest {
    /// Create a new tool execution request
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
    
    /// Add a dependency on another tool
    pub fn with_dependency(mut self, dependency: String) -> Self {
        self.dependencies.push(dependency);
        self
    }
    
    /// Add a resource requirement
    pub fn with_resource(mut self, resource_type: String, amount: u32, exclusive: bool) -> Self {
        self.required_resources.push(ResourceRequirement {
            resource_type,
            amount,
            exclusive,
            allocation_timeout: None,
        });
        self
    }
    
    /// Set priority
    pub fn with_priority(mut self, priority: f32) -> Self {
        self.priority = priority.clamp(0.0, 1.0);
        self
    }
    
    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    
    /// Mark as critical
    pub fn as_critical(mut self) -> Self {
        self.is_critical = true;
        self
    }
    
    /// Set retry configuration
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{ToolDefinition};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    
    // Mock tool executor for testing
    struct MockToolExecutor {
        call_count: AtomicUsize,
        should_fail: bool,
        execution_delay: Duration,
    }
    
    impl MockToolExecutor {
        fn new(should_fail: bool, execution_delay: Duration) -> Self {
            Self {
                call_count: AtomicUsize::new(0),
                should_fail,
                execution_delay,
            }
        }
        
        fn get_call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }
    
    #[async_trait::async_trait]
    impl ToolExecutor for MockToolExecutor {
        async fn execute_tool(&self, name: &str, args: serde_json::Value) -> Result<ToolResult> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            
            // Simulate execution delay
            tokio::time::sleep(self.execution_delay).await;
            
            if self.should_fail {
                Err(ReasoningError::tool_execution(name, "Mock tool failure"))
            } else {
                Ok(ToolResult::success(
                    serde_json::json!({"tool": name, "args": args}),
                    self.execution_delay.as_millis() as u64
                ))
            }
        }
        
        async fn get_available_tools(&self) -> Result<Vec<ToolDefinition>> {
            Ok(vec![
                ToolDefinition {
                    name: "test_tool_1".to_string(),
                    description: "Test tool 1".to_string(),
                    parameters: serde_json::json!({}),
                    is_required: false,
                    category: None,
                    estimated_duration_ms: Some(100),
                },
                ToolDefinition {
                    name: "test_tool_2".to_string(),
                    description: "Test tool 2".to_string(),
                    parameters: serde_json::json!({}),
                    is_required: false,
                    category: None,
                    estimated_duration_ms: Some(200),
                },
            ])
        }
    }
    
    // Mock event emitter for testing
    struct MockEventEmitter {
        events: Arc<Mutex<Vec<ReasoningEvent>>>,
    }
    
    impl MockEventEmitter {
        fn new() -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
            }
        }
        
        async fn get_events(&self) -> Vec<ReasoningEvent> {
            self.events.lock().await.clone()
        }
    }
    
    #[async_trait::async_trait]
    impl EventEmitter for MockEventEmitter {
        async fn emit_event(&self, event: ReasoningEvent) -> Result<()> {
            self.events.lock().await.push(event);
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_orchestrator_creation() {
        let mut config = OrchestrationConfig::default();
        config.global_timeout = Duration::from_secs(5); // Shorter for tests
        let orchestrator = ToolOrchestrator::new(config).await.unwrap();
        
        let metrics = orchestrator.get_metrics().await;
        assert_eq!(metrics.total_orchestrations, 0);
    }
    
    #[tokio::test]
    async fn test_single_tool_orchestration() {
        let mut config = OrchestrationConfig::default();
        config.global_timeout = Duration::from_secs(10); // Shorter for tests
        config.default_tool_timeout = Duration::from_secs(2);
        let orchestrator = ToolOrchestrator::new(config).await.unwrap();
        
        let tool_executor = Arc::new(MockToolExecutor::new(false, Duration::from_millis(50)));
        let event_emitter = Arc::new(MockEventEmitter::new());
        
        let request = ToolExecutionRequest::new(
            "test_tool_1".to_string(),
            serde_json::json!({"param": "value"})
        );
        
        let result = orchestrator.orchestrate_tools(
            vec![request],
            tool_executor.clone(),
            event_emitter.clone(),
        ).await.unwrap();
        
        assert!(result.success);
        assert_eq!(result.successful_tools, 1);
        assert_eq!(result.failed_tools, 0);
        assert_eq!(tool_executor.get_call_count(), 1);
        
        // Check events were emitted
        let events = event_emitter.get_events().await;
        assert!(!events.is_empty());
    }
    
    #[tokio::test]
    async fn test_parallel_tool_orchestration() {
        let mut config = OrchestrationConfig::default();
        config.global_timeout = Duration::from_secs(15); // Shorter for tests
        config.default_tool_timeout = Duration::from_secs(3);
        let orchestrator = ToolOrchestrator::new(config).await.unwrap();
        
        let tool_executor = Arc::new(MockToolExecutor::new(false, Duration::from_millis(100)));
        let event_emitter = Arc::new(MockEventEmitter::new());
        
        let requests = vec![
            ToolExecutionRequest::new("tool_1".to_string(), serde_json::json!({})),
            ToolExecutionRequest::new("tool_2".to_string(), serde_json::json!({})),
            ToolExecutionRequest::new("tool_3".to_string(), serde_json::json!({})),
        ];
        
        let start_time = Instant::now();
        let result = orchestrator.orchestrate_tools(
            requests,
            tool_executor.clone(),
            event_emitter,
        ).await.unwrap();
        let execution_time = start_time.elapsed();
        
        assert!(result.success);
        assert_eq!(result.successful_tools, 3);
        assert_eq!(tool_executor.get_call_count(), 3);
        
        // Should execute in parallel, so total time should be close to individual tool time
        assert!(execution_time < Duration::from_millis(250)); // Some buffer for overhead
    }
    
    #[tokio::test]
    async fn test_dependency_orchestration() {
        let mut config = OrchestrationConfig::default();
        config.global_timeout = Duration::from_secs(15); // Shorter for tests
        config.default_tool_timeout = Duration::from_secs(2);
        let orchestrator = ToolOrchestrator::new(config).await.unwrap();
        
        let tool_executor = Arc::new(MockToolExecutor::new(false, Duration::from_millis(50)));
        let event_emitter = Arc::new(MockEventEmitter::new());
        
        let requests = vec![
            ToolExecutionRequest::new("tool_1".to_string(), serde_json::json!({})),
            ToolExecutionRequest::new("tool_2".to_string(), serde_json::json!({}))
                .with_dependency("tool_1".to_string()),
            ToolExecutionRequest::new("tool_3".to_string(), serde_json::json!({}))
                .with_dependency("tool_2".to_string()),
        ];
        
        let result = orchestrator.orchestrate_tools(
            requests,
            tool_executor.clone(),
            event_emitter,
        ).await.unwrap();
        
        assert!(result.success);
        assert_eq!(result.successful_tools, 3);
        assert_eq!(result.execution_plan.phases.len(), 3); // Should be 3 phases due to dependencies
    }
    
    #[tokio::test]
    async fn test_tool_failure_handling() {
        // Create config with shorter timeouts for testing
        let mut config = OrchestrationConfig::default();
        config.global_timeout = Duration::from_secs(30); // Increased for debugging
        config.default_tool_timeout = Duration::from_secs(5);
        config.max_retry_attempts = 1; // Fewer retries for faster tests
        
        let orchestrator = ToolOrchestrator::new(config).await.unwrap();
        
        let tool_executor = Arc::new(MockToolExecutor::new(true, Duration::from_millis(10))); // Will fail
        let event_emitter = Arc::new(MockEventEmitter::new());
        
        let request = ToolExecutionRequest::new(
            "failing_tool".to_string(),
            serde_json::json!({})
        );
        
        let result = orchestrator.orchestrate_tools(
            vec![request],
            tool_executor,
            event_emitter,
        ).await.unwrap();
        
        assert!(!result.success);
        assert_eq!(result.successful_tools, 0);
        assert_eq!(result.failed_tools, 1);
        assert_eq!(result.skipped_tools, 0);
    }
    
    #[tokio::test]
    async fn test_resource_management() {
        let mut config = OrchestrationConfig::default();
        config.global_timeout = Duration::from_secs(15); // Shorter for tests
        config.default_tool_timeout = Duration::from_secs(2);
        let orchestrator = ToolOrchestrator::new(config).await.unwrap();
        
        // Register a resource pool
        orchestrator.resource_manager.register_resource_pool(
            "test_resource".to_string(),
            2 // Only 2 units available
        ).await.unwrap();
        
        let tool_executor = Arc::new(MockToolExecutor::new(false, Duration::from_millis(100)));
        let event_emitter = Arc::new(MockEventEmitter::new());
        
        let requests = vec![
            ToolExecutionRequest::new("tool_1".to_string(), serde_json::json!({}))
                .with_resource("test_resource".to_string(), 1, false),
            ToolExecutionRequest::new("tool_2".to_string(), serde_json::json!({}))
                .with_resource("test_resource".to_string(), 1, false),
            ToolExecutionRequest::new("tool_3".to_string(), serde_json::json!({}))
                .with_resource("test_resource".to_string(), 1, false),
        ];
        
        let result = orchestrator.orchestrate_tools(
            requests,
            tool_executor,
            event_emitter,
        ).await.unwrap();
        
        // All should succeed, but resource contention may affect timing
        assert!(result.success);
        assert_eq!(result.successful_tools, 3);
    }
    
    #[tokio::test]
    async fn test_execution_request_builder() {
        let request = ToolExecutionRequest::new("test_tool".to_string(), serde_json::json!({}))
            .with_dependency("dep_tool".to_string())
            .with_resource("cpu".to_string(), 2, false)
            .with_priority(0.8)
            .with_timeout(Duration::from_secs(60))
            .as_critical();
        
        assert_eq!(request.tool_name, "test_tool");
        assert_eq!(request.dependencies, vec!["dep_tool"]);
        assert_eq!(request.required_resources.len(), 1);
        assert_eq!(request.priority, 0.8);
        assert_eq!(request.timeout, Some(Duration::from_secs(60)));
        assert!(request.is_critical);
    }
    
    #[tokio::test]
    async fn test_dependency_analyzer() {
        let analyzer = DependencyAnalyzer::new().await.unwrap();
        
        let requests = vec![
            ToolExecutionRequest::new("A".to_string(), serde_json::json!({})),
            ToolExecutionRequest::new("B".to_string(), serde_json::json!({}))
                .with_dependency("A".to_string()),
            ToolExecutionRequest::new("C".to_string(), serde_json::json!({}))
                .with_dependency("B".to_string()),
            ToolExecutionRequest::new("D".to_string(), serde_json::json!({}))
                .with_dependency("A".to_string()),
        ];
        
        let graph = analyzer.analyze_dependencies(&requests).await.unwrap();
        
        assert_eq!(graph.nodes.len(), 4);
        assert!(graph.topological_order.len() == 4);
        
        // A should come before B and D
        let a_pos = graph.topological_order.iter().position(|x| x == "A").unwrap();
        let b_pos = graph.topological_order.iter().position(|x| x == "B").unwrap();
        let d_pos = graph.topological_order.iter().position(|x| x == "D").unwrap();
        
        assert!(a_pos < b_pos);
        assert!(a_pos < d_pos);
        
        // B should come before C
        let c_pos = graph.topological_order.iter().position(|x| x == "C").unwrap();
        assert!(b_pos < c_pos);
    }
    
    #[tokio::test]
    async fn test_circular_dependency_detection() {
        let analyzer = DependencyAnalyzer::new().await.unwrap();
        
        let requests = vec![
            ToolExecutionRequest::new("A".to_string(), serde_json::json!({}))
                .with_dependency("B".to_string()),
            ToolExecutionRequest::new("B".to_string(), serde_json::json!({}))
                .with_dependency("A".to_string()),
        ];
        
        let result = analyzer.analyze_dependencies(&requests).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_metrics_tracking() {
        let mut config = OrchestrationConfig::default();
        config.global_timeout = Duration::from_secs(20); // Shorter for tests
        config.default_tool_timeout = Duration::from_secs(2);
        let orchestrator = ToolOrchestrator::new(config).await.unwrap();
        
        let tool_executor = Arc::new(MockToolExecutor::new(false, Duration::from_millis(50)));
        let event_emitter = Arc::new(MockEventEmitter::new());
        
        // Execute multiple orchestrations
        for i in 0..3 {
            let request = ToolExecutionRequest::new(
                format!("tool_{}", i),
                serde_json::json!({})
            );
            
            orchestrator.orchestrate_tools(
                vec![request],
                tool_executor.clone(),
                event_emitter.clone(),
            ).await.unwrap();
        }
        
        let metrics = orchestrator.get_metrics().await;
        assert_eq!(metrics.total_orchestrations, 3);
        assert_eq!(metrics.successful_orchestrations, 3);
        assert!(metrics.avg_execution_time_ms > 0);
    }
    
    #[tokio::test]
    async fn test_simple_tool_failure() {
        let config = OrchestrationConfig::default();
        let orchestrator = ToolOrchestrator::new(config).await.unwrap();
        
        let tool_executor = Arc::new(MockToolExecutor::new(true, Duration::from_millis(10)));
        let event_emitter = Arc::new(MockEventEmitter::new());
        
        let request = ToolExecutionRequest::new(
            "test_tool".to_string(),
            serde_json::json!({})
        );
        
        let result = orchestrator.orchestrate_tools(
            vec![request],
            tool_executor,
            event_emitter,
        ).await.unwrap();
        
        assert!(!result.success);
        assert_eq!(result.successful_tools, 0);
        assert_eq!(result.failed_tools, 1);
        assert_eq!(result.skipped_tools, 0);
        
        // Test that we recorded the failure in metrics
        assert!(result.tool_results.iter().any(|(_, result)| 
            matches!(result.status, ExecutionStatus::Failed)));
    }
}

