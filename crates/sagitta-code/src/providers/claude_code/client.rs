use async_trait::async_trait;
use std::pin::Pin;
use futures_util::Stream;
use uuid::Uuid;
use tokio::time::Duration;

use crate::config::types::{SagittaCodeConfig, ClaudeCodeConfig};
use crate::llm::client::{
    LlmClient, Message, MessagePart, Role, ToolDefinition, LlmResponse, 
    StreamChunk, ThinkingConfig, GroundingConfig
};
use crate::utils::errors::SagittaCodeError;
use super::process::ClaudeProcess;
use super::streaming::ClaudeCodeStream;
use tokio_util::sync::CancellationToken;
use std::sync::Arc;
use super::models::ClaudeCodeModel;
use super::claude_interface::{ClaudeInterface, ClaudeModelInfo, ClaudeConfigInfo};
use super::mcp_integration::McpIntegration;
// Tool registry removed - tools now via MCP

/// Process timeout for Claude Code (10 minutes like Roo-Code)
const CLAUDE_CODE_TIMEOUT: Duration = Duration::from_secs(600);

/// Claude Code client implementing the LlmClient trait
pub struct ClaudeCodeClient {
    config: ClaudeCodeConfig,
    process_manager: ClaudeProcess,
    interface: ClaudeInterface,
    mcp_integration: Option<McpIntegration>,
    mcp_config_path: Option<String>,
    cancellation_token: Arc<tokio::sync::Mutex<CancellationToken>>,
}

impl ClaudeCodeClient {
    /// Create a new Claude Code client
    pub fn new(config: &SagittaCodeConfig) -> Result<Self, SagittaCodeError> {
        let claude_config = config.claude_code.clone()
            .unwrap_or_else(|| crate::config::types::ClaudeCodeConfig::default());
        
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
            interface: ClaudeInterface::new(claude_config.clone()),
            config: claude_config,
            mcp_integration: None,
            mcp_config_path: None,
            cancellation_token: Arc::new(tokio::sync::Mutex::new(CancellationToken::new())),
        })
    }
    
    /// Initialize MCP integration (tools provided by sagitta-mcp)
    pub async fn initialize_mcp(&mut self, _tool_registry: Option<()>) -> Result<(), SagittaCodeError> {
        log::info!("CLAUDE_CODE: Initializing MCP integration");
        
        let mut mcp = McpIntegration::new();
        
        // Start the MCP server and get config
        let mcp_config = mcp.start().await
            .map_err(|e| SagittaCodeError::LlmError(format!("Failed to start MCP: {e}")))?;
        
        // Extract the config path
        if let Some(path) = mcp_config.get("mcp_config_path").and_then(|v| v.as_str()) {
            self.mcp_config_path = Some(path.to_string());
            log::info!("CLAUDE_CODE: MCP config created at: {path}");
        }
        
        self.mcp_integration = Some(mcp);
        Ok(())
    }
    
    /// Cancel any ongoing stream
    pub async fn cancel(&self) {
        log::info!("CLAUDE_CODE: Cancelling client");
        let token = self.cancellation_token.lock().await;
        token.cancel();
        log::info!("CLAUDE_CODE: Cancellation token cancelled");
    }
    
    /// Get or create a fresh cancellation token for a new stream
    async fn get_fresh_cancellation_token(&self) -> Arc<CancellationToken> {
        let mut token_guard = self.cancellation_token.lock().await;
        if token_guard.is_cancelled() {
            // Create a new token since the old one was cancelled
            *token_guard = CancellationToken::new();
            log::debug!("CLAUDE_CODE: Created fresh cancellation token");
        }
        Arc::new(token_guard.child_token())
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
    
    /// Get MCP integration details for tests
    pub fn get_mcp_integration(&self) -> Option<serde_json::Value> {
        if self.mcp_integration.is_some() && self.mcp_config_path.is_some() {
            // Read the config to get the actual server name
            if let Ok(content) = std::fs::read_to_string(self.mcp_config_path.as_ref().unwrap()) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(servers) = config["mcpServers"].as_object() {
                        if let Some(server_name) = servers.keys().next() {
                            return Some(serde_json::json!({
                                "mcp_config_path": self.mcp_config_path.as_ref().unwrap(),
                                "server_name": server_name
                            }));
                        }
                    }
                }
            }
            None
        } else {
            None
        }
    }
    
    /// Get Claude CLI arguments with MCP config
    pub fn get_claude_cli_args(&self) -> Option<Vec<String>> {
        self.mcp_config_path.as_ref().map(|path| {
            vec!["--mcp-config".to_string(), path.clone()]
        })
    }
}

#[async_trait]
impl LlmClient for ClaudeCodeClient {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    
    fn client_type(&self) -> &str {
        "claude_code"
    }
    
