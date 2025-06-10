// Agent orchestration logic will go here

use futures_util::{Stream, StreamExt};
use serde_json::Value;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, broadcast};
use uuid::Uuid;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use log::{debug, info, warn, error, trace};
use chrono;
use std::time::Duration;
use std::collections::HashMap;
use std::boxed::Box;

use crate::agent::message::history::{MessageHistoryManager, ConversationAwareHistoryManager};
use crate::agent::message::types::{AgentMessage, ToolCall};
use crate::agent::message::history::MessageHistory;
use crate::agent::state::manager::{StateManager, StateEvent};
use crate::agent::state::types::{AgentState, AgentMode, ConversationStatus, AgentStateInfo};
use crate::agent::conversation::manager::{ConversationManager, ConversationManagerImpl};
use crate::agent::conversation::persistence::disk::DiskConversationPersistence;
use crate::agent::conversation::search::text::TextConversationSearchEngine;
use crate::agent::conversation::context_manager::ConversationContextManager;
use crate::agent::events::{AgentEvent, EventHandler};
use crate::agent::recovery::{RecoveryManager, RecoveryConfig, RecoveryState};
use crate::config::types::SagittaCodeConfig;
use crate::llm::client::{LlmClient, LlmResponse, Message, Role, StreamChunk as SagittaCodeStreamChunk, MessagePart as SagittaCodeMessagePart, ToolDefinition as LlmToolDefinition, ThinkingConfig};
use crate::llm::gemini::client::GeminiClient;
use crate::tools::executor::{ToolExecutor as SagittaCodeToolExecutorInternal, ToolExecutionEvent};
use crate::tools::registry::ToolRegistry;
use crate::tools::types::{ToolResult, ToolDefinition as ToolDefinitionType};
use crate::utils::errors::SagittaCodeError;
use crate::tools::code_search::tool::CodeSearchTool;
use crate::reasoning::{
    AgentToolExecutor,
    AgentEventEmitter,
    AgentStatePersistence,
    AgentMetricsCollector,
    config::create_reasoning_config,
    SagittaCodeIntentAnalyzer,
    llm_adapter::ReasoningLlmClientAdapter,
};
use reasoning_engine::ReasoningEngine;
use reasoning_engine::ReasoningError;
use reasoning_engine::ReasoningConfig;
use reasoning_engine::ReasoningState;

// --- New Imports for reasoning-engine integration ---
use reasoning_engine::traits::{
    ToolExecutor as ReasoningToolExecutorTrait,
    EventEmitter as ReasoningEventEmitterTrait,
    StreamHandler as ReasoningStreamHandlerTrait,
    StatePersistence as ReasoningStatePersistenceTrait,
    MetricsCollector as ReasoningMetricsCollectorTrait,
    LlmMessage, // Direct import
    LlmMessagePart, // Direct import
    ToolCall as ReasoningToolCallData, // Alias for clarity
    IntentAnalyzer as ReasoningIntentAnalyzerTrait, // For type alias if needed, or use directly
};
use reasoning_engine::streaming::StreamChunk as ReasoningEngineStreamingChunk; // The one from reasoning_engine/src/streaming.rs
// Import the EmbeddingProvider trait for generic use
use sagitta_embed::provider::EmbeddingProvider;
// --- End New Imports ---

// Tokio stream wrappers
use tokio_stream::wrappers::UnboundedReceiverStream;

// Import agent's Role for mapping
use crate::llm::client::Role as LlmClientRole; 

// Add imports for the traits
use crate::agent::conversation::persistence::ConversationPersistence;
use crate::agent::conversation::search::ConversationSearchEngine;

use terminal_stream::events::StreamEvent;

/// The system prompt instructing the agent how to respond
const DEFAULT_SYSTEM_PROMPT: &str = r#"You are Sagitta Code AI, powered by Gemini and sagitta-search.
You help developers understand and work with code repositories efficiently.
You have access to tools that can search and retrieve code, view file contents, and more.
When asked about code, use your tools to look up accurate and specific information.

CRITICAL INSTRUCTIONS FOR STEP-BY-STEP COMMUNICATION:
- ALWAYS start your response by acknowledging the user's request and providing a clear, numbered plan
- NEVER execute tools immediately - first explain what you will do
- After providing your plan, then proceed with tool execution
- Before executing any tool, explain what you're about to do and why
- After each tool execution, briefly explain what you found and what you'll do next
- Provide running commentary throughout multi-step processes
- Only provide a final summary after completing ALL steps

MANDATORY RESPONSE STRUCTURE:
1. First, acknowledge the request: "I'll help you with that!"
2. Then provide a numbered plan: "Here's my plan: 1) First I'll..., 2) Then I'll..., 3) Finally I'll..."
3. Then say: "Let me start by..." and execute the first tool
4. Continue with explanatory text between each tool execution
5. End with a comprehensive summary

EXAMPLE COMMUNICATION PATTERN:
User: "Search for X repository, add it, sync it, then query it"
You should respond like:
"I'll help you with that! Here's my plan:
1. First, I'll search the web for the X repository
2. Then I'll add it to our system with an appropriate name
3. Next, I'll sync it to get the latest code
4. Finally, I'll search for relevant examples in the codebase

