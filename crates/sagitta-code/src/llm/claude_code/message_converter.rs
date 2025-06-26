use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::llm::client::{Message, MessagePart, Role};

/// Claude Code message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessage {
    pub role: String,
    pub content: String,
}

/// Convert our internal message format to Claude Code format
pub fn convert_messages_to_claude(messages: &[Message]) -> Vec<ClaudeMessage> {
    messages.iter().filter_map(|msg| {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Function => "user", // Claude doesn't have a function role
        };
        
        let content = msg.parts.iter()
            .filter_map(|part| match part {
                MessagePart::Text { text } => Some(text.clone()),
                MessagePart::Thought { text } => Some(format!("<thinking>{}</thinking>", text)),
                MessagePart::ToolCall { tool_call_id, name, parameters } => {
                    // Convert tool calls to text format for Claude Code
                    let params_str = serde_json::to_string_pretty(parameters).unwrap_or_default();
                    Some(format!("Tool Call [{}]: {} with parameters:\n{}", tool_call_id, name, params_str))
                }
                MessagePart::ToolResult { tool_call_id, name, result } => {
                    let result_str = serde_json::to_string_pretty(result).unwrap_or_default();
                    Some(format!("Tool Result [{}] for {}: {}", tool_call_id, name, result_str))
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        
        if content.trim().is_empty() {
            log::debug!("CLAUDE_CODE: Skipping message with empty content for role: {:?}", msg.role);
            None
        } else {
            Some(ClaudeMessage {
                role: role.to_string(),
                content,
            })
        }
    }).collect()
}

/// Parse Claude Code output chunk types
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ClaudeChunk {
    #[serde(rename = "system")]
    System {
        subtype: String,
        #[serde(rename = "apiKeySource")]
        api_key_source: Option<String>,
    },
    #[serde(rename = "assistant")]
    Assistant {
        message: AssistantMessage,
    },
    #[serde(rename = "result")]
    Result {
        #[serde(default)]
        result: Option<serde_json::Value>, // Made optional to handle different result formats
        #[serde(rename = "total_cost_usd")]
        total_cost_usd: Option<f64>,
        #[serde(default)]
        subtype: Option<String>,
        #[serde(default)]
        is_error: Option<bool>,
    },
    #[serde(rename = "user")]
    User {
        message: UserMessage,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
    pub usage: Usage,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserMessage {
    pub role: String,
    pub content: Vec<UserContentBlock>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum UserContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_result")]
    ToolResult {
        content: String,
        tool_use_id: String,
        is_error: Option<bool>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking,
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_read_input_tokens: Option<i32>,
    pub cache_creation_input_tokens: Option<i32>,
}

