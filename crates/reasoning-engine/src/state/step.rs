//! Reasoning step management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{orchestration::OrchestrationResult, traits::ToolResult};

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
    /// Create a reasoning step from an orchestration result
    pub fn from_orchestration_result(
        orchestration_result: &OrchestrationResult,
        reasoning_override: Option<&str>,
    ) -> Self {
        let step_id = Uuid::new_v4();
        let timestamp = Utc::now();
        
        // Determine success, error, and tools used based on the orchestration result
        let success = orchestration_result.success;
        let tools_used: Vec<String> = orchestration_result.tool_results.keys().cloned().collect();
        
        // Collect any errors from failed tool executions
        let error = if !success {
            let errors: Vec<String> = orchestration_result
                .tool_results
                .values()
                .filter_map(|result| result.error.as_ref())
                .cloned()
                .collect();
            
            let orchestration_errors = &orchestration_result.orchestration_errors;
            
            let mut all_errors = errors;
            all_errors.extend(orchestration_errors.clone());
            
            if all_errors.is_empty() {
                Some("Orchestration failed with unknown error".to_string())
            } else {
                Some(all_errors.join("; "))
            }
        } else {
            None
        };
        
        let reasoning = reasoning_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if success {
                    "Orchestration completed successfully".to_string()
                } else {
                    format!("Orchestration failed: {}", error.as_ref().unwrap_or(&"Unknown error".to_string()))
                }
            });
        
        // Convert tool results to step output
        let output = if orchestration_result.tool_results.is_empty() {
            StepOutput::Text("No tool results".to_string())
        } else {
            let combined_results: HashMap<String, Value> = orchestration_result
                .tool_results
                .iter()
                .map(|(k, v)| (k.clone(), json!(v)))
                .collect();
            StepOutput::Data(json!(combined_results))
        };
        
        Self {
            id: step_id,
            step_type: StepType::Execute,
            timestamp,
            duration_ms: Some(orchestration_result.total_execution_time.as_millis() as u64),
            input: StepInput::Text("Orchestration request".to_string()),
            output,
            reasoning,
            confidence: if success { 0.8 } else { 0.3 },
            success,
            error,
            tools_used,
            decisions_made: Vec::new(),
            knowledge_gained: HashMap::new(),
            parent_step: None,
            child_steps: Vec::new(),
        }
    }
    
    /// Create a reasoning step for LLM interactions
    pub fn llm_interaction(
        input_text: String, // This would be the prompt or context given to LLM
        output_text: String, // This is the text response from LLM
        success: bool,
        error: Option<String>,
        // TODO: Consider adding duration_ms here if available
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            step_type: StepType::LlmCall,
            timestamp: Utc::now(),
            duration_ms: None, // Could be populated if timing info is available
            input: StepInput::Text(input_text),
            output: StepOutput::Text(output_text),
            reasoning: "LLM interaction".to_string(),
            confidence: if success { 0.7 } else { 0.2 },
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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