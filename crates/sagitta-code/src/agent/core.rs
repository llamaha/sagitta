// Agent orchestration logic will go here

use futures_util::Stream;
use serde_json::Value;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use log::{debug, info, trace};
use std::collections::HashMap;
use std::boxed::Box;

use crate::agent::message::history::ConversationAwareHistoryManager;
use crate::agent::message::types::{AgentMessage, ToolCall};
use crate::agent::state::manager::StateManager;
use crate::agent::state::types::{AgentState, AgentMode, AgentStateInfo};
use crate::agent::conversation::manager::{ConversationManager, ConversationManagerImpl};
use crate::agent::conversation::context_manager::ConversationContextManager;
use crate::agent::events::{AgentEvent, EventHandler};
use crate::agent::recovery::{RecoveryManager, RecoveryConfig, RecoveryState};
use crate::agent::streaming::StreamingProcessor;
use crate::config::types::SagittaCodeConfig;
use crate::llm::client::{LlmClient, StreamChunk as SagittaCodeStreamChunk, ThinkingConfig};
// Tool imports removed - tools now via MCP
use crate::utils::errors::SagittaCodeError;
// Import the EmbeddingProvider trait for generic use
use sagitta_embed::provider::EmbeddingProvider;

// Tokio stream wrappers

// Import agent's Role for mapping

// Import prompt providers
use crate::agent::prompts::{SystemPromptProvider, claude_code::ClaudeCodeSystemPrompt}; 

// Add imports for the traits
use crate::agent::conversation::persistence::ConversationPersistence;
use crate::agent::conversation::search::ConversationSearchEngine;



/// Core agent implementation that coordinates LLM calls and tool execution
#[derive(Clone)]
pub struct Agent {
    /// The LLM client
    llm_client: Arc<dyn LlmClient>,
    
    /// The tool registry (stub)
    tool_registry: Arc<crate::tools::registry::ToolRegistry>,
    
    /// The message history manager
    history: Arc<ConversationAwareHistoryManager>,
    
    /// The state manager
    state_manager: Arc<StateManager>,
    
    /// The tool executor (stub)
    tool_executor: Arc<tokio::sync::Mutex<crate::tools::executor::SagittaCodeToolExecutorInternal>>,
    
    /// Sender for agent events
    event_sender: broadcast::Sender<AgentEvent>,
    
    /// The configuration
    config: SagittaCodeConfig,
    state: Arc<tokio::sync::Mutex<AgentState>>,
    
    /// Pending tool calls waiting for human approval
    pending_tool_calls: Arc<tokio::sync::Mutex<HashMap<String, (ToolCall, u32, AgentMessage)>>>, // Review if new engine handles this
    
    /// Flag to request breaking out of reasoning loops
    loop_break_requested: Arc<tokio::sync::Mutex<bool>>, // New engine might have its own or use this
    
    /// Event handler for agent events
    event_handler: EventHandler,
    
    /// Recovery manager
    recovery_manager: Arc<RecoveryManager>,
    
    
    /// Phase 3: Conversation context manager for intelligent flow management
    context_manager: Arc<ConversationContextManager>,
    
    /// Streaming processor for handling LLM streaming responses
    streaming_processor: Arc<StreamingProcessor>,
    
    /// Cancellation token for interrupting operations
    cancellation_token: Arc<tokio::sync::Mutex<CancellationToken>>,
}

