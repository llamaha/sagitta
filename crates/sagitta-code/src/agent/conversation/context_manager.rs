use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use log::{debug, info};

use crate::utils::errors::SagittaCodeError;

/// Tracks context and flow for a conversation to provide intelligent assistance
#[derive(Debug, Clone)]
pub struct ConversationContextManager {
    /// Current conversation context
    context: Arc<RwLock<ConversationContext>>,
}

/// The current context state of a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    /// Conversation ID
    pub conversation_id: Uuid,
    
    /// Recent failures and their patterns
    pub failure_history: VecDeque<FailureRecord>,
    
    /// Multi-turn planning state
    pub planning_state: Option<MultiTurnPlan>,
    
    /// Context preservation data
    pub preserved_context: HashMap<String, serde_json::Value>,
    
    /// Progress tracking
    pub progress_tracker: ProgressTracker,
    
    /// Frustration detection metrics
    pub frustration_metrics: FrustrationMetrics,
    
    /// Last successful action context
    pub last_success_context: Option<SuccessContext>,
    
    /// Conversation flow state
    pub flow_state: ConversationFlowState,
}

/// Records of failed attempts to learn from them
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    /// When the failure occurred
    pub timestamp: DateTime<Utc>,
    
    /// What action failed
    pub failed_action: String,
    
    /// What parameters were used
    pub parameters: serde_json::Value,
    
    /// The error that occurred
    pub error_message: String,
    
    /// Whether this was a tool failure, LLM failure, or validation failure
    pub failure_type: FailureType,
    
    /// How many times this exact failure has occurred
    pub repeat_count: u32,
    
    /// Context when the failure occurred
    pub context_snapshot: HashMap<String, serde_json::Value>,
}

/// Types of failures that can occur
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FailureType {
    /// Tool execution failed
    ToolExecution,
    /// Tool parameter validation failed
    ParameterValidation,
    /// LLM generation failed
    LlmGeneration,
    /// Loop detection triggered
    InfiniteLoop,
    /// User interrupted/cancelled
    UserCancellation,
    /// System error
    SystemError,
}

/// Multi-turn planning and execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTurnPlan {
    /// Unique plan ID
    pub plan_id: Uuid,
    
    /// Original user request
    pub original_request: String,
    
    /// Broken down steps
    pub steps: Vec<PlanStep>,
    
    /// Current step index
    pub current_step_index: usize,
    
    /// Plan creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Whether plan is active
    pub is_active: bool,
    
    /// Checkpoints for recovery
    pub checkpoints: Vec<PlanCheckpoint>,
}

/// Individual step in a multi-turn plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Step ID
    pub step_id: Uuid,
    
    /// Step description
    pub description: String,
    
    /// Required tools for this step
    pub required_tools: Vec<String>,
    
    /// Expected outcomes
    pub expected_outcomes: Vec<String>,
    
    /// Step status
    pub status: StepStatus,
    
    /// Execution attempts
    pub attempts: Vec<StepAttempt>,
    
    /// Dependencies on other steps
    pub dependencies: Vec<Uuid>,
    
    /// Success criteria
    pub success_criteria: Vec<String>,
}

/// Status of a plan step
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepStatus {
    /// Not yet started
    Pending,
    /// Currently executing
    InProgress,
    /// Successfully completed
    Completed,
    /// Failed with possibility of retry
    Failed,
    /// Skipped due to dependencies or user choice
    Skipped,
    /// Blocked waiting for user input
    BlockedWaitingForUser,
}

/// Record of an attempt to execute a step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepAttempt {
    /// Attempt timestamp
    pub timestamp: DateTime<Utc>,
    
    /// What was tried
    pub action_taken: String,
    
    /// Result of the attempt
    pub result: StepAttemptResult,
    
    /// Any errors encountered
    pub errors: Vec<String>,
    
    /// Context at time of attempt
    pub context: HashMap<String, serde_json::Value>,
}

