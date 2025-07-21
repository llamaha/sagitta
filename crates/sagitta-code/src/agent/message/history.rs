// Message history management will go here

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use std::collections::HashMap;
use log::{debug, info, warn, error, trace};

use crate::agent::message::types::AgentMessage;
use crate::llm::client::{Message as LlmMessage, Role, MessagePart};
use crate::utils::errors::SagittaCodeError;
use crate::agent::conversation::manager::{ConversationManager, ConversationManagerImpl};
use crate::agent::conversation::types::{Conversation, ProjectContext, ProjectType};
use crate::config::types::ConversationConfig;
use crate::llm::token_counter::TokenCounter;

/// A history of messages in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHistory {
    /// The messages in the history
    pub messages: Vec<AgentMessage>,
    
    /// Maximum context tokens (0 = no limit)
    #[serde(default)]
    pub max_context_tokens: usize,
    
    /// Total tokens in the current history
    #[serde(skip, default)]
    pub total_tokens: usize,
    
    /// Token counter instance
    #[serde(skip)]
    token_counter: Option<TokenCounter>,
}

impl Default for MessageHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageHistory {
    /// Create a new empty message history
    pub fn new() -> Self {
        Self::with_token_limit(0)
    }
    
    /// Create a new message history with token limit
    pub fn with_token_limit(max_context_tokens: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_context_tokens,
            total_tokens: 0,
            token_counter: TokenCounter::new().ok(),
        }
    }
    
    /// Create a new message history with a system prompt
    pub fn with_system_prompt(system_prompt: impl Into<String>) -> Self {
        let mut history = Self::new();
        history.add_system_message(system_prompt);
        history
    }
    
    /// Get or initialize the token counter
    fn get_token_counter(&mut self) -> Option<&TokenCounter> {
        if self.token_counter.is_none() {
            self.token_counter = TokenCounter::new().ok();
        }
        self.token_counter.as_ref()
    }
    
    /// Calculate tokens for a message
    fn calculate_message_tokens(&mut self, message: &AgentMessage) -> usize {
        let Some(counter) = self.get_token_counter() else {
            // Fallback: estimate 4 characters per token
            return message.content.len() / 4;
        };
        
        let role_str = match message.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Function => "function",
        };
        
        counter.count_message_tokens(role_str, &message.content)
    }
    
    /// Add a new message to the history
    pub fn add_message(&mut self, message: AgentMessage) {
        // Calculate tokens for the new message
        let new_message_tokens = self.calculate_message_tokens(&message);
        
        // Add the message first
        self.messages.push(message);
        self.total_tokens += new_message_tokens;
        
        // Apply token-based truncation if we have a token limit
        if self.max_context_tokens > 0 {
            self.truncate_to_token_limit(self.max_context_tokens);
        }
        
        debug!("Message history: {} messages, ~{} tokens (limit: {})", 
            self.messages.len(), 
            self.total_tokens, 
            if self.max_context_tokens > 0 { self.max_context_tokens.to_string() } else { "none".to_string() }
        );
    }
    
    /// Add a system message to the history
    pub fn add_system_message(&mut self, content: impl Into<String>) {
        self.add_message(AgentMessage::system(content));
    }
    
    /// Add a user message to the history
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.add_message(AgentMessage::user(content));
    }
    
    /// Add an assistant message to the history
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        self.add_message(AgentMessage::assistant(content));
    }
    
    /// Get a message by its ID
    pub fn get_message(&self, id: Uuid) -> Option<&AgentMessage> {
        self.messages.iter().find(|m| m.id == id)
    }
    
    /// Get a mutable reference to a message by its ID
    pub fn get_message_mut(&mut self, id: Uuid) -> Option<&mut AgentMessage> {
        self.messages.iter_mut().find(|m| m.id == id)
    }
    
    /// Remove a message by its ID
    pub fn remove_message(&mut self, id: Uuid) -> Option<AgentMessage> {
        if let Some(index) = self.messages.iter().position(|m| m.id == id) {
            let removed_msg = self.messages.remove(index);
            let removed_tokens = self.calculate_message_tokens(&removed_msg);
            self.total_tokens = self.total_tokens.saturating_sub(removed_tokens);
            Some(removed_msg)
        } else {
            None
        }
    }
    
    /// Get the current total token count
    pub fn get_total_tokens(&self) -> usize {
        self.total_tokens
    }
    
    /// Check if we're approaching a token limit
    pub fn is_approaching_token_limit(&self, max_tokens: usize) -> bool {
        if max_tokens == 0 {
            false // No limit set
        } else {
            self.total_tokens > (max_tokens * 8 / 10) // 80% threshold
        }
    }
    
    /// Truncate history to fit within token limit
    pub fn truncate_to_token_limit(&mut self, max_tokens: usize) {
        if max_tokens == 0 || self.total_tokens <= max_tokens {
            return;
        }
        
        info!("Truncating message history from {} tokens to fit within {} limit", self.total_tokens, max_tokens);
        
        // Keep system messages and remove oldest non-system messages
        while self.total_tokens > max_tokens && self.messages.len() > 1 {
            if let Some(index) = self.messages.iter()
                .enumerate()
                .filter(|(_, msg)| msg.role != Role::System)
                .map(|(i, _)| i)
                .next()
            {
                let removed_msg = self.messages.remove(index);
                let removed_tokens = self.calculate_message_tokens(&removed_msg);
                self.total_tokens = self.total_tokens.saturating_sub(removed_tokens);
                debug!("Removed message at index {index} (~{removed_tokens} tokens)");
            } else {
                break; // Only system messages left
            }
        }
    }
    
    /// Recalculate total tokens from all messages
    pub fn recalculate_total_tokens(&mut self) {
        self.total_tokens = 0;
        let messages = self.messages.clone(); // Clone to avoid borrow issues
        for msg in messages {
            self.total_tokens += self.calculate_message_tokens(&msg);
        }
    }
    
    /// Get the messages as LlmMessages for the LlmClient
    pub fn to_llm_messages(&self) -> Vec<LlmMessage> {
        let mut llm_messages = Vec::new();
        trace!("MessageHistory: Converting {} agent messages to LLM messages.", self.messages.len());

        for (idx, agent_msg) in self.messages.iter().enumerate() {
            trace!("MessageHistory: Processing agent_msg #{} with ID: {} and Role: {:?}", idx, agent_msg.id, agent_msg.role);
            if agent_msg.role == Role::User || agent_msg.role == Role::System {
                llm_messages.push(agent_msg.to_llm_message());
                trace!("MessageHistory: Added {:?} message directly.", agent_msg.role);
                continue;
            }
            
            // Handle Function role messages (tool results)
            if agent_msg.role == Role::Function {
                // Function messages contain tool results
                for tc in &agent_msg.tool_calls {
                    if let Some(result_val) = &tc.result {
                        llm_messages.push(LlmMessage {
                            id: agent_msg.id,
                            role: Role::Function,
                            parts: vec![MessagePart::ToolResult {
                                tool_call_id: tc.id.clone(),
                                name: tc.name.clone(),
                                result: result_val.clone(),
                            }],
                            metadata: HashMap::new(),
                        });
                        trace!("MessageHistory: Added Function message with tool result for {}", tc.name);
                    }
                }
                continue;
            }

            if agent_msg.role == Role::Assistant {
                let mut assistant_parts = Vec::new();
                if !agent_msg.content.is_empty() {
                    assistant_parts.push(MessagePart::Text { text: agent_msg.content.clone() });
                    trace!("MessageHistory: Assistant message (ID: {}) adding text part.", agent_msg.id);
                }

                for tc in &agent_msg.tool_calls {
                    if tc.result.is_none() { 
                        assistant_parts.push(MessagePart::ToolCall {
                            tool_call_id: tc.id.clone(),
                            name: tc.name.clone(),
                            parameters: tc.arguments.clone(),
                        });
                        trace!("MessageHistory: Assistant message (ID: {}) adding ToolCall part for LLM: ID: {}, Name: {}", agent_msg.id, tc.id, tc.name);
                    }
                }
                
                if !assistant_parts.is_empty() {
                    let assistant_metadata: HashMap<String, serde_json::Value> = agent_msg.metadata.iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();

                    llm_messages.push(LlmMessage {
                        id: agent_msg.id, 
                        role: Role::Assistant,
                        parts: assistant_parts, 
                        metadata: assistant_metadata,
                    });
                    trace!("MessageHistory: Added Assistant LLM message (ID: {}) with {} parts.", agent_msg.id, llm_messages.last().unwrap().parts.len());
                }

                for tc in &agent_msg.tool_calls {
                    if let Some(result_val) = &tc.result {
                        let response_payload;
                        if tc.successful {
                            response_payload = result_val.clone();
                            trace!("MessageHistory: ToolResult (ID: {}, Name: {}) successful. Payload: {:?}", tc.id, tc.name, response_payload);
                        } else {
                            if let Some(err_str) = result_val.get("error").and_then(|v| v.as_str()) {
                                response_payload = serde_json::json!({ "error": err_str });
                            } else if let Some(direct_err_str) = result_val.as_str() {
                                response_payload = serde_json::json!({ "error": direct_err_str });
                            } else {
                                response_payload = serde_json::json!({ "error": "Tool execution failed with an unknown error structure." });
                            }
                            warn!("MessageHistory: ToolResult (ID: {}, Name: {}) failed. Error payload for LLM: {:?}", tc.id, tc.name, response_payload);
                        }

                        llm_messages.push(LlmMessage {
                            id: Uuid::new_v4(), 
                            role: Role::Function,
                            parts: vec![MessagePart::ToolResult {
                                tool_call_id: tc.id.clone(),
                                name: tc.name.clone(),
                                result: response_payload,
                            }],
                            metadata: HashMap::new(), 
                        });
                        trace!("MessageHistory: Added Function LLM message for ToolResult (Original Assistant ID: {}, Tool ID: {}).", agent_msg.id, tc.id);
                    }
                }
            }
        }
        trace!("MessageHistory: Prepared {} LLM messages. Roles: {:?}", llm_messages.len(), llm_messages.iter().map(|m| m.role.clone()).collect::<Vec<_>>());
        llm_messages
    }
    
    /// Add a tool call result to the most recent assistant message
    pub fn add_tool_result(&mut self, tool_call_id: &str, result: serde_json::Value, successful: bool) -> Result<(), SagittaCodeError> {
        trace!("MessageHistory: Adding tool result for Call ID: '{tool_call_id}'. Successful: {successful}. Result: {result:?}");
        // Find the most recent assistant message with a matching tool call
        for message in self.messages.iter_mut().rev() {
            if message.role == Role::Assistant {
                for tool_call in message.tool_calls.iter_mut() {
                    if tool_call.id == tool_call_id {
                        tool_call.result = Some(result.clone());
                        tool_call.successful = successful;
                        tool_call.execution_time = Some(chrono::Utc::now());
                        return Ok(());
                    }
                }
            }
        }
        
        Err(SagittaCodeError::Unknown(format!("No tool call found with ID {tool_call_id}")))
    }
    
    /// Clear all messages except system messages
    pub fn clear_except_system(&mut self) {
        self.messages.retain(|m| m.role == Role::System);
    }
    
    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
    }
    
    /// Get the number of messages in the history
    pub fn len(&self) -> usize {
        self.messages.len()
    }
    
    /// Check if the history is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