impl Agent {
    /// Create a new agent with the provided configuration
    pub async fn new(
        config: SagittaCodeConfig,
        tool_registry: Option<Arc<crate::tools::registry::ToolRegistry>>,
        _embedding_provider: Arc<dyn EmbeddingProvider + Send + Sync + 'static>,
        persistence: Box<dyn ConversationPersistence>,
        search_engine: Box<dyn ConversationSearchEngine>,
        llm_client: Arc<dyn LlmClient>,
    ) -> Result<Self, SagittaCodeError> {
        info!("Creating new agent...");
        info!("Using provided LLM client.");
        
        let (event_sender, _event_receiver) = broadcast::channel(100);

        // let agent_state_initial = AgentState::Idle; // Not directly used for Arc::new later
        
        debug!("Initializing StateManager.");
        let state_manager_instance = StateManager::new(); // Not Arc wrapped here

        // Create stub tool registry if not provided
        let tool_registry = tool_registry.unwrap_or_else(|| Arc::new(crate::tools::registry::ToolRegistry::new()));
        
        let (tool_executor_internal, tool_event_receiver_from_executor) = crate::tools::executor::SagittaCodeToolExecutorInternal::new(
            tool_registry.clone(), 
            Arc::new(state_manager_instance.clone())
        );
        debug!("Sagitta-code internal ToolExecutor created.");

        // Fetch tool definitions for system prompt (will be empty for stub)
        let tool_definitions_for_prompt = tool_registry.get_definitions().await;
        debug!("Fetched {} tool definitions for system prompt.", tool_definitions_for_prompt.len());
        
        // Always use Claude Code prompt provider
        let system_prompt = {
            info!("Using Claude Code prompt provider");
            let provider = ClaudeCodeSystemPrompt;
            provider.generate_system_prompt(&tool_definitions_for_prompt)
        };
        info!("System prompt constructed. Length: {}", system_prompt.len());
        trace!("System prompt content: {system_prompt}");

        // Set up conversation management system
        debug!("Setting up conversation management system...");
        
        // Determine storage path
        let _storage_path = if let Some(path) = &config.conversation.storage_path {
            path.clone()
        } else {
            // Use default path in user's config directory
            let mut default_path = dirs::config_dir()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
            default_path.push("sagitta-code");
            default_path.push("conversations");
            default_path
        };
        
        // Create conversation manager using the provided persistence and search_engine
        let conversation_manager = ConversationManagerImpl::new(
            persistence, // Use passed-in persistence
            search_engine, // Use passed-in search_engine
        ).await
        .map_err(|e| SagittaCodeError::Unknown(format!("Failed to create conversation manager: {e}")))?
        .with_auto_save(config.conversation.auto_save);
        debug!("Conversation manager created.");
        
        // Create conversation-aware history manager
        // Use Claude model's context window (all models have 200k tokens)
        let max_context_tokens = 200000;
        let history_manager = ConversationAwareHistoryManager::with_system_prompt(
            conversation_manager,
            config.conversation.clone(),
            None, // TODO: Detect workspace ID from project context
            max_context_tokens,
            system_prompt.clone(),
        ).await
        .map_err(|e| SagittaCodeError::Unknown(format!("Failed to create conversation-aware history manager: {e}")))?;
        debug!("Conversation-aware history manager created.");
        
        // Get current conversation ID for context manager
        let current_conversation_id = if let Ok(Some(conversation)) = history_manager.get_current_conversation().await {
            conversation.id
        } else {
            // Create a new conversation and get its ID
            let new_id = Uuid::new_v4();
            info!("No current conversation found, will use new ID for context manager: {new_id}");
            new_id
        };
        
        // Create conversation context manager for Phase 3 features
        let context_manager = Arc::new(ConversationContextManager::new(current_conversation_id));
        debug!("Conversation context manager created for conversation: {current_conversation_id}");

        // Create shared loop break flag
        let loop_break_requested_initial = Arc::new(tokio::sync::Mutex::new(false));
        
        // Create streaming processor
        let continue_reasoning_flag = Arc::new(Mutex::new(false));
        let streaming_processor = Arc::new(StreamingProcessor::new(
            llm_client.clone(),
            tool_registry.clone(),
            Arc::new(history_manager.clone()),
            Arc::new(state_manager_instance.clone()),
            Arc::new(tokio::sync::Mutex::new(tool_executor_internal.clone())),
            event_sender.clone(),
            continue_reasoning_flag,
        ));

        let agent_self = Self {
            llm_client,
            tool_registry,
            history: Arc::new(history_manager),
            state_manager: Arc::new(state_manager_instance.clone()), // Now Arc wrapping the instance
            tool_executor: Arc::new(tokio::sync::Mutex::new(tool_executor_internal)),
            event_sender: event_sender.clone(),
            config,
            state: Arc::new(tokio::sync::Mutex::new(AgentState::Idle)),
            pending_tool_calls: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            loop_break_requested: loop_break_requested_initial.clone(),
            event_handler: EventHandler::new(event_sender.clone()),
            recovery_manager: Arc::new(RecoveryManager::new(RecoveryConfig::default(), Arc::new(state_manager_instance), event_sender.clone())),
            context_manager,
            streaming_processor,
            cancellation_token: Arc::new(tokio::sync::Mutex::new(CancellationToken::new())),
        };
        
        // Start event listeners
        agent_self.event_handler.start_state_event_listener(agent_self.state_manager.clone());
        debug!("State event listener started for Agent state.");

        agent_self.event_handler.start_tool_event_listener(tool_event_receiver_from_executor, agent_self.history.clone());
        debug!("Tool event listener started for SagittaCodeToolExecutorInternal events.");

        info!("Agent created successfully.");
        Ok(agent_self)
    }
    
