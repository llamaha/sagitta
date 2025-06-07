// Common tool data structures will go here

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::utils::errors::SagittaCodeError;

/// A tool definition that describes a tool's interface
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    /// Unique name of the tool
    pub name: String,
    
    /// Human-readable description of the tool
    pub description: String,
    
    /// JSON Schema for the parameters
    pub parameters: Value,
    
    /// Whether the tool is required (must be used)
    #[serde(default)]
    pub is_required: bool,
    
    /// Category of the tool
    pub category: ToolCategory,
    
    /// Additional metadata for the tool
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
}

/// The category of a tool
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolCategory {
    /// Tool that searches or retrieves code
    #[serde(rename = "code_search")]
    CodeSearch,
    
    /// Tool that operates on files
    #[serde(rename = "file_operations")]
    FileOperations,
    
    /// Tool that manages repositories
    #[serde(rename = "repository")]
    Repository,
    
    /// Tool that searches the web
    #[serde(rename = "web_search")]
    WebSearch,
    
    /// Tool that manages conversations
    #[serde(rename = "conversation")]
    Conversation,
    
    /// Tool that edits code
    #[serde(rename = "code_edit")]
    CodeEdit,
    
    /// Tool that executes shell commands
    #[serde(rename = "shell_execution")]
    ShellExecution,
    
    /// Tool that executes tests
    #[serde(rename = "test_execution")]
    TestExecution,
    
    /// Other type of tool
    #[serde(rename = "other")]
    Other,
    
    /// Core variant for essential engine-level tools
    #[serde(rename = "core")]
    Core,
}

impl Default for ToolCategory {
    fn default() -> Self {
        Self::Other
    }
}

impl fmt::Display for ToolCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolCategory::CodeSearch => write!(f, "Code Search"),
            ToolCategory::FileOperations => write!(f, "File Operations"),
            ToolCategory::Repository => write!(f, "Repository"),
            ToolCategory::WebSearch => write!(f, "Web Search"),
            ToolCategory::Conversation => write!(f, "Conversation"),
            ToolCategory::CodeEdit => write!(f, "Code Edit"),
            ToolCategory::ShellExecution => write!(f, "Shell Execution"),
            ToolCategory::TestExecution => write!(f, "Test Execution"),
            ToolCategory::Other => write!(f, "Other"),
            ToolCategory::Core => write!(f, "Core"),
        }
    }
}

/// The result of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResult {
    /// The tool executed successfully
    Success(Value),
    
    /// The tool execution failed
    Error {
        /// The error message
        error: String,
    },
}

impl ToolResult {
    /// Create a successful tool result
    pub fn success(value: Value) -> Self {
        Self::Success(value)
    }
    
    /// Create a successful tool result from a serializable value
    pub fn success_from<T: Serialize>(value: &T) -> Result<Self, SagittaCodeError> {
        Ok(Self::Success(serde_json::to_value(value).map_err(|e| {
            SagittaCodeError::ToolError(format!("Failed to serialize tool result: {}", e))
        })?))
    }
    
    /// Create an error tool result
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            error: message.into(),
        }
    }
    
    /// Check if the result is successful
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }
    
    /// Check if the result is an error
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
    
    /// Get the success value, if this is a success result
    pub fn success_value(&self) -> Option<&Value> {
        match self {
            Self::Success(value) => Some(value),
            _ => None,
        }
    }
    
    /// Get the error message, if this is an error result
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Error { error } => Some(error),
            _ => None,
        }
    }
    
    /// Convert the result to a JSON value
    pub fn to_json(&self) -> Value {
        match self {
            Self::Success(value) => value.clone(),
            Self::Error { error } => json!({
                "error": error,
            }),
        }
    }
}

/// Trait for tool implementations
#[async_trait]
pub trait Tool: Send + Sync + std::fmt::Debug {
    /// Get the definition of this tool
    fn definition(&self) -> ToolDefinition;
    
    /// Execute the tool with the given parameters
    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError>;
    
    /// Get a reference to this tool as std::any::Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any;
}

/// A boxed tool that can be used by the registry
pub type BoxedTool = Arc<dyn Tool>;

/// A map of parameter names to values
pub type ToolParameters = HashMap<String, Value>;

