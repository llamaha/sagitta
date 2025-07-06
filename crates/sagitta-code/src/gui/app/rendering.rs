// UI rendering for the Sagitta Code application

use egui::{Context, Key, TopBottomPanel, Frame, Vec2};
use crate::gui::app::AppEvent;
use crate::gui::conversation::sidebar::OrganizationMode;
use super::SagittaCodeApp;
use super::super::chat::input::chat_input_ui;
use super::super::chat::view::modern_chat_view_ui;
use super::super::theme::{AppTheme, apply_theme};
use crate::utils::logging::LOG_COLLECTOR;
use futures_util::StreamExt;
use super::panels::ActivePanel;

/// Main rendering function for the application
pub fn render(app: &mut SagittaCodeApp, ctx: &Context) {
    // Apply theme to the entire application
    apply_theme(ctx, app.state.current_theme);

    // Handle keyboard shortcuts
    handle_keyboard_shortcuts(app, ctx);
    
    // Handle CLAUDE.md modal shortcuts
    app.claude_md_modal.handle_shortcuts(ctx);

    // Process agent events
    app.process_agent_events();
    
    // Process conversation events
    app.process_conversation_events();

    // Process app events
    app.process_app_events();
    
    // Update git controls repository context
    update_git_controls_repository_context(app);
    
    // Refresh conversation clusters periodically (every 5 minutes)
    refresh_clusters_periodically(app);
    
    // Refresh repository list periodically (every 30 seconds)
    refresh_repository_list_periodically(app);
    
    // Handle temporary thinking indicator timeout (3 seconds)
    if let Some(start_time) = app.state.thinking_start_time {
        if start_time.elapsed() > std::time::Duration::from_secs(3) {
            app.state.thinking_message = None;
            app.state.thinking_start_time = None;
        }
    }
    
    // Update current agent mode (periodically)
    if let Some(agent) = &app.agent {
        let _agent_clone = agent.clone();
        let _current_mode = app.state.current_agent_mode;
        tokio::spawn(async move {
            // This will run async and update the mode if needed
            // We can't directly update self here, but we could use events if needed
        });
    }
    
    // Handle clicked tool info - process once then clear to prevent repeated execution
    if let Some((tool_name, tool_args)) = app.state.clicked_tool_info.take() {
        if tool_name == "__CANCEL_TOOL__" {
            // Handle tool cancellation
            if let Ok(run_id) = uuid::Uuid::parse_str(&tool_args) {
                if let Err(e) = app.app_event_sender.send(AppEvent::CancelTool(run_id)) {
                    log::error!("Failed to send CancelTool event: {e}");
                }
            }
            // clicked_tool_info already cleared by take()
        } else {
            // Process the tool info modal
            render_tool_info_modal(app, ctx, &tool_name, &tool_args);
            // Note: render_tool_info_modal may set clicked_tool_info back if needed for terminal tools
        }
    }

    // Render main UI
    render_main_ui(app, ctx);
    
    // Render hotkeys modal if needed
    render_hotkeys_modal(app, ctx);
    render_tools_modal(app, ctx);
    
    // Render CLAUDE.md modal
    render_claude_md_modal(app, ctx);
    
    // Render toast notifications
    app.state.toasts.show(ctx);
}

/// Render tools list modal
fn render_tools_modal(app: &mut SagittaCodeApp, ctx: &Context) {
    if app.state.show_tools_modal {
        let theme = app.state.current_theme;
        egui::Window::new("Available Tools")
            .collapsible(false)
            .resizable(true)
            .default_width(700.0)
            .default_height(500.0)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("These tools are available to the AI assistant via MCP:").color(theme.accent_color()).strong());
                ui.separator();
                
                // Create a scrollable area for the tools list
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Tools list with descriptions - matches actual MCP tools passed to Claude-code
                    let tools = vec![
                        ("ping", "Check if the MCP server is responsive"),
                        ("repository_add", "Clone and add a new Git repository for indexing"),
                        ("repository_list", "List currently configured repositories"),
                        ("repository_remove", "Remove a repository configuration and delete its data"),
                        ("repository_sync", "Fetch latest changes, update local copy, and re-index a repository"),
                        ("query", "Perform semantic search on indexed repositories using hybrid vector search"),
                        ("repository_search_file", "Search for files within a repository using glob patterns"),
                        ("repository_view_file", "View the content of a specific file within a repository"),
                        ("repository_switch_branch", "Switch to a different branch or Git reference with automatic resync"),
                        ("repository_list_branches", "List branches and references in a repository with optional filtering"),
                        ("todo_read", "Read the current todo list with detailed status information"),
                        ("todo_write", "Update the todo list with structured todo items"),
                        ("edit_file", "Perform exact string replacements in files with diff output"),
                        ("multi_edit_file", "Perform multiple sequential edits to a single file"),
                        ("shell_execute", "Execute shell commands with cross-platform support"),
                        ("read_file", "Read file contents with optional line range support"),
                        ("write_file", "Write content to a file with optional parent directory creation"),
                    ];
                    
                    for (tool_name, description) in tools {
                        ui.group(|ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new(tool_name).color(theme.success_color()).strong());
                                ui.label(egui::RichText::new("-").color(theme.text_color()));
                                ui.label(egui::RichText::new(description).color(theme.text_color()));
                            });
                        });
                    }
                });
                
                ui.separator();
                
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Note:").color(theme.warning_color()).strong());
                    ui.label(egui::RichText::new("These tools are provided via MCP (Model Context Protocol) and accessible to Claude-code").color(theme.text_color()));
                });
                
                ui.separator();
                
                // Close button
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Press F2 or click Close to dismiss").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Close").color(theme.button_text_color())).clicked() {
                            app.state.show_tools_modal = false;
                        }
                    });
                });
            });
    }
}

/// Handle keyboard shortcuts
fn handle_keyboard_shortcuts(app: &mut SagittaCodeApp, ctx: &Context) {
    if ctx.input(|i| i.key_pressed(Key::N) && i.modifiers.ctrl) {
        // Ctrl+N: Create new conversation
        if let Err(e) = app.app_event_sender.send(AppEvent::CreateNewConversation) {
            log::error!("Failed to send CreateNewConversation event: {e}");
        }
    }
    if ctx.input(|i| i.key_pressed(Key::R) && i.modifiers.ctrl) {
        // Ctrl+R: Toggle repository panel
        app.panels.toggle_panel(ActivePanel::Repository);
    }
    if ctx.input(|i| i.key_pressed(Key::W) && i.modifiers.ctrl) {
        // Ctrl+W: Toggle preview panel
        app.panels.toggle_panel(ActivePanel::Preview);
    }
    if ctx.input(|i| i.key_pressed(Key::S) && i.modifiers.ctrl) {
        // Ctrl+S: Toggle settings panel
        app.panels.toggle_panel(ActivePanel::Settings);
    }
    if ctx.input(|i| i.key_pressed(Key::T) && i.modifiers.ctrl) {
        // Ctrl+T: Toggle conversation panel
        app.panels.toggle_panel(ActivePanel::Conversation);
    }
    if ctx.input(|i| i.key_pressed(Key::L) && i.modifiers.ctrl) {
        app.panels.logging_panel.toggle();
    }
    if ctx.input(|i| i.key_pressed(Key::E) && i.modifiers.ctrl) {
        // Ctrl+E: Toggle events panel
        app.panels.toggle_panel(ActivePanel::Events);
    }
    if ctx.input(|i| i.key_pressed(Key::G) && i.modifiers.ctrl) {
        // Ctrl+G: Toggle git history modal
        app.panels.toggle_panel(ActivePanel::GitHistory);
    }
    if ctx.input(|i| i.key_pressed(Key::A) && i.modifiers.ctrl && i.modifiers.shift) {
        // Ctrl+Shift+A: Toggle analytics panel
        app.panels.toggle_panel(ActivePanel::Analytics);
    }
    if ctx.input(|i| i.key_pressed(Key::T) && i.modifiers.ctrl && i.modifiers.shift) {
        // Ctrl+Shift+T: Toggle theme customizer panel
        app.panels.toggle_panel(ActivePanel::ThemeCustomizer);
    }
    if ctx.input(|i| i.key_pressed(Key::M) && i.modifiers.ctrl) {
        // Ctrl+M: Toggle model selection panel
        app.panels.toggle_panel(ActivePanel::ModelSelection);
    }
    if ctx.input(|i| i.key_pressed(Key::F1)) {
        // F1: Toggle hotkeys modal
        app.state.show_hotkeys_modal = !app.state.show_hotkeys_modal;
    }
    if ctx.input(|i| i.key_pressed(Key::F2)) {
        // F2: Toggle tools list modal
        app.state.show_tools_modal = !app.state.show_tools_modal;
    }
    if ctx.input(|i| i.key_pressed(Key::F3)) {
        // F3: Open CLAUDE.md modal
        app.open_claude_md_modal();
    }
    
    // Undo/redo for chat input
    if ctx.input(|i| i.key_pressed(Key::Z) && i.modifiers.ctrl && !i.modifiers.shift) {
        // Ctrl+Z: Undo
        if !app.state.input_undo_stack.is_empty() {
            // Save current state to redo stack
            app.state.input_redo_stack.push(app.state.chat_input_buffer.clone());
            // Restore from undo stack
            if let Some(previous_state) = app.state.input_undo_stack.pop() {
                app.state.chat_input_buffer = previous_state;
            }
        }
    }
    if ctx.input(|i| (i.key_pressed(Key::Y) && i.modifiers.ctrl) || 
                      (i.key_pressed(Key::Z) && i.modifiers.ctrl && i.modifiers.shift)) {
        // Ctrl+Y or Ctrl+Shift+Z: Redo
        if !app.state.input_redo_stack.is_empty() {
            // Save current state to undo stack
            app.state.input_undo_stack.push(app.state.chat_input_buffer.clone());
            // Restore from redo stack
            if let Some(next_state) = app.state.input_redo_stack.pop() {
                app.state.chat_input_buffer = next_state;
            }
        }
    }
    
    // Phase 10: Conversation sidebar organization mode shortcuts (Ctrl+1-6)
    let enable_shortcuts = {
        match app.config.try_lock() {
            Ok(config_guard) => config_guard.conversation.sidebar.enable_keyboard_shortcuts,
            Err(_) => false, // Default to false if lock fails
        }
    };
    if enable_shortcuts {
        if ctx.input(|i| i.key_pressed(Key::Num1) && i.modifiers.ctrl) {
            // Ctrl+1: Switch to Recency mode
            app.conversation_sidebar.set_organization_mode(OrganizationMode::Recency);
        }
        if ctx.input(|i| i.key_pressed(Key::Num2) && i.modifiers.ctrl) {
            // Ctrl+2: Switch to Project mode
            app.conversation_sidebar.set_organization_mode(OrganizationMode::Project);
        }
        if ctx.input(|i| i.key_pressed(Key::Num3) && i.modifiers.ctrl) {
            // Ctrl+3: Switch to Status mode
            app.conversation_sidebar.set_organization_mode(OrganizationMode::Status);
        }
        if ctx.input(|i| i.key_pressed(Key::Num4) && i.modifiers.ctrl) {
            // Ctrl+4: Switch to Clusters mode
            app.conversation_sidebar.set_organization_mode(OrganizationMode::Clusters);
        }
        if ctx.input(|i| i.key_pressed(Key::Num5) && i.modifiers.ctrl) {
            // Ctrl+5: Switch to Tags mode
            app.conversation_sidebar.set_organization_mode(OrganizationMode::Tags);
        }
        if ctx.input(|i| i.key_pressed(Key::Num6) && i.modifiers.ctrl) {
            // Ctrl+6: Switch to Success mode
            app.conversation_sidebar.set_organization_mode(OrganizationMode::Success);
        }
    }
    
    // Loop control shortcuts
    if ctx.input(|i| i.key_pressed(Key::I) && i.modifiers.ctrl) && app.state.is_in_loop {
        // Ctrl+I: Toggle loop injection input when in loop
        app.state.show_loop_inject_input = !app.state.show_loop_inject_input;
    }
    if ctx.input(|i| i.key_pressed(Key::B) && i.modifiers.ctrl) && app.state.is_in_loop {
        // Ctrl+B: Break loop
        app.state.loop_break_requested = true;
    }
}

