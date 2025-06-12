//! Core reasoning state and context management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;
use uuid::Uuid;

use crate::{error::Result, traits::ToolResult};
use super::{
    conversation::{ConversationContext, ConversationPhase},
    decision::{DecisionPoint, StateCheckpoint},
    goal::{Goal, SubGoal, CompletedGoal},
    session::{SessionMetadata, SessionSummary},
    step::{ReasoningStep, StepType, StepOutput},
    streaming::StreamingState,
    task_completion::{TaskCompletion, TaskCompletionAnalyzer, CompletionSignal, CompletionSignalType},
    tool_execution::{ToolExecutionState, CachedToolResult},
};

/// The main reasoning state that persists across reasoning steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningState {
    /// Unique session identifier
    pub session_id: Uuid,
    
    /// When this session was created
    pub created_at: DateTime<Utc>,
    
    /// When this state was last updated
    pub updated_at: DateTime<Utc>,
    
    /// The reasoning context
    pub context: ReasoningContext,
    
    /// History of reasoning steps
    pub history: Vec<ReasoningStep>,
    
    /// Current goal being worked on
    pub current_goal: Option<Goal>,
    
    /// Queue of sub-goals to be processed
    pub sub_goals: VecDeque<SubGoal>,
    
    /// Completed goals
    pub completed_goals: Vec<CompletedGoal>,
    
    /// Current iteration count
    pub iteration_count: u32,
    
    /// Overall confidence score for the current reasoning chain
    pub confidence_score: f32,
    
    /// Overall progress (0.0 to 1.0)
    pub overall_progress: f32,
    
    /// Decision points encountered during reasoning
    pub decision_points: Vec<DecisionPoint>,
    
    /// Checkpoints for backtracking
    pub checkpoints: Vec<StateCheckpoint>,
    
    /// Current checkpoint being used
    pub current_checkpoint: Option<Uuid>,
    
    /// Patterns used in this session
    pub patterns_used: Vec<String>,
    
    /// Strategies attempted
    pub strategies_attempted: Vec<String>,
    
    /// Success indicators for learning
    pub success_indicators: HashMap<String, f32>,
    
    /// Streaming state
    pub streaming_state: StreamingState,
    
    /// Current execution mode
    pub mode: ReasoningMode,
    
    /// Metadata for debugging and analysis
    pub metadata: HashMap<String, Value>,

    /// Fields for completion status
    pub completion_reason: Option<String>,
    pub is_final_success_status: Option<bool>,

    /// Conversation context to maintain state across reasoning loops
    pub conversation_context: ConversationContext,
    
    /// Session metadata for tracking across multiple reasoning calls
    pub session_metadata: SessionMetadata,

    /// Tool execution state for this specific reasoning session
    pub tool_execution_state: ToolExecutionState,

    /// Task completion tracking for this session
    pub current_task_completion: Option<TaskCompletion>,

    /// Last analyzed content to prevent re-processing
    pub last_analyzed_content: Option<String>,

    /// Content analysis cache
    pub content_analysis_cache: HashMap<String, DateTime<Utc>>,

    /// Task completion analyzer for focused analysis
    pub task_completion_analyzer: TaskCompletionAnalyzer,
}

/// Context that accumulates knowledge across reasoning steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningContext {
    /// The original user request
    pub original_request: String,
    
    /// Accumulated knowledge from reasoning steps
    pub accumulated_knowledge: HashMap<String, Value>,
    
    /// Tool results from this session
    pub tool_results: HashMap<String, ToolResult>,
    
    /// User preferences and constraints
    pub user_preferences: HashMap<String, Value>,
    
    /// Project context if available
    pub project_context: Option<ProjectContext>,
    
    /// Available tools and their capabilities
    pub available_tools: Vec<String>,
    
    /// Current working memory (limited size)
    pub working_memory: VecDeque<WorkingMemoryItem>,
    
    /// Maximum working memory size
    pub max_working_memory: usize,
}

/// Current execution mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReasoningMode {
    /// Fully autonomous reasoning
    Autonomous,
    /// Semi-autonomous with human checkpoints
    SemiAutonomous,
    /// Human-guided reasoning
    Guided,
    /// Step-by-step with human approval
    StepByStep,
}