    /// Subscribe to agent events
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.event_handler.subscribe()
    }
    
    /// Get current recovery state
    pub fn get_recovery_state(&self) -> RecoveryState {
        self.recovery_manager.get_recovery_state()
    }
    
    /// Reset recovery state
    pub fn reset_recovery_state(&self) {
        self.recovery_manager.reset_recovery_state()
    }
    
    /// Set recovery configuration
    pub fn with_recovery_config(self, _config: RecoveryConfig) -> Self {
        // Note: This creates a new recovery manager with the new config
        // In practice, you might want to update the existing one
        self
    }
    
    /// Process a user message with streaming
    /// This now calls the more general `process_message_stream_with_thinking_fixed`
    pub async fn process_message_stream(&self, message: impl Into<String>) 
        -> Result<Pin<Box<dyn Stream<Item = Result<SagittaCodeStreamChunk, SagittaCodeError>> + Send + '_>>, SagittaCodeError> 
    {
        self.process_message_stream_with_thinking_fixed(message, None).await
    }
    
    /// Process a user message with streaming and thinking mode enabled - MAIN ENTRY POINT
    pub async fn process_message_stream_with_thinking_fixed(&self, message: impl Into<String>, _thinking_config: Option<ThinkingConfig>) 
        -> Result<Pin<Box<dyn Stream<Item = Result<SagittaCodeStreamChunk, SagittaCodeError>> + Send + '_>>, SagittaCodeError> 
    {
        let message_text = message.into();
        info!("Processing user message (stream with thinking - FIXED): '{message_text}'");
        
        // Reset cancellation token for new operation
        self.reset_cancellation().await;
        
        self.state_manager.set_thinking("Processing user message with thinking").await?;
        
        // Get current conversation ID for analytics reporting
        let _current_conversation_id = self.history.get_current_conversation().await.ok().flatten().map(|c| c.id);
        
        // Use direct LLM streaming
        info!("Using direct LLM streaming");
        
        // Use the streaming processor which properly emits events
        let stream = self.streaming_processor.process_message_stream(message_text).await?;
        
        // StreamChunk already has the correct structure, just return it
        Ok(Box::pin(stream))
    }
    
    /// Execute a tool by name with parameters (with recovery)
    pub async fn execute_tool(&self, tool_name: &str, parameters: Value) -> Result<crate::agent::events::ToolResult, SagittaCodeError> {
        self.tool_executor.lock().await.execute_tool(tool_name, parameters).await
    }
    
    /// Get the current conversation history
    pub async fn get_history(&self) -> Vec<AgentMessage> {
        self.history.get_messages().await
    }
    
    /// Get the current agent state
    pub async fn get_state(&self) -> AgentState {
        self.state_manager.get_agent_state().await
    }
    
    /// Set the agent mode
    pub async fn set_mode(&self, mode: AgentMode) -> Result<(), SagittaCodeError> {
        self.state_manager.set_agent_mode(mode).await
    }
    
    /// Get the current agent mode
    pub async fn get_mode(&self) -> AgentMode {
        self.state_manager.get_agent_mode().await
    }
    
    
    /// Clear the conversation history (except system prompts)
    pub async fn clear_history(&self) -> Result<(), SagittaCodeError> {
        self.history.clear().await;
        Ok(())
    }
    
    /// Request breaking out of the current processing loop
    pub async fn request_loop_break(&self) {
        let mut break_flag = self.loop_break_requested.lock().await;
        *break_flag = true;
        log::info!("Loop break requested via Agent flag");
    }
    
    /// Check if a loop break has been requested (checks Agent's own flag)
    pub async fn is_loop_break_requested(&self) -> bool {
        let break_flag = self.loop_break_requested.lock().await;
        *break_flag
    }
    
    /// Clear the loop break request (clears Agent's own flag)
    pub async fn clear_loop_break_request(&self) {
        let mut break_flag = self.loop_break_requested.lock().await;
        *break_flag = false;
    }
    
    /// Cancel any ongoing operations
    pub async fn cancel(&self) {
        log::info!("Agent cancel requested");
        let token = self.cancellation_token.lock().await;
        token.cancel();
        
        // Cancel the LLM client (works for any client that implements cancel)
        log::info!("Calling LLM client cancel method");
        self.llm_client.cancel().await;
        log::info!("LLM client cancel method completed");
        
        // Also request loop break for consistency
        self.request_loop_break().await;
        
        // Send cancellation event
        let _ = self.event_sender.send(AgentEvent::Cancelled);
    }
    
    /// Check if cancellation has been requested
    pub async fn is_cancelled(&self) -> bool {
        let token = self.cancellation_token.lock().await;
        token.is_cancelled()
    }
    
    /// Reset cancellation token for new operations
    async fn reset_cancellation(&self) {
        let mut token_guard = self.cancellation_token.lock().await;
        if token_guard.is_cancelled() {
            // Create a new token since the old one can't be reset
            *token_guard = CancellationToken::new();
            log::debug!("Cancellation token reset");
        }
    }
    
    /// Get a child cancellation token for sub-operations
    pub async fn get_cancellation_token(&self) -> CancellationToken {
        let token = self.cancellation_token.lock().await;
        token.child_token()
    }
    
    /// Register a tool with the agent (stub - tools now come from MCP)
    pub async fn register_tool(&self, _tool: Arc<dyn std::any::Any + Send + Sync>) -> Result<(), SagittaCodeError> {
        // Tools are now provided via MCP, this is a no-op
        Ok(())
    }
    
    /// Get the conversation manager for advanced conversation operations
    pub fn get_conversation_manager(&self) -> Arc<tokio::sync::Mutex<ConversationManagerImpl>> {
        self.history.get_conversation_manager()
    }
    
    /// Get the current conversation
    pub async fn get_current_conversation(&self) -> Result<Option<crate::agent::conversation::types::Conversation>, SagittaCodeError> {
        self.history.get_current_conversation().await
    }
    
    /// Switch to a different conversation
    pub async fn switch_conversation(&self, conversation_id: uuid::Uuid) -> Result<(), SagittaCodeError> {
        self.history.switch_conversation(conversation_id).await
    }
    
    /// Create a new conversation
    pub async fn create_new_conversation(&self, title: String) -> Result<uuid::Uuid, SagittaCodeError> {
        let manager = self.get_conversation_manager();
        let manager_guard = manager.lock().await;
        manager_guard.create_conversation(title, None).await
            .map_err(|e| SagittaCodeError::Unknown(format!("Failed to create conversation: {e}")))
    }
    
    /// List all conversations
    pub async fn list_conversations(&self) -> Result<Vec<crate::agent::conversation::types::ConversationSummary>, SagittaCodeError> {
        let manager = self.get_conversation_manager();
        let manager_guard = manager.lock().await;
        manager_guard.list_conversations(None).await
            .map_err(|e| SagittaCodeError::Unknown(format!("Failed to list conversations: {e}")))
    }

    /// Add a tool result to the conversation history
    /// This is needed for OpenAI-compatible providers that execute tools externally
    pub async fn add_tool_result_to_history(&self, tool_call_id: &str, tool_name: &str, result: &serde_json::Value) -> Result<(), SagittaCodeError> {
        // Find the assistant message with this tool call and update it
        let messages = self.history.get_messages().await;
        for msg in messages.iter().rev() {
            if msg.role == crate::llm::client::Role::Assistant {
                for tool_call in &msg.tool_calls {
                    if tool_call.id == tool_call_id {
                        // Update the existing tool call with the result
                        let success = self.history.add_tool_result(tool_call_id, result.clone(), !result.get("error").is_some()).await?;
                        debug!("Updated tool call {} with result in assistant message", tool_call_id);
                        return Ok(());
                    }
                }
            }
        }
        
        // If we couldn't find the tool call in an assistant message, log an error
        log::error!("Could not find tool call {} in any assistant message", tool_call_id);
        Err(SagittaCodeError::Unknown(format!("Tool call {} not found in conversation history", tool_call_id)))
    }

    // Added getter for StateManager's state Arc
    pub fn get_state_manager_state_info_arc(&self) -> Arc<tokio::sync::RwLock<AgentStateInfo>> {
        self.state_manager.get_state_arc()
    }
}

// Include tests from separate module
#[cfg(test)]
mod tests;

