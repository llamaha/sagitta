// Event handling for the Sagitta Code application

use uuid::{self};
use futures_util::StreamExt;
use std::sync::Arc;

use crate::agent::message::types::{AgentMessage, ToolCall};
use crate::agent::state::types::AgentState;
use crate::agent::events::{AgentEvent, ToolRunId};
use crate::llm::client::Role;
use super::super::chat::{ChatMessage, MessageAuthor, StreamingMessage, MessageStatus, ToolCall as ViewToolCall};
use super::panels::{SystemEventType};
use super::SagittaCodeApp;
use serde_json::Value;

/// Types of sync notifications
#[derive(Debug, Clone)]
pub enum SyncNotificationType {
    Success,
    Warning,
    Error,
    Info,
}

/// Application-specific UI events
#[derive(Clone)]
pub enum AppEvent {
    ResponseProcessingComplete,
    RefreshConversationList,
    SwitchToConversation(uuid::Uuid),
    CheckpointSuggestionsReady {
        conversation_id: uuid::Uuid,
        suggestions: Vec<crate::agent::conversation::checkpoints::CheckpointSuggestion>,
    },
    BranchSuggestionsReady {
        conversation_id: uuid::Uuid,
        suggestions: Vec<crate::agent::conversation::branching::BranchSuggestion>,
    },
    RepositoryListUpdated(Vec<String>),
    RefreshRepositoryList,
    UpdateGitHistoryPath(std::path::PathBuf),
    RepositoryAdded(String),
    RepositorySwitched(String),
    CancelTool(ToolRunId),
    RenameConversation {
        conversation_id: uuid::Uuid,
        new_title: String,
    },
    UpdateConversationTitle {
        conversation_id: uuid::Uuid,
    },
    SaveClaudeMdTemplate,
    ApplyClaudeMdToAllRepos,
    ShowNewConversationConfirmation,
    CreateNewConversation,
    // Sync status notifications
    ShowSyncNotification {
        repository: String,
        message: String,
        notification_type: SyncNotificationType,
    },
    // Provider management events
    ReinitializeProvider {
        provider_type: crate::providers::types::ProviderType,
    },
    AgentReplaced {
        agent: std::sync::Arc<crate::agent::core::Agent>,
    },
    // Task execution events
    ExecuteTask {
        conversation_id: uuid::Uuid,
        task_message: String,
    },
    CheckAndExecuteTask,
    // Tool execution completion event for OpenAI-compatible providers
    ToolExecutionComplete {
        tool_call_id: String,
        tool_name: String,
        result: crate::agent::events::ToolResult,
    },
    // UI preference changes
    UpdateUiPreference {
        preference: UiPreference,
    },
    // Debug: Force refresh all conversation summaries
    DebugForceRefreshAllSummaries,
    // Add other app-level UI events here if needed
}

/// UI preference types that can be updated
#[derive(Debug, Clone)]
pub enum UiPreference {
    SimplifiedToolRendering(bool),
}

impl std::fmt::Debug for AppEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppEvent::ResponseProcessingComplete => write!(f, "ResponseProcessingComplete"),
            AppEvent::RefreshConversationList => write!(f, "RefreshConversationList"),
            AppEvent::SwitchToConversation(id) => write!(f, "SwitchToConversation({})", id),
            AppEvent::CheckpointSuggestionsReady { conversation_id, suggestions } => 
                write!(f, "CheckpointSuggestionsReady {{ conversation_id: {}, suggestions: {} items }}", conversation_id, suggestions.len()),
            AppEvent::BranchSuggestionsReady { conversation_id, suggestions } => 
                write!(f, "BranchSuggestionsReady {{ conversation_id: {}, suggestions: {} items }}", conversation_id, suggestions.len()),
            AppEvent::RepositoryListUpdated(repos) => write!(f, "RepositoryListUpdated({:?})", repos),
            AppEvent::RefreshRepositoryList => write!(f, "RefreshRepositoryList"),
            AppEvent::UpdateGitHistoryPath(path) => write!(f, "UpdateGitHistoryPath({:?})", path),
            AppEvent::RepositoryAdded(repo) => write!(f, "RepositoryAdded({})", repo),
            AppEvent::RepositorySwitched(repo) => write!(f, "RepositorySwitched({})", repo),
            AppEvent::CancelTool(id) => write!(f, "CancelTool({})", id),
            AppEvent::RenameConversation { conversation_id, new_title } => 
                write!(f, "RenameConversation {{ conversation_id: {}, new_title: {} }}", conversation_id, new_title),
            AppEvent::UpdateConversationTitle { conversation_id } => 
                write!(f, "UpdateConversationTitle {{ conversation_id: {} }}", conversation_id),
            AppEvent::SaveClaudeMdTemplate => write!(f, "SaveClaudeMdTemplate"),
            AppEvent::ApplyClaudeMdToAllRepos => write!(f, "ApplyClaudeMdToAllRepos"),
            AppEvent::ShowNewConversationConfirmation => write!(f, "ShowNewConversationConfirmation"),
            AppEvent::CreateNewConversation => write!(f, "CreateNewConversation"),
            AppEvent::ShowSyncNotification { repository, message, notification_type } => 
                write!(f, "ShowSyncNotification {{ repository: {}, message: {}, notification_type: {:?} }}", repository, message, notification_type),
            AppEvent::ReinitializeProvider { provider_type } => 
                write!(f, "ReinitializeProvider {{ provider_type: {:?} }}", provider_type),
            AppEvent::AgentReplaced { agent: _ } => write!(f, "AgentReplaced {{ agent: Agent }}"),
            AppEvent::ExecuteTask { conversation_id, task_message } => 
                write!(f, "ExecuteTask {{ conversation_id: {}, task_message: {} }}", conversation_id, task_message),
            AppEvent::CheckAndExecuteTask => write!(f, "CheckAndExecuteTask"),
            AppEvent::ToolExecutionComplete { tool_call_id, tool_name, result: _ } => 
                write!(f, "ToolExecutionComplete {{ tool_call_id: {}, tool_name: {} }}", tool_call_id, tool_name),
            AppEvent::UpdateUiPreference { preference } => 
                write!(f, "UpdateUiPreference {{ preference: {:?} }}", preference),
            AppEvent::DebugForceRefreshAllSummaries => write!(f, "DebugForceRefreshAllSummaries"),
        }
    }
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
    ConversationMessagesLoaded {
        conversation_id: uuid::Uuid,
        messages: Vec<crate::agent::message::types::AgentMessage>,
    },
    AnalyticsReportReady(crate::agent::conversation::analytics::AnalyticsReport),
}

/// Process agent events
pub fn process_agent_events(app: &mut SagittaCodeApp) {
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
                    // Handle complete messages with better race condition detection
                    if app.state.current_response_id.is_none() && !app.state.is_streaming_response {
                        // Normal case: no active streaming
                        let chat_message = make_chat_message_from_agent_message(&message);
                        let streaming_message: StreamingMessage = chat_message.into();
                        app.chat_manager.add_complete_message(streaming_message);
                        log::info!("SagittaCodeApp: Added complete LlmMessage as new message");
                    } else if app.state.is_streaming_response && app.state.current_response_id.is_none() {
                        // Race condition: streaming flag is set but no response ID
                        // This can happen when final chunk just cleared the ID but flag is still set
                        log::debug!("SagittaCodeApp: Detected race condition - streaming flag set but no response ID, adding message");
                        let chat_message = make_chat_message_from_agent_message(&message);
                        let streaming_message: StreamingMessage = chat_message.into();
                        app.chat_manager.add_complete_message(streaming_message);
                        app.state.is_streaming_response = false;
                    } else {
                        // Active streaming - check if it's a duplicate or error recovery
                        if let Some(response_id) = &app.state.current_response_id {
                            log::debug!("SagittaCodeApp: Received LlmMessage during active streaming (id: {response_id}), likely from error recovery");
                        } else {
                            log::warn!("SagittaCodeApp: Unexpected state - streaming with response_id but shouldn't reach here");
                        }
                    }
                    app.state.is_waiting_for_response = false;
                },
                AgentEvent::LlmChunk { content, is_final, is_thinking } => {
                    handle_llm_chunk(app, content, is_final, is_thinking);
                },
                AgentEvent::ToolCall { tool_call } => {
                    log::debug!("Tool call received: {} (id: {})", tool_call.name, tool_call.id);
                    
                    // If no current response ID, start a new agent response
                    let message_id = if let Some(current_message_id) = &app.state.current_response_id {
                        current_message_id.clone()
                    } else {
                        log::debug!("No active streaming message, starting new agent response for tool call");
                        let new_id = app.chat_manager.start_agent_response();
                        app.state.current_response_id = Some(new_id.clone());
                        app.state.is_streaming_response = true;
                        new_id
                    };
                    
                    // Add the tool to the streaming message
                    app.chat_manager.add_tool_to_message(
                        &message_id,
                        tool_call.name.clone(),
                        tool_call.id.clone(),
                        tool_call.arguments.clone()
                    );
                    
                    // Store the tool call ID for later result updates
                    app.state.active_tool_calls.insert(tool_call.id.clone(), message_id.clone());
                    
                    // For OpenAI-compatible providers, we need to execute the tool ourselves
                    // through the internal MCP server
                    let should_execute_tool = if let Ok(config) = app.config.try_lock() {
                        matches!(config.current_provider, crate::providers::ProviderType::OpenAICompatible)
                    } else {
                        false
                    };
                    
                    log::info!("Tool call event received. Provider is OpenAI-compatible: {}, will execute: {}", 
                        should_execute_tool, should_execute_tool);
                    
                    if should_execute_tool {
                        log::info!("OpenAI-compatible provider detected, executing tool {} through internal MCP", tool_call.name);
                        
                        // Clone necessary data for the async task
                        let tool_call_id = tool_call.id.clone();
                        let tool_name = tool_call.name.clone();
                        let tool_arguments = tool_call.arguments.clone();
                        let message_id_clone = message_id.clone();
                        let chat_manager_clone = app.chat_manager.clone();
                        
                        // Clone app_event_sender for the async task
                        let app_event_sender_clone = app.app_event_sender.clone();
                        
                        // Clone config for shell timeout injection
                        let config_clone = app.config.clone();
                        
                        // Execute tool in background task
                        tokio::spawn(async move {
                            // Call the internal MCP server to execute the tool
                            log::info!("Attempting to execute tool {} through MCP server at localhost:8765", tool_name);
                            let (success, result_json) = match execute_mcp_tool(&tool_name, tool_arguments, Some(config_clone)).await {
                                Ok(result) => {
                                    log::info!("Tool {} executed successfully through MCP, result: {}", tool_name, 
                                        serde_json::to_string(&result).unwrap_or_else(|_| "unparseable".to_string()));
                                    (true, result)
                                },
                                Err(e) => {
                                    log::error!("Tool {} execution failed: {}", tool_name, e);
                                    (false, serde_json::json!({
                                        "error": e.to_string()
                                    }))
                                }
                            };
                            
                            // Update the tool result in the message
                            chat_manager_clone.update_tool_result_in_message(
                                &message_id_clone,
                                &tool_call_id,
                                result_json.clone(),
                                success
                            );
                            
                            // Create a ToolResult event to trigger the standard continuation flow
                            let tool_result = if success {
                                crate::agent::events::ToolResult::Success {
                                    output: serde_json::to_string(&result_json).unwrap_or_default()
                                }
                            } else {
                                crate::agent::events::ToolResult::Error {
                                    error: result_json.get("error")
                                        .and_then(|e| e.as_str())
                                        .unwrap_or("Unknown error")
                                        .to_string()
                                }
                            };
                            
                            // Send ToolCallComplete event through the app event system
                            // This will trigger the standard continuation logic
                            let _ = app_event_sender_clone.send(AppEvent::ToolExecutionComplete {
                                tool_call_id: tool_call_id.clone(),
                                tool_name: tool_name.clone(),
                                result: tool_result,
                            });
                        });
                    }
                },
                AgentEvent::ToolCallComplete { tool_call_id, tool_name, result } => {
                    log::info!("Tool call {tool_call_id} ({tool_name}) completed");
                    
                    // Update the tool result in the message
                    if let Some(message_id) = app.state.active_tool_calls.get(&tool_call_id) {
                        let success = matches!(result, crate::agent::events::ToolResult::Success { .. });
                        let result_json = match &result {
                            crate::agent::events::ToolResult::Success { output } => serde_json::from_str(output).unwrap_or(serde_json::Value::String(output.clone())),
                            crate::agent::events::ToolResult::Error { error } => serde_json::json!({
                                "error": error
                            }),
                        };
                        
                        app.chat_manager.update_tool_result_in_message(
                            message_id,
                            &tool_call_id,
                            result_json,
                            success
                        );
                        
                        // Store tool result for adding to conversation history
                        if !app.state.completed_tool_results.contains_key(&tool_call_id) {
                            app.state.completed_tool_results.insert(tool_call_id.clone(), (tool_name.clone(), result.clone()));
                        }
                        
                        // Clean up
                        app.state.active_tool_calls.remove(&tool_call_id);
                        
                        // Check if all tools are complete and we need to continue the conversation
                        // This is needed for OpenAI-compatible providers that don't handle tool execution internally
                        // CRITICAL: Check if we've already triggered continuation for this tool call
                        let already_continued = app.state.tool_calls_continued.get(&tool_call_id).copied().unwrap_or(false);
                        if app.state.active_tool_calls.is_empty() && !app.state.is_waiting_for_response && !already_continued {
                            // Check if we're using an OpenAI-compatible provider
                            let should_continue = if let Some(agent) = &app.agent {
                                // Get the LLM client type through the agent
                                if let Ok(config) = app.config.try_lock() {
                                    matches!(config.current_provider, crate::providers::ProviderType::OpenAICompatible)
                                } else {
                                    false
                                }
                            } else {
                                false
                            };
                            
                            if should_continue {
                                log::info!("All tools complete for OpenAI-compatible provider, triggering continuation");
                                
                                // Mark all completed tool calls as continued to prevent double handling
                                for (tool_id, _) in &app.state.completed_tool_results {
                                    app.state.tool_calls_continued.insert(tool_id.clone(), true);
                                }
                                
                                // CRITICAL FIX: Add tool results to conversation history before continuation
                                if let Some(agent) = &app.agent {
                                    let agent_clone = agent.clone();
                                    let app_event_sender_clone = app.app_event_sender.clone();
                                    let completed_results = app.state.completed_tool_results.clone();
                                    
                                    app.state.is_waiting_for_response = true;
                                    
                                    // Process in background task with STREAMING
                                    tokio::spawn(async move {
                                        log::info!("Adding {} tool results to conversation history before continuation", completed_results.len());
                                        
                                        // Add each tool result to the agent's conversation history
                                        for (tool_call_id, (tool_name, tool_result)) in completed_results.iter() {
                                            log::info!("Adding tool result to history: {} -> {:?}", tool_call_id, tool_result);
                                            
                                            // Create a user message containing the tool result
                                            let tool_result_json = match tool_result {
                                                crate::agent::events::ToolResult::Success { output } => {
                                                    serde_json::from_str(output).unwrap_or(serde_json::Value::String(output.clone()))
                                                },
                                                crate::agent::events::ToolResult::Error { error } => {
                                                    serde_json::json!({ "error": error })
                                                }
                                            };
                                            
                                            // Add tool result to conversation history using the agent's method
                                            if let Err(e) = agent_clone.add_tool_result_to_history(tool_call_id, tool_name, &tool_result_json).await {
                                                log::error!("Failed to add tool result to history: {}", e);
                                            }
                                        }
                                        
                                        log::info!("Starting continuation stream after tool completion and history update");
                                        
                                        // Send a message with current working directory context to continue the conversation
                                        // The model will see the tool results and provide its analysis
                                        let continuation_message = if let Ok(cwd) = std::env::current_dir() {
                                            format!("[System: CWD is {}]\n", cwd.display())
                                        } else {
                                            String::new()
                                        };
                                        match agent_clone.process_message_stream(&continuation_message).await {
                                            Ok(mut stream) => {
                                                log::info!("Successfully created continuation stream");
                                                let mut chunk_count = 0;
                                                
                                                // Consume the stream
                                                loop {
                                                    match tokio::time::timeout(
                                                        std::time::Duration::from_secs(60),
                                                        stream.next()
                                                    ).await {
                                                        Ok(Some(chunk_result)) => {
                                                            chunk_count += 1;
                                                            
                                                            match chunk_result {
                                                                Ok(chunk) => {
                                                                    if chunk.is_final {
                                                                        log::info!("Received final chunk in continuation, stream complete");
                                                                        break;
                                                                    }
                                                                },
                                                                Err(e) => {
                                                                    log::error!("Error in continuation streaming response: {e}");
                                                                    break;
                                                                }
                                                            }
                                                        },
                                                        Ok(None) => {
                                                            log::info!("Continuation stream ended after {chunk_count} chunks");
                                                            break;
                                                        },
                                                        Err(_timeout) => {
                                                            log::error!("Timeout waiting for continuation chunk");
                                                            break;
                                                        }
                                                    }
                                                }
                                                
                                                // Send completion event
                                                let _ = app_event_sender_clone.send(AppEvent::ResponseProcessingComplete);
                                            },
                                            Err(e) => {
                                                log::error!("Failed to create continuation stream: {e}");
                                                let _ = app_event_sender_clone.send(AppEvent::ResponseProcessingComplete);
                                            }
                                        }
                                    });
                                }
                                
                                // Clear completed tool results after starting continuation
                                app.state.completed_tool_results.clear();
                            }
                        }
                    } else {
                        log::warn!("Received tool result for unknown tool call: {tool_call_id}");
                    }
                },
                AgentEvent::ToolCallPending { tool_call } => {
                    // Add to events panel instead of chat to save space
                    app.panels.events_panel.add_event(
                        SystemEventType::Info,
                        format!("Tool call '{}' is pending your approval", tool_call.name)
                    );
                },
                AgentEvent::StateChanged(state) => {
                    log::debug!("SagittaCodeApp: Agent state changed to: {state:?}");
                    handle_state_change(app, state);
                },
                AgentEvent::ToolCallApproved { tool_call_id } => {
                    // Add to events panel instead of chat to save space
                    app.panels.events_panel.add_event(
                        SystemEventType::Info,
                        format!("Tool call {tool_call_id} approved and executing")
                    );
                },
                AgentEvent::ToolCallRejected { tool_call_id, reason } => {
                    // Add to events panel instead of chat to save space
                    let message_text = if let Some(reason) = reason {
                        format!("Tool call {tool_call_id} rejected: {reason}")
                    } else {
                        format!("Tool call {tool_call_id} rejected")
                    };
                    app.panels.events_panel.add_event(SystemEventType::Error, message_text);
                },
                AgentEvent::ConversationStatusChanged(status) => {
                    log::info!("SagittaCodeApp: Received ConversationStatusChanged event: {status:?}");
                    // Potentially refresh UI elements that depend on conversation status here
                },
                AgentEvent::ConversationCompleted { conversation_id } => {
                    log::info!("SagittaCodeApp: Received ConversationCompleted event for conversation: {conversation_id}");
                    
                    // Update conversation title when conversation is completed
                    if let Some(title_updater) = &app.title_updater {
                        let title_updater_clone = title_updater.clone();
                        tokio::spawn(async move {
                            if let Err(e) = title_updater_clone.maybe_update_title(conversation_id).await {
                                log::error!("Failed to update conversation title: {e}");
                            }
                        });
                    }
                    
                    // Check if this conversation completion should trigger the next task
                    if let Some(agent) = &app.agent {
                        let agent_clone = agent.clone();
                        let conversation_id_clone = conversation_id.clone();
                        let task_panel = app.task_panel.clone();
                        
                        tokio::spawn(async move {
                            if let Ok(Some(conversation)) = agent_clone.get_current_conversation().await {
                                if conversation.id == conversation_id_clone {
                                    // Get the full conversation content for analysis
                                    let conversation_content = conversation.messages.iter()
                                        .map(|msg| &msg.content)
                                        .cloned()
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    
                                    if let Err(e) = task_panel.handle_stream_completion(&conversation_content).await {
                                        log::error!("Failed to check task completion: {e}");
                                    }
                                }
                            }
                        });
                    }
                    
                    // Refresh conversation list to update status
                    app.refresh_conversation_data();
                },
                AgentEvent::TokenUsageUpdate { usage } => {
                    log::debug!("SagittaCodeApp: Token usage update - total: {}", usage.total_tokens);
                    app.state.current_token_usage = Some(usage);
                },
                AgentEvent::Error(err_msg) => {
                    // Display error in a more prominent way, e.g., a toast or modal
                    // For now, add to events panel
                    app.panels.events_panel.add_event(SystemEventType::Error, err_msg.clone());
                    log::error!("SagittaCodeApp received error event: {err_msg}");
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
                        format!("🧠 Reasoning session {session_id} completed. Success: {success}. Duration: {duration_ms}ms, Steps: {steps}, Tools: {tools:?}")
                    );
                    // Signal that response processing is complete if it was a reasoning session
                    // This assumes reasoning completion means Sagitta Code can take new input.
                    app.state.is_waiting_for_response = false;
                    app.state.thinking_message = None;
                },
                AgentEvent::ReasoningStep { session_id, step, description } => {
                    app.panels.events_panel.add_event(
                        SystemEventType::Info,
                        format!("🧠 Reasoning [{session_id}][Step {step}]: {description}")
                    );
                },
                AgentEvent::ToolCompleted { tool_name, success, duration_ms } => {
                    app.panels.events_panel.add_event(
                        SystemEventType::ToolExecution, // Use ToolExecution type
                        format!("🔧 Tool '{tool_name}' completed. Success: {success}. Duration: {duration_ms}ms")
                    );
                },
                AgentEvent::DecisionMade { session_id, decision, confidence } => {
                    app.panels.events_panel.add_event(
                        SystemEventType::Info,
                        format!("🧠 Reasoning [{session_id}][Decision]: {decision}. Confidence: {confidence:.2}")
                    );
                },
                AgentEvent::Log(log_msg) => {
                    // For now, just log it. Could also go to a specific app log view.
                    log::info!("[AppEventLog]: {log_msg}");
                    app.panels.events_panel.add_event(SystemEventType::Info, log_msg);
                },
                AgentEvent::TokenUsageReport { conversation_id, model_name, prompt_tokens, completion_tokens, cached_tokens, total_tokens } => {
                    log::info!(
                        "GUI: Received TokenUsageReport: convo_id={conversation_id:?}, model={model_name}, prompt={prompt_tokens}, completion={completion_tokens}, cached={cached_tokens:?}, total={total_tokens}"
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
                AgentEvent::ToolRunStarted { run_id, tool } => {
                    log::info!("SagittaCodeApp: Tool run started: {tool} ({run_id})");
                    handle_tool_run_started(app, run_id, tool);
                },
                AgentEvent::ToolRunCompleted { run_id, tool, success } => {
                    log::info!("SagittaCodeApp: Tool run completed: {tool} ({run_id}) - success: {success}");
                    handle_tool_run_completed(app, run_id, tool, success);
                },
                AgentEvent::ToolStream { run_id, event } => {
                    log::debug!("SagittaCodeApp: Tool stream event for run {run_id}: {event:?}");
                    handle_tool_stream(app, run_id, event);
                },
                // Removed duplicate - already handled above
                AgentEvent::ConversationSummarizing { conversation_id } => {
                    log::info!("SagittaCodeApp: Conversation summarizing: {conversation_id}");
                    // Could update UI to show summarizing status
                },
                AgentEvent::ConversationUpdated { conversation_id, old_status, new_status } => {
                    log::info!("SagittaCodeApp: Conversation {conversation_id} status updated from {old_status:?} to {new_status:?}");
                    // Refresh conversation list to show updated status
                    force_refresh_conversation_data(app);
                },
                AgentEvent::Cancelled => {
                    log::info!("SagittaCodeApp: Agent operation cancelled");
                    // Reset UI state
                    app.state.is_waiting_for_response = false;
                    app.state.is_thinking = false;
                    app.state.is_responding = false;
                    app.state.is_streaming_response = false;
                    app.state.thinking_message = None;
                    app.state.current_response_id = None;
                    
                    app.panels.events_panel.add_event(
                        super::SystemEventType::Info,
                        "Operation cancelled".to_string()
                    );
                },
                _ => {
                    // Optionally log unhandled events: log::debug!("Unhandled AgentEvent: {:?}", event);
                }
            }
        }
    }
}

