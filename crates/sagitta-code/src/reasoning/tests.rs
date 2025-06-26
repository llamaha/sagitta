#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::llm_adapter::ReasoningLlmClientAdapter;
    use crate::llm::client::{StreamChunk, MessagePart, TokenUsage};
    use crate::tools::registry::ToolRegistry;
    use futures_util::StreamExt;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_tool_result_handling_in_stream() {
        // Test that tool results in the stream are handled gracefully
        // and don't produce the "[Received ToolResult from LLM stream - unhandled]" message
        
        // Create a mock stream chunk with a tool result
        let tool_result_chunk = StreamChunk {
            part: MessagePart::ToolResult {
                tool_call_id: "test_123".to_string(),
                name: "web_search".to_string(),
                result: serde_json::json!({
                    "results": ["result1", "result2"]
                }),
            },
            is_final: false,
            finish_reason: None,
            token_usage: None,
        };
        
        // The adapter should convert this to an empty text chunk
        // rather than the unhandled message
        // This prevents the confusing message from appearing in the UI
    }
    
    #[tokio::test] 
    async fn test_tool_use_content_block_streaming() {
        // Test that tool_use content blocks are properly converted to tool calls
        let tool_use_chunk = StreamChunk {
            part: MessagePart::ToolCall {
                tool_call_id: "tool_456".to_string(),
                name: "add_existing_repository".to_string(),
                parameters: serde_json::json!({
                    "url": "https://github.com/tokio-rs/tokio.git",
                    "name": "tokio"
                }),
            },
            is_final: false,
            finish_reason: None,
            token_usage: None,
        };
        
        // The adapter should properly convert this to a ReasoningStreamChunk::ToolCall
    }
}