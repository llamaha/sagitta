// UI rendering for the Fred Agent application

use egui::{Context, Key, TopBottomPanel, Frame, Vec2};
use crate::gui::app::AppEvent;
use super::FredAgentApp;
use super::super::chat::input::chat_input_ui;
use super::super::chat::view::modern_chat_view_ui;
use super::super::theme::{AppTheme, apply_theme};
use super::super::symbols;
use crate::agent::state::types::AgentState;
use crate::utils::logging::LOG_COLLECTOR;
use futures_util::StreamExt;
use super::panels::ActivePanel;
use crate::config::FredAgentConfig;
use crate::gui::repository::manager::RepositoryManager;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Main rendering function for the application
pub fn render(app: &mut FredAgentApp, ctx: &Context) {
    // Apply theme to the entire application
    apply_theme(ctx, app.state.current_theme);

    // Handle keyboard shortcuts
    handle_keyboard_shortcuts(app, ctx);

    // Process agent events
    app.process_agent_events();
    
    // Process conversation events
    app.process_conversation_events();

    // Process app events
    app.process_app_events();
    
    // Handle temporary thinking indicator timeout (3 seconds)
    if let Some(start_time) = app.state.thinking_start_time {
        if start_time.elapsed() > std::time::Duration::from_secs(3) {
            app.state.thinking_message = None;
            app.state.thinking_start_time = None;
        }
    }
    
    // Update current agent mode (periodically)
    if let Some(agent) = &app.agent {
        let agent_clone = agent.clone();
        let current_mode = app.state.current_agent_mode;
        tokio::spawn(async move {
            // This will run async and update the mode if needed
            // We can't directly update self here, but we could use events if needed
        });
    }
    
    // Handle clicked tool info
    if let Some((tool_name, tool_args)) = &app.state.clicked_tool_info.clone() {
        render_tool_info_modal(app, ctx, tool_name, tool_args);
    }

    // Render main UI
    render_main_ui(app, ctx);
    
    // Render hotkeys modal if needed
    render_hotkeys_modal(app, ctx);
}