/// Project context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    /// Project name
    pub name: String,
    /// Project description
    pub description: String,
    /// Project type
    pub project_type: String,
    /// Available files and directories
    pub file_structure: HashMap<String, Value>,
    /// Project-specific tools
    pub tools: Vec<String>,
    /// Project constraints
    pub constraints: Vec<String>,
}

/// Working memory item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemoryItem {
    /// Item identifier
    pub id: Uuid,
    /// Item content
    pub content: Value,
    /// Item type
    pub item_type: String,
    /// Relevance score (0.0 to 1.0)
    pub relevance: f32,
    /// When item was added
    pub added_at: DateTime<Utc>,
    /// Last accessed time
    pub last_accessed: DateTime<Utc>,
}

impl ReasoningState {
    /// Create a new reasoning state
    pub fn new(original_request: String) -> Self {
        let session_id = Uuid::new_v4();
        let now = Utc::now();
        
        Self {
            session_id,
            created_at: now,
            updated_at: now,
            context: ReasoningContext::new(original_request),
            history: Vec::new(),
            current_goal: None,
            sub_goals: VecDeque::new(),
            completed_goals: Vec::new(),
            iteration_count: 0,
            confidence_score: 1.0,
            overall_progress: 0.0,
            decision_points: Vec::new(),
            checkpoints: Vec::new(),
            current_checkpoint: None,
            patterns_used: Vec::new(),
            strategies_attempted: Vec::new(),
            success_indicators: HashMap::new(),
            streaming_state: StreamingState::new(),
            mode: ReasoningMode::Autonomous,
            metadata: HashMap::new(),
            completion_reason: None,
            is_final_success_status: None,
            conversation_context: ConversationContext::default(),
            session_metadata: SessionMetadata::default(),
            tool_execution_state: ToolExecutionState::default(),
            current_task_completion: None,
            last_analyzed_content: None,
            content_analysis_cache: HashMap::new(),
            task_completion_analyzer: TaskCompletionAnalyzer::default(),
        }
    }
    
    /// Create a new reasoning state as a continuation of a previous session
    pub fn new_continuation(
        original_request: String,
        previous_state: &ReasoningState,
        conversation_id: Option<Uuid>,
    ) -> Self {
        let mut new_state = Self::new(original_request);
        
        // Set up continuation metadata
        new_state.session_metadata.is_continuation = true;
        new_state.session_metadata.previous_session_id = Some(previous_state.session_id);
        new_state.session_metadata.total_reasoning_time = 
            previous_state.session_metadata.total_reasoning_time;
        
        // Inherit conversation context
        new_state.conversation_context = previous_state.conversation_context.clone();
        new_state.conversation_context.conversation_id = conversation_id;
        
        // Add previous session to history
        if previous_state.is_final_success_status.unwrap_or(false) {
            let session_summary = SessionSummary {
                session_id: previous_state.session_id,
                input: previous_state.context.original_request.clone(),
                result: previous_state.completion_reason.clone(),
                success: previous_state.is_final_success_status.unwrap_or(false),
                tools_used: previous_state.get_tools_used(),
                key_insights: previous_state.extract_key_insights(),
                duration: previous_state.updated_at.signed_duration_since(previous_state.created_at).to_std().unwrap_or(Duration::from_secs(0)),
                completed_at: previous_state.updated_at,
            };
            new_state.conversation_context.previous_sessions.push(session_summary);
        }
        
        // Update conversation state
        new_state.conversation_context.conversation_phase = ConversationPhase::Ongoing;
        
        new_state
    }
    
    /// Get tools used in this session
    pub fn get_tools_used(&self) -> Vec<String> {
        self.history
            .iter()
            .flat_map(|step| &step.tools_used)
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }
    
    /// Extract key insights from this session
    pub fn extract_key_insights(&self) -> Vec<String> {
        let mut insights = Vec::new();
        
        // Extract insights from successful steps
        for step in &self.history {
            if step.success {
                if let StepOutput::Data(data) = &step.output {
                    if let Some(insight) = self.extract_insight_from_step_output(&step.step_type, data) {
                        insights.push(insight);
                    }
                } else if let StepOutput::Text(text) = &step.output {
                    if let Some(insight) = self.extract_insight_from_text_output(text) {
                        insights.push(insight);
                    }
                }
            }
        }
        
        insights
    }
    
