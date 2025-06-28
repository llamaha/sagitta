use async_trait::async_trait;
use std::pin::Pin;
use futures_util::Stream;
use uuid::Uuid;
use tokio::time::Duration;

use crate::config::types::{SagittaCodeConfig, ClaudeCodeConfig};
use crate::llm::client::{
    LlmClient, Message, MessagePart, Role, ToolDefinition, LlmResponse, 
    StreamChunk, ThinkingConfig, GroundingConfig, TokenUsage
};
use crate::utils::errors::SagittaCodeError;
use super::error::ClaudeCodeError;
use super::process::ClaudeProcess;
use super::streaming::ClaudeCodeStream;
use super::message_converter::{convert_messages_to_claude, ClaudeMessage, ClaudeMessageContent, stream_message_as_json};
use super::models::ClaudeCodeModel;

/// Process timeout for Claude Code (10 minutes like Roo-Code)
const CLAUDE_CODE_TIMEOUT: Duration = Duration::from_secs(600);

/// Claude Code client implementing the LlmClient trait
pub struct ClaudeCodeClient {
    config: ClaudeCodeConfig,
    process_manager: ClaudeProcess,
}

impl ClaudeCodeClient {
    /// Create a new Claude Code client
    pub fn new(config: &SagittaCodeConfig) -> Result<Self, SagittaCodeError> {
        let claude_config = config.claude_code.clone();
        
        // Verify model exists
        if ClaudeCodeModel::find_by_id(&claude_config.model).is_none() {
            return Err(SagittaCodeError::ConfigError(
                format!("Unknown Claude model: {}", claude_config.model)
            ));
        }
        
        log::info!("CLAUDE_CODE: Initializing client with model: {}", claude_config.model);
        log::debug!("CLAUDE_CODE: Binary path: {}", claude_config.claude_path);
        
        Ok(Self {
            process_manager: ClaudeProcess::new(claude_config.clone()),
            config: claude_config,
        })
    }
    
    /// Extract system prompt from messages
    pub fn extract_system_prompt(messages: &[Message]) -> String {
        messages.iter()
            .find(|m| matches!(m.role, Role::System))
            .and_then(|m| {
                m.parts.iter()
                    .find_map(|p| match p {
                        MessagePart::Text { text } => Some(text.clone()),
                        _ => None,
                    })
            })
            .unwrap_or_else(|| "You are a helpful AI assistant.".to_string())
    }
    
    /// Filter out system messages as they're handled separately
    pub fn filter_non_system_messages(messages: &[Message]) -> Vec<Message> {
        messages.iter()
            .filter(|m| !matches!(m.role, Role::System))
            .cloned()
            .collect()
    }
}

#[async_trait]
impl LlmClient for ClaudeCodeClient {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    async fn generate(
        &self, 
        messages: &[Message], 
        tools: &[ToolDefinition]
    ) -> Result<LlmResponse, SagittaCodeError> {
        log::debug!("CLAUDE_CODE: Generate called with {} messages and {} tools", messages.len(), tools.len());
        
        // Get only the latest user message as the prompt
        let user_messages: Vec<&Message> = messages.iter()
            .filter(|m| matches!(m.role, Role::User))
            .collect();
        
        let prompt = if let Some(last_user_msg) = user_messages.last() {
            // Extract text from the message parts
            last_user_msg.parts.iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            log::warn!("CLAUDE_CODE: No user messages found, using default prompt");
            "Hello".to_string()
        };
        
        // Spawn process with prompt
        let child = self.process_manager
            .spawn(&prompt)
            .await
            .map_err(|e| SagittaCodeError::LlmError(e.to_string()))?;
        
        let stream = ClaudeCodeStream::new(child);
        
        // Collect all chunks
        let mut message_parts = Vec::new();
        let mut token_usage = None;
        let mut is_final = false;
        
        tokio::pin!(stream);
        while let Some(result) = futures_util::StreamExt::next(&mut stream).await {
            match result {
                Ok(chunk) => {
                    if !matches!(chunk.part, MessagePart::Text { ref text } if text.is_empty()) {
                        message_parts.push(chunk.part);
                    }
                    if chunk.is_final {
                        is_final = true;
                        token_usage = chunk.token_usage;
                    }
                }
                Err(e) => return Err(e),
            }
        }
        
        if !is_final {
            return Err(SagittaCodeError::LlmError("Stream ended without final chunk".to_string()));
        }
        
        // Separate tool calls from other message parts
        let mut tool_calls = Vec::new();
        let mut other_parts = Vec::new();
        
        for part in message_parts {
            match part {
                MessagePart::ToolCall { tool_call_id, name, parameters } => {
                    tool_calls.push((tool_call_id, name, parameters));
                }
                _ => {
                    other_parts.push(part);
                }
            }
        }
        
        Ok(LlmResponse {
            message: Message {
                id: Uuid::new_v4(),
                role: Role::Assistant,
                parts: other_parts,
                metadata: Default::default(),
            },
            tool_calls,
            usage: token_usage,
            grounding: None,
        })
    }
    
    async fn generate_with_thinking(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        log::debug!("CLAUDE_CODE: Thinking mode requested (budget: {:?})", thinking_config.thinking_budget);
        // Claude Code always includes thinking when available
        self.generate(messages, tools).await
    }
    
    async fn generate_with_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        log::warn!("CLAUDE_CODE: Grounding not supported by Claude Code");
        self.generate(messages, tools).await
    }
    
    async fn generate_with_thinking_and_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
        _grounding_config: &GroundingConfig,
    ) -> Result<LlmResponse, SagittaCodeError> {
        log::warn!("CLAUDE_CODE: Grounding not supported by Claude Code");
        self.generate_with_thinking(messages, tools, thinking_config).await
    }
    
    async fn generate_stream(
        &self, 
        messages: &[Message], 
        tools: &[ToolDefinition]
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        log::debug!("CLAUDE_CODE: Generate stream called with {} messages and {} tools", messages.len(), tools.len());
        
        // Get only the latest user message as the prompt
        let user_messages: Vec<&Message> = messages.iter()
            .filter(|m| matches!(m.role, Role::User))
            .collect();
        
        let prompt = if let Some(last_user_msg) = user_messages.last() {
            // Extract text from the message parts
            last_user_msg.parts.iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            log::warn!("CLAUDE_CODE: No user messages found, using default prompt");
            "Hello".to_string()
        };
        
        // Spawn process with prompt
        let child = self.process_manager
            .spawn(&prompt)
            .await
            .map_err(|e| SagittaCodeError::LlmError(e.to_string()))?;
        
        let stream = ClaudeCodeStream::new(child);
        Ok(Box::pin(stream))
    }
    
    async fn generate_stream_with_thinking(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        log::debug!("CLAUDE_CODE: Thinking stream requested (budget: {:?})", thinking_config.thinking_budget);
        self.generate_stream(messages, tools).await
    }
    
    async fn generate_stream_with_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        log::warn!("CLAUDE_CODE: Grounding not supported by Claude Code");
        self.generate_stream(messages, tools).await
    }
    
    async fn generate_stream_with_thinking_and_grounding(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        thinking_config: &ThinkingConfig,
        _grounding_config: &GroundingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, SagittaCodeError>> + Send>>, SagittaCodeError> {
        log::warn!("CLAUDE_CODE: Grounding not supported by Claude Code");
        self.generate_stream_with_thinking(messages, tools, thinking_config).await
    }
}