/// Result of a step attempt
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepAttemptResult {
    /// Step completed successfully
    Success,
    /// Step failed but can be retried
    RetryableFailure,
    /// Step failed and cannot be retried
    PermanentFailure,
    /// Step was cancelled by user
    Cancelled,
    /// Step needs user input to continue
    NeedsUserInput,
}

/// Checkpoint for plan recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanCheckpoint {
    /// Checkpoint ID
    pub checkpoint_id: Uuid,
    
    /// When checkpoint was created
    pub timestamp: DateTime<Utc>,
    
    /// Step index when checkpoint was created
    pub step_index: usize,
    
    /// State snapshot
    pub state_snapshot: HashMap<String, serde_json::Value>,
    
    /// Context snapshot
    pub context_snapshot: HashMap<String, serde_json::Value>,
    
    /// Description of what was accomplished
    pub description: String,
}

/// Tracks progress across conversation turns
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProgressTracker {
    /// Total tasks attempted
    pub total_tasks_attempted: u32,
    
    /// Tasks completed successfully
    pub tasks_completed: u32,
    
    /// Tasks that failed
    pub tasks_failed: u32,
    
    /// Current task being worked on
    pub current_task: Option<String>,
    
    /// Progress milestones reached
    pub milestones: Vec<ProgressMilestone>,
    
    /// Time spent on current task
    pub current_task_start_time: Option<DateTime<Utc>>,
}

/// Progress milestone marker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressMilestone {
    /// Milestone timestamp
    pub timestamp: DateTime<Utc>,
    
    /// What was accomplished
    pub description: String,
    
    /// Confidence level (0.0 - 1.0)
    pub confidence: f32,
    
    /// Associated context
    pub context: HashMap<String, serde_json::Value>,
}

/// Metrics for detecting user frustration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrustrationMetrics {
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    
    /// Number of times user has repeated similar requests
    pub repeated_requests: u32,
    
    /// Time since last successful action
    pub time_since_last_success: Option<DateTime<Utc>>,
    
    /// Indicators of user frustration
    pub frustration_indicators: Vec<FrustrationIndicator>,
    
    /// Current frustration level (0.0 - 1.0)
    pub frustration_level: f32,
}

/// Indicators that suggest user frustration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrustrationIndicator {
    /// When indicator was detected
    pub timestamp: DateTime<Utc>,
    
    /// Type of frustration indicator
    pub indicator_type: FrustrationIndicatorType,
    
    /// Confidence in detection (0.0 - 1.0)
    pub confidence: f32,
    
    /// Context when detected
    pub context: String,
}

/// Types of frustration indicators
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FrustrationIndicatorType {
    /// User repeated the same request multiple times
    RepeatedRequest,
    /// User used urgent language ("please", "urgent", "asap")
    UrgentLanguage,
    /// User expressed confusion ("I don't understand", "this doesn't work")
    ExpressedConfusion,
    /// User requested help explicitly
    RequestedHelp,
    /// Multiple tool failures in short timespan
    MultipleFailures,
    /// User interrupted or cancelled actions
    Interruptions,
}

/// Context from last successful action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessContext {
    /// When the success occurred
    pub timestamp: DateTime<Utc>,
    
    /// What action succeeded
    pub successful_action: String,
    
    /// Parameters that worked
    pub successful_parameters: serde_json::Value,
    
    /// Context that led to success
    pub context_snapshot: HashMap<String, serde_json::Value>,
    
    /// Why it succeeded (if known)
    pub success_factors: Vec<String>,
}

/// Overall conversation flow state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConversationFlowState {
    /// Normal conversation flow
    Normal,
    /// User seems to be exploring/learning
    Exploratory,
    /// User is working on a complex multi-step task
    MultiStepTask,
    /// User appears frustrated or stuck
    Struggling,
    /// User is in a troubleshooting session
    Troubleshooting,
    /// Conversation is in recovery from errors
    Recovery,
    /// User is being onboarded/learning the system
    Onboarding,
}

