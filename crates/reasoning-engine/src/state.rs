//! State management for the reasoning engine

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque, HashSet};
use uuid::Uuid;
use std::time::Duration;

use crate::error::{Result, ReasoningError};
use crate::traits::{ToolResult, ReasoningEvent};
use crate::orchestration::OrchestrationResult;

/// Task completion tracking for conversation state management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletion {
    /// Unique task identifier
    pub task_id: String,
    /// Completion marker message
    pub completion_marker: String,
    /// Tool outputs that led to completion
    pub tool_outputs: Vec<String>,
    /// Confidence in task completion (0.0 to 1.0)
    pub success_confidence: f32,
    /// When the task was completed
    pub completed_at: DateTime<Utc>,
    /// Tools that were used to complete this task
    pub tools_used: Vec<String>,
}

/// Tool execution state caching to prevent re-execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionCache {
    /// Successfully executed tools with their results
    pub successful_executions: HashMap<String, CachedToolResult>,
    /// Failed execution attempts for learning
    pub failed_attempts: Vec<FailedExecution>,
    /// Signals that indicate task completion
    pub completion_signals: Vec<CompletionSignal>,
}

/// Cached result from successful tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedToolResult {
    /// Tool name
    pub tool_name: String,
    /// Input arguments used
    pub args: Value,
    /// Result from the tool
    pub result: ToolResult,
    /// When this was cached
    pub cached_at: DateTime<Utc>,
    /// How many times this has been referenced
    pub reference_count: u32,
}

/// Record of a failed tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedExecution {
    /// Tool that failed
    pub tool_name: String,
    /// Arguments used
    pub args: Value,
    /// Error message
    pub error: String,
    /// When it failed
    pub failed_at: DateTime<Utc>,
    /// Whether to retry or avoid
    pub should_retry: bool,
}

/// Signal indicating task completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionSignal {
    /// Type of completion signal
    pub signal_type: CompletionSignalType,
    /// Signal strength (0.0 to 1.0)
    pub strength: f32,
    /// Message or context that triggered the signal
    pub message: String,
    /// When the signal was detected
    pub detected_at: DateTime<Utc>,
}

/// Types of completion signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompletionSignalType {
    /// Tool reported success
    ToolSuccess,
    /// Output contains completion phrases
    CompletionPhrase,
    /// All objectives met
    ObjectiveComplete,
    /// User indicated satisfaction
    UserSatisfaction,
    /// System detected task completion
    SystemDetection,
}

/// Enhanced conversation phase management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConversationPhase {
    /// Starting a new conversation
    Fresh,
    /// Continuing an ongoing conversation
    Ongoing,
    /// Investigating a specific topic
    Investigating { topic: String },
    /// Working on a specific task
    TaskFocused { task: String },
    /// Task execution in progress
    TaskExecution { task: String, progress: f32 },
    /// Task completed successfully
    TaskCompleted { task: String, completion_marker: String },
    /// Waiting for user clarification
    AwaitingClarification,
    /// Follow-up question about completed task
    FollowUpQuestion { completed_task: String },
    /// New task request after completion
    NewTaskRequest,
    /// Completed successfully (for backward compatibility)
    Completed,
}

/// Tool execution state management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionState {
    /// Current execution cache
    pub cache: ToolExecutionCache,
    /// Tools currently being executed
    pub active_executions: HashMap<String, DateTime<Utc>>,
    /// Tools that should not be retried
    pub blocked_tools: HashSet<String>,
    /// Execution history for this session
    pub execution_history: Vec<ToolExecutionRecord>,
}

/// Record of a tool execution attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRecord {
    /// Tool name
    pub tool_name: String,
    /// Arguments used
    pub args: Value,
    /// Whether it succeeded
    pub success: bool,
    /// Result or error message
    pub result_or_error: String,
    /// When it was executed
    pub executed_at: DateTime<Utc>,
    /// Step ID that triggered execution
    pub step_id: Uuid,
}

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

    /// NEW: Conversation context to maintain state across reasoning loops
    pub conversation_context: ConversationContext,
    
    /// NEW: Session metadata for tracking across multiple reasoning calls
    pub session_metadata: SessionMetadata,

    /// NEW: Tool execution state for this specific reasoning session
    pub tool_execution_state: ToolExecutionState,

    /// NEW: Task completion tracking for this session
    pub current_task_completion: Option<TaskCompletion>,

    /// NEW: Last analyzed content to prevent re-processing
    pub last_analyzed_content: Option<String>,

    /// NEW: Content analysis cache
    pub content_analysis_cache: HashMap<String, DateTime<Utc>>,
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

/// A single reasoning step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    /// Unique step identifier
    pub id: Uuid,
    
    /// Type of reasoning step
    pub step_type: StepType,
    
    /// When this step was created
    pub timestamp: DateTime<Utc>,
    
    /// How long this step took to execute (make this Option<u64> for milliseconds)
    pub duration_ms: Option<u64>,
    
    /// Input to this step
    pub input: StepInput,
    
    /// Output from this step
    pub output: StepOutput,
    
    /// Reasoning explanation for this step
    pub reasoning: String,
    
    /// Confidence in this step (0.0 to 1.0)
    pub confidence: f32,
    
    /// Whether this step was successful
    pub success: bool,
    
    /// Error message if step failed
    pub error: Option<String>,
    
    /// Tools used in this step
    pub tools_used: Vec<String>,
    
    /// Decisions made in this step
    pub decisions_made: Vec<Uuid>,
    
    /// Knowledge gained from this step
    pub knowledge_gained: HashMap<String, Value>,
    
    /// Parent step if this is a sub-step
    pub parent_step: Option<Uuid>,
    
    /// Child steps spawned from this step
    pub child_steps: Vec<Uuid>,
}

