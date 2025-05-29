// Event handling for the Fred Agent application

use std::sync::Arc;
use tokio::sync::mpsc;
use uuid;

use crate::agent::Agent;
use crate::agent::message::types::{AgentMessage, ToolCall};
use crate::agent::state::types::AgentState;
use crate::agent::events::AgentEvent;
use crate::llm::client::Role;
use super::super::chat::view::{ChatMessage, MessageAuthor, StreamingMessage, MessageStatus, ToolCall as ViewToolCall, MessageType};
use super::panels::{SystemEventType};
use super::FredAgentApp;

/// Application-specific UI events
#[derive(Debug, Clone)]
pub enum AppEvent {
    ResponseProcessingComplete,
    // Add other app-level UI events here if needed
}

/// Conversation event types
#[derive(Debug, Clone)]
pub enum ConversationEvent {
    DataLoaded {
        current_title: Option<String>,
        conversations: Vec<crate::agent::conversation::types::ConversationSummary>,
    },
    ConversationCreated(uuid::Uuid),
    ConversationSwitched(uuid::Uuid),
}

/// Process agent events
pub fn process_agent_events(app: &mut FredAgentApp) {
    if let Some(ref mut receiver) = app.agent_event_receiver {
        // Collect events first to avoid borrowing issues
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        
        // Then process each event
        for event in events {
            match event {
                AgentEvent::LlmMessage(message) => {
                    // CRITICAL FIX: Only add complete messages if we're NOT currently streaming
                    // This prevents overwriting streaming responses
                    if app.state.current_response_id.is_none() {
                        let chat_message = make_chat_message_from_agent_message(&message);
                        let streaming_message: StreamingMessage = chat_message.into();
                        app.chat_manager.add_complete_message(streaming_message);
                        log::info!("FredAgentApp: Added complete LlmMessage as new message");
                    } else {
                        log::warn!("FredAgentApp: Ignoring complete LlmMessage because we're currently streaming (response_id: {:?})", app.state.current_response_id);
                    }
                    app.state.is_waiting_for_response = false;
                },
                AgentEvent::LlmChunk { content, is_final } => {
                    log::info!("FredAgentApp: GUI received AgentEvent::LlmChunk - content: '{}', is_final: {}", content.chars().take(70).collect::<String>(), is_final);
                    handle_llm_chunk(app, content, is_final, None);
                },
                AgentEvent::ToolCall { tool_call } => {
                    handle_tool_call(app, tool_call);
                },
                AgentEvent::ToolCallComplete { tool_call_id, tool_name, result } => {
                    handle_tool_call_result(app, tool_call_id, tool_name, result);
                },
                AgentEvent::ToolCallPending { tool_call } => {
                    // Add to events panel instead of chat to save space
                    app.panels.events_panel.add_event(
                        SystemEventType::Info,
                        format!("Tool call '{}' is pending your approval", tool_call.name)
                    );
                },
                AgentEvent::StateChanged(state) => {
                    log::debug!("FredAgentApp: Agent state changed to: {:?}", state);
                    handle_state_change(app, state);
                },
                AgentEvent::ToolCallApproved { tool_call_id } => {
                    // Add to events panel instead of chat to save space
                    app.panels.events_panel.add_event(
                        SystemEventType::Info,
                        format!("Tool call {} approved and executing", tool_call_id)
                    );
                },
                AgentEvent::ToolCallRejected { tool_call_id, reason } => {
                    // Add to events panel instead of chat to save space
                    let message_text = if let Some(reason) = reason {
                        format!("Tool call {} rejected: {}", tool_call_id, reason)
                    } else {
                        format!("Tool call {} rejected", tool_call_id)
                    };
                    app.panels.events_panel.add_event(SystemEventType::Error, message_text);
                },
                AgentEvent::ConversationStatusChanged(status) => {
                    log::info!("FredAgentApp: Received ConversationStatusChanged event: {:?}", status);
                    // Potentially refresh UI elements that depend on conversation status here
                },
                AgentEvent::Error(err_msg) => {
                    // Display error in a more prominent way, e.g., a toast or modal
                    // For now, add to events panel
                    app.panels.events_panel.add_event(SystemEventType::Error, err_msg.clone());
                    log::error!("FredAgentApp received error event: {}", err_msg);
                    // Potentially update state to indicate error
                    app.state.is_waiting_for_response = false; // Stop waiting indicator on error
                    app.state.thinking_message = None;
                },
                AgentEvent::ReasoningStarted { session_id, input } => {
                    app.panels.events_panel.add_event(
                        SystemEventType::Info,
                        format!("🧠 Reasoning session {} started for input: '{}'", session_id, &input[..input.len().min(50)])
                    );
                },
                AgentEvent::ReasoningCompleted { session_id, success, duration_ms, steps, tools } => {
                    app.panels.events_panel.add_event(
                        SystemEventType::Info,
                        format!("🧠 Reasoning session {} completed. Success: {}. Duration: {}ms, Steps: {}, Tools: {:?}", 
                                 session_id, success, duration_ms, steps, tools)
                    );
                    // Signal that response processing is complete if it was a reasoning session
                    // This assumes reasoning completion means Fred can take new input.
                    app.state.is_waiting_for_response = false;
                    app.state.thinking_message = None;
                },
                AgentEvent::ReasoningStep { session_id, step, description } => {
                    app.panels.events_panel.add_event(
                        SystemEventType::Info,
                        format!("🧠 Reasoning [{}][Step {}]: {}", session_id, step, description)
                    );
                },
                AgentEvent::ToolCompleted { tool_name, success, duration_ms } => {
                    app.panels.events_panel.add_event(
                        SystemEventType::ToolExecution, // Use ToolExecution type
                        format!("🔧 Tool '{}' completed. Success: {}. Duration: {}ms", tool_name, success, duration_ms)
                    );
                },
                AgentEvent::DecisionMade { session_id, decision, confidence } => {
                    app.panels.events_panel.add_event(
                        SystemEventType::Info,
                        format!("🧠 Reasoning [{}][Decision]: {}. Confidence: {:.2}", session_id, decision, confidence)
                    );
                },
                AgentEvent::Log(log_msg) => {
                    // For now, just log it. Could also go to a specific app log view.
                    log::info!("[AppEventLog]: {}", log_msg);
                    app.panels.events_panel.add_event(SystemEventType::Info, log_msg);
                },
                AgentEvent::TokenUsageReport { conversation_id, model_name, prompt_tokens, completion_tokens, cached_tokens, total_tokens } => {
                    log::info!(
                        "GUI: Received TokenUsageReport: convo_id={:?}, model={}, prompt={}, completion={}, cached={:?}, total={}",
                        conversation_id,
                        model_name,
                        prompt_tokens,
                        completion_tokens,
                        cached_tokens,
                        total_tokens
                    );
                    
                    let conv_id_str = conversation_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "global_usage".to_string()); // Use a specific string for non-conversation usage

                    // Create TokenUsageEntry - Note: cost is calculated inside TokenUsageEntry::new
                    // We also need to ensure the fields align with what TokenUsageEntry expects.
                    // The event already has prompt_tokens, completion_tokens as u32.
                    // TokenUsageEntry::new calculates cost based on these.
                    // We don't explicitly pass cached_tokens or total_tokens to TokenUsageEntry's constructor
                    // as it doesn't use them for cost calculation directly, but they are good for display later.
                    // For now, the primary goal is to get basic token counts and costs in.
                    
                    let entry = crate::gui::app::panels::TokenUsageEntry::new(
                        conv_id_str, // conversation_id: String
                        model_name,  // model_name: String
                        prompt_tokens, // input_tokens: u32
                        completion_tokens // output_tokens: u32
                    );
                    
                    // Add to the analytics panel
                    app.panels.analytics_panel.add_usage_entry(entry);
                },
                // Add catch-all for new/unhandled events to avoid non-exhaustive match error
                _ => {
                    // Optionally log unhandled events: log::debug!("Unhandled AgentEvent: {:?}", event);
                }
            }
        }
    }
}