/// Handle loop control actions
fn handle_loop_control(app: &mut SagittaCodeApp) {
    if app.state.loop_break_requested {
        log::info!("Loop break requested by user");
        app.state.loop_break_requested = false;
        
        // If we have an agent, signal it to break the loop
        if let Some(agent) = &app.agent {
            agent.request_loop_break();
            
            app.panels.events_panel.add_event(
                super::SystemEventType::Info,
                "Loop break requested - agent will stop at next opportunity".to_string()
            );
            
            // Exit loop state immediately in UI
            app.state.is_in_loop = false;
        }
    }
    
    // Handle loop message injection
    if let Some(inject_msg) = app.state.loop_inject_message.take() {
        log::info!("Injecting message into loop: '{inject_msg}'");
        
        // Add the injected message to chat
        app.chat_manager.add_user_message(inject_msg.clone());
        
        // Add event to show the injection
        app.panels.events_panel.add_event(
            super::SystemEventType::Info,
            format!("Injected message into loop: '{inject_msg}'")
        );
    }
}

/// Handle chat input submission
fn handle_chat_input_submission(app: &mut SagittaCodeApp) {
    if app.state.chat_on_submit {
        if !app.state.chat_input_buffer.trim().is_empty() {
            let user_message = app.state.chat_input_buffer.trim().to_string();
            
            // Check if we're breaking out of a loop
            let breaking_loop = app.state.is_in_loop;
            if breaking_loop {
                log::info!("Breaking out of loop due to new user message: '{user_message}'");
                app.state.is_in_loop = false;
                app.state.loop_break_requested = false;
                
                app.panels.events_panel.add_event(
                    super::SystemEventType::Info,
                    format!("Loop broken by new message: '{user_message}'")
                );
            }
            
            // CRITICAL FIX: Prevent duplicate submissions
            if app.state.is_waiting_for_response && !breaking_loop {
                log::warn!("SagittaCodeApp: Ignoring duplicate submission while already waiting for response");
                app.state.chat_on_submit = false;
                return;
            }
            
            // Add user message to chat using the streaming manager
            app.chat_manager.add_user_message(user_message.clone());
            
            // Notify auto title updater if we have an active conversation
            if let (Some(conversation_id), Some(sender)) = (app.state.current_conversation_id, &app.auto_title_sender) {
                // Get current message count (user messages + assistant messages)
                let message_count = app.state.messages.len() + 1; // +1 for the message we just added
                
                crate::services::auto_title_updater::notify_conversation_updated(
                    sender,
                    conversation_id,
                    message_count,
                );
                
                log::debug!("Notified auto title updater for conversation {} with {} messages", conversation_id, message_count);
            }

            // CRITICAL FIX: Force clear current_response_id when user submits new message
            // This ensures Sagitta Code ALWAYS creates a new message for each response
            if let Some(old_response_id) = app.state.current_response_id.take() {
                log::warn!("SagittaCodeApp: Forcing clear of stale response_id {old_response_id} for new user message");
                // Finish any incomplete streaming response
                app.chat_manager.finish_streaming(&old_response_id);
            }
            app.state.current_response_id = None;
            log::info!("SagittaCodeApp: Cleared current_response_id for new user message: '{user_message}'");

            // Process the message with the agent using STREAMING
            if let Some(agent) = &app.agent {
                // Clone necessary values for async task
                let agent_clone = agent.clone();
                
                // Build context-aware message
                let mut context_aware_message = String::new();
                
                // Add repository context as a system message if available
                if let Some(repo_context) = &app.state.current_repository_context {
                    context_aware_message.push_str(&format!("[System: Current repository context is '{repo_context}'. When the user refers to 'this repository' or asks for operations without specifying a repository, use '{repo_context}']\n\n"));
                }
                
                
                // Append the actual user message
                context_aware_message.push_str(&user_message);
                
                let user_msg_clone = context_aware_message;
                let app_event_sender_clone = app.app_event_sender.clone();
                
                // Always using Claude Code now
                let is_claude_code = true;
                
                app.state.is_waiting_for_response = true;
                
                // Process in background task with STREAMING
                tokio::spawn(async move {
                    log::info!("Starting streaming task for user message: '{user_msg_clone}'");
                    
                    // Use the same streaming method as the CLI (without thinking config)
                    match agent_clone.process_message_stream(user_msg_clone).await {
                        Ok(mut stream) => {
                            log::info!("Successfully created stream, starting consumption");
                            let mut chunk_count = 0;
                            
                            // Consume the stream to trigger events
                            let mut last_chunk_time = std::time::Instant::now();
                            let mut consecutive_timeouts = 0;
                            
                            loop {                                
                                let timeout_duration = if is_claude_code {
                                    // Claude Code needs more time for complex requests with multiple tools
                                    if chunk_count == 0 {
                                        std::time::Duration::from_secs(120) // 2 minutes for initial response
                                    } else {
                                        std::time::Duration::from_secs(60) // 1 minute for subsequent chunks
                                    }
                                } else {
                                    // Original timeouts for other providers
                                    if chunk_count > 0 && last_chunk_time.elapsed() > std::time::Duration::from_secs(10) {
                                        std::time::Duration::from_secs(60) // Longer timeout for tool execution
                                    } else {
                                        std::time::Duration::from_secs(30) // Normal timeout
                                    }
                                };
                                
                                match tokio::time::timeout(timeout_duration, stream.next()).await {
                                    Ok(Some(chunk_result)) => {
                                        chunk_count += 1;
                                        last_chunk_time = std::time::Instant::now();
                                        consecutive_timeouts = 0;
                                        // Only log substantial chunks or final chunks
                                        if chunk_count % 10 == 0 || match &chunk_result {
                                            Ok(chunk) => chunk.is_final,
                                            Err(_) => true,
                                        } {
                                            log::debug!("Received chunk #{chunk_count}: {chunk_result:?}");
                                        }
                                        
                                        match chunk_result {
                                            Ok(chunk) => {
                                                if chunk.is_final || chunk_count % 10 == 0 {
                                                    log::trace!("Successfully processed chunk #{chunk_count}");
                                                }
                                                // The chunk processing is handled via events
                                                // so we don't need to do anything here
                                                
                                                // Check if this is the final chunk
                                                if chunk.is_final {
                                                    log::info!("Received final chunk, stream complete");
                                                    break;
                                                }
                                            },
                                            Err(e) => {
                                                log::error!("Error in streaming response chunk #{chunk_count}: {e}");
                                                
                                                // Check if this is a recoverable streaming error
                                                let error_msg = e.to_string();
                                                let is_recoverable = error_msg.contains("EOF while parsing") || 
                                                                    error_msg.contains("Failed to parse Gemini SSE") ||
                                                                    error_msg.contains("network") ||
                                                                    error_msg.contains("timeout") ||
                                                                    error_msg.contains("connection");
                                                
                                                if is_recoverable && chunk_count < 10 {
                                                    log::warn!("Recoverable streaming error detected, attempting to continue...");
                                                    // Don't break immediately for recoverable errors
                                                    // This allows the stream to potentially recover
                                                    continue;
                                                } else {
                                                    // Non-recoverable error or too many chunks processed
                                                    log::error!("Non-recoverable streaming error or too many chunks processed, stopping stream");
                                                    break;
                                                }
                                            }
                                        }
                                    },
                                    Ok(None) => {
                                        log::info!("Stream ended naturally after {chunk_count} chunks");
                                        break;
                                    },
                                    Err(_timeout) => {
                                        consecutive_timeouts += 1;
                                        log::warn!("Timeout #{} waiting for next chunk after {} chunks ({}s since last chunk)", 
                                                  consecutive_timeouts, chunk_count, last_chunk_time.elapsed().as_secs());
                                        
                                        // Allow multiple timeouts before giving up, especially during tool execution
                                        if consecutive_timeouts >= 3 {
                                            log::error!("Too many consecutive timeouts ({consecutive_timeouts}), stopping stream");
                                            break;
                                        } else {
                                            log::info!("Continuing to wait for more chunks...");
                                            continue;
                                        }
                                    }
                                }
                            }
                            
                            log::info!("Stream consumption completed with {chunk_count} total chunks");
                        },
                        Err(e) => {
                            log::error!("Failed to start streaming response: {e}");
                        }
                    }
                    // Added: Send event to signal response processing is complete
                    if let Err(e) = app_event_sender_clone.send(AppEvent::ResponseProcessingComplete) {
                        log::error!("Failed to send ResponseProcessingComplete event: {e}");
                    }
                });
            } else {
                // Add to events panel instead of chat
                app.panels.events_panel.add_event(
                    super::SystemEventType::Error,
                    "Agent not initialized. Please check settings.".to_string()
                );
            }
            
            app.state.chat_input_buffer.clear();
        }
        app.state.chat_on_submit = false;
    }
}