impl ConversationContextManager {
    /// Create a new context manager for a conversation
    pub fn new(conversation_id: Uuid) -> Self {
        let context = ConversationContext {
            conversation_id,
            failure_history: VecDeque::with_capacity(50), // Keep last 50 failures
            planning_state: None,
            preserved_context: HashMap::new(),
            progress_tracker: ProgressTracker::default(),
            frustration_metrics: FrustrationMetrics::default(),
            last_success_context: None,
            flow_state: ConversationFlowState::Normal,
        };

        Self {
            context: Arc::new(RwLock::new(context)),
        }
    }

    /// Record a failure for learning and pattern detection
    pub async fn record_failure(
        &self,
        action: String,
        parameters: serde_json::Value,
        error: String,
        failure_type: FailureType,
        context_snapshot: HashMap<String, serde_json::Value>,
    ) -> Result<(), SagittaCodeError> {
        let mut context = self.context.write().await;
        
        // Check if this is a repeat of a recent failure
        let repeat_count = context.failure_history
            .iter()
            .filter(|f| f.failed_action == action && f.parameters == parameters)
            .count() as u32;

        let failure_record = FailureRecord {
            timestamp: Utc::now(),
            failed_action: action,
            parameters,
            error_message: error,
            failure_type,
            repeat_count: repeat_count + 1,
            context_snapshot,
        };

        // Add to failure history
        context.failure_history.push_back(failure_record.clone());
        
        // Keep only last 50 failures
        if context.failure_history.len() > 50 {
            context.failure_history.pop_front();
        }

        // Update frustration metrics
        context.frustration_metrics.consecutive_failures += 1;
        
        // Detect frustration patterns
        if repeat_count >= 2 {
            context.frustration_metrics.frustration_indicators.push(FrustrationIndicator {
                timestamp: Utc::now(),
                indicator_type: FrustrationIndicatorType::MultipleFailures,
                confidence: 0.8,
                context: format!("Same action failed {} times", repeat_count + 1),
            });
        }

        // Update flow state based on failure patterns
        if context.frustration_metrics.consecutive_failures >= 3 {
            context.flow_state = ConversationFlowState::Struggling;
        }

        // Calculate frustration level
        context.frustration_metrics.frustration_level = self.calculate_frustration_level(&context).await;

        debug!("Recorded failure: {} (repeat: {})", failure_record.failed_action, failure_record.repeat_count);
        
        Ok(())
    }

    /// Record a successful action
    pub async fn record_success(
        &self,
        action: String,
        parameters: serde_json::Value,
        context_snapshot: HashMap<String, serde_json::Value>,
        success_factors: Vec<String>,
    ) -> Result<(), SagittaCodeError> {
        let mut context = self.context.write().await;
        
        // Reset consecutive failures
        context.frustration_metrics.consecutive_failures = 0;
        context.frustration_metrics.time_since_last_success = Some(Utc::now());
        
        // Record success context
        context.last_success_context = Some(SuccessContext {
            timestamp: Utc::now(),
            successful_action: action.clone(),
            successful_parameters: parameters,
            context_snapshot,
            success_factors,
        });

        // Update progress
        context.progress_tracker.tasks_completed += 1;
        
        // Add progress milestone
        context.progress_tracker.milestones.push(ProgressMilestone {
            timestamp: Utc::now(),
            description: format!("Successfully completed: {action}"),
            confidence: 1.0,
            context: HashMap::new(),
        });

        // Update flow state
        if context.flow_state == ConversationFlowState::Struggling {
            context.flow_state = ConversationFlowState::Recovery;
        } else if context.flow_state == ConversationFlowState::Recovery {
            context.flow_state = ConversationFlowState::Normal;
        }

        // Recalculate frustration level
        context.frustration_metrics.frustration_level = self.calculate_frustration_level(&context).await;

        info!("Recorded success: {action}");
        
        Ok(())
    }

