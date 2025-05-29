// Message data structures will go here

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::llm::client::{Message as LlmMessage, MessagePart, Role};

/// A message in the agent's conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Unique identifier for the message
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    
    /// The role of the message sender
    pub role: Role,
    
    /// The message content
    pub content: String,
    
    /// Whether this message is being streamed (still incomplete)
    #[serde(default)]
    pub is_streaming: bool,
    
    /// Time when this message was created
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
    
    /// Additional metadata about the message
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
    
    /// Tool calls in this message
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

/// A tool call within a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for the tool call
    pub id: String,
    
    /// The name of the tool that was called
    pub name: String,
    
    /// The arguments passed to the tool
    pub arguments: serde_json::Value,
    
    /// The result of the tool call (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    
    /// Whether the tool call was successful
    #[serde(default)]
    pub successful: bool,
    
    /// Time when this tool call was executed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time: Option<DateTime<Utc>>,
}

impl AgentMessage {
    /// Create a new user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::User,
            content: content.into(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            tool_calls: Vec::new(),
        }
    }
    
    /// Create a new system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::System,
            content: content.into(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            tool_calls: Vec::new(),
        }
    }
    
    /// Create a new assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            content: content.into(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            tool_calls: Vec::new(),
        }
    }
    
    /// Create a new assistant message with streaming enabled
    pub fn assistant_streaming(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            content: content.into(),
            is_streaming: true,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            tool_calls: Vec::new(),
        }
    }
    
    /// Add a tool call to this message
    pub fn with_tool_call(mut self, tool_call: ToolCall) -> Self {
        self.tool_calls.push(tool_call);
        self
    }
    
    /// Add metadata to this message
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    
    /// Convert this AgentMessage to a LlmMessage for the LlmClient
    pub fn to_llm_message(&self) -> LlmMessage {
        let mut parts = Vec::new();
        
        // Add the content as text
        if !self.content.is_empty() {
            parts.push(MessagePart::Text { text: self.content.clone() });
        }
        
        // Add any tool call *requests* (where result is None)
        if self.role == Role::Assistant { // Tool call requests are only from assistants
            for tool_call in &self.tool_calls {
                if tool_call.result.is_none() {
                    parts.push(MessagePart::ToolCall {
                        tool_call_id: tool_call.id.clone(),
                        name: tool_call.name.clone(),
                        parameters: tool_call.arguments.clone(),
                    });
                }
            }
        }
        
        // Convert metadata from HashMap<String, String> to HashMap<String, Value>
        let llm_metadata: HashMap<String, serde_json::Value> = self.metadata.iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();

        LlmMessage {
            id: self.id,
            role: self.role.clone(),
            parts,
            metadata: llm_metadata, 
        }
    }
    
    /// Create an AgentMessage from a LlmMessage
    pub fn from_llm_message(message: &LlmMessage) -> Self {
        let mut content = String::new();
        let mut tool_calls = Vec::new();
        
        for part in &message.parts {
            match part {
                MessagePart::Text { text } => {
                    if !content.is_empty() && !text.is_empty() {
                        content.push(' ');
                    }
                    content.push_str(text);
                },
                MessagePart::Thought { text } => {
                    // For now, treat thoughts similar to text but maybe with a prefix
                    if !content.is_empty() && !text.is_empty() {
                        content.push(' ');
                    }
                    content.push_str("[THOUGHT] ");
                    content.push_str(text);
                },
                MessagePart::ToolCall { tool_call_id, name, parameters } => {
                    tool_calls.push(ToolCall {
                        id: tool_call_id.clone(),
                        name: name.clone(),
                        arguments: parameters.clone(),
                        result: None,
                        successful: false,
                        execution_time: None,
                    });
                },
                MessagePart::ToolResult { .. } => {
                    // Tool results are typically added separately after execution
                },
            }
        }
        
        Self {
            id: message.id,
            role: message.role.clone(),
            content,
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            tool_calls,
        }
    }

    /// Helper to check if this is a summary/finalization message
    pub fn is_summary(&self) -> bool {
        self.metadata.get("message_type").map(|v| v == "summary").unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_agent_message_user_creation() {
        let message = AgentMessage::user("Hello, world!");
        
        assert_eq!(message.role, Role::User);
        assert_eq!(message.content, "Hello, world!");
        assert!(!message.is_streaming);
        assert!(message.tool_calls.is_empty());
        assert!(message.metadata.is_empty());
    }

    #[test]
    fn test_agent_message_system_creation() {
        let message = AgentMessage::system("System prompt");
        
        assert_eq!(message.role, Role::System);
        assert_eq!(message.content, "System prompt");
        assert!(!message.is_streaming);
        assert!(message.tool_calls.is_empty());
        assert!(message.metadata.is_empty());
    }

    #[test]
    fn test_agent_message_assistant_creation() {
        let message = AgentMessage::assistant("Assistant response");
        
        assert_eq!(message.role, Role::Assistant);
        assert_eq!(message.content, "Assistant response");
        assert!(!message.is_streaming);
        assert!(message.tool_calls.is_empty());
        assert!(message.metadata.is_empty());
    }

    #[test]
    fn test_agent_message_assistant_streaming() {
        let message = AgentMessage::assistant_streaming("Streaming response");
        
        assert_eq!(message.role, Role::Assistant);
        assert_eq!(message.content, "Streaming response");
        assert!(message.is_streaming);
        assert!(message.tool_calls.is_empty());
        assert!(message.metadata.is_empty());
    }

    #[test]
    fn test_agent_message_with_tool_call() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "search_web".to_string(),
            arguments: json!({"query": "rust programming"}),
            result: None,
            successful: false,
            execution_time: None,
        };
        
        let message = AgentMessage::assistant("I'll search for that")
            .with_tool_call(tool_call.clone());
        
        assert_eq!(message.tool_calls.len(), 1);
        assert_eq!(message.tool_calls[0].id, "call_123");
        assert_eq!(message.tool_calls[0].name, "search_web");
    }

    #[test]
    fn test_agent_message_with_metadata() {
        let message = AgentMessage::user("Test message")
            .with_metadata("source", "test")
            .with_metadata("priority", "high");
        
        assert_eq!(message.metadata.len(), 2);
        assert_eq!(message.metadata.get("source"), Some(&"test".to_string()));
        assert_eq!(message.metadata.get("priority"), Some(&"high".to_string()));
    }

    #[test]
    fn test_agent_message_chaining() {
        let tool_call = ToolCall {
            id: "call_456".to_string(),
            name: "get_weather".to_string(),
            arguments: json!({"location": "New York"}),
            result: Some(json!({"temperature": 72})),
            successful: true,
            execution_time: Some(Utc::now()),
        };
        
        let message = AgentMessage::assistant("Weather info")
            .with_tool_call(tool_call)
            .with_metadata("type", "weather_response")
            .with_metadata("location", "NYC");
        
        assert_eq!(message.role, Role::Assistant);
        assert_eq!(message.content, "Weather info");
        assert_eq!(message.tool_calls.len(), 1);
        assert_eq!(message.metadata.len(), 2);
        assert_eq!(message.tool_calls[0].name, "get_weather");
        assert!(message.tool_calls[0].successful);
    }

    #[test]
    fn test_tool_call_creation() {
        let tool_call = ToolCall {
            id: "call_789".to_string(),
            name: "calculate".to_string(),
            arguments: json!({"expression": "2 + 2"}),
            result: Some(json!({"result": 4})),
            successful: true,
            execution_time: Some(Utc::now()),
        };
        
        assert_eq!(tool_call.id, "call_789");
        assert_eq!(tool_call.name, "calculate");
        assert_eq!(tool_call.arguments, json!({"expression": "2 + 2"}));
        assert_eq!(tool_call.result, Some(json!({"result": 4})));
        assert!(tool_call.successful);
        assert!(tool_call.execution_time.is_some());
    }

    #[test]
    fn test_agent_message_to_llm_message_text_only() {
        let message = AgentMessage::user("Simple text message");
        let llm_message = message.to_llm_message();
        
        assert_eq!(llm_message.role, Role::User);
        assert_eq!(llm_message.parts.len(), 1);
        
        if let MessagePart::Text { text } = &llm_message.parts[0] {
            assert_eq!(text, "Simple text message");
        } else {
            panic!("Expected text part");
        }
    }

    #[test]
    fn test_agent_message_to_llm_message_empty_content() {
        let message = AgentMessage::assistant("");
        let llm_message = message.to_llm_message();
        
        assert_eq!(llm_message.role, Role::Assistant);
        assert!(llm_message.parts.is_empty()); // Empty content should result in no parts
    }

    #[test]
    fn test_agent_message_to_llm_message_with_tool_calls() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "search".to_string(),
            arguments: json!({"query": "test"}),
            result: None, // No result yet
            successful: false,
            execution_time: None,
        };
        
        let message = AgentMessage::assistant("I'll search for that")
            .with_tool_call(tool_call);
        
        let llm_message = message.to_llm_message();
        
        assert_eq!(llm_message.parts.len(), 2); // Text + tool call
        
        // Check text part
        if let MessagePart::Text { text } = &llm_message.parts[0] {
            assert_eq!(text, "I'll search for that");
        } else {
            panic!("Expected text part");
        }
        
        // Check tool call part
        if let MessagePart::ToolCall { tool_call_id, name, parameters } = &llm_message.parts[1] {
            assert_eq!(tool_call_id, "call_123");
            assert_eq!(name, "search");
            assert_eq!(parameters, &json!({"query": "test"}));
        } else {
            panic!("Expected tool call part");
        }
    }

    #[test]
    fn test_agent_message_to_llm_message_with_completed_tool_calls() {
        let tool_call = ToolCall {
            id: "call_456".to_string(),
            name: "calculate".to_string(),
            arguments: json!({"expression": "1+1"}),
            result: Some(json!({"result": 2})), // Has result
            successful: true,
            execution_time: Some(Utc::now()),
        };
        
        let message = AgentMessage::assistant("Calculation complete")
            .with_tool_call(tool_call);
        
        let llm_message = message.to_llm_message();
        
        // Should only have text part, not tool call part (since result exists)
        assert_eq!(llm_message.parts.len(), 1);
        
        if let MessagePart::Text { text } = &llm_message.parts[0] {
            assert_eq!(text, "Calculation complete");
        } else {
            panic!("Expected text part");
        }
    }

    #[test]
    fn test_agent_message_to_llm_message_user_with_tool_calls() {
        let tool_call = ToolCall {
            id: "call_789".to_string(),
            name: "user_tool".to_string(),
            arguments: json!({"input": "test"}),
            result: None,
            successful: false,
            execution_time: None,
        };
        
        let message = AgentMessage::user("User message")
            .with_tool_call(tool_call);
        
        let llm_message = message.to_llm_message();
        
        // User messages should not include tool calls in LLM format
        assert_eq!(llm_message.parts.len(), 1);
        
        if let MessagePart::Text { text } = &llm_message.parts[0] {
            assert_eq!(text, "User message");
        } else {
            panic!("Expected text part");
        }
    }

    #[test]
    fn test_agent_message_to_llm_message_metadata_conversion() {
        let message = AgentMessage::user("Test")
            .with_metadata("key1", "value1")
            .with_metadata("key2", "value2");
        
        let llm_message = message.to_llm_message();
        
        assert_eq!(llm_message.metadata.len(), 2);
        assert_eq!(llm_message.metadata.get("key1"), Some(&json!("value1")));
        assert_eq!(llm_message.metadata.get("key2"), Some(&json!("value2")));
    }

    #[test]
    fn test_agent_message_from_llm_message_text_only() {
        let llm_message = LlmMessage {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![MessagePart::Text { text: "Hello from LLM".to_string() }],
            metadata: HashMap::new(),
        };
        
        let agent_message = AgentMessage::from_llm_message(&llm_message);
        
        assert_eq!(agent_message.id, llm_message.id);
        assert_eq!(agent_message.role, Role::User);
        assert_eq!(agent_message.content, "Hello from LLM");
        assert!(!agent_message.is_streaming);
        assert!(agent_message.tool_calls.is_empty());
    }

    #[test]
    fn test_agent_message_from_llm_message_multiple_text_parts() {
        let llm_message = LlmMessage {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            parts: vec![
                MessagePart::Text { text: "First part".to_string() },
                MessagePart::Text { text: "Second part".to_string() },
            ],
            metadata: HashMap::new(),
        };
        
        let agent_message = AgentMessage::from_llm_message(&llm_message);
        
        assert_eq!(agent_message.content, "First part Second part");
    }

    #[test]
    fn test_agent_message_from_llm_message_with_tool_calls() {
        let llm_message = LlmMessage {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            parts: vec![
                MessagePart::Text { text: "I'll help you".to_string() },
                MessagePart::ToolCall {
                    tool_call_id: "call_123".to_string(),
                    name: "search".to_string(),
                    parameters: json!({"query": "rust"}),
                },
            ],
            metadata: HashMap::new(),
        };
        
        let agent_message = AgentMessage::from_llm_message(&llm_message);
        
        assert_eq!(agent_message.content, "I'll help you");
        assert_eq!(agent_message.tool_calls.len(), 1);
        assert_eq!(agent_message.tool_calls[0].id, "call_123");
        assert_eq!(agent_message.tool_calls[0].name, "search");
        assert_eq!(agent_message.tool_calls[0].arguments, json!({"query": "rust"}));
        assert!(agent_message.tool_calls[0].result.is_none());
        assert!(!agent_message.tool_calls[0].successful);
    }

    #[test]
    fn test_agent_message_from_llm_message_with_tool_results() {
        let llm_message = LlmMessage {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![
                MessagePart::ToolResult {
                    tool_call_id: "call_456".to_string(),
                    name: "search".to_string(),
                    result: json!({"results": ["item1", "item2"]}),
                },
            ],
            metadata: HashMap::new(),
        };
        
        let agent_message = AgentMessage::from_llm_message(&llm_message);
        
        // Tool results are typically added separately, so content should be empty
        assert_eq!(agent_message.content, "");
        assert!(agent_message.tool_calls.is_empty());
    }

    #[test]
    fn test_agent_message_from_llm_message_empty_parts() {
        let llm_message = LlmMessage {
            id: Uuid::new_v4(),
            role: Role::System,
            parts: vec![],
            metadata: HashMap::new(),
        };
        
        let agent_message = AgentMessage::from_llm_message(&llm_message);
        
        assert_eq!(agent_message.content, "");
        assert!(agent_message.tool_calls.is_empty());
    }

    #[test]
    fn test_agent_message_serialization() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            arguments: json!({"param": "value"}),
            result: Some(json!({"output": "success"})),
            successful: true,
            execution_time: Some(Utc::now()),
        };
        
        let message = AgentMessage::assistant("Test message")
            .with_tool_call(tool_call)
            .with_metadata("test_key", "test_value");
        
        // Test serialization
        let serialized = serde_json::to_string(&message).unwrap();
        assert!(serialized.contains("Test message"));
        assert!(serialized.contains("test_tool"));
        assert!(serialized.contains("test_key"));
        
        // Test deserialization
        let deserialized: AgentMessage = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.content, message.content);
        assert_eq!(deserialized.tool_calls.len(), message.tool_calls.len());
        assert_eq!(deserialized.metadata.len(), message.metadata.len());
    }

    #[test]
    fn test_tool_call_serialization() {
        let tool_call = ToolCall {
            id: "call_456".to_string(),
            name: "serialize_test".to_string(),
            arguments: json!({"input": "test", "number": 42}),
            result: Some(json!({"status": "ok"})),
            successful: true,
            execution_time: Some(Utc::now()),
        };
        
        // Test serialization
        let serialized = serde_json::to_string(&tool_call).unwrap();
        assert!(serialized.contains("serialize_test"));
        assert!(serialized.contains("call_456"));
        
        // Test deserialization
        let deserialized: ToolCall = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.id, tool_call.id);
        assert_eq!(deserialized.name, tool_call.name);
        assert_eq!(deserialized.arguments, tool_call.arguments);
        assert_eq!(deserialized.successful, tool_call.successful);
    }

    #[test]
    fn test_agent_message_default_values() {
        let message = AgentMessage {
            id: Uuid::new_v4(),
            role: Role::User,
            content: "Test".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            tool_calls: Vec::new(),
        };
        
        // Test that default values work correctly
        assert!(!message.is_streaming);
        assert!(message.metadata.is_empty());
        assert!(message.tool_calls.is_empty());
    }

    #[test]
    fn test_tool_call_default_values() {
        let tool_call = ToolCall {
            id: "test".to_string(),
            name: "test".to_string(),
            arguments: json!({}),
            result: None,
            successful: false,
            execution_time: None,
        };
        
        // Test that default values work correctly
        assert!(!tool_call.successful);
        assert!(tool_call.result.is_none());
        assert!(tool_call.execution_time.is_none());
    }

    #[test]
    fn test_agent_message_clone() {
        let original = AgentMessage::user("Original message")
            .with_metadata("key", "value");
        
        let cloned = original.clone();
        
        assert_eq!(original.id, cloned.id);
        assert_eq!(original.content, cloned.content);
        assert_eq!(original.metadata, cloned.metadata);
    }

    #[test]
    fn test_tool_call_clone() {
        let original = ToolCall {
            id: "original".to_string(),
            name: "test".to_string(),
            arguments: json!({"test": true}),
            result: Some(json!({"success": true})),
            successful: true,
            execution_time: Some(Utc::now()),
        };
        
        let cloned = original.clone();
        
        assert_eq!(original.id, cloned.id);
        assert_eq!(original.name, cloned.name);
        assert_eq!(original.arguments, cloned.arguments);
        assert_eq!(original.successful, cloned.successful);
    }

    #[test]
    fn test_agent_message_debug_format() {
        let message = AgentMessage::user("Debug test");
        let debug_str = format!("{:?}", message);
        
        assert!(debug_str.contains("AgentMessage"));
        assert!(debug_str.contains("Debug test"));
        assert!(debug_str.contains("User"));
    }

    #[test]
    fn test_tool_call_debug_format() {
        let tool_call = ToolCall {
            id: "debug_test".to_string(),
            name: "debug_tool".to_string(),
            arguments: json!({"debug": true}),
            result: None,
            successful: false,
            execution_time: None,
        };
        
        let debug_str = format!("{:?}", tool_call);
        
        assert!(debug_str.contains("ToolCall"));
        assert!(debug_str.contains("debug_test"));
        assert!(debug_str.contains("debug_tool"));
    }
}

