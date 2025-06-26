#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::config::types::{SagittaCodeConfig, LlmProvider, ClaudeCodeConfig};
    use crate::llm::client::{Message, MessagePart, Role, ToolDefinition};
    use crate::llm::claude_code::client::ClaudeCodeClient;
    use crate::llm::claude_code::message_converter::{convert_messages_to_claude, ClaudeChunk, ContentBlock};
    use crate::llm::claude_code::models::ClaudeCodeModel;
    use crate::llm::claude_code::error::ClaudeCodeError;
    use crate::llm::claude_code::tool_parser::parse_tool_calls_from_text;
    use std::collections::HashMap;
    use uuid::Uuid;
    use serde_json::json;
    
    fn create_test_config() -> SagittaCodeConfig {
        let mut config = SagittaCodeConfig::default();
        config.provider = LlmProvider::ClaudeCode;
        config.claude_code = ClaudeCodeConfig {
            claude_path: "claude".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            max_output_tokens: 1000,
            verbose: false,
            timeout: 30,
        };
        config
    }
    
    fn create_test_message(role: Role, text: &str) -> Message {
        Message {
            id: Uuid::new_v4(),
            role,
            parts: vec![MessagePart::Text { text: text.to_string() }],
            metadata: HashMap::new(),
        }
    }
    
    #[test]
    fn test_claude_code_model_find_by_id() {
        // Test finding valid models
        let model = ClaudeCodeModel::find_by_id("claude-sonnet-4-20250514");
        assert!(model.is_some());
        assert_eq!(model.unwrap().name, "Claude 4 Sonnet");
        
        let model = ClaudeCodeModel::find_by_id("claude-opus-4-20250514");
        assert!(model.is_some());
        assert_eq!(model.unwrap().name, "Claude 4 Opus");
        
        // Test finding invalid model
        let model = ClaudeCodeModel::find_by_id("invalid-model");
        assert!(model.is_none());
    }
    
    #[test]
    fn test_claude_code_model_default() {
        let default_model = ClaudeCodeModel::default();
        assert_eq!(default_model.id, "claude-sonnet-4-20250514");
        assert_eq!(default_model.name, "Claude 4 Sonnet");
        assert_eq!(default_model.context_window, 200000);
    }
    
    #[test]
    fn test_message_conversion() {
        let messages = vec![
            create_test_message(Role::System, "You are a helpful assistant"),
            create_test_message(Role::User, "Hello"),
            create_test_message(Role::Assistant, "Hi there!"),
        ];
        
        let claude_messages = convert_messages_to_claude(&messages);
        
        assert_eq!(claude_messages.len(), 3);
        assert_eq!(claude_messages[0].role, "system");
        assert_eq!(claude_messages[0].content, "You are a helpful assistant");
        assert_eq!(claude_messages[1].role, "user");
        assert_eq!(claude_messages[1].content, "Hello");
        assert_eq!(claude_messages[2].role, "assistant");
        assert_eq!(claude_messages[2].content, "Hi there!");
    }
    
    #[test]
    fn test_message_conversion_with_empty_content() {
        let messages = vec![
            create_test_message(Role::User, ""),
            create_test_message(Role::Assistant, "Response"),
        ];
        
        let claude_messages = convert_messages_to_claude(&messages);
        
        // Empty messages should be filtered out
        assert_eq!(claude_messages.len(), 1);
        assert_eq!(claude_messages[0].role, "assistant");
        assert_eq!(claude_messages[0].content, "Response");
    }
    
    #[test]
    fn test_message_conversion_with_tool_calls() {
        let mut message = Message {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            parts: vec![
                MessagePart::Text { text: "Let me search for that.".to_string() },
                MessagePart::ToolCall {
                    tool_call_id: "call_123".to_string(),
                    name: "search".to_string(),
                    parameters: serde_json::json!({"query": "test"}),
                },
            ],
            metadata: HashMap::new(),
        };
        
        let claude_messages = convert_messages_to_claude(&[message]);
        
        assert_eq!(claude_messages.len(), 1);
        assert!(claude_messages[0].content.contains("Let me search for that"));
        assert!(claude_messages[0].content.contains("Tool Call [call_123]: search"));
        assert!(claude_messages[0].content.contains("query"));
    }
    
    #[test]
    fn test_message_conversion_with_thought() {
        let mut message = Message {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            parts: vec![
                MessagePart::Thought { text: "I need to think about this".to_string() },
                MessagePart::Text { text: "Here's my answer".to_string() },
            ],
            metadata: HashMap::new(),
        };
        
        let claude_messages = convert_messages_to_claude(&[message]);
        
        assert_eq!(claude_messages.len(), 1);
        assert!(claude_messages[0].content.contains("<thinking>I need to think about this</thinking>"));
        assert!(claude_messages[0].content.contains("Here's my answer"));
    }
    
    #[test]
    fn test_claude_code_error_from_json_error() {
        let json_err = serde_json::from_str::<ClaudeChunk>("invalid json").unwrap_err();
        let claude_err: ClaudeCodeError = json_err.into();
        
        match claude_err {
            ClaudeCodeError::ParseError(msg) => {
                assert!(msg.contains("expected"));
            }
            _ => panic!("Expected ParseError"),
        }
    }
    
    #[test]
    fn test_extract_system_prompt() {
        let messages = vec![
            create_test_message(Role::System, "System prompt"),
            create_test_message(Role::User, "User message"),
        ];
        
        let system_prompt = ClaudeCodeClient::extract_system_prompt(&messages);
        assert_eq!(system_prompt, "System prompt");
    }
    
    #[test]
    fn test_extract_system_prompt_no_system_message() {
        let messages = vec![
            create_test_message(Role::User, "User message"),
        ];
        
        let system_prompt = ClaudeCodeClient::extract_system_prompt(&messages);
        assert_eq!(system_prompt, "You are a helpful AI assistant.");
    }
    
    #[test]
    fn test_filter_non_system_messages() {
        let messages = vec![
            create_test_message(Role::System, "System prompt"),
            create_test_message(Role::User, "User message"),
            create_test_message(Role::Assistant, "Assistant message"),
        ];
        
        let filtered = ClaudeCodeClient::filter_non_system_messages(&messages);
        assert_eq!(filtered.len(), 2);
        assert!(matches!(filtered[0].role, Role::User));
        assert!(matches!(filtered[1].role, Role::Assistant));
    }
    
    #[test]
    fn test_get_disabled_tools() {
        let tools = ClaudeCodeClient::get_disabled_tools();
        assert!(tools.contains(&"Task".to_string()));
        assert!(tools.contains(&"Bash".to_string()));
        assert!(tools.contains(&"Read".to_string()));
        assert!(tools.contains(&"WebSearch".to_string()));
        assert!(tools.len() > 10); // Should have many tools
    }
    
    #[test]
    fn test_claude_code_client_creation_invalid_model() {
        let mut config = create_test_config();
        config.claude_code.model = "invalid-model".to_string();
        
        let result = ClaudeCodeClient::new(&config);
        assert!(result.is_err());
        
        if let Err(err) = result {
            match err {
                crate::utils::errors::SagittaCodeError::ConfigError(msg) => {
                    assert!(msg.contains("Unknown Claude model"));
                }
                _ => panic!("Expected ConfigError"),
            }
        }
    }
    
    #[test]
    fn test_claude_chunk_deserialization() {
        // Test system chunk
        let json = r#"{"type": "system", "subtype": "init", "apiKeySource": "none"}"#;
        let chunk: ClaudeChunk = serde_json::from_str(json).unwrap();
        
        match chunk {
            ClaudeChunk::System { subtype, api_key_source } => {
                assert_eq!(subtype, "init");
                assert_eq!(api_key_source, Some("none".to_string()));
            }
            _ => panic!("Expected System chunk"),
        }
        
        // Test assistant chunk
        let json = r#"{
            "type": "assistant",
            "message": {
                "content": [{"type": "text", "text": "Hello"}],
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 5,
                    "cache_read_input_tokens": 0,
                    "cache_creation_input_tokens": 0
                },
                "stop_reason": "stop"
            }
        }"#;
        
        let chunk: ClaudeChunk = serde_json::from_str(json).unwrap();
        
        match chunk {
            ClaudeChunk::Assistant { message } => {
                assert_eq!(message.content.len(), 1);
                assert_eq!(message.usage.input_tokens, 10);
                assert_eq!(message.usage.output_tokens, 5);
                assert_eq!(message.stop_reason, Some("stop".to_string()));
            }
            _ => panic!("Expected Assistant chunk"),
        }
    }
    
    #[test]
    fn test_content_block_deserialization() {
        // Test text block
        let json = r#"{"type": "text", "text": "Hello world"}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        
        match block {
            ContentBlock::Text { text } => {
                assert_eq!(text, "Hello world");
            }
            _ => panic!("Expected Text block"),
        }
        
        // Test thinking block
        let json = r#"{"type": "thinking", "thinking": "Let me think..."}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        
        match block {
            ContentBlock::Thinking { thinking } => {
                assert_eq!(thinking, "Let me think...");
            }
            _ => panic!("Expected Thinking block"),
        }
        
        // Test redacted thinking
        let json = r#"{"type": "redacted_thinking"}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        
        match block {
            ContentBlock::RedactedThinking => {}
            _ => panic!("Expected RedactedThinking block"),
        }
    }
    
    #[test]
    fn test_format_tools_for_system_prompt() {
        let tools = vec![
            ToolDefinition {
                name: "search".to_string(),
                description: "Search for information".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum results"
                        }
                    },
                    "required": ["query"]
                }),
                is_required: false,
            },
        ];
        
        let prompt = ClaudeCodeClient::format_tools_for_system_prompt(&tools);
        assert!(prompt.contains("## Available Tools"));
        assert!(prompt.contains("### search"));
        assert!(prompt.contains("<search>"));
        assert!(prompt.contains("<query>value</query>"));
        assert!(prompt.contains("<limit>123</limit>"));
        assert!(prompt.contains("</search>"));
        
        // Check for sequential tool execution rules
        assert!(prompt.contains("CRITICAL TOOL USAGE RULES"));
        assert!(prompt.contains("You MUST use only ONE tool per response"));
        assert!(prompt.contains("After using a tool, wait for the result before proceeding"));
        assert!(prompt.contains("Never attempt to use multiple tools in a single response"));
    }
    
    #[test]
    fn test_parse_tool_calls_from_text() {
        let text = "Let me search for that. <search><query>rust documentation</query></search> I found some results.";
        
        let (remaining_text, tool_calls) = parse_tool_calls_from_text(text);
        
        assert_eq!(remaining_text, "Let me search for that.  I found some results.");
        assert_eq!(tool_calls.len(), 1);
        
        match &tool_calls[0] {
            MessagePart::ToolCall { name, parameters, .. } => {
                assert_eq!(name, "search");
                assert_eq!(parameters.get("query").and_then(|v| v.as_str()), Some("rust documentation"));
            }
            _ => panic!("Expected ToolCall"),
        }
    }
    
    #[test]
    fn test_parse_multiple_tool_calls() {
        let text = "<read_file><path>test.rs</path></read_file> Then <edit_file><path>test.rs</path><content>new content</content></edit_file>";
        
        let (remaining_text, tool_calls) = parse_tool_calls_from_text(text);
        
        assert_eq!(remaining_text, "Then");
        assert_eq!(tool_calls.len(), 2);
        
        match &tool_calls[0] {
            MessagePart::ToolCall { name, .. } => assert_eq!(name, "read_file"),
            _ => panic!("Expected ToolCall"),
        }
        
        match &tool_calls[1] {
            MessagePart::ToolCall { name, .. } => assert_eq!(name, "edit_file"),
            _ => panic!("Expected ToolCall"),
        }
    }
    
    #[test]
    fn test_parse_tool_calls_with_comments() {
        let text = "<search><query>test <!-- search query --></query><limit>10 <!-- number of results --></limit></search>";
        
        let (_, tool_calls) = parse_tool_calls_from_text(text);
        
        assert_eq!(tool_calls.len(), 1);
        match &tool_calls[0] {
            MessagePart::ToolCall { parameters, .. } => {
                assert_eq!(parameters.get("query").and_then(|v| v.as_str()), Some("test"));
                assert_eq!(parameters.get("limit").and_then(|v| v.as_i64()), Some(10));
            }
            _ => panic!("Expected ToolCall"),
        }
    }
    
    #[test]
    fn test_parse_tool_calls_with_mixed_types() {
        let text = "<config><enabled>true</enabled><count>42</count><rate>3.14</rate><items>[\"a\", \"b\", \"c\"]</items></config>";
        
        let (_, tool_calls) = parse_tool_calls_from_text(text);
        
        assert_eq!(tool_calls.len(), 1);
        match &tool_calls[0] {
            MessagePart::ToolCall { parameters, .. } => {
                assert_eq!(parameters.get("enabled").and_then(|v| v.as_bool()), Some(true));
                assert_eq!(parameters.get("count").and_then(|v| v.as_i64()), Some(42));
                assert_eq!(parameters.get("rate").and_then(|v| v.as_f64()), Some(3.14));
                assert!(parameters.get("items").and_then(|v| v.as_array()).is_some());
            }
            _ => panic!("Expected ToolCall"),
        }
    }
}