/// A thread-safe wrapper around MessageHistory
#[derive(Debug, Clone)]
pub struct MessageHistoryManager {
    history: Arc<RwLock<MessageHistory>>,
}

impl Default for MessageHistoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageHistoryManager {
    /// Create a new MessageHistoryManager
    pub fn new() -> Self {
        Self {
            history: Arc::new(RwLock::new(MessageHistory::new())),
        }
    }
    
    /// Create a new MessageHistoryManager with token limit
    pub fn with_token_limit(max_context_tokens: usize) -> Self {
        Self {
            history: Arc::new(RwLock::new(MessageHistory::with_token_limit(max_context_tokens))),
        }
    }
    
    /// Create a new MessageHistoryManager with a system prompt
    pub fn with_system_prompt(system_prompt: impl Into<String>) -> Self {
        Self {
            history: Arc::new(RwLock::new(MessageHistory::with_system_prompt(system_prompt))),
        }
    }
    
    /// Add a message to the history
    pub async fn add_message(&self, message: AgentMessage) {
        let mut history = self.history.write().await;
        history.add_message(message);
    }
    
    /// Add a system message to the history
    pub async fn add_system_message(&self, content: impl Into<String>) {
        let mut history = self.history.write().await;
        history.add_system_message(content);
    }
    
