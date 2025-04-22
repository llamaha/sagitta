use serde::{Deserialize, Serialize};

// Re-export the Result type from our error module for convenience
pub use crate::utils::error::Result;

/// Represents a role in the conversation (user or assistant).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// Represents a single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: Role,
    pub content: Vec<AnthropicContent>,
}

/// Represents different types of content blocks within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicContent {
    #[serde(rename = "text")]
    Text { text: String },
    // TODO: Potentially add other content types like Image in the future
}

/// The main request structure for the Anthropic Messages API.
#[derive(Debug, Clone, Serialize)]
pub struct AnthropicRequest<'a> {
    pub model: String,
    pub messages: &'a [AnthropicMessage],
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<&'a str>,
}

// --- Response & Streaming Structures ---

/// Represents an event received from the streaming API.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub delta: Option<AnthropicDelta>,
    #[serde(default)]
    pub message: Option<AnthropicResponseMessage>,
    #[serde(default)]
    pub error: Option<AnthropicError>,
    #[serde(default)]
    pub usage: Option<AnthropicUsage>, // Usage can appear in message_delta too
}

/// Represents a change (delta) in the streaming response, typically text content.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicDelta {
    #[serde(default)] // Use default if field is missing
    pub text: Option<String>,
    // Potentially other delta types in the future
}

/// Represents the complete message structure received in non-streaming or final stream events.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicResponseMessage {
    pub id: String,
    #[serde(default)]
    pub content: Vec<AnthropicResponseContent>,
    pub role: Role,
    pub model: String,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
    #[serde(default)]
    pub usage: Option<AnthropicUsage>,
}

/// Represents a content block within an API response message.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicResponseContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(default)]
    pub text: Option<String>,
}

/// Represents token usage information provided by the API.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicUsage {
    #[serde(default)] // input_tokens might not be present in all usage events (e.g., message_delta)
    pub input_tokens: Option<u32>, 
    pub output_tokens: u32,
}

/// Represents an error message from the Anthropic API.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicError {
    #[serde(rename = "type")] // Map the 'type' field in JSON to 'error_type'
    pub error_type: String,
    pub message: String,
} 