use async_trait::async_trait;
use std::pin::Pin;
use futures_util::Stream;
use uuid::Uuid;

use crate::config::types::{SagittaCodeConfig, ClaudeCodeConfig};
use crate::llm::client::{
    LlmClient, Message, MessagePart, Role, ToolDefinition, LlmResponse, 
    StreamChunk, ThinkingConfig, GroundingConfig, TokenUsage
};
use crate::utils::errors::SagittaCodeError;
use super::error::ClaudeCodeError;
use super::process::ClaudeProcess;
use super::streaming::ClaudeCodeStream;
use super::message_converter::{convert_messages_to_claude, ClaudeMessage};
use super::models::ClaudeCodeModel;

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
    
    /// Format tools as text for inclusion in system prompt
    pub fn format_tools_for_system_prompt(tools: &[ToolDefinition]) -> String {
        if tools.is_empty() {
            return String::new();
        }
        
        let mut prompt = String::from("\n\n## Available Tools\n\n");
        prompt.push_str("You have access to the following tools. Use them by writing XML tags in your response:\n\n");
        
        for tool in tools {
            prompt.push_str(&format!("### {}\n", tool.name));
            prompt.push_str(&format!("Description: {}\n", tool.description));
            prompt.push_str("Usage:\n```xml\n");
            prompt.push_str(&format!("<tool_use>\n<tool_name>{}</tool_name>\n<parameters>\n", tool.name));
            
            // Add parameter schema as example
            if let Some(props) = tool.parameters.get("properties").and_then(|p| p.as_object()) {
                for (param_name, param_schema) in props {
                    let param_type = param_schema.get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("string");
                    let description = param_schema.get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("");
                    prompt.push_str(&format!("  <{}>{}</{}> <!-- {} -->\n", 
                        param_name, 
                        match param_type {
                            "string" => "value",
                            "number" | "integer" => "123",
                            "boolean" => "true",
                            "array" => "[item1, item2]",
                            "object" => "{\"key\": \"value\"}",
                            _ => "value"
                        },
                        param_name,
                        description
                    ));
                }
            }
            
            prompt.push_str("</parameters>\n</tool_use>\n```\n\n");
        }
        
        prompt.push_str("When you need to use a tool, include the XML block in your response. ");
        prompt.push_str("You may use multiple tools in a single response.\n");
        
        prompt
    }
    
    /// Filter out system messages as they're handled separately
    pub fn filter_non_system_messages(messages: &[Message]) -> Vec<Message> {
        messages.iter()
            .filter(|m| !matches!(m.role, Role::System))
            .cloned()
            .collect()
    }
    
    /// Get list of tools to disable (all Claude built-in tools)
    pub fn get_disabled_tools() -> Vec<String> {
        vec![
            // Claude's built-in tools
            "Task", "Bash", "Glob", "Grep", "LS", "exit_plan_mode",
            "Read", "Edit", "MultiEdit", "Write", "NotebookRead",
            "NotebookEdit", "WebFetch", "TodoRead", "TodoWrite", "WebSearch",
            // MCP tools that might be present
            "mcp__sagitta-mcp-stdio__ping",
            "mcp__sagitta-mcp-stdio__repository_add",
            "mcp__sagitta-mcp-stdio__repository_list",
            "mcp__sagitta-mcp-stdio__repository_remove",
            "mcp__sagitta-mcp-stdio__repository_sync",
            "mcp__sagitta-mcp-stdio__query",
            "mcp__sagitta-mcp-stdio__repository_search_file",
            "mcp__sagitta-mcp-stdio__repository_view_file",
            "mcp__sagitta-mcp-stdio__repository_map",
            "mcp__sagitta-mcp-stdio__repository_switch_branch",
            "mcp__sagitta-mcp-stdio__repository_list_branches"
        ].iter().map(|s| s.to_string()).collect()
    }
}

#[async_trait]
impl LlmClient for ClaudeCodeClient {
    async fn generate(
        &self, 
        messages: &[Message], 
        tools: &[ToolDefinition]
    ) -> Result<LlmResponse, SagittaCodeError> {
        log::debug!("CLAUDE_CODE: Generate called with {} messages and {} tools", messages.len(), tools.len());
        
        // Extract base system prompt and append tools
        let mut system_prompt = Self::extract_system_prompt(messages);
        if !tools.is_empty() {
            system_prompt.push_str(&Self::format_tools_for_system_prompt(tools));
            log::debug!("CLAUDE_CODE: Added {} tools to system prompt", tools.len());
        }
        
        let filtered_messages = Self::filter_non_system_messages(messages);
        let claude_messages = convert_messages_to_claude(&filtered_messages);
        
        // Spawn process and collect response
        let child = self.process_manager
            .spawn(&system_prompt, &claude_messages, &Self::get_disabled_tools())
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
        
        // Extract base system prompt and append tools
        let mut system_prompt = Self::extract_system_prompt(messages);
        if !tools.is_empty() {
            system_prompt.push_str(&Self::format_tools_for_system_prompt(tools));
            log::debug!("CLAUDE_CODE: Added {} tools to system prompt", tools.len());
        }
        
        let filtered_messages = Self::filter_non_system_messages(messages);
        let claude_messages = convert_messages_to_claude(&filtered_messages);
        
        // Spawn process
        let child = self.process_manager
            .spawn(&system_prompt, &claude_messages, &Self::get_disabled_tools())
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