use super::types::*;
use crate::llm::client::{Message, MessagePart, Role, ToolDefinition};
use crate::utils::errors::SagittaCodeError;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

pub struct OpenAITranslator;

impl OpenAITranslator {
    /// Convert tool definition to OpenAI function format
    pub fn tool_to_openai(tool: &ToolDefinition) -> OpenAITool {
        OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
        }
    }

    /// Convert internal messages to OpenAI format
    pub fn messages_to_openai(messages: &[Message]) -> Vec<OpenAIMessage> {
        messages
            .iter()
            .flat_map(|msg| Self::message_to_openai(msg))
            .collect()
    }

    /// Convert a single message to OpenAI format (may produce multiple messages)
    fn message_to_openai(message: &Message) -> Vec<OpenAIMessage> {
        let role = match message.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Function => "tool",
        };

        let mut openai_messages = Vec::new();

        // Collect text parts
        let text_parts: Vec<String> = message
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect();

        // Collect tool calls for assistant messages
        let tool_calls: Vec<OpenAIToolCall> = message
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::ToolCall {
                    tool_call_id,
                    name,
                    parameters,
                } => Some(OpenAIToolCall {
                    id: tool_call_id.clone(),
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionCall {
                        name: name.clone(),
                        arguments: serde_json::to_string(parameters).unwrap_or_default(),
                    },
                }),
                _ => None,
            })
            .collect();

        // Create main message if we have text or tool calls
        if !text_parts.is_empty() || !tool_calls.is_empty() {
            let content = if text_parts.is_empty() {
                None
            } else {
                Some(text_parts.join("\n"))
            };

            openai_messages.push(OpenAIMessage {
                role: role.to_string(),
                content,
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                },
                tool_call_id: None,
                name: None,
            });
        }

        // Handle tool results as separate messages
        for part in &message.parts {
            if let MessagePart::ToolResult {
                tool_call_id,
                name,
                result,
            } = part
            {
                openai_messages.push(OpenAIMessage {
                    role: "tool".to_string(),
                    content: Some(serde_json::to_string(result).unwrap_or_default()),
                    tool_calls: None,
                    tool_call_id: Some(tool_call_id.clone()),
                    name: Some(name.clone()),
                });
            }
        }

        openai_messages
    }

    /// Convert OpenAI message to internal format
    pub fn openai_to_message(openai_msg: &OpenAIMessage) -> Message {
        let role = match openai_msg.role.as_str() {
            "system" => Role::System,
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "tool" | "function" => Role::Function,
            _ => Role::User, // Default fallback
        };

        let mut parts = Vec::new();

        // Add text content if present
        if let Some(content) = &openai_msg.content {
            if !content.is_empty() {
                parts.push(MessagePart::Text {
                    text: content.clone(),
                });
            }
        }

        // Add tool calls if present
        if let Some(tool_calls) = &openai_msg.tool_calls {
            for tc in tool_calls {
                let parameters: Value = serde_json::from_str(&tc.function.arguments)
                    .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));

                parts.push(MessagePart::ToolCall {
                    tool_call_id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    parameters,
                });
            }
        }

        // Handle tool results
        if openai_msg.role == "tool" {
            if let (Some(tool_call_id), Some(name), Some(content)) = 
                (&openai_msg.tool_call_id, &openai_msg.name, &openai_msg.content) {
                let result: Value = serde_json::from_str(content)
                    .unwrap_or_else(|_| Value::String(content.clone()));
                
                parts.push(MessagePart::ToolResult {
                    tool_call_id: tool_call_id.clone(),
                    name: name.clone(),
                    result,
                });
            }
        }

        Message {
            id: Uuid::new_v4(),
            role,
            parts,
            metadata: HashMap::new(),
        }
    }

    /// Convert OpenAI chat response to internal message
    pub fn response_to_message(response: &OpenAIChatResponse) -> Result<Message, SagittaCodeError> {
        let choice = response
            .choices
            .first()
            .ok_or_else(|| SagittaCodeError::ParseError("No choices in response".to_string()))?;

        Ok(Self::openai_to_message(&choice.message))
    }

    /// Convert tool execution result to OpenAI tool message
    pub fn tool_result_to_openai(
        tool_call_id: &str,
        tool_name: &str,
        result: &Value,
    ) -> OpenAIMessage {
        OpenAIMessage {
            role: "tool".to_string(),
            content: Some(serde_json::to_string(result).unwrap_or_default()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
            name: Some(tool_name.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_to_openai() {
        let tool = ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather information".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string"
                    }
                }
            }),
            is_required: false,
        };

        let openai_tool = OpenAITranslator::tool_to_openai(&tool);

        assert_eq!(openai_tool.tool_type, "function");
        assert_eq!(openai_tool.function.name, "get_weather");
        assert_eq!(openai_tool.function.description, "Get weather information");
    }

    #[test]
    fn test_message_conversion() {
        let msg = Message {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            parts: vec![
                MessagePart::Text {
                    text: "I'll help you with that.".to_string(),
                },
                MessagePart::ToolCall {
                    tool_call_id: "call_123".to_string(),
                    name: "get_weather".to_string(),
                    parameters: serde_json::json!({"location": "NYC"}),
                },
            ],
            metadata: HashMap::new(),
        };

        let openai_msgs = OpenAITranslator::messages_to_openai(&[msg]);

        assert_eq!(openai_msgs.len(), 1);
        assert_eq!(openai_msgs[0].role, "assistant");
        assert_eq!(
            openai_msgs[0].content,
            Some("I'll help you with that.".to_string())
        );
        assert!(openai_msgs[0].tool_calls.is_some());
        assert_eq!(openai_msgs[0].tool_calls.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_tool_result_conversion() {
        let result = serde_json::json!({
            "temperature": 72,
            "condition": "sunny"
        });

        let msg = OpenAITranslator::tool_result_to_openai("call_123", "get_weather", &result);

        assert_eq!(msg.role, "tool");
        assert!(msg.content.is_some());
        assert_eq!(msg.tool_call_id, Some("call_123".to_string()));
        assert_eq!(msg.name, Some("get_weather".to_string()));
    }
}