    /// Add a user message to the history
    pub async fn add_user_message(&self, content: impl Into<String>) {
        let mut history = self.history.write().await;
        let content_str = content.into();
        debug!("MessageHistoryManager: Adding user message: '{content_str}'");
        history.add_user_message(content_str);
    }
    
    /// Add an assistant message to the history
    pub async fn add_assistant_message(&self, content: impl Into<String>) {
        let mut history = self.history.write().await;
        let content_str = content.into();
        debug!("MessageHistoryManager: Adding assistant message: '{content_str}'");
        history.add_assistant_message(content_str);
    }
    
    /// Get a message by its ID
    pub async fn get_message(&self, id: Uuid) -> Option<AgentMessage> {
        let history = self.history.read().await;
        history.get_message(id).cloned()
    }
    
    /// Remove a message by its ID
    pub async fn remove_message(&self, id: Uuid) -> Option<AgentMessage> {
        let mut history = self.history.write().await;
        history.remove_message(id)
    }
    
    /// Get all messages in the history
    pub async fn get_messages(&self) -> Vec<AgentMessage> {
        let history = self.history.read().await;
        history.messages.clone()
    }
    
    /// Get the messages as LlmMessages for the LlmClient
    pub async fn to_llm_messages(&self) -> Vec<LlmMessage> {
        let history = self.history.read().await;
        history.to_llm_messages()
    }
    