/// Render all panels
fn render_panels(app: &mut SagittaCodeApp, ctx: &Context) {
    // Render based on active panel
    match app.panels.active_panel {
        ActivePanel::Repository => {
            // Ensure the repository panel is open
            if !app.repo_panel.is_open() {
                app.repo_panel.toggle();
            }
            app.repo_panel.render(ctx, app.state.current_theme);
        },
        ActivePanel::Preview => app.panels.preview_panel.render(ctx, app.state.current_theme),
        ActivePanel::Events => app.panels.events_panel.render(ctx, app.state.current_theme),
        ActivePanel::Analytics => {
            if let Some(action) = app.panels.analytics_panel.render(ctx, app.state.current_theme) {
                match action {
                    super::panels::AnalyticsAction::SwitchToSuccessMode => {
                        // Switch conversation sidebar to success organization mode
                        app.conversation_sidebar.organization_mode = OrganizationMode::Success;
                        
                        // Also switch to conversation panel to show the success mode
                        app.panels.toggle_panel(ActivePanel::Conversation);
                        
                        // Add event to events panel
                        app.panels.events_panel.add_event(
                            super::panels::SystemEventType::Info,
                            "Switched to Success organization mode".to_string()
                        );
                    },
                    super::panels::AnalyticsAction::RefreshAnalytics => {
                        // Trigger analytics refresh
                        if let Some(service) = &app.conversation_service {
                            let service_clone = service.clone();
                            let event_sender = app.conversation_event_sender.clone();
                            
                            tokio::spawn(async move {
                                match service_clone.generate_analytics().await {
                                    Ok(report) => {
                                        log::info!("Generated analytics report with {} conversations", 
                                            report.overall_metrics.total_conversations);
                                        
                                        // Send the analytics report via the conversation event system
                                        if let Some(sender) = event_sender {
                                            if let Err(e) = sender.send(super::events::ConversationEvent::AnalyticsReportReady(report)) {
                                                log::error!("Failed to send analytics report event: {e}");
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        log::error!("Failed to generate analytics report: {e}");
                                    }
                                }
                            });
                            
                            // Add event to events panel
                            app.panels.events_panel.add_event(
                                super::panels::SystemEventType::Info,
                                "Refreshing analytics report...".to_string()
                            );
                        } else {
                            // Add event to events panel
                            app.panels.events_panel.add_event(
                                super::panels::SystemEventType::Error,
                                "No conversation service available for analytics".to_string()
                            );
                        }
                    },
                    super::panels::AnalyticsAction::ExportReport => {
                        // TODO: Implement report export functionality
                        app.panels.events_panel.add_event(
                            super::panels::SystemEventType::Info,
                            "Analytics report export requested (not yet implemented)".to_string()
                        );
                    },
                    super::panels::AnalyticsAction::FilterByProject(project_type) => {
                        // Update analytics panel project filter
                        app.panels.analytics_panel.project_filter = super::panels::ProjectFilter::Specific(project_type);
                        
                        // Add event to events panel
                        app.panels.events_panel.add_event(
                            super::panels::SystemEventType::Info,
                            format!("Filtered analytics by project: {project_type:?}")
                        );
                    },
                    super::panels::AnalyticsAction::FilterByDateRange(date_range) => {
                        // Update analytics panel date range filter
                        app.panels.analytics_panel.date_range_filter = date_range.clone();
                        
                        // Add event to events panel
                        app.panels.events_panel.add_event(
                            super::panels::SystemEventType::Info,
                            format!("Filtered analytics by date range: {date_range:?}")
                        );
                    },
                }
            }
        },
        ActivePanel::ThemeCustomizer => {
            // Render theme customizer panel
            if app.panels.theme_customizer.render(ctx) {
                // Theme was changed, apply it to the app
                app.state.current_theme = AppTheme::Custom;
                
                // Update the config to save the theme change
                let config = app.config.clone();
                tokio::spawn(async move {
                    let mut config_guard = config.lock().await;
                    config_guard.ui.theme = "custom".to_string();
                    
                    // Save the current custom theme colors to a file if not already set
                    if config_guard.ui.custom_theme_path.is_none() {
                        // Create a default theme file path
                                                 if let Ok(config_path) = crate::config::paths::get_sagitta_code_app_config_path() {
                             if let Some(config_dir) = config_path.parent() {
                            let theme_file_path = config_dir.join("custom_theme.sagitta-theme.json");
                            config_guard.ui.custom_theme_path = Some(theme_file_path.clone());
                            
                            // Save the current custom colors to the file
                            let custom_colors = crate::gui::theme::get_custom_theme_colors();
                            if let Ok(theme_json) = serde_json::to_string_pretty(&custom_colors) {
                                if let Err(e) = tokio::fs::write(&theme_file_path, theme_json).await {
                                    log::error!("Failed to save custom theme file to {}: {}", theme_file_path.display(), e);
                                } else {
                                    log::info!("Saved custom theme to: {}", theme_file_path.display());
                                }
                            }
                        }
                    }
                    } else if let Some(theme_path) = &config_guard.ui.custom_theme_path {
                        // Update existing theme file
                        let custom_colors = crate::gui::theme::get_custom_theme_colors();
                        if let Ok(theme_json) = serde_json::to_string_pretty(&custom_colors) {
                            if let Err(e) = tokio::fs::write(theme_path, theme_json).await {
                                log::error!("Failed to update custom theme file at {}: {}", theme_path.display(), e);
                            } else {
                                log::info!("Updated custom theme at: {}", theme_path.display());
                            }
                        }
                    }
                    
                    // Respect test isolation by using save_config which handles test paths
                    if let Err(err) = crate::config::save_config(&config_guard) {
                        log::error!("Failed to save custom theme config: {err}");
                    }
                });
            }
        },
        ActivePanel::Settings => {
            // Ensure the settings panel is open
            if !app.settings_panel.is_open() {
                app.settings_panel.toggle();
            }
            
            // Render the settings panel and check if it should be closed
            if app.settings_panel.render(ctx, app.state.current_theme) {
                // Settings panel requested to be closed
                app.panels.toggle_panel(ActivePanel::None);
            }
        },
        ActivePanel::ModelSelection => {
            // Handle model selection
            if let Some(selected_model) = app.panels.render_model_selection_panel(ctx, app.state.current_theme) {
                // Update the current model in the app configuration
                app.panels.set_current_model(selected_model.clone());
                
                // Update the Claude Code configuration
                let config = app.config.clone();
                let model_id = selected_model.clone();
                tokio::spawn(async move {
                    let mut config_guard = config.lock().await;
                    config_guard.claude_code.model = model_id.clone();
                    
                    // Respect test isolation by using save_config which handles test paths
                    if let Err(err) = crate::config::save_config(&config_guard) {
                        log::error!("Failed to save model selection: {err}");
                    } else {
                        log::info!("Model selection saved: {model_id}");
                    }
                });
                
                // Add event to events panel
                app.panels.events_panel.add_event(
                    super::panels::SystemEventType::Info,
                    format!("Selected model: {selected_model}")
                );
                
                // Close the model selection panel
                app.panels.toggle_panel(ActivePanel::None);
            }
        },
        ActivePanel::Conversation => {
            // Use the sophisticated ConversationSidebar component instead of basic UI
            let theme = app.state.current_theme;
            let conversation_service = app.get_conversation_service();
            let app_event_sender = app.app_event_sender.clone();
            let sagitta_config = app.config.clone();
            app.conversation_sidebar.show(ctx, &mut app.state, &theme, conversation_service, app_event_sender, sagitta_config);
        },
        ActivePanel::GitHistory => {
            // Render git history modal
            app.panels.git_history_modal.render(ctx, app.state.current_theme);
        },
        ActivePanel::None => {
            // When no panel is active, ensure all panels are closed
            if app.repo_panel.is_open() {
                app.repo_panel.toggle();
            }
            if app.settings_panel.is_open() {
                app.settings_panel.toggle();
            }
        }
    }
}

/// Render hotkeys modal
fn render_hotkeys_modal(app: &mut SagittaCodeApp, ctx: &Context) {
    if app.state.show_hotkeys_modal {
        let theme = app.state.current_theme;
        egui::Window::new("Keyboard Shortcuts")
            .collapsible(false)
            .resizable(true)
            .default_width(500.0)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Panel Controls:").color(theme.accent_color()).strong());
                
                // Repository Panel
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + R: Toggle Repository Panel").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.panels.toggle_panel(ActivePanel::Repository);
                        }
                    });
                });
                
                // Preview Panel
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + W: Toggle Preview Panel").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.panels.toggle_panel(ActivePanel::Preview);
                        }
                    });
                });
                
                // Settings Panel
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + S: Toggle Settings Panel").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.panels.toggle_panel(ActivePanel::Settings);
                        }
                    });
                });
                
                // Conversation Panel
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + T: Toggle Conversation Panel").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.panels.toggle_panel(ActivePanel::Conversation);
                        }
                    });
                });
                
                // Events Panel
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + E: Toggle Events Panel").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.panels.toggle_panel(ActivePanel::Events);
                        }
                    });
                });
                
                // Logging Panel
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + L: Toggle Logging Panel").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.panels.logging_panel.toggle();
                        }
                    });
                });
                
                // Git History Modal
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + G: Toggle Git History").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.panels.toggle_panel(ActivePanel::GitHistory);
                        }
                    });
                });
                
                // Model Selection Panel
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + M: Toggle Model Selection Panel").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.panels.toggle_panel(ActivePanel::ModelSelection);
                        }
                    });
                });
                
                // Analytics Panel
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + Shift + A: Toggle Analytics Panel").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.panels.toggle_panel(ActivePanel::Analytics);
                        }
                    });
                });
                
                // Theme Customizer Panel
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + Shift + T: Toggle Theme Customizer").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.panels.toggle_panel(ActivePanel::ThemeCustomizer);
                        }
                    });
                });
                
                
                ui.separator();
                
                // Phase 10: Organization mode shortcuts
                let enable_shortcuts = {
                    match app.config.try_lock() {
                        Ok(config_guard) => config_guard.conversation.sidebar.enable_keyboard_shortcuts,
                        Err(_) => false, // Default to false if lock fails
                    }
                };
                if enable_shortcuts {
                    ui.label(egui::RichText::new("Conversation Organization:").color(theme.accent_color()).strong());
                    
                    // Recency Mode
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ctrl + 1: Recency Mode").color(theme.text_color()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(egui::RichText::new("Switch").color(theme.button_text_color())).clicked() {
                                app.conversation_sidebar.set_organization_mode(OrganizationMode::Recency);
                            }
                        });
                    });
                    
                    // Project Mode
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ctrl + 2: Project Mode").color(theme.text_color()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(egui::RichText::new("Switch").color(theme.button_text_color())).clicked() {
                                app.conversation_sidebar.set_organization_mode(OrganizationMode::Project);
                            }
                        });
                    });
                    
                    // Status Mode
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ctrl + 3: Status Mode").color(theme.text_color()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(egui::RichText::new("Switch").color(theme.button_text_color())).clicked() {
                                app.conversation_sidebar.set_organization_mode(OrganizationMode::Status);
                            }
                        });
                    });
                    
                    // Clusters Mode
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ctrl + 4: Clusters Mode").color(theme.text_color()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(egui::RichText::new("Switch").color(theme.button_text_color())).clicked() {
                                app.conversation_sidebar.set_organization_mode(OrganizationMode::Clusters);
                            }
                        });
                    });
                    
                    // Tags Mode
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ctrl + 5: Tags Mode").color(theme.text_color()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(egui::RichText::new("Switch").color(theme.button_text_color())).clicked() {
                                app.conversation_sidebar.set_organization_mode(OrganizationMode::Tags);
                            }
                        });
                    });
                    
                    // Success Mode
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ctrl + 6: Success Mode").color(theme.text_color()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(egui::RichText::new("Switch").color(theme.button_text_color())).clicked() {
                                app.conversation_sidebar.set_organization_mode(OrganizationMode::Success);
                            }
                        });
                    });
                    
                    ui.separator();
                }
                
                ui.label(egui::RichText::new("General:").color(theme.accent_color()).strong());
                
                // New Conversation
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + N: New Conversation").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("New").color(theme.button_text_color())).clicked() {
                            if let Err(e) = app.app_event_sender.send(AppEvent::CreateNewConversation) {
                                log::error!("Failed to send CreateNewConversation event: {e}");
                            }
                        }
                    });
                });
                
                // F1 Help
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("F1: Show/Hide This Help").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.state.show_hotkeys_modal = false;
                        }
                    });
                });
                
                // F2 Tools List
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("F2: Show/Hide Tools List").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() {
                            app.state.show_tools_modal = !app.state.show_tools_modal;
                        }
                    });
                });
                
                // F3 CLAUDE.md Template Manager
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("F3: CLAUDE.md Template Manager").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Open").color(theme.button_text_color())).clicked() {
                            app.open_claude_md_modal();
                        }
                    });
                });
                
                ui.separator();
                ui.label(egui::RichText::new("Loop Control:").color(theme.accent_color()).strong());
                
                // Loop Injection Input
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + I: Toggle Loop Injection Input").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Toggle").color(theme.button_text_color())).clicked() 
                            && app.state.is_in_loop {
                            app.state.show_loop_inject_input = !app.state.show_loop_inject_input;
                        }
                    });
                });
                
                // Break Loop
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl + B: Break Loop").color(theme.text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Break").color(theme.button_text_color())).clicked() 
                            && app.state.is_in_loop {
                            app.state.loop_break_requested = true;
                        }
                    });
                });
                
                ui.separator();
                if ui.button(egui::RichText::new("Close").color(theme.button_text_color())).clicked() {
                    app.state.show_hotkeys_modal = false;
                }
            });
    }
}

