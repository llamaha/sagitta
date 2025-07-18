use crate::llm::client::{StreamChunk, MessagePart};
use crate::agent::message::types::ToolCall;
use std::collections::HashMap;
use serde_json::Value;

/// Result of creating a final chunk
#[derive(Debug)]
pub struct FinalChunk {
    pub content: String,
}

/// Handles accumulation of streaming data for OpenAI compatible providers
#[derive(Debug, Default)]
pub struct StreamProcessor {
    /// Accumulates content text
    content_buffer: String,
    
    /// Accumulates tool calls by index
    tool_calls: HashMap<usize, PartialToolCall>,
    
    /// Tracks if we're currently in a thinking block
    is_thinking: bool,
    
    /// Accumulates thinking content
    thinking_buffer: String,
}

#[derive(Debug, Clone)]
struct PartialToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

impl StreamProcessor {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Process a delta from the OpenAI stream
    pub fn process_delta(&mut self, delta: &Value) -> Vec<StreamChunk> {
        let mut chunks = Vec::new();
        
        // Handle content delta
        if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
            // Check for thinking tags
            if let Some(processed_chunks) = self.process_content_with_thinking(content) {
                chunks.extend(processed_chunks);
            }
        }
        
        // Handle tool calls delta
        if let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
            for tool_call in tool_calls {
                if let Some(index) = tool_call.get("index").and_then(|v| v.as_u64()) {
                    let index = index as usize;
                    
                    // Get or create partial tool call
                    let partial = self.tool_calls.entry(index).or_insert_with(|| PartialToolCall {
                        id: None,
                        name: None,
                        arguments: String::new(),
                    });
                    
                    // Update fields
                    if let Some(id) = tool_call.get("id").and_then(|v| v.as_str()) {
                        partial.id = Some(id.to_string());
                    }
                    
                    if let Some(function) = tool_call.get("function") {
                        if let Some(name) = function.get("name").and_then(|v| v.as_str()) {
                            partial.name = Some(name.to_string());
                        }
                        
                        if let Some(args) = function.get("arguments").and_then(|v| v.as_str()) {
                            partial.arguments.push_str(args);
                        }
                    }
                }
            }
        }
        
        chunks
    }
    
    /// Process content that might contain thinking tags
    fn process_content_with_thinking(&mut self, content: &str) -> Option<Vec<StreamChunk>> {
        let mut chunks = Vec::new();
        let mut remaining = content;
        
        while !remaining.is_empty() {
            if self.is_thinking {
                // Look for closing thinking tag
                if let Some(end_pos) = remaining.find("</thinking>") {
                    // Add content up to closing tag to thinking buffer
                    self.thinking_buffer.push_str(&remaining[..end_pos]);
                    
                    // Emit thinking chunk
                    if !self.thinking_buffer.is_empty() {
                        chunks.push(StreamChunk {
                            part: MessagePart::Thought { 
                                text: self.thinking_buffer.clone() 
                            },
                            is_final: false,
                            finish_reason: None,
                            token_usage: None,
                        });
                        self.thinking_buffer.clear();
                    }
                    
                    self.is_thinking = false;
                    remaining = &remaining[end_pos + 11..]; // Skip past </thinking>
                } else {
                    // No closing tag, add all to thinking buffer
                    self.thinking_buffer.push_str(remaining);
                    break;
                }
            } else {
                // Look for opening thinking tag
                if let Some(start_pos) = remaining.find("<thinking>") {
                    // Emit any content before thinking tag
                    if start_pos > 0 {
                        let content_before = &remaining[..start_pos];
                        self.content_buffer.push_str(content_before);
                        
                        if !self.content_buffer.is_empty() {
                            chunks.push(StreamChunk {
                                part: MessagePart::Text { 
                                    text: self.content_buffer.clone() 
                                },
                                is_final: false,
                                finish_reason: None,
                                token_usage: None,
                            });
                            self.content_buffer.clear();
                        }
                    }
                    
                    self.is_thinking = true;
                    remaining = &remaining[start_pos + 10..]; // Skip past <thinking>
                } else {
                    // No thinking tag, add all to content buffer
                    self.content_buffer.push_str(remaining);
                    
                    // Emit content chunk
                    if !self.content_buffer.is_empty() {
                        chunks.push(StreamChunk {
                            part: MessagePart::Text { 
                                text: self.content_buffer.clone() 
                            },
                            is_final: false,
                            finish_reason: None,
                            token_usage: None,
                        });
                        self.content_buffer.clear();
                    }
                    break;
                }
            }
        }
        
        if chunks.is_empty() {
            None
        } else {
            Some(chunks)
        }
    }
    
    /// Get completed tool calls and clear the buffer
    pub fn get_completed_tool_calls(&mut self) -> Vec<ToolCall> {
        let mut completed = Vec::new();
        
        // Check each tool call to see if it's complete
        let mut indices: Vec<_> = self.tool_calls.keys().cloned().collect();
        indices.sort();
        
        for index in indices {
            if let Some(partial) = self.tool_calls.get(&index) {
                if let (Some(id), Some(name)) = (&partial.id, &partial.name) {
                    // Try to parse arguments as JSON
                    if !partial.arguments.is_empty() {
                        match serde_json::from_str::<Value>(&partial.arguments) {
                            Ok(args) => {
                                completed.push(ToolCall {
                                    id: id.clone(),
                                    name: name.clone(),
                                    arguments: args,
                                    result: None,
                                    successful: false,
                                    execution_time: None,
                                });
                                
                                // Remove completed tool call
                                self.tool_calls.remove(&index);
                            }
                            Err(_) => {
                                // Arguments not yet complete JSON, keep accumulating
                            }
                        }
                    }
                }
            }
        }
        
        completed
    }
    
    /// Create a final chunk with any remaining data
    pub fn create_final_chunk(&mut self, _finish_reason: Option<String>) -> Option<FinalChunk> {
        // Flush any remaining content
        let has_content = !self.content_buffer.is_empty() || !self.thinking_buffer.is_empty();
        
        if has_content {
            let mut content = String::new();
            
            if !self.content_buffer.is_empty() {
                content.push_str(&self.content_buffer);
                self.content_buffer.clear();
            }
            
            if !self.thinking_buffer.is_empty() {
                // If we have unclosed thinking content, emit it as regular content
                content.push_str(&self.thinking_buffer);
                self.thinking_buffer.clear();
            }
            
            Some(FinalChunk { content })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_process_simple_content() {
        let mut processor = StreamProcessor::new();
        
        let delta = json!({
            "content": "Hello, world!"
        });
        
        let chunks = processor.process_delta(&delta);
        assert_eq!(chunks.len(), 1);
        if let MessagePart::Text { text } = &chunks[0].part {
            assert_eq!(text, "Hello, world!");
        } else {
            panic!("Expected text part");
        }
    }
    
    #[test]
    fn test_process_thinking_content() {
        let mut processor = StreamProcessor::new();
        
        // First chunk with opening tag
        let delta1 = json!({
            "content": "Let me think about this <thinking>I need to"
        });
        
        let chunks1 = processor.process_delta(&delta1);
        assert_eq!(chunks1.len(), 1);
        if let MessagePart::Text { text } = &chunks1[0].part {
            assert_eq!(text, "Let me think about this ");
        } else {
            panic!("Expected text part");
        }
        
        // Second chunk continuing thinking
        let delta2 = json!({
            "content": " consider the options</thinking> The answer is"
        });
        
        let chunks2 = processor.process_delta(&delta2);
        assert_eq!(chunks2.len(), 2);
        if let MessagePart::Thought { text } = &chunks2[0].part {
            assert_eq!(text, "I need to consider the options");
        } else {
            panic!("Expected thought part");
        }
        if let MessagePart::Text { text } = &chunks2[1].part {
            assert_eq!(text, " The answer is");
        } else {
            panic!("Expected text part");
        }
    }
    
    #[test]
    fn test_process_tool_calls() {
        let mut processor = StreamProcessor::new();
        
        // First chunk with tool call start
        let delta1 = json!({
            "tool_calls": [{
                "index": 0,
                "id": "call_123",
                "function": {
                    "name": "get_weather",
                    "arguments": "{\"location\":"
                }
            }]
        });
        
        processor.process_delta(&delta1);
        let completed1 = processor.get_completed_tool_calls();
        assert_eq!(completed1.len(), 0); // Not complete yet
        
        // Second chunk completing the arguments
        let delta2 = json!({
            "tool_calls": [{
                "index": 0,
                "function": {
                    "arguments": " \"San Francisco\"}"
                }
            }]
        });
        
        processor.process_delta(&delta2);
        let completed2 = processor.get_completed_tool_calls();
        assert_eq!(completed2.len(), 1);
        assert_eq!(completed2[0].id, "call_123");
        assert_eq!(completed2[0].name, "get_weather");
        assert_eq!(completed2[0].arguments, json!({"location": "San Francisco"}));
    }
    
    #[test]
    fn test_multiple_tool_calls() {
        let mut processor = StreamProcessor::new();
        
        // Multiple tool calls in parallel
        let delta = json!({
            "tool_calls": [
                {
                    "index": 0,
                    "id": "call_1",
                    "function": {
                        "name": "func1",
                        "arguments": "{\"arg\": 1}"
                    }
                },
                {
                    "index": 1,
                    "id": "call_2",
                    "function": {
                        "name": "func2",
                        "arguments": "{\"arg\": 2}"
                    }
                }
            ]
        });
        
        processor.process_delta(&delta);
        let mut completed = processor.get_completed_tool_calls();
        completed.sort_by(|a, b| a.id.cmp(&b.id));
        
        assert_eq!(completed.len(), 2);
        assert_eq!(completed[0].id, "call_1");
        assert_eq!(completed[0].name, "func1");
        assert_eq!(completed[1].id, "call_2");
        assert_eq!(completed[1].name, "func2");
    }
    
    #[test]
    fn test_final_chunk() {
        let mut processor = StreamProcessor::new();
        
        // Add some content
        processor.process_delta(&json!({"content": "Final message"}));
        
        // Create final chunk
        let final_chunk = processor.create_final_chunk(Some("stop".to_string())).unwrap();
        assert_eq!(final_chunk.content, "Final message");
    }
}