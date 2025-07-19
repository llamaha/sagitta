use crate::llm::client::{Message, MessagePart, Role, ToolDefinition};
use crate::providers::openai_compatible::client::OpenAICompatibleClient;
use crate::utils::errors::SagittaCodeError;
use std::sync::Arc;
use tokio::sync::Mutex;
use futures_util::StreamExt;
use log::{info, debug};

/// Manages conversation continuation for OpenAI-compatible APIs
/// 
/// OpenAI's API pattern requires explicit continuation after tool calls:
/// 1. User sends message
/// 2. Assistant responds with tool calls
/// 3. Tools are executed and results added as function messages  
/// 4. Another API call is made to get the final response
/// 5. Assistant provides final answer
pub struct ContinuationManager {
    client: Arc<OpenAICompatibleClient>,
    pending_tool_calls: Arc<Mutex<Vec<String>>>, // Track tool call IDs
}

impl ContinuationManager {
    pub fn new(client: Arc<OpenAICompatibleClient>) -> Self {
        Self {
            client,
            pending_tool_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    /// Register a tool call that needs completion
    pub async fn register_tool_call(&self, tool_call_id: String) {
        let mut pending = self.pending_tool_calls.lock().await;
        pending.push(tool_call_id);
        debug!("Registered pending tool call, total pending: {}", pending.len());
    }
    
    /// Mark a tool call as completed
    pub async fn complete_tool_call(&self, tool_call_id: &str) -> bool {
        let mut pending = self.pending_tool_calls.lock().await;
        pending.retain(|id| id != tool_call_id);
        let all_complete = pending.is_empty();
        debug!("Completed tool call {}, remaining pending: {}", tool_call_id, pending.len());
        all_complete
    }
    
    /// Check if all tool calls are complete
    pub async fn all_tools_complete(&self) -> bool {
        let pending = self.pending_tool_calls.lock().await;
        pending.is_empty()
    }
    
    /// Clear all pending tool calls
    pub async fn clear_pending(&self) {
        let mut pending = self.pending_tool_calls.lock().await;
        pending.clear();
    }
    
    /// Check if we should continue the conversation after tool execution
    /// Returns true if we have tool results in the message history but no pending tools
    pub async fn should_continue(&self, messages: &[Message]) -> bool {
        // Check if we have any function role messages (tool results)
        let has_tool_results = messages.iter().any(|msg| msg.role == Role::Function);
        
        // Check if the last assistant message had tool calls
        let last_assistant_had_tools = messages.iter()
            .rev()
            .find(|msg| msg.role == Role::Assistant)
            .map(|msg| !msg.parts.is_empty() && msg.parts.iter().any(|part| {
                matches!(part, MessagePart::ToolCall { .. })
            }))
            .unwrap_or(false);
        
        // Continue if we have tool results and the last assistant message had tool calls
        has_tool_results && last_assistant_had_tools && self.all_tools_complete().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_continuation_tracking() {
        let client = Arc::new(OpenAICompatibleClient::new(
            "http://localhost:1234/v1".to_string(),
            None,
            None,
            Arc::new(crate::providers::claude_code::mcp_integration::McpIntegration::new()),
            120,
            3,
        ));
        
        let manager = ContinuationManager::new(client);
        
        // Initially no pending tools
        assert!(manager.all_tools_complete().await);
        
        // Register some tool calls
        manager.register_tool_call("tool1".to_string()).await;
        manager.register_tool_call("tool2".to_string()).await;
        assert!(!manager.all_tools_complete().await);
        
        // Complete one tool
        let all_done = manager.complete_tool_call("tool1").await;
        assert!(!all_done);
        assert!(!manager.all_tools_complete().await);
        
        // Complete the other tool
        let all_done = manager.complete_tool_call("tool2").await;
        assert!(all_done);
        assert!(manager.all_tools_complete().await);
    }
}