    /// Add a tool call result to the most recent assistant message
    pub async fn add_tool_result(&self, tool_call_id: &str, result: serde_json::Value, successful: bool) -> Result<(), SagittaCodeError> {
        let mut history = self.history.write().await;
        history.add_tool_result(tool_call_id, result, successful)
    }
    
    /// Clear all messages except system messages
    pub async fn clear_except_system(&self) {
        let mut history = self.history.write().await;
        history.clear_except_system();
    }
    
    /// Clear all messages
    pub async fn clear(&self) {
        let mut history = self.history.write().await;
        history.clear();
    }
    
    /// Get the number of messages in the history
    pub async fn len(&self) -> usize {
        let history = self.history.read().await;
        history.len()
    }
    
    /// Check if the history is empty
    pub async fn is_empty(&self) -> bool {
        let history = self.history.read().await;
        history.is_empty()
    }
}

/// A conversation-aware wrapper that implements MessageHistoryManager interface
/// but uses the advanced conversation management system internally
#[derive(Clone)]
pub struct ConversationAwareHistoryManager {
    /// The conversation manager
    conversation_manager: Arc<tokio::sync::Mutex<ConversationManagerImpl>>,
    
    /// Current active conversation ID
    current_conversation_id: Arc<RwLock<Option<Uuid>>>,
    
    /// System prompt for new conversations
    system_prompt: String,
    
    /// Configuration
    config: ConversationConfig,
    
    /// Workspace ID for project context
    workspace_id: Option<Uuid>,
    
    /// Maximum context tokens for history truncation
    max_context_tokens: usize,
}

impl ConversationAwareHistoryManager {
    /// Create a new ConversationAwareHistoryManager
    pub async fn new(
        conversation_manager: ConversationManagerImpl,
        config: ConversationConfig,
        workspace_id: Option<Uuid>,
        max_context_tokens: usize,
    ) -> Result<Self, SagittaCodeError> {
        Ok(Self {
            conversation_manager: Arc::new(tokio::sync::Mutex::new(conversation_manager)),
            current_conversation_id: Arc::new(RwLock::new(None)),
            system_prompt: String::new(),
            config,
            workspace_id,
            max_context_tokens,
        })
    }
    
    /// Create a new ConversationAwareHistoryManager with a system prompt
    pub async fn with_system_prompt(
        conversation_manager: ConversationManagerImpl,
        config: ConversationConfig,
        workspace_id: Option<Uuid>,
        max_context_tokens: usize,
        system_prompt: impl Into<String>,
    ) -> Result<Self, SagittaCodeError> {
        Ok(Self {
            conversation_manager: Arc::new(tokio::sync::Mutex::new(conversation_manager)),
            current_conversation_id: Arc::new(RwLock::new(None)),
            system_prompt: system_prompt.into(),
            config,
            workspace_id,
            max_context_tokens,
        })
    }
    
    /// Ensure we have an active conversation, creating one if needed
    async fn ensure_active_conversation(&self) -> Result<Uuid, SagittaCodeError> {
        let current_id = {
            let current_id_guard = self.current_conversation_id.read().await;
            *current_id_guard
        };
        
        if let Some(id) = current_id {
            // Verify the conversation still exists
            let manager = self.conversation_manager.lock().await;
            if let Ok(Some(_)) = manager.get_conversation(id).await {
                return Ok(id);
            }
        }
        
        // Create a new conversation
        if self.config.auto_create {
            self.create_new_conversation().await
        } else {
            Err(SagittaCodeError::Unknown("No active conversation and auto-create is disabled".to_string()))
        }
    }
    
