// Chat UI module
pub mod input;
pub mod view;

use view::{StreamingMessage, MessageAuthor, MessageStatus, ToolCall, MessageType};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

/// Example chat manager that demonstrates proper streaming usage
pub struct StreamingChatManager {
    messages: Arc<Mutex<Vec<StreamingMessage>>>,
    active_streams: Arc<Mutex<HashMap<String, StreamingMessage>>>,
}

impl StreamingChatManager {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            active_streams: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Add a complete message directly to the main list
    pub fn add_complete_message(&self, mut message: StreamingMessage) {
        if message.message_type != MessageType::Summary && message.message_type != MessageType::Tool && message.message_type != MessageType::System {
            message.message_type = MessageType::Normal;
        }
        let mut messages = self.messages.lock().unwrap();
        messages.push(message);
    }
    
    /// Add a user message
    pub fn add_user_message(&self, content: String) -> String {
        let mut message = StreamingMessage::from_text(MessageAuthor::User, content);
        message.message_type = MessageType::Normal;
        let id = message.id.clone();
        
        {
            let mut messages = self.messages.lock().unwrap();
            messages.push(message);
        }
        
        id
    }
    
    /// Start a new streaming response from the agent
    pub fn start_agent_response(&self) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let mut message = StreamingMessage::new_streaming(MessageAuthor::Agent, id.clone());
        message.message_type = MessageType::Normal;
        
        {
            let mut active_streams = self.active_streams.lock().unwrap();
            active_streams.insert(id.clone(), message);
        }
        
