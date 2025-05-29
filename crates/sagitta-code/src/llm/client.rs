use async_trait::async_trait;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;
use uuid::Uuid;
use std::collections::HashMap;

use crate::utils::errors::SagittaCodeError;

/// Represents a role in the conversation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "function")]
    Function,
}

/// Configuration for thinking mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Whether to include thought summaries in the response
    pub include_thoughts: bool,
    
    /// Budget for thinking tokens (0-24576, only supported by some models)
    /// Setting to 0 disables thinking
    pub thinking_budget: Option<i32>,
}

impl Default for ThinkingConfig {
    fn default() -> Self {
        Self {
            include_thoughts: false,
            thinking_budget: None, // Auto budget
        }
    }
}

/// Configuration for grounding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingConfig {
    /// Whether to enable grounding with Google Search
    pub enable_web_search: bool,
    
    /// Dynamic retrieval threshold (0.0 = always ground, 1.0 = never ground)
    /// Default: 0.3
    pub dynamic_threshold: Option<f32>,
}

impl Default for GroundingConfig {
    fn default() -> Self {
        Self {
            enable_web_search: false,
            dynamic_threshold: Some(0.3),
        }
    }
}

/// A tool/function definition that can be called by the LLM
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
}

/// A message part that can be text or an attachment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessagePart {
    #[serde(rename = "text")]
    Text { text: String },
    
    #[serde(rename = "tool_call")]
    ToolCall {
        tool_call_id: String,
        name: String,
        parameters: Value,
    },
    
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_call_id: String,
        name: String,
        result: Value,
    },
    
    #[serde(rename = "thought")]
    Thought { text: String },
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for the message
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    
    /// The role of the message sender
    pub role: Role,
    
    /// The parts that make up this message
    pub parts: Vec<MessagePart>,
    
    /// Optional metadata for the message
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, Value>,
}

/// A chunk from the streaming API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// The message part received in this chunk
    pub part: MessagePart,
    
    /// Whether this is the final chunk
    pub is_final: bool,
    
    /// The reason why the stream ended (only set when is_final is true)
    pub finish_reason: Option<String>,

    /// Optional token usage, usually sent with the final chunk or a summary chunk
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
}

/// Grounding metadata from the response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingInfo {
    /// Web search queries that were performed
    pub search_queries: Vec<String>,
    
    /// Grounding sources/chunks
    pub sources: Vec<GroundingSource>,
}

/// A grounding source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingSource {
    /// URI of the source
    pub uri: String,
    
    /// Title of the source
    pub title: String,
    
    /// Confidence score for this source
    pub confidence: f32,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    /// Number of prompt tokens
    pub prompt_tokens: i32,
    
    /// Number of completion tokens
    pub completion_tokens: i32,
    
    /// Total number of tokens
    pub total_tokens: i32,
    
    /// Number of thinking tokens (if thinking mode was used)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<i32>,

    /// Name of the model that produced this usage
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model_name: String,

    /// Cached tokens (if applicable)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i32>,
}

/// A response from the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// The message generated by the LLM
    pub message: Message,
    
    /// Optional tool calls that were generated
    #[serde(default)]
    pub tool_calls: Vec<(String, String, Value)>, // (id, name, parameters)
    
    /// Token usage information
    pub usage: Option<TokenUsage>,
    
    /// Grounding information if grounding was used
    pub grounding: Option<GroundingInfo>,
}

/// Trait for LLM clients that can be used by the agent
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Generate a response from the LLM
    async fn generate(&self, 
        messages: &[Message], 
        tools: &[ToolDefinition]
    ) -> Result<LlmResponse, SagittaCodeError>;
    
    /// Generate a response from the LLM with thinking configuration
    async fn generate_with_thinking(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
    ) -> Result<LlmResponse, SagittaCodeError>;
    
    /// Generate a response from the LLM with grounding support
    async fn generate_with_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError>;
    
    /// Generate a response from the LLM with both thinking and grounding
    async fn generate_with_thinking_and_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
        grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError>;
    
    /// Generate a streaming response from the LLM
    async fn generate_stream(&self, 
        messages: &[Message], 
        tools: &[ToolDefinition]
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError>;
    
    /// Generate a streaming response from the LLM with thinking configuration
    async fn generate_stream_with_thinking(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError>;
    
    /// Generate a streaming response from the LLM with grounding support
    async fn generate_stream_with_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError>;
    
    /// Generate a streaming response from the LLM with both thinking and grounding
    async fn generate_stream_with_thinking_and_grounding(&self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
        grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError>;
}