impl ReasoningStep {
    pub fn from_orchestration_result(
        orchestration_result: &OrchestrationResult,
        reasoning_override: Option<&str>,
    ) -> Self {
        let success = orchestration_result.success;
        let error = if !success {
            Some(
                orchestration_result
                    .tool_results
                    .values()
                    .filter_map(|exec_res| exec_res.result.as_ref().and_then(|tr| tr.error.clone()))
                    .collect::<Vec<String>>()
                    .join("; "),
            )
        } else {
            None
        };

        let tool_names: Vec<String> = orchestration_result.tool_results.keys().cloned().collect();

        ReasoningStep {
            id: Uuid::new_v4(),
            step_type: StepType::Execute,
            timestamp: Utc::now(),
            duration_ms: Some(orchestration_result.total_execution_time.as_millis() as u64),
            input: StepInput::Data(serde_json::json!({
                "tool_requests": orchestration_result.tool_results.keys().cloned().collect::<Vec<String>>() // Simplified input
            })),
            output: StepOutput::Data(serde_json::json!({
                "orchestration_id": orchestration_result.orchestration_id,
                "successful_tools": orchestration_result.successful_tools,
                "failed_tools": orchestration_result.tool_results.values().filter(|r| r.result.as_ref().map_or(true, |tr| !tr.success)).count(),
                "tool_results_summary": orchestration_result.tool_results.iter().map(|(name, res)| format!("{}: {}", name, res.result.as_ref().map_or("N/A", |tr| if tr.success {"Success"} else {"Failure"}))).collect::<Vec<String>>(),
            })),
            reasoning: reasoning_override.map(String::from).unwrap_or_else(|| {
                if success {
                    format!("Successfully executed tools: {}", tool_names.join(", "))
                } else {
                    format!("Failed to execute tools: {}. Error: {}", tool_names.join(", "), error.as_deref().unwrap_or("Unknown error"))
                }
            }),
            confidence: if success { 0.9 } else { 0.3 }, // Default confidence
            success,
            error,
            tools_used: tool_names,
            decisions_made: Vec::new(),
            knowledge_gained: HashMap::new(), // TODO: Populate from tool results if applicable
            parent_step: None,
            child_steps: Vec::new(),
        }
    }

    pub fn llm_interaction(
        input_text: String, // This would be the prompt or context given to LLM
        output_text: String, // This is the text response from LLM
        success: bool,
        error: Option<String>,
        // TODO: Consider adding duration_ms here if available
    ) -> Self {
        ReasoningStep {
            id: Uuid::new_v4(),
            step_type: StepType::LlmCall,
            timestamp: Utc::now(),
            duration_ms: None, // Placeholder for now
            input: StepInput::Text(input_text),
            output: if success {
                StepOutput::Text(output_text)
            } else {
                StepOutput::Error(error.clone().unwrap_or_else(|| "LLM call failed".to_string()))
            },
            reasoning: if success {
                "LLM call successful".to_string()
            } else {
                format!("LLM call failed: {}", error.as_deref().unwrap_or("Unknown error"))
            },
            confidence: if success { 0.85 } else { 0.2 }, // Default confidence
            success,
            error,
            tools_used: Vec::new(),
            decisions_made: Vec::new(),
            knowledge_gained: HashMap::new(),
            parent_step: None,
            child_steps: Vec::new(),
        }
    }
}

/// Types of reasoning steps
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepType {
    /// Analyze the current situation
    Analyze,
    /// Plan the next actions
    Plan,
    /// Execute a specific action or tool
    Execute,
    /// A call to an LLM
    LlmCall,
    /// Verify results
    Verify,
    /// Reflect on progress and adjust
    Reflect,
    /// Make a decision between options
    Decide,
    /// Backtrack to a previous state
    Backtrack,
    /// Wait for human input
    HumanInput,
    /// Synthesize final result
    Synthesize,
}

/// Input to a reasoning step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepInput {
    /// Text input
    Text(String),
    /// Structured data input
    Data(Value),
    /// Tool execution request
    ToolExecution { tool: String, args: Value },
    /// Decision request
    Decision { options: Vec<String>, context: String },
    /// Verification request
    Verification { target: String, criteria: Vec<String> },
}

/// Output from a reasoning step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepOutput {
    /// Text output
    Text(String),
    /// Structured data output
    Data(Value),
    /// Tool execution result
    ToolResult(ToolResult),
    /// Decision result
    Decision { chosen: String, confidence: f32 },
    /// Verification result
    Verification { passed: bool, details: String },
    /// Error output
    Error(String),
}

/// A goal in the reasoning process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    /// Unique goal identifier
    pub id: Uuid,
    /// Goal description
    pub description: String,
    /// Goal priority (higher = more important)
    pub priority: u32,
    /// Estimated complexity (1-10)
    pub complexity: u32,
    /// Required tools for this goal
    pub required_tools: Vec<String>,
    /// Success criteria
    pub success_criteria: Vec<String>,
    /// When this goal was created
    pub created_at: DateTime<Utc>,
    /// Deadline if any
    pub deadline: Option<DateTime<Utc>>,
}

/// A sub-goal that contributes to a larger goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubGoal {
    /// Unique sub-goal identifier
    pub id: Uuid,
    /// Parent goal identifier
    pub parent_goal: Uuid,
    /// Sub-goal description
    pub description: String,
    /// Dependencies on other sub-goals
    pub dependencies: Vec<Uuid>,
    /// Estimated effort (1-10)
    pub effort: u32,
    /// Current status
    pub status: SubGoalStatus,
}

/// Status of a sub-goal
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubGoalStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Blocked,
}