/// Process application-specific UI events
pub fn process_app_events(app: &mut FredAgentApp) {
    // Collect events first to avoid borrowing issues
    let mut events = Vec::new();
    if let Some(ref mut receiver) = app.app_event_receiver { // Assume app_event_receiver will be added to FredAgentApp
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
    }

    // Then process each event
    for event in events {
        match event {
            AppEvent::ResponseProcessingComplete => {
                log::info!("FredAgentApp: Received ResponseProcessingComplete event. Resetting is_waiting_for_response.");
                app.state.is_waiting_for_response = false;
                // Potentially clear other related state if needed, e.g., thinking indicators
                app.state.is_thinking = false;
                app.state.is_responding = false;

                // Ensure current_response_id is also cleared if the stream truly finished.
                // This might already be handled by AgentEvent::LlmChunk is_final=true,
                // but adding it here ensures consistency if the event is sent separately.
                if app.state.current_response_id.is_some() {
                    log::warn!("FredAgentApp: ResponseProcessingComplete event received, but current_response_id was still set. Clearing it now.");
                    // If there's an active response ID, try to finalize it in chat_manager
                    // This is a safeguard; ideally, it's finalized when the stream ends.
                    app.chat_manager.finish_streaming(app.state.current_response_id.as_ref().unwrap());
                    app.state.current_response_id = None;
                }
            }
        }
    }
}