    /// Extract insight from step output
    fn extract_insight_from_step_output(&self, step_type: &StepType, data: &Value) -> Option<String> {
        match step_type {
            StepType::Execute => {
                if let Some(tools) = data.get("successful_tools").and_then(|v| v.as_array()) {
                    if !tools.is_empty() {
                        Some(format!("Successfully executed {} tools", tools.len()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            StepType::Analyze => {
                if let Some(analysis) = data.get("analysis").and_then(|v| v.as_str()) {
                    Some(format!("Analysis: {}", analysis))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    
    /// Extract insight from text output
    fn extract_insight_from_text_output(&self, text: &str) -> Option<String> {
        // Look for key patterns that indicate insights
        if text.contains("I found") || text.contains("I discovered") {
            Some(text.lines().next().unwrap_or(text).to_string())
        } else if text.contains("The issue is") || text.contains("The problem is") {
            Some(text.lines().next().unwrap_or(text).to_string())
        } else {
            None
        }
    }
    
    /// Update conversation context with new information
    pub fn update_conversation_context(&mut self, key: String, value: Value) {
        self.conversation_context.accumulated_knowledge.insert(key, value);
        self.updated_at = Utc::now();
    }
    
    /// Mark a tool as successful for future reference
    pub fn mark_tool_successful(&mut self, tool_name: String) {
        self.conversation_context.successful_tools.insert(tool_name);
        self.updated_at = Utc::now();
    }
    
    /// Add an effective pattern for future reference
    pub fn add_effective_pattern(&mut self, pattern: String) {
        self.conversation_context.effective_patterns.push(pattern);
        self.updated_at = Utc::now();
    }
    
    /// Get context summary for the next reasoning session
    pub fn get_context_summary(&self) -> String {
        let mut summary = Vec::new();
        
        // Add conversation state
        match &self.conversation_context.conversation_phase {
            ConversationPhase::Fresh => summary.push("Starting fresh conversation".to_string()),
            ConversationPhase::Ongoing => summary.push("Continuing ongoing conversation".to_string()),
            ConversationPhase::Investigating { topic } => {
                summary.push(format!("Currently investigating: {}", topic));
            }
            ConversationPhase::TaskFocused { task } => {
                summary.push(format!("Working on task: {}", task));
            }
            ConversationPhase::TaskExecution { task, progress } => {
                summary.push(format!("Executing task: {} ({}% complete)", task, (progress * 100.0) as u32));
            }
            ConversationPhase::TaskCompleted { task, completion_marker } => {
                summary.push(format!("Completed task: {} - {}", task, completion_marker));
            }
            ConversationPhase::AwaitingClarification => {
                summary.push("Awaiting user clarification".to_string());
            }
            ConversationPhase::FollowUpQuestion { completed_task } => {
                summary.push(format!("Follow-up question about: {}", completed_task));
            }
            ConversationPhase::NewTaskRequest => {
                summary.push("Ready for new task request".to_string());
            }
            ConversationPhase::Completed => summary.push("Previous task completed".to_string()),
            ConversationPhase::Planning => summary.push("In planning phase".to_string()),
            ConversationPhase::Review => summary.push("In review phase".to_string()),
            ConversationPhase::Completion => summary.push("In completion phase".to_string()),
        }
        
        // Add successful tools
        if !self.conversation_context.successful_tools.is_empty() {
            summary.push(format!(
                "Previously successful tools: {}",
                self.conversation_context.successful_tools.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            ));
        }
        
        // Add key insights from previous sessions
        for session in &self.conversation_context.previous_sessions {
            if session.success && !session.key_insights.is_empty() {
                summary.push(format!(
                    "Previous insights: {}",
                    session.key_insights.join("; ")
                ));
            }
        }
        
        summary.join("\n")
    }

    /// Add a reasoning step
    pub fn add_step(&mut self, step: ReasoningStep) {
        self.history.push(step);
        self.iteration_count += 1;
        self.updated_at = Utc::now();
        
        // Update confidence based on step confidence
        self.update_confidence();
    }
    
    /// Update overall confidence based on recent steps
    fn update_confidence(&mut self) {
        if self.history.is_empty() {
            self.confidence_score = 1.0; // Default if no history
            return;
        }
        
        // Use weighted average of recent steps (more recent = higher weight)
        let recent_steps = self.history.iter().rev().take(5);
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;
        
        for (i, step) in recent_steps.enumerate() {
            let weight = 1.0 / (i as f32 + 1.0); // Decreasing weight for older steps
            weighted_sum += step.confidence * weight;
            weight_sum += weight;
        }
        
        if weight_sum > 0.0 {
            self.confidence_score = weighted_sum / weight_sum;
        } else if let Some(last_step) = self.history.last() {
            // Fallback to last step's confidence if weights somehow sum to 0 (e.g. only one step)
            self.confidence_score = last_step.confidence;
        } else {
            self.confidence_score = 1.0; // Default if all else fails
        }
    }
    
    /// Create a checkpoint of the current state
    pub fn create_checkpoint(&mut self, description: String, step_id: Uuid) -> Result<Uuid> {
        let checkpoint_id = Uuid::new_v4();
        
        // Serialize current state
        let state_data = bincode::serialize(self)
            .map_err(|e| crate::error::ReasoningError::state("checkpoint_creation", format!("Failed to serialize state: {}", e)))?;
        
        let checkpoint = StateCheckpoint {
            id: checkpoint_id,
            description,
            created_at: Utc::now(),
            state_snapshot: state_data,
            step_id,
            confidence: self.confidence_score,
        };
        
        self.checkpoints.push(checkpoint);
        self.current_checkpoint = Some(checkpoint_id);
        
        Ok(checkpoint_id)
    }
    
    /// Get the latest step
    pub fn latest_step(&self) -> Option<&ReasoningStep> {
        self.history.last()
    }
    
    /// Get steps of a specific type
    pub fn steps_of_type(&self, step_type: StepType) -> Vec<&ReasoningStep> {
        self.history.iter().filter(|s| s.step_type == step_type).collect()
    }
    
    /// Calculate overall progress
    pub fn calculate_progress(&mut self) {
        if self.sub_goals.is_empty() && self.completed_goals.is_empty() {
            self.overall_progress = if self.history.is_empty() { 0.0 } else { 0.5 };
            return;
        }
        
        let total_goals = self.sub_goals.len() + self.completed_goals.len();
        if total_goals == 0 {
            self.overall_progress = 1.0;
            return;
        }
        
        self.overall_progress = self.completed_goals.len() as f32 / total_goals as f32;
    }
    
    /// Add metadata
    pub fn add_metadata(&mut self, key: String, value: Value) {
        self.metadata.insert(key, value);
        self.updated_at = Utc::now();
    }
    
    /// Get summary for debugging
    pub fn summary(&self) -> String {
        format!(
            "Session {}: {} steps, {:.2} confidence, {:.1}% progress, {} goals completed",
            self.session_id,
            self.history.len(),
            self.confidence_score,
            self.overall_progress * 100.0,
            self.completed_goals.len()
        )
    }

    /// Set the completion status of the reasoning session.
    /// This should be called when the session terminates, either successfully or due to error/max_iterations.
    pub fn set_completed(&mut self, success: bool, reason: String) {
        self.is_final_success_status = Some(success);
        self.completion_reason = Some(reason);
        self.updated_at = Utc::now();
        // Potentially update overall_progress to 1.0 if not already set
        if self.overall_progress < 1.0 {
            self.overall_progress = 1.0;
        }
    }

    /// Check if the reasoning session was completed successfully.
    /// Returns true if `is_final_success_status` is Some(true).
    pub fn is_successful(&self) -> bool {
        self.is_final_success_status.unwrap_or(false)
    }

    /// Enhanced task completion detection using dedicated analyzer
    pub fn detect_task_completion(&mut self, response_text: &str, tool_results: &HashMap<String, ToolResult>) -> Option<TaskCompletion> {
        self.task_completion_analyzer.detect_completion(
            &self.context.original_request,
            response_text,
            tool_results
        )
    }
    
    /// Check if content has been analyzed recently to prevent re-processing
    pub fn has_content_been_analyzed(&self, content: &str) -> bool {
        if let Some(last_content) = &self.last_analyzed_content {
            if last_content == content {
                return true;
            }
        }
        
        // Check in analysis cache
        if let Some(analyzed_at) = self.content_analysis_cache.get(content) {
            let elapsed = Utc::now().signed_duration_since(*analyzed_at);
            // Consider content "fresh" for 5 minutes
            elapsed.num_minutes() < 5
        } else {
            false
        }
    }
    
    /// Mark content as analyzed
    pub fn mark_content_analyzed(&mut self, content: String) {
        self.content_analysis_cache.insert(content, Utc::now());
    }
    
    /// Check if tool execution should be skipped (already executed successfully)
    pub fn should_skip_tool_execution(&self, tool_name: &str, args: &serde_json::Value) -> Option<&CachedToolResult> {
        let tool_key = format!("{}_{}", tool_name, serde_json::to_string(args).unwrap_or_default());
        self.tool_execution_state.cache.successful_executions.get(&tool_key)
    }
    
    /// Cache successful tool execution
    pub fn cache_tool_execution(&mut self, tool_name: String, args: serde_json::Value, result: ToolResult) {
        let tool_key = format!("{}_{}", tool_name, serde_json::to_string(&args).unwrap_or_default());
        let cached_result = CachedToolResult {
            tool_name: tool_name.clone(),
            args: args.clone(),
            result,
            cached_at: Utc::now(),
            reference_count: 1,
        };
        
        self.tool_execution_state.cache.successful_executions.insert(tool_key, cached_result);
        
        // Add to execution history
        let execution_record = super::tool_execution::ToolExecutionRecord {
            tool_name,
            args,
            success: true,
            result_or_error: "Cached successful execution".to_string(),
            executed_at: Utc::now(),
            step_id: Uuid::new_v4(),
        };
        
        self.tool_execution_state.execution_history.push(execution_record);
    }
    
    /// Record failed tool execution
    pub fn record_failed_execution(&mut self, tool_name: String, args: serde_json::Value, error: String) {
        let should_retry = !error.to_lowercase().contains("already exists") && 
                           !error.to_lowercase().contains("not found") &&
                           !error.to_lowercase().contains("permission denied");
        
        let failed_execution = super::tool_execution::FailedExecution {
            tool_name: tool_name.clone(),
            args: args.clone(),
            error: error.clone(),
            failed_at: Utc::now(),
            should_retry,
        };
        
        self.tool_execution_state.cache.failed_attempts.push(failed_execution);
        
        // Add to execution history
        let execution_record = super::tool_execution::ToolExecutionRecord {
            tool_name,
            args,
            success: false,
            result_or_error: error,
            executed_at: Utc::now(),
            step_id: Uuid::new_v4(),
        };
        
        self.tool_execution_state.execution_history.push(execution_record);
    }
    
    /// Update conversation phase based on current state
    pub fn update_conversation_phase(&mut self, new_phase: ConversationPhase) -> Result<()> {
        // Validate the state transition before applying it
        self.validate_conversation_phase_transition(&self.conversation_context.conversation_phase, &new_phase)?;
        
        self.conversation_context.conversation_phase = new_phase;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Enhanced conversation phase validation with comprehensive state transition rules
    fn validate_conversation_phase_transition(&self, old_phase: &ConversationPhase, new_phase: &ConversationPhase) -> Result<()> {
        use crate::state::ConversationPhase::*;
        
        let valid_transition = match (&self.conversation_context.conversation_phase, new_phase) {
            // Fresh state transitions
            (Fresh, Ongoing) => true,
            (Fresh, Investigating { .. }) => true,
            (Fresh, TaskFocused { .. }) => true,
            (Fresh, AwaitingClarification) => true,
            (Fresh, Planning) => true,
            
            // Ongoing state transitions
            (Ongoing, Investigating { .. }) => true,
            (Ongoing, TaskFocused { .. }) => true,
            (Ongoing, TaskExecution { .. }) => true,
            (Ongoing, AwaitingClarification) => true,
            (Ongoing, Completed) => true,
            (Ongoing, Planning) => true,
            (Ongoing, Review) => true,
            
            // Investigating state transitions
            (Investigating { .. }, Ongoing) => true,
            (Investigating { .. }, TaskFocused { .. }) => true,
            (Investigating { .. }, TaskExecution { .. }) => true,
            (Investigating { .. }, AwaitingClarification) => true,
            (Investigating { .. }, Completed) => true,
            
            // TaskFocused state transitions
            (TaskFocused { .. }, TaskExecution { .. }) => true,
            (TaskFocused { .. }, TaskCompleted { .. }) => true,
            (TaskFocused { .. }, AwaitingClarification) => true,
            (TaskFocused { .. }, Ongoing) => true,
            (TaskFocused { .. }, Review) => true,
            
            // From TaskExecution
            (TaskExecution { .. }, TaskCompleted { .. }) => true,
            (TaskExecution { .. }, AwaitingClarification) => true,
            (TaskExecution { .. }, TaskFocused { .. }) => true, // Allow stepping back for retries
            (TaskExecution { .. }, Ongoing) => true, // Allow general fallback
            (TaskExecution { .. }, Review) => true,
            
            // From TaskCompleted
            (TaskCompleted { .. }, FollowUpQuestion { .. }) => true,
            (TaskCompleted { .. }, NewTaskRequest) => true,
            (TaskCompleted { .. }, Ongoing) => true,
            (TaskCompleted { .. }, Completed) => true,
            (TaskCompleted { .. }, Completion) => true,
            
            // From AwaitingClarification
            (AwaitingClarification, Ongoing) => true,
            (AwaitingClarification, TaskFocused { .. }) => true,
            (AwaitingClarification, TaskExecution { .. }) => true,
            (AwaitingClarification, Investigating { .. }) => true,
            
            // From FollowUpQuestion
            (FollowUpQuestion { .. }, Ongoing) => true,
            (FollowUpQuestion { .. }, TaskFocused { .. }) => true,
            (FollowUpQuestion { .. }, NewTaskRequest) => true,
            (FollowUpQuestion { .. }, Completed) => true,
            
            // From NewTaskRequest
            (NewTaskRequest, TaskFocused { .. }) => true,
            (NewTaskRequest, TaskExecution { .. }) => true,
            (NewTaskRequest, Ongoing) => true,
            (NewTaskRequest, AwaitingClarification) => true,
            (NewTaskRequest, Planning) => true,
            
            // From Completed (terminal state - only allow specific transitions)
            (Completed, NewTaskRequest) => true,
            (Completed, FollowUpQuestion { .. }) => true,
            (Completed, Ongoing) => true, // Allow reopening conversation
            
            // Planning, Review, Completion phase transitions
            (Planning, TaskFocused { .. }) => true,
            (Planning, TaskExecution { .. }) => true,
            (Planning, Ongoing) => true,
            (Planning, Review) => true,
            
            (Review, Completed) => true,
            (Review, TaskCompleted { .. }) => true,
            (Review, Ongoing) => true, // Allow going back to ongoing
            (Review, Completion) => true,
            
            (Completion, NewTaskRequest) => true,
            (Completion, Ongoing) => true,
            (Completion, Completed) => true,
            
            // Same state transitions (always allowed for progress updates)
            (a, b) if std::mem::discriminant(a) == std::mem::discriminant(b) => true,
            
            _ => false,
        };

        if !valid_transition {
            return Err(crate::error::ReasoningError::state(
                "conversation_phase_transition",
                format!("Invalid conversation phase transition from {:?} to {:?}", old_phase, new_phase)
            ));
        }
        
        Ok(())
    }
    
    /// Check if iteration should terminate
    pub fn should_terminate_iteration(&self) -> bool {
        // Check if we have a completion reason
        if self.completion_reason.is_some() {
            return true;
        }
        
        // Check if task completion was detected
        if self.current_task_completion.is_some() {
            return true;
        }
        
        false
    }
    
    /// Get completion confidence
    pub fn get_completion_confidence(&self) -> f32 {
        if let Some(task_completion) = &self.current_task_completion {
            task_completion.success_confidence
        } else if self.is_final_success_status.unwrap_or(false) {
            self.confidence_score
        } else {
            0.0
        }
    }
}

impl ReasoningContext {
    /// Create a new reasoning context
    pub fn new(original_request: String) -> Self {
        Self {
            original_request,
            accumulated_knowledge: HashMap::new(),
            tool_results: HashMap::new(),
            user_preferences: HashMap::new(),
            project_context: None,
            available_tools: Vec::new(),
            working_memory: VecDeque::new(),
            max_working_memory: 100,
        }
    }
    
    /// Add knowledge to the context
    pub fn add_knowledge(&mut self, key: String, value: Value) {
        self.accumulated_knowledge.insert(key, value);
    }
    
    /// Add item to working memory
    pub fn add_to_working_memory(&mut self, item: WorkingMemoryItem) {
        self.working_memory.push_back(item);
        
        // Maintain size limit
        while self.working_memory.len() > self.max_working_memory {
            self.working_memory.pop_front();
        }
    }
    
    /// Get relevant memory items
    pub fn get_relevant_memory(&self, _query: &str, limit: usize) -> Vec<&WorkingMemoryItem> {
        // Simple implementation - return most recent items
        // Could be enhanced with semantic similarity matching
        self.working_memory.iter().rev().take(limit).collect()
    }
} 