//! Tool orchestration and coordination
//!
//! This module implements a sophisticated tool orchestration system that manages and coordinates
//! the execution of multiple tools, potentially in parallel, while handling dependencies between
//! tool executions and managing their results.

pub mod types;
pub mod resource_manager;
pub mod execution_planner;
pub mod recovery;
pub mod validation;

#[cfg(test)]
pub mod tests;

// Re-export the main types and structures
pub use types::*;
pub use resource_manager::ResourceManager;
pub use execution_planner::{ExecutionPlanner, DependencyAnalyzer, ResourceOptimizer};
pub use recovery::RecoveryEngine;
pub use validation::ValidationEngine;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;
use futures_util::future::join_all;

use crate::error::{Result, ReasoningError};
use crate::config::OrchestrationConfig;
use crate::traits::{ToolExecutor, ToolResult, EventEmitter, ReasoningEvent};

/// Main tool orchestrator for managing and coordinating tool execution
pub struct ToolOrchestrator {
    config: OrchestrationConfig,
    resource_manager: Arc<ResourceManager>,
    execution_planner: Arc<ExecutionPlanner>,
    recovery_engine: RecoveryEngine,
    validation_engine: ValidationEngine,
    metrics: Arc<RwLock<OrchestrationMetrics>>,
    active_executions: Arc<RwLock<HashMap<Uuid, ExecutionContext>>>,
}

