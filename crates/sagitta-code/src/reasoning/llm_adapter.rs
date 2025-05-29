use async_trait::async_trait;
use std::sync::Arc;
use futures_util::{Stream, StreamExt, TryStreamExt};
use std::pin::Pin;
use std::collections::HashMap;
use uuid::Uuid;

use crate::utils::errors::FredAgentError;

// Types from sagitta_code::llm::client
use crate::llm::client::{
    LlmClient as FredLlmClient,
    Message as FredMessage,
    MessagePart as FredMessagePart,
    StreamChunk as FredStreamChunk,
    Role as FredRole
};

// Types and traits from reasoning_engine::traits
use reasoning_engine::traits::{
    LlmClient as ReasoningLlmClient,
    LlmMessage as ReasoningMessage,
    LlmMessagePart as ReasoningMessagePart,
    LlmStreamChunk as ReasoningStreamChunk,
    ToolCall as ReasoningToolCall,
    TokenUsage as ReasoningTokenUsage,
};
use reasoning_engine::ReasoningError;

// Import ToolRegistry and the agent's ToolDefinition for conversion
use crate::tools::registry::ToolRegistry;
use crate::tools::types::ToolDefinition as AgentToolDefinition; 

/// Adapter to make Fred-Agent's LlmClient usable by ReasoningEngine
#[derive(Clone)] // Clone is needed if ReasoningEngine will store Arc<Self>
pub struct ReasoningLlmClientAdapter {
    fred_llm_client: Arc<dyn FredLlmClient>,
    tool_registry: Arc<ToolRegistry>, // Added tool_registry
}

impl ReasoningLlmClientAdapter {
    pub fn new(fred_llm_client: Arc<dyn FredLlmClient>, tool_registry: Arc<ToolRegistry>) -> Self {
        Self { fred_llm_client, tool_registry }
    }
}

#[async_trait]
impl ReasoningLlmClient for ReasoningLlmClientAdapter {
    async fn generate_stream(
        &self,
        messages: Vec<ReasoningMessage>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ReasoningStreamChunk, ReasoningError>> + Send>>, ReasoningError> {
        
        let fred_messages: Vec<FredMessage> = messages.into_iter().map(|msg| {
            let role = match msg.role.as_str() {
                "user" => FredRole::User,
                "assistant" => FredRole::Assistant,
                "tool" => FredRole::User,
                _ => FredRole::User,
            };
            let parts = msg.parts.into_iter().filter_map(|part| {
                match part {
                    ReasoningMessagePart::Text(text) => Some(FredMessagePart::Text { text }),
                    ReasoningMessagePart::ToolCall(tool_call_data) => {
                        Some(FredMessagePart::ToolCall {
                            tool_call_id: Uuid::new_v4().to_string(),
                            name: tool_call_data.name,
                            parameters: tool_call_data.args,
                        })
                    }
                }
            }).collect();
            FredMessage {
                id: Uuid::new_v4(),
                role, 
                parts, 
                metadata: HashMap::new()
            }
        }).collect();

        // Fetch and convert tool definitions
        let agent_tools: Vec<AgentToolDefinition> = self.tool_registry.get_definitions().await;
        let fred_tools: Vec<crate::llm::client::ToolDefinition> = agent_tools.into_iter().map(|agent_tool_def| {
            crate::llm::client::ToolDefinition {
                name: agent_tool_def.name,
                description: agent_tool_def.description,
                parameters: agent_tool_def.parameters,
                is_required: agent_tool_def.is_required, // This field exists on both
            }
        }).collect();

        match self.fred_llm_client.generate_stream(&fred_messages, &fred_tools).await {
            Ok(fred_stream) => {
                let reasoning_stream = fred_stream.map_ok(|fred_chunk: FredStreamChunk| {
                    // If FredStreamChunk has token_usage, map it directly
                    if let Some(usage) = fred_chunk.token_usage {
                        // Convert sagitta_code::llm::client::TokenUsage to reasoning_engine::traits::TokenUsage
                        let reasoning_usage = ReasoningTokenUsage {
                            prompt_tokens: usage.prompt_tokens,
                            completion_tokens: usage.completion_tokens,
                            total_tokens: usage.total_tokens,
                            thinking_tokens: usage.thinking_tokens,
                            model_name: usage.model_name,
                            cached_tokens: usage.cached_tokens,
                        };
                        return ReasoningStreamChunk::TokenUsage(reasoning_usage);
                    }

                    // Otherwise, convert FredStreamChunk to ReasoningStreamChunk based on part
                    match fred_chunk.part {
                        FredMessagePart::Text { text } => {
                            ReasoningStreamChunk::Text { // Use enum variant
                                content: text,
                                is_final: fred_chunk.is_final,
                            }
                        }
                        FredMessagePart::Thought { text } => { // Added handling for Thought
                             ReasoningStreamChunk::Text { // Use enum variant
                                content: format!("THINKING:{}", text), // Prefix content
                                is_final: fred_chunk.is_final,
                            }
                        }
                        FredMessagePart::ToolCall { name, parameters, tool_call_id: _ } => { // Correctly destructure, ignore tool_call_id for now
                            ReasoningStreamChunk::ToolCall {
                                tool_call: ReasoningToolCall { name, args: parameters }, // Map parameters to args
                                is_final: fred_chunk.is_final, // Assuming a tool call can be final
                            }
                        }
                        FredMessagePart::ToolResult { name: _, result: _, tool_call_id: _ } => {
                            ReasoningStreamChunk::Text {
                                content: "[Received ToolResult from LLM stream - unhandled]".to_string(),
                                is_final: fred_chunk.is_final,
                            }
                        }
                        // Other FredMessagePart types (like ToolResult) would need mapping if they appear here.
                        _ => {
                             // Default for unhandled/empty parts.
                             // Consider if this should be an error or an empty text chunk.
                             // For robustness, an empty text chunk might be safer if the stream can send unexpected empty parts.
                            ReasoningStreamChunk::Text {
                                content: String::new(),
                                is_final: fred_chunk.is_final,
                            }
                        }
                    }
                }).map_err(|e: FredAgentError| ReasoningError::llm(e.to_string()));
                Ok(Box::pin(reasoning_stream))
            }
            Err(e) => Err(ReasoningError::llm(format!("FredLlmClient failed to generate stream: {}", e))),
        }
    }
} 