Let me start by searching for the repository..."
[Execute web search tool]
"Great! I found the X repository at [URL]. Now I'll add it to our system..."
[Execute repo add tool]
"Repository added successfully! Now I'll sync it to get the latest code..."
[Execute sync tool]
"Sync completed! Now let me search for examples in the codebase..."
[Execute search tool]
"Here's what I found: [comprehensive summary with all results]"

CRITICAL INSTRUCTIONS FOR MULTI-TOOL REASONING:
- When a user requests a multi-step task, you MUST complete ALL steps in sequence
- Do NOT stop after completing just one tool call - continue reasoning until the ENTIRE user request is fulfilled
- Each tool call should logically lead to the next step in the sequence
- Only stop when you have fully completed the user's request and provided a comprehensive answer
- If you're unsure whether to continue, err on the side of continuing the reasoning chain

EXAMPLES OF MULTI-STEP TASKS:
- "Search for X repository, add it, sync it, then query it" = 4 tool calls minimum
- "Find information about X, then add the repository and search its code" = 3+ tool calls minimum
- "Look up X online, then add it to our system and analyze it" = 3+ tool calls minimum

CRITICAL INSTRUCTIONS FOR WRAP-UP AND FOLLOW-UP:
- ALWAYS end your responses with a clear wrap-up of what you accomplished
- If you believe the task is finished, write a concise summary of what you accomplished and your final answer
- If you have concrete ideas for next steps, briefly summarize your progress and ask the user whether they would like you to continue with those specific steps
- Do NOT ask open-ended 'how can I help' questions; propose specific next actions instead
- Examples of good follow-up questions: "Would you like me to analyze the error handling patterns in this codebase?" or "Should I search for similar implementations in other repositories?"

Remember: Your goal is to be thorough, communicative, and complete the user's full request while keeping them informed of your progress at each step. ALWAYS start with acknowledgment and a plan before executing any tools, and ALWAYS end with a clear wrap-up."#;

/// Helper function to convert AgentMessages to ReasoningEngine's LlmMessages
fn convert_agent_messages_to_reasoning_llm_messages(
    agent_messages: Vec<AgentMessage>,
) -> Vec<LlmMessage> {
    agent_messages
        .into_iter()
        .filter_map(|agent_msg| {
            let role_str = match agent_msg.role { // Use agent_msg.role
                LlmClientRole::User => "user".to_string(),
                LlmClientRole::Assistant => "assistant".to_string(),
                LlmClientRole::System => "system".to_string(), 
                LlmClientRole::Function => "assistant".to_string(), // Map Function role to assistant, as it often carries tool calls/results from assistant
            };

            let mut parts: Vec<LlmMessagePart> = Vec::new();

            // Add main content if not empty
            if !agent_msg.content.is_empty() {
                parts.push(LlmMessagePart::Text(agent_msg.content.clone()));
            }

            // Process tool_calls associated with this AgentMessage
            for tc_entry in agent_msg.tool_calls {
                if tc_entry.result.is_none() { // This is a tool call *request* by the assistant
                    if agent_msg.role == LlmClientRole::Assistant { // Only assistants should have ToolCall requests in this model
                        parts.push(LlmMessagePart::ToolCall(ReasoningToolCallData {
                            name: tc_entry.name,
                            args: tc_entry.arguments, // arguments is the field name
                        }));
                    } else {
                        // Log or handle user/system message having a tool call request if that's unexpected
                        warn!("User/System message found with a tool call request: {:?}", tc_entry);
                    }
                } else { // This is a tool_call *with a result*, presented by the assistant
                    let result_str = format!(
                        "Tool '{}' executed. Success: {}. Result: {}",
                        tc_entry.name,
                        tc_entry.successful,
                        serde_json::to_string(&tc_entry.result) // tc_entry.result is Option<Value>
                            .unwrap_or_else(|e| format!("(Error serializing result: {})", e))
                    );
                    parts.push(LlmMessagePart::Text(result_str));
                }
            }
            
            if parts.is_empty() {
                None 
            } else {
                Some(LlmMessage { role: role_str, parts })
            }
        })
        .collect()
}

/// Core agent implementation that coordinates LLM calls and tool execution
#[derive(Clone)]
pub struct Agent {
    /// The LLM client
    llm_client: Arc<dyn LlmClient>,
    
    /// The tool registry
    tool_registry: Arc<ToolRegistry>,
    
    /// The message history manager
    history: Arc<ConversationAwareHistoryManager>,
    
    /// The state manager
    state_manager: Arc<StateManager>,
    
    /// The tool executor (from sagitta-code's own tools module)
    tool_executor: Arc<tokio::sync::Mutex<SagittaCodeToolExecutorInternal>>,
    
    /// Sender for agent events
    event_sender: broadcast::Sender<AgentEvent>,
    
    /// The configuration
    config: SagittaCodeConfig,
    state: Arc<tokio::sync::Mutex<AgentState>>, // General agent state, distinct from reasoning engine state
    
    /// Pending tool calls waiting for human approval
    pending_tool_calls: Arc<tokio::sync::Mutex<HashMap<String, (ToolCall, u32, AgentMessage)>>>, // Review if new engine handles this
    
    /// Flag to request breaking out of reasoning loops
    loop_break_requested: Arc<tokio::sync::Mutex<bool>>, // New engine might have its own or use this
    
    /// NEW: The new reasoning engine, wrapped in Arc<Mutex<>> for shared mutable access
    new_reasoning_engine: Arc<tokio::sync::Mutex<ReasoningEngine<ReasoningLlmClientAdapter, SagittaCodeIntentAnalyzer>>>,

