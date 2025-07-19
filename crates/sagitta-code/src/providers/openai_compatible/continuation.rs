use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::llm::client::{LlmClient, Message, MessagePart, Role, ToolDefinition};
use crate::agent::message::types::ToolCall;

/// Manages conversation continuation for OpenAI-compatible providers
/// 
/// OpenAI-style APIs require explicit continuation after tool calls:
/// 1. Model returns tool_calls with finish_reason: "tool_calls"
/// 2. Client executes tools
/// 3. Client sends new request with tool results
/// 4. Model provides final response
#[derive(Debug, Clone)]
pub struct ContinuationManager {
    /// Track pending tool calls per conversation
    pending_tool_calls: Arc<Mutex<HashMap<Uuid, Vec<ToolCall>>>>,
    
    /// Track tool results per conversation
    tool_results: Arc<Mutex<HashMap<Uuid, Vec<(String, serde_json::Value)>>>>,
}

impl ContinuationManager {
    pub fn new() -> Self {
        Self {
            pending_tool_calls: Arc::new(Mutex::new(HashMap::new())),
            tool_results: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Register tool calls that need execution
    pub async fn register_tool_calls(&self, conversation_id: Uuid, tool_calls: Vec<ToolCall>) {
        let mut pending = self.pending_tool_calls.lock().await;
        pending.insert(conversation_id, tool_calls);
    }
    
    /// Add a tool result
    pub async fn add_tool_result(&self, conversation_id: Uuid, tool_call_id: String, result: serde_json::Value) {
        let mut results = self.tool_results.lock().await;
        let conv_results = results.entry(conversation_id).or_insert_with(Vec::new);
        conv_results.push((tool_call_id, result));
    }
    
    /// Check if all tools have been executed for a conversation
    pub async fn all_tools_complete(&self, conversation_id: Uuid) -> bool {
        let pending = self.pending_tool_calls.lock().await;
        let results = self.tool_results.lock().await;
        
        if let Some(tool_calls) = pending.get(&conversation_id) {
            if let Some(tool_results) = results.get(&conversation_id) {
                // Check if we have results for all tool calls
                tool_calls.len() == tool_results.len()
            } else {
                false
            }
        } else {
            // No pending tool calls
            true
        }
    }
    
    /// Get tool results for building continuation message
    pub async fn get_tool_results(&self, conversation_id: Uuid) -> Vec<(String, serde_json::Value)> {
        let results = self.tool_results.lock().await;
        results.get(&conversation_id).cloned().unwrap_or_default()
    }
    
    /// Clear tool state for a conversation
    pub async fn clear_conversation(&self, conversation_id: Uuid) {
        let mut pending = self.pending_tool_calls.lock().await;
        let mut results = self.tool_results.lock().await;
        pending.remove(&conversation_id);
        results.remove(&conversation_id);
    }
    
    /// Build tool result messages for OpenAI API
    pub fn build_tool_result_messages(&self, tool_results: Vec<(String, serde_json::Value)>) -> Vec<Message> {
        tool_results.into_iter().map(|(tool_call_id, result)| {
            Message {
                id: Uuid::new_v4(),
                role: Role::Function,
                parts: vec![MessagePart::ToolResult {
                    tool_call_id: tool_call_id.clone(),
                    name: String::new(), // Tool name would need to be tracked separately
                    result,
                }],
                metadata: HashMap::new(),
            }
        }).collect()
    }
}

impl Default for ContinuationManager {
    fn default() -> Self {
        Self::new()
    }
}