/// Helper function to get background color from theme
fn theme_to_background_color(theme: AppTheme) -> egui::Color32 {
    theme.panel_background()
}

/// Render main UI
fn render_main_ui(app: &mut SagittaCodeApp, ctx: &Context) {
    render_panels(app, ctx);

    // Update logging panel logs from the global collector
    {
        if let Ok(logs) = LOG_COLLECTOR.lock() {
            app.panels.logging_panel.logs = logs.clone();
        }
    }
    app.panels.logging_panel.render(ctx, app.state.current_theme);

    // --- Input Panel (Bottom) ---
    TopBottomPanel::bottom("chat_input_panel")
        .resizable(false)
        .min_height(100.0) // Min height for the input area
        .max_height(200.0) // Max height to prevent it from taking too much space
        .frame(Frame::NONE.fill(theme_to_background_color(app.state.current_theme)).inner_margin(Vec2::new(16.0, 12.0)))
        .show(ctx, |ui| {
            let mut repository_refresh_requested = false;
            
            // Track text changes for undo/redo
            let text_before = app.state.chat_input_buffer.clone();
            
            let input_id = chat_input_ui(
                ui, 
                &mut app.state.chat_input_buffer, 
                &mut app.state.chat_on_submit,
                app.state.is_waiting_for_response,
                app.state.current_theme,
                &mut app.state.show_hotkeys_modal,
                // Repository context parameters
                &app.state.current_repository_context,
                &app.state.available_repositories,
                &mut app.state.pending_repository_context_change,
                &mut repository_refresh_requested,
                // Git controls
                &mut app.git_controls,
                // Loop control parameters
                app.state.is_in_loop,
                &mut app.state.loop_break_requested,
                &mut app.state.loop_inject_buffer,
                &mut app.state.show_loop_inject_input,
                &mut app.state.loop_inject_message,
                // Focus management
                &mut app.state.should_focus_input,
                // Token usage
                &app.state.current_token_usage,
            );
            
            // Handle repository refresh request
            if repository_refresh_requested {
                if let Err(e) = app.app_event_sender.send(super::events::AppEvent::RefreshRepositoryList) {
                    log::error!("Failed to send RefreshRepositoryList event: {e}");
                }
            }
            
            // Track text changes for undo/redo
            if text_before != app.state.chat_input_buffer {
                // Text changed - save snapshot for undo
                // Only save if it's significantly different from the last snapshot
                let should_save_snapshot = if let Some(last_snapshot) = app.state.input_undo_stack.last() {
                    // Save if more than 5 chars different or it's been cleared
                    (last_snapshot.len() as i32 - text_before.len() as i32).abs() > 5 || 
                    text_before.is_empty() || 
                    app.state.chat_input_buffer.is_empty()
                } else {
                    true // Always save if no history
                };
                
                if should_save_snapshot && !text_before.is_empty() {
                    app.state.input_undo_stack.push(text_before);
                    // Limit undo stack size
                    if app.state.input_undo_stack.len() > 50 {
                        app.state.input_undo_stack.remove(0);
                    }
                    // Clear redo stack when new changes are made
                    app.state.input_redo_stack.clear();
                }
            }

            // Handle repository context changes
            if let Some(new_repo) = app.state.pending_repository_context_change.take() {
                // Check for special flags to open repository panel with specific tabs
                if new_repo == "__CREATE_NEW_REPOSITORY__" {
                    // Open the repository panel
                    if !app.repo_panel.is_open() {
                        app.repo_panel.toggle();
                    }
                    // Ensure panel is set as active
                    app.panels.active_panel = ActivePanel::Repository;
                    // Set the active tab to CreateProject
                    app.repo_panel.set_active_tab(crate::gui::repository::types::RepoPanelTab::CreateProject);
                    // Don't process this as a normal repository change
                } else if new_repo == "__ADD_EXISTING_REPOSITORY__" {
                    // Open the repository panel
                    if !app.repo_panel.is_open() {
                        app.repo_panel.toggle();
                    }
                    // Ensure panel is set as active
                    app.panels.active_panel = ActivePanel::Repository;
                    // Set the active tab to Add
                    app.repo_panel.set_active_tab(crate::gui::repository::types::RepoPanelTab::Add);
                    // Don't process this as a normal repository change
                } else {
                    let repo_context = if new_repo.is_empty() { None } else { Some(new_repo.clone()) };
                    app.state.set_repository_context(repo_context.clone());
                
                // Check if auto-create CLAUDE.md is enabled and ensure it exists
                if let Some(repo_name) = &repo_context {
                    let repo_manager = app.repo_panel.get_repo_manager();
                    let repo_name_clone = repo_name.clone();
                    let config_clone = app.config.clone();
                    
                    tokio::spawn(async move {
                        // Check if auto-create is enabled
                        let auto_create_enabled = {
                            let config_guard = config_clone.lock().await;
                            config_guard.ui.auto_create_claude_md
                        };
                        
                        if auto_create_enabled {
                            // Get repository config and ensure CLAUDE.md exists
                            let repo_manager_guard = repo_manager.lock().await;
                            if let Ok(repositories) = repo_manager_guard.list_repositories().await {
                                if let Some(repo_config) = repositories.iter().find(|r| r.name == repo_name_clone) {
                                    if let Err(e) = repo_manager_guard.ensure_claude_md(repo_config).await {
                                        log::warn!("Failed to ensure CLAUDE.md for repository '{repo_name_clone}': {e}");
                                    }
                                }
                            }
                        }
                    });
                }
                
                // Update git history modal with new repository
                if let Some(repo_name) = &repo_context {
                    // Get repository manager to find the path
                    let repo_manager = app.repo_panel.get_repo_manager();
                    let repo_name_clone = repo_name.clone();
                    let app_event_sender = app.app_event_sender.clone();
                    
                    tokio::spawn(async move {
                        let repo_manager_guard = repo_manager.lock().await;
                        if let Ok(repositories) = repo_manager_guard.list_repositories().await {
                            if let Some(repo_config) = repositories.iter().find(|r| r.name == repo_name_clone) {
                                let local_path = repo_config.local_path.clone();
                                // Send event to update git history modal
                                if let Err(e) = app_event_sender.send(AppEvent::UpdateGitHistoryPath(local_path)) {
                                    log::error!("Failed to send UpdateGitHistoryPath event: {e}");
                                }
                            }
                        }
                    });
                }
                
                // Update working directory if we have a working directory manager
                if let Some(working_dir_manager) = &app.working_dir_manager {
                    if let Some(repo_name) = &repo_context {
                        // Get repository manager
                        let repo_manager = app.repo_panel.get_repo_manager();
                        let working_dir_manager_clone = working_dir_manager.clone();
                        let repo_name_clone = repo_name.clone();
                        
                        // Change working directory in background
                        tokio::spawn(async move {
                            let repo_manager_lock = repo_manager.lock().await;
                            // Get the actual repository path from the repository configuration
                            if let Ok(repositories) = repo_manager_lock.list_repositories().await {
                                if let Some(repo_config) = repositories.iter().find(|r| r.name == repo_name_clone) {
                                    let repo_path = std::path::Path::new(&repo_config.local_path);
                                    match working_dir_manager_clone.set_repository_context(Some(repo_path)) {
                                        Ok(()) => {
                                            log::info!("Changed working directory to repository '{repo_name_clone}' at path '{}'", repo_config.local_path.display());
                                            
                                            // Write the current repository path to state file for MCP server
                                            let mut state_path = dirs::config_dir().unwrap_or_default();
                                            state_path.push("sagitta-code");
                                            
                                            // Ensure directory exists
                                            if let Err(e) = tokio::fs::create_dir_all(&state_path).await {
                                                log::warn!("Failed to create state directory: {e}");
                                            } else {
                                                state_path.push("current_repository.txt");
                                                if let Err(e) = tokio::fs::write(&state_path, repo_config.local_path.to_string_lossy().as_bytes()).await {
                                                    log::warn!("Failed to write repository state file: {e}");
                                                } else {
                                                    log::debug!("Wrote current repository path to state file: {}", state_path.display());
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("Failed to change working directory to repository '{repo_name_clone}': {e}");
                                        }
                                    }
                                } else {
                                    log::error!("Repository '{repo_name_clone}' not found in repository list");
                                }
                            } else {
                                log::error!("Failed to get repository list when changing working directory");
                            }
                        });
                    } else {
                        // No repository selected, reset to base directory
                        let working_dir_manager_clone = working_dir_manager.clone();
                        let base_dir = working_dir_manager.get_base_directory().to_path_buf();
                        
                        tokio::spawn(async move {
                            match working_dir_manager_clone.change_directory(&base_dir) {
                                Ok(()) => {
                                    log::info!("Reset working directory to base");
                                    
                                    // Clear the repository state file
                                    let mut state_path = dirs::config_dir().unwrap_or_default();
                                    state_path.push("sagitta-code");
                                    state_path.push("current_repository.txt");
                                    
                                    if let Err(e) = tokio::fs::remove_file(&state_path).await {
                                        if e.kind() != std::io::ErrorKind::NotFound {
                                            log::warn!("Failed to remove repository state file: {e}");
                                        }
                                    } else {
                                        log::debug!("Cleared repository state file");
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to reset working directory to base: {e}");
                                }
                            }
                        });
                    }
                }
                
                // Save the repository context to config
                let config = app.config.clone();
                let repo_context_for_save = repo_context.clone();
                tokio::spawn(async move {
                    match config.try_lock() {
                        Ok(mut config_guard) => {
                            config_guard.ui.current_repository_context = repo_context_for_save;
                            
                            // Save the config
                            if let Err(e) = crate::config::save_config(&config_guard) {
                                log::error!("Failed to save repository context to config: {e}");
                            } else {
                                log::info!("Repository context saved to config");
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to lock config for saving repository context: {e}");
                        }
                    }
                });
                
                log::info!("Repository context changed to: {new_repo:?}");
                
                // Trigger repository switched event if a repository was selected
                if let Some(repo_name) = &repo_context {
                    if !repo_name.is_empty() {
                        if let Err(e) = app.app_event_sender.send(super::events::AppEvent::RepositorySwitched(repo_name.clone())) {
                            log::error!("Failed to send RepositorySwitched event: {e}");
                        }
                    }
                }
                }
            }
            
            // Store the input ID for potential future use
            if let Some(_id) = input_id {
                // We can use this ID later if needed
            }
        });

    // Terminal functionality removed

    // --- Chat View Panel (Central) ---
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(theme_to_background_color(app.state.current_theme)))
        .show(ctx, |ui| {
            // Force UI to use the full available width and reset text wrap settings
            ui.set_min_width(ui.available_width());
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                // Show sync status warning if repository is out of sync
                if let Some(sync_status) = app.git_controls.get_current_sync_status() {
                    if sync_status.is_out_of_sync {
                        let current_theme = app.state.current_theme;
                        let warning_frame = Frame::default()
                            .fill(current_theme.warning_color().gamma_multiply(0.1))
                            .stroke(egui::Stroke::new(1.0, current_theme.warning_color()))
                            .inner_margin(8.0)
                            .corner_radius(4.0);
                            
                        warning_frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("⚠").color(current_theme.warning_color()).size(16.0));
                                ui.label(egui::RichText::new("Repository is out of sync").color(current_theme.warning_color()));
                                ui.label(egui::RichText::new("- changes are not indexed. Run sync to update.").color(current_theme.text_color()));
                                
                                if ui.button(egui::RichText::new("Sync Now").color(current_theme.button_text_color()))
                                    .on_hover_text("Sync repository to update indexed content")
                                    .clicked() {
                                    if let Some(repo_name) = &app.state.current_repository_context {
                                        app.git_controls.send_command(crate::gui::repository::git_controls::GitCommand::ForceSync { 
                                            repo_name: repo_name.clone() 
                                        });
                                    }
                                }
                            });
                        });
                        
                        ui.add_space(8.0);
                    }
                }
                
                // Use the modern streaming chat view with all items (messages + tool cards)
                let items = app.chat_manager.get_all_items();
                
                // Check for tool clicks
                if let Some((tool_name, tool_args)) = modern_chat_view_ui(ui, &items, app.state.current_theme, &mut app.state.copy_button_state, &app.state.running_tools, &mut app.state.collapsed_thinking, &app.state.tool_results) {
                    app.state.clicked_tool_info = Some((tool_name, tool_args));
                }
            });
        });
        
    // Handle loop control actions
    handle_loop_control(app);
        
    // Process chat input submission
    handle_chat_input_submission(app);
}