    /// Create a new conversation
    async fn create_new_conversation(&self) -> Result<Uuid, SagittaCodeError> {
        let title = format!("Conversation {}", chrono::Utc::now().format("%Y-%m-%d %H:%M"));
        
        let conversation_id = {
            let manager = self.conversation_manager.lock().await;
            manager.create_conversation(title, self.workspace_id).await
                .map_err(|e| SagittaCodeError::Unknown(format!("Failed to create conversation: {e}")))?
        }; // Release the lock here
        
        // Set as current conversation
        {
            let mut current_id_guard = self.current_conversation_id.write().await;
            *current_id_guard = Some(conversation_id);
        }
        
        // Add system message if we have one (now safe to call without deadlock)
        if !self.system_prompt.is_empty() {
            let system_message = AgentMessage::system(&self.system_prompt);
            self.add_message_to_conversation(conversation_id, system_message).await?;
        }
        
        // Detect and set project context
        if let Ok(current_dir) = std::env::current_dir() {
            let project_type = ProjectType::from_project_name(&current_dir.to_string_lossy());
            let project_name = current_dir.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_string());
            let project_context = ProjectContext {
                name: project_name.unwrap_or_else(|| "Unknown Project".to_string()),
                project_type,
                root_path: Some(current_dir.clone()),
                description: None,
                repositories: Vec::new(),
                settings: HashMap::new(),
            };
            
            let manager = self.conversation_manager.lock().await;
            if let Ok(Some(mut conversation)) = manager.get_conversation(conversation_id).await {
                conversation.project_context = Some(project_context);
                let _ = manager.update_conversation(conversation).await;
            }
        }
        
        info!("Created new conversation: {conversation_id}");
        Ok(conversation_id)
    }
    
    /// Add a message to a specific conversation
    async fn add_message_to_conversation(&self, conversation_id: Uuid, message: AgentMessage) -> Result<(), SagittaCodeError> {
        let manager = self.conversation_manager.lock().await;
        
        if let Ok(Some(mut conversation)) = manager.get_conversation(conversation_id).await {
            conversation.add_message(message);
            manager.update_conversation(conversation).await
                .map_err(|e| SagittaCodeError::Unknown(format!("Failed to update conversation: {e}")))?;
        } else {
            return Err(SagittaCodeError::Unknown(format!("Conversation not found: {conversation_id}")));
        }
        
        Ok(())
    }
    
    /// Get the current conversation
    pub async fn get_current_conversation(&self) -> Result<Option<Conversation>, SagittaCodeError> {
        let current_id = {
            let current_id_guard = self.current_conversation_id.read().await;
            *current_id_guard
        };
        
        if let Some(id) = current_id {
            let manager = self.conversation_manager.lock().await;
            manager.get_conversation(id).await
                .map_err(|e| SagittaCodeError::Unknown(format!("Failed to get conversation: {e}")))
        } else {
            Ok(None)
        }
    }
    
    /// Switch to a different conversation
    pub async fn switch_conversation(&self, conversation_id: Uuid) -> Result<(), SagittaCodeError> {
        let manager = self.conversation_manager.lock().await;
        
        // Verify the conversation exists
        if let Ok(Some(_)) = manager.get_conversation(conversation_id).await {
            let mut current_id_guard = self.current_conversation_id.write().await;
            *current_id_guard = Some(conversation_id);
            info!("Switched to conversation: {conversation_id}");
            Ok(())
        } else {
            Err(SagittaCodeError::Unknown(format!("Conversation not found: {conversation_id}")))
        }
    }
    
    /// Get the conversation manager for advanced operations
    pub fn get_conversation_manager(&self) -> Arc<tokio::sync::Mutex<ConversationManagerImpl>> {
        self.conversation_manager.clone()
    }
}

// Implement the same interface as MessageHistoryManager
impl ConversationAwareHistoryManager {
    /// Add a message to the history
    pub async fn add_message(&self, message: AgentMessage) {
        if let Ok(conversation_id) = self.ensure_active_conversation().await {
            if let Err(e) = self.add_message_to_conversation(conversation_id, message).await {
                error!("Failed to add message to conversation: {e}");
            }
        } else {
            error!("Failed to ensure active conversation for adding message");
        }
    }
    
    /// Add a system message to the history
    pub async fn add_system_message(&self, content: impl Into<String>) {
        let message = AgentMessage::system(content);
        self.add_message(message).await;
    }
    