/// Create a chat message from an agent message
pub fn make_chat_message_from_agent_message(agent_msg: &AgentMessage) -> ChatMessage {
    let author = match agent_msg.role {
        Role::User => MessageAuthor::User,
        Role::Assistant => MessageAuthor::Agent,
        Role::System => MessageAuthor::System,
        Role::Function => MessageAuthor::Tool,
    };

    let mut chat_message = ChatMessage::new(author, agent_msg.content.clone());
    chat_message.id = Some(agent_msg.id.to_string());
    // Optionally, set a summary marker in the ChatMessage (if needed)
    chat_message
}

/// Handle LLM chunk events
pub fn handle_llm_chunk(app: &mut FredAgentApp, content: String, is_final: bool, message_type: Option<MessageType>) {
    // CRITICAL FIX: Always start a new response for each new conversation turn
    // Don't reuse existing response IDs from previous messages
    if app.state.current_response_id.is_none() {
        let response_id = app.chat_manager.start_agent_response();
        app.state.current_response_id = Some(response_id);
        log::info!("FredAgentApp: Started NEW agent response with ID: {}", app.state.current_response_id.as_ref().unwrap());
    }
    
    // Check if this is thinking content
    if let Some(ref response_id) = app.state.current_response_id {
        if content.starts_with("THINKING:") {
            // This is thinking content - use new streaming thinking in conversation view
            let thinking_text = content.strip_prefix("THINKING:").unwrap_or(&content);
            
            // Use the new streaming thinking functionality
            log::info!("handle_llm_chunk: Appending THINKING content for ID: {}", response_id);
            app.chat_manager.append_thinking(response_id, thinking_text.to_string());
            
            // Add to events panel for system tracking
            app.panels.events_panel.add_event(
                SystemEventType::Info,
                format!("Thinking: {}", thinking_text.chars().take(100).collect::<String>())
            );
            
            log::info!("FredAgentApp: Added thinking content to conversation stream: {} chars", thinking_text.len());
            
            // Clear the old modal thinking indicator since we're now using inline thinking
            app.state.thinking_message = None;
            app.state.thinking_start_time = None;
        } else if !content.is_empty() {
            // This is regular text content (only append if not empty)
            // When regular content starts, the thinking will automatically start fading
            log::info!("handle_llm_chunk: Appending REGULAR content for ID: '{}': '{}'", response_id, content.chars().take(70).collect::<String>());
            
            // Check if this is a summary message from the reasoning engine
            let detected_message_type = message_type.or_else(|| {
                // Detect summary messages from reasoning engine based on content patterns
                if content.contains("Okay, I've finished those tasks") || 
                   content.contains("Successfully completed:") ||
                   content.contains("What would you like to do next?") {
                    Some(MessageType::Summary)
                } else {
                    None
                }
            });
            
            // Propagate message_type if provided or detected
            if let Some(mt) = detected_message_type {
                if mt == MessageType::Summary {
                    app.chat_manager.append_content(response_id, content.clone());
                    app.chat_manager.set_message_type(response_id, MessageType::Summary);
                    log::info!("handle_llm_chunk: Tagged message as Summary type");
                } else {
                    app.chat_manager.append_content(response_id, content);
                }
            } else {
                app.chat_manager.append_content(response_id, content);
            }
            
            // Clear the old modal thinking indicator
            app.state.thinking_message = None;
            app.state.thinking_start_time = None;
        }
        // Note: Empty content is allowed for final chunks to signal completion
        
        if is_final {
            // Finish thinking stream if it was active
            app.chat_manager.finish_thinking_stream(response_id);
            
            app.chat_manager.finish_streaming(response_id);
            app.state.current_response_id = None;
            app.state.is_waiting_for_response = false;
            log::info!("FredAgentApp: Finished streaming response, cleared current_response_id for NEXT response");
        }
    }
}