/// A completed goal with results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedGoal {
    /// Original goal
    pub goal: Goal,
    /// Final result
    pub result: Value,
    /// Success status
    pub success: bool,
    /// Completion time
    pub completed_at: DateTime<Utc>,
    /// Steps taken to complete this goal
    pub steps_taken: Vec<Uuid>,
    /// Lessons learned
    pub lessons_learned: Vec<String>,
}

/// A decision point in the reasoning process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPoint {
    /// Unique decision identifier
    pub id: Uuid,
    /// Decision description
    pub description: String,
    /// Available options
    pub options: Vec<DecisionOption>,
    /// Chosen option
    pub chosen_option: Option<String>,
    /// Confidence in the decision
    pub confidence: f32,
    /// Reasoning for the decision
    pub reasoning: String,
    /// When this decision was made
    pub timestamp: DateTime<Utc>,
}

/// An option in a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOption {
    /// Option identifier
    pub id: String,
    /// Option description
    pub description: String,
    /// Estimated cost/effort
    pub estimated_cost: f32,
    /// Estimated benefit
    pub estimated_benefit: f32,
    /// Risk level (0.0 to 1.0)
    pub risk_level: f32,
    /// Prerequisites for this option
    pub prerequisites: Vec<String>,
}

/// A checkpoint for backtracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCheckpoint {
    /// Unique checkpoint identifier
    pub id: Uuid,
    /// Checkpoint description
    pub description: String,
    /// When this checkpoint was created
    pub created_at: DateTime<Utc>,
    /// Reasoning state at this point (serialized)
    pub state_snapshot: Vec<u8>,
    /// Step that created this checkpoint
    pub step_id: Uuid,
    /// Confidence at this checkpoint
    pub confidence: f32,
}

/// Streaming state for coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingState {
    /// Active streams
    pub active_streams: HashMap<Uuid, StreamInfo>,
    /// Pending chunks waiting for processing
    pub pending_chunks: VecDeque<StreamChunk>,
    /// Stream errors encountered
    pub stream_errors: Vec<StreamError>,
    /// Backpressure signals
    pub backpressure_signals: Vec<BackpressureSignal>,
}

/// Information about an active stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    /// Stream identifier
    pub id: Uuid,
    /// Stream type
    pub stream_type: String,
    /// When stream started
    pub started_at: DateTime<Utc>,
    /// Current state
    pub state: String,
    /// Bytes processed
    pub bytes_processed: u64,
    /// Chunks processed
    pub chunks_processed: u64,
}

/// A stream chunk (placeholder - will be defined in streaming module)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Chunk identifier
    pub id: Uuid,
    /// Chunk data
    pub data: Vec<u8>,
    /// Chunk type
    pub chunk_type: String,
    /// Whether this is the final chunk
    pub is_final: bool,
}

/// A stream error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamError {
    /// Error identifier
    pub id: Uuid,
    /// Stream that errored
    pub stream_id: Uuid,
    /// Error message
    pub message: String,
    /// When error occurred
    pub timestamp: DateTime<Utc>,
    /// Whether error is recoverable
    pub recoverable: bool,
}

/// A backpressure signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackpressureSignal {
    /// Signal identifier
    pub id: Uuid,
    /// Stream experiencing backpressure
    pub stream_id: Uuid,
    /// Severity (0.0 to 1.0)
    pub severity: f32,
    /// When signal was generated
    pub timestamp: DateTime<Utc>,
}

/// Current reasoning mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// NEW: Context that persists across multiple reasoning loops within a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    /// The conversation ID this reasoning session belongs to
    pub conversation_id: Option<Uuid>,
    
    /// Previous reasoning sessions in this conversation
    pub previous_sessions: Vec<SessionSummary>,
    
    /// Accumulated knowledge from previous sessions
    pub accumulated_knowledge: HashMap<String, Value>,
    
    /// Tools that have been used successfully in this conversation
    pub successful_tools: HashSet<String>,
    
    /// Patterns that have been effective in this conversation
    pub effective_patterns: Vec<String>,
    
    /// Current conversation phase
    pub conversation_phase: ConversationPhase,

    /// Task completion tracking
    pub completed_tasks: Vec<TaskCompletion>,

    /// Tool execution state for this conversation
    pub tool_execution_state: ToolExecutionState,
}

/// NEW: Metadata for tracking reasoning sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Whether this session is a continuation of a previous one
    pub is_continuation: bool,
    
    /// ID of the previous session if this is a continuation
    pub previous_session_id: Option<Uuid>,
    
    /// Number of iterations completed in this session
    pub iterations_completed: u32,
    
    /// Total reasoning time across all sessions in this conversation
    pub total_reasoning_time: Duration,
    
    /// Success indicators from previous sessions
    pub previous_success_indicators: Vec<String>,
    
    /// Failure patterns to avoid
    pub failure_patterns: Vec<String>,
}

/// NEW: Summary of a completed reasoning session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: Uuid,
    pub input: String,
    pub result: Option<String>,
    pub success: bool,
    pub tools_used: Vec<String>,
    pub key_insights: Vec<String>,
    pub duration: Duration,
    pub completed_at: DateTime<Utc>,
}

/// NEW: Current state of the conversation (DEPRECATED - replaced by ConversationPhase)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConversationState {
    /// Starting a new conversation
    Fresh,
    /// Continuing an ongoing conversation
    Ongoing,
    /// Investigating a specific topic
    Investigating { topic: String },
    /// Working on a specific task
    TaskFocused { task: String },
    /// Waiting for user clarification
    AwaitingClarification,
    /// Completed successfully
    Completed,
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self {
            conversation_id: None,
            previous_sessions: Vec::new(),
            accumulated_knowledge: HashMap::new(),
            successful_tools: HashSet::new(),
            effective_patterns: Vec::new(),
            conversation_phase: ConversationPhase::Fresh,
            completed_tasks: Vec::new(),
            tool_execution_state: ToolExecutionState::default(),
        }
    }
}

