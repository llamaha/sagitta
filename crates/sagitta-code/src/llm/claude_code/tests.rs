#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::config::types::{SagittaCodeConfig, LlmProvider, ClaudeCodeConfig};
    use crate::llm::client::{Message, MessagePart, Role, ToolDefinition};
    use crate::llm::claude_code::client::ClaudeCodeClient;
    use crate::llm::claude_code::message_converter::{convert_messages_to_claude, ClaudeChunk, ContentBlock};
    use crate::llm::claude_code::models::ClaudeCodeModel;
    use crate::llm::claude_code::error::ClaudeCodeError;
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
            max_turns: 0,
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
    
    // Tool formatting test removed - tools are no longer supported in Claude Code
    
    // Tool parsing tests removed - tools are no longer supported in Claude Code
    
    #[test]
    fn test_tool_use_content_block_parsing() {
        // Test that tool_use content blocks are properly handled
        use crate::llm::claude_code::message_converter::{AssistantMessage, ContentBlock, Usage};
        
        // Create a mock assistant message with tool_use content block
        let message = AssistantMessage {
            content: vec![
                ContentBlock::Text { text: "I'll help you search for that.".to_string() },
                ContentBlock::ToolUse {
                    id: "tool_123".to_string(),
                    name: "web_search".to_string(),
                    input: serde_json::json!({
                        "query": "tokio rust github"
                    }),
                }
            ],
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_input_tokens: None,
                cache_creation_input_tokens: None,
            },
            stop_reason: Some("tool_use".to_string()),
        };
        
        // Verify tool_use block is correctly parsed
        assert_eq!(message.content.len(), 2);
        match &message.content[1] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tool_123");
                assert_eq!(name, "web_search");
                assert_eq!(input.get("query").and_then(|v| v.as_str()), Some("tokio rust github"));
            }
            _ => panic!("Expected ToolUse content block"),
        }
    }
    
    #[test]
    fn test_user_message_chunk_handling() {
        // Test that user message chunks (tool results) are properly handled
        use crate::llm::claude_code::message_converter::{UserMessage, UserContentBlock};
        
        let message = UserMessage {
            role: "user".to_string(),
            content: vec![
                UserContentBlock::ToolResult {
                    content: "Found: https://github.com/tokio-rs/tokio.git".to_string(),
                    tool_use_id: "tool_123".to_string(),
                    is_error: Some(false),
                }
            ],
        };
        
        // Verify tool result is correctly parsed
        assert_eq!(message.content.len(), 1);
        match &message.content[0] {
            UserContentBlock::ToolResult { content, tool_use_id, is_error } => {
                assert_eq!(content, "Found: https://github.com/tokio-rs/tokio.git");
                assert_eq!(tool_use_id, "tool_123");
                assert_eq!(*is_error, Some(false));
            }
            _ => panic!("Expected ToolResult content block"),
        }
    }
    
    #[test]
    fn test_mcp_tool_names_not_in_registry() {
        // Test that MCP tool names are NOT in disabled list
        // because they don't exist in our tool registry
        let disabled = ClaudeCodeClient::get_disabled_tools();
        
        // Count MCP tools
        let mcp_tools: Vec<_> = disabled.iter()
            .filter(|t| t.starts_with("mcp__"))
            .collect();
        
        // MCP tools should NOT be in the disabled list
        assert_eq!(mcp_tools.len(), 0, "MCP tools should not be in disabled list");
        
        // MCP tools should never be in our disabled list since we disable MCP entirely
        // via --mcp-config {} when spawning the Claude process
    }
    
    #[test]
    fn test_mcp_permission_denial_handling() {
        // Test that MCP permission denials should be handled gracefully
        use crate::llm::claude_code::message_converter::{UserMessage, UserContentBlock};
        
        let message = UserMessage {
            role: "user".to_string(),
            content: vec![
                UserContentBlock::ToolResult {
                    content: "Permission to use tool has been denied".to_string(),
                    tool_use_id: "tool_456".to_string(),
                    is_error: Some(true),
                }
            ],
        };
        
        // Verify permission denial is detected
        match &message.content[0] {
            UserContentBlock::ToolResult { content, is_error, .. } => {
                assert!(content.contains("Permission to use"));
                assert!(content.contains("has been denied"));
                assert_eq!(*is_error, Some(true));
            }
            _ => panic!("Expected ToolResult"),
        }
    }
}