/// Handle tool call events
pub fn handle_tool_call(app: &mut FredAgentApp, tool_call: ToolCall) {
    // Add to events panel for system tracking
    app.panels.events_panel.add_event(
        SystemEventType::ToolExecution,
        format!("Executing tool (events.rs): {}", tool_call.name)
    );
    
    // Add tool call to the current streaming message if one exists
    if let Some(ref response_id) = app.state.current_response_id {
        let view_tool_call = ViewToolCall {
            name: tool_call.name.clone(),
            arguments: serde_json::to_string(&tool_call.arguments).unwrap_or_default(),
            result: None,
            status: MessageStatus::Streaming,
        };
        app.chat_manager.add_tool_call(response_id, view_tool_call);
    } else {
        // Add to events panel instead of chat to save space
        app.panels.events_panel.add_event(
            SystemEventType::ToolExecution,
            format!("Calling tool (events.rs): {}", tool_call.name)
        );
    }
    
    // Optionally show detailed arguments in the preview pane if needed
    if let Ok(json) = serde_json::to_string_pretty(&tool_call.arguments) {
        app.show_preview(&format!("{} Arguments", tool_call.name), &json);
    }
}

/// Handle tool call result events
pub fn handle_tool_call_result(app: &mut FredAgentApp, tool_call_id: String, tool_name: String, result: crate::tools::types::ToolResult) {
    // Add to events panel
    let event_message = match &result {
        crate::tools::types::ToolResult::Success(_) => {
            format!("Tool {} completed successfully", tool_name)
        },
        crate::tools::types::ToolResult::Error { error } => {
            format!("Tool {} failed: {}", tool_name, error)
        }
    };
    
    let event_type = match &result {
        crate::tools::types::ToolResult::Success(_) => SystemEventType::ToolExecution,
        crate::tools::types::ToolResult::Error { .. } => SystemEventType::Error,
    };
    
    app.panels.events_panel.add_event(event_type, event_message);
    
    // CRITICAL FIX: Update tool call status in the streaming chat manager
    let result_string = match &result {
        crate::tools::types::ToolResult::Success(data) => {
            serde_json::to_string_pretty(data).unwrap_or_else(|_| format!("{:?}", data))
        },
        crate::tools::types::ToolResult::Error { error } => {
            format!("Error: {}", error)
        }
    };
    
    let is_success = matches!(result, crate::tools::types::ToolResult::Success(_));
    
    // Update tool call status in the streaming chat manager
    // Try to update by tool_call_id first (most precise)
    let updated = app.chat_manager.update_tool_call_result_by_id(&tool_call_id, result_string.clone(), is_success);
    
    if !updated {
        // Fallback: try to update by tool name
        let updated_by_name = app.chat_manager.update_tool_call_result_by_name(&tool_name, result_string.clone(), is_success);
        
        if !updated_by_name {
            log::warn!("Could not find tool call to update: ID={}, Name={}", tool_call_id, tool_name);
        }
    }
    
    // Store the result for potential preview display
    app.state.tool_results.insert(tool_call_id.clone(), result_string);
    
    log::info!("Tool call {} ({}) completed with result stored", tool_call_id, tool_name);
}