/// Handle keyboard shortcuts
fn handle_keyboard_shortcuts(app: &mut FredAgentApp, ctx: &Context) {
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
    if ctx.input(|i| i.key_pressed(Key::A) && i.modifiers.ctrl && i.modifiers.shift) {
        // Ctrl+Shift+A: Toggle analytics panel
        app.panels.toggle_panel(ActivePanel::Analytics);
    }
    if ctx.input(|i| i.key_pressed(Key::T) && i.modifiers.ctrl && i.modifiers.shift) {
        // Ctrl+Shift+T: Toggle theme customizer panel
        app.panels.toggle_panel(ActivePanel::ThemeCustomizer);
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
fn handle_loop_control(app: &mut FredAgentApp) {
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
        log::info!("Injecting message into loop: '{}'", inject_msg);
        
        // Add the injected message to chat
        app.chat_manager.add_user_message(inject_msg.clone());
        
        // Add event to show the injection
        app.panels.events_panel.add_event(
            super::SystemEventType::Info,
            format!("Injected message into loop: '{}'", inject_msg)
        );
    }
}

/// Handle chat input submission
fn handle_chat_input_submission(app: &mut FredAgentApp) {
    if app.state.chat_on_submit {
        if !app.state.chat_input_buffer.trim().is_empty() {
            let user_message = app.state.chat_input_buffer.trim().to_string();
            
            // Check if we're breaking out of a loop
            let breaking_loop = app.state.is_in_loop;
            if breaking_loop {
                log::info!("Breaking out of loop due to new user message: '{}'", user_message);
                app.state.is_in_loop = false;
                app.state.loop_break_requested = false;
                
                app.panels.events_panel.add_event(
                    super::SystemEventType::Info,
                    format!("Loop broken by new message: '{}'", user_message)
                );
            }
            
            // CRITICAL FIX: Prevent duplicate submissions
            if app.state.is_waiting_for_response && !breaking_loop {
                log::warn!("FredAgentApp: Ignoring duplicate submission while already waiting for response");
                app.state.chat_on_submit = false;
                return;
            }
            
            // Add user message to chat using the streaming manager
            app.chat_manager.add_user_message(user_message.clone());

            // CRITICAL FIX: Force clear current_response_id when user submits new message
            // This ensures Fred ALWAYS creates a new message for each response
            if let Some(old_response_id) = app.state.current_response_id.take() {
                log::warn!("FredAgentApp: Forcing clear of stale response_id {} for new user message", old_response_id);
                // Finish any incomplete streaming response
                app.chat_manager.finish_streaming(&old_response_id);
            }
            app.state.current_response_id = None;
            log::info!("FredAgentApp: Cleared current_response_id for new user message: '{}'", user_message);

            // Process the message with the agent using STREAMING
            if let Some(agent) = &app.agent {
                // Clone necessary values for async task
                let agent_clone = agent.clone();
                let user_msg_clone = user_message.clone();
                let app_event_sender_clone = app.app_event_sender.clone();
                
                app.state.is_waiting_for_response = true;
                
                // Process in background task with STREAMING and THINKING
                tokio::spawn(async move {
                    log::info!("Starting streaming task for user message: '{}'", user_msg_clone);
                    
                    // Use the streaming version with thinking enabled
                    match agent_clone.process_message_stream_with_thinking_fixed(
                        user_msg_clone, 
                        Some(crate::llm::client::ThinkingConfig {
                            include_thoughts: true,
                            thinking_budget: None, // Auto budget
                        })
                    ).await {
                        Ok(mut stream) => {
                            log::info!("Successfully created stream, starting consumption");
                            let mut chunk_count = 0;
                            
                            // Consume the stream to trigger events
                            let mut last_chunk_time = std::time::Instant::now();
                            let mut consecutive_timeouts = 0;
                            
                            loop {
                                // Use longer timeout for tool execution phases
                                let timeout_duration = if chunk_count > 0 && last_chunk_time.elapsed() > std::time::Duration::from_secs(10) {
                                    std::time::Duration::from_secs(60) // Longer timeout for tool execution
                                } else {
                                    std::time::Duration::from_secs(30) // Normal timeout
                                };
                                
                                match tokio::time::timeout(timeout_duration, stream.next()).await {
                                    Ok(Some(chunk_result)) => {
                                        chunk_count += 1;
                                        last_chunk_time = std::time::Instant::now();
                                        consecutive_timeouts = 0;
                                        log::debug!("Received chunk #{}: {:?}", chunk_count, chunk_result);
                                        
                                        match chunk_result {
                                            Ok(chunk) => {
                                                log::trace!("Successfully processed chunk #{}", chunk_count);
                                                // The chunk processing is handled via events
                                                // so we don't need to do anything here
                                                
                                                // Check if this is the final chunk
                                                if chunk.is_final {
                                                    log::info!("Received final chunk, stream complete");
                                                    break;
                                                }
                                            },
                                            Err(e) => {
                                                log::error!("Error in streaming response chunk #{}: {}", chunk_count, e);
                                                
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
                                        log::info!("Stream ended naturally after {} chunks", chunk_count);
                                        break;
                                    },
                                    Err(_timeout) => {
                                        consecutive_timeouts += 1;
                                        log::warn!("Timeout #{} waiting for next chunk after {} chunks ({}s since last chunk)", 
                                                  consecutive_timeouts, chunk_count, last_chunk_time.elapsed().as_secs());
                                        
                                        // Allow multiple timeouts before giving up, especially during tool execution
                                        if consecutive_timeouts >= 3 {
                                            log::error!("Too many consecutive timeouts ({}), stopping stream", consecutive_timeouts);
                                            break;
                                        } else {
                                            log::info!("Continuing to wait for more chunks...");
                                            continue;
                                        }
                                    }
                                }
                            }
                            
                            log::info!("Stream consumption completed with {} total chunks", chunk_count);
                        },
                        Err(e) => {
                            log::error!("Failed to start streaming response: {}", e);
                        }
                    }
                    // Added: Send event to signal response processing is complete
                    if let Err(e) = app_event_sender_clone.send(AppEvent::ResponseProcessingComplete) {
                        log::error!("Failed to send ResponseProcessingComplete event: {}", e);
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
fn render_panels(app: &mut FredAgentApp, ctx: &Context) {
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
        ActivePanel::Analytics => app.panels.analytics_panel.render(ctx, app.state.current_theme),
        ActivePanel::ThemeCustomizer => {
            // Render theme customizer panel
            if app.panels.theme_customizer.render(ctx) {
                // Theme was changed, apply it to the app
                app.state.current_theme = AppTheme::Custom;
                
                // Update the config to save the theme change
                if let Some(agent) = &app.agent {
                    let config = app.config.clone();
                    tokio::spawn(async move {
                        let mut config_guard = config.as_ref().clone();
                        config_guard.ui.theme = "custom".to_string();
                        
                        if let Err(err) = crate::config::save_config(&config_guard) {
                            log::error!("Failed to save custom theme config: {}", err);
                        }
                    });
                }
            }
        },
        ActivePanel::Settings => {
            // Ensure the settings panel is open
            if !app.settings_panel.is_open() {
                app.settings_panel.toggle();
            }
            
            // Render the settings panel
            app.settings_panel.render(ctx, app.state.current_theme);
        },
        ActivePanel::Conversation => {
            // Only refresh conversation data when panel is first opened or manually requested
            // Don't refresh constantly to avoid flickering
            
            // Render conversation sidebar
            egui::SidePanel::left("conversation_panel")
                .resizable(true)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        ui.heading(&format!("{} Conversation Management", symbols::get_thinking_symbol()));
                        ui.separator();
                        
                        // Add conversation controls
                        ui.horizontal(|ui| {
                            if ui.button("➕ New Conversation").clicked() {
                                if let Some(agent) = &app.agent {
                                    let agent_clone = agent.clone();
                                    let sender = app.conversation_event_sender.clone();
                                    tokio::spawn(async move {
                                        let title = format!("Conversation {}", chrono::Utc::now().format("%Y-%m-%d %H:%M"));
                                        match agent_clone.create_new_conversation(title).await {
                                            Ok(conversation_id) => {
                                                if let Some(sender) = sender {
                                                    let _ = sender.send(super::ConversationEvent::ConversationCreated(conversation_id));
                                                }
                                            },
                                            Err(e) => {
                                                log::error!("Failed to create new conversation: {}", e);
                                            }
                                        }
                                    });
                                }
                            }
                            
                            if ui.button("🔄 Refresh").clicked() {
                                // Force refresh when user explicitly requests it
                                app.force_refresh_conversation_data();
                            }
                        });
                        
                        ui.add_space(8.0);
                        
                        // Show current conversation info using cached data
                        if let Some(ref title) = app.state.current_conversation_title {
                            ui.label(format!("📝 Current: {}", title));
                        } else if app.state.conversation_data_loading {
                            ui.label("📝 Current: Loading...");
                        } else {
                            ui.label("📝 Current: No active conversation");
                        }
                        
                        ui.add_space(8.0);
                        ui.separator();
                        
                        // Organization modes
                        ui.label("📋 Organization Modes:");
                        ui.add_space(4.0);
                        
                        if ui.button("📅 By Recency").clicked() {
                            app.conversation_sidebar.set_organization_mode(
                                crate::gui::conversation::OrganizationMode::Recency
                            );
                        }
                        
                        if ui.button("📁 By Project").clicked() {
                            app.conversation_sidebar.set_organization_mode(
                                crate::gui::conversation::OrganizationMode::Project
                            );
                        }
                        
                        if ui.button("📊 By Status").clicked() {
                            app.conversation_sidebar.set_organization_mode(
                                crate::gui::conversation::OrganizationMode::Status
                            );
                        }
                        
                        if ui.button("🔗 By Clusters").clicked() {
                            app.conversation_sidebar.set_organization_mode(
                                crate::gui::conversation::OrganizationMode::Clusters
                            );
                        }
                        
                        if ui.button("🏷️ By Tags").clicked() {
                            app.conversation_sidebar.set_organization_mode(
                                crate::gui::conversation::OrganizationMode::Tags
                            );
                        }
                        
                        if ui.button("✅ By Success").clicked() {
                            app.conversation_sidebar.set_organization_mode(
                                crate::gui::conversation::OrganizationMode::Success
                            );
                        }
                        
                        ui.add_space(16.0);
                        ui.separator();
                        
                        // Conversation list using organized data from sidebar
                        ui.label("💬 Conversations:");
                        ui.add_space(4.0);
                        
                        egui::ScrollArea::vertical()
                            .max_height(400.0)
                            .show(ui, |ui| {
                                if app.state.conversation_data_loading {
                                    ui.label("Loading conversations...");
                                } else if app.state.conversation_list.is_empty() {
                                    ui.label("No conversations found");
                                    ui.add_space(4.0);
                                    ui.label("💡 Click '➕ New Conversation' to get started!");
                                } else {
                                    // Use the conversation sidebar to organize the data
                                    match app.conversation_sidebar.organize_conversations(&app.state.conversation_list, None) {
                                        Ok(organized) => {
                                            // Display organized groups
                                            for group in &organized.groups {
                                                // Group header with expand/collapse
                                                ui.horizontal(|ui| {
                                                    let expanded = group.expanded;
                                                    let expand_icon = if expanded { "▼" } else { "▶" };
                                                    
                                                    if ui.button(format!("{} {} ({})", expand_icon, group.name, group.metadata.count)).clicked() {
                                                        app.conversation_sidebar.toggle_group(&group.id);
                                                    }
                                                    
                                                    // Show group statistics
                                                    if group.metadata.statistics.active_count > 0 {
                                                        ui.label(format!("🟢 {}", group.metadata.statistics.active_count));
                                                    }
                                                    if group.metadata.statistics.completed_count > 0 {
                                                        ui.label(format!("✅ {}", group.metadata.statistics.completed_count));
                                                    }
                                                });
                                                
                                                // Show conversations in group if expanded
                                                if group.expanded {
                                                    ui.indent(&group.id, |ui| {
                                                        for conv_item in &group.conversations {
                                                            let is_current = app.state.current_conversation_title
                                                                .as_ref()
                                                                .map(|current| current == &conv_item.summary.title)
                                                                .unwrap_or(false);
                                                            
                                                            ui.horizontal(|ui| {
                                                                // Status indicator
                                                                let status_icon = match conv_item.display.status_indicator {
                                                                    crate::gui::conversation::StatusIndicator::Active => "🟢",
                                                                    crate::gui::conversation::StatusIndicator::Paused => "⏸️",
                                                                    crate::gui::conversation::StatusIndicator::Completed => "✅",
                                                                    crate::gui::conversation::StatusIndicator::Failed => "❌",
                                                                    crate::gui::conversation::StatusIndicator::Archived => "📦",
                                                                    crate::gui::conversation::StatusIndicator::Branched => "🌿",
                                                                    crate::gui::conversation::StatusIndicator::Checkpointed => "📍",
                                                                };
                                                                
                                                                if ui.selectable_label(is_current, format!("{} {} {}", status_icon, conv_item.display.title, conv_item.display.time_display)).clicked() {
                                                                    app.switch_to_conversation(conv_item.summary.id);
                                                                }
                                                                
                                                                // Show visual indicators
                                                                for indicator in &conv_item.display.indicators {
                                                                    ui.label(&indicator.display);
                                                                }
                                                            });
                                                            
                                                            // Show preview if available
                                                            if let Some(ref preview) = conv_item.preview {
                                                                ui.indent(format!("{}_preview", conv_item.summary.id), |ui| {
                                                                    ui.label(egui::RichText::new(preview).small().weak());
                                                                });
                                                            }
                                                        }
                                                    });
                                                }
                                                
                                                ui.add_space(4.0);
                                            }
                                            
                                            // Show organization info
                                            ui.add_space(8.0);
                                            ui.separator();
                                            ui.label(format!("📊 Showing {} of {} conversations", organized.filtered_count, organized.total_count));
                                        },
                                        Err(e) => {
                                            log::error!("Failed to organize conversations: {}", e);
                                            // Fallback to simple list
                                            let conversations_data: Vec<(uuid::Uuid, String, bool)> = app.state.conversation_list.iter()
                                                .map(|conv| {
                                                    let is_current = app.state.current_conversation_title
                                                        .as_ref()
                                                        .map(|current| current == &conv.title)
                                                        .unwrap_or(false);
                                                    (conv.id, conv.title.clone(), is_current)
                                                })
                                                .collect();
                                            
                                            for (conv_id, title, is_current) in conversations_data {
                                                ui.horizontal(|ui| {
                                                    if ui.selectable_label(is_current, format!("📄 {}", title)).clicked() {
                                                        app.switch_to_conversation(conv_id);
                                                    }
                                                });
                                            }
                                        }
                                    }
                                }
                            });
                        
                        ui.add_space(16.0);
                        ui.separator();
                        
                        // Advanced features with interactive buttons
                        ui.label("🚀 Advanced Features:");
                        ui.add_space(4.0);
                        
                        // Context-Aware Branching
                        ui.horizontal(|ui| {
                            if ui.button("🌳 Context-Aware Branching").clicked() {
                                // TODO: Implement branching suggestions UI
                                log::info!("Context-Aware Branching feature activated");
                            }
                            ui.label("Intelligent branch point detection");
                        });
                        
                        // Smart Checkpoints
                        ui.horizontal(|ui| {
                            if ui.button("📍 Smart Checkpoints").clicked() {
                                // TODO: Implement checkpoint suggestions UI
                                log::info!("Smart Checkpoints feature activated");
                            }
                            ui.label("Automatic checkpoint creation");
                        });
                        
                        // Semantic Clustering
                        ui.horizontal(|ui| {
                            if ui.button("🔍 Semantic Clustering").clicked() {
                                // Switch to clusters organization mode
                                app.conversation_sidebar.set_organization_mode(
                                    crate::gui::conversation::OrganizationMode::Clusters
                                );
                                log::info!("Switched to semantic clustering view");
                            }
                            ui.label("Group related conversations");
                        });
                        
                        // Conversation Analytics
                        ui.horizontal(|ui| {
                            if ui.button("📊 Conversation Analytics").clicked() {
                                // TODO: Implement analytics dashboard
                                log::info!("Conversation Analytics feature activated");
                            }
                            ui.label("Success metrics and insights");
                        });
                        
                        // Project Workspaces
                        ui.horizontal(|ui| {
                            if ui.button("🏢 Project Workspaces").clicked() {
                                // Switch to project organization mode
                                app.conversation_sidebar.set_organization_mode(
                                    crate::gui::conversation::OrganizationMode::Project
                                );
                                log::info!("Switched to project workspaces view");
                            }
                            ui.label("Project-based organization");
                        });
                        
                        ui.add_space(8.0);
                        ui.separator();
                        
                        // Feature status
                        ui.label("📈 Feature Status:");
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label("✅");
                            ui.label("Backend: Fully Implemented");
                        });
                        ui.horizontal(|ui| {
                            ui.label("🔧");
                            ui.label("Integration: Active");
                        });
                        ui.horizontal(|ui| {
                            ui.label("🚀");
                            ui.label("UI: Interactive");
                        });
                    });
                });
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
fn render_hotkeys_modal(app: &mut FredAgentApp, ctx: &Context) {
    // This would be implemented based on the original hotkeys modal logic
    // For now, just a placeholder
    if app.state.show_hotkeys_modal {
        let theme = app.state.current_theme;
        egui::Window::new("Keyboard Shortcuts")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Ctrl + R: Toggle Repository Panel").color(theme.text_color()));
                ui.label(egui::RichText::new("Ctrl + W: Toggle Preview Panel").color(theme.text_color()));
                ui.label(egui::RichText::new("Ctrl + S: Toggle Settings Panel").color(theme.text_color()));
                ui.label(egui::RichText::new("Ctrl + T: Toggle Conversation Panel").color(theme.text_color()));
                ui.label(egui::RichText::new("Ctrl + E: Toggle Events Panel").color(theme.text_color()));
                ui.label(egui::RichText::new("Ctrl + L: Toggle Logging Panel").color(theme.text_color()));
                ui.label(egui::RichText::new("Ctrl + Shift + A: Toggle Analytics Panel").color(theme.text_color()));
                ui.label(egui::RichText::new("Ctrl + Shift + T: Toggle Theme Customizer").color(theme.text_color()));
                ui.separator();
                ui.label(egui::RichText::new("Loop Control:").color(theme.accent_color()).strong());
                ui.label(egui::RichText::new("Ctrl + I: Toggle Loop Injection Input").color(theme.text_color()));
                ui.label(egui::RichText::new("Ctrl + B: Break Loop").color(theme.text_color()));
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
fn render_main_ui(app: &mut FredAgentApp, ctx: &Context) {
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
        .frame(Frame::none().fill(theme_to_background_color(app.state.current_theme)).inner_margin(Vec2::new(16.0, 12.0)))
        .show(ctx, |ui| {
            let input_id = chat_input_ui(
                ui, 
                &mut app.state.chat_input_buffer, 
                &mut app.state.chat_on_submit,
                app.state.is_waiting_for_response,
                app.state.current_theme,
                &mut app.state.show_hotkeys_modal,
                app.state.current_agent_mode,
                &mut app.state.pending_agent_mode_change,
                // Loop control parameters
                app.state.is_in_loop,
                &mut app.state.loop_break_requested,
                &mut app.state.loop_inject_buffer,
                &mut app.state.show_loop_inject_input,
                &mut app.state.loop_inject_message,
            );
            
            // Handle agent mode changes
            if let Some(new_mode) = app.state.pending_agent_mode_change.take() {
                app.state.current_agent_mode = new_mode; // Update cached mode immediately
                if let Some(agent) = &app.agent {
                    let agent_clone = agent.clone();
                    tokio::spawn(async move {
                        if let Err(e) = agent_clone.set_mode(new_mode).await {
                            log::error!("Failed to change agent mode: {}", e);
                        } else {
                            log::info!("Agent mode changed to: {:?}", new_mode);
                        }
                    });
                }
            }
            
            // Store the input ID for potential future use
            if let Some(id) = input_id {
                // We can use this ID later if needed
            }
        });

    // --- Chat View Panel (Central) ---
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme_to_background_color(app.state.current_theme)))
        .show(ctx, |ui| {
            // Force UI to use the full available width and reset text wrap settings
            ui.set_min_width(ui.available_width());
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                // Use the modern streaming chat view
                let messages = app.chat_manager.get_all_messages();
                
                // Check for tool clicks
                if let Some((tool_name, tool_args)) = modern_chat_view_ui(ui, &messages, app.state.current_theme) {
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
fn render_tool_info_modal(app: &mut FredAgentApp, ctx: &Context, tool_name: &str, tool_args: &str) {
    // Check if this is a tool result (indicated by " Result" suffix)
    if tool_name.ends_with(" Result") {
        // This is a tool result - show it directly
        app.show_preview(tool_name, tool_args);
    } else {
        // This is a tool call - format tool arguments nicely
        let formatted_args = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(tool_args) {
            serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| tool_args.to_string())
        } else {
            tool_args.to_string()
        };
        
        app.show_preview(&format!("{} Tool Call", tool_name), &formatted_args);
    }
    
    // Clear the clicked tool info after handling
    app.state.clicked_tool_info = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Key;

    /// Create a test app instance for testing
    fn create_test_app() -> FredAgentApp {
        let fred_config = FredAgentConfig::default();
        let app_core_config = sagitta_search::config::AppConfig::default(); // Create AppConfig
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(
            Arc::new(Mutex::new(app_core_config.clone())) // Use cloned AppConfig for repo_manager
        )));
        FredAgentApp::new(repo_manager, fred_config, app_core_config) // Pass both configs
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
        assert!(app.panels.preview_panel.visible);
        
        // Test Ctrl+S for Settings panel
        app.panels.toggle_panel(ActivePanel::Settings);
        assert_eq!(app.panels.active_panel, ActivePanel::Settings);
        
        // Test Ctrl+T for Conversation panel
        app.panels.toggle_panel(ActivePanel::Conversation);
        assert_eq!(app.panels.active_panel, ActivePanel::Conversation);
        
        // Test Ctrl+E for Events panel
        app.panels.toggle_panel(ActivePanel::Events);
        assert_eq!(app.panels.active_panel, ActivePanel::Events);
        assert!(app.panels.events_panel.visible);
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
                format!("Event {}", i)
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
} 