        id
    }
    
    /// Set thinking mode for a streaming message
    pub fn set_thinking(&self, message_id: &str, thinking_content: String) {
        let mut active_streams = self.active_streams.lock().unwrap();
        if let Some(message) = active_streams.get_mut(message_id) {
            message.set_thinking(thinking_content);
        }
    }
    
    /// Append content to a streaming message
    pub fn append_content(&self, message_id: &str, chunk: String) {
        log::info!("StreamingChatManager::append_content CALLED for ID: '{}', chunk: '{}'", message_id, chunk.chars().take(70).collect::<String>());
        let mut active_streams = self.active_streams.lock().unwrap();
        if let Some(message) = active_streams.get_mut(message_id) {
            log::info!("StreamingChatManager::append_content - Found active stream for ID: '{}'", message_id);
            // If we're thinking, switch to streaming but preserve thinking content
            if message.is_thinking() {
                message.status = MessageStatus::Streaming;
                // Don't clear thinking_content - it should remain visible
            }
            message.append_content(&chunk);
        } else {
            log::warn!("StreamingChatManager::append_content - NO active stream found for ID: '{}'", message_id);
        }
    }
    
    /// Append thinking content to a streaming message (keeps it separate from regular content)
    pub fn append_thinking(&self, message_id: &str, thinking_chunk: String) {
        let mut active_streams = self.active_streams.lock().unwrap();
        if let Some(message) = active_streams.get_mut(message_id) {
            // Use the new streaming thinking functionality
            if !message.thinking_is_streaming {
                message.start_thinking_stream();
            }
            message.append_thinking_stream(&thinking_chunk);
        }
    }
    
    /// Start streaming thinking for a message
    pub fn start_thinking_stream(&self, message_id: &str) {
        let mut active_streams = self.active_streams.lock().unwrap();
        if let Some(message) = active_streams.get_mut(message_id) {
            message.start_thinking_stream();
        }
    }
    
    /// Finish streaming thinking for a message
    pub fn finish_thinking_stream(&self, message_id: &str) {
        let mut active_streams = self.active_streams.lock().unwrap();
        if let Some(message) = active_streams.get_mut(message_id) {
            message.finish_thinking_stream();
        }
    }
    
    /// Add a tool call to a streaming message
    pub fn add_tool_call(&self, message_id: &str, tool_call: ToolCall) {
        log::info!("StreamingChatManager::add_tool_call CALLED for message_id: '{}', tool_name: '{}', args: '{}'", message_id, tool_call.name, tool_call.arguments.chars().take(100).collect::<String>());
        let mut active_streams = self.active_streams.lock().unwrap();
        if let Some(message) = active_streams.get_mut(message_id) {
            message.add_tool_call(tool_call);
        } else {
            log::warn!("StreamingChatManager::add_tool_call - NO active stream found for message_id: '{}'", message_id);
        }
    }
    
    /// Finish streaming and move message to main list
    pub fn finish_streaming(&self, message_id: &str) {
        let mut active_streams = self.active_streams.lock().unwrap();
        if let Some(mut message) = active_streams.remove(message_id) {
            message.finish_streaming();
            
            let mut messages = self.messages.lock().unwrap();
            messages.push(message);
        }
    }
    
    /// Set error status for a streaming message
    pub fn set_error(&self, message_id: &str, error: String) {
        let mut active_streams = self.active_streams.lock().unwrap();
        if let Some(message) = active_streams.get_mut(message_id) {
            message.set_error(error);
            
            // Move to main messages even if error
            let message = active_streams.remove(message_id).unwrap();
            let mut messages = self.messages.lock().unwrap();
            messages.push(message);
        }
    }
    
    /// Get all messages for display (includes active streams)
    pub fn get_all_messages(&self) -> Vec<StreamingMessage> {
        let messages = self.messages.lock().unwrap();
        let active_streams = self.active_streams.lock().unwrap();
        
        let mut all_messages = messages.clone();
        
        // Add active streaming messages
        for message in active_streams.values() {
            all_messages.push(message.clone());
        }
        
        // Sort by timestamp
        all_messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        all_messages
    }
    
    /// Clear all messages (for conversation switching)
    pub fn clear_all_messages(&self) {
        {
            let mut messages = self.messages.lock().unwrap();
            messages.clear();
        }
        {
            let mut active_streams = self.active_streams.lock().unwrap();
            active_streams.clear();
        }
        log::info!("StreamingChatManager: Cleared all messages and active streams");
    }
    
    /// Update a tool call result by tool_call_id (more precise than by tool name)
    pub fn update_tool_call_result_by_id(&self, tool_call_id: &str, result: String, is_success: bool) -> bool {
        // Check active streams first
        {
            let mut active_streams = self.active_streams.lock().unwrap();
            for message in active_streams.values_mut() {
                for tool_call in &mut message.tool_calls {
                    // Try to match by tool call ID in the arguments (since tool_call.name is the tool name, not ID)
                    if tool_call.arguments.contains(tool_call_id) {
                        tool_call.result = Some(result.clone());
                        tool_call.status = if is_success {
                            MessageStatus::Complete
                        } else {
                            MessageStatus::Error("Tool execution failed".to_string())
                        };
                        return true;
                    }
                }
            }
        }
        
        // Check completed messages
        {
            let mut messages = self.messages.lock().unwrap();
            for message in messages.iter_mut() {
                for tool_call in &mut message.tool_calls {
                    // Try to match by tool call ID in the arguments
                    if tool_call.arguments.contains(tool_call_id) {
                        tool_call.result = Some(result.clone());
                        tool_call.status = if is_success {
                            MessageStatus::Complete
                        } else {
                            MessageStatus::Error("Tool execution failed".to_string())
                        };
                        return true;
                    }
                }
            }
        }
        
        false
    }
    
    /// Update a tool call result by tool name (fallback method)
    pub fn update_tool_call_result_by_name(&self, tool_name: &str, result: String, is_success: bool) -> bool {
        // Check active streams first
        {
            let mut active_streams = self.active_streams.lock().unwrap();
            for message in active_streams.values_mut() {
                for tool_call in &mut message.tool_calls {
                    if tool_call.name == tool_name && tool_call.result.is_none() {
                        tool_call.result = Some(result.clone());
                        tool_call.status = if is_success {
                            MessageStatus::Complete
                        } else {
                            MessageStatus::Error("Tool execution failed".to_string())
                        };
                        return true;
                    }
                }
            }
        }
        
        // Check completed messages
        {
            let mut messages = self.messages.lock().unwrap();
            for message in messages.iter_mut() {
                for tool_call in &mut message.tool_calls {
                    if tool_call.name == tool_name && tool_call.result.is_none() {
                        tool_call.result = Some(result.clone());
                        tool_call.status = if is_success {
                            MessageStatus::Complete
                        } else {
                            MessageStatus::Error("Tool execution failed".to_string())
                        };
                        return true;
                    }
                }
            }
        }
        
        false
    }
    
    /// Set the message_type for a message by ID (active or completed)
    pub fn set_message_type(&self, message_id: &str, message_type: MessageType) {
        {
            let mut active_streams = self.active_streams.lock().unwrap();
            if let Some(message) = active_streams.get_mut(message_id) {
                message.message_type = message_type.clone();
                return;
            }
        }
        {
            let mut messages = self.messages.lock().unwrap();
            for message in messages.iter_mut() {
                if message.id == message_id {
                    message.message_type = message_type.clone();
                    return;
                }
            }
        }
    }
}