/// Handle agent state changes
pub fn handle_state_change(app: &mut FredAgentApp, state: AgentState) {
    // Add state changes to events panel
    let (state_message, event_type) = match &state {
        AgentState::Idle => {
            app.state.is_waiting_for_response = false;
            if app.state.is_in_loop {
                log::info!("Agent exited loop state - updating UI (handle_state_change)");
                app.state.is_in_loop = false;
                app.state.loop_break_requested = false;
            }
            ("Agent is Idle".to_string(), SystemEventType::StateChange)
        },
        AgentState::Thinking { message: _ } => {
            app.state.is_waiting_for_response = true; 
            app.state.is_thinking = true;
            app.state.is_responding = false;
            app.state.is_executing_tool = false;
            ("Agent started thinking".to_string(), SystemEventType::StateChange)
        },
        AgentState::Responding { is_streaming, step_info: _ } => {
            app.state.is_waiting_for_response = true;
            app.state.is_thinking = false;
            app.state.is_responding = true;
            app.state.is_streaming_response = *is_streaming;
            app.state.is_executing_tool = false;
            if *is_streaming {
                ("Agent started responding (streaming)".to_string(), SystemEventType::StateChange) 
            } else { 
                ("Agent started responding".to_string(), SystemEventType::StateChange)
            }
        },
        AgentState::ExecutingTool { tool_name, .. } => {
            // This specific log is now primarily handled by handle_tool_call
            // But we can still log the state transition if desired.
            (format!("Agent state: Executing tool (events.rs) {}", tool_name), SystemEventType::StateChange)
        },
        AgentState::InLoop { step, interruptible } => {
            if *interruptible {
                (format!("Agent entered reasoning loop (step {}, interruptible)", step), SystemEventType::StateChange)
            } else {
                (format!("Agent entered reasoning loop (step {}, not interruptible)", step), SystemEventType::StateChange)
            }
        },
        AgentState::Error { message, .. } => {
            app.state.is_waiting_for_response = false;
            app.state.current_response_id = None;
            app.state.is_thinking = false;
            app.state.is_responding = false;
            app.state.is_executing_tool = false;
            (format!("Agent error: {}", message), SystemEventType::Error)
        },
    };
    
    if !state_message.is_empty() {
        app.panels.events_panel.add_event(event_type, state_message);
    }
    
    // Update app.state.is_in_loop based on AgentState (ensure this logic is consistent)
    match &state {
        AgentState::InLoop { .. } => {
            if !app.state.is_in_loop {
                log::info!("Agent entered loop state - updating UI (handle_state_change)");
                app.state.is_in_loop = true;
            }
        },
        AgentState::Idle | AgentState::Error { .. } => {
            if app.state.is_in_loop {
                log::info!("Agent exited loop state (Idle/Error) - updating UI (handle_state_change)");
                app.state.is_in_loop = false;
                app.state.loop_break_requested = false;
            }
        },
        _ => {
            // For other states, don't change loop state unless explicitly exiting
            // This ensures if we are in a loop and go to Thinking/Responding, we don't accidentally set is_in_loop to false.
        }
    }
}

/// Process conversation events from async tasks
pub fn process_conversation_events(app: &mut FredAgentApp) {
    // Collect events first to avoid borrowing issues
    let mut events = Vec::new();
    if let Some(ref mut receiver) = app.conversation_event_receiver {
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
    }
    
    // Then process each event
    for event in events {
        match event {
            ConversationEvent::DataLoaded { current_title, conversations } => {
                app.state.current_conversation_title = current_title;
                app.state.conversation_list = conversations;
                app.state.conversation_data_loading = false;
                log::debug!("Updated conversation cache with {} conversations", app.state.conversation_list.len());
            },
            ConversationEvent::ConversationCreated(id) => {
                log::info!("Conversation created: {}", id);
                // Force refresh to show the new conversation
                force_refresh_conversation_data(app);
            },
            ConversationEvent::ConversationSwitched(id) => {
                log::info!("Conversation switched: {}", id);
                // Force refresh to update current conversation display
                force_refresh_conversation_data(app);
            },
        }
    }
}