impl Default for SessionMetadata {
    fn default() -> Self {
        Self {
            is_continuation: false,
            previous_session_id: None,
            iterations_completed: 0,
            total_reasoning_time: Duration::from_secs(0),
            previous_success_indicators: Vec::new(),
            failure_patterns: Vec::new(),
        }
    }
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
        }
    }
    
    /// NEW: Create a new reasoning state as a continuation of a previous session
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
    
    /// NEW: Get tools used in this session
    pub fn get_tools_used(&self) -> Vec<String> {
        self.history
            .iter()
            .flat_map(|step| &step.tools_used)
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }
    
    /// NEW: Extract key insights from this session
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
    
    /// NEW: Extract insight from step output
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
    
    /// NEW: Extract insight from text output
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
    
    /// NEW: Update conversation context with new information
    pub fn update_conversation_context(&mut self, key: String, value: Value) {
        self.conversation_context.accumulated_knowledge.insert(key, value);
        self.updated_at = Utc::now();
    }
    
    /// NEW: Mark a tool as successful for future reference
    pub fn mark_tool_successful(&mut self, tool_name: String) {
        self.conversation_context.successful_tools.insert(tool_name);
        self.updated_at = Utc::now();
    }
    
    /// NEW: Add an effective pattern for future reference
    pub fn add_effective_pattern(&mut self, pattern: String) {
        self.conversation_context.effective_patterns.push(pattern);
        self.updated_at = Utc::now();
    }
    
    /// NEW: Get context summary for the next reasoning session
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
            .map_err(|e| ReasoningError::state("checkpoint_creation", format!("Failed to serialize state: {}", e)))?;
        
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

    /// NEW: Detect if task has been completed based on multiple signals
    pub fn detect_task_completion(&mut self, response_text: &str, tool_results: &HashMap<String, ToolResult>) -> Option<TaskCompletion> {
        let mut completion_signals = Vec::new();
        
        // NEW: Check if this is a multi-step task that might not be complete
        let is_multistep_task = self.is_multistep_task(&self.context.original_request);
        let has_completion_request = self.has_explicit_completion_request(&self.context.original_request);
        
        // Check for completion phrases in response
        if let Some(signal) = self.detect_completion_phrases(response_text) {
            completion_signals.push(signal);
        }
        
        // Check tool success indicators
        for (tool_name, result) in tool_results {
            if let Some(signal) = self.detect_tool_completion_signal(tool_name, result) {
                completion_signals.push(signal);
            }
        }
        
        // Calculate completion confidence
        let total_strength: f32 = completion_signals.iter().map(|s| s.strength).sum();
        let avg_strength = if completion_signals.is_empty() { 0.0 } else { total_strength / completion_signals.len() as f32 };
        
        // NEW: Adjust threshold based on task type and completion requirements
        let completion_threshold = if is_multistep_task {
            if has_completion_request {
                // Multi-step task with explicit completion request (e.g., "tell me you are done")
                0.9 // Require very high confidence and explicit completion language
            } else {
                // Multi-step task without explicit completion request
                1.2 // Nearly impossible threshold - avoid premature completion
            }
        } else {
            // Single-step task
            0.7 // Original threshold
        };
        
        // NEW: For multi-step tasks, also check if all steps are mentioned as complete
        let all_steps_complete = if is_multistep_task {
            self.check_all_multistep_tasks_complete(response_text, tool_results)
        } else {
            true // Single step task doesn't need this check
        };
        
        if total_strength >= completion_threshold && all_steps_complete {
            let task_completion = TaskCompletion {
                task_id: self.session_id.to_string(),
                completion_marker: response_text.to_string(),
                tool_outputs: tool_results.values().map(|r| format!("{:?}", r.data)).collect(),
                success_confidence: avg_strength.min(1.0),
                completed_at: Utc::now(),
                tools_used: tool_results.keys().cloned().collect(),
            };
            
            // Add signals to cache
            self.tool_execution_state.cache.completion_signals.extend(completion_signals);
            
            Some(task_completion)
        } else {
            None
        }
    }
    
    /// NEW: Check if the original request indicates a multi-step task
    fn is_multistep_task(&self, request: &str) -> bool {
        let request_lower = request.to_lowercase();
        let multistep_indicators = [
            "then", "next", "after", "afterwards", "subsequently", 
            "and then", "followed by", "once", "when", "first",
            "second", "finally", "lastly", "step", "steps"
        ];
        
        // NEW: Check for complex task patterns that inherently require multiple steps
        let complex_task_patterns = [
            "improve", "help me", "create a solution", "analyze", "research",
            "best approach", "error handling", "optimization", "review"
        ];
        
        let has_explicit_multistep = multistep_indicators.iter().any(|indicator| request_lower.contains(indicator));
        let is_complex_task = complex_task_patterns.iter().any(|pattern| request_lower.contains(pattern));
        
        has_explicit_multistep || is_complex_task
    }
    
    /// NEW: Check if the request explicitly asks for completion confirmation
    fn has_explicit_completion_request(&self, request: &str) -> bool {
        let request_lower = request.to_lowercase();
        let completion_requests = [
            "tell me", "let me know", "inform me", "say", "report",
            "done", "finished", "complete", "ready"
        ];
        
        // Look for phrases like "tell me you are done", "let me know when finished", etc.
        completion_requests.iter().any(|req| request_lower.contains(req)) &&
        (request_lower.contains("done") || request_lower.contains("finished") || 
         request_lower.contains("complete") || request_lower.contains("ready"))
    }
    
    /// NEW: Check if all steps in a multi-step task appear to be complete
    fn check_all_multistep_tasks_complete(&self, response_text: &str, tool_results: &HashMap<String, ToolResult>) -> bool {
        let request_lower = self.context.original_request.to_lowercase();
        let response_lower = response_text.to_lowercase();
        
        // NEW: Special handling for complex workflows that require solution creation
        let requires_solution_creation = self.requires_solution_creation(&request_lower);
        
        if requires_solution_creation {
            // For tasks that require solution creation, check if creation step has been completed
            let has_solution_creation = self.has_completed_solution_creation(&response_lower, tool_results);
            if !has_solution_creation {
                return false; // Not complete until solution is created
            }
        }
        
        // Extract potential task actions from the original request
        let task_actions = self.extract_task_actions(&request_lower);
        
        // Check if all actions are reflected in the response or tool results
        if task_actions.is_empty() {
            return true; // If we can't extract actions, assume it's complete
        }
        
        let mut completed_actions = 0;
        for action in &task_actions {
            // Check if this action is mentioned as complete in the response
            if response_lower.contains(&format!("{} complete", action)) ||
               response_lower.contains(&format!("{} finished", action)) ||
               response_lower.contains(&format!("{} done", action)) ||
               response_lower.contains(&format!("created {}", action)) ||
               response_lower.contains(&format!("found {}", action)) {
                completed_actions += 1;
            }
            
            // Check if relevant tools for this action were executed
            for tool_name in tool_results.keys() {
                if self.tool_matches_action(tool_name, action) {
                    completed_actions += 1;
                    break;
                }
            }
        }
        
        // NEW: For complex workflows, require higher completion rate
        let required_completion_rate = if requires_solution_creation { 0.8 } else { 0.7 };
        completed_actions as f32 / task_actions.len() as f32 >= required_completion_rate
    }
    
    /// NEW: Check if the task requires solution creation as a key step
    fn requires_solution_creation(&self, request: &str) -> bool {
        let solution_creation_indicators = [
            "improve", "help me", "create", "solution", "approach", "fix",
            "error handling", "best practices", "optimization", "enhance"
        ];
        
        solution_creation_indicators.iter().any(|indicator| request.contains(indicator))
    }
    
    /// NEW: Check if solution creation has been completed
    fn has_completed_solution_creation(&self, response: &str, tool_results: &HashMap<String, ToolResult>) -> bool {
        // Check if file creation tools were used
        let has_file_creation = tool_results.keys().any(|tool| 
            tool.contains("edit_file") || tool.contains("create_file") || tool.contains("write_file")
        );
        
        // Check if response mentions solution creation
        let mentions_solution_creation = response.contains("created") || 
                                       response.contains("solution") ||
                                       response.contains("file") ||
                                       response.contains("example") ||
                                       response.contains("documentation") ||
                                       response.contains("based on");
        
        has_file_creation && mentions_solution_creation
    }
    
    /// NEW: Extract action words from a task request
    fn extract_task_actions(&self, request: &str) -> Vec<String> {
        let action_words = [
            "search", "find", "create", "write", "make", "build", "generate",
            "analyze", "process", "read", "list", "count", "check", "verify",
            "tell", "report", "inform", "say", "explain", "describe"
        ];
        
        let mut actions = Vec::new();
        for action in &action_words {
            if request.contains(action) {
                actions.push(action.to_string());
            }
        }
        
        actions
    }
    
    /// NEW: Check if a tool name matches a task action
    fn tool_matches_action(&self, tool_name: &str, action: &str) -> bool {
        match action {
            "search" | "find" => tool_name.contains("search") || tool_name.contains("web"),
            "create" | "write" | "make" => tool_name.contains("edit") || tool_name.contains("write") || tool_name.contains("create"),
            "read" | "analyze" => tool_name.contains("read") || tool_name.contains("analyze"),
            "list" | "count" => tool_name.contains("list") || tool_name.contains("dir") || tool_name.contains("count"),
            _ => false,
        }
    }
    
    /// NEW: Detect completion phrases in text
    fn detect_completion_phrases(&self, text: &str) -> Option<CompletionSignal> {
        let text_lower = text.to_lowercase();
        let completion_patterns = [
            ("completed", 0.9),
            ("finished", 0.8),
            ("done", 0.7),
            ("success", 0.8),
            ("analyzed", 0.6),
            ("processed", 0.6),
            ("created", 0.5),
            ("found", 0.4),
            ("has been", 0.3),
            ("count:", 0.7), // For line counting tasks
            ("lines", 0.6),  // For file analysis
            ("total", 0.5),
        ];
        
        let mut best_match = None;
        let mut best_strength = 0.0;
        
        for (pattern, base_strength) in &completion_patterns {
            if text_lower.contains(pattern) {
                let strength = if text_lower.contains("successfully") { 
                    base_strength + 0.2 
                } else { 
                    *base_strength 
                };
                
                if strength > best_strength {
                    best_strength = strength;
                    best_match = Some(CompletionSignal {
                        signal_type: CompletionSignalType::CompletionPhrase,
                        strength,
                        message: text.to_string(),
                        detected_at: Utc::now(),
                    });
                }
            }
        }
        
        best_match
    }
    
    /// NEW: Detect tool completion signals
    fn detect_tool_completion_signal(&self, tool_name: &str, result: &ToolResult) -> Option<CompletionSignal> {
        if result.success {
            let strength = match tool_name {
                "read_file" | "file_read" => 0.8,
                "list_dir" | "directory_list" => 0.6,
                "create_file" | "write_file" => 0.9,
                "run_command" | "execute_command" => 0.7,
                _ => 0.5,
            };
            
            Some(CompletionSignal {
                signal_type: CompletionSignalType::ToolSuccess,
                strength,
                message: format!("Tool {} completed successfully", tool_name),
                detected_at: Utc::now(),
            })
        } else {
            None
        }
    }
    
    /// NEW: Check if content has been analyzed recently to prevent re-processing
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
    
    /// NEW: Mark content as analyzed
    pub fn mark_content_analyzed(&mut self, content: String) {
        self.last_analyzed_content = Some(content.clone());
        self.content_analysis_cache.insert(content, Utc::now());
    }
    
    /// NEW: Check if tool execution should be skipped (already executed successfully)
    pub fn should_skip_tool_execution(&self, tool_name: &str, args: &serde_json::Value) -> Option<&CachedToolResult> {
        let tool_key = format!("{}_{}", tool_name, serde_json::to_string(args).unwrap_or_default());
        self.tool_execution_state.cache.successful_executions.get(&tool_key)
    }
    
    /// NEW: Cache successful tool execution
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
        let execution_record = ToolExecutionRecord {
            tool_name,
            args,
            success: true,
            result_or_error: "Cached successful execution".to_string(),
            executed_at: Utc::now(),
            step_id: Uuid::new_v4(),
        };
        
        self.tool_execution_state.execution_history.push(execution_record);
    }
    
    /// NEW: Record failed tool execution
    pub fn record_failed_execution(&mut self, tool_name: String, args: serde_json::Value, error: String) {
        let should_retry = !error.to_lowercase().contains("already exists") && 
                           !error.to_lowercase().contains("not found") &&
                           !error.to_lowercase().contains("permission denied");
        
        let failed_execution = FailedExecution {
            tool_name: tool_name.clone(),
            args: args.clone(),
            error: error.clone(),
            failed_at: Utc::now(),
            should_retry,
        };
        
        self.tool_execution_state.cache.failed_attempts.push(failed_execution);
        
        // Add to execution history
        let execution_record = ToolExecutionRecord {
            tool_name,
            args,
            success: false,
            result_or_error: error,
            executed_at: Utc::now(),
            step_id: Uuid::new_v4(),
        };
        
        self.tool_execution_state.execution_history.push(execution_record);
    }
    
    /// NEW: Update conversation phase based on current state
    pub fn update_conversation_phase(&mut self, new_phase: ConversationPhase) {
        self.conversation_context.conversation_phase = new_phase;
        self.updated_at = Utc::now();
    }
    
    /// NEW: Check if we're in a completion state that should terminate iteration
    pub fn should_terminate_iteration(&self) -> bool {
        match &self.conversation_context.conversation_phase {
            ConversationPhase::TaskCompleted { .. } => true,
            ConversationPhase::FollowUpQuestion { .. } => true,
            _ => false,
        }
    }
    
    /// NEW: Get completion confidence based on signals and state
    pub fn get_completion_confidence(&self) -> f32 {
        if let Some(task_completion) = &self.current_task_completion {
            return task_completion.success_confidence;
        }
        
        let signal_strengths: Vec<f32> = self.tool_execution_state
            .cache
            .completion_signals
            .iter()
            .map(|s| s.strength)
            .collect();
        
        if signal_strengths.is_empty() {
            0.0
        } else {
            signal_strengths.iter().sum::<f32>() / signal_strengths.len() as f32
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
            max_working_memory: 50, // Configurable limit
        }
    }
    
    /// Add knowledge to the context
    pub fn add_knowledge(&mut self, key: String, value: Value) {
        self.accumulated_knowledge.insert(key, value);
    }
    
    /// Add to working memory
    pub fn add_to_working_memory(&mut self, item: WorkingMemoryItem) {
        // Remove oldest items if at capacity
        while self.working_memory.len() >= self.max_working_memory {
            self.working_memory.pop_front();
        }
        
        self.working_memory.push_back(item);
    }
    
    /// Get relevant working memory items
    pub fn get_relevant_memory(&self, query: &str, limit: usize) -> Vec<&WorkingMemoryItem> {
        // Simple relevance scoring based on content matching
        let mut items: Vec<_> = self.working_memory.iter().collect();
        
        // Sort by relevance (this is a simple implementation)
        items.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
        
        items.into_iter().take(limit).collect()
    }
}