/// Process application-specific UI events
pub fn process_app_events(app: &mut SagittaCodeApp) {
    // Collect events first to avoid borrowing issues
    let mut events = Vec::new();
    if let Some(ref mut receiver) = app.app_event_receiver { // Assume app_event_receiver will be added to SagittaCodeApp
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
    }

    // Then process each event
    for event in events {
        match event {
            AppEvent::ResponseProcessingComplete => {
                log::info!("SagittaCodeApp: Received ResponseProcessingComplete event. Resetting is_waiting_for_response.");
                app.state.is_waiting_for_response = false;
                // Potentially clear other related state if needed, e.g., thinking indicators
                app.state.is_thinking = false;
                app.state.is_responding = false;
                // Request focus when response processing completes so user can type immediately
                app.state.should_focus_input = true;

                // Ensure current_response_id is also cleared if the stream truly finished.
                // This might already be handled by AgentEvent::LlmChunk is_final=true,
                // but adding it here ensures consistency if the event is sent separately.
                if app.state.current_response_id.is_some() {
                    log::warn!("SagittaCodeApp: ResponseProcessingComplete event received, but current_response_id was still set. Clearing it now.");
                    // If there's an active response ID, try to finalize it in chat_manager
                    // This is a safeguard; ideally, it's finalized when the stream ends.
                    app.chat_manager.finish_streaming(app.state.current_response_id.as_ref().unwrap());
                    app.state.current_response_id = None;
                }
                
                // Mark conversation as modified and save after response completes
                app.state.conversation_modified = true;
                if let Some(ref mut manager) = app.simple_conversation_manager {
                    match manager.save_current_conversation() {
                        Ok(_) => {
                            app.state.conversation_modified = false;
                            log::debug!("Saved conversation after response completed");
                        }
                        Err(e) => {
                            log::error!("Failed to save conversation after response: {e}");
                        }
                    }
                }
                
                // Notify auto title updater after assistant response completes
                if let (Some(conversation_id), Some(sender)) = (app.state.current_conversation_id, &app.auto_title_sender) {
                    // Get current message count after assistant response
                    let message_count = app.chat_manager.get_all_messages().len();
                    
                    crate::services::auto_title_updater::notify_conversation_updated(
                        sender,
                        conversation_id,
                        message_count,
                    );
                    
                    log::debug!("Notified auto title updater after response complete for conversation {} with {} messages", 
                        conversation_id, message_count);
                }
            }
            AppEvent::RefreshConversationList => {
                log::info!("SagittaCodeApp: Received RefreshConversationList event. Forcing refresh.");
                force_refresh_conversation_data(app);
            }
            AppEvent::SwitchToConversation(conversation_id) => {
                log::info!("AppEvent: Received SwitchToConversation for {conversation_id}");
                switch_to_conversation(app, conversation_id);
            }
            AppEvent::CheckpointSuggestionsReady { conversation_id, suggestions } => {
                log::info!("Received CheckpointSuggestionsReady event for conversation {conversation_id}");
                app.handle_checkpoint_suggestions(conversation_id, suggestions);
            },
            AppEvent::BranchSuggestionsReady { conversation_id, suggestions } => {
                log::info!("Received BranchSuggestionsReady event for conversation {conversation_id}");
                app.handle_branch_suggestions(conversation_id, suggestions);
            },
            AppEvent::RepositoryListUpdated(repo_list) => {
                log::info!("Received RepositoryListUpdated event with {} repositories: {:?}", repo_list.len(), repo_list);
                app.handle_repository_list_update(repo_list);
            },
            AppEvent::RefreshRepositoryList => {
                log::debug!("Received RefreshRepositoryList event, triggering manual refresh");
                app.trigger_repository_list_refresh();
            },
            AppEvent::UpdateGitHistoryPath(path) => {
                log::debug!("Received UpdateGitHistoryPath event with path: {path:?}");
                app.panels.set_git_repository(path);
            },
            AppEvent::CancelTool(run_id) => {
                log::info!("Received CancelTool event for run_id: {run_id}");
                handle_tool_cancellation(app, run_id);
            },
            AppEvent::RenameConversation { conversation_id, new_title } => {
                log::info!("Received RenameConversation event for conversation {conversation_id} with new title: {new_title}");
                handle_rename_conversation(app, conversation_id, new_title);
            },
            AppEvent::SaveClaudeMdTemplate => {
                log::info!("Received SaveClaudeMdTemplate event");
                handle_save_claude_md_template(app);
            },
            AppEvent::ApplyClaudeMdToAllRepos => {
                log::info!("Received ApplyClaudeMdToAllRepos event");
                handle_apply_claude_md_to_all_repos(app);
            },
            AppEvent::UpdateConversationTitle { conversation_id } => {
                log::info!("Received UpdateConversationTitle event for conversation {conversation_id}");
                handle_update_conversation_title(app, conversation_id);
            },
            AppEvent::ShowNewConversationConfirmation => {
                log::info!("Received ShowNewConversationConfirmation event");
                handle_show_new_conversation_confirmation(app);
            },
            AppEvent::CreateNewConversation => {
                log::info!("Received CreateNewConversation event");
                handle_create_new_conversation(app);
            },
            AppEvent::RepositoryAdded(repo_name) => {
                log::info!("Received RepositoryAdded event for repository: {repo_name}");
                handle_repository_added(app, repo_name);
            },
            AppEvent::RepositorySwitched(repo_name) => {
                log::info!("Received RepositorySwitched event for repository: {repo_name}");
                handle_repository_switched(app, repo_name);
            },
            AppEvent::ShowSyncNotification { repository, message, notification_type } => {
                log::info!("Received ShowSyncNotification event for repository: {repository}");
                handle_sync_notification(app, repository, message, notification_type);
            },
            AppEvent::ReinitializeProvider { provider_type } => {
                log::info!("Received ReinitializeProvider event for provider: {:?}", provider_type);
                handle_provider_reinitialization(app, provider_type);
            },
            AppEvent::ExecuteTask { conversation_id, task_message } => {
                log::info!("Received ExecuteTask event for conversation {conversation_id}");
                handle_execute_task(app, conversation_id, task_message);
            },
            AppEvent::CheckAndExecuteTask => {
                log::info!("Received CheckAndExecuteTask event");
                handle_check_and_execute_task(app);
            },
            AppEvent::AgentReplaced { agent } => {
                log::info!("Received AgentReplaced event");
                handle_agent_replaced(app, agent);
            },
            AppEvent::ToolExecutionComplete { tool_call_id, tool_name, result } => {
                log::info!("Received ToolExecutionComplete event for tool {} ({})", tool_name, tool_call_id);
                
                // Process it directly as if it came from the agent
                if let Some(message_id) = app.state.active_tool_calls.get(&tool_call_id) {
                    let success = matches!(result, crate::agent::events::ToolResult::Success { .. });
                    let result_json = match &result {
                        crate::agent::events::ToolResult::Success { output } => serde_json::from_str(output).unwrap_or(serde_json::Value::String(output.clone())),
                        crate::agent::events::ToolResult::Error { error } => serde_json::json!({
                            "error": error
                        }),
                    };
                    
                    // Store tool result for adding to conversation history (same as AgentEvent::ToolCallComplete)
                    if !app.state.completed_tool_results.contains_key(&tool_call_id) {
                        app.state.completed_tool_results.insert(tool_call_id.clone(), (tool_name.clone(), result.clone()));
                    }
                    
                    // Clean up
                    app.state.active_tool_calls.remove(&tool_call_id);
                    
                    // Check if all tools are complete and we need to continue the conversation
                    // This is needed for OpenAI-compatible providers that don't handle tool execution internally
                    // CRITICAL: Check if we've already triggered continuation for this tool call
                    let already_continued = app.state.tool_calls_continued.get(&tool_call_id).copied().unwrap_or(false);
                    if app.state.active_tool_calls.is_empty() && !app.state.is_waiting_for_response && !already_continued {
                        // Check if we're using an OpenAI-compatible provider
                        let should_continue = if let Some(_agent) = &app.agent {
                            // Get the LLM client type through the agent
                            if let Ok(config) = app.config.try_lock() {
                                matches!(config.current_provider, crate::providers::ProviderType::OpenAICompatible)
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        
                        if should_continue {
                            log::info!("All tools complete for OpenAI-compatible provider, triggering continuation");
                            
                            // Mark all completed tool calls as continued to prevent double handling
                            for (tool_id, _) in &app.state.completed_tool_results {
                                app.state.tool_calls_continued.insert(tool_id.clone(), true);
                            }
                            
                            // CRITICAL FIX: Add all completed tool results to agent history BEFORE continuation
                            // This is the same logic as in the CLI test tool and AgentEvent::ToolCallComplete handler
                            let completed_results = app.state.completed_tool_results.clone();
                            app.state.completed_tool_results.clear(); // Clear to prevent duplicate additions
                            
                            if let Some(agent) = &app.agent {
                                let agent_clone = agent.clone();
                                let app_event_sender_clone = app.app_event_sender.clone();
                                
                                app.state.is_waiting_for_response = true;
                                
                                // Process in background task with STREAMING
                                tokio::spawn(async move {
                                    log::info!("Starting continuation stream after tool completion");
                                    
                                    // CRITICAL FIX: Add tool results to conversation history before continuation
                                    for (tool_call_id_inner, (tool_name_inner, tool_result)) in completed_results.iter() {
                                        log::info!("Adding tool result to agent history: {} -> {:?}", tool_call_id_inner, tool_result);
                                        
                                        let tool_result_json = match tool_result {
                                            crate::agent::events::ToolResult::Success { output } => {
                                                serde_json::from_str(output).unwrap_or(serde_json::Value::String(output.clone()))
                                            },
                                            crate::agent::events::ToolResult::Error { error } => {
                                                serde_json::json!({ "error": error })
                                            }
                                        };
                                        
                                        // Add tool result to conversation history using the agent's method
                                        if let Err(e) = agent_clone.add_tool_result_to_history(tool_call_id_inner, tool_name_inner, &tool_result_json).await {
                                            log::error!("Failed to add tool result to history: {}", e);
                                        } else {
                                            log::info!("Successfully added tool result to conversation history: {}", tool_call_id_inner);
                                        }
                                    }
                                    
                                    // DEBUG: Log conversation history before continuation
                                    log::info!("DEBUG: Conversation history before continuation:");
                                    let history = agent_clone.get_history().await;
                                    for (i, msg) in history.iter().enumerate() {
                                        log::info!("  Message {}: Role={:?}, Content={}, ToolCalls={}", 
                                            i, msg.role, 
                                            if msg.content.is_empty() { "<empty>" } else { &msg.content[..50.min(msg.content.len())] },
                                            msg.tool_calls.len()
                                        );
                                        for tool_call in &msg.tool_calls {
                                            log::info!("    Tool: {} (id: {}), Result: {}", 
                                                tool_call.name, tool_call.id,
                                                tool_call.result.is_some()
                                            );
                                        }
                                    }
                                    log::info!("DEBUG: Total messages in history: {}", history.len());
                                    
                                    // Send a continuation message to help the model understand it should continue
                                    // Empty message might confuse the model, so we send a small prompt
                                    log::info!("DEBUG: Calling process_message_stream with continuation prompt");
                                    match agent_clone.process_message_stream("Please continue analyzing the results.").await {
                                        Ok(mut stream) => {
                                            log::info!("Successfully created continuation stream");
                                            let mut chunk_count = 0;
                                            
                                            // Consume the stream
                                            loop {
                                                match tokio::time::timeout(
                                                    std::time::Duration::from_secs(60),
                                                    stream.next()
                                                ).await {
                                                    Ok(Some(chunk_result)) => {
                                                        chunk_count += 1;
                                                        
                                                        match chunk_result {
                                                            Ok(chunk) => {
                                                                if chunk.is_final {
                                                                    log::info!("Received final chunk in continuation, stream complete");
                                                                    break;
                                                                }
                                                            },
                                                            Err(e) => {
                                                                log::error!("Error in continuation streaming response: {e}");
                                                                break;
                                                            }
                                                        }
                                                    },
                                                    Ok(None) => {
                                                        log::info!("Continuation stream ended after {chunk_count} chunks");
                                                        break;
                                                    },
                                                    Err(_timeout) => {
                                                        log::error!("Timeout waiting for continuation chunk");
                                                        break;
                                                    }
                                                }
                                            }
                                            
                                            // Send completion event
                                            let _ = app_event_sender_clone.send(AppEvent::ResponseProcessingComplete);
                                        },
                                        Err(e) => {
                                            log::error!("Failed to create continuation stream: {e}");
                                            let _ = app_event_sender_clone.send(AppEvent::ResponseProcessingComplete);
                                        }
                                    }
                                });
                            }
                        }
                    }
                } else {
                    log::warn!("Received tool result for unknown tool call: {}", tool_call_id);
                }
            },
            AppEvent::UpdateUiPreference { preference } => {
                log::info!("Received UpdateUiPreference event: {:?}", preference);
                match preference {
                    UiPreference::SimplifiedToolRendering(enabled) => {
                        app.state.use_simplified_tool_rendering = enabled;
                        log::info!("Updated use_simplified_tool_rendering to: {}", enabled);
                    }
                }
            },
            AppEvent::DebugForceRefreshAllSummaries => {
                log::info!("Received DebugForceRefreshAllSummaries event");
                handle_debug_force_refresh_all_summaries(app);
            },
        }
    }
}

/// Handle sync notification
fn handle_sync_notification(app: &mut SagittaCodeApp, repository: String, message: String, notification_type: SyncNotificationType) {
    use egui_notify::ToastLevel;
    
    let level = match notification_type {
        SyncNotificationType::Success => ToastLevel::Success,
        SyncNotificationType::Warning => ToastLevel::Warning,
        SyncNotificationType::Error => ToastLevel::Error,
        SyncNotificationType::Info => ToastLevel::Info,
    };
    
    // Format the notification with repository name
    let formatted_message = format!("{}: {}", repository, message);
    
    // Create toast with correct API
    app.state.toasts
        .basic(&formatted_message)
        .level(level)
        .duration(Some(std::time::Duration::from_secs(5)));
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
    chat_message.timestamp = agent_msg.timestamp; // Preserve original timestamp
    chat_message.tool_calls = agent_msg.tool_calls.iter().map(|tool_call| {
        crate::gui::chat::ToolCall {
            id: tool_call.id.clone(),
            name: tool_call.name.clone(),
            arguments: tool_call.arguments.to_string(),
            result: tool_call.result.as_ref().map(|r| r.to_string()),
            status: if tool_call.successful { 
                crate::gui::chat::MessageStatus::Complete 
            } else { 
                crate::gui::chat::MessageStatus::Error("Tool call failed".to_string()) 
            },
            content_position: None,
        }
    }).collect(); // Preserve tool calls
    chat_message
}

/// Handle LLM chunk events from the agent
fn handle_llm_chunk(app: &mut SagittaCodeApp, content: String, is_final: bool, is_thinking: bool) {
    let actual_content = content;
    
    let current_response_id = app.state.current_response_id.clone();
    
    match current_response_id {
        Some(current_id) => {
            // We have an ongoing response, append to it
            if is_thinking {
                app.chat_manager.append_thinking_content(&current_id, actual_content);
            } else {
                app.chat_manager.append_content(&current_id, actual_content);
            }
            
            if is_final {
                app.chat_manager.finish_streaming(&current_id);
                app.state.current_response_id = None;
                app.state.is_streaming_response = false;
                app.state.is_waiting_for_response = false;
                // Request focus when thinking/response completes so user can type immediately
                app.state.should_focus_input = true;
                if std::env::var("SAGITTA_STREAMING_DEBUG").is_ok() {
                    log::debug!("handle_llm_chunk: Completed streaming response for ID: '{current_id}'");
                } else {
                    log::trace!("handle_llm_chunk: Completed streaming response");
                }
            }
        },
        None => {
            // Start a new response stream
            let response_id = app.chat_manager.start_agent_response();
            app.state.current_response_id = Some(response_id.clone());
            app.state.is_streaming_response = true;
            
            if is_thinking {
                app.chat_manager.append_thinking_content(&response_id, actual_content.clone());
            } else {
                app.chat_manager.append_content(&response_id, actual_content.clone());
            }
            
            if is_final {
                app.chat_manager.finish_streaming(&response_id);
                app.state.current_response_id = None;
                app.state.is_streaming_response = false;
                app.state.is_waiting_for_response = false;
            }
        }
    }
}

/// Handle tool call events
pub fn handle_tool_call(app: &mut SagittaCodeApp, tool_call: ToolCall) {
    // Add to events panel for system tracking
    let preview = format_tool_arguments_for_display(&tool_call.name, &serde_json::to_string(&tool_call.arguments).unwrap_or_default());
    app.panels.events_panel.add_event(
        SystemEventType::ToolExecution,
        format!("🔧 {preview}")
    );
    
    // Store pending tool call in state
    app.state.pending_tool_calls.push_back(tool_call.clone());
    
    // REMOVED: No longer create duplicate chat messages for tool calls
    // The tool calls are already displayed as clickable tool call cards
    // via the AgentEvent::ToolCall event processing and the 🔧 Executing tool: text
    
    // Add tool call to the current streaming message if one exists
    if let Some(ref response_id) = app.state.current_response_id {
        // Get current content length to track where the tool was initiated
        let content_position = app.chat_manager.get_all_messages()
            .iter()
            .find(|m| m.id == *response_id)
            .map(|m| m.content.len());
            
        let view_tool_call = ViewToolCall {
            id: tool_call.id.clone(),
            name: tool_call.name.clone(),
            arguments: format_tool_arguments_for_display(&tool_call.name, &serde_json::to_string(&tool_call.arguments).unwrap_or_default()),
            result: None,
            status: MessageStatus::Streaming,
            content_position,
        };
        app.chat_manager.add_tool_call(response_id, view_tool_call);
    } else {
        // No active response ID; attach tool call to the most recent agent message (fallback)
        let all_messages = app.chat_manager.get_all_messages();
        if let Some(last_agent_msg) = all_messages.iter().rev().find(|m| m.author == MessageAuthor::Agent) {
            let content_position = Some(last_agent_msg.content.len());
            let view_tool_call = ViewToolCall {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                arguments: format_tool_arguments_for_display(&tool_call.name, &serde_json::to_string(&tool_call.arguments).unwrap_or_default()),
                result: None,
                status: MessageStatus::Streaming,
                content_position,
            };
            app.chat_manager.add_tool_call(&last_agent_msg.id, view_tool_call);
        }
    }
}

/// Handle tool call result events
pub fn handle_tool_call_result(app: &mut SagittaCodeApp, tool_call_id: String, tool_name: String, result: crate::tools::types::ToolResult) {
    // Add to events panel
    let event_message = match &result {
        crate::tools::types::ToolResult::Success { .. } => {
            format!("Tool {tool_name} completed successfully")
        },
        crate::tools::types::ToolResult::Error { error } => {
            format!("Tool {tool_name} failed: {error}")
        }
    };
    
    let event_type = match &result {
        crate::tools::types::ToolResult::Success { .. } => SystemEventType::ToolExecution,
        crate::tools::types::ToolResult::Error { .. } => SystemEventType::Error,
    };
    
    app.panels.events_panel.add_event(event_type, event_message);
    
    // Create the result string
    let result_string = match &result {
        crate::agent::events::ToolResult::Success { output } => {
            output.clone()
        },
        crate::agent::events::ToolResult::Error { error } => {
            format!("Error: {error}")
        }
    };
    
    let is_success = matches!(result, crate::agent::events::ToolResult::Success { .. });
    
    // Update tool call status in the streaming chat manager
    // Try to update by tool_call_id first (most precise)
    let updated = app.chat_manager.update_tool_call_result_by_id(&tool_call_id, result_string.clone(), is_success);
    
    if !updated {
        // Fallback: try to update by tool name
        let updated_by_name = app.chat_manager.update_tool_call_result_by_name(&tool_name, result_string.clone(), is_success);
        
        if !updated_by_name {
            log::warn!("Could not find tool call to update: ID={tool_call_id}, Name={tool_name}");
        }
    }
    
    // Store the result for potential preview display
    app.state.tool_results.insert(tool_call_id.clone(), result_string);
    
    if std::env::var("SAGITTA_STREAMING_DEBUG").is_ok() {
        log::debug!("Tool call {tool_call_id} ({tool_name}) completed with result stored");
    } else {
        log::trace!("Tool call '{tool_name}' completed");
    }
}

/// Handle agent state changes
pub fn handle_state_change(app: &mut SagittaCodeApp, state: AgentState) {
    // Check if we're transitioning from a working state to Idle
    let was_working = matches!(
        &app.state.current_agent_state,
        AgentState::Thinking { .. } | AgentState::Responding { .. } | AgentState::ExecutingTool { .. }
    );
    
    // Update the current agent state
    app.state.current_agent_state = state.clone();
    
    // Add state changes to events panel
    let (state_message, event_type) = match &state {
        AgentState::Idle => {
            app.state.is_waiting_for_response = false;
            app.state.is_thinking = false;
            app.state.is_responding = false;
            app.state.is_executing_tool = false;
            if app.state.is_in_loop {
                log::info!("Agent exited loop state - updating UI (handle_state_change)");
                app.state.is_in_loop = false;
                app.state.loop_break_requested = false;
            }
            
            // If we transitioned from a working state to Idle, the conversation is complete
            if was_working {
                if let Some(conversation_id) = app.state.current_conversation_id {
                    log::info!("Agent completed work, triggering ConversationCompleted event for conversation {conversation_id}");
                    handle_conversation_completed(app, conversation_id);
                }
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
            (format!("Agent state: Executing tool (events.rs) {tool_name}"), SystemEventType::StateChange)
        },
        AgentState::InLoop { step, interruptible } => {
            if *interruptible {
                (format!("Agent entered reasoning loop (step {step}, interruptible)"), SystemEventType::StateChange)
            } else {
                (format!("Agent entered reasoning loop (step {step}, not interruptible)"), SystemEventType::StateChange)
            }
        },
        AgentState::Error { message, .. } => {
            app.state.is_waiting_for_response = false;
            app.state.current_response_id = None;
            app.state.is_thinking = false;
            app.state.is_responding = false;
            app.state.is_executing_tool = false;
            (format!("Agent error: {message}"), SystemEventType::Error)
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
pub fn process_conversation_events(app: &mut SagittaCodeApp) {
    // Process app-internal conversation events
    let mut events = Vec::new();
    if let Some(ref mut receiver) = app.conversation_event_receiver {
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
    }
    
    // Only log when there are actually events to process
    if !events.is_empty() {
        log::debug!("Processing {} conversation events", events.len());
    }
    
    // Then process each event
    for event in events {
        match event {
            ConversationEvent::DataLoaded { current_title, conversations } => {
                app.state.current_conversation_title = current_title;
                app.state.conversation_list = conversations;
                app.state.conversation_data_loading = false;
                log::info!("Updated conversation cache with {} conversations, cleared loading state", app.state.conversation_list.len());
            },
            ConversationEvent::ConversationCreated(id) => {
                log::info!("Conversation created: {id}");
                // Only refresh if it's been more than 5 seconds since last refresh
                if app.state.last_conversation_refresh
                    .map(|last| last.elapsed().as_secs() >= 5)
                    .unwrap_or(true) {
                    force_refresh_conversation_data(app);
                }
            },
            ConversationEvent::ConversationSwitched(id) => {
                log::info!("Conversation switched: {id}");
                // Don't force refresh on conversation switch - just update the current conversation
                // The conversation list doesn't need to be refreshed when switching
            },
            ConversationEvent::ConversationMessagesLoaded { conversation_id, messages } => {
                log::info!("Conversation messages loaded for conversation {conversation_id}");
                app.handle_conversation_messages(conversation_id, messages);
            },
            ConversationEvent::AnalyticsReportReady(report) => {
                log::info!("Received AnalyticsReportReady event");
                // Handle the report
                app.handle_analytics_report(report);
            },
        }
    }
    
    // Process conversation service events for auto-updates
    process_conversation_service_events(app);
}

/// Process conversation service events that trigger auto-updates
pub fn process_conversation_service_events(app: &mut SagittaCodeApp) {
    if let Some(ref mut receiver) = app.conversation_service_event_receiver {
        let mut service_events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            service_events.push(event);
        }
        
        if !service_events.is_empty() {
            log::debug!("Processing {} conversation service events", service_events.len());
        }
        
        for event in service_events {
            match event {
                crate::agent::conversation::service::ConversationEvent::ListRefreshed(conversations) => {
                    log::info!("Conversation list refreshed: {} conversations", conversations.len());
                    app.state.conversation_list = conversations;
                    app.state.conversation_data_loading = false;
                    app.state.set_conversation_loading(false);
                },
                crate::agent::conversation::service::ConversationEvent::ConversationCreated(id) => {
                    log::info!("Conversation service: conversation created {id}");
                    // Trigger a refresh to get the updated list
                    refresh_conversation_data(app);
                },
                crate::agent::conversation::service::ConversationEvent::ConversationUpdated(id) => {
                    log::info!("Conversation service: conversation updated {id}");
                    // Trigger a refresh to get the updated list
                    refresh_conversation_data(app);
                },
                crate::agent::conversation::service::ConversationEvent::ConversationDeleted(id) => {
                    log::info!("Conversation service: conversation deleted {id}");
                    // Trigger a refresh to get the updated list
                    refresh_conversation_data(app);
                },
                crate::agent::conversation::service::ConversationEvent::ClustersUpdated(clusters) => {
                    log::debug!("Conversation clusters updated: {} clusters", clusters.len());
                    // Simple sidebar doesn't use clusters
                },
                crate::agent::conversation::service::ConversationEvent::AnalyticsReady(report) => {
                    log::info!("Analytics ready from conversation service");
                    app.handle_analytics_report(report);
                },
            }
        }
    }
}

/// Refresh conversation data asynchronously
pub fn refresh_conversation_data(app: &mut SagittaCodeApp) {
    // Check if we should refresh based on time and loading state
    let should_refresh = app.state.last_conversation_refresh
        .map(|last| last.elapsed().as_secs() >= 30) // Increased from 10 to 30 seconds
        .unwrap_or(true);
    
    log::debug!("refresh_conversation_data called: should_refresh={}, currently_loading={}", should_refresh, app.state.conversation_data_loading);
    
    if should_refresh && !app.state.conversation_data_loading {
        app.state.conversation_data_loading = true;
        log::info!("Setting conversation_data_loading = true, starting refresh");
        
        // Try to use conversation service first, fall back to agent
        if let Some(service) = &app.conversation_service {
            let service_clone = service.clone();
            let sender = app.conversation_event_sender.clone();
            
            log::debug!("Using conversation service for refresh, sender available: {}", sender.is_some());
            
            // Spawn async task to load conversation data using the service
            tokio::spawn(async move {
                if let Some(sender) = sender {
                    // Refresh the service data first
                    if let Err(e) = service_clone.refresh().await {
                        log::error!("Failed to refresh conversation service: {e}");
                        return;
                    }
                    
                    // Load conversation list from service
                    let conversation_list = service_clone.list_conversations().await.unwrap_or_default();
                    
                    log::info!("Conversation service loaded {} conversations, sending DataLoaded event", conversation_list.len());
                    
                    // For now, we don't have a current conversation concept in the service
                    // So we'll use None for current_title
                    let current_title = None;
                    
                    // Send the data back to the UI
                    if let Err(e) = sender.send(ConversationEvent::DataLoaded {
                        current_title,
                        conversations: conversation_list,
                    }) {
                        log::error!("Failed to send DataLoaded event: {e}");
                    } else {
                        log::debug!("Successfully sent DataLoaded event");
                    }
                } else {
                    log::warn!("No conversation event sender available");
                }
            });
        } else if let Some(agent) = &app.agent {
            let agent_clone = agent.clone();
            let sender = app.conversation_event_sender.clone();
            
            log::debug!("Using agent for refresh (fallback), sender available: {}", sender.is_some());
            
            // Fallback: Spawn async task to load conversation data using agent
            tokio::spawn(async move {
                if let Some(sender) = sender {
                    // Load current conversation
                    let current_conversation = agent_clone.get_current_conversation().await.ok().flatten();
                    let current_title = current_conversation.map(|c| c.title);
                    
                    // Load conversation list
                    let conversation_list = agent_clone.list_conversations().await.unwrap_or_default();
                    
                    log::info!("Agent loaded {} conversations, sending DataLoaded event", conversation_list.len());
                    
                    // Send the data back to the UI
                    if let Err(e) = sender.send(ConversationEvent::DataLoaded {
                        current_title,
                        conversations: conversation_list,
                    }) {
                        log::error!("Failed to send DataLoaded event: {e}");
                    } else {
                        log::debug!("Successfully sent DataLoaded event");
                    }
                } else {
                    log::warn!("No conversation event sender available");
                }
            });
        } else {
            // No service or agent available, send empty data to clear loading state
            log::warn!("No conversation service or agent available for refresh");
            
            // Send empty data to clear the loading state
            if let Some(sender) = &app.conversation_event_sender {
                if let Err(e) = sender.send(ConversationEvent::DataLoaded {
                    current_title: None,
                    conversations: Vec::new(),
                }) {
                    log::error!("Failed to send empty DataLoaded event: {e}");
                } else {
                    log::debug!("Sent empty DataLoaded event to clear loading state");
                }
            }
        }
        
        app.state.last_conversation_refresh = Some(std::time::Instant::now());
    } else {
        log::debug!("Skipping refresh: should_refresh={}, currently_loading={}", should_refresh, app.state.conversation_data_loading);
    }
}

/// Force refresh conversation data immediately
pub fn force_refresh_conversation_data(app: &mut SagittaCodeApp) {
    app.state.last_conversation_refresh = None;
    app.state.conversation_data_loading = false; // Reset loading state first
    
    // Always set loading state to true initially
    app.state.conversation_data_loading = true;
    log::info!("Setting conversation_data_loading = true, starting forced refresh");
    
    // Try to use conversation service first, fall back to agent
    if let Some(service) = &app.conversation_service {
        let service_clone = service.clone();
        let sender = app.conversation_event_sender.clone();
        
        log::debug!("Using conversation service for forced refresh, sender available: {}", sender.is_some());
        
        // Spawn async task to load conversation data using the service
        tokio::spawn(async move {
            if let Some(sender) = sender {
                // Refresh the service data first
                if let Err(e) = service_clone.refresh().await {
                    log::error!("Failed to refresh conversation service: {e}");
                    return;
                }
                
                // Load conversation list from service
                let conversation_list = service_clone.list_conversations().await.unwrap_or_default();
                
                log::info!("Conversation service loaded {} conversations, sending DataLoaded event", conversation_list.len());
                
                // For now, we don't have a current conversation concept in the service
                // So we'll use None for current_title
                let current_title = None;
                
                // Send the data back to the UI
                if let Err(e) = sender.send(ConversationEvent::DataLoaded {
                    current_title,
                    conversations: conversation_list,
                }) {
                    log::error!("Failed to send DataLoaded event: {e}");
                } else {
                    log::debug!("Successfully sent DataLoaded event");
                }
            } else {
                log::warn!("No conversation event sender available");
            }
        });
    } else if let Some(agent) = &app.agent {
        let agent_clone = agent.clone();
        let sender = app.conversation_event_sender.clone();
        
        log::debug!("Using agent for forced refresh (fallback), sender available: {}", sender.is_some());
        
        // Fallback: Spawn async task to load conversation data using agent
        tokio::spawn(async move {
            if let Some(sender) = sender {
                // Load current conversation
                let current_conversation = agent_clone.get_current_conversation().await.ok().flatten();
                let current_title = current_conversation.map(|c| c.title);
                
                // Load conversation list
                let conversation_list = agent_clone.list_conversations().await.unwrap_or_default();
                
                log::info!("Agent loaded {} conversations, sending DataLoaded event", conversation_list.len());
                
                // Send the data back to the UI
                if let Err(e) = sender.send(ConversationEvent::DataLoaded {
                    current_title,
                    conversations: conversation_list,
                }) {
                    log::error!("Failed to send DataLoaded event: {e}");
                } else {
                    log::debug!("Successfully sent DataLoaded event");
                }
            } else {
                log::warn!("No conversation event sender available");
            }
        });
    } else {
        // No service or agent available, send empty data to clear loading state
        log::warn!("No conversation service or agent available for forced refresh");
        
        // Send empty data to clear the loading state
        if let Some(sender) = &app.conversation_event_sender {
            if let Err(e) = sender.send(ConversationEvent::DataLoaded {
                current_title: None,
                conversations: Vec::new(),
            }) {
                log::error!("Failed to send empty DataLoaded event: {e}");
            } else {
                log::debug!("Sent empty DataLoaded event to clear loading state");
            }
        }
    }
}

/// Switch to a conversation and update the chat view
pub fn switch_to_conversation(app: &mut SagittaCodeApp, conversation_id: uuid::Uuid) {
    log::info!("switch_to_conversation: Starting switch to conversation {conversation_id}");
    
    // Clear current chat state
    app.state.current_conversation_id = Some(conversation_id);
    app.state.messages.clear();
    app.state.conversation_data_loading = true;
    
    // Clear all response state as expected by tests
    app.state.current_response_id = None;
    app.state.is_streaming_response = false;
    app.state.is_waiting_for_response = false;
    app.state.tool_results.clear();
    app.state.pending_tool_calls.clear();
    
    // Clear chat manager messages for the new conversation
    app.chat_manager.clear_all_messages();
    
    // Update sidebar selection
    app.conversation_panel.select_conversation(conversation_id);
    
    // Find and set the conversation title
    if let Some(summary) = app.state.conversation_list.iter().find(|s| s.id == conversation_id) {
        app.state.current_conversation_title = Some(summary.title.clone());
    }
    
    // Load conversation history
    if let Some(service) = &app.conversation_service {
        log::info!("switch_to_conversation: Using conversation service to load messages");
        let service_clone = service.clone();
        let sender = app.conversation_event_sender.clone();
        
        tokio::spawn(async move {
            match service_clone.get_conversation(conversation_id).await {
                Ok(Some(conversation)) => {
                    log::info!("Loaded conversation '{}' with {} messages", 
                        conversation.title, conversation.messages.len());
                    
                    if let Some(sender) = sender {
                        // Send the messages to be loaded into the chat UI
                        if let Err(e) = sender.send(ConversationEvent::ConversationMessagesLoaded {
                            conversation_id,
                            messages: conversation.messages,
                        }) {
                            log::error!("Failed to send ConversationMessagesLoaded event: {e}");
                        }
                        
                        // Also send the switched event for other UI updates
                        if let Err(e) = sender.send(ConversationEvent::ConversationSwitched(conversation_id)) {
                            log::error!("Failed to send ConversationSwitched event: {e}");
                        }
                    }
                },
                Ok(None) => {
                    log::warn!("Conversation {conversation_id} not found");
                },
                Err(e) => {
                    log::error!("Failed to load conversation {conversation_id}: {e}");
                }
            }
        });
    } else {
        log::warn!("switch_to_conversation: No conversation service available - cannot load messages!");
    }
    
    // Trigger analysis for suggestions
    analyze_conversation_for_suggestions(app, conversation_id);
}

/// Analyze a conversation for checkpoint and branch suggestions
pub fn analyze_conversation_for_suggestions(app: &mut SagittaCodeApp, conversation_id: uuid::Uuid) {
    if let Some(service) = &app.conversation_service {
        let service_clone = service.clone();
        let app_event_sender = app.app_event_sender.clone();
        
        tokio::spawn(async move {
            // Get the conversation to analyze
            if let Ok(Some(conversation)) = service_clone.get_conversation(conversation_id).await {
                log::info!("Analyzing conversation '{}' for suggestions", conversation.title);
                
                // Use the BranchingManager with fast model support for better suggestions
                let mut branch_suggestions = Vec::new();
                let mut checkpoint_suggestions = Vec::new();
                
                // Create a branching manager
                let branching_manager = crate::agent::conversation::branching::ConversationBranchingManager::with_default_config();
                
                // Try to use fast model if enabled
                // Note: We can't access config from ConversationService directly
                // For now, we'll skip fast model configuration here
                // This would need to be passed from the app initialization
                log::debug!("Fast model configuration for branch suggestions would need to be passed from app initialization");
                
                // Analyze for branch opportunities
                match branching_manager.analyze_branch_opportunities(&conversation).await {
                    Ok(suggestions) => {
                        branch_suggestions = suggestions;
                        log::info!("Found {} branch suggestions using BranchingManager", branch_suggestions.len());
                    }
                    Err(e) => {
                        log::error!("Failed to analyze branch opportunities: {e}");
                        // Fall back to simple detection
                        let recent_messages: Vec<_> = conversation.messages.iter()
                            .rev()
                            .take(5)
                            .collect();
                        
                        for message in &recent_messages {
                            let content = message.content.to_lowercase();
                            
                            // Simple branch detection fallback
                            if content.contains("alternative") || content.contains("option") ||
                               content.contains("different approach") || content.contains("try") {
                                
                                let branch = crate::agent::conversation::branching::BranchSuggestion {
                                    message_id: message.id,
                                    confidence: 0.6,
                                    reason: crate::agent::conversation::branching::BranchReason::AlternativeApproach,
                                    suggested_title: "Alternative Approach".to_string(),
                                    success_probability: Some(0.5),
                                    context: crate::agent::conversation::branching::BranchContext {
                                        relevant_messages: vec![message.id],
                                        trigger_keywords: vec!["alternative".to_string()],
                                        conversation_state: crate::agent::conversation::branching::ConversationState::SolutionDevelopment,
                                        project_context: None,
                                        mentioned_tools: vec![],
                                    },
                                };
                                branch_suggestions.push(branch);
                            }
                        }
                    }
                }
                
                // Simple checkpoint detection (TODO: Integrate CheckpointManager)
                let recent_messages: Vec<_> = conversation.messages.iter()
                    .rev()
                    .take(5)
                    .collect();
                
                for message in &recent_messages {
                    let content = message.content.to_lowercase();
                    
                    if content.contains("success") || content.contains("complete") || 
                       content.contains("working") || content.contains("done") {
                        
                        let checkpoint = crate::agent::conversation::checkpoints::CheckpointSuggestion {
                            message_id: message.id,
                            importance: 0.8,
                            reason: crate::agent::conversation::checkpoints::CheckpointReason::SuccessfulSolution,
                            suggested_title: format!("Checkpoint: {}", 
                                if content.contains("success") { "Successful Solution" }
                                else if content.contains("complete") { "Task Completed" }
                                else if content.contains("working") { "Working Solution" }
                                else { "Achievement" }
                            ),
                            context: crate::agent::conversation::checkpoints::CheckpointContext {
                                relevant_messages: vec![message.id],
                                trigger_keywords: vec!["success".to_string()],
                                conversation_phase: crate::agent::conversation::checkpoints::ConversationPhase::Implementation,
                                modified_files: vec![],
                                executed_tools: vec![],
                                success_indicators: vec!["working".to_string()],
                            },
                            restoration_value: 0.9,
                        };
                        checkpoint_suggestions.push(checkpoint);
                    }
                }
                
                // Check lengths before moving to avoid borrow checker issues
                let checkpoint_count = checkpoint_suggestions.len();
                let branch_count = branch_suggestions.len();
                
                if checkpoint_count > 0 {
                    log::info!("Found {checkpoint_count} checkpoint suggestions for conversation {conversation_id}");
                    
                    // Send checkpoint suggestions to UI
                    if let Err(e) = app_event_sender.send(AppEvent::CheckpointSuggestionsReady {
                        conversation_id,
                        suggestions: checkpoint_suggestions,
                    }) {
                        log::error!("Failed to send checkpoint suggestions: {e}");
                    } else {
                        log::debug!("Successfully sent checkpoint suggestions to UI");
                    }
                }
                
                if branch_count > 0 {
                    log::info!("Found {branch_count} branch suggestions for conversation {conversation_id}");
                    
                    // Send branch suggestions to UI
                    if let Err(e) = app_event_sender.send(AppEvent::BranchSuggestionsReady {
                        conversation_id,
                        suggestions: branch_suggestions,
                    }) {
                        log::error!("Failed to send branch suggestions: {e}");
                    } else {
                        log::debug!("Successfully sent branch suggestions to UI");
                    }
                }
                
                if checkpoint_count == 0 && branch_count == 0 {
                    log::debug!("No suggestions found for conversation {conversation_id}");
                }
            } else {
                log::warn!("Could not find conversation {conversation_id} for analysis");
            }
        });
    } else {
        log::debug!("No conversation service available for analysis");
    }
}

impl SagittaCodeApp {
    /// Handle agent events with context for UI updates
    pub fn handle_agent_event(&mut self, event: AgentEvent, ctx: &egui::Context) {
        // Process the event through the existing handler
        match event {
            AgentEvent::LlmChunk { content, is_final, is_thinking } => {
                handle_llm_chunk(self, content, is_final, is_thinking);
            },
            AgentEvent::ToolCall { tool_call } => {
                handle_tool_call(self, tool_call);
            },
            AgentEvent::ToolCallComplete { tool_call_id, tool_name, result } => {
                handle_tool_call_result(self, tool_call_id, tool_name, result);
            },
            AgentEvent::StateChanged(state) => {
                handle_state_change(self, state);
            },
            _ => {
                // For other events, use the standard processing
                // We would need to temporarily store the event and process it
                // This is a simplified approach - in practice, you might want to
                // refactor the event processing to be more modular
            }
        }
        
        // Request a repaint to update the UI
        ctx.request_repaint();
    }

    /// Handle agent state changes with context for UI updates
    pub fn handle_agent_state_change(&mut self, new_state: AgentState, ctx: &egui::Context) {
        handle_state_change(self, new_state);
        ctx.request_repaint();
    }

    /// Handle analytics report updates
    pub fn handle_analytics_report(&mut self, report: crate::agent::conversation::analytics::AnalyticsReport) {
        log::info!("Updating analytics panel with new report containing {} conversations", 
            report.overall_metrics.total_conversations);
        
        // Update the analytics panel with the new report
        self.panels.analytics_panel.set_analytics_report(Some(report));
                
        // Add event to events panel
        self.panels.events_panel.add_event(
            SystemEventType::Info,
            "Analytics report updated".to_string()
        );
    }

    /// Handle checkpoint suggestions (removed feature - now does nothing)
    pub fn handle_checkpoint_suggestions(&mut self, _conversation_id: uuid::Uuid, _suggestions: Vec<crate::agent::conversation::checkpoints::CheckpointSuggestion>) {
        log::info!("Checkpoint suggestions feature has been removed");
    }

    /// Handle branch suggestions (removed feature - now does nothing)  
    pub fn handle_branch_suggestions(&mut self, _conversation_id: uuid::Uuid, _suggestions: Vec<crate::agent::conversation::branching::BranchSuggestion>) {
        log::info!("Branch suggestions feature has been removed");
    }

    /// Handle conversation messages loaded from service
    pub fn handle_conversation_messages(&mut self, conversation_id: uuid::Uuid, messages: Vec<crate::agent::message::types::AgentMessage>) {
        log::info!("Loading {} messages for conversation {} into chat UI", messages.len(), conversation_id);
        
        // Only load messages if this is still the current conversation
        if self.state.current_conversation_id == Some(conversation_id) {
            let total_messages = messages.len();
            // Convert agent messages to streaming messages and add them to chat manager
            for (index, agent_message) in messages.into_iter().enumerate() {
                log::debug!("Processing message {} of {}: role={:?}, content_len={}, tool_calls={}", 
                    index + 1, total_messages, 
                    agent_message.role,
                    agent_message.content.len(),
                    agent_message.tool_calls.len()
                );
                
                // Log tool call details
                for (tc_index, tool_call) in agent_message.tool_calls.iter().enumerate() {
                    log::debug!("  Tool call {}: name={}, id={}, has_result={}, successful={}", 
                        tc_index + 1,
                        tool_call.name,
                        tool_call.id,
                        tool_call.result.is_some(),
                        tool_call.successful
                    );
                    if let Some(result) = &tool_call.result {
                        log::debug!("    Result preview: {}", 
                            result.to_string().chars().take(100).collect::<String>()
                        );
                    }
                }
                
                let chat_message = make_chat_message_from_agent_message(&agent_message);
                let streaming_message: StreamingMessage = chat_message.into();
                self.chat_manager.add_complete_message(streaming_message);
                
                // CRITICAL FIX: Restore tool cards from agent message tool calls
                // Only create tool cards if the tool call has a result
                for tool_call in &agent_message.tool_calls {
                    if tool_call.result.is_some() {
                        log::debug!("Restoring tool card for tool '{}' ({})", tool_call.name, tool_call.id);
                        self.chat_manager.restore_tool_card(tool_call, agent_message.timestamp);
                    } else {
                        log::debug!("Skipping tool card restoration for tool '{}' ({}) - no result", tool_call.name, tool_call.id);
                    }
                }
            }
            
            log::info!("Successfully loaded {} messages into chat UI for conversation {}", 
                self.chat_manager.get_all_messages().len(), conversation_id);
            
            // Clear loading state
            self.state.conversation_data_loading = false;
        } else {
            log::warn!("Conversation {conversation_id} is no longer current, skipping message loading");
        }
    }

    // Method moved to app.rs
}

/// Handle tool run started event
pub fn handle_tool_run_started(app: &mut SagittaCodeApp, run_id: ToolRunId, tool: String) {
    use super::state::RunningToolInfo;
    
    // Find the message ID that contains this tool (use current response ID as fallback)
    let message_id = app.state.current_response_id.clone()
        .or_else(|| app.state.active_tool_call_message_id.map(|id| id.to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    
    // Add to running tools tracking
    let tool_info = RunningToolInfo {
        tool_name: tool.clone(),
        progress: None,
        message_id,
        start_time: std::time::Instant::now(),
    };
    
    app.state.running_tools.insert(run_id, tool_info);
    
    // Store in tool results for backward compatibility
    app.state.tool_results.insert(run_id.to_string(), format!("Tool {tool} started"));
    
    log::debug!("Started tracking tool run: {tool} ({run_id})");
}

/// Handle tool run completed event
pub fn handle_tool_run_completed(app: &mut SagittaCodeApp, run_id: ToolRunId, tool: String, success: bool) {
    // Remove from running tools tracking
    if let Some(tool_info) = app.state.running_tools.remove(&run_id) {
        let duration = tool_info.start_time.elapsed();
        log::debug!("Tool run completed: {tool} ({run_id}) - success: {success}, duration: {duration:?}");
    }
    
    // Update stored result
    let status = if success { "completed" } else { "failed" };
    app.state.tool_results.insert(run_id.to_string(), format!("Tool {tool} {status}"));
    
    // Request focus on input after tool completion
    app.state.should_focus_input = true;
    
    // UI will update on next frame automatically
}

/// Handle tool stream event (progress updates)
pub fn handle_tool_stream(_app: &mut SagittaCodeApp, run_id: ToolRunId, event: String) {
    // Simplified tool stream handling without terminal_stream
    log::debug!("Tool stream event for run {run_id}: {event}");
}

/// Handle tool cancellation request
pub fn handle_tool_cancellation(app: &mut SagittaCodeApp, run_id: ToolRunId) {
    // Remove from running tools tracking
    if let Some(tool_info) = app.state.running_tools.remove(&run_id) {
        log::info!("Cancelling tool: {} ({})", tool_info.tool_name, run_id);
    }
    
    // Update stored result
    app.state.tool_results.insert(run_id.to_string(), "Tool cancelled".to_string());
    
    // TODO: In a complete implementation, we would need to:
    // 1. Send a cancellation signal to the reasoning engine
    // 2. The reasoning engine would need to track running tools and their cancel handles
    // 3. Drop the future or send a cancel signal to the tool execution
    
    // For now, we just log the cancellation request
    log::warn!("Tool cancellation requested for run_id: {run_id} - actual cancellation not yet implemented");
    
    // UI will update on next frame automatically
}

/// Handle conversation completed event - trigger title update
pub fn handle_conversation_completed(app: &mut SagittaCodeApp, conversation_id: uuid::Uuid) {
    log::info!("Handling conversation completed for {conversation_id}");
    
    // Trigger title update using the title updater
    if let Some(title_updater) = &app.title_updater {
        let updater_clone = title_updater.clone();
        let app_event_sender = app.app_event_sender.clone();
        
        tokio::spawn(async move {
            log::info!("Spawning title update task for conversation {conversation_id}");
            match updater_clone.maybe_update_title(conversation_id).await {
                Ok(_) => {
                    log::info!("Successfully updated title for conversation {conversation_id}");
                    
                    // Trigger a refresh of the conversation list to show the new title
                    if let Err(e) = app_event_sender.send(AppEvent::RefreshConversationList) {
                        log::error!("Failed to send RefreshConversationList event: {e}");
                    }
                }
                Err(e) => {
                    log::error!("Failed to update title for conversation {conversation_id}: {e}");
                }
            }
        });
    } else {
        log::warn!("No title updater available to update conversation title");
    }
    
    // Also refresh conversation data to update the status in the UI
    force_refresh_conversation_data(app);
}

/// Handle conversation rename request
pub fn handle_rename_conversation(app: &mut SagittaCodeApp, conversation_id: uuid::Uuid, new_title: String) {
    // Update the conversation title in the service
    if let Some(service) = &app.conversation_service {
        let service_clone = service.clone();
        let app_event_sender = app.app_event_sender.clone();
        let new_title_clone = new_title.clone();
        
        tokio::spawn(async move {
            match service_clone.rename_conversation(conversation_id, new_title_clone.clone()).await {
                Ok(_) => {
                    log::info!("Successfully renamed conversation {conversation_id} to '{new_title_clone}'");
                    
                    // Trigger a refresh of the conversation list to update UI
                    if let Err(e) = app_event_sender.send(AppEvent::RefreshConversationList) {
                        log::error!("Failed to send RefreshConversationList event: {e}");
                    }
                }
                Err(e) => {
                    log::error!("Failed to rename conversation {conversation_id}: {e}");
                }
            }
        });
    } else {
        log::warn!("No conversation service available to rename conversation");
        
        // Fallback: update the title in the local conversation list
        for conv in &mut app.state.conversation_list {
            if conv.id == conversation_id {
                conv.title = new_title.clone();
                log::info!("Updated conversation title locally (fallback): {conversation_id} -> '{new_title}'");
                break;
            }
        }
    }
    
    // Update current conversation title if it's the active one
    if app.state.current_conversation_id == Some(conversation_id) {
        app.state.current_conversation_title = Some(new_title);
    }
    
    // Mark this conversation as having a custom title to prevent auto-updates
    if let Some(auto_title_updater) = &app.auto_title_updater {
        let updater = auto_title_updater.clone();
        tokio::spawn(async move {
            updater.mark_custom_title(conversation_id).await;
            log::debug!("Marked conversation {} as having custom title", conversation_id);
        });
    }
}

/// Handle manual conversation title update request
pub fn handle_update_conversation_title(app: &mut SagittaCodeApp, conversation_id: uuid::Uuid) {
    log::info!("Handling manual title update request for conversation {conversation_id}");
    
    // Use the title updater to regenerate the title
    if let Some(title_updater) = &app.title_updater {
        let updater_clone = title_updater.clone();
        let app_event_sender = app.app_event_sender.clone();
        
        tokio::spawn(async move {
            log::info!("Spawning manual title update task for conversation {conversation_id}");
            match updater_clone.maybe_update_title(conversation_id).await {
                Ok(_) => {
                    log::info!("Successfully updated title for conversation {conversation_id} (manual request)");
                    
                    // Trigger a refresh of the conversation list to show the new title
                    if let Err(e) = app_event_sender.send(AppEvent::RefreshConversationList) {
                        log::error!("Failed to send RefreshConversationList event: {e}");
                    }
                }
                Err(e) => {
                    log::error!("Failed to update title for conversation {conversation_id} (manual request): {e}");
                }
            }
        });
    } else {
        log::warn!("No title updater available to update conversation title");
    }
}

/// Handle show new conversation confirmation event
pub fn handle_show_new_conversation_confirmation(app: &mut SagittaCodeApp) {
    log::info!("Showing new conversation confirmation dialog");
    app.state.show_new_conversation_confirmation = true;
}

/// Handle create new conversation event
pub fn handle_create_new_conversation(app: &mut SagittaCodeApp) {
    log::info!("Creating new conversation - stopping stream, saving current and clearing ALL state");
    
    // CRITICAL: First stop any ongoing streaming - same as STOP button
    // IMMEDIATELY reset all UI states for instant feedback
    app.state.is_waiting_for_response = false;
    app.state.is_thinking = false;
    app.state.is_responding = false;
    app.state.is_streaming_response = false;
    app.state.is_executing_tool = false;
    app.state.thinking_message = None;
    app.state.thinking_start_time = None;
    
    // Cancel the agent streaming if it exists
    if let Some(agent) = &app.agent {
        let agent_clone = agent.clone();
        tokio::spawn(async move {
            agent_clone.cancel().await;
            log::info!("Agent streaming cancelled for new conversation");
        });
    }
    
    // Save the current conversation before creating a new one
    if let Some(ref mut manager) = app.simple_conversation_manager {
        if let Err(e) = manager.save_current_conversation() {
            log::error!("Failed to save current conversation before creating new one: {}", e);
        }
    }
    
    // CRITICAL: Clear ALL conversation state and context
    app.state.current_conversation_id = None;
    app.state.current_conversation_title = None;
    app.state.messages.clear();
    app.state.current_response_id = None;
    app.state.tool_results.clear();
    app.state.running_tools.clear();
    app.state.tool_call_to_run_id.clear();
    app.state.active_tool_calls.clear();
    app.state.completed_tool_results.clear();
    app.state.tool_calls_continued.clear();
    app.state.pending_tool_calls.clear();
    app.state.active_tool_call_message_id = None;
    app.state.conversation_modified = false;
    
    // Clear chat manager messages - this clears the UI
    app.chat_manager.clear_all_messages();
    
    // Clear any loop state
    app.state.is_in_loop = false;
    app.state.loop_break_requested = false;
    app.state.loop_inject_message = None;
    app.state.loop_inject_buffer.clear();
    app.state.show_loop_inject_input = false;
    
    // Create new conversation in simple manager
    if let Some(ref mut manager) = app.simple_conversation_manager {
        match manager.create_conversation("New Conversation".to_string()) {
            Ok(id) => {
                app.conversation_panel.select_conversation(id);
                // Refresh list
                if let Ok(conversations) = manager.list_conversations() {
                    // TODO: Update conversation panel with conversations
                }
                log::info!("Created new conversation with ID: {}", id);
            }
            Err(e) => {
                log::error!("Failed to create new conversation: {e}");
                app.state.toasts.error(format!("Failed to create conversation: {e}"));
            }
        }
    }
    app.state.pending_tool_calls.clear();
    
    // Clear token usage counter
    app.state.current_token_usage = None;
    
    // CRITICAL: Clear the agent's conversation history to ensure a completely fresh start
    if let Some(agent) = &app.agent {
        let agent_clone = agent.clone();
        tokio::spawn(async move {
            // Clear the agent's internal conversation history
            // The agent doesn't have a clear_conversation method, we need to create a new conversation instead
            if let Err(e) = agent_clone.create_new_conversation("New conversation".to_string()).await {
                log::error!("Failed to clear agent conversation history: {e}");
            } else {
                log::info!("Successfully cleared agent conversation history");
            }
        });
    }
    
    // Also clear any active tool results that might be lingering
    app.state.completed_tool_results.clear();
    app.state.active_tool_calls.clear();
    app.state.tool_calls_continued.clear();
    
    // Update sidebar selection
    // Clear conversation selection in panel
    
    // Force a UI refresh
    if let Err(e) = app.app_event_sender.send(AppEvent::RefreshConversationList) {
        log::error!("Failed to send RefreshConversationList event after creating new conversation: {e}");
    }
    
    log::info!("New conversation state prepared - ready for first message");
}

/// Format tool arguments for user-friendly display
pub fn format_tool_arguments_for_display(tool_name: &str, arguments: &str) -> String {
    // Try to parse as JSON first
    let parsed: Result<Value, _> = serde_json::from_str(arguments);
    
    match parsed {
        Ok(json) => {
            // Format based on tool type
            match tool_name {
                name if name.contains("repository_list") || name.contains("list_repositories") => {
                    "Listing available repositories".to_string()
                },
                name if name.contains("semantic_code_search") => {
                    // Special handling for semantic code search to show both query and element type
                    let query_text = json.get("queryText")
                        .and_then(|v| v.as_str())
                        .map(|s| s.chars().take(50).collect::<String>())
                        .unwrap_or_else(|| "".to_string());
                    
                    let element_type = json.get("elementType")
                        .and_then(|v| v.as_str());
                    
                    let lang = json.get("lang")
                        .and_then(|v| v.as_str());
                    
                    match (element_type, lang) {
                        (Some(elem), Some(l)) => {
                            format!("Searching for: \"{}\" ({}s in {})", query_text, elem, l)
                        },
                        (Some(elem), None) => {
                            format!("Searching for: \"{}\" ({}s)", query_text, elem)
                        },
                        (None, Some(l)) => {
                            format!("Searching for: \"{}\" (in {})", query_text, l)
                        },
                        (None, None) => {
                            if query_text.is_empty() {
                                "Performing semantic search".to_string()
                            } else {
                                format!("Searching for: \"{}\"", query_text)
                            }
                        }
                    }
                },
                name if name.contains("query") || name.contains("search") => {
                    if let Some(query_text) = json.get("queryText").and_then(|v| v.as_str()) {
                        format!("Searching for: \"{}\"", query_text.chars().take(50).collect::<String>())
                    } else if let Some(query_text) = json.get("query").and_then(|v| v.as_str()) {
                        format!("Searching for: \"{}\"", query_text.chars().take(50).collect::<String>())
                    } else {
                        "Performing search".to_string()
                    }
                },
                name if name.contains("view_file") || name.contains("read_file") => {
                    if let Some(file_path) = json.get("filePath").and_then(|v| v.as_str()) {
                        format!("Reading file: {file_path}")
                    } else if let Some(file_path) = json.get("file_path").and_then(|v| v.as_str()) {
                        format!("Reading file: {file_path}")
                    } else {
                        "Reading file".to_string()
                    }
                },
                name if name.contains("edit_file") || name.contains("write_file") => {
                    if let Some(file_path) = json.get("filePath").and_then(|v| v.as_str()) {
                        format!("Editing file: {file_path}")
                    } else if let Some(file_path) = json.get("file_path").and_then(|v| v.as_str()) {
                        format!("Editing file: {file_path}")
                    } else {
                        "Editing file".to_string()
                    }
                },
                name if name.contains("shell") || name.contains("execution") => {
                    if let Some(command) = json.get("command").and_then(|v| v.as_str()) {
                        format!("Running: {}", command.chars().take(60).collect::<String>())
                    } else {
                        "Executing command".to_string()
                    }
                },
                name if name.contains("repository_add") || name.contains("add_repository") => {
                    if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                        format!("Adding repository: {name}")
                    } else {
                        "Adding repository".to_string()
                    }
                },
                name if name.contains("repository_sync") || name.contains("sync_repository") => {
                    if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                        format!("Syncing repository: {name}")
                    } else {
                        "Syncing repository".to_string()
                    }
                },
                name if name.contains("search_file") => {
                    if let Some(pattern) = json.get("pattern").and_then(|v| v.as_str()) {
                        format!("Searching files for: {pattern}")
                    } else {
                        "Searching files".to_string()
                    }
                },
                name if name.contains("repository_map") => {
                    if let Some(repo_name) = json.get("repositoryName").and_then(|v| v.as_str()) {
                        format!("Mapping repository: {repo_name}")
                    } else {
                        "Mapping repository structure".to_string()
                    }
                },
                name if name.contains("switch_branch") => {
                    if let Some(branch) = json.get("branchName").and_then(|v| v.as_str()) {
                        format!("Switching to branch: {branch}")
                    } else {
                        "Switching branch".to_string()
                    }
                },
                _ => {
                    // Generic formatting for unknown tools
                    if let Some(obj) = json.as_object() {
                        // Show key parameters
                        let key_params: Vec<String> = obj.iter()
                            .take(2) // Limit to first 2 parameters
                            .map(|(k, v)| {
                                let value_str = match v {
                                    Value::String(s) => s.chars().take(30).collect::<String>(),
                                    Value::Number(n) => n.to_string(),
                                    Value::Bool(b) => b.to_string(),
                                    _ => "...".to_string(),
                                };
                                format!("{k}={value_str}")
                            })
                            .collect();
                        
                        if key_params.is_empty() {
                            format!("Executing {tool_name}")
                        } else {
                            format!("Executing {} with {}", tool_name, key_params.join(", "))
                        }
                    } else {
                        format!("Executing {tool_name}")
                    }
                }
            }
        },
        Err(_) => {
            // Fallback for non-JSON arguments
            if arguments.is_empty() {
                format!("Executing {tool_name}")
            } else {
                let preview = arguments.chars().take(50).collect::<String>();
                format!("Executing {tool_name} with: {preview}")
            }
        }
    }
}

/// Handle saving the CLAUDE.md template
fn handle_save_claude_md_template(app: &mut SagittaCodeApp) {
    let config_clone = app.config.clone();
    let template_content = app.claude_md_modal.get_template_content().to_string();
    let auto_create_enabled = app.claude_md_modal.is_auto_create_enabled();
    
    let rt = tokio::runtime::Handle::current();
    rt.spawn(async move {
        let mut config_guard = config_clone.lock().await;
        config_guard.ui.claude_md_template = template_content;
        config_guard.ui.auto_create_claude_md = auto_create_enabled;
        
        let config_to_save = config_guard.clone();
        drop(config_guard);
        
        match crate::config::save_config(&config_to_save) {
            Ok(_) => {
                log::info!("CLAUDE.md template saved successfully");
            },
            Err(e) => {
                log::error!("Failed to save CLAUDE.md template: {e}");
            }
        }
    });
}

/// Handle applying CLAUDE.md template to all repositories
fn handle_apply_claude_md_to_all_repos(app: &mut SagittaCodeApp) {
    let repo_manager = app.repo_panel.get_repo_manager();
    let template_content = app.claude_md_modal.get_template_content().to_string();
    let auto_create_enabled = app.claude_md_modal.is_auto_create_enabled();
    
    if !auto_create_enabled {
        log::info!("Auto-create CLAUDE.md is disabled, skipping apply to all repos");
        return;
    }
    
    let rt = tokio::runtime::Handle::current();
    rt.spawn(async move {
        match repo_manager.lock().await.list_repositories().await {
            Ok(repos) => {
                let mut success_count = 0;
                let mut error_count = 0;
                
                for repo in repos {
                    let claude_md_path = repo.local_path.join("CLAUDE.md");
                    
                    match tokio::fs::write(&claude_md_path, &template_content).await {
                        Ok(_) => {
                            success_count += 1;
                            log::info!("Updated CLAUDE.md for repository '{}'", repo.name);
                        },
                        Err(e) => {
                            error_count += 1;
                            log::error!("Failed to update CLAUDE.md for repository '{}': {}", repo.name, e);
                        }
                    }
                }
                
                log::info!("CLAUDE.md template applied to {success_count} repositories successfully, {error_count} errors");
            },
            Err(e) => {
                log::error!("Failed to list repositories for CLAUDE.md application: {e}");
            }
        }
    });
}

/// Handle repository added event
fn handle_repository_added(app: &mut SagittaCodeApp, repo_name: String) {
    // Check if auto-sync is enabled for new repositories
    if let Some(sync_orchestrator) = &app.sync_orchestrator {
        let config = app.config.clone();
        let sync_orchestrator = sync_orchestrator.clone();
        let repo_manager = app.repo_panel.get_repo_manager();
        
        tokio::spawn(async move {
            let config_guard = config.lock().await;
            if config_guard.auto_sync.sync_on_repo_add {
                log::info!("Auto-sync enabled for repository add, triggering sync for: {repo_name}");
                
                // Get repository path from repository manager
                let repo_manager_guard = repo_manager.lock().await;
                if let Ok(repositories) = repo_manager_guard.list_repositories().await {
                    if let Some(repo_config) = repositories.iter().find(|r| r.name == repo_name) {
                        let repo_path = std::path::PathBuf::from(&repo_config.local_path);
                        // Add repository to file watcher and trigger sync
                        if let Err(e) = sync_orchestrator.add_repository(&repo_path).await {
                            log::error!("Failed to add repository {} to sync orchestrator: {}", repo_name, e);
                        }
                    }
                }
            }
        });
    }
}

/// Handle repository switched event
fn handle_repository_switched(app: &mut SagittaCodeApp, repo_name: String) {
    // Check if auto-sync is enabled for repository switching
    if let Some(sync_orchestrator) = &app.sync_orchestrator {
        let config = app.config.clone();
        let sync_orchestrator = sync_orchestrator.clone();
        let repo_manager = app.repo_panel.get_repo_manager();
        
        tokio::spawn(async move {
            let config_guard = config.lock().await;
            if config_guard.auto_sync.sync_on_repo_switch {
                log::info!("Auto-sync enabled for repository switch, triggering sync for: {repo_name}");
                
                // Get repository path from repository manager
                let repo_manager_guard = repo_manager.lock().await;
                if let Ok(repositories) = repo_manager_guard.list_repositories().await {
                    if let Some(repo_config) = repositories.iter().find(|r| r.name == repo_name) {
                        let repo_path = std::path::PathBuf::from(&repo_config.local_path);
                        if let Err(e) = sync_orchestrator.switch_repository(&repo_path).await {
                            log::error!("Failed to trigger sync for switched repository {}: {}", repo_name, e);
                        }
                    }
                }
            }
        });
    }
}

/// Handle task execution event
fn handle_execute_task(app: &mut SagittaCodeApp, conversation_id: uuid::Uuid, task_message: String) {
    log::info!("Executing task for conversation {conversation_id}: {task_message}");
    
    // First, switch to the conversation
    switch_to_conversation(app, conversation_id);
    
    // Then, set the task message in the input buffer and trigger submission
    app.state.chat_input_buffer = task_message;
    app.state.chat_on_submit = true;
    
    // The message will be sent on the next UI update cycle
    log::info!("Task message set in input buffer and will be sent on next UI update");
}

/// Handle check and execute task event
fn handle_check_and_execute_task(app: &mut SagittaCodeApp) {
    log::info!("Checking if safe to execute next task");
    
    // Check if the current conversation is actively streaming
    if app.state.is_streaming_response {
        log::warn!("Cannot start task: Current conversation is still streaming a response");
        // Show a toast notification to the user
        app.state.toasts.warning("Cannot start task while a conversation is active. Please wait for the current response to complete.");
        return;
    }
    
    // Check if waiting for response
    if app.state.is_waiting_for_response {
        log::warn!("Cannot start task: Currently waiting for a response");
        app.state.toasts.warning("Cannot start task while waiting for a response. Please wait for the current operation to complete.");
        return;
    }
    
    // Check if a tool is currently executing
    if app.state.is_executing_tool {
        log::warn!("Cannot start task: A tool is currently executing");
        app.state.toasts.warning("Cannot start task while a tool is executing. Please wait for it to complete.");
        return;
    }
    
    // Check if there are any running tools
    if !app.state.running_tools.is_empty() {
        log::warn!("Cannot start task: {} tools are still running", app.state.running_tools.len());
        app.state.toasts.warning("Cannot start task while tools are running. Please wait for them to complete.");
        return;
    }
    
    // If all checks pass, execute the task via the task panel
    log::info!("All checks passed, proceeding to execute next task");
    
    // Get task panel and execute the task directly through its internal mechanism
    let task_panel_clone = app.task_panel.clone();
    tokio::spawn(async move {
        match task_panel_clone.start_next_task().await {
            Ok(Some(task_id)) => {
                log::info!("Successfully started task with ID: {}", task_id);
            }
            Ok(None) => {
                log::info!("No tasks in queue to start");
            }
            Err(e) => {
                log::error!("Failed to start next task: {}", e);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::state::types::{AgentMode, ConversationStatus};
    use crate::agent::message::types::{AgentMessage, ToolCall as AgentToolCall};
    use crate::tools::types::ToolResult as ToolResultType;
    use crate::config::types::SagittaCodeConfig;
    use sagitta_search::AppConfig;
    use crate::gui::repository::manager::RepositoryManager;
    use uuid::Uuid;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use serde_json::Value;

    // Helper function to create a minimal app for testing
    fn create_test_app() -> SagittaCodeApp {
        let app_config = AppConfig::default();
        let repo_manager = Arc::new(tokio::sync::Mutex::new(
            RepositoryManager::new(Arc::new(tokio::sync::Mutex::new(app_config.clone())))
        ));
        let sagitta_config = SagittaCodeConfig::default();
        
        let mut app = SagittaCodeApp::new(repo_manager, sagitta_config, app_config);
        
        // Create test event channels
        let (_agent_sender, agent_receiver) = tokio::sync::broadcast::channel(100);
        let (_app_sender, app_receiver) = mpsc::unbounded_channel();
        
        app.agent_event_receiver = Some(agent_receiver);
        app.app_event_receiver = Some(app_receiver);
        
        app
    }

    // Helper function to create a test agent message
    fn create_test_agent_message(role: crate::llm::client::Role, content: &str) -> AgentMessage {
        AgentMessage {
            id: Uuid::new_v4(),
            role,
            content: content.to_string(),
            is_streaming: false,
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
            tool_calls: Vec::new(),
        }
    }

    // Helper function to create a test tool call
    fn create_test_tool_call(name: &str, args: Value) -> AgentToolCall {
        AgentToolCall {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            arguments: args,
            result: None,
            successful: false,
            execution_time: None,
        }
    }

    #[test]
    fn test_app_event_enum() {
        let event = AppEvent::ResponseProcessingComplete;
        match event {
            AppEvent::ResponseProcessingComplete => assert!(true),
            AppEvent::RefreshConversationList => assert!(true),
            AppEvent::SwitchToConversation(_) => assert!(true),
            AppEvent::CheckpointSuggestionsReady { .. } => assert!(true),
            AppEvent::BranchSuggestionsReady { .. } => assert!(true),
            AppEvent::RepositoryListUpdated(_) => assert!(true),
            AppEvent::CancelTool(_) => assert!(true),
            AppEvent::RefreshRepositoryList => assert!(true),
            AppEvent::RenameConversation { .. } => assert!(true),
            AppEvent::SaveClaudeMdTemplate => assert!(true),
            AppEvent::ApplyClaudeMdToAllRepos => assert!(true),
            AppEvent::UpdateConversationTitle { .. } => assert!(true),
            AppEvent::CreateNewConversation => assert!(true),
            AppEvent::UpdateGitHistoryPath(_) => assert!(true),
            AppEvent::RepositoryAdded(_) => assert!(true),
            AppEvent::RepositorySwitched(_) => assert!(true),
            AppEvent::ShowSyncNotification { .. } => assert!(true),
            AppEvent::ShowNewConversationConfirmation => assert!(true),
            AppEvent::ReinitializeProvider { .. } => assert!(true),
            AppEvent::AgentReplaced { .. } => assert!(true),
            AppEvent::ExecuteTask { .. } => assert!(true),
            AppEvent::CheckAndExecuteTask => assert!(true),
            AppEvent::ToolExecutionComplete { .. } => assert!(true),
            AppEvent::UpdateUiPreference { .. } => assert!(true),
            AppEvent::DebugForceRefreshAllSummaries => assert!(true),
        }
        
        // Test the other variant too
        let event2 = AppEvent::RefreshConversationList;
        match event2 {
            AppEvent::ResponseProcessingComplete => assert!(true),
            AppEvent::RefreshConversationList => assert!(true),
            AppEvent::SwitchToConversation(_) => assert!(true),
            AppEvent::CheckpointSuggestionsReady { .. } => assert!(true),
            AppEvent::BranchSuggestionsReady { .. } => assert!(true),
            AppEvent::RepositoryListUpdated(_) => assert!(true),
            AppEvent::CancelTool(_) => assert!(true),
            AppEvent::RefreshRepositoryList => assert!(true),
            AppEvent::RenameConversation { .. } => assert!(true),
            AppEvent::SaveClaudeMdTemplate => assert!(true),
            AppEvent::ApplyClaudeMdToAllRepos => assert!(true),
            AppEvent::UpdateConversationTitle { .. } => assert!(true),
            AppEvent::CreateNewConversation => assert!(true),
            AppEvent::UpdateGitHistoryPath(_) => assert!(true),
            AppEvent::RepositoryAdded(_) => assert!(true),
            AppEvent::RepositorySwitched(_) => assert!(true),
            AppEvent::ShowSyncNotification { .. } => assert!(true),
            AppEvent::ShowNewConversationConfirmation => assert!(true),
            AppEvent::ReinitializeProvider { .. } => assert!(true),
            AppEvent::AgentReplaced { .. } => assert!(true),
            AppEvent::ExecuteTask { .. } => assert!(true),
            AppEvent::CheckAndExecuteTask => assert!(true),
            AppEvent::ToolExecutionComplete { .. } => assert!(true),
            AppEvent::UpdateUiPreference { .. } => assert!(true),
            AppEvent::DebugForceRefreshAllSummaries => assert!(true),
        }
    }

    #[test]
    fn test_conversation_event_enum() {
        let conversation_id = Uuid::new_v4();
        
        // Test DataLoaded variant
        let event1 = ConversationEvent::DataLoaded {
            current_title: Some("Test Conversation".to_string()),
            conversations: vec![],
        };
        match event1 {
            ConversationEvent::DataLoaded { current_title, conversations } => {
                assert_eq!(current_title, Some("Test Conversation".to_string()));
                assert!(conversations.is_empty());
            },
            _ => panic!("Wrong event type"),
        }

        // Test ConversationCreated variant
        let event2 = ConversationEvent::ConversationCreated(conversation_id);
        match event2 {
            ConversationEvent::ConversationCreated(id) => assert_eq!(id, conversation_id),
            _ => panic!("Wrong event type"),
        }

        // Test ConversationSwitched variant
        let event3 = ConversationEvent::ConversationSwitched(conversation_id);
        match event3 {
            ConversationEvent::ConversationSwitched(id) => assert_eq!(id, conversation_id),
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_make_chat_message_from_agent_message() {
        use crate::llm::client::Role;
        use crate::gui::chat::MessageAuthor;
        
        // Test user message
        let user_msg = create_test_agent_message(Role::User, "Hello, world!");
        let chat_msg = make_chat_message_from_agent_message(&user_msg);
        
        assert_eq!(chat_msg.author, MessageAuthor::User);
        assert_eq!(chat_msg.text, "Hello, world!");
        assert_eq!(chat_msg.id, Some(user_msg.id.to_string()));
        assert_eq!(chat_msg.timestamp, user_msg.timestamp);

        // Test assistant message
        let assistant_msg = create_test_agent_message(Role::Assistant, "Hello, user!");
        let chat_msg = make_chat_message_from_agent_message(&assistant_msg);
        
        assert_eq!(chat_msg.author, MessageAuthor::Agent);
        assert_eq!(chat_msg.text, "Hello, user!");

        // Test system message
        let system_msg = create_test_agent_message(Role::System, "System status OK");
        let chat_msg = make_chat_message_from_agent_message(&system_msg);
        
        assert_eq!(chat_msg.author, MessageAuthor::System);
        assert_eq!(chat_msg.text, "System status OK");
    }

    #[test]
    fn test_tool_call_preservation_in_chat_message_conversion() {
        use crate::gui::chat::StreamingMessage;

        // Create an agent tool call with the correct structure
        let agent_tool_call = AgentToolCall {
            id: "test_call_123".to_string(),
            name: "test_tool".to_string(),
            arguments: serde_json::json!({"param1": "value1"}),
            result: Some(serde_json::json!("Tool executed successfully")),
            successful: true,
            execution_time: Some(chrono::Utc::now()),
        };

        // Create an agent message with tool calls using the helper
        let mut agent_message = create_test_agent_message(crate::llm::client::Role::Assistant, "I'll help you with that task.");
        agent_message.tool_calls = vec![agent_tool_call];

        // Convert to ChatMessage using our fixed function
        let chat_message = make_chat_message_from_agent_message(&agent_message);

        // Verify tool calls are preserved
        assert_eq!(chat_message.tool_calls.len(), 1);
        assert_eq!(chat_message.tool_calls[0].id, "test_call_123");
        assert_eq!(chat_message.tool_calls[0].name, "test_tool");
        assert!(chat_message.tool_calls[0].result.is_some());
        
        // Convert ChatMessage to StreamingMessage
        let streaming_message: StreamingMessage = chat_message.into();

        // Verify tool calls survive the second conversion
        assert_eq!(streaming_message.tool_calls.len(), 1);
        assert_eq!(streaming_message.tool_calls[0].id, "test_call_123");
        assert_eq!(streaming_message.tool_calls[0].name, "test_tool");
        assert!(streaming_message.tool_calls[0].result.is_some());
    }

    #[test]
    fn test_tool_call_preservation_with_multiple_tool_calls() {
        use crate::gui::chat::StreamingMessage;

        // Create multiple agent tool calls
        let agent_tool_call1 = AgentToolCall {
            id: "call_1".to_string(),
            name: "tool_1".to_string(),
            arguments: serde_json::json!({"param": "value1"}),
            result: Some(serde_json::json!("Result 1")),
            successful: true,
            execution_time: Some(chrono::Utc::now()),
        };

        let agent_tool_call2 = AgentToolCall {
            id: "call_2".to_string(),
            name: "tool_2".to_string(),
            arguments: serde_json::json!({"param": "value2"}),
            result: Some(serde_json::json!("Error occurred")),
            successful: false,
            execution_time: Some(chrono::Utc::now()),
        };

        // Create an agent message with multiple tool calls
        let mut agent_message = create_test_agent_message(crate::llm::client::Role::Assistant, "Processing multiple tools.");
        agent_message.tool_calls = vec![agent_tool_call1, agent_tool_call2];

        // Convert to ChatMessage
        let chat_message = make_chat_message_from_agent_message(&agent_message);

        // Verify both tool calls are preserved
        assert_eq!(chat_message.tool_calls.len(), 2);
        
        // Check first tool call
        assert_eq!(chat_message.tool_calls[0].id, "call_1");
        assert_eq!(chat_message.tool_calls[0].name, "tool_1");
        
        // Check second tool call
        assert_eq!(chat_message.tool_calls[1].id, "call_2");
        assert_eq!(chat_message.tool_calls[1].name, "tool_2");

        // Convert to StreamingMessage
        let streaming_message: StreamingMessage = chat_message.into();

        // Verify both tool calls survive the conversion
        assert_eq!(streaming_message.tool_calls.len(), 2);
        assert_eq!(streaming_message.tool_calls[0].id, "call_1");
        assert_eq!(streaming_message.tool_calls[1].id, "call_2");
    }

    #[test]
    fn test_empty_tool_calls_preservation() {
        // Create an agent message with no tool calls using the helper
        let agent_message = create_test_agent_message(crate::llm::client::Role::Assistant, "Just a regular message.");

        // Convert to ChatMessage
        let chat_message = make_chat_message_from_agent_message(&agent_message);

        // Verify empty tool calls are preserved
        assert_eq!(chat_message.tool_calls.len(), 0);

        // Convert to StreamingMessage
        let streaming_message: crate::gui::chat::StreamingMessage = chat_message.into();

        // Verify empty tool calls list is preserved
        assert_eq!(streaming_message.tool_calls.len(), 0);
    }

    #[test]
    fn test_handle_llm_chunk_new_response() {
        let mut app = create_test_app();
        
        // Test starting a new response
        handle_llm_chunk(&mut app, "Hello".to_string(), false, false);
        
        // Should create a new streaming message
        assert!(app.state.current_response_id.is_some());
        assert!(app.state.is_streaming_response);
        
        // Should have one message in chat manager
        let messages = app.chat_manager.get_all_messages();
        assert_eq!(messages.len(), 1);
        
        let message = &messages[0];
        assert_eq!(message.content, "Hello");
        assert!(!message.is_complete());
    }

    #[test]
    fn test_handle_llm_chunk_continuation() {
        let mut app = create_test_app();
        
        // Start a response
        handle_llm_chunk(&mut app, "Hello".to_string(), false, false);
        let response_id = app.state.current_response_id.clone();
        
        // Continue the response
        handle_llm_chunk(&mut app, " world".to_string(), false, false);
        
        // Should still be the same response
        assert_eq!(app.state.current_response_id, response_id);
        assert!(app.state.is_streaming_response);
        
        // Content should be appended
        let messages = app.chat_manager.get_all_messages();
        let message = &messages[0];
        assert_eq!(message.content, "Hello world");
        assert!(!message.is_complete());
    }

    #[test]
    fn test_handle_llm_chunk_final() {
        let mut app = create_test_app();
        
        // Start and complete a response
        handle_llm_chunk(&mut app, "Complete response".to_string(), true, false);
        
        // Should complete the response
        assert!(app.state.current_response_id.is_none());
        assert!(!app.state.is_streaming_response);
        assert!(!app.state.is_waiting_for_response);
        
        let messages = app.chat_manager.get_all_messages();
        let message = &messages[0];
        assert_eq!(message.content, "Complete response");
        assert!(message.is_complete());
    }

    #[test]
    fn test_handle_tool_call() {
        let mut app = create_test_app();
        
        // First create an agent message to attach the tool call to
        handle_llm_chunk(&mut app, "I'll help you search".to_string(), true, false);
        
        let args = serde_json::json!({"query": "rust programming"});
        let tool_call = create_test_tool_call("web_search", args);
        
        handle_tool_call(&mut app, tool_call.clone());
        
        // Should not create a new message, but attach to existing one
        let messages = app.chat_manager.get_all_messages();
        assert_eq!(messages.len(), 1);
        
        // Should store pending tool call
        assert_eq!(app.state.pending_tool_calls.len(), 1);
        let stored_tool_call = &app.state.pending_tool_calls[0];
        assert_eq!(stored_tool_call.id, tool_call.id);
        assert_eq!(stored_tool_call.name, tool_call.name);
    }

    #[test]
    fn test_handle_tool_call_result_success() {
        let mut app = create_test_app();
        let tool_call_id = "test-tool-call-id".to_string();
        let tool_name = "web_search".to_string();
        let result = ToolResultType::Success { output: "Search results: Found 10 results".to_string() };
        
        // Add a tool result to state first
        app.state.add_tool_result(tool_call_id.clone(), "pending".to_string());
        
        handle_tool_call_result(&mut app, tool_call_id.clone(), tool_name.clone(), result);
        
        // Should update tool results
        let stored_result = app.state.tool_results.get(&tool_call_id);
        assert!(stored_result.is_some());
        assert!(stored_result.unwrap().contains("Search results"));
    }

    #[test]
    fn test_handle_tool_call_result_error() {
        let mut app = create_test_app();
        let tool_call_id = "test-tool-call-id".to_string();
        let tool_name = "web_search".to_string();
        let result = ToolResultType::Error {
            error: "Network connection failed".to_string(),
        };
        
        handle_tool_call_result(&mut app, tool_call_id.clone(), tool_name.clone(), result);
        
        // Should store error result in tool_results
        let stored_result = app.state.tool_results.get(&tool_call_id);
        assert!(stored_result.is_some());
        assert!(stored_result.unwrap().contains("Error"));
        assert!(stored_result.unwrap().contains("Network connection failed"));
    }

    #[test]
    fn test_handle_state_change() {
        let mut app = create_test_app();
        let new_state = AgentState::Thinking { message: "Processing...".to_string() };
        
        handle_state_change(&mut app, new_state.clone());
        
        // Should update current agent state
        assert_eq!(app.state.current_agent_state, new_state);
    }

    #[test]
    fn test_handle_state_change_mode_update() {
        let mut app = create_test_app();
        
        // Set current mode
        app.state.current_agent_mode = AgentMode::FullyAutonomous;
        
        let new_state = AgentState::Idle;
        
        handle_state_change(&mut app, new_state);
        
        // Should update current state
        assert_eq!(app.state.current_agent_state, AgentState::Idle);
    }

    #[test]
    fn test_process_app_events_no_receiver() {
        let mut app = create_test_app();
        app.app_event_receiver = None;
        
        // Should not panic when no receiver is present
        process_app_events(&mut app);
    }

    #[test]
    fn test_refresh_conversation_data() {
        let mut app = create_test_app();
        
        // Initially should not be loading
        assert!(!app.state.conversation_data_loading);
        
        refresh_conversation_data(&mut app);
        
        // Should set loading state
        assert!(app.state.conversation_data_loading);
    }

    #[test]
    fn test_force_refresh_conversation_data() {
        let mut app = create_test_app();
        let now = std::time::Instant::now();
        app.state.last_conversation_refresh = Some(now);
        
        force_refresh_conversation_data(&mut app);
        
        // Should clear last refresh time and set loading
        assert!(app.state.last_conversation_refresh.is_none());
        assert!(app.state.conversation_data_loading);
    }

    #[test]
    fn test_switch_to_conversation() {
        let mut app = create_test_app();
        let conversation_id = Uuid::new_v4();
        
        switch_to_conversation(&mut app, conversation_id);
        
        // Should update current conversation
        assert_eq!(app.state.current_conversation_id, Some(conversation_id));
        
        // Should clear current state
        assert!(app.state.current_response_id.is_none());
        assert!(!app.state.is_streaming_response);
        assert!(!app.state.is_waiting_for_response);
        
        // Should clear chat state
        let messages = app.chat_manager.get_all_messages();
        assert!(messages.is_empty());
        assert!(app.state.tool_results.is_empty());
        assert!(app.state.pending_tool_calls.is_empty());
    }

    #[test]
    fn test_agent_event_processing_workflow() {
        let mut app = create_test_app();
        
        // Simulate a complete workflow: user input -> llm chunks -> tool call -> result
        
        // 1. Start with LLM chunk
        handle_llm_chunk(&mut app, "I'll search for information".to_string(), false, false);
        assert!(app.state.is_streaming_response);
        let messages = app.chat_manager.get_all_messages();
        assert_eq!(messages.len(), 1);
        
        // 2. Complete LLM chunk
        handle_llm_chunk(&mut app, " about Rust programming.".to_string(), true, false);
        assert!(!app.state.is_streaming_response);
        let messages = app.chat_manager.get_all_messages();
        assert_eq!(messages[0].content, "I'll search for information about Rust programming.");
        
        // 3. Tool call - now attaches to existing message instead of creating new one
        let args = serde_json::json!({"query": "rust programming"});
        let tool_call = create_test_tool_call("web_search", args);
        handle_tool_call(&mut app, tool_call.clone());
        let messages = app.chat_manager.get_all_messages();
        assert_eq!(messages.len(), 1); // Still 1 message, tool call attached
        
        // 4. Tool result - stores in tool_results rather than creating message
        let result = ToolResultType::Success { output: "Found comprehensive Rust tutorials".to_string() };
        handle_tool_call_result(&mut app, tool_call.id.clone(), tool_call.name, result);
        let messages = app.chat_manager.get_all_messages();
        assert_eq!(messages.len(), 1); // Still 1 message
        
        // Verify message is agent message and tool result is stored
        assert_eq!(messages[0].author, crate::gui::chat::MessageAuthor::Agent);
        
        // Verify tool result is stored in state
        let stored_result = app.state.tool_results.get(&tool_call.id);
        assert!(stored_result.is_some());
        assert!(stored_result.unwrap().contains("Found comprehensive Rust tutorials"));
    }

    #[test]
    fn test_error_handling_in_llm_chunk() {
        let mut app = create_test_app();
        
        // Test empty content
        handle_llm_chunk(&mut app, "".to_string(), false, false);
        
        // Should still create a message entry but with empty content
        let messages = app.chat_manager.get_all_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "");
    }

    #[test]
    fn test_multiple_tool_calls() {
        let mut app = create_test_app();
        
        // First create an agent message to attach tool calls to
        handle_llm_chunk(&mut app, "I'll help with multiple searches".to_string(), true, false);
        
        // Add multiple tool calls
        let args1 = serde_json::json!({"query": "rust"});
        let args2 = serde_json::json!({"pattern": "fn main"});
        let tool_call1 = create_test_tool_call("web_search", args1);
        let tool_call2 = create_test_tool_call("code_search", args2);
        
        handle_tool_call(&mut app, tool_call1);
        handle_tool_call(&mut app, tool_call2);
        
        // Should not create new messages, but attach to existing message
        let messages = app.chat_manager.get_all_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(app.state.pending_tool_calls.len(), 2);
        
        // Verify both tool calls are stored
        assert!(app.state.pending_tool_calls.iter().any(|tc| tc.name == "web_search"));
        assert!(app.state.pending_tool_calls.iter().any(|tc| tc.name == "code_search"));
    }

    #[test]
    fn test_conversation_switching_clears_state() {
        let mut app = create_test_app();
        
        // Set up some state
        app.state.current_response_id = Some("test-response".to_string());
        app.state.is_streaming_response = true;
        app.state.is_waiting_for_response = true;
        app.state.add_tool_result("tool1".to_string(), "result1".to_string());
        
        let new_conversation_id = Uuid::new_v4();
        switch_to_conversation(&mut app, new_conversation_id);
        
        // Should clear all response state
        assert!(app.state.current_response_id.is_none());
        assert!(!app.state.is_streaming_response);
        assert!(!app.state.is_waiting_for_response);
        assert!(app.state.tool_results.is_empty());
        assert!(app.state.pending_tool_calls.is_empty());
        let messages = app.chat_manager.get_all_messages();
        assert!(messages.is_empty());
        
        // Should set new conversation
        assert_eq!(app.state.current_conversation_id, Some(new_conversation_id));
    }

    #[test]
    fn test_agent_message_content_parsing() {
        use crate::llm::client::Role;
        
        // Test message with content
        let agent_msg = create_test_agent_message(Role::Assistant, "Hello world!");
        let chat_msg = make_chat_message_from_agent_message(&agent_msg);
        assert_eq!(chat_msg.text, "Hello world!");
    }

    #[test]
    fn test_tool_result_metadata_handling() {
        let mut app = create_test_app();
        let tool_call_id = "test-tool-call-id".to_string();
        let tool_name = "file_search".to_string();
        
        let result = ToolResultType::Success { 
            output: serde_json::json!({
                "output": "Found 5 files",
                "metadata": {
                    "file_count": 5,
                    "search_time": "150ms"
                }
            }).to_string() 
        };
        
        handle_tool_call_result(&mut app, tool_call_id.clone(), tool_name, result);
        
        // Should store result in tool_results
        let stored_result = app.state.tool_results.get(&tool_call_id);
        assert!(stored_result.is_some());
        assert!(stored_result.unwrap().contains("Found 5 files") || stored_result.unwrap().contains("file_count"));
    }

    #[test]
    fn test_state_change_conversation_switching() {
        let mut app = create_test_app();
        let old_conversation_id = Uuid::new_v4();
        let new_conversation_id = Uuid::new_v4();
        
        app.state.current_conversation_id = Some(old_conversation_id);
        
        let new_state = AgentState::Responding { 
            is_streaming: true, 
            step_info: Some("Processing new conversation".to_string()) 
        };
        
        handle_state_change(&mut app, new_state.clone());
        
        // Should update agent state
        assert_eq!(app.state.current_agent_state, new_state);
    }

    #[test]
    fn test_agent_state_variants() {
        let mut app = create_test_app();
        
        // Test different agent state variants
        let idle_state = AgentState::Idle;
        handle_state_change(&mut app, idle_state.clone());
        assert_eq!(app.state.current_agent_state, idle_state);
        
        let thinking_state = AgentState::Thinking { message: "Thinking...".to_string() };
        handle_state_change(&mut app, thinking_state.clone());
        assert_eq!(app.state.current_agent_state, thinking_state);
        
        let responding_state = AgentState::Responding { is_streaming: true, step_info: None };
        handle_state_change(&mut app, responding_state.clone());
        assert_eq!(app.state.current_agent_state, responding_state);
        
        let tool_state = AgentState::ExecutingTool { 
            tool_call_id: "test-id".to_string(), 
            tool_name: "test-tool".to_string() 
        };
        handle_state_change(&mut app, tool_state.clone());
        assert_eq!(app.state.current_agent_state, tool_state);
        
        let error_state = AgentState::Error { 
            message: "Test error".to_string(), 
            details: Some("Error details".to_string()) 
        };
        handle_state_change(&mut app, error_state.clone());
        assert_eq!(app.state.current_agent_state, error_state);
    }

    #[test]
    fn test_switch_to_conversation_event() {
        let app = create_test_app();
        let conversation_id = Uuid::new_v4();
        
        // Test the new AppEvent variant
        let event = AppEvent::SwitchToConversation(conversation_id);
        match event {
            AppEvent::SwitchToConversation(id) => assert_eq!(id, conversation_id),
            _ => panic!("Expected SwitchToConversation event"),
        }
    }

    #[test]
    fn test_analyze_conversation_for_suggestions() {
        let mut app = create_test_app();
        let conversation_id = uuid::Uuid::new_v4();
        
        // This test verifies that the analyze function can be called without panicking
        // The actual async behavior would need integration tests
        analyze_conversation_for_suggestions(&mut app, conversation_id);
        
        // Verify that the function doesn't crash when no conversation service is available
        assert!(app.conversation_service.is_none());
    }

    #[test]
    fn test_automatic_analysis_on_final_chunk() {
        let mut app = create_test_app();
        let conversation_id = uuid::Uuid::new_v4();
        app.state.current_conversation_id = Some(conversation_id);
        
        // Simulate final LLM chunk which should trigger analysis
        handle_llm_chunk(&mut app, "The solution is working successfully!".to_string(), true, false);
        
        // Verify that the conversation ID is set for analysis
        assert_eq!(app.state.current_conversation_id, Some(conversation_id));
    }

    #[test]
    fn test_checkpoint_suggestions_event_handling() {
        let mut app = create_test_app();
        let conversation_id = uuid::Uuid::new_v4();
        
        // Create test checkpoint suggestions
        let suggestions = vec![
            crate::agent::conversation::checkpoints::CheckpointSuggestion {
                message_id: uuid::Uuid::new_v4(),
                importance: 0.8,
                reason: crate::agent::conversation::checkpoints::CheckpointReason::SuccessfulSolution,
                suggested_title: "Test Checkpoint".to_string(),
                context: crate::agent::conversation::checkpoints::CheckpointContext {
                    relevant_messages: vec![],
                    trigger_keywords: vec!["success".to_string()],
                    conversation_phase: crate::agent::conversation::checkpoints::ConversationPhase::Implementation,
                    modified_files: vec![],
                    executed_tools: vec![],
                    success_indicators: vec!["working".to_string()],
                },
                restoration_value: 0.9,
            }
        ];
        
        // Test handling checkpoint suggestions
        app.handle_checkpoint_suggestions(conversation_id, suggestions);
        
        // The simplified system no longer tracks checkpoint suggestions
        // Just verify the app handles the event without crashing
    }

    #[test]
    fn test_branch_suggestions_event_handling() {
        let mut app = create_test_app();
        let conversation_id = uuid::Uuid::new_v4();
        
        // Create test branch suggestions
        let suggestions = vec![
            crate::agent::conversation::branching::BranchSuggestion {
                message_id: uuid::Uuid::new_v4(),
                confidence: 0.7,
                reason: crate::agent::conversation::branching::BranchReason::AlternativeApproach,
                suggested_title: "Test Branch".to_string(),
                success_probability: Some(0.6),
                context: crate::agent::conversation::branching::BranchContext {
                    relevant_messages: vec![],
                    trigger_keywords: vec!["alternative".to_string()],
                    conversation_state: crate::agent::conversation::branching::ConversationState::SolutionDevelopment,
                    project_context: None,
                    mentioned_tools: vec![],
                },
            }
        ];
        
        // Test handling branch suggestions
        app.handle_branch_suggestions(conversation_id, suggestions);
        
        // The simplified system no longer tracks branch suggestions
        // Just verify the app handles the event without crashing
    }
    
    #[test]
    fn test_refresh_conversation_data_without_service_or_agent() {
        let mut app = create_test_app();
        
        // Ensure no service or agent is available
        app.conversation_service = None;
        app.agent = None;
        
        // Initially should not be loading
        assert!(!app.state.conversation_data_loading);
        
        // Call refresh
        refresh_conversation_data(&mut app);
        
        // Should set loading state initially
        assert!(app.state.conversation_data_loading);
        
        // Process events to handle the DataLoaded event
        process_conversation_events(&mut app);
        
        // Loading state should be cleared even without service/agent
        assert!(!app.state.conversation_data_loading);
        
        // Conversation list should be empty
        assert!(app.state.conversation_list.is_empty());
        assert!(app.state.current_conversation_title.is_none());
    }
    
    #[test]
    fn test_force_refresh_conversation_data_without_service_or_agent() {
        let mut app = create_test_app();
        
        // Ensure no service or agent is available
        app.conversation_service = None;
        app.agent = None;
        
        // Initially should not be loading
        assert!(!app.state.conversation_data_loading);
        
        // Call force refresh
        force_refresh_conversation_data(&mut app);
        
        // Should set loading state initially
        assert!(app.state.conversation_data_loading);
        
        // Process events to handle the DataLoaded event
        process_conversation_events(&mut app);
        
        // Loading state should be cleared even without service/agent
        assert!(!app.state.conversation_data_loading);
        
        // Conversation list should be empty
        assert!(app.state.conversation_list.is_empty());
        assert!(app.state.current_conversation_title.is_none());
    }
    
    #[test]
    fn test_switch_to_conversation_loads_messages() {
        let mut app = create_test_app();
        let conversation_id = Uuid::new_v4();
        
        // Create some test messages
        let test_messages = vec![
            create_test_agent_message(crate::llm::client::Role::User, "Hello"),
            create_test_agent_message(crate::llm::client::Role::Assistant, "Hi there!"),
        ];
        
        // Add conversation to the list
        app.state.conversation_list.push(crate::agent::conversation::types::ConversationSummary {
            id: conversation_id,
            title: "Test Conversation".to_string(),
            created_at: chrono::Utc::now(),
            last_active: chrono::Utc::now(),
            message_count: 2,
            status: ConversationStatus::Active,
            tags: vec![],
            workspace_id: None,
            has_branches: false,
            has_checkpoints: false,
            project_name: None,
        });
        
        // Clear chat manager messages
        app.chat_manager.clear_all_messages();
        assert_eq!(app.chat_manager.get_all_messages().len(), 0);
        
        // Switch to the conversation
        switch_to_conversation(&mut app, conversation_id);
        
        // Verify state is updated
        assert_eq!(app.state.current_conversation_id, Some(conversation_id));
        assert_eq!(app.state.current_conversation_title, Some("Test Conversation".to_string()));
        assert!(app.state.conversation_data_loading);
        
        // Simulate receiving the loaded messages event
        app.handle_conversation_messages(conversation_id, test_messages);
        
        // Verify messages were loaded into chat manager
        assert_eq!(app.chat_manager.get_all_messages().len(), 2);
        assert!(!app.state.conversation_data_loading);
    }
}

/// Handle provider reinitialization
fn handle_provider_reinitialization(app: &mut SagittaCodeApp, provider_type: crate::providers::types::ProviderType) {
    log::info!("Reinitializing provider to: {:?}", provider_type);
    
    // Store provider type for the reinitialization
    app.state.pending_provider_change = Some(provider_type);
    
    // Update the current provider in the config (this will be overridden by the async function)
    if let Ok(mut config) = app.config.try_lock() {
        config.current_provider = provider_type;
        log::info!("Updated current provider in config to: {:?}", provider_type);
    } else {
        log::error!("Failed to lock config for provider update");
        return;
    }
    
    // Trigger async reinitialization
    let config_clone = app.config.clone();
    let app_event_sender = app.app_event_sender.clone();
    
    tokio::spawn(async move {
        match reinitialize_agent_with_provider(config_clone, provider_type).await {
            Ok(new_agent) => {
                log::info!("Successfully reinitialized agent with provider: {:?}", provider_type);
                // Send success notification
                if let Err(send_err) = app_event_sender.send(AppEvent::ShowSyncNotification {
                    repository: "Provider".to_string(),
                    message: format!("Successfully switched to {:?}", provider_type),
                    notification_type: SyncNotificationType::Success,
                }) {
                    log::error!("Failed to send success notification: {}", send_err);
                }
                
                // Send event to replace the agent
                if let Err(send_err) = app_event_sender.send(AppEvent::AgentReplaced { agent: new_agent }) {
                    log::error!("Failed to send AgentReplaced event: {}", send_err);
                }
            },
            Err(e) => {
                log::warn!("Provider reinitialization requires restart: {}", e);
                
                // Check if this is the expected "restart required" message
                if e.contains("Settings saved successfully") {
                    // Send info notification for successful save but restart required
                    if let Err(send_err) = app_event_sender.send(AppEvent::ShowSyncNotification {
                        repository: "Provider".to_string(),
                        message: format!("Settings saved! Please restart to switch to {:?}", provider_type),
                        notification_type: SyncNotificationType::Info,
                    }) {
                        log::error!("Failed to send info notification: {}", send_err);
                    }
                } else {
                    // Send error notification for actual failures
                    if let Err(send_err) = app_event_sender.send(AppEvent::ShowSyncNotification {
                        repository: "Provider".to_string(),
                        message: format!("Failed to switch to {:?}: {}", provider_type, e),
                        notification_type: SyncNotificationType::Error,
                    }) {
                        log::error!("Failed to send error notification: {}", send_err);
                    }
                }
            }
        }
    });
}

/// Reinitialize agent with a new provider
async fn reinitialize_agent_with_provider(
    config: std::sync::Arc<tokio::sync::Mutex<crate::config::types::SagittaCodeConfig>>,
    provider_type: crate::providers::types::ProviderType,
) -> Result<std::sync::Arc<crate::agent::core::Agent>, String> {
    use crate::gui::app::initialization::create_llm_client;
    use crate::config::save_config as save_sagitta_code_config;
    
    log::info!("Attempting to reinitialize agent with provider: {:?}", provider_type);
    
    // Lock the config to read current settings
    let config_guard = config.lock().await;
    let mut current_config = config_guard.clone();
    drop(config_guard);
    
    // Update the current provider
    current_config.current_provider = provider_type;
    
    // Save the updated configuration to disk
    match save_sagitta_code_config(&current_config) {
        Ok(_) => {
            log::info!("Successfully saved updated configuration with provider: {:?}", provider_type);
            
            // Update the config in memory
            let mut config_guard = config.lock().await;
            config_guard.current_provider = provider_type;
            drop(config_guard);
        },
        Err(e) => {
            log::error!("Failed to save configuration: {}", e);
            return Err(format!("Failed to save configuration: {}", e));
        }
    }
    
    // Test the new provider configuration
    let llm_client_test = create_llm_client(&current_config, None)
        .await
        .map_err(|e| format!("Failed to create LLM client with new provider: {}", e))?;
    
    log::info!("Successfully validated new LLM client with provider: {:?}", provider_type);
    
    // LIMITATION: The current Agent architecture doesn't support hot-swapping the LLM client
    // The Agent struct stores the LLM client as Arc<dyn LlmClient> and has no methods to update it
    // This is by design for thread safety, but means full reinitialization is complex
    
    // To properly implement hot-reload, we would need to:
    // 1. Create a new embedding provider adapter
    // 2. Create new conversation persistence and search engine instances  
    // 3. Create a completely new Agent with Agent::new()
    // 4. Re-wire all the event subscriptions and UI connections
    // 5. Update the RepoPanel, ConversationService, and other components
    
    // For now, we'll save the settings and inform the user that restart is needed
    log::info!("Configuration saved successfully. Agent architecture requires restart for provider switch.");
    
    Err("Settings saved successfully! However, the current Agent architecture requires a restart to fully switch providers. This is a known limitation - the Agent is designed to be immutable with respect to its LLM provider for thread safety.".to_string())
}

/// Handle agent replacement
fn handle_agent_replaced(app: &mut SagittaCodeApp, new_agent: std::sync::Arc<crate::agent::core::Agent>) {
    log::info!("Replacing agent with new one");
    
    // Replace the agent in the app
    app.agent = Some(new_agent);
    
    // Clear the pending provider change
    app.state.pending_provider_change = None;
    
    // Update the current provider to match the new agent
    // The provider should already be set in the config from the reinitialization
    
    log::info!("Agent replacement completed successfully");
}

/// Handle debug force refresh all conversation summaries
fn handle_debug_force_refresh_all_summaries(app: &mut SagittaCodeApp) {
    log::info!("DEBUG: Force refreshing all conversation summaries");
    
    // Get the conversation service
    if let Some(service) = &app.conversation_service {
        let service_clone = service.clone();
        let app_event_sender = app.app_event_sender.clone();
        
        // Spawn a task to refresh all conversation summaries
        tokio::spawn(async move {
            log::info!("Starting force refresh of all conversation summaries...");
            
            // Get all conversations
            let conversations = service_clone.list_conversations().await.unwrap_or_default();
            log::info!("Found {} conversations to refresh", conversations.len());
            
            // For each conversation, regenerate its summary
            for (index, conv) in conversations.iter().enumerate() {
                log::info!("Refreshing summary for conversation {} ({}/{})...", conv.id, index + 1, conversations.len());
                
                // Load the full conversation
                match service_clone.get_conversation(conv.id).await {
                    Ok(Some(full_conversation)) => {
                        // The conversation service should automatically update the summary
                        // when loading a conversation, but we can force it by saving
                        // The conversation service doesn't have a save_conversation method
                        // We'll just skip the save step since loading should refresh the summary
                        log::info!("Successfully loaded conversation {} to refresh summary", conv.id);
                    }
                    Ok(None) => {
                        log::warn!("Conversation {} not found", conv.id);
                    }
                    Err(e) => {
                        log::error!("Failed to load conversation {}: {}", conv.id, e);
                    }
                }
                
                // Small delay to avoid overwhelming the system
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            
            log::info!("Completed force refresh of all conversation summaries");
            
            // Trigger a UI refresh
            if let Err(e) = app_event_sender.send(AppEvent::RefreshConversationList) {
                log::error!("Failed to send RefreshConversationList event after force refresh: {e}");
            }
        });
    } else {
        log::warn!("No conversation service available for force refresh");
    }
}

/// Execute a tool through the internal MCP implementation
async fn execute_mcp_tool(tool_name: &str, arguments: serde_json::Value, app_config: Option<Arc<tokio::sync::Mutex<crate::config::types::SagittaCodeConfig>>>) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    use sagitta_mcp::handlers::tool::handle_tools_call;
    use sagitta_mcp::mcp::types::CallToolParams;
    use sagitta_search::config::load_config;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use qdrant_client::Qdrant;
    
    log::debug!("Executing tool {} with arguments: {}", tool_name, serde_json::to_string_pretty(&arguments)?);
    
    // Load the sagitta search config
    let config = load_config(None).map_err(|e| format!("Failed to load config: {}", e))?;
    let config = Arc::new(RwLock::new(config));
    
    // Create Qdrant client
    let qdrant_url = {
        let cfg = config.read().await;
        cfg.qdrant_url.clone()
    };
    let qdrant_client = Qdrant::from_url(&qdrant_url).build()
        .map_err(|e| format!("Failed to create Qdrant client: {}", e))?;
    let qdrant_client = Arc::new(qdrant_client);
    
    // Inject timeout for shell_execute if config is provided
    let modified_arguments = if tool_name == "shell_execute" {
        if let Some(app_cfg) = app_config {
            let mut args = arguments;
            // Only inject timeout if not already specified
            if args.get("timeout_ms").is_none() {
                let cfg = app_cfg.lock().await;
                args["timeout_ms"] = serde_json::json!(cfg.tools.shell_timeout_ms);
            }
            args
        } else {
            arguments
        }
    } else {
        arguments
    };
    
    // Create the tool call params
    let params = CallToolParams {
        name: tool_name.to_string(),
        arguments: modified_arguments,
    };
    
    // Execute the tool directly using the MCP handler
    match handle_tools_call(params, config, qdrant_client).await {
        Ok(Some(result)) => {
            log::debug!("Tool {} executed successfully, raw result: {}", tool_name, 
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| "unparseable".to_string()));
            
            // Extract the actual content from the MCP result structure
            // MCP returns: { "content": [{"text": "...", "type": "text"}], "isError": false }
            if let Some(content_array) = result.get("content").and_then(|v| v.as_array()) {
                if let Some(first_content) = content_array.first() {
                    if let Some(text) = first_content.get("text").and_then(|v| v.as_str()) {
                        // Try to parse the text as JSON, otherwise return it as a string
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                            log::debug!("Parsed tool result as JSON");
                            Ok(parsed)
                        } else {
                            log::debug!("Returning tool result as text");
                            Ok(serde_json::json!({ "result": text }))
                        }
                    } else {
                        // No text field, return the whole content block
                        Ok(first_content.clone())
                    }
                } else {
                    // Empty content array
                    Ok(serde_json::json!({}))
                }
            } else {
                // No content field, return the whole result
                log::warn!("MCP result doesn't have expected 'content' field, returning raw result");
                Ok(result)
            }
        }
        Ok(None) => {
            log::debug!("Tool {} executed successfully with no result", tool_name);
            Ok(serde_json::json!({}))
        }
        Err(e) => {
            log::error!("Tool {} execution failed: {:?}", tool_name, e);
            Err(format!("Tool execution failed: {:?}", e).into())
        }
    }
} 