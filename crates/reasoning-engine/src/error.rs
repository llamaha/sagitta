#![allow(dead_code)] // Allow dead code for now, to be reviewed later
//! Error types for the reasoning engine

use thiserror::Error;

/// Result type alias for reasoning engine operations
pub type Result<T> = std::result::Result<T, ReasoningError>;

/// Errors that can occur during reasoning operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum ReasoningError {
    /// Configuration validation error
    #[error("Configuration error: {message}")]
    Configuration { message: String },
    
    /// State management error
    #[error("State error in {context}: {message}")]
    State { context: String, message: String },
    
    /// Tool execution error
    #[error("Tool execution error for '{tool}': {message}")]
    ToolExecution { tool: String, message: String },
    
    /// Tool orchestration error
    #[error("Orchestration error: {message}")]
    Orchestration { message: String },
    
    /// Decision making error
    #[error("Decision error: {message} (confidence: {confidence})")]
    Decision { message: String, confidence: f32 },
    
    /// Graph execution error
    #[error("Graph execution error in node '{node}': {message}")]
    GraphExecution { node: String, message: String },
    
    /// Streaming error
    #[error("Streaming error for stream '{stream_id}': {message}")]
    Streaming { stream_id: String, message: String },
    
    /// Backtracking error
    #[error("Backtracking error: {message}")]
    Backtracking { message: String },
    
    /// Reflection error
    #[error("Reflection error: {message}")]
    Reflection { message: String },
    
    /// Pattern recognition error
    #[error("Pattern error: {message}")]
    Pattern { message: String },
    
    /// Confidence calculation error
    #[error("Confidence error: {message}")]
    Confidence { message: String },
    
    /// Coordination error between components
    #[error("Coordination error: {message}")]
    Coordination { message: String },
    
    /// Timeout error
    #[error("Timeout error: {operation} timed out after {duration:?}")]
    Timeout { operation: String, duration: std::time::Duration },
    
    /// Resource exhaustion error
    #[error("Resource exhaustion: {resource} limit exceeded")]
    ResourceExhaustion { resource: String },
    
    /// Serialization/deserialization error
    #[error("Serialization error: {message}")]
    Serialization { message: String },
    
    /// External service error
    #[error("External service error: {service} - {message}")]
    ExternalService { service: String, message: String },
    
    /// LLM error
    #[error("LLM error: {message}")]
    LlmError { message: String },
    
    /// Intent analysis error
    #[error("Intent analysis error: {message}")]
    IntentAnalysisError { message: String },
    
    /// Other reasoning error (covers Unknown)
    #[error("Other reasoning error: {message}")]
    Other { message: String },
}

