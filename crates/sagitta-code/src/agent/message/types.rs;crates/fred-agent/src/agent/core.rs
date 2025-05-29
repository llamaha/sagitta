/// Create an AgentMessage from a LlmMessage
pub fn from_llm_message(message: &LlmMessage) -> Self {
    let mut content = String::new();
    
    for part in &message.parts {
        match part {
            MessagePart::Text { text } => {
                if !content.is_empty() && !text.is_empty() {
                    content.push(' ');
                }
                content.push_str(text);
            },
            MessagePart::ToolCall { .. } => {
                // Tool calls are now handled exclusively by iterating LlmResponse.tool_calls
                // in Agent::process_llm_response to avoid duplication.
                // This part is primarily for reconstructing the assistant's textual content if any,
                // or if the model intersperses text with tool call requests (though less common for Gemini).
                log::trace!("AgentMessage::from_llm_message: Encountered ToolCall part, will be processed from LlmResponse.tool_calls.");
            },
            MessagePart::ToolResult { .. } => {
                // Tool results are typically added separately after execution
                log::trace!("AgentMessage::from_llm_message: Encountered ToolResult part.");
            },
        }
    }
    
    Self {
        id: message.id,
        role: message.role.clone(),
        content,
        is_streaming: false, // from_llm_message is usually for non-streaming complete messages
        timestamp: Utc::now(),
        metadata: HashMap::new(), // TODO: Convert metadata if LlmMessage.metadata becomes richer
        tool_calls: Vec::new(), // IMPORTANT: Initialize as empty. Will be populated by Agent::process_llm_response
    }
} 