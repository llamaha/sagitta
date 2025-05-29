//! # Reasoning Engine
//!
//! A sophisticated graph-based reasoning engine with streaming coordination for AI agents.
//! 
//! This crate provides:
//! - Stateful graph execution with conditional routing
//! - Intelligent decision making with confidence scoring
//! - Self-reflection and learning capabilities
//! - Robust streaming infrastructure with error recovery
//! - Tool orchestration with backtracking support
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use reasoning_engine::{ReasoningEngine, ReasoningConfig};
//! use std::sync::Arc;
//! // Mock LlmClient and IntentAnalyzer for the example
//! struct MockLlmClient;
//! #[async_trait::async_trait]
//! impl reasoning_engine::LlmClient for MockLlmClient {
//!     async fn generate_stream(&self, _messages: Vec<reasoning_engine::LlmMessage>) -> reasoning_engine::Result<std::pin::Pin<Box<dyn futures_util::Stream<Item = reasoning_engine::Result<reasoning_engine::LlmStreamChunk>> + Send>>> {
//!         unimplemented!("MockLlmClient is for doc examples only")
//!     }
//! }
//! struct MockIntentAnalyzer;
//! #[async_trait::async_trait]
//! impl reasoning_engine::IntentAnalyzer for MockIntentAnalyzer {
//!    async fn analyze_intent(&self, _text: &str, _conversation_context: Option<&[reasoning_engine::LlmMessage]>) -> reasoning_engine::Result<reasoning_engine::DetectedIntent> {
//!        unimplemented!("MockIntentAnalyzer is for doc examples only")
//!    }
//! }
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ReasoningConfig::default();
//!     let mock_llm_client = Arc::new(MockLlmClient);
//!     let mock_intent_analyzer = Arc::new(MockIntentAnalyzer);
//!     let engine = ReasoningEngine::new(config, mock_llm_client, mock_intent_analyzer).await?;
//!     
//!     // The reasoning engine now supports tool orchestration
//!     // Full implementation will be available after integration
//!     
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod traits;
pub mod state;
pub mod graph;
pub mod decision;
pub mod streaming;
pub mod coordination;
pub mod reflection;
pub mod backtracking;
pub mod patterns;
pub mod confidence;
pub mod orchestration;
pub mod config;

// Re-export main types for convenience
pub use error::{ReasoningError, Result};
pub use traits::{ToolExecutor, EventEmitter, StreamHandler, ToolResult, ToolDefinition, ReasoningEvent, LlmClient, LlmMessage, LlmMessagePart, LlmStreamChunk, ToolCall, IntentAnalyzer, DetectedIntent};
pub use state::{ReasoningState, ReasoningContext, ReasoningStep};
pub use graph::{ReasoningGraph, ReasoningNode, NodeType, GraphEdge, EdgeCondition};
pub use decision::{DecisionEngine, Decision, DecisionContext, DecisionOption};
pub use streaming::{StreamingEngine, StreamChunk, StreamEvent, StreamState};
pub use orchestration::{ToolOrchestrator, ToolExecutionRequest, OrchestrationResult, ExecutionStatus};
pub use config::{ReasoningConfig, StreamingConfig, OrchestrationConfig};
pub use coordination::StreamCoordinator;

use std::sync::Arc;
use uuid::Uuid;
use std::collections::HashMap;
use std::time::{Instant, Duration};
use futures_util::StreamExt;
use serde_json::Value;
use chrono::Utc;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

/// The main reasoning engine that orchestrates all components
pub struct ReasoningEngine<LC: LlmClient + 'static, IA: IntentAnalyzer + 'static> {
    graph: ReasoningGraph,
    streaming: StreamingEngine,
    decision_engine: DecisionEngine,
    orchestrator: ToolOrchestrator,
    coordinator: StreamCoordinator,
    config: ReasoningConfig,
    llm_client: Arc<LC>,
    intent_analyzer: Arc<IA>,
}

impl<LC: LlmClient + 'static, IA: IntentAnalyzer + 'static> ReasoningEngine<LC, IA> {
    /// Create a new reasoning engine with the given configuration
    pub async fn new(config: ReasoningConfig, llm_client: Arc<LC>, intent_analyzer: Arc<IA>) -> Result<Self> {
        tracing::info!("Initializing reasoning engine with config: {:?}", config);
        
        // Validate configuration
        config.validate().map_err(|e| ReasoningError::configuration(&e))?;
        
        let graph = ReasoningGraph::new(config.clone()).await?;
        let streaming = StreamingEngine::new(config.streaming.clone()).await?;
        let decision_engine = DecisionEngine::new(config.decision.clone()).await?;
        let orchestrator = ToolOrchestrator::new(config.orchestration.clone()).await?;
        let coordinator = StreamCoordinator::new(config.clone()).await?;
        
        Ok(Self {
            graph,
            streaming,
            decision_engine,
            orchestrator,
            coordinator,
            config,
            llm_client,
            intent_analyzer,
        })
    }
    
    /// Process a reasoning request with full orchestration
    pub async fn process<T, E, S>(
        &mut self,
        full_llm_conversation_history: Vec<LlmMessage>,
        tool_executor: Arc<T>,
        event_emitter: Arc<E>,
        stream_handler: Arc<S>,
    ) -> Result<ReasoningState>
    where
        T: ToolExecutor + 'static,
        E: EventEmitter + 'static,
        S: StreamHandler + 'static,
    {
        self.process_with_context(full_llm_conversation_history, tool_executor, event_emitter, stream_handler, None, None).await
    }