impl ReasoningError {
    /// Create a configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration { message: message.into() }
    }
    
    /// Create a state error
    pub fn state(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self::State { context: context.into(), message: message.into() }
    }
    
    /// Create a graph execution error
    pub fn graph_execution(node: impl Into<String>, message: impl Into<String>) -> Self {
        Self::GraphExecution { 
            node: node.into(), 
            message: message.into() 
        }
    }
    
    /// Create a decision error
    pub fn decision(message: impl Into<String>, confidence: f32) -> Self {
        Self::Decision { 
            message: message.into(), 
            confidence 
        }
    }
    
    /// Create a streaming error
    pub fn streaming(stream_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Streaming { 
            stream_id: stream_id.into(), 
            message: message.into() 
        }
    }
    
    /// Create a tool execution error
    pub fn tool_execution(tool: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ToolExecution { 
            tool: tool.into(), 
            message: message.into() 
        }
    }
    
    /// Create a coordination error
    pub fn coordination(message: impl Into<String>) -> Self {
        Self::Coordination { message: message.into() }
    }
    
    /// Create a timeout error
    pub fn timeout(operation: impl Into<String>, duration: std::time::Duration) -> Self {
        Self::Timeout { 
            operation: operation.into(), 
            duration 
        }
    }
    
    /// Create a resource exhaustion error
    pub fn resource_exhausted(resource: impl Into<String>) -> Self {
        Self::ResourceExhaustion { 
            resource: resource.into() 
        }
    }
    
    /// Create an external service error
    pub fn external_service(service: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ExternalService { 
            service: service.into(), 
            message: message.into() 
        }
    }
    
    /// Create an orchestration error
    pub fn orchestration(message: impl Into<String>) -> Self {
        Self::Orchestration {
            message: message.into(),
        }
    }
    
    /// Create an LLM error
    pub fn llm(message: impl Into<String>) -> Self {
        Self::LlmError { message: message.into() }
    }
    
    /// Create an intent analysis error
    pub fn intent_analysis(message: impl Into<String>) -> Self {
        Self::IntentAnalysisError { message: message.into() }
    }
    
    /// Create a misc/other error
    pub fn misc(message: impl Into<String>) -> Self {
        Self::Other { message: message.into() }
    }
    
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::ExternalService { .. } => true,
            Self::Timeout { .. } => true,
            Self::LlmError { .. } => true,
            Self::ResourceExhaustion { .. } => false,
            Self::Configuration { .. } => false,
            Self::Serialization { .. } => false,
            _ => false,
        }
    }
    
    /// Get error category for metrics
    pub fn category(&self) -> &'static str {
        match self {
            Self::Configuration { .. } => "configuration",
            Self::State { .. } => "state",
            Self::ToolExecution { .. } => "tool_execution",
            Self::Orchestration { .. } => "orchestration",
            Self::Decision { .. } => "decision",
            Self::GraphExecution { .. } => "graph_execution",
            Self::Streaming { .. } => "streaming",
            Self::Backtracking { .. } => "backtracking",
            Self::Pattern { .. } => "pattern",
            Self::Reflection { .. } => "reflection",
            Self::Confidence { .. } => "confidence",
            Self::Coordination { .. } => "coordination",
            Self::Timeout { .. } => "timeout",
            Self::ResourceExhaustion { .. } => "resource_exhausted",
            Self::ExternalService { .. } => "external_service",
            Self::Serialization { .. } => "serialization",
            Self::LlmError { .. } => "llm_error",
            Self::IntentAnalysisError { .. } => "intent_analysis",
            Self::Other { .. } => "other",
        }
    }
}

/// Convert from serde_json errors
impl From<serde_json::Error> for ReasoningError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization { 
            message: err.to_string() 
        }
    }
}

/// Convert from reqwest errors
impl From<reqwest::Error> for ReasoningError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::Timeout { 
                operation: "HTTP request".to_string(), 
                duration: std::time::Duration::from_secs(0)
            }
        } else if err.is_connect() {
            Self::ExternalService { 
                service: "HTTP".to_string(), 
                message: format!("Connection error: {}", err) 
            }
        } else {
            Self::ExternalService { 
                service: "HTTP".to_string(), 
                message: err.to_string() 
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_creation_and_display() {
        let err = ReasoningError::configuration("Invalid config");
        assert_eq!(err.category(), "configuration");
        assert!(!err.is_retryable());
        assert_eq!(err.to_string(), "Configuration error: Invalid config");

        let stream_err = ReasoningError::streaming("stream123".to_string(), "broken pipe".to_string());
        assert_eq!(stream_err.to_string(), "Streaming error for stream 'stream123': broken pipe");
    }
    
    #[test]
    fn test_error_retryable() {
        let network_err = ReasoningError::ExternalService { 
            service: "HTTP".to_string(), 
            message: "Connection failed".to_string() 
        };
        assert!(network_err.is_retryable());
        
        let config_err = ReasoningError::Configuration { 
            message: "Invalid config".to_string() 
        };
        assert!(!config_err.is_retryable());

        let llm_err = ReasoningError::LlmError { message: "rate limit".to_string() };
        assert!(llm_err.is_retryable());
    }
    
    #[test]
    fn test_error_categories() {
        let errors = vec![
            ReasoningError::configuration("test"),
            ReasoningError::state("context", "test"),
            ReasoningError::streaming("stream_id".to_string(), "test".to_string()),
            ReasoningError::tool_execution("tool", "test"),
            ReasoningError::orchestration("test".to_string()),
            ReasoningError::intent_analysis("test".to_string()),
            ReasoningError::misc("test".to_string()),
        ];
        
        let categories: Vec<&str> = errors.iter().map(|e| e.category()).collect();
        assert_eq!(categories, vec![
            "configuration", "state", "streaming", "tool_execution", 
            "orchestration", "intent_analysis", "other"
        ]);
    }
} 