/// Refresh conversation data asynchronously
pub fn refresh_conversation_data(app: &mut FredAgentApp) {
    if let Some(agent) = &app.agent {
        // Check if we should refresh (every 10 seconds max to reduce flickering)
        let should_refresh = app.state.last_conversation_refresh
            .map(|last| last.elapsed().as_secs() >= 10)
            .unwrap_or(true);
        
        if should_refresh && !app.state.conversation_data_loading {
            app.state.conversation_data_loading = true;
            let agent_clone = agent.clone();
            let sender = app.conversation_event_sender.clone();
            
            // Spawn async task to load conversation data
            tokio::spawn(async move {
                if let Some(sender) = sender {
                    // Load current conversation
                    let current_conversation = agent_clone.get_current_conversation().await.ok().flatten();
                    let current_title = current_conversation.map(|c| c.title);
                    
                    // Load conversation list
                    let conversation_list = agent_clone.list_conversations().await.unwrap_or_default();
                    
                    // Send the data back to the UI
                    let _ = sender.send(ConversationEvent::DataLoaded {
                        current_title,
                        conversations: conversation_list,
                    });
                }
            });
            
            app.state.last_conversation_refresh = Some(std::time::Instant::now());
        }
    }
}

/// Force refresh conversation data immediately
pub fn force_refresh_conversation_data(app: &mut FredAgentApp) {
    app.state.last_conversation_refresh = None;
    app.state.conversation_data_loading = false;
    refresh_conversation_data(app);
}

/// Switch to a conversation and update the chat view
pub fn switch_to_conversation(app: &mut FredAgentApp, conversation_id: uuid::Uuid) {
    if let Some(agent) = &app.agent {
        let agent_clone = agent.clone();
        let sender = app.conversation_event_sender.clone();
        let chat_manager = app.chat_manager.clone();
        
        tokio::spawn(async move {
            match agent_clone.switch_conversation(conversation_id).await {
                Ok(()) => {
                    // Get the conversation history and update chat
                    let history = agent_clone.get_history().await;
                    
                    // Clear current chat and load conversation history
                    chat_manager.clear();
                    
                    // Add all messages from the conversation to chat
                    for message in history {
                        let chat_message = match message.role {
                            crate::llm::client::Role::User => {
                                ChatMessage::new(
                                    MessageAuthor::User,
                                    message.content
                                )
                            },
                            crate::llm::client::Role::Assistant => {
                                ChatMessage::new(
                                    MessageAuthor::Agent,
                                    message.content
                                )
                            },
                            crate::llm::client::Role::System => {
                                ChatMessage::new(
                                    MessageAuthor::System,
                                    message.content
                                )
                            },
                            crate::llm::client::Role::Function => {
                                ChatMessage::new(
                                    MessageAuthor::Tool,
                                    message.content
                                )
                            },
                        };
                        
                        let streaming_message: StreamingMessage = chat_message.into();
                        chat_manager.add_complete_message(streaming_message);
                    }
                    
                    if let Some(sender) = sender {
                        let _ = sender.send(ConversationEvent::ConversationSwitched(conversation_id));
                    }
                },
                Err(e) => {
                    log::error!("Failed to switch conversation: {}", e);
                }
            }
        });
    }
}

impl FredAgentApp {
    pub fn handle_agent_event(&mut self, event: AgentEvent, ctx: &egui::Context) {
        match event {
            AgentEvent::LlmChunk { content, is_final } => {
                self.handle_llm_chunk(content, is_final, ctx);
            }
            AgentEvent::ToolCall { tool_call } => {
                self.state.pending_tool_calls.push_back(tool_call);
            }
            AgentEvent::ToolCallComplete { tool_call_id, tool_name: _, result } => {
                self.state.pending_tool_calls.retain(|tc| tc.id != tool_call_id);
                if let Some(message_id) = self.state.active_tool_call_message_id.take() {
                    if let Some(msg_index) = self.state.messages.iter().rposition(|m| m.id == message_id) {
                        if let Some(tc_index) = self.state.messages[msg_index].tool_calls.iter().rposition(|tc| tc.id == tool_call_id) {
                            self.state.messages[msg_index].tool_calls[tc_index].result = Some(serde_json::to_value(&result).unwrap_or_default());
                            self.state.messages[msg_index].tool_calls[tc_index].successful = result.is_success();
                        }
                    }
                }
                self.state.active_tool_call_message_id = None; 
            }
            AgentEvent::StateChanged(new_state) => {
                self.handle_agent_state_change(new_state, ctx);
            }
            AgentEvent::Error(err_msg) => {
                self.toasts.error(err_msg);
            }
            _ => {}
        }
    }