    /// NEW: Process a reasoning request with optional previous state for continuity
    pub async fn process_with_context<T, E, S>(
        &mut self,
        full_llm_conversation_history: Vec<LlmMessage>,
        tool_executor: Arc<T>,
        event_emitter: Arc<E>,
        stream_handler: Arc<S>,
        previous_state: Option<&ReasoningState>,
        conversation_id: Option<Uuid>,
    ) -> Result<ReasoningState>
    where
        T: ToolExecutor + 'static,
        E: EventEmitter + 'static,
        S: StreamHandler + 'static,
    {
        let session_id = Uuid::new_v4();

        // Extract the current user input from the last message if possible, for state and initial tool
        let current_user_input_text = full_llm_conversation_history
            .last()
            .filter(|msg| msg.role == "user")
            .map(|msg| {
                // Collect ALL text parts from the user message, not just the first one
                msg.parts.iter()
                    .filter_map(|part| match part {
                        LlmMessagePart::Text(text) => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<&str>>()
                    .join("\n") // Join multiple text parts with newlines
            })
            .filter(|text| !text.trim().is_empty()) // Only use non-empty text
            .unwrap_or_else(|| "<missing_user_input_from_history>".to_string());

        // Create state with continuity support
        let mut state = if let Some(prev_state) = previous_state {
            ReasoningState::new_continuation(current_user_input_text.clone(), prev_state, conversation_id)
        } else {
            ReasoningState::new(current_user_input_text.clone())
        };
        state.session_id = session_id;
        
        tracing::info!(%session_id, initial_user_input = %current_user_input_text, history_len = full_llm_conversation_history.len(), is_continuation = state.session_metadata.is_continuation, "Starting reasoning process with history");
        
        // Add context summary to the LLM conversation if this is a continuation
        let mut llm_conversation_history = full_llm_conversation_history;
        if state.session_metadata.is_continuation {
            let context_summary = state.get_context_summary();
            if !context_summary.is_empty() {
                // Insert context summary as a system message before the current user input
                let context_message = LlmMessage {
                    role: "system".to_string(),
                    parts: vec![LlmMessagePart::Text(format!(
                        "CONVERSATION CONTEXT:\n{}\n\nContinuing with the current request...",
                        context_summary
                    ))],
                };
                
                // Insert before the last user message
                if llm_conversation_history.len() > 1 {
                    llm_conversation_history.insert(llm_conversation_history.len() - 1, context_message);
                } else {
                    llm_conversation_history.insert(0, context_message);
                }
                
                tracing::info!(%session_id, "Added conversation context to LLM history");
            }
        }
        
        event_emitter.emit_event(ReasoningEvent::SessionStarted {
            session_id,
            input: current_user_input_text.clone(),
            timestamp: chrono::Utc::now(),
        }).await?;

        // Variable to store the result of the last successful tool orchestration
        let mut pending_tool_summary_info: Option<OrchestrationResult> = None;
        
        // Track the last analyzed LLM response to prevent duplicate intent analysis
        let mut last_analyzed_content: Option<String> = None;

        // Initial planning step (analyze_input) - operates on the *current* user input text
        let initial_tool_request = ToolExecutionRequest::new(
            "analyze_input".to_string(),
            serde_json::json!({ 
                "input": current_user_input_text.clone(),
                "context": if state.session_metadata.is_continuation {
                    Some(state.get_context_summary())
                } else {
                    None
                }
            })
        );
        tracing::debug!(%session_id, ?initial_tool_request, "Constructed initial_tool_request for analyze_input");

        let initial_orchestration_result_maybe = self.orchestrator.orchestrate_tools(
            vec![initial_tool_request.clone()], // Clone for logging if needed later
            tool_executor.clone(),
            event_emitter.clone(),
        ).await;

        // This variable will hold the successful result if Ok, or be None if Err
        let mut initial_analysis_succeeded_and_value: Option<(bool, Value)> = None;

        match initial_orchestration_result_maybe {
            Ok(orchestration_result) => {
                tracing::info!(%session_id, initial_orchestration_result = ?orchestration_result, "Full Initial 'analyze_input' orchestration result OBTAINED SUCCESSFULLY.");
                state.add_step(ReasoningStep::from_orchestration_result(&orchestration_result, Some("Initial analysis with context")));

                if !orchestration_result.success {
                    tracing::info!(%session_id, success_field = orchestration_result.success, "EARLY EXIT: Initial 'analyze_input' orchestration result indicates FAILURE. Ending session BEFORE main loop.");
                    state.set_completed(false, "Initial analysis failed".to_string());
                    event_emitter.emit_event(ReasoningEvent::SessionCompleted {
                        session_id,
                        success: false,
                        total_duration_ms: orchestration_result.total_execution_time.as_millis() as u64,
                        steps_executed: state.history.len() as u32,
                        tools_used: orchestration_result.tool_results.keys().cloned().collect(),
                    }).await?;
                    return Ok(state);
                } else {
                    tracing::info!(%session_id, success_field = orchestration_result.success, "Initial 'analyze_input' orchestration result indicates SUCCESS. Proceeding past early exit check.");
                    
                    let analysis_output_opt_val: Option<Value>;
                    match orchestration_result
                        .tool_results
                        .get("analyze_input") // Option<&ToolExecutionResult>
                        .and_then(|exec_res: &crate::orchestration::ToolExecutionResult| exec_res.result.as_ref()) // Option<&crate::traits::ToolResult>
                    {
                        Some(reasoning_res) => { // reasoning_res is &crate::traits::ToolResult
                            analysis_output_opt_val = Some(reasoning_res.data.clone()); 
                        }
                        None => {
                            analysis_output_opt_val = None;
                        }
                    }
                    
                    let final_analysis_val = analysis_output_opt_val.unwrap_or(Value::Null);
                    initial_analysis_succeeded_and_value = Some((orchestration_result.success, final_analysis_val));
                }
            }
            Err(e) => {
                tracing::error!(%session_id, "ERROR from orchestrate_tools for analyze_input: {}. Orchestration aborted before obtaining result.", e);
                state.set_completed(false, format!("Initial analysis orchestration error: {}", e));
                event_emitter.emit_event(ReasoningEvent::SessionCompleted {
                    session_id,
                    success: false,
                    total_duration_ms: Duration::from_secs(0).as_millis() as u64, // No result to get time from
                    steps_executed: state.history.len() as u32,
                    tools_used: vec![],
                }).await?;
                return Ok(state); // Return Ok(state) here as per sagitta_code's expectation of Ok(ReasoningState)
                                  // but the state itself indicates failure.
            }
        }
        
        // Add tool result to conversation history only if initial_analysis_succeeded_and_value is Some
        if let Some((succeeded, analysis_val)) = initial_analysis_succeeded_and_value {
            if succeeded && !analysis_val.is_null() {
                state.mark_tool_successful("analyze_input".to_string()); // Mark successful in state
                llm_conversation_history.push(LlmMessage {
                    role: "user".to_string(),
                    parts: vec![LlmMessagePart::Text(serde_json::to_string(&json!({
                        "tool_name": "analyze_input",
                        "success": true,
                        "data": analysis_val,
                        "error": Value::Null
                    })).unwrap_or_default())],
                });
            } else if !succeeded {
                 // If initial_orchestration_result.success was false, but we didn't return early (e.g. if logic changes)
                 // we might log or handle that analyze_input didn't truly succeed for history addition.
                 // For now, this branch is unlikely if the early return for !orchestration_result.success is hit.
            }
        } else {
            // This case implies orchestrate_tools returned Err and we already returned Ok(state) with error status.
            // So, nothing to do here for llm_conversation_history regarding analyze_input.
        }

        tracing::info!(%session_id, "REACHED POINT JUST BEFORE MAIN ITERATION LOOP.");
        // DEBUG: Log state before entering main iteration loop
        tracing::debug!(%session_id, "About to enter main iteration loop with max_iterations={}", self.config.max_iterations);
        tracing::debug!(%session_id, history_len = llm_conversation_history.len(), "Conversation history before main loop");
        
        for iteration in 0..self.config.max_iterations {
            tracing::info!(%session_id, iteration, "Starting reasoning iteration");
            
            // DEBUG: Log the conversation history before LLM call
            tracing::debug!(%session_id, iteration, history_len = llm_conversation_history.len(), "About to call LLM with conversation history");
            for (i, msg) in llm_conversation_history.iter().enumerate() {
                tracing::debug!(%session_id, iteration, msg_index = i, role = %msg.role, parts_count = msg.parts.len(), "History message");
            }

            // If tools were successfully executed in the previous iteration and a summary was generated,
            // this iteration is for the LLM to respond to "What would you like to do next?".
            // We clear the flag here.
            // The actual summary sending and history modification for "What next?" happens *after* tool execution block.

            tracing::debug!(%session_id, iteration, "Calling LLM generate_stream");
            let llm_stream_result = self.llm_client.generate_stream(llm_conversation_history.clone()).await;

            let mut current_llm_text_response = String::new();
            let mut requested_tool_calls: Vec<ToolCall> = Vec::new();
            let mut llm_call_successful = false;
            let mut last_stream_error: Option<String> = None;
            let mut text_was_streamed = false; // Track if we streamed any text chunks

            match llm_stream_result {
                Ok(mut llm_stream) => {
                    llm_call_successful = true;
                    let stream_interaction_id = Uuid::new_v4();
                    tracing::info!(%session_id, %stream_interaction_id, "LLM stream initiated.");

                    while let Some(chunk_result) = llm_stream.next().await {
                        match chunk_result {
                            Ok(llm_chunk) => {
                                match llm_chunk {
                                    LlmStreamChunk::Text { content, is_final } => {
                                        tracing::debug!(%session_id, %stream_interaction_id, content_len = content.len(), is_final, "LLM text chunk received.");
                                        current_llm_text_response.push_str(&content);
                                        
                                        // Send the chunk directly to stream_handler for real-time streaming
                                        let engine_chunk = crate::streaming::StreamChunk {
                                            id: Uuid::new_v4(),
                                            data: content.into_bytes(),
                                            chunk_type: "text".to_string(),
                                            is_final,
                                            priority: 0,
                                            created_at: Instant::now(),
                                            metadata: HashMap::new(),
                                        };
                                        if stream_handler.handle_chunk(engine_chunk).await.is_err() {
                                            tracing::warn!(%session_id, %stream_interaction_id, "Stream handler failed to process text chunk.");
                                            llm_call_successful = false; 
                                            break;
                                        }
                                        text_was_streamed = true; // Mark that we streamed text
                                        
                                        if is_final && requested_tool_calls.is_empty() {
                                            // If it's final (from LLM perspective for this chunk) and no tools were called yet in this turn,
                                            // this might be the end of the LLM's output for this turn.
                                            break;
                                        }
                                    }
                                    LlmStreamChunk::ToolCall { tool_call, is_final } => {
                                        tracing::info!(%session_id, tool_name = %tool_call.name, "LLM requested tool call. Attempting to stream it.");
                                        requested_tool_calls.push(tool_call.clone()); // Clone for execution later

                                        // Serialize the tool_call for streaming
                                        match serde_json::to_vec(&tool_call) {
                                            Ok(tool_call_data) => {
                                                let engine_tool_call_chunk = crate::streaming::StreamChunk {
                                                    id: Uuid::new_v4(),
                                                    data: tool_call_data,
                                                    chunk_type: "tool_call".to_string(), // New chunk type
                                                    is_final, // Mirroring is_final from LlmStreamChunk
                                                    priority: 0, // Or higher if tool calls should be prioritized
                                                    created_at: Instant::now(),
                                                    metadata: HashMap::new(),
                                                };
                                                if stream_handler.handle_chunk(engine_tool_call_chunk).await.is_err() {
                                                    tracing::warn!(%session_id, %stream_interaction_id, tool_name = %tool_call.name, "Stream handler failed to process tool_call chunk.");
                                                    llm_call_successful = false;
                                                    // Potentially break here if streaming tool_call is critical
                                                } else {
                                                    tracing::debug!(%session_id, tool_name = %tool_call.name, "Successfully streamed tool_call chunk.");
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!(%session_id, tool_name = %tool_call.name, "Failed to serialize tool_call for streaming: {:?}", e);
                                                // Decide if this should halt the llm_call or just log
                                            }
                                        }
                                        
                                        if is_final { break; }
                                    }
                                    LlmStreamChunk::TokenUsage(usage_info) => {
                                        tracing::info!(%session_id, "Received token usage information from LLM stream: {:?}", usage_info);
                                        // Emit an event for this token usage
                                        if let Err(e) = event_emitter.emit_event(ReasoningEvent::TokenUsageReceived {
                                            session_id, 
                                            usage: usage_info, // usage_info is reasoning_engine::traits::TokenUsage
                                        }).await {
                                            tracing::warn!(%session_id, "Failed to emit TokenUsageReceived event: {:?}", e);
                                        }
                                        // This chunk type doesn't directly contribute to current_llm_text_response
                                        // or requested_tool_calls. It's metadata.
                                        // Continue to the next chunk.
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(%session_id, %stream_interaction_id, "LLM stream error: {:?}", e);
                                stream_handler.handle_stream_error(stream_interaction_id, e.clone()).await.ok();
                                llm_call_successful = false;
                                last_stream_error = Some(e.to_string());
                                break;
                            }
                        }
                    }
                    stream_handler.handle_stream_complete(stream_interaction_id).await.ok();
                    state.add_step(ReasoningStep::llm_interaction(
                        llm_conversation_history.last().map(|m| m.parts.iter().filter_map(|p| if let LlmMessagePart::Text(t) = p { Some(t.clone()) } else {None}).collect::<Vec<String>>().join("\n") ).unwrap_or_default(), 
                        current_llm_text_response.clone(), 
                        llm_call_successful, 
                        last_stream_error
                    ));
                }
                Err(e) => {
                    tracing::error!(%session_id, "Failed to initiate LLM stream: {:?}", e);
                    let error_msg = format!("LLM call failed: {:?}", e);
                    state.add_step(ReasoningStep::llm_interaction(
                        llm_conversation_history.last().map(|m| m.parts.iter().filter_map(|p| if let LlmMessagePart::Text(t) = p { Some(t.clone()) } else {None}).collect::<Vec<String>>().join("\n") ).unwrap_or_default(),
                        String::new(), 
                        false, 
                        Some(error_msg.clone())
                    ));
                    state.set_completed(false, error_msg);
                    break; // Break main loop
                }
            }

            // If the LLM call failed but we accumulated some text, send it to stream_handler.
            if !current_llm_text_response.is_empty() && !llm_call_successful {
                tracing::info!(%session_id, "Sending partial LLM text response due to stream error to stream_handler.");
                let partial_chunk = crate::streaming::StreamChunk {
                    id: Uuid::new_v4(),
                    data: current_llm_text_response.clone().into_bytes(),
                    chunk_type: "text".to_string(),
                    is_final: true, // Mark as final as the stream is ending here due to error
                    priority: 0,
                    created_at: Instant::now(),
                    metadata: HashMap::new(),
                };
                if stream_handler.handle_chunk(partial_chunk).await.is_err() {
                    tracing::warn!(%session_id, "Stream handler failed to process partial erroring chunk.");
                }
            }

            // Add assistant's text response to conversation history
            if !current_llm_text_response.is_empty() {
                llm_conversation_history.push(LlmMessage {
                    role: "assistant".to_string(),
                    parts: vec![LlmMessagePart::Text(current_llm_text_response.clone())],
                });
            }

            if !llm_call_successful {
                tracing::warn!(%session_id, iteration, "LLM call failed or stream error. Terminating loop.");
                state.set_completed(false, "LLM interaction failed".to_string());
                break;
            }

            if requested_tool_calls.is_empty() {
                // LLM provided text and/or no tool calls. This is where a summary of *previously* executed tools might be sent.
                let mut final_message_to_user_parts: Vec<String> = Vec::new();
                let mut will_prompt_llm_what_next = false;
                let mut nudge_performed_this_iteration = false;

                // Remove duplicate summary generation - summaries are now handled immediately after tool execution
                // if let Some(ref tools_result_to_summarize) = pending_tool_summary_info {
                //     if !tools_result_to_summarize.tool_results.is_empty() {
                //         let tool_summary_text = generate_tool_summary(&tools_result_to_summarize.tool_results);
                //         if !tool_summary_text.is_empty() {
                //             final_message_to_user_parts.push(tool_summary_text);
                //         }
                //         will_prompt_llm_what_next = true; 
                //     }
                //     pending_tool_summary_info = None; 
                // }
                
                let mut final_message_str = final_message_to_user_parts.join(" ");

                // Analyze intent of the LLM's text response to decide on loop continuation and "What next?"
                // CRITICAL FIX: Prevent duplicate intent analysis of the same content
                let should_analyze_intent = if let Some(ref last_content) = last_analyzed_content {
                    last_content != &current_llm_text_response
                } else {
                    true
                };
                
                let intent_analysis_result = if should_analyze_intent {
                    tracing::debug!(%session_id, iteration, "Analyzing intent for new content: '{}'", current_llm_text_response.chars().take(100).collect::<String>());
                    let result = self.intent_analyzer.analyze_intent(&current_llm_text_response, Some(&llm_conversation_history)).await;
                    last_analyzed_content = Some(current_llm_text_response.clone());
                    result
                } else {
                    tracing::debug!(%session_id, iteration, "Skipping duplicate intent analysis for same content");
                    // Return the same intent as before to avoid re-processing
                    Ok(DetectedIntent::RequestsMoreInput)
                };
                
                let mut loop_should_break = false;
                let mut break_reason = "LLM interaction concluded.".to_string();
                let mut current_intent_for_loop_break_logic: Option<DetectedIntent> = None;

                match intent_analysis_result {
                    Ok(intent_val) => {
                        current_intent_for_loop_break_logic = Some(intent_val.clone());
                        tracing::info!(%session_id, iteration, ?intent_val, "LLM text intent analyzed.");
                        match intent_val {
                            DetectedIntent::ProvidesFinalAnswer | DetectedIntent::StatesInabilityToProceed => {
                                loop_should_break = true;
                                break_reason = format!("LLM intent ({:?}) indicates completion.", intent_val);
                                will_prompt_llm_what_next = false; 
                            }
                            DetectedIntent::AsksClarifyingQuestion | DetectedIntent::RequestsMoreInput | DetectedIntent::GeneralConversation => {
                                will_prompt_llm_what_next = true; 
                                loop_should_break = iteration == self.config.max_iterations - 1;
                                if loop_should_break {
                                    break_reason = "Max iterations reached after conversational turn.".to_string();
                                } else {
                                   break_reason = format!("LLM intent ({:?}) indicates user input needed.", intent_val);
                                }
                            }
                            DetectedIntent::ProvidesPlanWithoutExplicitAction => {
                                if iteration < self.config.max_iterations - 1 {
                                    tracing::info!(%session_id, iteration, "LLM provided plan, nudging for tool call.");
                                    llm_conversation_history.push(LlmMessage {
                                        role: "user".to_string(),
                                        parts: vec![LlmMessagePart::Text(
                                            "Your plan is noted. Please proceed with the next action by making a tool call, or explicitly state that the task is fully complete if no further actions are needed.".to_string()
                                        )],
                                    });
                                    state.add_step(ReasoningStep::llm_interaction(
                                        llm_conversation_history.last().map(|m| m.parts.iter().filter_map(|p| if let LlmMessagePart::Text(t) = p { Some(t.clone()) } else {None}).collect::<Vec<String>>().join("\n") ).unwrap_or_default(),
                                        "[NUDGE SENT TO LLM - Based on Intent: ProvidesPlanWithoutExplicitAction]".to_string(),
                                        true,
                                        None
                                    ));
                                    will_prompt_llm_what_next = false;
                                    current_llm_text_response.clear();
                                    nudge_performed_this_iteration = true;
                                } else {
                                    tracing::warn!(%session_id, iteration, "Max iterations reached, cannot nudge LLM for plan execution.");
                                    loop_should_break = true;
                                    break_reason = "Max iterations reached after LLM provided plan.".to_string();
                                    will_prompt_llm_what_next = false;
                                }
                            }
                            DetectedIntent::Ambiguous => {
                                tracing::warn!(%session_id, iteration, "LLM response intent is ambiguous.");
                                loop_should_break = true;
                                break_reason = "LLM response intent ambiguous, requires clarification.".to_string();
                                will_prompt_llm_what_next = true;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(%session_id, iteration, "Intent analysis failed: {:?}. Terminating loop.", e);
                        loop_should_break = true;
                        break_reason = format!("Intent analysis failed: {}", e);
                        will_prompt_llm_what_next = false;
                    }
                }
                
                // "What would you like to do next?" logic block
                if will_prompt_llm_what_next {
                    let what_next_q = " What would you like to do next?".to_string();
                    
                    // final_message_str was built from final_message_to_user_parts.
                    // If a nudge cleared current_llm_text_response, that change is not yet reflected in final_message_str.
                    // This is acceptable for the llm_conversation_history, as the LLM should see its own plan text followed by the nudge.
                    // However, the user-facing message (if sent) should not contain the plan if a nudge occurred.

                    if !final_message_str.is_empty() && !final_message_str.ends_with(['.', '?', '!']) {
                        final_message_str.push('.');
                    }
                    final_message_str = format!("{} {}", final_message_str, what_next_q).trim().to_string();
                    
                    // Add "What next?" to LLM history for its *next* turn if the loop isn't breaking for other reasons.
                    if !loop_should_break || iteration < self.config.max_iterations - 1 {
                         llm_conversation_history.push(LlmMessage {
                            role: "user".to_string(), 
                            parts: vec![LlmMessagePart::Text(what_next_q.clone())],
                        });
                    }
                }

                // Guard the actual sending of the message to the user/stream
                if !nudge_performed_this_iteration {
                    // If a nudge was performed, current_llm_text_response (the plan) was cleared before this point.
                    // We need to construct the message to send to the user without the original plan text if a nudge happened.
                    // However, the !nudge_performed_this_iteration guard means we only enter here if no nudge happened.
                    // So, final_message_str as it was (potentially including what_next_q) is what we want to send.
                    if !final_message_str.is_empty() {
                        let final_chunk = crate::streaming::StreamChunk {
                            id: Uuid::new_v4(),
                            data: final_message_str.clone().into_bytes(), // Send the final_message_str
                            chunk_type: "text".to_string(),
                            is_final: true,
                            priority: 0,
                            created_at: Instant::now(),
                            metadata: HashMap::new(),
                        };
                        if stream_handler.handle_chunk(final_chunk).await.is_err() {
                            tracing::warn!(%session_id, "Stream handler failed to process final message chunk.");
                        }
                    }
                }
                
                // Determine if loop should break based on empty responses, considering nudge
                let effective_final_message_for_check = if nudge_performed_this_iteration { 
                    String::new() // If nudge, no message was sent to user, so treat as empty for this check
                } else { 
                    final_message_str.clone() // Otherwise, use what might have been sent
                };

                // If no nudge was performed in this iteration, and LLM gave no text, no tools, and no pending summary, then terminate.
                if !nudge_performed_this_iteration && current_llm_text_response.is_empty() && pending_tool_summary_info.is_none() && effective_final_message_for_check.is_empty() {
                    tracing::info!(%session_id, iteration, "LLM provided no text, no tools, and no pending summary (and no nudge performed). Terminating.");
                    loop_should_break = true;
                    break_reason = "LLM provided no further response or action.".to_string();
                }

                if loop_should_break {
                    let mut actual_success = !(break_reason.contains("Max iterations reached") || 
                                             break_reason.contains("Intent analysis failed") ||
                                             break_reason.contains("LLM provided no further response"));
                    
                    if break_reason == "Max iterations reached after conversational turn." {
                        if let Some(DetectedIntent::GeneralConversation) = current_intent_for_loop_break_logic {
                            actual_success = true;
                        }
                    }
                    state.set_completed(actual_success, break_reason); 
                    break;
                }

            } else {
              // Tool calls were requested, proceed to execute them
              tracing::debug!(%session_id, iteration, "LLM requested {} tool calls. Proceeding to execution.", requested_tool_calls.len());
              
              let tool_execution_requests: Vec<ToolExecutionRequest> = requested_tool_calls.into_iter()
                .map(|tc| ToolExecutionRequest::new(tc.name, tc.args))
                .collect();

              let current_orchestration_result = self.orchestrator.orchestrate_tools(
                  tool_execution_requests.clone(), 
                  tool_executor.clone(),
                  event_emitter.clone(),
              ).await?;

              let mut actual_orchestration_success = current_orchestration_result.success;
              if actual_orchestration_success {
                  for (_tool_name, tool_exec_result) in &current_orchestration_result.tool_results {
                      if let Some(res) = &tool_exec_result.result {
                          if !res.success {
                              actual_orchestration_success = false;
                              tracing::warn!(%session_id, iteration, tool_name = %tool_exec_result.request.tool_name, "Tool reported failure despite overall orchestration success claim.");
                              break;
                          }
                      } else {
                          actual_orchestration_success = false;
                          tracing::warn!(%session_id, iteration, tool_name = %tool_exec_result.request.tool_name, "Tool has no result object, implying failure.");
                          break;
                      }
                  }
              }

              let mut corrected_orchestration_result = current_orchestration_result;
              corrected_orchestration_result.success = actual_orchestration_success;

              state.add_step(ReasoningStep::from_orchestration_result(&corrected_orchestration_result, Some(&format!("Iteration {}", iteration))));
            
              let tool_results_for_llm: Vec<LlmMessagePart> = corrected_orchestration_result.tool_results.iter().map(|(name, exec_result)| {
                  let result_content = serde_json::json!({
                      "tool_name": name,
                      "success": exec_result.result.as_ref().map_or(false, |r| r.success),
                      "data": exec_result.result.as_ref().map_or(Value::Null, |r| r.data.clone()),
                      "error": exec_result.result.as_ref().and_then(|r| r.error.clone()),
                  });
                  LlmMessagePart::Text(serde_json::to_string(&result_content).unwrap_or_else(|_| format!("{{\\\"tool_name\\\": \\\"{}\\\", \\\"error\\\": \\\"Serialization failed\\\"}}", name)))
              }).collect();

              if !tool_results_for_llm.is_empty() {
                   llm_conversation_history.push(LlmMessage {
                      role: "user".to_string(), 
                      parts: tool_results_for_llm,
                  });
              }

              if !actual_orchestration_success {
                  tracing::warn!(%session_id, iteration, "Tool orchestration failed (verified). Preparing to inform LLM.");
                  // Even for failed tools, we should generate a summary so the user knows what failed
                  if !corrected_orchestration_result.tool_results.is_empty() {
                      pending_tool_summary_info = Some(corrected_orchestration_result);
                  } else {
                      pending_tool_summary_info = None;
                  }
                  // DO NOT break here, let the loop continue so LLM can see the tool error
              } else {
                  if !corrected_orchestration_result.tool_results.is_empty() {
                    pending_tool_summary_info = Some(corrected_orchestration_result);
                  } else {
                    pending_tool_summary_info = None;
                  }
              }
            }

            // If we have pending tool summary info, generate and stream the summary
            if let Some(ref tool_summary_orchestration_result) = pending_tool_summary_info {
                let summary_text = generate_tool_summary(&tool_summary_orchestration_result.tool_results);
                if !summary_text.is_empty() {
                    tracing::info!(%session_id, iteration, summary_len = summary_text.len(), "Generated tool summary. Emitting Summary event.");
                    
                    // Emit Summary event - this will be converted to LlmChunk by AgentEventEmitter
                    event_emitter.emit_event(ReasoningEvent::Summary {
                        session_id,
                        content: summary_text.clone(),
                        timestamp: chrono::Utc::now(),
                    }).await?;
                    
                    // Remove duplicate StreamChunk emission - Summary event is sufficient
                    // The AgentEventEmitter will convert the Summary event to an LlmChunk
                    // and the AgentStreamHandler will handle summary chunks properly
                }
                pending_tool_summary_info = None; // Clear after processing
            }
        }

        event_emitter.emit_event(ReasoningEvent::SessionCompleted {
            session_id,
            success: state.is_successful(), 
            total_duration_ms: Utc::now().signed_duration_since(state.created_at).num_milliseconds().abs() as u64,
            steps_executed: state.history.len() as u32,
            tools_used: state.history.iter().flat_map(|s| s.tools_used.clone()).collect::<std::collections::HashSet<_>>().into_iter().collect(),
        }).await?;

        tracing::info!(%session_id, success = state.is_successful(), "Reasoning session completed.");
        
        Ok(state)
    }
    
    /// Start a reasoning session with the given input (legacy method)
    pub async fn reason(&self, input: &str) -> Result<String> {
        tracing::info!("Starting reasoning session with input: {}", input);
        
        // This is a placeholder for backward compatibility
        // Users should migrate to the new `process` method for full functionality
        Ok(format!("Processed: {}", input))
    }
    
    /// Get the current configuration
    pub fn config(&self) -> &ReasoningConfig {
        &self.config
    }
    
    /// Get orchestration metrics
    pub async fn get_orchestration_metrics(&self) -> crate::orchestration::OrchestrationMetrics {
        self.orchestrator.get_metrics().await
    }
    
    /// Get active orchestrations
    pub async fn get_active_orchestrations(&self) -> Vec<Uuid> {
        self.orchestrator.get_active_orchestrations().await
    }
}

// Helper function to generate a summary from tool results
// This is a basic implementation. It can be made more sophisticated.
fn generate_tool_summary(tool_results: &HashMap<String, crate::orchestration::ToolExecutionResult>) -> String {
    if tool_results.is_empty() {
        return String::new(); // Return empty if no tools, so "What next?" is the only message.
    }

    let mut successful_summaries = Vec::new();
    let mut failed_summaries = Vec::new();

    for (name, exec_result) in tool_results {
        if let Some(res_data) = exec_result.result.as_ref() {
            if res_data.success {
                let tool_specific_summary = match name.as_str() {
                    "add_repository" => {
                        // Extract repository name from the result data
                        if let Some(repo_name) = res_data.data.get("name").and_then(|v| v.as_str()) {
                            format!("repository '{}' was added", repo_name)
                        } else if let Some(repo_url) = res_data.data.get("url").and_then(|v| v.as_str()) {
                            format!("repository from '{}' was added", repo_url)
                        } else {
                            "repository was added".to_string()
                        }
                    },
                    "sync_repository" => {
                        if let Some(repo_name) = res_data.data.get("name").and_then(|v| v.as_str()) {
                            format!("repository '{}' was synced", repo_name)
                        } else {
                            "repository was synced".to_string()
                        }
                    },
                    "web_search" => {
                        if let Some(search_term) = res_data.data.get("search_term").and_then(|v| v.as_str()) {
                            format!("web search for '{}' completed", search_term)
                        } else {
                            "web search completed".to_string()
                        }
                    },
                    "search_code" => {
                        if let Some(query) = res_data.data.get("query").and_then(|v| v.as_str()) {
                            format!("code search for '{}' completed", query)
                        } else {
                            "code search completed".to_string()
                        }
                    },
                    "repository_map" => {
                        if let Some(repo_name) = res_data.data.get("repository_name").and_then(|v| v.as_str()) {
                            format!("repository map for '{}' generated", repo_name)
                        } else {
                            "repository map generated".to_string()
                        }
                    },
                    _ => format!("'{}' completed successfully", name),
                };
                successful_summaries.push(tool_specific_summary);
            } else {
                failed_summaries.push(format!("'{}' failed: {}", name, res_data.error.as_deref().unwrap_or("Unknown error")));
            }
        } else {
            failed_summaries.push(format!("'{}' provided no result data", name));
        }
    }

    let mut parts = Vec::new();
    if !successful_summaries.is_empty() {
        parts.push(format!("Successfully completed: {}", successful_summaries.join(", ")));
    }
    if !failed_summaries.is_empty() {
        parts.push(format!("Failed actions: {}", failed_summaries.join(", ")));
    }

    if parts.is_empty() {
        // Should not happen if tool_results was not empty, but as a safeguard
        "The requested actions were processed.\n\n".to_string()
    } else {
        // Add proper spacing and newlines to prevent concatenation issues
        format!("Okay, I've finished those tasks. {}\n\n", parts.join(". "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering, AtomicBool};
    use futures_util::stream;
    use crate::traits::ToolCall;
    use std::sync::Arc;
    use serde_json::json;
    use std::time::Duration;

    // --- Mock IntentAnalyzer for tests ---
    use crate::traits::{IntentAnalyzer, DetectedIntent, LlmMessage as ReasoningLlmMessage};
    use async_trait::async_trait;

    #[derive(Debug)]
    struct MockIntentAnalyzer {
        response_intent: DetectedIntent,
    }

    impl MockIntentAnalyzer {
        fn new(intent: DetectedIntent) -> Self {
            Self { response_intent: intent }
        }
    }

    #[async_trait]
    impl IntentAnalyzer for MockIntentAnalyzer {
        async fn analyze_intent(
            &self,
            _text: &str, 
            _conversation_context: Option<&[ReasoningLlmMessage]>,
        ) -> Result<DetectedIntent> {
            Ok(self.response_intent.clone())
        }
    }
    // --- End Mock IntentAnalyzer ---

    // --- Stateful Mock IntentAnalyzer for specific tests ---
    #[derive(Debug)]
    struct StatefulMockIntentAnalyzer {
        call_count: AtomicUsize,
        intents_sequence: Vec<DetectedIntent>,
        default_intent: DetectedIntent,
    }

    impl StatefulMockIntentAnalyzer {
        fn new(intents_sequence: Vec<DetectedIntent>, default_intent: DetectedIntent) -> Self {
            Self { 
                call_count: AtomicUsize::new(0), 
                intents_sequence, 
                default_intent 
            }
        }
    }

    #[async_trait]
    impl IntentAnalyzer for StatefulMockIntentAnalyzer {
        async fn analyze_intent(&self, _text: &str, _conversation_context: Option<&[ReasoningLlmMessage]>) -> Result<DetectedIntent> {
            let count = self.call_count.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(self.intents_sequence.get(count).cloned().unwrap_or_else(|| self.default_intent.clone()))
        }
    }
    // --- End Stateful Mock IntentAnalyzer ---

    // Mock LlmClient for tests
    struct MockLlmClient {
        simulated_responses: Vec<Vec<Result<LlmStreamChunk>>>,
        response_index: AtomicUsize,
    }

    impl MockLlmClient {
        fn new(responses: Vec<Vec<Result<LlmStreamChunk>>>) -> Self {
            Self {
                simulated_responses: responses,
                response_index: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl LlmClient for MockLlmClient {
        async fn generate_stream(
            &self, 
            _messages: Vec<LlmMessage>,
        ) -> Result<std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<LlmStreamChunk>> + Send>>> {
            let index = self.response_index.fetch_add(1, AtomicOrdering::SeqCst);
            if index < self.simulated_responses.len() {
                let chunks = self.simulated_responses[index].clone();
                let stream = stream::iter(chunks.into_iter().map(|res_chunk| res_chunk.map_err(|e| ReasoningError::LlmError { message: e.to_string() })));
                Ok(Box::pin(stream))
            } else {
                let stream = stream::iter(vec![Ok(LlmStreamChunk::Text { content: "Default mock response".to_string(), is_final: true })]);
                Ok(Box::pin(stream))
            }
        }
    }

    // Mock implementations for testing
    struct MockToolExecutor {
        call_count: AtomicUsize,
        fail_tool_named: Option<String>,
    }

    impl MockToolExecutor {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
                fail_tool_named: None,
            }
        }

        fn with_failure_for(name: &str) -> Self {
            Self {
                call_count: AtomicUsize::new(0),
                fail_tool_named: Some(name.to_string()),
            }
        }
        
        fn get_call_count(&self) -> usize {
            self.call_count.load(AtomicOrdering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl ToolExecutor for MockToolExecutor {
        async fn execute_tool(&self, name: &str, args: serde_json::Value) -> Result<ToolResult> {
            self.call_count.fetch_add(1, AtomicOrdering::SeqCst);
            if let Some(fail_name) = &self.fail_tool_named {
                if name == fail_name {
                    return Ok(ToolResult::failure("Simulated tool failure".to_string(), 50));
                }
            }
            if name == "analyze_input" {
                 return Ok(ToolResult::success(
                    serde_json::json!({"processed_input": "analyzed: "}),
                    100
                ));
            }
            Ok(ToolResult::success(
                serde_json::json!({"tool": name, "args": args, "result": "success"}),
                100
            ))
        }

        async fn get_available_tools(&self) -> Result<Vec<ToolDefinition>> {
            Ok(vec![
                ToolDefinition {
                    name: "analyze_input".to_string(),
                    description: "Analyze input text".to_string(),
                    parameters: serde_json::json!({}),
                    is_required: false,
                    category: None,
                    estimated_duration_ms: Some(100),
                },
                ToolDefinition {
                    name: "another_tool".to_string(),
                    description: "Another tool for testing".to_string(),
                    parameters: serde_json::json!({}),
                    is_required: false,
                    category: None,
                    estimated_duration_ms: Some(50),
                },
            ])
        }
    }

    struct MockEventEmitter {
        events: Arc<tokio::sync::Mutex<Vec<ReasoningEvent>>>
    }
    impl MockEventEmitter {
        fn new() -> Self { Self { events: Arc::new(tokio::sync::Mutex::new(Vec::new())) } }
        async fn get_events(&self) -> Vec<ReasoningEvent> { self.events.lock().await.clone() }
    }

    #[async_trait::async_trait]
    impl EventEmitter for MockEventEmitter {
        async fn emit_event(&self, event: ReasoningEvent) -> Result<()> {
            self.events.lock().await.push(event);
            Ok(())
        }
    }

    struct MockStreamHandler {
        chunks: Arc<tokio::sync::Mutex<Vec<String>>>,
        completed: Arc<AtomicBool>,
        errors: Arc<AtomicUsize>,
    }
    impl MockStreamHandler {
        fn new() -> Self {
            Self {
                chunks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
                completed: Arc::new(AtomicBool::new(false)),
                errors: Arc::new(AtomicUsize::new(0)),
            }
        }
        async fn get_chunks_as_string(&self) -> String {
            self.chunks.lock().await.join("")
        }
    }

    #[async_trait::async_trait]
    impl StreamHandler for MockStreamHandler {
        async fn handle_chunk(&self, chunk: StreamChunk) -> Result<()> {
            self.chunks.lock().await.push(String::from_utf8(chunk.data).unwrap_or_default());
            Ok(())
        }

        async fn handle_stream_complete(&self, _stream_id: Uuid) -> Result<()> {
            self.completed.store(true, AtomicOrdering::SeqCst);
            Ok(())
        }

        async fn handle_stream_error(&self, _stream_id: Uuid, _error: ReasoningError) -> Result<()> {
            self.errors.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        }
    }

    fn default_config_for_test() -> ReasoningConfig {
        let mut config = ReasoningConfig::default();
        config
    }

    #[tokio::test]
    async fn test_reasoning_engine_creation() {
        let config = default_config_for_test();
        let llm_client = Arc::new(MockLlmClient::new(vec![]));
        let intent_analyzer = Arc::new(MockIntentAnalyzer::new(DetectedIntent::GeneralConversation));
        let result = ReasoningEngine::new(config, llm_client, intent_analyzer).await;
        
        assert!(result.is_ok());
        let engine = result.unwrap();
        assert_eq!(engine.config().max_iterations, 50);
    }

    #[tokio::test]
    async fn test_single_llm_response_no_tools() {
        let config = default_config_for_test();
        let llm_responses = vec![
            vec![ 
                Ok(LlmStreamChunk::Text { content: "Final answer.".to_string(), is_final: true }),
            ]
        ];
        let llm_client = Arc::new(MockLlmClient::new(llm_responses));
        let intent_analyzer = Arc::new(MockIntentAnalyzer::new(DetectedIntent::ProvidesFinalAnswer)); 
        let mut engine = ReasoningEngine::new(config, llm_client, intent_analyzer).await.unwrap();
        
        let tool_executor = Arc::new(MockToolExecutor::new());
        let event_emitter = Arc::new(MockEventEmitter::new());
        let stream_handler = Arc::new(MockStreamHandler::new());
        
        let result_state = engine.process(
            vec![ LlmMessage { role: "user".to_string(), parts: vec![LlmMessagePart::Text("Test input for direct answer".to_string())]} ],
            tool_executor.clone(), event_emitter.clone(), stream_handler.clone(),
        ).await.unwrap();
        
        assert!(result_state.is_successful());
        assert_eq!(tool_executor.get_call_count(), 1); // For analyze_input
        // Streamed output should only be the direct LLM response, no duplication from RE sending final_message_str.
        assert_eq!(stream_handler.get_chunks_as_string().await, "Final answer."); 
        assert!(stream_handler.completed.load(AtomicOrdering::SeqCst));
        // History: analyze_input, llm_interaction
        assert_eq!(result_state.history.len(), 2, "History check. Got: {:?}", result_state.history.iter().map(|s| format!("Type: {:?}, Tools: {:?}, Output: {:?}", s.step_type, s.tools_used, s.output)).collect::<Vec<_>>());
        let last_step = result_state.history.last().unwrap();
        assert_eq!(last_step.step_type, state::StepType::LlmCall);
        assert!(last_step.success);
    }

    #[tokio::test]
    async fn test_llm_requests_one_tool_then_answers() {
        let mut config = default_config_for_test();
        config.max_iterations = 5;
        let tool_call_to_request = ToolCall { name: "another_tool".to_string(), args: serde_json::json!({ "param": "value" }) };
        let llm_responses = vec![
            vec![
                Ok(LlmStreamChunk::Text { content: "Okay, I need to use a tool. ".to_string(), is_final: false }),
                Ok(LlmStreamChunk::ToolCall { 
                    tool_call: tool_call_to_request.clone(),
                    is_final: true, 
                }),
            ],
            vec![
                Ok(LlmStreamChunk::Text { content: "Tool executed. Final answer.".to_string(), is_final: true }),
            ]
        ];
        let llm_client = Arc::new(MockLlmClient::new(llm_responses));
        let intent_analyzer = Arc::new(MockIntentAnalyzer::new(DetectedIntent::ProvidesFinalAnswer)); 
        let mut engine = ReasoningEngine::new(config, llm_client, intent_analyzer).await.unwrap();

        let tool_executor = Arc::new(MockToolExecutor::new());
        let event_emitter = Arc::new(MockEventEmitter::new());
        let stream_handler = Arc::new(MockStreamHandler::new());

        let result_state = engine.process(
            vec![
                LlmMessage {
                    role: "user".to_string(),
                    parts: vec![LlmMessagePart::Text("Test input for tool use".to_string())],
                },
                // Simulate history correctly for multi-turn
                LlmMessage {
                    role: "assistant".to_string(),
                    parts: vec![LlmMessagePart::Text("Okay, I need to use a tool. ".to_string())],
                },
                LlmMessage {
                    role: "assistant".to_string(), // Corrected: LLM sends tool call, so role is assistant
                    parts: vec![LlmMessagePart::ToolCall(tool_call_to_request.clone())]
                },
                // Tool Result (as if from user/system informing LLM of tool output)
                LlmMessage {
                    role: "user".to_string(), // Tool results are fed back as user role to LLM in current RE logic
                    parts: vec![LlmMessagePart::Text(serde_json::to_string(&json!({
                        "tool_name": "another_tool",
                        "success": true,
                        "data": {"tool": "another_tool", "args": {"param":"value"}, "result": "success"},
                        "error": null
                    })).unwrap())]
                },
            ],
            tool_executor.clone(),
            event_emitter.clone(),
            stream_handler.clone(),
        ).await.unwrap();

        assert!(result_state.is_successful(), "Reasoning should be successful. Final state: {:?}", result_state);
        // analyze_input + another_tool
        assert_eq!(tool_executor.get_call_count(), 2, "Expected 2 tool executions (analyze_input + another_tool)"); 
        
        let initial_llm_text = "Okay, I need to use a tool. ";
        let serialized_tool_call = serde_json::to_string(&tool_call_to_request).unwrap();
        let subsequent_llm_text = "Tool executed. Final answer.";
        // The actual streaming order is now: LLM text + tool call + LLM text (summary is emitted as Summary event, not StreamChunk)
        let expected_stream_output = format!("{}{}{}", initial_llm_text, serialized_tool_call, subsequent_llm_text);
        
        assert_eq!(stream_handler.get_chunks_as_string().await.trim(), expected_stream_output.trim(), "Streamed output mismatch");
        // state.history: analyze_input, llm_interaction (text+TC), tool_execution (another_tool), llm_interaction (loop2 text)
        assert_eq!(result_state.history.len(), 4, "State history should have 4 steps. Got: {:?}", result_state.history.iter().map(|s| format!("Type: {:?}, Tools: {:?}, Output: {:?}", s.step_type, s.tools_used, s.output)).collect::<Vec<_>>()); 
    }

    #[tokio::test]
    async fn test_reasoning_engine_legacy_reason() {
        let config = default_config_for_test();
        let llm_client = Arc::new(MockLlmClient::new(vec![]));
        let intent_analyzer = Arc::new(MockIntentAnalyzer::new(DetectedIntent::GeneralConversation));
        let engine = ReasoningEngine::new(config, llm_client, intent_analyzer).await.unwrap();
        
        let result = engine.reason("Test input").await.unwrap();
        assert!(result.contains("Test input"));
    }
    
    #[tokio::test]
    async fn test_reasoning_engine_metrics() {
        let config = default_config_for_test();
        let llm_client = Arc::new(MockLlmClient::new(vec![]));
        let intent_analyzer = Arc::new(MockIntentAnalyzer::new(DetectedIntent::GeneralConversation));
        let engine = ReasoningEngine::new(config, llm_client, intent_analyzer).await.unwrap();
        
        let metrics = engine.get_orchestration_metrics().await;
        assert_eq!(metrics.total_orchestrations, 0);
        
        let active = engine.get_active_orchestrations().await;
        assert!(active.is_empty());
    }

    #[tokio::test]
    async fn test_max_iterations_reached() {
        let mut config = default_config_for_test();
        config.max_iterations = 2; 
        let first_tool_call = ToolCall { name: "another_tool".to_string(), args: serde_json::json!({}) };

        let llm_responses = vec![
            vec![
                Ok(LlmStreamChunk::Text { content: "Loop 1. Requesting tool. ".to_string(), is_final: false }),
                Ok(LlmStreamChunk::ToolCall { 
                    tool_call: first_tool_call.clone(),
                    is_final: true,
                }),
            ],
            vec![
                Ok(LlmStreamChunk::Text { content: "Loop 2. Still thinking.".to_string(), is_final: true }), 
            ],
        ];
        let llm_client = Arc::new(MockLlmClient::new(llm_responses));
        let intent_analyzer = Arc::new(MockIntentAnalyzer::new(DetectedIntent::ProvidesPlanWithoutExplicitAction)); 
        let mut engine = ReasoningEngine::new(config, llm_client, intent_analyzer).await.unwrap();

        let tool_executor = Arc::new(MockToolExecutor::new());
        let event_emitter = Arc::new(MockEventEmitter::new());
        let stream_handler = Arc::new(MockStreamHandler::new());

        let result_state = engine.process(
            vec![ LlmMessage { role: "user".to_string(), parts: vec![LlmMessagePart::Text("User initiates a task that will loop".to_string())]} ],
            tool_executor.clone(), event_emitter.clone(), stream_handler.clone(),
        ).await.unwrap();

        assert!(!result_state.is_successful());
        assert_eq!(result_state.completion_reason, Some("Max iterations reached after LLM provided plan.".to_string()));
        assert_eq!(tool_executor.get_call_count(), 2); 
        
        let initial_llm_text_loop1 = "Loop 1. Requesting tool. ";
        let serialized_tool_call_loop1 = serde_json::to_string(&first_tool_call).unwrap();
        let llm_text_loop2 = "Loop 2. Still thinking.";
        
        // The actual streaming order is now: LLM text + tool call + LLM text (summary is emitted as Summary event, not StreamChunk)
        let expected_stream_output = format!("{}{}{}", initial_llm_text_loop1, serialized_tool_call_loop1, llm_text_loop2);
        
        assert_eq!(stream_handler.get_chunks_as_string().await.trim(), expected_stream_output.trim(), "Streamed output mismatch for max_iterations test");
        assert_eq!(result_state.history.len(), 4, "State history: analyze_input, llm_iter1(text+TC), exec(another_tool), llm_iter2(text). Got: {:?}", result_state.history.iter().map(|s| (s.step_type.clone(), s.reasoning.clone())).collect::<Vec<_>>()); 
    }

    #[tokio::test]
    async fn test_tool_execution_failure_in_loop() {
        let config = default_config_for_test();
        let faulty_tool_call = ToolCall { name: "faulty_tool".to_string(), args: serde_json::json!({}) };

        let llm_responses = vec![
            // LLM Call 1: Request the faulty tool
            vec![
                Ok(LlmStreamChunk::Text { content: "I will now attempt to use the 'faulty_tool'. ".to_string(), is_final: false }),
                Ok(LlmStreamChunk::ToolCall { 
                    tool_call: faulty_tool_call.clone(),
                    is_final: true,
                }),
            ],
            // LLM Call 2: LLM acknowledges the tool failure
            vec![
                Ok(LlmStreamChunk::Text { content: "It appears the 'faulty_tool' encountered an error: Simulated tool failure. I cannot proceed with that specific action.".to_string(), is_final: true }),
            ]
        ];
        let llm_client = Arc::new(MockLlmClient::new(llm_responses));
        // If LLM states inability to proceed, it should be a successful session completion (agent did its job of reporting)
        let intent_analyzer = Arc::new(MockIntentAnalyzer::new(DetectedIntent::StatesInabilityToProceed)); 
        let mut engine = ReasoningEngine::new(config, llm_client, intent_analyzer).await.unwrap();

        let tool_executor = Arc::new(MockToolExecutor::with_failure_for("faulty_tool"));
        let event_emitter = Arc::new(MockEventEmitter::new());
        let stream_handler = Arc::new(MockStreamHandler::new());

        let result_state = engine.process(
            vec![
                LlmMessage {
                    role: "user".to_string(),
                    parts: vec![LlmMessagePart::Text("Please use the faulty_tool.".to_string())],
                },
            ],
            tool_executor.clone(),
            event_emitter.clone(),
            stream_handler.clone(),
        ).await.unwrap();

        // The overall session should be considered successful if the agent correctly reports the failure.
        // The completion reason will come from the LLM's final statement.
        assert!(result_state.is_successful(), "Session should be successful as LLM handled the error. State: {:?}", result_state);
        assert_eq!(result_state.completion_reason, Some("LLM intent (StatesInabilityToProceed) indicates completion.".to_string()));
        
        // analyze_input + faulty_tool (attempted)
        assert_eq!(tool_executor.get_call_count(), 2, "Expected 2 tool executions (analyze_input + faulty_tool attempt)");
        
        let llm_text_1 = "I will now attempt to use the 'faulty_tool'. ";
        let serialized_tool_call = serde_json::to_string(&faulty_tool_call).unwrap();
        let llm_text_2_error_summary = "It appears the 'faulty_tool' encountered an error: Simulated tool failure. I cannot proceed with that specific action.";
        
        // The actual streaming order is now: LLM text + tool call + LLM text (summary is emitted as Summary event, not StreamChunk)
        let expected_stream_output = format!("{}{}{}", llm_text_1, serialized_tool_call, llm_text_2_error_summary);

        assert_eq!(stream_handler.get_chunks_as_string().await.trim(), expected_stream_output.trim(), "Streamed output mismatch");

        // Check the history for the execution of faulty_tool
        // state.history: analyze_input, llm_interaction (text+TC), tool_execution (faulty_tool), llm_interaction (error summary)
        assert_eq!(result_state.history.len(), 4, "State history should have 4 steps. Got: {:?}", result_state.history.iter().map(|s| format!("Type: {:?}, Tools: {:?}, Output: {:?}", s.step_type, s.tools_used, s.output)).collect::<Vec<_>>());

        let tool_exec_step = result_state.history.iter().find(|s| 
            s.step_type == state::StepType::Execute && 
            s.tools_used.iter().any(|tool_name| tool_name == "faulty_tool")
        );
        assert!(tool_exec_step.is_some(), "No execution step found for faulty_tool that used 'faulty_tool'");
        assert!(!tool_exec_step.unwrap().success, "Execution step for faulty_tool should indicate failure.");
    }

    #[tokio::test]
    async fn test_summary_event_emission() {
        let config = default_config_for_test();
        let tool_call_to_request = ToolCall { name: "another_tool".to_string(), args: serde_json::json!({ "param": "value" }) };
        let llm_responses = vec![
            vec![
                Ok(LlmStreamChunk::Text { content: "I'll use a tool. ".to_string(), is_final: false }),
                Ok(LlmStreamChunk::ToolCall { 
                    tool_call: tool_call_to_request.clone(),
                    is_final: true, 
                }),
            ],
            vec![
                Ok(LlmStreamChunk::Text { content: "Tool completed.".to_string(), is_final: true }),
            ]
        ];
        let llm_client = Arc::new(MockLlmClient::new(llm_responses));
        let intent_analyzer = Arc::new(MockIntentAnalyzer::new(DetectedIntent::ProvidesFinalAnswer)); 
        let mut engine = ReasoningEngine::new(config, llm_client, intent_analyzer).await.unwrap();

        let tool_executor = Arc::new(MockToolExecutor::new());
        let event_emitter = Arc::new(MockEventEmitter::new());
        let stream_handler = Arc::new(MockStreamHandler::new());

        let _result_state = engine.process(
            vec![LlmMessage {
                role: "user".to_string(),
                parts: vec![LlmMessagePart::Text("Test tool execution with summary".to_string())],
            }],
            tool_executor.clone(),
            event_emitter.clone(),
            stream_handler.clone(),
        ).await.unwrap();

        // Check that a Summary event was emitted
        let events = event_emitter.get_events().await;
        let summary_events: Vec<_> = events.iter().filter(|e| matches!(e, ReasoningEvent::Summary { .. })).collect();
        
        assert!(!summary_events.is_empty(), "Expected at least one Summary event to be emitted");
        
        if let ReasoningEvent::Summary { content, .. } = &summary_events[0] {
            assert!(content.contains("Okay, I've finished those tasks"), "Summary should contain expected text");
            assert!(content.contains("another_tool"), "Summary should mention the executed tool");
        } else {
            panic!("Expected Summary event");
        }
    }

    #[tokio::test]
    async fn test_summary_chunk_metadata() {
        let config = default_config_for_test();
        let tool_call_to_request = ToolCall { name: "another_tool".to_string(), args: serde_json::json!({ "param": "value" }) };
        let llm_responses = vec![
            vec![
                Ok(LlmStreamChunk::ToolCall { 
                    tool_call: tool_call_to_request.clone(),
                    is_final: true, 
                }),
            ],
            vec![
                Ok(LlmStreamChunk::Text { content: "Done.".to_string(), is_final: true }),
            ]
        ];
        let llm_client = Arc::new(MockLlmClient::new(llm_responses));
        let intent_analyzer = Arc::new(MockIntentAnalyzer::new(DetectedIntent::ProvidesFinalAnswer)); 
        let mut engine = ReasoningEngine::new(config, llm_client, intent_analyzer).await.unwrap();

        let tool_executor = Arc::new(MockToolExecutor::new());
        let event_emitter = Arc::new(MockEventEmitter::new());
        
        // Custom stream handler to capture chunk metadata
        struct MetadataCapturingStreamHandler {
            chunks_with_metadata: Arc<tokio::sync::Mutex<Vec<(String, HashMap<String, String>)>>>,
        }
        impl MetadataCapturingStreamHandler {
            fn new() -> Self {
                Self {
                    chunks_with_metadata: Arc::new(tokio::sync::Mutex::new(Vec::new())),
                }
            }
            async fn get_summary_chunks(&self) -> Vec<(String, HashMap<String, String>)> {
                self.chunks_with_metadata.lock().await.clone()
            }
        }
        #[async_trait::async_trait]
        impl StreamHandler for MetadataCapturingStreamHandler {
            async fn handle_chunk(&self, chunk: StreamChunk) -> Result<()> {
                let content = String::from_utf8(chunk.data).unwrap_or_default();
                self.chunks_with_metadata.lock().await.push((content, chunk.metadata));
                Ok(())
            }
            async fn handle_stream_complete(&self, _stream_id: Uuid) -> Result<()> { Ok(()) }
            async fn handle_stream_error(&self, _stream_id: Uuid, _error: ReasoningError) -> Result<()> { Ok(()) }
        }
        
        let stream_handler = Arc::new(MetadataCapturingStreamHandler::new());

        let _result_state = engine.process(
            vec![LlmMessage {
                role: "user".to_string(),
                parts: vec![LlmMessagePart::Text("Test summary metadata".to_string())],
            }],
            tool_executor.clone(),
            event_emitter.clone(),
            stream_handler.clone(),
        ).await.unwrap();

        // Check that Summary events were emitted (not StreamChunks)
        let events = event_emitter.get_events().await;
        let summary_events: Vec<_> = events.iter().filter(|e| matches!(e, ReasoningEvent::Summary { .. })).collect();
        
        assert!(!summary_events.is_empty(), "Expected at least one Summary event to be emitted");
        
        if let ReasoningEvent::Summary { content, .. } = &summary_events[0] {
            assert!(content.contains("Okay, I've finished those tasks"), "Summary should contain expected text");
            assert!(content.contains("another_tool"), "Summary should mention the executed tool");
        } else {
            panic!("Expected Summary event");
        }
    }
}

