use egui::{Context, Ui, Frame, Margin, ScrollArea, Layout, Align, RichText, Color32, TextEdit, ComboBox};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use std::time::{Duration, Instant};

use crate::gui::theme::AppTheme;
use crate::gui::app::{AppState, events::AppEvent};
use crate::agent::conversation::service::ConversationService;
use crate::agent::state::types::ConversationStatus;
use crate::config::SagittaCodeConfig;
use super::types::{ConversationSidebar, ConversationGroup, ConversationItem, SidebarAction, OrganizationMode};

impl ConversationSidebar {
    /// Main rendering method for the sidebar
    pub fn show(
        &mut self, 
        ctx: &Context, 
        app_state: &mut AppState, 
        theme: &AppTheme, 
        conversation_service: Option<Arc<ConversationService>>, 
        app_event_sender: UnboundedSender<AppEvent>, 
        sagitta_config: Arc<tokio::sync::Mutex<SagittaCodeConfig>>
    ) {
        // Auto-save state periodically
        self.auto_save_state(sagitta_config.clone());
        
        let panel_frame = Frame {
            inner_margin: Margin::same(8),
            outer_margin: Margin::same(0),
            corner_radius: egui::Rounding::same(4),
            fill: theme.panel_background(),
            stroke: egui::Stroke::NONE,
            ..Default::default()
        };

        // Get screen size for responsive constraints
        let screen_size = ctx.screen_rect().size();
        let is_small_screen = self.config.responsive.enabled && 
            screen_size.x <= self.config.responsive.small_screen_breakpoint;
        
        // Responsive width constraints - made wider to show more information
        let (default_width, min_width, max_width) = if is_small_screen {
            (280.0, 220.0, 380.0)
        } else {
            (360.0, 280.0, 500.0)
        };

        egui::SidePanel::left("conversation_sidebar")
            .frame(panel_frame)
            .default_width(default_width)
            .min_width(min_width)
            .max_width(max_width)
            .resizable(true)
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .auto_shrink(false)
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        
                        self.render_header(ui, app_state, theme);
                        
                        let spacing = if is_small_screen && self.config.responsive.compact_mode.reduced_spacing {
                            2.0
                        } else {
                            4.0
                        };
                        ui.add_space(spacing);
                        
                        self.render_search_bar(ui, app_state);
                        ui.add_space(spacing);

                        if app_state.conversation_data_loading {
                            ui.centered_and_justified(|ui| {
                                ui.spinner();
                                ui.label("Loading conversations...");
                            });
                            return;
                        }

                        log::trace!("Sidebar: Organizing {} conversations", app_state.conversation_list.len());
                        match self.organize_conversations(
                            &app_state.conversation_list,
                            Some(&self.clusters),
                        ) {
                            Ok(organized_data) => {
                                if self.show_branch_suggestions {
                                    if let Some(conversation_id) = app_state.current_conversation_id {
                                        if let Ok(Some(action)) = self.branch_suggestions_ui.render(ui, conversation_id, theme) {
                                            if let Some(sidebar_action) = self.handle_branch_suggestion_action(action) {
                                                self.pending_action = Some(sidebar_action);
                                            }
                                        }
                                    }
                                }
                                
                                log::trace!("Sidebar: Rendering {} conversation groups", organized_data.groups.len());
                                for (index, group) in organized_data.groups.iter().enumerate() {
                                    log::trace!("Sidebar: Rendering group {} with {} conversations", group.name, group.conversations.len());
                                    self.render_conversation_group(ui, group, app_state, theme);
                                    
                                    if index < organized_data.groups.len() - 1 {
                                        ui.add_space(if is_small_screen { 1.0 } else { 2.0 });
                                    }
                                }
                                
                                ui.add_space(if is_small_screen { 4.0 } else { 6.0 });
                                ui.separator();
                                ui.add_space(if is_small_screen { 1.0 } else { 2.0 });
                                ui.label(format!("📊 Showing {} of {} conversations", 
                                    organized_data.filtered_count, organized_data.total_count));
                            },
                            Err(e) => {
                                log::error!("Failed to organize conversations: {}", e);
                                self.render_simple_conversation_list(ui, app_state, theme);
                            }
                        }
                        
                        if self.show_checkpoint_suggestions {
                            if let Some(conversation_id) = app_state.current_conversation_id {
                                ui.add_space(if is_small_screen { 4.0 } else { 6.0 });
                                ui.separator();
                                ui.add_space(if is_small_screen { 1.0 } else { 2.0 });
                                
                                match self.checkpoint_suggestions_ui.render(ui, conversation_id, theme) {
                                    Ok(Some(action)) => {
                                        if let Some(sidebar_action) = self.handle_checkpoint_suggestion_action(action) {
                                            self.pending_action = Some(sidebar_action);
                                        }
                                    },
                                    Ok(None) => {},
                                    Err(e) => {
                                        log::error!("Failed to render checkpoint suggestions: {}", e);
                                    }
                                }
                            }
                        }
                        
                        ui.add_space(8.0);
                    });
            });
        
        log::trace!("Sidebar: About to handle sidebar actions, pending_action: {:?}", self.pending_action);
        self.handle_sidebar_actions(app_state, ctx, conversation_service, app_event_sender);
    }

    /// Render the sidebar header
    fn render_header(&mut self, ui: &mut Ui, app_state: &mut AppState, theme: &AppTheme) {
        let screen_size = ui.ctx().screen_rect().size();
        let is_small_screen = self.config.responsive.enabled && 
            screen_size.x <= self.config.responsive.small_screen_breakpoint;
        
        ui.horizontal(|ui| {
            if is_small_screen && self.config.responsive.compact_mode.abbreviated_labels {
                ui.label("💬");
            } else {
                ui.heading("💬 Conversations");
            }
            
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let button_fn = if is_small_screen && self.config.responsive.compact_mode.small_buttons {
                    |ui: &mut Ui, text: &str| ui.small_button(text)
                } else {
                    |ui: &mut Ui, text: &str| ui.button(text)
                };
                
                if button_fn(ui, "🔄").on_hover_text("Refresh conversations").clicked() {
                    self.pending_action = Some(SidebarAction::RefreshConversations);
                }
                if button_fn(ui, "➕").on_hover_text("New conversation").clicked() {
                    self.pending_action = Some(SidebarAction::CreateNewConversation);
                }
                
                let branch_icon = if self.show_branch_suggestions { "🌳" } else { "🌿" };
                if button_fn(ui, branch_icon).on_hover_text("Toggle branch suggestions").clicked() {
                    self.toggle_branch_suggestions();
                }
                
                let checkpoint_icon = if self.show_checkpoint_suggestions { "📍" } else { "📌" };
                if button_fn(ui, checkpoint_icon).on_hover_text("Toggle checkpoint suggestions").clicked() {
                    self.toggle_checkpoint_suggestions();
                }
            });
        });
        
        let spacing = if is_small_screen && self.config.responsive.compact_mode.reduced_spacing {
            1.0
        } else {
            2.0
        };
        ui.add_space(spacing);
        
        ui.horizontal(|ui| {
            ui.label("Organize by:");
            ComboBox::from_label("")
                .selected_text(self.get_organization_mode_label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Recency, "⏰ Recency");
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Project, "📁 Project");
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Status, "🎯 Status");
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Clusters, "🧩 Clusters");
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Tags, "🏷️ Tags");
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Success, "📈 Success");
                });
        });
    }

    /// Render the search bar
    fn render_search_bar(&mut self, ui: &mut Ui, app_state: &mut AppState) {
        ui.horizontal(|ui| {
            ui.label("🔍");
            
            let response = ui.add(
                TextEdit::singleline(&mut self.edit_buffer)
                    .hint_text("Search by title, tags, or project...")
                    .desired_width(ui.available_width() - 50.0)
            );
            
            if response.changed() {
                self.last_search_query = Some(self.edit_buffer.clone());
                self.search_debounce_timer = Some(Instant::now());
            }
            
            if let Some(timer) = self.search_debounce_timer {
                if timer.elapsed() > Duration::from_millis(300) {
                    self.search_query = if self.edit_buffer.is_empty() {
                        None
                    } else {
                        Some(self.edit_buffer.clone())
                    };
                    self.search_debounce_timer = None;
                }
            }
            
            if ui.button("🚫").on_hover_text("Clear search").clicked() {
                self.edit_buffer.clear();
                self.search_query = None;
                self.search_debounce_timer = None;
            }
        });
        
        ui.horizontal(|ui| {
            ui.label("Filters:");
            ui.toggle_value(&mut self.filter_active, "Active");
            ui.toggle_value(&mut self.filter_completed, "Completed");
            ui.toggle_value(&mut self.filter_archived, "Archived");
            
            if ui.button(if self.show_filters { "Hide Filters" } else { "Show Filters" }).clicked() {
                self.show_filters = !self.show_filters;
            }
        });
        
        self.filters.statuses.clear();
        if self.filter_active {
            self.filters.statuses.push(ConversationStatus::Active);
        }
        if self.filter_completed {
            self.filters.statuses.push(ConversationStatus::Completed);
        }
        if self.filter_archived {
            self.filters.statuses.push(ConversationStatus::Archived);
        }
    }

    /// Render the filters panel
    fn render_filters(&mut self, ui: &mut Ui) {
        if self.show_filters {
            ui.group(|ui| {
                ui.label(RichText::new("Advanced Filters").strong());
                ui.separator();
                
                ui.checkbox(&mut self.filters.favorites_only, "Favorites only");
                ui.checkbox(&mut self.filters.branches_only, "Has branches");
                ui.checkbox(&mut self.filters.checkpoints_only, "Has checkpoints");
                
                if let Some(min_messages) = &mut self.filters.min_messages {
                    ui.horizontal(|ui| {
                        ui.label("Min messages:");
                        ui.add(egui::DragValue::new(min_messages).speed(1));
                    });
                }
            });
        }
    }

    /// Render a conversation group
    fn render_conversation_group(
        &mut self, 
        ui: &mut Ui, 
        group: &ConversationGroup, 
        app_state: &mut AppState, 
        theme: &AppTheme
    ) {
        // Default to expanded only for "today" group
        let collapsed_key = format!("collapsed_{}", group.id);
        let is_expanded = if self.expanded_groups.contains(&collapsed_key) {
            false // Explicitly collapsed
        } else if self.expanded_groups.contains(&format!("expanded_{}", group.id)) {
            true  // Explicitly expanded
        } else {
            // Default: only expand "today"
            group.id == "today"
        };
        log::trace!("Sidebar: Group {} expanded state: {} (id: {})", 
            group.name, is_expanded, group.id);
        
        ui.horizontal(|ui| {
            let arrow = if is_expanded { "▼" } else { "▶" };
            if ui.button(arrow).clicked() {
                self.toggle_group(&group.id);
            }
            
            ui.label(RichText::new(&group.name).strong());
            ui.label(format!("({})", group.metadata.count));
        });
        
        if is_expanded {
            ui.indent(&group.id, |ui| {
                log::trace!("Sidebar: Rendering {} conversations in expanded group {}", group.conversations.len(), group.name);
                for item in &group.conversations {
                    self.render_conversation_item(ui, item, app_state, theme);
                }
            });
        }
    }

    /// Render a conversation item
    fn render_conversation_item(
        &mut self, 
        ui: &mut Ui, 
        item: &ConversationItem, 
        app_state: &mut AppState, 
        theme: &AppTheme
    ) {
        log::trace!("Sidebar: Rendering conversation item: {} ({})", item.display.title, item.summary.id);
        let is_selected = item.selected;
        let is_editing = self.editing_conversation_id == Some(item.summary.id);
        
        let item_color = if is_selected {
            theme.accent_color().gamma_multiply(0.3)
        } else {
            Color32::TRANSPARENT
        };
        
        let response = ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let status_icon = get_status_icon(item.summary.status.clone());
                if !status_icon.is_empty() {
                    ui.label(status_icon);
                }
                
                if is_editing {
                    let response = ui.text_edit_singleline(&mut self.edit_buffer);
                    if response.lost_focus() {
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.pending_action = Some(SidebarAction::RenameConversation(
                                item.summary.id,
                                self.edit_buffer.clone()
                            ));
                        }
                        self.editing_conversation_id = None;
                        self.edit_buffer.clear();
                    }
                } else {
                    let response = ui.add(egui::Label::new(
                        RichText::new(&item.display.title).underline()
                    ).sense(egui::Sense::click()));
                    
                    // Debug logging
                    if response.hovered() {
                        log::trace!("Sidebar: Hovering over conversation {}", item.summary.id);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    
                    if response.clicked() {
                        log::info!("Sidebar: Conversation {} clicked", item.summary.id);
                        self.pending_action = Some(SidebarAction::SwitchToConversation(item.summary.id));
                    } else if ui.input(|i| i.pointer.any_click()) && response.hovered() {
                        log::warn!("Sidebar: Click detected while hovering but response.clicked() was false for {}", item.summary.id);
                    }
                }
                
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(&item.display.time_display).small().color(theme.muted_text_color()));
                });
            });
            
            if !item.display.indicators.is_empty() {
                ui.horizontal(|ui| {
                    for indicator in &item.display.indicators {
                        ui.label(&indicator.display);
                    }
                });
            }
            
            if let Some(preview) = &item.preview {
                if self.config.show_previews {
                    ui.label(RichText::new(preview).small().color(theme.muted_text_color()));
                }
            }
            
            // Display tags if present
            if !item.summary.tags.is_empty() && self.config.show_tags {
                ui.horizontal_wrapped(|ui| {
                    for tag in &item.summary.tags {
                        // Create a small frame for each tag
                        let tag_frame = Frame {
                            inner_margin: Margin { left: 4, right: 4, top: 1, bottom: 1 },
                            corner_radius: egui::Rounding::same(3),
                            fill: theme.accent_color().gamma_multiply(0.2),
                            stroke: egui::Stroke::NONE,
                            ..Default::default()
                        };
                        
                        tag_frame.show(ui, |ui| {
                            ui.label(RichText::new(tag).small().color(theme.accent_color()));
                        });
                    }
                });
            }
        });
        
        // Make the group interactive and handle clicks
        let group_response = response.response.interact(egui::Sense::click());
        if group_response.clicked() && !is_editing {
            log::info!("Sidebar: Conversation {} clicked (via group)", item.summary.id);
            self.pending_action = Some(SidebarAction::SwitchToConversation(item.summary.id));
        }
        
        group_response.context_menu(|ui| {
            if ui.button("📝 Rename").clicked() {
                self.editing_conversation_id = Some(item.summary.id);
                self.edit_buffer = item.summary.title.clone();
                ui.close_menu();
            }
            if ui.button("🔄 Update Title").on_hover_text("Regenerate title based on conversation content").clicked() {
                self.pending_action = Some(SidebarAction::UpdateConversationTitle(item.summary.id));
                ui.close_menu();
            }
            if ui.button("🗑️ Delete").clicked() {
                self.pending_action = Some(SidebarAction::RequestDeleteConversation(item.summary.id));
                ui.close_menu();
            }
        });
    }

    /// Render simple conversation list
    fn render_simple_conversation_list(
        &mut self, 
        ui: &mut Ui, 
        app_state: &mut AppState, 
        theme: &AppTheme
    ) {
        let conversations = app_state.conversation_list.clone();
        for conv in conversations {
            let item = self.create_conversation_item(conv);
            self.render_conversation_item(ui, &item, app_state, theme);
        }
    }

    /// Handle sidebar actions
    pub(super) fn handle_sidebar_actions(
        &mut self, 
        app_state: &mut AppState, 
        ctx: &egui::Context, 
        conversation_service: Option<Arc<ConversationService>>, 
        app_event_sender: UnboundedSender<AppEvent>
    ) {
        if let Some(action) = self.pending_action.take() {
            match action {
                SidebarAction::SwitchToConversation(id) => {
                    log::info!("Sidebar: Switching to conversation {}", id);
                    app_state.current_conversation_id = Some(id);
                    self.selected_conversation = Some(id);
                    match app_event_sender.send(AppEvent::SwitchToConversation(id)) {
                        Ok(_) => log::info!("Sidebar: Successfully sent SwitchToConversation event"),
                        Err(e) => log::error!("Sidebar: Failed to send SwitchToConversation event: {}", e),
                    }
                }
                SidebarAction::CreateNewConversation => {
                    log::info!("Creating new conversation");
                    
                    // Clear current conversation to start fresh
                    app_state.current_conversation_id = None;
                    app_state.messages.clear();
                    self.selected_conversation = None;
                    
                    // Send event to notify the app about the new conversation
                    if let Err(e) = app_event_sender.send(AppEvent::CreateNewConversation) {
                        log::error!("Failed to send CreateNewConversation event: {}", e);
                    }
                    
                    // The next message will automatically create a new conversation
                    log::info!("Ready to create new conversation on next message");
                }
                SidebarAction::RefreshConversations => {
                    let _ = app_event_sender.send(AppEvent::RefreshConversationList);
                }
                SidebarAction::RequestDeleteConversation(id) => {
                    // TODO: Implement delete confirmation dialog
                }
                SidebarAction::RenameConversation(id, new_name) => {
                    // Use the proper event system to rename conversation
                    if let Err(e) = app_event_sender.send(AppEvent::RenameConversation {
                        conversation_id: id,
                        new_title: new_name,
                    }) {
                        log::error!("Failed to send RenameConversation event: {}", e);
                    }
                }
                SidebarAction::SetWorkspace(id) => {
                    let _ = app_event_sender.send(AppEvent::RefreshConversationList);
                }
                SidebarAction::UpdateConversationTitle(id) => {
                    log::info!("Sidebar: Requesting title update for conversation {}", id);
                    if let Err(e) = app_event_sender.send(AppEvent::UpdateConversationTitle { conversation_id: id }) {
                        log::error!("Failed to send UpdateConversationTitle event: {}", e);
                    }
                }
                _ => {}
            }
        }
    }
    
    /// Get organization mode label
    fn get_organization_mode_label(&self) -> &str {
        match &self.organization_mode {
            OrganizationMode::Recency => "⏰ Recency",
            OrganizationMode::Project => "📁 Project",
            OrganizationMode::Status => "🎯 Status",
            OrganizationMode::Clusters => "🧩 Clusters",
            OrganizationMode::Tags => "🏷️ Tags",
            OrganizationMode::Success => "📈 Success",
            OrganizationMode::Custom(_) => "⚙️ Custom",
        }
    }
}

fn get_status_icon(status: ConversationStatus) -> &'static str {
    match status {
        ConversationStatus::Active => "",
        ConversationStatus::Paused => "",
        ConversationStatus::Completed => "",
        ConversationStatus::Archived => "",
        ConversationStatus::Summarizing => "",
    }
} 