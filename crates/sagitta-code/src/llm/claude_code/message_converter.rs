use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::llm::client::{Message, MessagePart, Role};
use std::io::Write;

/// Claude Code message format for stdin streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessage {
    pub role: String,
    pub content: Vec<ClaudeMessageContent>,
}

/// Content block for Claude messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClaudeMessageContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_result")]
    ToolResult { 
        tool_use_id: String,
        content: String,
    },
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
        
        let mut content_blocks = Vec::new();
        
        for part in &msg.parts {
            match part {
                MessagePart::Text { text } => {
                    if !text.trim().is_empty() {
                        content_blocks.push(ClaudeMessageContent::Text { 
                            text: text.clone() 
                        });
                    }
                }
                MessagePart::Thought { text } => {
                    content_blocks.push(ClaudeMessageContent::Text { 
                        text: format!("<thinking>{text}</thinking>") 
                    });
                }
                MessagePart::ToolCall { tool_call_id, name, parameters } => {
                    // Convert tool calls to text format for Claude Code
                    let params_str = serde_json::to_string_pretty(parameters).unwrap_or_default();
                    content_blocks.push(ClaudeMessageContent::Text { 
                        text: format!("Tool Call [{tool_call_id}]: {name} with parameters:\n{params_str}") 
                    });
                }
                MessagePart::ToolResult { tool_call_id, name: _, result } => {
                    // Tool results should use the tool_result content type
                    let result_str = match result {
                        Value::String(s) => s.clone(),
                        _ => serde_json::to_string_pretty(result).unwrap_or_default(),
                    };
                    content_blocks.push(ClaudeMessageContent::ToolResult { 
                        tool_use_id: tool_call_id.clone(),
                        content: result_str,
                    });
                }
            }
        }
        
        if content_blocks.is_empty() {
            log::debug!("CLAUDE_CODE: Skipping message with empty content for role: {:?}", msg.role);
            None
        } else {
            Some(ClaudeMessage {
                role: role.to_string(),
                content: content_blocks,
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
        #[serde(deserialize_with = "deserialize_tool_result_content")]
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

/// Custom deserializer for tool result content that can be either a string or an array
fn deserialize_tool_result_content<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    
    struct ContentVisitor;
    
    impl<'de> Visitor<'de> for ContentVisitor {
        type Value = String;
        
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or an array of content blocks")
        }
        
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }
        
        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value)
        }
        
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut parts = Vec::new();
            
            while let Some(value) = seq.next_element::<serde_json::Value>()? {
                if let Some(obj) = value.as_object() {
                    if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                        parts.push(text.to_string());
                    }
                }
            }
            
            Ok(parts.join("\n"))
        }
    }
    
    deserializer.deserialize_any(ContentVisitor)
}

/// Stream a message as JSON to a writer
pub fn stream_message_as_json<W: Write>(
    message: &ClaudeMessage,
    writer: &mut W
) -> Result<(), std::io::Error> {
    serde_json::to_writer(&mut *writer, message)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