impl Default for StreamingChatManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Example usage demonstrating proper streaming
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_streaming_chat_manager() {
        let manager = StreamingChatManager::new();
        
        // Add user message
        let user_id = manager.add_user_message("Hello, can you help me?".to_string());
        
        // Start agent response
        let agent_id = manager.start_agent_response();
        
        // Set thinking
        manager.set_thinking(&agent_id, "Let me think about how to help you...".to_string());
        
        // Start streaming content
        manager.append_content(&agent_id, "Of course! I'd be happy to help you with ".to_string());
        manager.append_content(&agent_id, "whatever you need. What specifically ".to_string());
        manager.append_content(&agent_id, "would you like assistance with?".to_string());
        
        // Add a tool call
        let tool_call = ToolCall {
            name: "web_search".to_string(),
            arguments: r#"{"query": "help assistance"}"#.to_string(),
            result: Some("Found relevant help resources".to_string()),
            status: MessageStatus::Complete,
        };
        manager.add_tool_call(&agent_id, tool_call);
        
        // Finish streaming
        manager.finish_streaming(&agent_id);
        
        // Get all messages
        let messages = manager.get_all_messages();
        
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].author, MessageAuthor::User);
        assert_eq!(messages[1].author, MessageAuthor::Agent);
        assert!(messages[1].is_complete());
        assert!(!messages[1].tool_calls.is_empty());
    }
    
    #[test]
    fn test_error_handling() {
        let manager = StreamingChatManager::new();
        
        let agent_id = manager.start_agent_response();
        manager.append_content(&agent_id, "Starting response...".to_string());
        manager.set_error(&agent_id, "Network timeout".to_string());
        
        let messages = manager.get_all_messages();
        assert_eq!(messages.len(), 1);
        assert!(matches!(messages[0].status, MessageStatus::Error(_)));
    }
    
    #[test]
    fn test_thinking_to_streaming_transition() {
        let manager = StreamingChatManager::new();
        
        let agent_id = manager.start_agent_response();
        manager.set_thinking(&agent_id, "Analyzing the problem...".to_string());
        
        // When we start appending content, it should switch from thinking to streaming
        manager.append_content(&agent_id, "Based on my analysis, ".to_string());
        
        let messages = manager.get_all_messages();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].is_streaming());
        assert!(messages[0].thinking_content.is_some());
    }
    
    #[test]
    fn test_streaming_thinking_functionality() {
        let manager = StreamingChatManager::new();
        
        let agent_id = manager.start_agent_response();
        
        // Start streaming thinking
        manager.start_thinking_stream(&agent_id);
        
        // Append thinking chunks
        manager.append_thinking(&agent_id, "Let me think about this...".to_string());
        manager.append_thinking(&agent_id, " I need to consider multiple factors.".to_string());
        
        // Check the active streams
        let active_streams = manager.active_streams.lock().unwrap();
        let message = active_streams.get(&agent_id).unwrap();
        
        assert!(message.thinking_is_streaming);
        assert_eq!(message.thinking_stream_content, "Let me think about this... I need to consider multiple factors.");
        assert!(message.has_thinking_content());
        assert!(message.should_show_thinking());
        assert_eq!(message.get_thinking_opacity(), 1.0); // Should be fully opaque when not fading
        
        drop(active_streams);
        
        // Start regular content - this should trigger fade
        manager.append_content(&agent_id, "Based on my analysis, ".to_string());
        
        let active_streams = manager.active_streams.lock().unwrap();
        let message = active_streams.get(&agent_id).unwrap();
        
        assert!(message.thinking_should_fade);
        assert!(message.thinking_fade_start.is_some());
        
        drop(active_streams);
        
        // Finish thinking stream
        manager.finish_thinking_stream(&agent_id);
        
        let active_streams = manager.active_streams.lock().unwrap();
        let message = active_streams.get(&agent_id).unwrap();
        
        assert!(!message.thinking_is_streaming);
        
        drop(active_streams);
        
        // Finish streaming
        manager.finish_streaming(&agent_id);
        
        let messages = manager.get_all_messages();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].is_complete());
        assert!(messages[0].has_thinking_content());
    }
    
    #[test]
    fn test_thinking_fade_timing() {
        let manager = StreamingChatManager::new();
        
        let agent_id = manager.start_agent_response();
        manager.append_thinking(&agent_id, "Thinking...".to_string());
        
        let active_streams = manager.active_streams.lock().unwrap();
        let mut message = active_streams.get(&agent_id).unwrap().clone();
        drop(active_streams);
        
        // Initially should show thinking
        assert!(message.should_show_thinking());
        assert_eq!(message.get_thinking_opacity(), 1.0);
        
        // Start fade
        message.start_thinking_fade();
        
        // Should still show thinking immediately after fade starts
        assert!(message.should_show_thinking());
        
        // Simulate time passing by manually setting fade start time to past
        message.thinking_fade_start = Some(std::time::Instant::now() - std::time::Duration::from_secs(3));
        
        // After fade duration, should not show thinking
        assert!(!message.should_show_thinking());
        assert_eq!(message.get_thinking_opacity(), 0.0);
    }
    
    #[test]
    fn test_multiple_agent_responses_create_separate_messages() {
        let manager = StreamingChatManager::new();
        
        // Add user message
        let user_id = manager.add_user_message("First question".to_string());
        
        // First agent response
        let agent_id_1 = manager.start_agent_response();
        manager.append_content(&agent_id_1, "First response".to_string());
        manager.finish_streaming(&agent_id_1);
        
        // User asks another question
        let user_id_2 = manager.add_user_message("Second question".to_string());
        
        // Second agent response (should be a NEW message, not overwriting the first)
        let agent_id_2 = manager.start_agent_response();
        manager.append_content(&agent_id_2, "Second response".to_string());
        manager.finish_streaming(&agent_id_2);
        
        // Get all messages
        let messages = manager.get_all_messages();
        
        // Should have 4 messages total: user1, agent1, user2, agent2
        assert_eq!(messages.len(), 4);
        
        // Check that agent responses are separate messages with different IDs
        let agent_messages: Vec<_> = messages.iter()
            .filter(|m| m.author == MessageAuthor::Agent)
            .collect();
        
        assert_eq!(agent_messages.len(), 2);
        assert_ne!(agent_messages[0].id, agent_messages[1].id);
        assert_eq!(agent_messages[0].content, "First response");
        assert_eq!(agent_messages[1].content, "Second response");
        
        // Check timestamps are different (second should be later)
        assert!(agent_messages[1].timestamp > agent_messages[0].timestamp);
    }
    
    #[test]
    fn test_sagitta_code_streaming_behavior_integration() {
        let manager = StreamingChatManager::new();
        
        // Simulate a conversation where Sagitta Code should create separate messages
        
        // User asks first question
        let user_id_1 = manager.add_user_message("What is Rust?".to_string());
        
        // Sagitta Code starts responding (first response)
        let sagitta_code_id_1 = manager.start_agent_response();
        manager.set_thinking(&sagitta_code_id_1, "Let me think about Rust programming language...".to_string());
        manager.append_content(&sagitta_code_id_1, "Rust is a systems programming language".to_string());
        manager.append_content(&sagitta_code_id_1, " that focuses on safety and performance.".to_string());
        manager.finish_streaming(&sagitta_code_id_1);
        
        // User asks follow-up question
        let user_id_2 = manager.add_user_message("Can you give me an example?".to_string());
        
        // Sagitta Code starts NEW response (should be separate message with new timestamp)
        let sagitta_code_id_2 = manager.start_agent_response();
        manager.append_content(&sagitta_code_id_2, "Here's a simple Rust example:\n\n".to_string());
        manager.append_content(&sagitta_code_id_2, "```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```".to_string());
        manager.finish_streaming(&sagitta_code_id_2);
        
        // Get all messages
        let messages = manager.get_all_messages();
        
        // Verify we have 4 separate messages
        assert_eq!(messages.len(), 4, "Should have 4 separate messages: user1, sagitta_code1, user2, sagitta_code2");
        
        // Verify message order and content
        assert_eq!(messages[0].author, MessageAuthor::User);
        assert_eq!(messages[0].content, "What is Rust?");
        
        assert_eq!(messages[1].author, MessageAuthor::Agent);
        assert_eq!(messages[1].content, "Rust is a systems programming language that focuses on safety and performance.");
        assert!(messages[1].thinking_content.is_some());
        assert!(messages[1].is_complete());
        
        assert_eq!(messages[2].author, MessageAuthor::User);
        assert_eq!(messages[2].content, "Can you give me an example?");
        
        assert_eq!(messages[3].author, MessageAuthor::Agent);
        assert!(messages[3].content.contains("Here's a simple Rust example"));
        assert!(messages[3].content.contains("fn main()"));
        assert!(messages[3].is_complete());
        
        // Verify Sagitta Code's responses have different IDs (not overwriting same message)
        assert_ne!(messages[1].id, messages[3].id, "Sagitta Code's responses should have different IDs");
        
        // Verify timestamps are in order (later messages have later timestamps)
        assert!(messages[1].timestamp <= messages[2].timestamp);
        assert!(messages[2].timestamp <= messages[3].timestamp);
        
        // Verify Sagitta Code's second response has a later timestamp than the first
        assert!(messages[3].timestamp > messages[1].timestamp, 
                "Sagitta Code's second response should have a later timestamp than the first");
    }
}