    /// Add a user message to the history
    pub async fn add_user_message(&self, content: impl Into<String>) {
        let content_str = content.into();
        debug!("ConversationAwareHistoryManager: Adding user message: '{content_str}'");
        let message = AgentMessage::user(content_str);
        self.add_message(message).await;
    }
    
    /// Add an assistant message to the history
    pub async fn add_assistant_message(&self, content: impl Into<String>) {
        let content_str = content.into();
        debug!("ConversationAwareHistoryManager: Adding assistant message: '{content_str}'");
        let message = AgentMessage::assistant(content_str);
        self.add_message(message).await;
    }
    
    /// Get a message by its ID
    pub async fn get_message(&self, id: Uuid) -> Option<AgentMessage> {
        if let Ok(Some(conversation)) = self.get_current_conversation().await {
            conversation.messages.iter().find(|m| m.id == id).cloned()
        } else {
            None
        }
    }
    
    /// Remove a message by its ID
    pub async fn remove_message(&self, id: Uuid) -> Option<AgentMessage> {
        if let Ok(conversation_id) = self.ensure_active_conversation().await {
            let manager = self.conversation_manager.lock().await;
            if let Ok(Some(mut conversation)) = manager.get_conversation(conversation_id).await {
                if let Some(pos) = conversation.messages.iter().position(|m| m.id == id) {
                    let removed = conversation.messages.remove(pos);
                    let _ = manager.update_conversation(conversation).await;
                    return Some(removed);
                }
            }
        }
        None
    }
    
    /// Get all messages in the history
    pub async fn get_messages(&self) -> Vec<AgentMessage> {
        if let Ok(Some(conversation)) = self.get_current_conversation().await {
            conversation.get_active_messages().clone()
        } else {
            Vec::new()
        }
    }
    
    /// Get the messages as LlmMessages for the LlmClient
    pub async fn to_llm_messages(&self) -> Vec<LlmMessage> {
        let messages = self.get_messages().await;
        let mut history = MessageHistory {
            messages,
            max_context_tokens: self.max_context_tokens,
            total_tokens: 0,
            token_counter: None,
        };
        
        // Calculate total tokens and apply truncation if needed
        history.recalculate_total_tokens();
        if history.max_context_tokens > 0 {
            history.truncate_to_token_limit(history.max_context_tokens);
        }
        
        history.to_llm_messages()
    }
    
    /// Add a tool call result to the most recent assistant message
    pub async fn add_tool_result(&self, tool_call_id: &str, result: serde_json::Value, successful: bool) -> Result<(), SagittaCodeError> {
        if let Ok(conversation_id) = self.ensure_active_conversation().await {
            let manager = self.conversation_manager.lock().await;
            if let Ok(Some(mut conversation)) = manager.get_conversation(conversation_id).await {
                // Find the most recent assistant message with a matching tool call
                for message in conversation.messages.iter_mut().rev() {
                    if message.role == Role::Assistant {
                        for tool_call in message.tool_calls.iter_mut() {
                            if tool_call.id == tool_call_id {
                                tool_call.result = Some(result.clone());
                                tool_call.successful = successful;
                                tool_call.execution_time = Some(chrono::Utc::now());
                                
                                // Update the conversation
                                manager.update_conversation(conversation).await
                                    .map_err(|e| SagittaCodeError::Unknown(format!("Failed to update conversation: {e}")))?;
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        
        Err(SagittaCodeError::Unknown(format!("No tool call found with ID {tool_call_id}")))
    }
    
    /// Clear all messages except system messages
    pub async fn clear_except_system(&self) {
        if let Ok(conversation_id) = self.ensure_active_conversation().await {
            let manager = self.conversation_manager.lock().await;
            if let Ok(Some(mut conversation)) = manager.get_conversation(conversation_id).await {
                conversation.messages.retain(|m| m.role == Role::System);
                let _ = manager.update_conversation(conversation).await;
            }
        }
    }
    
    /// Clear all messages
    pub async fn clear(&self) {
        if let Ok(conversation_id) = self.ensure_active_conversation().await {
            let manager = self.conversation_manager.lock().await;
            if let Ok(Some(mut conversation)) = manager.get_conversation(conversation_id).await {
                conversation.messages.clear();
                let _ = manager.update_conversation(conversation).await;
            }
        }
    }
    
    /// Get the number of messages in the history
    pub async fn len(&self) -> usize {
        self.get_messages().await.len()
    }
    
    /// Check if the history is empty
    pub async fn is_empty(&self) -> bool {
        self.get_messages().await.is_empty()
    }
}