impl ToolOrchestrator {
    /// Create a new tool orchestrator
    pub async fn new(config: OrchestrationConfig) -> Result<Self> {
        Ok(Self {
            resource_manager: Arc::new(ResourceManager::new().await?),
            execution_planner: Arc::new(ExecutionPlanner::new(config.clone()).await?),
            recovery_engine: RecoveryEngine::new(),
            validation_engine: ValidationEngine::new(),
            metrics: Arc::new(RwLock::new(OrchestrationMetrics::default())),
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            config,
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

        // Emit start event - using generic event since ToolOrchestrationStarted doesn't exist
        event_emitter.emit_event(ReasoningEvent::StepCompleted {
            session_id: orchestration_id,
            step_id: Uuid::new_v4(),
            step_type: "tool_orchestration_started".to_string(),
            confidence: 1.0,
            duration_ms: 0,
        }).await?;

        // Create execution plan
        let execution_plan = self.execution_planner.create_plan(&requests).await?;
        
        // Execute the plan
        let tool_results = match self.execute_plan(
            orchestration_id,
            requests.clone(),
            tool_executor,
            event_emitter.clone(),
        ).await {
            Ok(results) => results,
            Err(e) => {
                event_emitter.emit_event(ReasoningEvent::ErrorOccurred {
                    session_id: orchestration_id,
                    error_type: "tool_orchestration_failed".to_string(),
                    error_message: e.to_string(),
                    recoverable: true,
                }).await?;
                return Err(e);
            }
        };

        let total_execution_time = start_time.elapsed();
        
        // Calculate metrics
        let successful_tools = tool_results.values().filter(|r| r.status == ExecutionStatus::Completed).count() as u32;
        let failed_tools = tool_results.values().filter(|r| r.status == ExecutionStatus::Failed).count() as u32;
        let skipped_tools = tool_results.values().filter(|r| r.status == ExecutionStatus::Skipped).count() as u32;

        let result = OrchestrationResult {
            orchestration_id,
            success: failed_tools == 0,
            tool_results,
            total_execution_time,
            successful_tools,
            failed_tools,
            skipped_tools,
            execution_plan,
            orchestration_errors: Vec::new(),
            metrics: self.metrics.read().await.clone(),
        };

        // Update metrics
        self.update_metrics(&result).await;

        // Emit completion event
        event_emitter.emit_event(ReasoningEvent::StepCompleted {
            session_id: orchestration_id,
            step_id: Uuid::new_v4(),
            step_type: "tool_orchestration_completed".to_string(),
            confidence: if result.success { 1.0 } else { 0.5 },
            duration_ms: total_execution_time.as_millis() as u64,
        }).await?;

        Ok(result)
    }

    /// Execute the execution plan
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
        let execution_plan = self.execution_planner.create_plan(&requests).await?;
        let mut completed_tools = HashMap::new();
        let requests_by_name: HashMap<String, ToolExecutionRequest> = requests
            .into_iter()
            .map(|r| (r.tool_name.clone(), r))
            .collect();

        // Execute phases sequentially, tools within phases in parallel
        for phase in &execution_plan.phases {
            let mut phase_futures = Vec::new();

            for tool_name in &phase.tools {
                if let Some(request) = requests_by_name.get(tool_name) {
                    let request_clone = request.clone();
                    let tool_executor_clone = tool_executor.clone();
                    let event_emitter_clone = event_emitter.clone();
                    let completed_tools_ref = &completed_tools;
                    let orchestrator = self;

                    let future = async move {
                        orchestrator.execute_single_tool(
                            request_clone,
                            tool_executor_clone,
                            event_emitter_clone,
                            completed_tools_ref,
                        ).await
                    };

                    phase_futures.push(future);
                }
            }

            // Wait for all tools in this phase to complete
            let phase_results = join_all(phase_futures).await;
            
            for result in phase_results {
                match result {
                    Ok(tool_result) => {
                        completed_tools.insert(tool_result.request.tool_name.clone(), tool_result);
                    }
                    Err(e) => {
                        // Log error but continue with other tools
                        eprintln!("Tool execution failed: {}", e);
                    }
                }
            }
        }

        // Remove execution context
        let mut active_executions = self.active_executions.write().await;
        active_executions.remove(&orchestration_id);

        Ok(completed_tools)
    }

    /// Execute a single tool with retry logic and resource management
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
        let mut retry_attempts = 0;
        let retry_config = request.retry_config.clone().unwrap_or_else(|| self.default_retry_config());

        loop {
            // Check dependencies
            if !self.check_dependencies(&request, completed_tools).await? {
                return Ok(ToolExecutionResult {
                    request: request.clone(),
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
            let allocated_resources = match self.allocate_resources(&request).await {
                Ok(resources) => resources,
                Err(e) => {
                    return Ok(ToolExecutionResult {
                        request: request.clone(),
                        result: None,
                        status: ExecutionStatus::Failed,
                        retry_attempts,
                        resource_wait_time: resource_start.elapsed(),
                        execution_time: Duration::ZERO,
                        error: Some(format!("Resource allocation failed: {}", e)),
                        allocated_resources: Vec::new(),
                        recovery_suggestions: None,
                    });
                }
            };
            let resource_wait_time = resource_start.elapsed();

            // Execute tool
            let execution_start = Instant::now();
            let execution_result = tool_executor.execute_tool(&request.tool_name, request.parameters.clone()).await;
            let execution_time = execution_start.elapsed();

            // Release resources
            if let Err(e) = self.release_resources(&allocated_resources).await {
                eprintln!("Failed to release resources: {}", e);
            }

            match execution_result {
                Ok(mut tool_result) => {
                    // Validate the result
                    let validation_outcome = self.validation_engine
                        .validate_tool_execution_result(&request, &mut tool_result, execution_time)
                        .await;

                    match validation_outcome {
                        ValidationOutcome::Validated => {
                            return Ok(ToolExecutionResult {
                                request: request.clone(),
                                result: Some(tool_result),
                                status: ExecutionStatus::Completed,
                                retry_attempts,
                                resource_wait_time,
                                execution_time,
                                error: None,
                                allocated_resources,
                                recovery_suggestions: None,
                            });
                        }
                        ValidationOutcome::NeedsVerification { reason: _ } => {
                            // Still consider it successful but mark for verification
                            return Ok(ToolExecutionResult {
                                request: request.clone(),
                                result: Some(tool_result),
                                status: ExecutionStatus::Completed,
                                retry_attempts,
                                resource_wait_time,
                                execution_time,
                                error: None,
                                allocated_resources,
                                recovery_suggestions: None,
                            });
                        }
                        ValidationOutcome::Inconsistent { details } => {
                            // Treat as failure and retry if possible
                            if retry_attempts < retry_config.max_attempts {
                                retry_attempts += 1;
                                let delay = self.calculate_retry_delay(&retry_config, retry_attempts);
                                tokio::time::sleep(delay).await;
                                continue;
                            } else {
                                let recovery_suggestions = self.recovery_engine
                                    .analyze_failure_and_suggest_recovery(
                                        &request.tool_name,
                                        &details,
                                        &request.parameters,
                                        retry_attempts,
                                    )
                                    .await
                                    .ok();

                                return Ok(ToolExecutionResult {
                                    request: request.clone(),
                                    result: Some(tool_result),
                                    status: ExecutionStatus::Failed,
                                    retry_attempts,
                                    resource_wait_time,
                                    execution_time,
                                    error: Some(details),
                                    allocated_resources,
                                    recovery_suggestions,
                                });
                            }
                        }
                        ValidationOutcome::VerificationFailed { error } => {
                            return Ok(ToolExecutionResult {
                                request: request.clone(),
                                result: Some(tool_result),
                                status: ExecutionStatus::Failed,
                                retry_attempts,
                                resource_wait_time,
                                execution_time,
                                error: Some(error),
                                allocated_resources,
                                recovery_suggestions: None,
                            });
                        }
                    }
                }
                Err(e) => {
                    // Tool execution failed
                    if retry_attempts < retry_config.max_attempts {
                        retry_attempts += 1;
                        let delay = self.calculate_retry_delay(&retry_config, retry_attempts);
                        tokio::time::sleep(delay).await;
                        continue;
                    } else {
                        let recovery_suggestions = self.recovery_engine
                            .analyze_failure_and_suggest_recovery(
                                &request.tool_name,
                                &e.to_string(),
                                &request.parameters,
                                retry_attempts,
                            )
                            .await
                            .ok();

                        return Ok(ToolExecutionResult {
                            request: request.clone(),
                            result: None,
                            status: ExecutionStatus::Failed,
                            retry_attempts,
                            resource_wait_time,
                            execution_time,
                            error: Some(e.to_string()),
                            allocated_resources,
                            recovery_suggestions,
                        });
                    }
                }
            }
        }
    }

    /// Check if dependencies are satisfied
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

    /// Allocate resources for a tool execution
    async fn allocate_resources(&self, request: &ToolExecutionRequest) -> Result<Vec<AllocatedResource>> {
        let mut allocated = Vec::new();
        
        for resource_req in &request.required_resources {
            let resource = self.resource_manager.allocate_resource(
                &resource_req.resource_type,
                resource_req.amount,
                request.priority,
                &request.tool_name,
            ).await?;
            allocated.push(resource);
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
        let base_delay_ms = config.base_delay.as_millis() as f64;
        let delay_ms = base_delay_ms * (config.backoff_multiplier as f64).powi(attempt as i32 - 1);
        let delay_ms = delay_ms.min(config.max_delay.as_millis() as f64);
        Duration::from_millis(delay_ms as u64)
    }

    /// Get default retry configuration
    fn default_retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(30),
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
        
        // Update timing metrics
        let execution_time_ms = result.total_execution_time.as_millis() as u64;
        let total_time = metrics.total_orchestrations * metrics.avg_execution_time_ms + execution_time_ms;
        metrics.avg_execution_time_ms = total_time / metrics.total_orchestrations;
        
        // Update tool count metrics
        let tool_count = result.tool_results.len() as f32;
        let total_tools = (metrics.total_orchestrations - 1) as f32 * metrics.avg_tools_per_orchestration + tool_count;
        metrics.avg_tools_per_orchestration = total_tools / metrics.total_orchestrations as f32;
    }

    /// Get current orchestration metrics
    pub async fn get_metrics(&self) -> OrchestrationMetrics {
        self.metrics.read().await.clone()
    }

    /// Get active orchestration IDs
    pub async fn get_active_orchestrations(&self) -> Vec<Uuid> {
        self.active_executions.read().await.keys().cloned().collect()
    }

    /// Get circuit breaker state for failure categories
    pub async fn get_circuit_breaker_state(&self, category: &FailureCategory) -> crate::streaming::CircuitBreakerState {
        // For now, return a default state
        // In a real implementation, this would check actual circuit breaker state
        crate::streaming::CircuitBreakerState::Closed
    }
}

impl Default for OrchestrationMetrics {
    fn default() -> Self {
        Self {
            total_orchestrations: 0,
            successful_orchestrations: 0,
            avg_execution_time_ms: 0,
            avg_tools_per_orchestration: 0.0,
            resource_efficiency: 1.0,
            parallelization_efficiency: 1.0,
            failure_reasons: HashMap::new(),
            tool_performance: HashMap::new(),
        }
    }
} 