    async fn generate(
        &self, 
        messages: &[Message], 
        tools: &[ToolDefinition]
    ) -> Result<LlmResponse, SagittaCodeError> {
        log::debug!("CLAUDE_CODE: Generate called with {} messages and {} tools", messages.len(), tools.len());
        
        // Build conversation history as a prompt
        let mut prompt = String::new();
        
        for message in messages {
            match message.role {
                Role::User => {
                    let text = message.parts.iter()
                        .filter_map(|part| match part {
                            MessagePart::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    prompt.push_str(&format!("Human: {text}\n\n"));
                }
                Role::Assistant => {
                    // Only include actual text content, not thinking
                    let text = message.parts.iter()
                        .filter_map(|part| match part {
                            MessagePart::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !text.is_empty() {
                        prompt.push_str(&format!("Assistant: {text}\n\n"));
                    }
                }
                Role::System => {
                    // Skip system messages for Claude CLI
                    log::debug!("CLAUDE_CODE: Skipping system message in prompt");
                }
                Role::Function => {
                    // Skip function messages for Claude CLI
                    log::debug!("CLAUDE_CODE: Skipping function message in prompt");
                }
            }
        }
        
        // Ensure we have a prompt
        if prompt.trim().is_empty() {
            log::warn!("CLAUDE_CODE: Empty prompt after processing messages");
            prompt = "Human: Hello\n\n".to_string();
        }
        
        log::debug!("CLAUDE_CODE: Final prompt:\n{}", prompt);
        
        // Spawn process with prompt and MCP config if available
        let child = if let Some(ref mcp_path) = self.mcp_config_path {
            self.process_manager
                .spawn_with_mcp(&prompt, Some(mcp_path))
                .await
                .map_err(|e| SagittaCodeError::LlmError(e.to_string()))?
        } else {
            self.process_manager
                .spawn(&prompt)
                .await
                .map_err(|e| SagittaCodeError::LlmError(e.to_string()))?
        };
        
        let fresh_token = self.get_fresh_cancellation_token().await;
        let stream = ClaudeCodeStream::new_with_cancellation(child, fresh_token);
        
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
        
        // Build conversation history as a prompt
        let mut prompt = String::new();
        
        for message in messages {
            match message.role {
                Role::User => {
                    let text = message.parts.iter()
                        .filter_map(|part| match part {
                            MessagePart::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    prompt.push_str(&format!("Human: {text}\n\n"));
                }
                Role::Assistant => {
                    // Only include actual text content, not thinking
                    let text = message.parts.iter()
                        .filter_map(|part| match part {
                            MessagePart::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !text.is_empty() {
                        prompt.push_str(&format!("Assistant: {text}\n\n"));
                    }
                }
                Role::System => {
                    // Skip system messages for Claude CLI
                    log::debug!("CLAUDE_CODE: Skipping system message in prompt");
                }
                Role::Function => {
                    // Skip function messages for Claude CLI
                    log::debug!("CLAUDE_CODE: Skipping function message in prompt");
                }
            }
        }
        
        // Ensure we have a prompt
        if prompt.trim().is_empty() {
            log::warn!("CLAUDE_CODE: Empty prompt after processing messages");
            prompt = "Human: Hello\n\n".to_string();
        }
        
        log::debug!("CLAUDE_CODE: Final prompt:\n{}", prompt);
        
        // Spawn process with prompt and MCP config if available
        let child = if let Some(ref mcp_path) = self.mcp_config_path {
            self.process_manager
                .spawn_with_mcp(&prompt, Some(mcp_path))
                .await
                .map_err(|e| SagittaCodeError::LlmError(e.to_string()))?
        } else {
            self.process_manager
                .spawn(&prompt)
                .await
                .map_err(|e| SagittaCodeError::LlmError(e.to_string()))?
        };
        
        let fresh_token = self.get_fresh_cancellation_token().await;
        let stream = ClaudeCodeStream::new_with_cancellation(child, fresh_token);
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

impl ClaudeCodeClient {
    /// Get the Claude interface for advanced configuration
    pub fn interface(&self) -> &ClaudeInterface {
        &self.interface
    }

    /// Get model information
    pub fn get_model_info(&self) -> Result<ClaudeModelInfo, SagittaCodeError> {
        self.interface.get_model_info()
    }

    /// Get complete configuration information
    pub async fn get_config_info(&self) -> Result<ClaudeConfigInfo, SagittaCodeError> {
        self.interface.get_config_info().await
    }

    /// Validate the Claude binary and configuration
    pub async fn validate(&self) -> Result<(), SagittaCodeError> {
        self.interface.validate().await
    }

    /// Get available models
    pub async fn get_available_models(&self) -> Result<Vec<String>, SagittaCodeError> {
        self.interface.get_available_models().await
    }
}

impl Drop for ClaudeCodeClient {
    fn drop(&mut self) {
        // Clean up MCP integration
        if let Some(mut mcp) = self.mcp_integration.take() {
            // We can't use async in drop, so we spawn a task to clean up
            tokio::spawn(async move {
                mcp.stop().await;
            });
        }
    }
}