    /// Create a multi-turn plan for complex requests
    pub async fn create_multi_turn_plan(
        &self,
        request: String,
        steps: Vec<(String, Vec<String>, Vec<String>)>, // (description, required_tools, expected_outcomes)
    ) -> Result<Uuid, SagittaCodeError> {
        let mut context = self.context.write().await;
        
        let plan_id = Uuid::new_v4();
        let plan_steps: Vec<PlanStep> = steps
            .into_iter()
            .map(|(description, required_tools, expected_outcomes)| PlanStep {
                step_id: Uuid::new_v4(),
                description,
                required_tools,
                expected_outcomes,
                status: StepStatus::Pending,
                attempts: Vec::new(),
                dependencies: Vec::new(),
                success_criteria: Vec::new(),
            })
            .collect();

        let plan = MultiTurnPlan {
            plan_id,
            original_request: request,
            steps: plan_steps,
            current_step_index: 0,
            created_at: Utc::now(),
            is_active: true,
            checkpoints: Vec::new(),
        };

        context.planning_state = Some(plan);
        context.flow_state = ConversationFlowState::MultiStepTask;
        context.progress_tracker.current_task = Some("Multi-step plan execution".to_string());
        context.progress_tracker.current_task_start_time = Some(Utc::now());

        info!("Created multi-turn plan with {} steps", context.planning_state.as_ref().unwrap().steps.len());
        
        Ok(plan_id)
    }

    /// Get current step in active plan
    pub async fn get_current_step(&self) -> Option<PlanStep> {
        let context = self.context.read().await;
        if let Some(plan) = &context.planning_state {
            if plan.is_active && plan.current_step_index < plan.steps.len() {
                return Some(plan.steps[plan.current_step_index].clone());
            }
        }
        None
    }

    /// Mark current step as completed and move to next
    pub async fn complete_current_step(&self, success_context: HashMap<String, serde_json::Value>) -> Result<bool, SagittaCodeError> {
        let mut context = self.context.write().await;
        
        if let Some(plan) = &mut context.planning_state {
            if plan.is_active && plan.current_step_index < plan.steps.len() {
                // Mark current step as completed
                plan.steps[plan.current_step_index].status = StepStatus::Completed;
                
                // Add success attempt
                plan.steps[plan.current_step_index].attempts.push(StepAttempt {
                    timestamp: Utc::now(),
                    action_taken: "Step completed".to_string(),
                    result: StepAttemptResult::Success,
                    errors: Vec::new(),
                    context: success_context.clone(),
                });

                // Create checkpoint
                let checkpoint = PlanCheckpoint {
                    checkpoint_id: Uuid::new_v4(),
                    timestamp: Utc::now(),
                    step_index: plan.current_step_index,
                    state_snapshot: HashMap::new(),
                    context_snapshot: success_context,
                    description: format!("Completed step: {}", plan.steps[plan.current_step_index].description),
                };
                plan.checkpoints.push(checkpoint);

                // Move to next step
                plan.current_step_index += 1;
                
                // Check if plan is complete
                if plan.current_step_index >= plan.steps.len() {
                    plan.is_active = false;
                    context.flow_state = ConversationFlowState::Normal;
                    context.progress_tracker.current_task = None;
                    
                    info!("Multi-turn plan completed successfully");
                    return Ok(true); // Plan complete
                }
                
                info!("Moved to step {} of {}", plan.current_step_index + 1, plan.steps.len());
                return Ok(false); // More steps remaining
            }
        }
        
        Ok(false)
    }