/// Render tool info modal
fn render_tool_info_modal(app: &mut SagittaCodeApp, _ctx: &Context, tool_name: &str, tool_args: &str) {
    log::debug!("render_tool_info_modal called with tool_name: {tool_name}");
    
    // Check if this is a tool result (indicated by " Result" suffix or " - Terminal Output" suffix)
    if tool_name.ends_with(" Result") || tool_name.contains(" - ") {
        log::debug!("Detected tool result, determining display method");
        // This is a tool result - determine how to display it
        // Check specifically for shell/terminal commands, not code search
        // Fix: Be more specific about what constitutes a terminal output
        let is_terminal_output = tool_name.contains("Terminal Output") || 
            tool_name.contains("streaming_shell_execution") ||
            (tool_name.contains("shell") && !tool_name.contains("search")) || 
            (tool_name.contains("shell_execution") && !tool_name.contains("search")) ||
            (tool_args.contains("stdout") && tool_args.contains("stderr") && tool_args.contains("exit_code"));
            
        if is_terminal_output {
            log::debug!("Terminal output detected for: {tool_name} (terminal functionality removed)");
            // Terminal functionality removed - fall through to preview
        } else {
            // This is a non-shell tool result - show in preview
            log::debug!("Showing preview for non-shell tool result: {tool_name}");
            
            // For "Tool Result", tool_args contains the tool call ID - look up actual result
            if tool_name == "Tool Result" {
                if let Some(tool_result) = app.state.tool_results.get(tool_args).cloned() {
                    app.show_preview(&format!("Tool Result ({tool_args})"), &tool_result);
                } else {
                    app.show_preview(tool_name, &format!("Tool result not found for ID: {tool_args}"));
                }
            } else {
                app.show_preview(tool_name, tool_args);
            }
        }
    } else {
        // This is a tool call - format tool arguments nicely and show in preview
        let formatted_args = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(tool_args) {
            serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| tool_args.to_string())
        } else {
            tool_args.to_string()
        };
        
        app.show_preview(&format!("{tool_name} Tool Call"), &formatted_args);
    }
    
    // Note: clicked_tool_info is now cleared by take() in the main render function
    // This prevents the terminal output from being added repeatedly every frame
}

/// Refresh conversation clusters periodically
fn refresh_clusters_periodically(app: &mut SagittaCodeApp) {
    // Skip clustering if conversation service is not available or clustering failed to initialize
    if app.conversation_service.is_none() {
        return;
    }
    
    // Check if we should refresh clusters (every 5 minutes instead of 30 seconds)
    let should_refresh = app.state.last_conversation_refresh
        .map(|last| last.elapsed().as_secs() >= 300) // Changed from 30 to 300 seconds (5 minutes)
        .unwrap_or(true);
    
    if should_refresh {
        let service = app.conversation_service.clone();
        
        tokio::spawn(async move {
            if let Some(service) = service {
                // Only attempt clustering if we have a clustering manager
                match service.refresh_clusters().await {
                    Ok(()) => {
                        match service.get_clusters().await {
                            Ok(clusters) => {
                                log::debug!("Refreshed {} conversation clusters", clusters.len());
                                // Note: We can't directly update the sidebar here due to borrowing rules
                                // The actual update will happen in the next render cycle
                            },
                            Err(e) => {
                                log::debug!("No clusters available (clustering disabled): {e}");
                            }
                        }
                    },
                    Err(e) => {
                        log::debug!("Clustering not available: {e}");
                    }
                }
            }
        });
        
        // Update the last refresh time
        app.state.last_conversation_refresh = Some(std::time::Instant::now());
    }
}

/// Refresh repository list periodically
fn refresh_repository_list_periodically(app: &mut SagittaCodeApp) {
    // Check if we should refresh repository list (every 30 seconds)
    static mut LAST_REPO_REFRESH: Option<std::time::Instant> = None;
    
    let should_refresh = unsafe {
        LAST_REPO_REFRESH
            .map(|last| last.elapsed().as_secs() >= 30)
            .unwrap_or(true)
    };
    
    if should_refresh {
        log::debug!("Refreshing repository list...");
        let repo_manager = app.repo_panel.get_repo_manager();
        let app_event_sender = app.app_event_sender.clone();
        
        tokio::spawn(async move {
            log::debug!("Starting repository list refresh task");
            match repo_manager.lock().await.list_repositories().await {
                Ok(repositories) => {
                    let repo_names: Vec<String> = repositories
                        .iter()
                        .map(|repo| repo.name.clone())
                        .collect();
                    
                    log::info!("Refreshed repository list: {repo_names:?}");
                    
                    // Send the repository list update event
                    if let Err(e) = app_event_sender.send(super::events::AppEvent::RepositoryListUpdated(repo_names)) {
                        log::error!("Failed to send repository list update event: {e}");
                    } else {
                        log::debug!("Successfully sent repository list update event");
                    }
                },
                Err(e) => {
                    log::error!("Failed to refresh repository list: {e}");
                }
            }
        });
        
        unsafe {
            LAST_REPO_REFRESH = Some(std::time::Instant::now());
        }
    }
}