    /// Event handler for agent events
    event_handler: EventHandler,
    
    /// Recovery manager
    recovery_manager: Arc<RecoveryManager>,

    /// NEW: Reasoning state cache for session continuity
    reasoning_state_cache: Arc<tokio::sync::Mutex<HashMap<Uuid, ReasoningState>>>,
    
    /// Terminal event sender for streaming shell execution
    terminal_event_sender: Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::Sender<StreamEvent>>>>,
    
    /// Phase 3: Conversation context manager for intelligent flow management
    context_manager: Arc<ConversationContextManager>,
}

impl Agent {
    /// Create a new agent with the provided configuration
    pub async fn new(
        config: SagittaCodeConfig,
        tool_registry: Arc<ToolRegistry>,
        embedding_provider: Arc<dyn EmbeddingProvider + Send + Sync + 'static>,
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

        let (tool_executor_internal, tool_event_receiver_from_executor) = SagittaCodeToolExecutorInternal::new(
            tool_registry.clone(), 
            Arc::new(state_manager_instance.clone()) // SagittaCodeToolExecutorInternal might expect Arc
        );
        debug!("Sagitta-code internal ToolExecutor created.");

        // Construct the dynamic system prompt
        let tool_definitions_for_prompt = tool_registry.get_definitions().await;
        debug!("Fetched {} tool definitions for system prompt.", tool_definitions_for_prompt.len());
        let mut tool_prompt_parts = Vec::new();
        if !tool_definitions_for_prompt.is_empty() {
            tool_prompt_parts.push("\n\nYou have the following tools available:".to_string());
            for tool_def in tool_definitions_for_prompt {
                let params_json = match serde_json::to_string_pretty(&tool_def.parameters) {
                    Ok(json) => json,
                    Err(_) => "Error serializing parameters".to_string(),
                };
                tool_prompt_parts.push(format!(
                    "\nTool: {}\nDescription: {}\nParameters Schema:\n{}",
                    tool_def.name,
                    tool_def.description,
                    params_json
                ));
            }
        }
        let system_prompt = format!("{}{}", DEFAULT_SYSTEM_PROMPT, tool_prompt_parts.join(""));
        info!("System prompt constructed. Length: {}", system_prompt.len());
        trace!("System prompt content: {}", system_prompt);

        // Set up conversation management system
        debug!("Setting up conversation management system...");
        
        // Determine storage path
        let storage_path = if let Some(path) = &config.conversation.storage_path {
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
        .map_err(|e| SagittaCodeError::Unknown(format!("Failed to create conversation manager: {}", e)))?
        .with_auto_save(config.conversation.auto_save);
        debug!("Conversation manager created.");
        
        // Create conversation-aware history manager
        let history_manager = ConversationAwareHistoryManager::with_system_prompt(
            conversation_manager,
            config.conversation.clone(),
            None, // TODO: Detect workspace ID from project context
            system_prompt.clone(),
        ).await
        .map_err(|e| SagittaCodeError::Unknown(format!("Failed to create conversation-aware history manager: {}", e)))?;
        debug!("Conversation-aware history manager created.");
        
        // Get current conversation ID for context manager
        let current_conversation_id = if let Ok(Some(conversation)) = history_manager.get_current_conversation().await {
            conversation.id
        } else {
            // Create a new conversation and get its ID
            let new_id = Uuid::new_v4();
            info!("No current conversation found, will use new ID for context manager: {}", new_id);
            new_id
        };
        
        // Create conversation context manager for Phase 3 features
        let context_manager = Arc::new(ConversationContextManager::new(current_conversation_id));
        debug!("Conversation context manager created for conversation: {}", current_conversation_id);

        // Create shared loop break flag
        let loop_break_requested_initial = Arc::new(tokio::sync::Mutex::new(false));

        // 0. Create Intent Analyzer (using the provided embedding_provider)
        let intent_analyzer_impl = Arc::new(SagittaCodeIntentAnalyzer::new(embedding_provider.clone()));

        // 1. Create the llm_client adapter FIRST
        let llm_adapter_for_re = Arc::new(ReasoningLlmClientAdapter::new(llm_client.clone(), tool_registry.clone()));

        // 2. Prepare reasoning_config
        let reasoning_config_data = create_reasoning_config(&config);
        info!("ReasoningEngine config data created: {:?}", reasoning_config_data);

        // 3. Attempt to create the ReasoningEngine
        let reasoning_engine_instance_result =
            ReasoningEngine::<ReasoningLlmClientAdapter, SagittaCodeIntentAnalyzer>::new(
                reasoning_config_data.clone(), 
                llm_adapter_for_re.clone(), 
                intent_analyzer_impl.clone()
            ).await;

        let new_reasoning_engine_arc: Arc<tokio::sync::Mutex<ReasoningEngine<ReasoningLlmClientAdapter, SagittaCodeIntentAnalyzer>>> = match reasoning_engine_instance_result {
            Ok(engine) => {
                info!("New ReasoningEngine instance created successfully.");
                Arc::new(tokio::sync::Mutex::new(engine))
            }
            Err(e) => {
                error!("Failed to create ReasoningEngine: {}. Using a fallback.", e);
                 let fallback_llm_adapter = Arc::new(ReasoningLlmClientAdapter::new(llm_client.clone(), tool_registry.clone()));
                 Arc::new(tokio::sync::Mutex::new(
                     ReasoningEngine::<ReasoningLlmClientAdapter, SagittaCodeIntentAnalyzer>::new(
                        reasoning_config_data, 
                        fallback_llm_adapter, 
                        intent_analyzer_impl
                    ).await 
                     .expect("Fallback ReasoningEngine creation failed catastrophically")
                 ))
            }
        };

        let mut agent_self = Self {
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
            new_reasoning_engine: new_reasoning_engine_arc,
            event_handler: EventHandler::new(event_sender.clone()),
            recovery_manager: Arc::new(RecoveryManager::new(RecoveryConfig::default(), Arc::new(state_manager_instance), event_sender.clone())),
            reasoning_state_cache: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            terminal_event_sender: Arc::new(tokio::sync::Mutex::new(None)),
            context_manager,
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
    pub fn with_recovery_config(self, config: RecoveryConfig) -> Self {
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
        info!("Processing user message (stream with thinking - FIXED): '{}'", message_text);
        
        self.state_manager.set_thinking("Processing user message with thinking").await?;
        
        self.history.add_user_message(&message_text).await;
        debug!("Added current user message to history manager.");

        // Fetch the complete conversation history from ConversationAwareHistoryManager
        let mut agent_conversation_history: Vec<AgentMessage> = self.history.get_messages().await; // Corrected method name
        debug!("Fetched {} messages from history manager for LLM.", agent_conversation_history.len());
        
        // CRITICAL FIX: Ensure we always have at least the current user message in the history
        if agent_conversation_history.is_empty() || !agent_conversation_history.iter().any(|msg| {
            msg.role == LlmClientRole::User && msg.content.trim() == message_text.trim()
        }) {
            warn!("History is missing current user message, adding it manually");
            let user_message = AgentMessage::user(message_text.clone());
            agent_conversation_history.push(user_message);
        }
        
        // Get current conversation ID for analytics reporting
        let current_conversation_id = self.history.get_current_conversation().await.ok().flatten().map(|c| c.id);
        
        let reasoning_llm_history = convert_agent_messages_to_reasoning_llm_messages(agent_conversation_history);
        
        // CRITICAL FIX: Always ensure we have at least one message for the reasoning engine
        let final_reasoning_history = if reasoning_llm_history.is_empty() {
            warn!("Converted reasoning_llm_history is empty, creating minimal history with current user message");
            vec![LlmMessage {
                role: "user".to_string(),
                parts: vec![LlmMessagePart::Text(message_text.clone())],
            }]
        } else {
            reasoning_llm_history
        };
        
        debug!("Final reasoning history contains {} LlmMessages for ReasoningEngine.", final_reasoning_history.len());

        let (tx, rx) = mpsc::unbounded_channel::<Result<SagittaCodeStreamChunk, SagittaCodeError>>();

        let mut tool_executor_adapter = AgentToolExecutor::new(self.tool_registry.clone());
        
        // Configure terminal event sender if available
        if let Some(terminal_sender) = self.terminal_event_sender.lock().await.clone() {
            tool_executor_adapter.set_terminal_event_sender(terminal_sender);
            log::debug!("Configured AgentToolExecutor with terminal event sender for streaming shell execution");
        }
        
        // Phase 1 Fix: Configure event sender for LLM feedback
        tool_executor_adapter.set_event_sender(self.event_sender.clone());
        log::debug!("Configured AgentToolExecutor with event sender for LLM feedback");
        
        let tool_executor_adapter = Arc::new(tool_executor_adapter);
        let event_emitter_adapter = Arc::new(AgentEventEmitter::new(self.event_sender.clone()));
        let stream_handler_adapter = Arc::new(AgentStreamHandler::new(tx.clone(), self.event_sender.clone(), current_conversation_id, self.history.clone()));
        
        let engine_arc_clone: Arc<tokio::sync::Mutex<ReasoningEngine<ReasoningLlmClientAdapter, SagittaCodeIntentAnalyzer>>> = Arc::clone(&self.new_reasoning_engine);
        let event_sender_clone = self.event_sender.clone();
        let reasoning_state_cache_clone = Arc::clone(&self.reasoning_state_cache);

        tokio::spawn(async move {
            debug!("Starting reasoning engine task...");
            
            let mut engine_guard = engine_arc_clone.lock().await;
            
            debug!("Calling ReasoningEngine::process_with_context with {} history messages.", final_reasoning_history.len());
            
            // NEW: Get previous reasoning state for session continuity
            let previous_state = if let Some(conv_id) = current_conversation_id {
                reasoning_state_cache_clone.lock().await.get(&conv_id).cloned()
            } else {
                None
            };
            
            debug!("About to call ReasoningEngine::process_with_context");
            
            let reasoning_result = engine_guard.process_with_context(
                final_reasoning_history, 
                tool_executor_adapter,
                event_emitter_adapter,
                stream_handler_adapter,
                previous_state.as_ref(),
                current_conversation_id,
            ).await;

            debug!("ReasoningEngine::process_with_context completed");

            // More detailed logging for the result of process_with_context
            match &reasoning_result { // Use a reference here to log before consuming
                Ok(final_state) => {
                    info!("ReasoningEngine::process_with_context returned Ok. Session ID: {}. Success: {}. Completion Reason: {:?}", 
                          final_state.session_id, final_state.is_successful(), final_state.completion_reason);
                    // NEW: Cache the reasoning state for future continuity
                    if let Some(conv_id) = current_conversation_id {
                        reasoning_state_cache_clone.lock().await.insert(conv_id, final_state.clone()); // Clone final_state here for caching
                    }
                }
                Err(e) => {
                    error!("ReasoningEngine::process_with_context returned Err: {}", e);
                    if tx.send(Err(SagittaCodeError::ReasoningError(e.to_string()))).is_err() {
                        warn!("Reasoning process failed (Err returned), but output stream receiver is already gone.");
                    }
                    let _ = event_sender_clone.send(AgentEvent::Error(format!("Reasoning process failed: {}", e)));
                }
            }

            debug!("Reasoning engine task completed");
        });

        Ok(Box::pin(UnboundedReceiverStream::new(rx)))
    }
    
    /// Execute a tool by name with parameters (with recovery)
    pub async fn execute_tool(&self, tool_name: &str, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
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
    
    /// Set the terminal event sender for streaming shell execution
    pub async fn set_terminal_event_sender(&self, sender: tokio::sync::mpsc::Sender<StreamEvent>) {
        // Set the terminal event sender on the internal tool executor
        self.tool_executor.lock().await.set_terminal_event_sender(sender.clone());
        log::info!("Terminal event sender set on agent's tool executor");
        
        // Store the sender for use in future reasoning sessions
        {
            let mut terminal_sender_guard = self.terminal_event_sender.lock().await;
            *terminal_sender_guard = Some(sender);
        }
        
        log::info!("Terminal event sender configured for reasoning engine tool executor");
    }
    
    /// Clear the conversation history (except system prompts)
    pub async fn clear_history(&self) -> Result<(), SagittaCodeError> {
        self.history.clear().await;
        Ok(())
    }
    
    /// Request breaking out of the current reasoning loop
    pub async fn request_loop_break(&self) {
        let mut break_flag = self.loop_break_requested.lock().await;
        *break_flag = true;
        log::info!("Loop break requested via Agent flag");
        // NOTE: This flag is for sagitta-code. ReasoningEngine::process does not currently accept a break flag.
        // True cancellation would require changes in ReasoningEngine or task abortion.
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
    
    /// Register a tool with the agent
    pub async fn register_tool(&self, tool: Arc<dyn crate::tools::types::Tool>) -> Result<(), SagittaCodeError> {
        self.tool_registry.register(tool).await
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
        let mut manager_guard = manager.lock().await;
        manager_guard.create_conversation(title, None).await
            .map_err(|e| SagittaCodeError::Unknown(format!("Failed to create conversation: {}", e)))
    }
    
    /// List all conversations
    pub async fn list_conversations(&self) -> Result<Vec<crate::agent::conversation::types::ConversationSummary>, SagittaCodeError> {
        let manager = self.get_conversation_manager();
        let manager_guard = manager.lock().await;
        manager_guard.list_conversations(None).await
            .map_err(|e| SagittaCodeError::Unknown(format!("Failed to list conversations: {}", e)))
    }

    // Added getter for StateManager's state Arc
    pub fn get_state_manager_state_info_arc(&self) -> Arc<tokio::sync::RwLock<AgentStateInfo>> {
        self.state_manager.get_state_arc()
    }
}

// Include tests from separate module
#[cfg(test)]
mod tests;

// Adapter for reasoning_engine::StreamHandler
pub(crate) struct AgentStreamHandler {
    raw_chunk_sender: mpsc::UnboundedSender<Result<SagittaCodeStreamChunk, SagittaCodeError>>,
    agent_event_sender: broadcast::Sender<AgentEvent>,
    conversation_id: Option<Uuid>, // Added conversation_id
    history: Arc<ConversationAwareHistoryManager>, // NEW: Add history manager
    buffered_response: Arc<tokio::sync::Mutex<String>>, // NEW: Buffer for assistant response
}

impl AgentStreamHandler {
    pub(crate) fn new(
        raw_chunk_sender: mpsc::UnboundedSender<Result<SagittaCodeStreamChunk, SagittaCodeError>>,
        agent_event_sender: broadcast::Sender<AgentEvent>,
        conversation_id: Option<Uuid>, // Added conversation_id
        history: Arc<ConversationAwareHistoryManager>, // NEW: Add history parameter
    ) -> Self {
        Self { 
            raw_chunk_sender, 
            agent_event_sender, 
            conversation_id,
            history, // NEW: Store history
            buffered_response: Arc::new(tokio::sync::Mutex::new(String::new())), // NEW: Initialize buffer
        }
    }
}

#[async_trait]
impl ReasoningStreamHandlerTrait for AgentStreamHandler {
    // This now correctly receives reasoning_engine::streaming::StreamChunk (the struct)
    async fn handle_chunk(&self, chunk: reasoning_engine::streaming::StreamChunk) -> Result<(), ReasoningError> {
        log::debug!("AgentStreamHandler: Processing chunk with type='{}', data_len={}, is_final={}", 
                   chunk.chunk_type, chunk.data.len(), chunk.is_final);

        let sagitta_code_chunk_part_result: Result<Option<SagittaCodeMessagePart>, SagittaCodeError> = match chunk.chunk_type.as_str() {
            "text" | "llm_text" | "llm_output" => {
                match String::from_utf8(chunk.data.clone()) {
                    Ok(text_content) => {
                        log::debug!("AgentStreamHandler: Text chunk content: '{}'", text_content);
                        
                        // NEW: Buffer the text content for assistant message
                        if !text_content.trim().is_empty() {
                            let mut buffer = self.buffered_response.lock().await;
                            buffer.push_str(&text_content);
                            log::debug!("AgentStreamHandler: Buffered text, total length: {}", buffer.len());
                        }
                        
                        // CRITICAL FIX: Always create a text part for non-empty text content
                        if !text_content.trim().is_empty() {
                            Ok(Some(SagittaCodeMessagePart::Text { text: text_content }))
                        } else {
                            Ok(None) // Skip empty text chunks
                        }
                    }
                    Err(e) => {
                        log::error!("AgentStreamHandler: Failed to parse text chunk as UTF-8: {}", e);
                        Err(SagittaCodeError::ParseError(format!("Stream chunk data is not valid UTF-8 for text: {}", e)))
                    }
                }
            }
            "tool_call" => {
                // Assuming chunk.data for tool_call is JSON bytes of reasoning_engine::traits::ToolCall
                match serde_json::from_slice::<reasoning_engine::traits::ToolCall>(&chunk.data) {
                    Ok(tc_data) => {
                        log::debug!("AgentStreamHandler: ToolCall data parsed successfully");
                        Ok(Some(SagittaCodeMessagePart::ToolCall {
                            tool_call_id: Uuid::new_v4().to_string(),
                            name: tc_data.name,
                            parameters: tc_data.args,
                        }))
                    }
                    Err(e) => {
                        log::error!("AgentStreamHandler: Failed to parse tool_call data: {}", e);
                        Err(SagittaCodeError::ParseError(format!("Failed to parse tool_call data: {}", e)))
                    }
                }
            }
            "thought" => { // Assuming thoughts might also come as simple text with this chunk_type
                 String::from_utf8(chunk.data.clone())
                    .map_err(|e| SagittaCodeError::ParseError(format!("Stream chunk data is not valid UTF-8 for thought: {}", e)))
                    .map(|text_content| {
                        if !text_content.trim().is_empty() {
                            Some(SagittaCodeMessagePart::Thought { text: text_content })
                        } else {
                            None
                        }
                    })
            }
            "tool_result" => {
                // First, attempt to parse the canonical UiToolResultChunk struct
                match serde_json::from_slice::<terminal_stream::UiToolResultChunk>(&chunk.data) {
                    Ok(ui_chunk) => {
                        log::debug!("AgentStreamHandler: UiToolResultChunk parsed successfully");
                        Ok(Some(SagittaCodeMessagePart::ToolResult {
                            tool_call_id: ui_chunk.tool_call_id,
                            name: ui_chunk.name,
                            result: ui_chunk.data,
                        }))
                    }
                    Err(parse_err) => {
                        // Fall back to generic ToolResult (older format) or raw bytes so the stream does not fail
                        log::warn!("AgentStreamHandler: Failed to parse UiToolResultChunk: {}. Falling back to generic handling.", parse_err);

                        match serde_json::from_slice::<serde_json::Value>(&chunk.data) {
                            Ok(raw_json) => Ok(Some(SagittaCodeMessagePart::ToolResult {
                                tool_call_id: chunk.metadata.get("tool_call_id")
                                    .map(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string(),
                                name: chunk.metadata.get("tool_name")
                                    .map(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string(),
                                result: raw_json,
                            })),
                            Err(_) => {
                                // As an absolute last resort, emit a Text chunk with base64 content
                                Ok(Some(SagittaCodeMessagePart::Text { text: format!("[Unparsable tool_result]: {} bytes", chunk.data.len()) }))
                            }
                        }
                    }
                }
            }
            // Other chunk_types like "tool_result" could be handled if the ReasoningEngine sends them.
            _ => {
                log::debug!("AgentStreamHandler: Received chunk_type: '{}' with {} bytes", chunk.chunk_type, chunk.data.len());
                // CRITICAL FIX: Try to parse unknown chunk types as text if they contain valid UTF-8
                if let Ok(text_content) = String::from_utf8(chunk.data.clone()) {
                    if !text_content.trim().is_empty() {
                        log::debug!("AgentStreamHandler: Treating unknown chunk_type '{}' as text: '{}'", chunk.chunk_type, text_content);
                        
                        // NEW: Buffer unknown text content too
                        let mut buffer = self.buffered_response.lock().await;
                        buffer.push_str(&text_content);
                        log::debug!("AgentStreamHandler: Buffered unknown text, total length: {}", buffer.len());
                        
                        Ok(Some(SagittaCodeMessagePart::Text { text: text_content }))
                    } else {
                        Ok(None)
                    }
                } else {
                    log::warn!("AgentStreamHandler: Received unhandled chunk_type: '{}' with non-UTF8 data", chunk.chunk_type);
                    Ok(None)
                }
            }
        };

        // Store the parsed UiToolResultChunk for later use if this is a tool result
        let parsed_ui_tool_result = if chunk.chunk_type == "tool_result" {
            serde_json::from_slice::<terminal_stream::UiToolResultChunk>(&chunk.data).ok()
        } else {
            None
        };

        // NEW: Save assistant message to history when final chunk is received
        if chunk.is_final {
            let buffer = self.buffered_response.lock().await;
            if !buffer.trim().is_empty() {
                log::info!("AgentStreamHandler: Stream is final, saving assistant message to history. Content length: {}", buffer.len());
                let assistant_content = buffer.clone();
                drop(buffer); // Release the lock before async operation
                
                // Save the complete assistant message to history
                self.history.add_assistant_message(assistant_content.clone()).await;
                log::info!("AgentStreamHandler: Successfully saved assistant message to history");
                
                // Emit an AgentEvent::LlmMessage for GUI consumption
                let assistant_message = AgentMessage::assistant(assistant_content);
                if let Err(e) = self.agent_event_sender.send(AgentEvent::LlmMessage(assistant_message)) {
                    log::warn!("AgentStreamHandler: Failed to send AgentEvent::LlmMessage: {}", e);
                }
                
                // Clear the buffer for next conversation turn
                let mut buffer = self.buffered_response.lock().await;
                buffer.clear();
                log::debug!("AgentStreamHandler: Cleared response buffer");
            } else {
                log::debug!("AgentStreamHandler: Stream is final but buffer is empty, not saving to history");
            }
        }

        match sagitta_code_chunk_part_result {
            Ok(Some(sagitta_code_part)) => {
                let sagitta_code_stream_chunk = SagittaCodeStreamChunk {
                    part: sagitta_code_part.clone(),
                    is_final: chunk.is_final,
                    finish_reason: chunk.metadata.get("finish_reason").cloned(), // metadata value is String
                    token_usage: None, // This handler receives the struct StreamChunk, not the enum with TokenUsage
                };
                
                // CRITICAL FIX: Always send to raw chunk sender for stream consumption
                if let Err(_) = self.raw_chunk_sender.send(Ok(sagitta_code_stream_chunk)) {
                    log::warn!("AgentStreamHandler: Raw chunk receiver dropped.");
                    // Don't return an error when receiver is dropped - this is expected when stream completes
                    // return Err(ReasoningError::streaming("raw_sender_dropped".to_string(), "Failed to send to raw_chunk_sender".to_string()));
                } else {
                    log::debug!("AgentStreamHandler: Successfully sent chunk to stream");
                }
                
                // ALSO emit AgentEvent::LlmChunk for GUI consumption
                match &sagitta_code_part {
                    SagittaCodeMessagePart::Text { text } => {
                        log::debug!("AgentStreamHandler: Emitting LlmChunk event for text: '{}'", text);
                        if let Err(e) = self.agent_event_sender.send(AgentEvent::LlmChunk {
                            content: text.clone(),
                            is_final: chunk.is_final,
                        }) {
                            log::warn!("AgentStreamHandler: Failed to send AgentEvent::LlmChunk: {}", e);
                        }
                    }
                    SagittaCodeMessagePart::Thought { text } => {
                        if let Err(e) = self.agent_event_sender.send(AgentEvent::LlmChunk {
                            content: format!("THINKING: {}", text),
                            is_final: chunk.is_final,
                        }) {
                            log::warn!("AgentStreamHandler: Failed to send AgentEvent::LlmChunk for thought: {}", e);
                        }
                    }
                    SagittaCodeMessagePart::ToolCall { tool_call_id, name, parameters, .. } => {
                        // CRITICAL FIX: Emit proper ToolCall event for GUI tool card creation
                        log::debug!("AgentStreamHandler: Emitting ToolCall event for tool: '{}'", name);
                        
                        // Convert to the AgentMessage ToolCall format expected by the GUI
                        let gui_tool_call = crate::agent::message::types::ToolCall {
                            id: tool_call_id.clone(),
                            name: name.clone(),
                            arguments: parameters.clone(),
                            result: None, // Will be populated by the tool result
                            successful: false, // Will be updated by the tool result
                            execution_time: None, // Will be populated by the tool result
                        };
                        
                        if let Err(e) = self.agent_event_sender.send(AgentEvent::ToolCall {
                            tool_call: gui_tool_call,
                        }) {
                            log::warn!("AgentStreamHandler: Failed to send AgentEvent::ToolCall: {}", e);
                        }
                        
                        // NOTE: Removed "🔧 Executing tool" text chunks
                        // The GUI handles tool calls via the ToolCall event and shows them as clickable cards
                        // Text chunks for tool execution are redundant
                    }
                    SagittaCodeMessagePart::ToolResult { tool_call_id, name, result } => {
                        // CRITICAL FIX: Emit proper ToolCallComplete event for GUI tool card connection
                        log::debug!("AgentStreamHandler: Emitting ToolCallComplete event for tool: '{}'", name);
                        
                        // Try to extract success information from the stored parsed UiToolResultChunk if available
                        let (tool_result, _result_summary) = if let Some(ui_chunk) = &parsed_ui_tool_result {
                            // Use the original UiToolResultChunk data for accurate success/error info
                            let tool_result = if ui_chunk.success {
                                crate::tools::types::ToolResult::Success(ui_chunk.data.clone())
                            } else {
                                crate::tools::types::ToolResult::Error { 
                                    error: ui_chunk.error.clone().unwrap_or_else(|| "Tool execution failed".to_string())
                                }
                            };
                            
                            let summary = if ui_chunk.success {
                                format!("✅ Tool {} completed in {}ms", name, "234") // TODO: Extract timing from metadata
                            } else {
                                format!("❌ Tool {} failed: {}", name, ui_chunk.error.as_deref().unwrap_or("Unknown error"))
                            };
                            
                            (tool_result, summary)
                        } else {
                            // Fallback: assume success if we have result data
                            let tool_result = crate::tools::types::ToolResult::Success(result.clone());
                            let summary = match result.as_str() {
                                Some(str_result) if str_result.len() > 100 => {
                                    format!("✅ Tool {} completed in {}ms", name, "234")
                                }
                                Some(str_result) => {
                                    format!("✅ Tool {} completed: {}", name, str_result)
                                }
                                None => {
                                    format!("✅ Tool {} completed in {}ms", name, "234")
                                }
                            };
                            (tool_result, summary)
                        };
                        
                        if let Err(e) = self.agent_event_sender.send(AgentEvent::ToolCallComplete {
                            tool_call_id: tool_call_id.clone(),
                            tool_name: name.clone(),
                            result: tool_result,
                        }) {
                            log::warn!("AgentStreamHandler: Failed to send AgentEvent::ToolCallComplete: {}", e);
                        }
                        
                        // NOTE: Removed text chunk emission for tool results
                        // The GUI handles tool results via the ToolCallComplete event and creates clickable cards
                        // Tool completion messages are redundant since we show status icons
                    }
                    _ => {
                        // For other types, send a generic processing indicator with emoji
                        if let Err(e) = self.agent_event_sender.send(AgentEvent::LlmChunk {
                            content: "⏳ Processing...".to_string(),
                            is_final: chunk.is_final,
                        }) {
                            log::warn!("AgentStreamHandler: Failed to send AgentEvent::LlmChunk for other type: {}", e);
                        }
                    }
                }
                
                // Handle token usage reporting if conversation_id is available
                if let Some(conv_id) = self.conversation_id {
                    if let Some(usage_metadata) = chunk.metadata.get("usage_metadata") {
                        // Try to parse usage metadata from chunk
                        if let Ok(usage_data) = serde_json::from_str::<serde_json::Value>(usage_metadata) {
                            if let (Some(input_tokens), Some(output_tokens)) = (
                                usage_data.get("promptTokenCount").and_then(|v| v.as_u64()),
                                usage_data.get("candidatesTokenCount").and_then(|v| v.as_u64())
                            ) {
                                let model_name = chunk.metadata.get("model_name")
                                    .cloned()
                                    .unwrap_or_else(|| "gemini-1.5-flash".to_string());
                                
                                if let Err(e) = self.agent_event_sender.send(AgentEvent::TokenUsageReport {
                                    conversation_id: Some(conv_id),
                                    model_name,
                                    prompt_tokens: input_tokens as u32,
                                    completion_tokens: output_tokens as u32,
                                    cached_tokens: None,
                                    total_tokens: (input_tokens + output_tokens) as u32,
                                }) {
                                    log::warn!("AgentStreamHandler: Failed to send TokenUsageReport: {}", e);
                                }
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                log::debug!("AgentStreamHandler: Skipping chunk (no content to forward)");
                // Unhandled or intentionally skipped chunk type
            }
            Err(e) => {
                log::error!("AgentStreamHandler: Error processing chunk data: {}", e);
                if self.raw_chunk_sender.send(Err(e)).is_err() {
                    log::warn!("AgentStreamHandler: Raw chunk receiver dropped while sending error.");
                }
            }
        }
        Ok(())
    }

    async fn handle_stream_complete(&self, _stream_id: Uuid) -> Result<(), ReasoningError> {
        debug!("AgentStreamHandler: Stream complete notification received for stream_id: {}", _stream_id);

        // NEW: Ensure any buffered assistant content is persisted even if the ReasoningEngine
        // did not flag the final chunk with `is_final = true`. This provides an additional
        // safeguard so that assistant responses are never lost.

        let mut buffer_guard = self.buffered_response.lock().await;

        if !buffer_guard.trim().is_empty() {
            let assistant_content = buffer_guard.clone();

            // Persist to history.
            self.history.add_assistant_message(assistant_content.clone()).await;
            log::info!(
                "AgentStreamHandler: Saved buffered assistant message to history on stream_complete (len={}).",
                assistant_content.len()
            );

            // Emit a full LlmMessage event so the UI can reflect the completed assistant turn.
            let assistant_message = AgentMessage::assistant(assistant_content);
            if let Err(e) = self.agent_event_sender.send(AgentEvent::LlmMessage(assistant_message)) {
                log::warn!("AgentStreamHandler: Failed to send AgentEvent::LlmMessage on stream_complete: {}", e);
            }

            // Clear buffer for safety.
            buffer_guard.clear();
        } else {
            log::debug!("AgentStreamHandler: No buffered content to persist on stream_complete");
        }

        Ok(())
    }

    async fn handle_stream_error(&self, _stream_id: Uuid, error: ReasoningError) -> Result<(), ReasoningError> {
        let sagitta_code_error = SagittaCodeError::ReasoningError(error.to_string());
        if self.raw_chunk_sender.send(Err(sagitta_code_error.clone())).is_err() {
            warn!("AgentStreamHandler: Raw chunk receiver dropped for stream error.");
        }
        if let Err(e) = self.agent_event_sender.send(AgentEvent::Error(sagitta_code_error.to_string())) {
            warn!("AgentStreamHandler: Failed to send AgentEvent::Error: {}", e);
        }
        Err(error) 
    }
}

