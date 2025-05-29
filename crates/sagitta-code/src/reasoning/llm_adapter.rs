use async_trait::async_trait;
use std::sync::Arc;
use futures_util::{Stream, StreamExt, TryStreamExt};
use std::pin::Pin;
use std::collections::HashMap;
use uuid::Uuid;

use crate::utils::errors::SagittaCodeError;

// Types from sagitta_code::llm::client
use crate::llm::client::{
    LlmClient as SagittaCodeLlmClient,
    Message as SagittaCodeMessage,
    MessagePart as SagittaCodeMessagePart,
    StreamChunk as SagittaCodeStreamChunk,
    Role as SagittaCodeRole
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

/// Adapter to make Sagitta-Code's LlmClient usable by ReasoningEngine
#[derive(Clone)] // Clone is needed if ReasoningEngine will store Arc<Self>
pub struct ReasoningLlmClientAdapter {
    sagitta_code_llm_client: Arc<dyn SagittaCodeLlmClient>,
    tool_registry: Arc<ToolRegistry>, // Added tool_registry
}

impl ReasoningLlmClientAdapter {
    pub fn new(sagitta_code_llm_client: Arc<dyn SagittaCodeLlmClient>, tool_registry: Arc<ToolRegistry>) -> Self {
        Self { sagitta_code_llm_client, tool_registry }
    }
}

#[async_trait]
impl ReasoningLlmClient for ReasoningLlmClientAdapter {
    async fn generate_stream(
        &self,
        messages: Vec<ReasoningMessage>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ReasoningStreamChunk, ReasoningError>> + Send>>, ReasoningError> {
        
        let sagitta_code_messages: Vec<SagittaCodeMessage> = messages.into_iter().map(|msg| {
            let role = match msg.role.as_str() {
                "user" => SagittaCodeRole::User,
                "assistant" => SagittaCodeRole::Assistant,
                "tool" => SagittaCodeRole::User,
                _ => SagittaCodeRole::User,
            };
            let parts = msg.parts.into_iter().filter_map(|part| {
                match part {
                    ReasoningMessagePart::Text(text) => Some(SagittaCodeMessagePart::Text { text }),
                    ReasoningMessagePart::ToolCall(tool_call_data) => {
                        Some(SagittaCodeMessagePart::ToolCall {
                            tool_call_id: Uuid::new_v4().to_string(),
                            name: tool_call_data.name,
                            parameters: tool_call_data.args,
                        })
                    }
                }
            }).collect();
            SagittaCodeMessage {
                id: Uuid::new_v4(),
                role, 
                parts, 
                metadata: HashMap::new()
            }
        }).collect();

        // Fetch and convert tool definitions
        let agent_tools: Vec<AgentToolDefinition> = self.tool_registry.get_definitions().await;
        let sagitta_code_tools: Vec<crate::llm::client::ToolDefinition> = agent_tools.into_iter().map(|agent_tool_def| {
            crate::llm::client::ToolDefinition {
                name: agent_tool_def.name,
                description: agent_tool_def.description,
                parameters: agent_tool_def.parameters,
                is_required: agent_tool_def.is_required, // This field exists on both
            }
        }).collect();

        match self.sagitta_code_llm_client.generate_stream(&sagitta_code_messages, &sagitta_code_tools).await {
            Ok(sagitta_code_stream) => {
                let reasoning_stream = sagitta_code_stream.map_ok(|sagitta_code_chunk: SagittaCodeStreamChunk| {
                    // If SagittaCodeStreamChunk has token_usage, map it directly
                    if let Some(usage) = sagitta_code_chunk.token_usage {
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

                    // Otherwise, convert SagittaCodeStreamChunk to ReasoningStreamChunk based on part
                    match sagitta_code_chunk.part {
                        SagittaCodeMessagePart::Text { text } => {
                            ReasoningStreamChunk::Text { // Use enum variant
                                content: text,
                                is_final: sagitta_code_chunk.is_final,
                            }
                        }
                        SagittaCodeMessagePart::Thought { text } => { // Added handling for Thought
                             ReasoningStreamChunk::Text { // Use enum variant
                                content: format!("THINKING:{}", text), // Prefix content
                                is_final: sagitta_code_chunk.is_final,
                            }
                        }
                        SagittaCodeMessagePart::ToolCall { name, parameters, tool_call_id: _ } => { // Correctly destructure, ignore tool_call_id for now
                            ReasoningStreamChunk::ToolCall {
                                tool_call: ReasoningToolCall { name, args: parameters }, // Map parameters to args
                                is_final: sagitta_code_chunk.is_final, // Assuming a tool call can be final
                            }
                        }
                        SagittaCodeMessagePart::ToolResult { name: _, result: _, tool_call_id: _ } => {
                            ReasoningStreamChunk::Text {
                                content: "[Received ToolResult from LLM stream - unhandled]".to_string(),
                                is_final: sagitta_code_chunk.is_final,
                            }
                        }
                        // Other SagittaCodeMessagePart types (like ToolResult) would need mapping if they appear here.
                        _ => {
                             // Default for unhandled/empty parts.
                             // Consider if this should be an error or an empty text chunk.
                             // For robustness, an empty text chunk might be safer if the stream can send unexpected empty parts.
                            ReasoningStreamChunk::Text {
                                content: String::new(),
                                is_final: sagitta_code_chunk.is_final,
                            }
                        }
                    }
                }).map_err(|e: SagittaCodeError| ReasoningError::llm(e.to_string()));
                Ok(Box::pin(reasoning_stream))
            }
            Err(e) => Err(ReasoningError::llm(format!("SagittaCodeLlmClient failed to generate stream: {}", e))),
        }
    }
} 