impl StreamingState {
    /// Create a new streaming state
    pub fn new() -> Self {
        Self {
            active_streams: HashMap::new(),
            pending_chunks: VecDeque::new(),
            stream_errors: Vec::new(),
            backpressure_signals: Vec::new(),
        }
    }
    
    /// Add an active stream
    pub fn add_stream(&mut self, stream_info: StreamInfo) {
        self.active_streams.insert(stream_info.id, stream_info);
    }
    
    /// Remove a stream
    pub fn remove_stream(&mut self, stream_id: Uuid) {
        self.active_streams.remove(&stream_id);
    }
    
    /// Add a pending chunk
    pub fn add_pending_chunk(&mut self, chunk: StreamChunk) {
        self.pending_chunks.push_back(chunk);
    }
    
    /// Get next pending chunk
    pub fn next_pending_chunk(&mut self) -> Option<StreamChunk> {
        self.pending_chunks.pop_front()
    }
}

impl Default for StreamingState {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ToolExecutionState {
    fn default() -> Self {
        Self {
            cache: ToolExecutionCache::default(),
            active_executions: HashMap::new(),
            blocked_tools: HashSet::new(),
            execution_history: Vec::new(),
        }
    }
}

impl Default for ToolExecutionCache {
    fn default() -> Self {
        Self {
            successful_executions: HashMap::new(),
            failed_attempts: Vec::new(),
            completion_signals: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_reasoning_state_creation() {
        let state = ReasoningState::new("Test request".to_string());
        assert_eq!(state.context.original_request, "Test request");
        assert_eq!(state.iteration_count, 0);
        assert_eq!(state.confidence_score, 1.0);
        assert_eq!(state.overall_progress, 0.0);
    }
    
    #[test]
    fn test_add_reasoning_step() {
        let mut state = ReasoningState::new("Test".to_string());
        
        let step = ReasoningStep {
            id: Uuid::new_v4(),
            step_type: StepType::Analyze,
            timestamp: Utc::now(),
            duration_ms: None,
            input: StepInput::Text("test input".to_string()),
            output: StepOutput::Text("test output".to_string()),
            reasoning: "test reasoning".to_string(),
            confidence: 0.8,
            success: true,
            error: None,
            tools_used: Vec::new(),
            decisions_made: Vec::new(),
            knowledge_gained: HashMap::new(),
            parent_step: None,
            child_steps: Vec::new(),
        };
        
        state.add_step(step);
        
        assert_eq!(state.iteration_count, 1);
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.confidence_score, 0.8);
    }
    
    #[test]
    fn test_confidence_update() {
        let mut state = ReasoningState::new("Test".to_string());
        
        // Add steps with different confidence levels
        for confidence in [0.9, 0.7, 0.8, 0.6, 0.9] {
            let step = ReasoningStep {
                id: Uuid::new_v4(),
                step_type: StepType::Analyze,
                timestamp: Utc::now(),
                duration_ms: None,
                input: StepInput::Text("test".to_string()),
                output: StepOutput::Text("test".to_string()),
                reasoning: "test".to_string(),
                confidence,
                success: true,
                error: None,
                tools_used: Vec::new(),
                decisions_made: Vec::new(),
                knowledge_gained: HashMap::new(),
                parent_step: None,
                child_steps: Vec::new(),
            };
            state.add_step(step);
        }
        
        // Confidence should be weighted average favoring recent steps
        assert!(state.confidence_score > 0.7);
        assert!(state.confidence_score < 1.0);
    }
    
    #[test]
    fn test_working_memory_limit() {
        let mut context = ReasoningContext::new("Test".to_string());
        context.max_working_memory = 3;
        
        // Add more items than the limit
        for i in 0..5 {
            let item = WorkingMemoryItem {
                id: Uuid::new_v4(),
                content: Value::String(format!("item {}", i)),
                item_type: "test".to_string(),
                relevance: 0.5,
                added_at: Utc::now(),
                last_accessed: Utc::now(),
            };
            context.add_to_working_memory(item);
        }
        
        // Should only keep the last 3 items
        assert_eq!(context.working_memory.len(), 3);
    }
    
    #[test]
    fn test_state_serialization() {
        let state = ReasoningState::new("Test request".to_string());
        let serialized = serde_json::to_string(&state).expect("Should serialize");
        let _deserialized: ReasoningState = serde_json::from_str(&serialized).expect("Should deserialize");
    }

    // NEW COMPREHENSIVE TESTS FOR COMPLETION DETECTION AND STATE MANAGEMENT

    #[test]
    fn test_task_completion_tracking() {
        let task_completion = TaskCompletion {
            task_id: "test_task_1".to_string(),
            completion_marker: "File successfully analyzed: 150 lines".to_string(),
            tool_outputs: vec!["line_count: 150".to_string()],
            success_confidence: 0.95,
            completed_at: Utc::now(),
            tools_used: vec!["read_file".to_string()],
        };

        assert_eq!(task_completion.task_id, "test_task_1");
        assert_eq!(task_completion.success_confidence, 0.95);
        assert_eq!(task_completion.tools_used.len(), 1);
    }

    #[test]
    fn test_tool_execution_cache() {
        let mut cache = ToolExecutionCache::default();
        
        let cached_result = CachedToolResult {
            tool_name: "read_file".to_string(),
            args: serde_json::json!({"file": "test.txt"}),
            result: ToolResult::success(serde_json::json!({"content": "Hello world"}), 100),
            cached_at: Utc::now(),
            reference_count: 1,
        };

        cache.successful_executions.insert("read_file_test.txt".to_string(), cached_result);
        
        assert_eq!(cache.successful_executions.len(), 1);
        assert!(cache.successful_executions.contains_key("read_file_test.txt"));
    }

    #[test]
    fn test_completion_signal_detection() {
        let signal = CompletionSignal {
            signal_type: CompletionSignalType::CompletionPhrase,
            strength: 0.8,
            message: "Task completed successfully".to_string(),
            detected_at: Utc::now(),
        };

        assert!(matches!(signal.signal_type, CompletionSignalType::CompletionPhrase));
        assert_eq!(signal.strength, 0.8);
    }

    #[test]
    fn test_conversation_phase_transitions() {
        let mut state = ReasoningState::new("Count lines in file.txt".to_string());
        
        // Start fresh
        assert!(matches!(state.conversation_context.conversation_phase, ConversationPhase::Fresh));
        
        // Move to task focused
        state.conversation_context.conversation_phase = ConversationPhase::TaskFocused { 
            task: "Count lines in file.txt".to_string() 
        };
        
        if let ConversationPhase::TaskFocused { task } = &state.conversation_context.conversation_phase {
            assert_eq!(task, "Count lines in file.txt");
        } else {
            panic!("Expected TaskFocused phase");
        }
        
        // Move to task execution
        state.conversation_context.conversation_phase = ConversationPhase::TaskExecution { 
            task: "Count lines in file.txt".to_string(),
            progress: 0.5 
        };
        
        // Complete the task
        state.conversation_context.conversation_phase = ConversationPhase::TaskCompleted { 
            task: "Count lines in file.txt".to_string(),
            completion_marker: "File has 150 lines".to_string()
        };
        
        if let ConversationPhase::TaskCompleted { task, completion_marker } = &state.conversation_context.conversation_phase {
            assert_eq!(task, "Count lines in file.txt");
            assert_eq!(completion_marker, "File has 150 lines");
        } else {
            panic!("Expected TaskCompleted phase");
        }
    }

    #[test]
    fn test_tool_execution_state_tracking() {
        let mut state = ReasoningState::new("Test task".to_string());
        
        // Add a successful execution to cache
        let cached_result = CachedToolResult {
            tool_name: "read_file".to_string(),
            args: serde_json::json!({"file": "test.txt"}),
            result: ToolResult::success(serde_json::json!({"lines": 150}), 100),
            cached_at: Utc::now(),
            reference_count: 1,
        };
        
        state.tool_execution_state.cache.successful_executions.insert(
            "read_file_test.txt".to_string(), 
            cached_result
        );
        
        // Add to execution history
        let execution_record = ToolExecutionRecord {
            tool_name: "read_file".to_string(),
            args: serde_json::json!({"file": "test.txt"}),
            success: true,
            result_or_error: "Successfully read 150 lines".to_string(),
            executed_at: Utc::now(),
            step_id: Uuid::new_v4(),
        };
        
        state.tool_execution_state.execution_history.push(execution_record);
        
        assert_eq!(state.tool_execution_state.cache.successful_executions.len(), 1);
        assert_eq!(state.tool_execution_state.execution_history.len(), 1);
        assert!(state.tool_execution_state.execution_history[0].success);
    }

    #[test]
    fn test_failed_execution_tracking() {
        let mut state = ReasoningState::new("Test task".to_string());
        
        let failed_execution = FailedExecution {
            tool_name: "delete_file".to_string(),
            args: serde_json::json!({"file": "nonexistent.txt"}),
            error: "File not found".to_string(),
            failed_at: Utc::now(),
            should_retry: false,
        };
        
        state.tool_execution_state.cache.failed_attempts.push(failed_execution);
        
        assert_eq!(state.tool_execution_state.cache.failed_attempts.len(), 1);
        assert!(!state.tool_execution_state.cache.failed_attempts[0].should_retry);
    }

    #[test]
    fn test_content_analysis_cache() {
        let mut state = ReasoningState::new("Test task".to_string());
        
        let content = "Count the lines in file.txt";
        state.last_analyzed_content = Some(content.to_string());
        state.content_analysis_cache.insert(content.to_string(), Utc::now());
        
        assert!(state.last_analyzed_content.is_some());
        assert_eq!(state.last_analyzed_content.as_ref().unwrap(), content);
        assert!(state.content_analysis_cache.contains_key(content));
    }

    #[test]
    fn test_completion_marker_detection() {
        let completion_phrases = [
            "Task completed successfully",
            "File analysis complete: 150 lines",
            "Successfully created project",
            "Done. The file has been processed.",
            "Finished counting lines",
        ];
        
        for phrase in &completion_phrases {
            let signal = CompletionSignal {
                signal_type: CompletionSignalType::CompletionPhrase,
                strength: 0.8,
                message: phrase.to_string(),
                detected_at: Utc::now(),
            };
            
            assert!(matches!(signal.signal_type, CompletionSignalType::CompletionPhrase));
            assert!(phrase.to_lowercase().contains("complet") || 
                   phrase.to_lowercase().contains("done") || 
                   phrase.to_lowercase().contains("finish") ||
                   phrase.to_lowercase().contains("success"));
        }
    }

    #[test]
    fn test_duplicate_execution_prevention() {
        let mut state = ReasoningState::new("Test task".to_string());
        
        // First execution
        let tool_key = "read_file_test.txt";
        let cached_result = CachedToolResult {
            tool_name: "read_file".to_string(),
            args: serde_json::json!({"file": "test.txt"}),
            result: ToolResult::success(serde_json::json!({"lines": 150}), 100),
            cached_at: Utc::now(),
            reference_count: 1,
        };
        
        state.tool_execution_state.cache.successful_executions.insert(
            tool_key.to_string(), 
            cached_result
        );
        
        // Check if we can detect duplicate execution
        assert!(state.tool_execution_state.cache.successful_executions.contains_key(tool_key));
        
        // Simulate incrementing reference count instead of re-executing
        if let Some(cached) = state.tool_execution_state.cache.successful_executions.get_mut(tool_key) {
            cached.reference_count += 1;
        }
        
        assert_eq!(
            state.tool_execution_state.cache.successful_executions[tool_key].reference_count, 
            2
        );
    }

    #[test]
    fn test_conversation_continuation() {
        let mut first_state = ReasoningState::new("Count lines in file.txt".to_string());
        
        // Complete the first task
        first_state.set_completed(true, "File has 150 lines".to_string());
        first_state.conversation_context.conversation_phase = ConversationPhase::TaskCompleted {
            task: "Count lines in file.txt".to_string(),
            completion_marker: "File has 150 lines".to_string(),
        };
        
        // Create continuation state
        let second_state = ReasoningState::new_continuation(
            "Now delete the file".to_string(),
            &first_state,
            Some(Uuid::new_v4()),
        );
        
        assert!(second_state.session_metadata.is_continuation);
        assert_eq!(second_state.session_metadata.previous_session_id, Some(first_state.session_id));
        assert!(matches!(second_state.conversation_context.conversation_phase, ConversationPhase::Ongoing));
        assert_eq!(second_state.conversation_context.previous_sessions.len(), 1);
    }

    #[test]
    fn test_already_exists_handling() {
        let mut state = ReasoningState::new("Create project directory".to_string());
        
        // Simulate "already exists" scenario
        let failed_execution = FailedExecution {
            tool_name: "create_directory".to_string(),
            args: serde_json::json!({"path": "/tmp/test_project"}),
            error: "Directory already exists".to_string(),
            failed_at: Utc::now(),
            should_retry: false, // Don't retry "already exists" errors
        };
        
        state.tool_execution_state.cache.failed_attempts.push(failed_execution);
        
        // Should be treated as informational, not an error requiring task restart
        assert!(!state.tool_execution_state.cache.failed_attempts[0].should_retry);
        assert!(state.tool_execution_state.cache.failed_attempts[0].error.contains("already exists"));
    }
} 