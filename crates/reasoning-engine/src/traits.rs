//! Core traits for integrating with external systems

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{Result, ReasoningError};
use crate::streaming::StreamChunk;

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool execution was successful
    pub success: bool,
    /// The result data from the tool
    pub data: Value,
    /// Optional error message if execution failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Additional metadata about the execution
    pub metadata: HashMap<String, Value>,
}

impl ToolResult {
    /// Create a successful tool result
    pub fn success(data: Value, execution_time_ms: u64) -> Self {
        Self {
            success: true,
            data,
            error: None,
            execution_time_ms,
            metadata: HashMap::new(),
        }
    }
    
    /// Create a failed tool result
    pub fn failure(error: String, execution_time_ms: u64) -> Self {
        Self {
            success: false,
            data: Value::Null,
            error: Some(error),
            execution_time_ms,
            metadata: HashMap::new(),
        }
    }
    
    /// Add metadata to the result
    pub fn with_metadata(mut self, key: String, value: Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
    
    /// Check if the result indicates success
    pub fn is_success(&self) -> bool {
        self.success
    }
    
    /// Get the error message if any
    pub fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// Tool definition for available tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON schema for tool parameters
    pub parameters: Value,
    /// Whether this tool is required for the current context
    pub is_required: bool,
    /// Tool category for organization
    pub category: Option<String>,
    /// Estimated execution time in milliseconds
    pub estimated_duration_ms: Option<u64>,
}

/// Trait for executing tools
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool with the given name and arguments
    async fn execute_tool(&self, name: &str, args: Value) -> Result<ToolResult>;
    
    /// Get all available tools
    async fn get_available_tools(&self) -> Result<Vec<ToolDefinition>>;
    
    /// Check if a tool is available
    async fn is_tool_available(&self, name: &str) -> bool {
        match self.get_available_tools().await {
            Ok(tools) => tools.iter().any(|t| t.name == name),
            Err(_) => false,
        }
    }
    
    /// Get a specific tool definition
    async fn get_tool_definition(&self, name: &str) -> Result<Option<ToolDefinition>> {
        let tools = self.get_available_tools().await?;
        Ok(tools.into_iter().find(|t| t.name == name))
    }
}

/// Events emitted by the reasoning engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReasoningEvent {
    /// Reasoning session started
    SessionStarted {
        session_id: Uuid,
        input: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// Reasoning step completed
    StepCompleted {
        session_id: Uuid,
        step_id: Uuid,
        step_type: String,
        confidence: f32,
        duration_ms: u64,
    },
    
    /// Decision made
    DecisionMade {
        session_id: Uuid,
        decision_id: Uuid,
        options_considered: u32,
        chosen_option: String,
        confidence: f32,
    },
    
    /// Tool execution started
    ToolExecutionStarted {
        session_id: Uuid,
        tool_name: String,
        tool_args: Value,
    },
    
    /// Tool execution completed
    ToolExecutionCompleted {
        session_id: Uuid,
        tool_name: String,
        success: bool,
        duration_ms: u64,
    },
    
    /// Summary/finalization message
    Summary {
        session_id: Uuid,
        content: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// Stream chunk received
    StreamChunkReceived {
        session_id: Uuid,
        chunk_type: String,
        chunk_size: usize,
    },
    
    /// Error occurred
    ErrorOccurred {
        session_id: Uuid,
        error_type: String,
        error_message: String,
        recoverable: bool,
    },
    
    /// Backtracking initiated
    BacktrackingStarted {
        session_id: Uuid,
        reason: String,
        target_step: Option<Uuid>,
    },
    
    /// Reflection completed
    ReflectionCompleted {
        session_id: Uuid,
        insights: Vec<String>,
        confidence_adjustment: f32,
    },
    
    /// Session completed
    SessionCompleted {
        session_id: Uuid,
        success: bool,
        total_duration_ms: u64,
        steps_executed: u32,
        tools_used: Vec<String>,
    },

    /// User has been prompted for the next action after a tool sequence.
    UserPromptedForNextAction {
        session_id: Uuid,
        prompt: String, // The prompt message shown to the user (e.g., "What would you like to do next?")
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Token usage information received from the LLM client
    TokenUsageReceived {
        session_id: Uuid,
        usage: TokenUsage, // Uses the TokenUsage struct defined in this crate
    },
}

/// Trait for emitting events to external systems
#[async_trait]
pub trait EventEmitter: Send + Sync {
    /// Emit a reasoning event
    async fn emit_event(&self, event: ReasoningEvent) -> Result<()>;
    
    /// Emit multiple events in batch
    async fn emit_events(&self, events: Vec<ReasoningEvent>) -> Result<()> {
        for event in events {
            self.emit_event(event).await?;
        }
        Ok(())
    }
}

/// Trait for handling streaming data
#[async_trait]
pub trait StreamHandler: Send + Sync {
    /// Handle a stream chunk
    async fn handle_chunk(&self, chunk: StreamChunk) -> Result<()>;
    
    /// Handle stream completion
    async fn handle_stream_complete(&self, stream_id: Uuid) -> Result<()>;
    
    /// Handle stream error
    async fn handle_stream_error(&self, stream_id: Uuid, error: ReasoningError) -> Result<()>;
}

/// Trait for state persistence
#[async_trait]
pub trait StatePersistence: Send + Sync {
    /// Save reasoning state
    async fn save_state(&self, session_id: Uuid, state: &[u8]) -> Result<()>;
    
    /// Load reasoning state
    async fn load_state(&self, session_id: Uuid) -> Result<Option<Vec<u8>>>;
    
    /// Delete reasoning state
    async fn delete_state(&self, session_id: Uuid) -> Result<()>;
    
    /// List all saved states
    async fn list_states(&self) -> Result<Vec<Uuid>>;
}

/// Trait for metrics collection
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    /// Record a counter metric
    async fn record_counter(&self, name: &str, value: u64, tags: HashMap<String, String>) -> Result<()>;
    
    /// Record a gauge metric
    async fn record_gauge(&self, name: &str, value: f64, tags: HashMap<String, String>) -> Result<()>;
    
    /// Record a histogram metric
    async fn record_histogram(&self, name: &str, value: f64, tags: HashMap<String, String>) -> Result<()>;
    
    /// Record timing information
    async fn record_timing(&self, name: &str, duration_ms: u64, tags: HashMap<String, String>) -> Result<()> {
        self.record_histogram(name, duration_ms as f64, tags).await
    }
}

// --- LLM Client Traits and Types (simplified for ReasoningEngine) ---

/// Represents a call to a tool, including its name and arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub args: Value,
}