// Helper function to build dependency context message
fn build_dependency_context(enabled_dependencies: &[String]) -> String {
    let mut context_message = String::new();
    
    if !enabled_dependencies.is_empty() {
        if enabled_dependencies.len() == 1 {
            context_message.push_str(&format!(
                "[System: The repository '{}' is enabled as a dependency. You can use the repository tools to understand this dependency's requirements]\n\n", 
                enabled_dependencies[0]
            ));
        } else {
            let deps_list = enabled_dependencies.join("', '");
            context_message.push_str(&format!(
                "[System: The following repositories are enabled as dependencies: '{deps_list}'. You can use the repository tools to understand these dependencies' requirements]\n\n"
            ));
        }
    }
    
    context_message
}

/// Render CLAUDE.md modal
fn render_claude_md_modal(app: &mut SagittaCodeApp, ctx: &Context) {
    // Render the modal and handle any actions
    if let Some(action) = app.claude_md_modal.render(ctx, &app.state.current_theme) {
        // Handle the action
        match action {
            crate::gui::claude_md_modal::ClaudeMdModalAction::Save => {
                // Schedule save action to be handled asynchronously
                if let Err(e) = app.app_event_sender.send(crate::gui::app::AppEvent::SaveClaudeMdTemplate) {
                    log::error!("Failed to send SaveClaudeMdTemplate event: {e}");
                }
            },
            crate::gui::claude_md_modal::ClaudeMdModalAction::LoadFromFile => {
                // Trigger file dialog (would be handled by main app)
                log::info!("CLAUDE.md modal: Load from file requested");
            },
            crate::gui::claude_md_modal::ClaudeMdModalAction::ShowHelp => {
                // Show help information
                log::info!("CLAUDE.md modal: Help requested");
            },
            crate::gui::claude_md_modal::ClaudeMdModalAction::ApplyToAllRepos => {
                // Apply template to all repositories
                if let Err(e) = app.app_event_sender.send(crate::gui::app::AppEvent::ApplyClaudeMdToAllRepos) {
                    log::error!("Failed to send ApplyClaudeMdToAllRepos event: {e}");
                }
            },
        }
    }
}