    pub fn handle_agent_state_change(&mut self, new_state: AgentState, ctx: &egui::Context) {
        log::debug!("Agent state changed to: {:?}", new_state);
        self.state.current_agent_state = new_state.clone(); 

        match new_state {
            AgentState::Idle => {
                self.state.is_thinking = false;
                self.state.is_responding = false;
                self.state.is_executing_tool = false;
            }
            AgentState::Thinking { message: _ } => {
                self.state.is_waiting_for_response = true;
                self.state.is_thinking = true;
                self.state.is_responding = false;
                self.state.is_executing_tool = false;
            }
            AgentState::Responding { is_streaming, step_info: _ } => {
                self.state.is_waiting_for_response = true;
                self.state.is_thinking = false;
                self.state.is_responding = true;
                self.state.is_streaming_response = is_streaming;
                self.state.is_executing_tool = false;
            }
            AgentState::ExecutingTool { tool_name, .. } => { 
                self.state.is_thinking = false; 
                self.state.is_responding = false;
                self.state.is_executing_tool = true;
            }
            AgentState::InLoop { step: _, interruptible: _ } => {
                self.state.is_thinking = true; 
                self.state.is_responding = false;
                self.state.is_executing_tool = false;
            }
            AgentState::Error { message: _, details: _ } => {
                self.state.is_thinking = false;
                self.state.is_responding = false;
                self.state.is_executing_tool = false;
            }
        }
        ctx.request_repaint();
    }

    pub fn handle_llm_chunk(&mut self, content: String, is_final: bool, _ctx: &egui::Context) {
        // CRITICAL FIX: Always start a new response for each new conversation turn
        // Don't reuse existing response IDs from previous messages
        if self.state.current_response_id.is_none() {
            let response_id = self.chat_manager.start_agent_response();
            self.state.current_response_id = Some(response_id);
            log::info!("FredAgentApp: Started NEW agent response with ID: {}", self.state.current_response_id.as_ref().unwrap());
        }
        
        // Check if this is thinking content
        if let Some(ref response_id) = self.state.current_response_id {
            if content.starts_with("THINKING:") {
                // This is thinking content - use new streaming thinking in conversation view
                let thinking_text = content.strip_prefix("THINKING:").unwrap_or(&content);
                
                // Use the new streaming thinking functionality
                self.chat_manager.append_thinking(response_id, thinking_text.to_string());
                
                // Add to events panel for system tracking
                self.panels.events_panel.add_event(
                    SystemEventType::Info,
                    format!("Thinking: {}", thinking_text.chars().take(100).collect::<String>())
                );
                
                self.state.thinking_message = None;
                self.state.thinking_start_time = None;
            } else if !content.is_empty() {
                self.chat_manager.append_content(response_id, content);
                
                // Clear the old modal thinking indicator
                self.state.thinking_message = None;
                self.state.thinking_start_time = None;
            }
            // Note: Empty content is allowed for final chunks to signal completion
            
            if is_final {
                // Finish thinking stream if it was active
                self.chat_manager.finish_thinking_stream(response_id);
                
                self.chat_manager.finish_streaming(response_id);
                self.state.current_response_id = None;
                self.state.is_waiting_for_response = false;
                log::info!("FredAgentApp: Finished streaming response, cleared current_response_id for NEXT response");
            }
        }
    }

    // ... (other methods like switch_to_conversation, etc.) ...
} 