/// A message part for LLM communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmMessagePart {
    Text(String),
    ToolCall(ToolCall),
    // Add other parts if needed by ReasoningEngine, e.g., ToolResult
}

/// A message for LLM communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String, // Simplified: "user", "assistant"
    pub parts: Vec<LlmMessagePart>,
}

/// Token usage information (defined within reasoning-engine)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<i32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i32>,
}

/// A chunk from an LLM stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmStreamChunk {
    Text { content: String, is_final: bool },
    ToolCall { tool_call: ToolCall, is_final: bool },
    TokenUsage(TokenUsage), // Now references the local TokenUsage
    // Consider adding an Error variant if streams can emit structured errors
}

/// Simplified LLM client trait for ReasoningEngine
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn generate_stream(
        &self, 
        messages: Vec<LlmMessage>,
        // tools: Vec<ToolDefinition> // Tools might be handled by ToolExecutor
    ) -> Result<std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<LlmStreamChunk>> + Send>>>;
}

// --- End LLM Client Traits and Types ---

// --- Intent Analysis Traits and Types ---

/// Represents the detected intent of an LLM's text response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetectedIntent {
    /// LLM provides a direct answer or completes the request without further actions.
    ProvidesFinalAnswer,
    /// LLM is asking a question to the user for clarification.
    AsksClarifyingQuestion,
    /// LLM is requesting more input or information from the user to proceed.
    RequestsMoreInput,
    /// LLM indicates it cannot proceed or fulfill the request.
    StatesInabilityToProceed,
    /// LLM has outlined a plan or next steps but hasn't made an explicit tool call.
    ProvidesPlanWithoutExplicitAction,
    /// LLM response is conversational (e.g., greeting, salutation, acknowledgement).
    GeneralConversation,
    /// The intent is unclear or could not be confidently determined.
    Ambiguous,
}

/// Trait for components that can analyze text to determine LLM intent.
#[async_trait::async_trait]
pub trait IntentAnalyzer: Send + Sync {
    /// Analyzes the provided text and returns a detected intent.
    ///
    /// # Arguments
    /// * `text` - The LLM text response to analyze.
    /// * `conversation_context` - Optional broader conversation context which might help disambiguate.
    ///
    /// # Returns
    /// A `Result` containing the `DetectedIntent` or a `ReasoningError` if analysis fails.
    async fn analyze_intent(
        &self,
        text: &str,
        conversation_context: Option<&[LlmMessage]>, // Provide some context
    ) -> Result<DetectedIntent>;
}

// --- End Intent Analysis Traits and Types ---

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tool_result_creation() {
        let success_result = ToolResult::success(
            serde_json::json!({"result": "test"}), 
            100
        );
        assert!(success_result.is_success());
        assert!(success_result.error_message().is_none());
        
        let failure_result = ToolResult::failure(
            "Tool failed".to_string(), 
            50
        );
        assert!(!failure_result.is_success());
        assert_eq!(failure_result.error_message(), Some("Tool failed"));
    }
    
    #[test]
    fn test_tool_result_metadata() {
        let result = ToolResult::success(
            serde_json::json!({"result": "test"}), 
            100
        ).with_metadata(
            "source".to_string(), 
            serde_json::json!("test_tool")
        );
        
        assert_eq!(result.metadata.len(), 1);
        assert_eq!(result.metadata.get("source"), Some(&serde_json::json!("test_tool")));
    }
    
    #[test]
    fn test_reasoning_event_serialization() {
        let event = ReasoningEvent::SessionStarted {
            session_id: Uuid::new_v4(),
            input: "test input".to_string(),
            timestamp: chrono::Utc::now(),
        };
        
        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: ReasoningEvent = serde_json::from_str(&serialized).unwrap();
        
        match (event, deserialized) {
            (ReasoningEvent::SessionStarted { input: input1, .. }, 
             ReasoningEvent::SessionStarted { input: input2, .. }) => {
                assert_eq!(input1, input2);
            }
            _ => panic!("Event type mismatch after serialization"),
        }
    }
} 