/// Update git controls repository context if it has changed
fn update_git_controls_repository_context(app: &mut SagittaCodeApp) {
    use crate::gui::repository::git_controls::GitCommand;
    
    let current_git_repo = app.git_controls.state().current_repository.clone();
    let current_app_repo = app.state.current_repository_context.clone();
    
    if current_git_repo != current_app_repo {
        // Send a command to update the repository context
        if let Some(repo_name) = current_app_repo.clone() {
            if !repo_name.is_empty() && !repo_name.starts_with("__") {
                app.git_controls.send_command(GitCommand::UpdateRepository { 
                    repo_name: Some(repo_name) 
                });
            }
        } else {
            app.git_controls.send_command(GitCommand::UpdateRepository { 
                repo_name: None 
            });
        }
    }
    
    // Periodically update sync statuses
    static LAST_SYNC_UPDATE: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
    let should_update = {
        let mut last_update = LAST_SYNC_UPDATE.lock().unwrap();
        match *last_update {
            None => {
                *last_update = Some(std::time::Instant::now());
                true
            }
            Some(time) if time.elapsed() > std::time::Duration::from_secs(5) => {
                *last_update = Some(std::time::Instant::now());
                true
            }
            _ => false
        }
    };
    
    if should_update {
        app.git_controls.send_command(GitCommand::UpdateSyncStatuses);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Key;
    use crate::SagittaCodeConfig;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use crate::gui::repository::manager::RepositoryManager;

    /// Create a test app instance for testing
    fn create_test_app() -> SagittaCodeApp {
        let sagitta_code_config = SagittaCodeConfig::default();
        let app_core_config = sagitta_search::config::AppConfig::default(); // Create AppConfig
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(
            Arc::new(Mutex::new(app_core_config.clone())) // Use cloned AppConfig for repo_manager
        )));
        SagittaCodeApp::new(repo_manager, sagitta_code_config, app_core_config) // Pass both configs
    }

    /// Mock egui context for testing keyboard input
    struct MockEguiInput {
        pub key_pressed: Option<Key>,
        pub ctrl_pressed: bool,
    }

    impl MockEguiInput {
        fn new() -> Self {
            Self {
                key_pressed: None,
                ctrl_pressed: false,
            }
        }

        fn with_key_and_ctrl(key: Key) -> Self {
            Self {
                key_pressed: Some(key),
                ctrl_pressed: true,
            }
        }
    }

    /// Test that keyboard shortcuts properly toggle panels
    #[test]
    fn test_keyboard_shortcuts_toggle_panels() {
        let mut app = create_test_app();
        
        // Initially no panel should be active
        assert_eq!(app.panels.active_panel, ActivePanel::None);
        
        // Test Ctrl+R for Repository panel
        app.panels.toggle_panel(ActivePanel::Repository);
        assert_eq!(app.panels.active_panel, ActivePanel::Repository);
        
        // Toggle again should close it
        app.panels.toggle_panel(ActivePanel::Repository);
        assert_eq!(app.panels.active_panel, ActivePanel::None);
        
        // Test Ctrl+W for Preview panel
        app.panels.toggle_panel(ActivePanel::Preview);
        assert_eq!(app.panels.active_panel, ActivePanel::Preview);
        
        // Test Ctrl+S for Settings panel
        app.panels.toggle_panel(ActivePanel::Settings);
        assert_eq!(app.panels.active_panel, ActivePanel::Settings);
        
        // Test Ctrl+T for Conversation panel
        app.panels.toggle_panel(ActivePanel::Conversation);
        assert_eq!(app.panels.active_panel, ActivePanel::Conversation);
        
        // Test Ctrl+E for Events panel
        app.panels.toggle_panel(ActivePanel::Events);
        assert_eq!(app.panels.active_panel, ActivePanel::Events);
    }

    #[test]
    fn test_panel_manager_close_all_panels() {
        let mut app = create_test_app();
        
        // Open preview panel
        app.panels.toggle_panel(ActivePanel::Preview);
        assert_eq!(app.panels.active_panel, ActivePanel::Preview);
        assert!(app.panels.preview_panel.visible);
        
        // Open events panel
        app.panels.toggle_panel(ActivePanel::Events);
        assert_eq!(app.panels.active_panel, ActivePanel::Events);
        assert!(app.panels.events_panel.visible);
        
        // Close all panels
        app.panels.close_all_panels();
        assert_eq!(app.panels.active_panel, ActivePanel::None);
        assert!(!app.panels.preview_panel.visible);
        assert!(!app.panels.events_panel.visible);
    }

    #[test]
    fn test_panel_manager_exclusive_panels() {
        let mut app = create_test_app();
        
        // Open repository panel
        app.panels.toggle_panel(ActivePanel::Repository);
        assert_eq!(app.panels.active_panel, ActivePanel::Repository);
        
        // Open settings panel - should close repository panel
        app.panels.toggle_panel(ActivePanel::Settings);
        assert_eq!(app.panels.active_panel, ActivePanel::Settings);
        
        // Open conversation panel - should close settings panel
        app.panels.toggle_panel(ActivePanel::Conversation);
        assert_eq!(app.panels.active_panel, ActivePanel::Conversation);
    }

    #[test]
    fn test_repository_and_settings_panel_visibility() {
        let mut app = create_test_app();
        
        // Initially both panels should be closed
        assert!(!app.repo_panel.is_open());
        assert!(!app.settings_panel.is_open());
        
        // Set active panel to Repository - this simulates keyboard shortcut
        app.panels.active_panel = ActivePanel::Repository;
        
        // The render_panels function should open the repository panel
        // We can't call render_panels directly in tests, but we can simulate the logic
        if !app.repo_panel.is_open() {
            app.repo_panel.toggle();
        }
        assert!(app.repo_panel.is_open());
        
        // Set active panel to Settings - this simulates keyboard shortcut
        app.panels.active_panel = ActivePanel::Settings;
        
        // The render_panels function should open the settings panel
        if !app.settings_panel.is_open() {
            app.settings_panel.toggle();
        }
        assert!(app.settings_panel.is_open());
        
        // Set active panel to None - this should close both panels
        app.panels.active_panel = ActivePanel::None;
        
        // Simulate the ActivePanel::None logic
        if app.repo_panel.is_open() {
            app.repo_panel.toggle();
        }
        if app.settings_panel.is_open() {
            app.settings_panel.toggle();
        }
        
        assert!(!app.repo_panel.is_open());
        assert!(!app.settings_panel.is_open());
    }

    #[test]
    fn test_preview_panel_functionality() {
        let mut app = create_test_app();
        
        // Initially preview panel should not be visible
        assert!(!app.panels.preview_panel.visible);
        assert!(app.panels.preview_panel.content.is_empty());
        assert!(app.panels.preview_panel.title.is_empty());
        
        // Show preview content
        app.show_preview("Test Title", "Test Content");
        assert!(app.panels.preview_panel.visible);
        assert_eq!(app.panels.preview_panel.title, "Test Title");
        assert_eq!(app.panels.preview_panel.content, "Test Content");
        assert_eq!(app.panels.active_panel, ActivePanel::Preview);
        
        // Toggle preview panel off
        app.panels.preview_panel.toggle();
        assert!(!app.panels.preview_panel.visible);
    }

    #[test]
    fn test_events_panel_functionality() {
        let mut app = create_test_app();
        
        // Initially events panel should not be visible and have no events
        assert!(!app.panels.events_panel.visible);
        assert!(app.panels.events_panel.events.is_empty());
        
        // Add an event
        app.panels.events_panel.add_event(
            super::super::panels::SystemEventType::Info,
            "Test event message".to_string()
        );
        
        assert_eq!(app.panels.events_panel.events.len(), 1);
        assert_eq!(app.panels.events_panel.events[0].message, "Test event message");
        
        // Add multiple events to test max_events limit
        for i in 0..150 {
            app.panels.events_panel.add_event(
                super::super::panels::SystemEventType::Info,
                format!("Event {i}")
            );
        }
        
        // Should not exceed max_events (100)
        assert_eq!(app.panels.events_panel.events.len(), app.panels.events_panel.max_events);
    }

    #[test]
    fn test_loop_control_state() {
        let mut app = create_test_app();
        
        // Initially not in loop
        assert!(!app.state.is_in_loop);
        assert!(!app.state.loop_break_requested);
        assert!(app.state.loop_inject_message.is_none());
        assert!(!app.state.show_loop_inject_input);
        
        // Simulate entering loop state
        app.state.is_in_loop = true;
        assert!(app.state.is_in_loop);
        
        // Test loop break request
        app.state.loop_break_requested = true;
        assert!(app.state.loop_break_requested);
        
        // Test loop injection
        app.state.loop_inject_message = Some("Test injection".to_string());
        assert_eq!(app.state.loop_inject_message.as_ref().unwrap(), "Test injection");
    }

    #[test]
    fn test_theme_background_color() {
        // Test light theme
        let light_color = theme_to_background_color(AppTheme::Light);
        assert_eq!(light_color, AppTheme::Light.panel_background());
        
        // Test dark theme
        let dark_color = theme_to_background_color(AppTheme::Dark);
        assert_eq!(dark_color, AppTheme::Dark.panel_background());
    }

    #[test]
    fn test_active_panel_enum_variants() {
        // Test that all ActivePanel variants are properly defined and accessible
        let panels = vec![
            ActivePanel::None,
            ActivePanel::Repository,
            ActivePanel::Preview,
            ActivePanel::Settings,
            ActivePanel::Conversation,
            ActivePanel::Events,
            ActivePanel::Analytics,
            ActivePanel::ThemeCustomizer,
            ActivePanel::ModelSelection,
        ];
        
        // Test that we can match on all variants
        for panel in panels {
            match panel {
                ActivePanel::None => {},
                ActivePanel::Repository => {},
                ActivePanel::Preview => {},
                ActivePanel::Settings => {},
                ActivePanel::Conversation => {},
                ActivePanel::Events => {},
                ActivePanel::Analytics => {},
                ActivePanel::ThemeCustomizer => {},
                ActivePanel::ModelSelection => {},
                ActivePanel::GitHistory => {},
            }
        }
    }

    #[test]
    fn test_keyboard_shortcut_imports_are_valid() {
        // This test ensures that the ActivePanel import is working correctly
        // and that we can reference all the panel types used in keyboard shortcuts
        
        let _repository = ActivePanel::Repository;
        let _preview = ActivePanel::Preview;
        let _settings = ActivePanel::Settings;
        let _conversation = ActivePanel::Conversation;
        let _events = ActivePanel::Events;
        let _analytics = ActivePanel::Analytics;
        let _none = ActivePanel::None;
        
        // If this compiles, the imports are working correctly
        assert!(true);
    }

    #[test]
    fn test_panel_state_consistency() {
        let mut app = create_test_app();
        
        // Test that panel state remains consistent across operations
        app.panels.toggle_panel(ActivePanel::Preview);
        assert_eq!(app.panels.active_panel, ActivePanel::Preview);
        assert!(app.panels.preview_panel.visible);
        
        // Switch to another panel
        app.panels.toggle_panel(ActivePanel::Events);
        assert_eq!(app.panels.active_panel, ActivePanel::Events);
        assert!(app.panels.events_panel.visible);
        assert!(!app.panels.preview_panel.visible); // Previous panel should be closed
        
        // Close all panels
        app.panels.close_all_panels();
        assert_eq!(app.panels.active_panel, ActivePanel::None);
        assert!(!app.panels.preview_panel.visible);
        assert!(!app.panels.events_panel.visible);
    }

    /// Integration test to ensure keyboard shortcuts work end-to-end
    #[test]
    fn test_keyboard_shortcuts_integration() {
        let mut app = create_test_app();
        
        // This test simulates the keyboard shortcut handling logic
        // to ensure it works correctly with the panel system
        
        // Simulate Ctrl+R (Repository)
        app.panels.toggle_panel(ActivePanel::Repository);
        assert_eq!(app.panels.active_panel, ActivePanel::Repository);
        
        // Simulate Ctrl+W (Preview)
        app.panels.toggle_panel(ActivePanel::Preview);
        assert_eq!(app.panels.active_panel, ActivePanel::Preview);
        assert!(app.panels.preview_panel.visible);
        
        // Simulate Ctrl+S (Settings)
        app.panels.toggle_panel(ActivePanel::Settings);
        assert_eq!(app.panels.active_panel, ActivePanel::Settings);
        assert!(!app.panels.preview_panel.visible); // Should close previous panel
        
        // Simulate Ctrl+T (Conversation)
        app.panels.toggle_panel(ActivePanel::Conversation);
        assert_eq!(app.panels.active_panel, ActivePanel::Conversation);
        
        // Simulate Ctrl+E (Events)
        app.panels.toggle_panel(ActivePanel::Events);
        assert_eq!(app.panels.active_panel, ActivePanel::Events);
        assert!(app.panels.events_panel.visible);
        
        // Simulate Ctrl+Shift+A (Analytics)
        app.panels.toggle_panel(ActivePanel::Analytics);
        assert_eq!(app.panels.active_panel, ActivePanel::Analytics);
        assert!(app.panels.analytics_panel.visible);
        assert!(!app.panels.events_panel.visible); // Should close previous panel
        
        // Simulate pressing the same key again (should close)
        app.panels.toggle_panel(ActivePanel::Analytics);
        assert_eq!(app.panels.active_panel, ActivePanel::None);
        assert!(!app.panels.analytics_panel.visible);
    }

    #[test]
    fn test_new_keyboard_shortcuts() {
        let mut app = create_test_app();
        
        // Test Ctrl+M for ModelSelection panel
        app.panels.toggle_panel(ActivePanel::ModelSelection);
        assert_eq!(app.panels.active_panel, ActivePanel::ModelSelection);
        assert!(app.panels.model_selection_panel.visible);
        
        // Test F1 for hotkeys modal (simulated)
        app.state.show_hotkeys_modal = false;
        // Simulate F1 press
        app.state.show_hotkeys_modal = !app.state.show_hotkeys_modal;
        assert!(app.state.show_hotkeys_modal);
        
        // Press F1 again to close
        app.state.show_hotkeys_modal = !app.state.show_hotkeys_modal;
        assert!(!app.state.show_hotkeys_modal);
    }


    /// Phase 5: Test sequential panel open/close events to ensure no races
    #[test]
    fn test_phase5_sequential_panel_operations() {
        let mut app = create_test_app();
        
        // Test rapid sequential panel switches
        let panels_to_test = vec![
            ActivePanel::Repository,
            ActivePanel::Preview,
            ActivePanel::Settings,
            ActivePanel::Conversation,
            ActivePanel::Events,
            ActivePanel::Analytics,
            ActivePanel::ThemeCustomizer,
            ActivePanel::ModelSelection,
        ];
        
        // Open each panel in sequence
        for panel in &panels_to_test {
            app.panels.toggle_panel(panel.clone());
            assert_eq!(app.panels.active_panel, *panel);
            
            // Verify only the current panel is active
            match panel {
                ActivePanel::Preview => assert!(app.panels.preview_panel.visible),
                ActivePanel::Events => assert!(app.panels.events_panel.visible),
                ActivePanel::Analytics => assert!(app.panels.analytics_panel.visible),
                ActivePanel::ThemeCustomizer => assert!(app.panels.theme_customizer.is_open()),
                ActivePanel::ModelSelection => assert!(app.panels.model_selection_panel.visible),
                _ => {} // Repository, Settings, Conversation handled by main app
            }
        }
        
        // Close all panels
        app.panels.close_all_panels();
        assert_eq!(app.panels.active_panel, ActivePanel::None);
        
        // Verify all panels are closed
        assert!(!app.panels.preview_panel.visible);
        assert!(!app.panels.events_panel.visible);
        assert!(!app.panels.analytics_panel.visible);
        assert!(!app.panels.theme_customizer.is_open());
        assert!(!app.panels.model_selection_panel.visible);
    }

    /// Phase 5: Test that hotkeys modal buttons provide same functionality as keyboard shortcuts
    #[test]
    fn test_phase5_hotkeys_modal_button_equivalence() {
        let mut app = create_test_app();
        
        // Test that modal button actions match keyboard shortcut actions
        
        // Repository Panel - keyboard vs button should have same effect
        app.panels.toggle_panel(ActivePanel::Repository);
        let keyboard_state = app.panels.active_panel.clone();
        
        app.panels.toggle_panel(ActivePanel::None); // Reset
        
        // Simulate button click (same as keyboard shortcut)
        app.panels.toggle_panel(ActivePanel::Repository);
        let button_state = app.panels.active_panel.clone();
        
        assert_eq!(keyboard_state, button_state);
        
        // Test with Preview Panel
        app.panels.toggle_panel(ActivePanel::Preview);
        assert_eq!(app.panels.active_panel, ActivePanel::Preview);
        assert!(app.panels.preview_panel.visible);
        
        // Toggle again (same as button would do)
        app.panels.toggle_panel(ActivePanel::Preview);
        assert_eq!(app.panels.active_panel, ActivePanel::None);
        assert!(!app.panels.preview_panel.visible);
    }

    /// Phase 5: Test idempotency of panel toggle operations
    #[test]
    fn test_phase5_panel_toggle_idempotency() {
        let mut app = create_test_app();
        
        // Test that multiple toggles of the same panel work correctly
        
        // Open Preview panel
        app.panels.toggle_panel(ActivePanel::Preview);
        assert_eq!(app.panels.active_panel, ActivePanel::Preview);
        assert!(app.panels.preview_panel.visible);
        
        // Toggle same panel again (should close)
        app.panels.toggle_panel(ActivePanel::Preview);
        assert_eq!(app.panels.active_panel, ActivePanel::None);
        assert!(!app.panels.preview_panel.visible);
        
        // Toggle again (should open)
        app.panels.toggle_panel(ActivePanel::Preview);
        assert_eq!(app.panels.active_panel, ActivePanel::Preview);
        assert!(app.panels.preview_panel.visible);
        
        // Test with Events panel
        app.panels.toggle_panel(ActivePanel::Events);
        assert_eq!(app.panels.active_panel, ActivePanel::Events);
        assert!(app.panels.events_panel.visible);
        assert!(!app.panels.preview_panel.visible); // Previous panel should be closed
        
        // Toggle Events again
        app.panels.toggle_panel(ActivePanel::Events);
        assert_eq!(app.panels.active_panel, ActivePanel::None);
        assert!(!app.panels.events_panel.visible);
    }

    /// Phase 5: Test that all keyboard shortcuts are properly centralized
    #[test]
    fn test_phase5_centralized_keyboard_handling() {
        let mut app = create_test_app();
        
        // Verify that all panel shortcuts go through PanelManager::toggle_panel
        // This ensures consistency and idempotency
        
        let all_panels = vec![
            ActivePanel::Repository,
            ActivePanel::Preview,
            ActivePanel::Settings,
            ActivePanel::Conversation,
            ActivePanel::Events,
            ActivePanel::Analytics,
            ActivePanel::ThemeCustomizer,
            ActivePanel::ModelSelection,
        ];
        
        for panel in all_panels {
            // Each panel should be toggleable
            app.panels.toggle_panel(panel.clone());
            
            // Should not crash or cause inconsistent state
            match panel {
                ActivePanel::Repository | ActivePanel::Settings | ActivePanel::Conversation => {
                    // These are handled by main app, just check active_panel is set
                    assert_eq!(app.panels.active_panel, panel);
                },
                ActivePanel::Preview => {
                    assert_eq!(app.panels.active_panel, panel);
                    assert!(app.panels.preview_panel.visible);
                },
                ActivePanel::Events => {
                    assert_eq!(app.panels.active_panel, panel);
                    assert!(app.panels.events_panel.visible);
                },
                ActivePanel::Analytics => {
                    assert_eq!(app.panels.active_panel, panel);
                    assert!(app.panels.analytics_panel.visible);
                },
                ActivePanel::ThemeCustomizer => {
                    assert_eq!(app.panels.active_panel, panel);
                    assert!(app.panels.theme_customizer.is_open());
                },
                ActivePanel::ModelSelection => {
                    assert_eq!(app.panels.active_panel, panel);
                    assert!(app.panels.model_selection_panel.visible);
                },
                ActivePanel::GitHistory => {
                    assert_eq!(app.panels.active_panel, panel);
                    // Can't test git history panel visibility since it might not be exposed
                },
                ActivePanel::None => {
                    // Should not happen in this test
                    panic!("Unexpected None panel");
                }
            }
            
            // Close the panel
            app.panels.toggle_panel(panel);
            assert_eq!(app.panels.active_panel, ActivePanel::None);
        }
    }

    /// Phase 5: Test logging panel special handling (Ctrl+L)
    #[test]
    fn test_phase5_logging_panel_special_handling() {
        let mut app = create_test_app();
        
        // Logging panel has special handling - it doesn't go through PanelManager::toggle_panel
        // but directly calls logging_panel.toggle()
        
        // Initially closed
        assert!(!app.panels.logging_panel.visible);
        
        // Simulate Ctrl+L
        app.panels.logging_panel.toggle();
        assert!(app.panels.logging_panel.visible);
        
        // Toggle again
        app.panels.logging_panel.toggle();
        assert!(!app.panels.logging_panel.visible);
        
        // Logging panel should not affect active_panel state
        assert_eq!(app.panels.active_panel, ActivePanel::None);
        
        // Open another panel, then logging panel
        app.panels.toggle_panel(ActivePanel::Preview);
        assert_eq!(app.panels.active_panel, ActivePanel::Preview);
        
        app.panels.logging_panel.toggle();
        assert!(app.panels.logging_panel.visible);
        assert_eq!(app.panels.active_panel, ActivePanel::Preview); // Should not change
        assert!(app.panels.preview_panel.visible); // Should still be open
    }

    /// Test Create project and Add project button behavior
    #[test]
    fn test_repository_context_button_behavior() {
        let mut app = create_test_app();
        
        // Test initial state
        assert!(app.state.pending_repository_context_change.is_none());
        
        // Simulate Create project button click
        app.state.pending_repository_context_change = Some("__CREATE_NEW_REPOSITORY__".to_string());
        
        // The handling logic would:
        // 1. Open repository panel if closed
        // 2. Set active panel to Repository
        // 3. Set active tab to CreateProject
        
        // Simulate Add project button click
        app.state.pending_repository_context_change = Some("__ADD_EXISTING_REPOSITORY__".to_string());
        
        // The handling logic would:
        // 1. Open repository panel if closed
        // 2. Set active panel to Repository
        // 3. Set active tab to Add
    }

    /// Test that repository panel opens with correct tab
    #[tokio::test]
    async fn test_repository_panel_tab_switching() {
        let mut app = create_test_app();
        
        // Test that repository panel can be toggled
        assert!(!app.repo_panel.is_open());
        app.repo_panel.toggle();
        assert!(app.repo_panel.is_open());
        
        // Test setting active tab to CreateProject
        app.repo_panel.set_active_tab(crate::gui::repository::types::RepoPanelTab::CreateProject);
        
        // Test setting active tab to Add
        app.repo_panel.set_active_tab(crate::gui::repository::types::RepoPanelTab::Add);
        
        // Both operations should work without panicking
    }

    /// Test that repository context changes are handled correctly
    #[test]
    fn test_repository_context_special_flags() {
        let mut app = create_test_app();
        
        // Test normal repository context change
        app.state.pending_repository_context_change = Some("my-repo".to_string());
        // This should update current_repository_context to Some("my-repo")
        
        // Test empty repository (no repository selected)
        app.state.pending_repository_context_change = Some("".to_string());
        // This should update current_repository_context to None
        
        // Test special flags don't update repository context
        app.state.pending_repository_context_change = Some("__CREATE_NEW_REPOSITORY__".to_string());
        // This should NOT update current_repository_context
        
        app.state.pending_repository_context_change = Some("__ADD_EXISTING_REPOSITORY__".to_string());
        // This should NOT update current_repository_context
    }

    /// Test dependency context message building
    #[test]
    fn test_dependency_context_message_building() {
        // Test building context message with no dependencies
        let context_message = build_dependency_context(&[]);
        assert!(context_message.is_empty());
        
        // Test with single dependency
        let single_dep = vec!["my-lib".to_string()];
        let context_message = build_dependency_context(&single_dep);
        assert!(context_message.contains("repository 'my-lib' is enabled as a dependency"));
        assert!(context_message.contains("use the repository tools"));
        
        // Test with multiple dependencies
        let multi_deps = vec!["lib1".to_string(), "lib2".to_string(), "lib3".to_string()];
        let context_message = build_dependency_context(&multi_deps);
        assert!(context_message.contains("following repositories are enabled as dependencies"));
        assert!(context_message.contains("lib1"));
        assert!(context_message.contains("lib2"));
        assert!(context_message.contains("lib3"));
        assert!(context_message.contains("use the repository tools"));
    }

    /// Test that clicked tool info is not cleared for terminal tools
    /// This ensures the "view details" button keeps working for terminal output
    #[test]
    fn test_clicked_tool_info_persistence_for_terminal_tools() {
        // Terminal tool names that should NOT clear clicked_tool_info
        let terminal_tools = vec![
            ("shell_execution - Terminal Output", r#"{"stdout": "test", "exit_code": 0}"#),
            ("bash Result", r#"{"stdout": "output", "stderr": "", "exit_code": 0}"#),
            ("streaming_shell_execution", r#"{"command": "ls", "exit_code": 0}"#),
        ];
        
        for (tool_name, tool_args) in terminal_tools {
            // Check if this would be treated as a shell command
            let is_shell = tool_name.contains("Terminal Output") || 
                          tool_name.contains("shell") || 
                          (tool_name.contains("execution") && !tool_name.contains("search")) ||
                          tool_args.contains("stdout") || 
                          tool_args.contains("stderr") || 
                          tool_args.contains("exit_code");
            
            assert!(is_shell, "{tool_name} should be treated as shell command");
        }
    }

    /// Test that search_code is not treated as a shell command
    #[test]
    fn test_search_code_opens_preview_not_terminal() {
        let non_shell_tools = vec![
            ("search_code Result", r#"{"results": []}"#),
            ("code_search_execution Result", r#"{"results": []}"#),
            ("repository_map Result", r#"{"map": "content"}"#),
        ];
        
        for (tool_name, tool_args) in non_shell_tools {
            // Check if this would be treated as a shell command
            let is_terminal_output = tool_name.contains("Terminal Output") || 
                tool_name.contains("streaming_shell_execution") ||
                (tool_name.contains("shell") && !tool_name.contains("search")) || 
                (tool_name.contains("shell_execution") && !tool_name.contains("search")) ||
                (tool_args.contains("stdout") && tool_args.contains("stderr") && tool_args.contains("exit_code"));
            
            assert!(!is_terminal_output, "{tool_name} should NOT be treated as terminal output");
        }
    }
    
    /// Test that clicked_tool_info is processed only once to prevent repeated terminal writes
    #[test]
    fn test_clicked_tool_info_processed_once() {
        // This test verifies the fix for the flickering issue where terminal output
        // was being added every frame when clicked_tool_info remained Some
        
        // The fix uses take() to ensure clicked_tool_info is processed only once
        let mut clicked_tool_info: Option<(String, String)> = Some(("test_tool".to_string(), "args".to_string()));
        
        // First frame - should process and clear
        if let Some((tool_name, tool_args)) = clicked_tool_info.take() {
            assert_eq!(tool_name, "test_tool");
            assert_eq!(tool_args, "args");
        }
        
        // clicked_tool_info should now be None
        assert!(clicked_tool_info.is_none());
        
        // Second frame - should not process anything
        if clicked_tool_info.take().is_some() {
            panic!("clicked_tool_info should have been cleared by first take()");
        }
    }
    
    /// Test the updated terminal output detection logic
    #[test]
    fn test_terminal_output_detection() {
        struct TestCase {
            tool_name: &'static str,
            tool_args: &'static str,
            expected: bool,
            description: &'static str,
        }
        
        let test_cases = vec![
            TestCase {
                tool_name: "streaming_shell_execution - Terminal Output",
                tool_args: r#"{"stdout": "output", "stderr": "", "exit_code": 0}"#,
                expected: true,
                description: "streaming_shell_execution with Terminal Output suffix",
            },
            TestCase {
                tool_name: "shell - Result",
                tool_args: r#"{"stdout": "test", "stderr": "", "exit_code": 0}"#,
                expected: true,
                description: "shell tool with all terminal fields",
            },
            TestCase {
                tool_name: "search_code - Result",
                tool_args: r#"{"results": [], "query": "test"}"#,
                expected: false,
                description: "search_code should open preview, not terminal",
            },
            TestCase {
                tool_name: "code_search_execution - Result",
                tool_args: r#"{"results": []}"#,
                expected: false,
                description: "code search execution should open preview",
            },
            TestCase {
                tool_name: "execution - Result",
                tool_args: r#"{"result": "data"}"#,
                expected: false,
                description: "execution without terminal fields should open preview",
            },
        ];
        
        for test in test_cases {
            let is_terminal_output = test.tool_name.contains("Terminal Output") || 
                test.tool_name.contains("streaming_shell_execution") ||
                (test.tool_name.contains("shell") && !test.tool_name.contains("search")) || 
                (test.tool_name.contains("shell_execution") && !test.tool_name.contains("search")) ||
                (test.tool_args.contains("stdout") && test.tool_args.contains("stderr") && test.tool_args.contains("exit_code"));
                          
            assert_eq!(is_terminal_output, test.expected, 
                      "Failed for {}: {}", test.tool_name, test.description);
        }
    }
} 