    /// Detect if user needs proactive assistance
    pub async fn should_offer_proactive_assistance(&self) -> ProactiveAssistanceRecommendation {
        let context = self.context.read().await;
        
        let mut recommendations = Vec::new();
        let mut confidence = 0.0;

        // Check frustration indicators
        if context.frustration_metrics.frustration_level > 0.6 {
            recommendations.push("The user seems frustrated. Offer to help or suggest an alternative approach.".to_string());
            confidence += 0.3;
        }

        // Check consecutive failures
        if context.frustration_metrics.consecutive_failures >= 3 {
            recommendations.push("Multiple consecutive failures detected. Suggest troubleshooting or ask for clarification.".to_string());
            confidence += 0.4;
        }

        // Check if user is stuck on same action - but only consider failures after last success
        let failures_to_check: Vec<_> = if let Some(last_success) = &context.last_success_context {
            // Only look at failures that happened after the last success
            context.failure_history
                .iter()
                .filter(|f| f.timestamp > last_success.timestamp)
                .rev()
                .take(5)
                .collect()
        } else {
            // No success yet, check all recent failures
            context.failure_history
                .iter()
                .rev()
                .take(5)
                .collect()
        };
        
        if failures_to_check.len() >= 3 {
            let same_action_failures = failures_to_check.iter()
                .filter(|f| f.failed_action == failures_to_check[0].failed_action)
                .count();
            
            if same_action_failures >= 3 {
                recommendations.push(format!(
                    "User is stuck repeating the same action: '{}'. Suggest alternative approaches or ask for more context.",
                    failures_to_check[0].failed_action
                ));
                confidence += 0.5;
            }
        }

        // Check if last success was long ago
        if let Some(last_success) = &context.last_success_context {
            let time_since_success = Utc::now().signed_duration_since(last_success.timestamp);
            if time_since_success.num_minutes() > 10 {
                recommendations.push("It's been a while since the last successful action. Offer to review what worked before.".to_string());
                confidence += 0.2;
            }
        }

        // Check multi-step plan progress
        if let Some(plan) = &context.planning_state {
            if plan.is_active {
                let failed_steps = plan.steps.iter().filter(|s| s.status == StepStatus::Failed).count();
                if failed_steps > 0 {
                    recommendations.push("Some steps in the current plan have failed. Suggest reviewing the plan or breaking it down further.".to_string());
                    confidence += 0.3;
                }
            }
        }

        ProactiveAssistanceRecommendation {
            should_assist: confidence > 0.4,
            confidence,
            recommendations,
            suggested_actions: self.generate_suggested_actions(&context).await,
        }
    }

    /// Generate suggested actions based on context
    async fn generate_suggested_actions(&self, context: &ConversationContext) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Based on failure patterns
        if !context.failure_history.is_empty() {
            let recent_failure = &context.failure_history[context.failure_history.len() - 1];
            match recent_failure.failure_type {
                FailureType::ParameterValidation => {
                    suggestions.push("Let me help you with the correct parameters for this action.".to_string());
                },
                FailureType::ToolExecution => {
                    suggestions.push("This tool failed. Would you like me to try a different approach?".to_string());
                },
                FailureType::InfiniteLoop => {
                    suggestions.push("I detected I was repeating the same action. Let me try a different strategy.".to_string());
                },
                _ => {}
            }
        }

        // Based on last success
        if let Some(success) = &context.last_success_context {
            suggestions.push(format!(
                "The last successful action was '{}'. Would you like me to try something similar?",
                success.successful_action
            ));
        }

        // Based on flow state
        match context.flow_state {
            ConversationFlowState::Struggling => {
                suggestions.push("I notice you might be having difficulties. Would you like me to explain what I'm trying to do?".to_string());
                suggestions.push("Should I break this down into smaller steps?".to_string());
            },
            ConversationFlowState::MultiStepTask => {
                if let Some(plan) = &context.planning_state {
                    suggestions.push(format!(
                        "We're currently on step {} of {}. Would you like me to continue or modify the plan?",
                        plan.current_step_index + 1,
                        plan.steps.len()
                    ));
                }
            },
            _ => {}
        }

        suggestions
    }

    /// Calculate current frustration level
    async fn calculate_frustration_level(&self, context: &ConversationContext) -> f32 {
        let mut frustration = 0.0;

        // Factor in consecutive failures
        frustration += (context.frustration_metrics.consecutive_failures as f32) * 0.1;

        // Factor in repeated requests  
        frustration += (context.frustration_metrics.repeated_requests as f32) * 0.05;

        // Factor in time since last success
        if let Some(last_success_time) = context.frustration_metrics.time_since_last_success {
            let minutes_since_success = Utc::now().signed_duration_since(last_success_time).num_minutes();
            if minutes_since_success > 5 {
                frustration += (minutes_since_success as f32) * 0.01;
            }
        }

        // Factor in frustration indicators
        frustration += context.frustration_metrics.frustration_indicators.len() as f32 * 0.1;

        // Cap at 1.0
        frustration.min(1.0)
    }

    /// Get conversation insights for the agent
    pub async fn get_conversation_insights(&self) -> ConversationInsights {
        let context = self.context.read().await;
        
        ConversationInsights {
            flow_state: context.flow_state.clone(),
            frustration_level: context.frustration_metrics.frustration_level,
            recent_failures: context.failure_history.iter().rev().take(5).cloned().collect(),
            current_plan_status: context.planning_state.as_ref().map(|p| PlanStatus {
                plan_id: p.plan_id,
                current_step: p.current_step_index,
                total_steps: p.steps.len(),
                is_active: p.is_active,
                recent_attempts: p.steps.get(p.current_step_index)
                    .map(|s| s.attempts.clone())
                    .unwrap_or_default(),
            }),
            progress_summary: ProgressSummary {
                total_attempted: context.progress_tracker.total_tasks_attempted,
                completed: context.progress_tracker.tasks_completed,
                failed: context.progress_tracker.tasks_failed,
                current_task: context.progress_tracker.current_task.clone(),
            },
            last_success: context.last_success_context.clone(),
            proactive_assistance: self.should_offer_proactive_assistance().await,
        }
    }

    /// Preserve context for recovery
    pub async fn preserve_context(&self, key: String, value: serde_json::Value) -> Result<(), SagittaCodeError> {
        let mut context = self.context.write().await;
        context.preserved_context.insert(key, value);
        Ok(())
    }

    /// Retrieve preserved context
    pub async fn get_preserved_context(&self, key: &str) -> Option<serde_json::Value> {
        let context = self.context.read().await;
        context.preserved_context.get(key).cloned()
    }

    /// Clear preserved context
    pub async fn clear_preserved_context(&self) -> Result<(), SagittaCodeError> {
        let mut context = self.context.write().await;
        context.preserved_context.clear();
        Ok(())
    }
}

/// Recommendation for proactive assistance
#[derive(Debug, Clone)]
pub struct ProactiveAssistanceRecommendation {
    /// Whether to offer assistance
    pub should_assist: bool,
    
    /// Confidence level (0.0 - 1.0)
    pub confidence: f32,
    
    /// Specific recommendations
    pub recommendations: Vec<String>,
    
    /// Suggested actions to take
    pub suggested_actions: Vec<String>,
}

/// Summary of conversation insights
#[derive(Debug, Clone)]
pub struct ConversationInsights {
    /// Current conversation flow state
    pub flow_state: ConversationFlowState,
    
    /// Current frustration level
    pub frustration_level: f32,
    
    /// Recent failures
    pub recent_failures: Vec<FailureRecord>,
    
    /// Current plan status
    pub current_plan_status: Option<PlanStatus>,
    
    /// Progress summary
    pub progress_summary: ProgressSummary,
    
    /// Last successful action
    pub last_success: Option<SuccessContext>,
    
    /// Proactive assistance recommendation
    pub proactive_assistance: ProactiveAssistanceRecommendation,
}

/// Status of current plan
#[derive(Debug, Clone)]
pub struct PlanStatus {
    /// Plan ID
    pub plan_id: Uuid,
    
    /// Current step index
    pub current_step: usize,
    
    /// Total steps
    pub total_steps: usize,
    
    /// Whether plan is active
    pub is_active: bool,
    
    /// Recent attempts on current step
    pub recent_attempts: Vec<StepAttempt>,
}

/// Summary of progress
#[derive(Debug, Clone)]
pub struct ProgressSummary {
    /// Total tasks attempted
    pub total_attempted: u32,
    
    /// Tasks completed
    pub completed: u32,
    
    /// Tasks failed
    pub failed: u32,
    
    /// Current task
    pub current_task: Option<String>,
} 