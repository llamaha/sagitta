// Conversation panel implementation following the pattern of other working panels

use super::simple_manager::SimpleConversationManager;
use crate::gui::theme::AppTheme;
use egui::{Context, SidePanel, Frame, Vec2, Ui};
use uuid::Uuid;
use std::sync::Arc;

/// Represents a single conversation in the panel
#[derive(Debug, Clone)]
pub struct ConversationInfo {
    pub id: Uuid,
    pub title: String,
    pub preview: String,
    pub last_active: chrono::DateTime<chrono::Utc>,
}

/// Actions that can be requested from the panel
#[derive(Debug, Clone)]
pub enum PanelAction {
    SelectConversation(Uuid),
    DeleteConversation(Uuid),
    CreateNewConversation,
    RenameConversation(Uuid, String),
}

/// Main conversation panel following the pattern of repo_panel and settings_panel
pub struct ConversationPanel {
    /// Panel configuration
    visible: bool,
    current_width: f32,
    default_width: f32,
    min_width: f32,
    max_width: f32,
    resizable: bool,
    
    /// Conversation data
    conversations: Vec<ConversationInfo>,
    selected_conversation: Option<Uuid>,
    search_query: String,
    
    /// Pending actions
    pending_action: Option<PanelAction>,
    pending_new_conversation: bool,
    
    /// UI state
    show_delete_confirmation: Option<Uuid>,
    editing_conversation: Option<Uuid>,
    edit_buffer: String,
    // Integration with conversation system (removed for now)
}

impl ConversationPanel {
    /// Create a new conversation panel with default settings
    pub fn new() -> Self {
        Self {
            // Panel configuration
            visible: true,
            current_width: 350.0,
            default_width: 350.0,
            min_width: 250.0,
            max_width: 800.0,
            resizable: true,
            
            // Conversation data
            conversations: Vec::new(),
            selected_conversation: None,
            search_query: String::new(),
            
            // Pending actions
            pending_action: None,
            pending_new_conversation: false,
            
            // UI state
            show_delete_confirmation: None,
            editing_conversation: None,
            edit_buffer: String::new(),
            
            // Integration removed for now
        }
    }
    
    /// Update conversations list from external source
    pub fn update_conversations(&mut self, conversations: Vec<ConversationInfo>) {
        self.conversations = conversations;
    }
    
    /// Render the panel using the standard SidePanel approach
    pub fn render(&mut self, ctx: &Context, theme: AppTheme) {
        if !self.visible {
            return;
        }
        
        SidePanel::left("conversation_panel")
            .default_width(self.current_width)
            .min_width(self.min_width)
            .max_width(self.max_width)
            .resizable(self.resizable)
            .frame(theme.side_panel_frame())
            .show(ctx, |ui| {
                // Update current width from actual panel width
                self.current_width = ui.available_width();
                
                // Render panel content with theme
                self.render_content(ui, theme);
            });
    }
    
    /// Render the panel content
    fn render_content(&mut self, ui: &mut Ui, theme: AppTheme) {
        // Header with title and actions
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Conversations").color(theme.text_color()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(egui::RichText::new("➕").color(theme.button_text_color()))
                    .on_hover_text("New conversation")
                    .clicked() 
                {
                    self.pending_new_conversation = true;
                    self.pending_action = Some(PanelAction::CreateNewConversation);
                }
            });
        });
        
        ui.separator();
        
        // Search bar
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🔍").color(theme.hint_text_color()));
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text("Search conversations...")
            );
            if response.changed() {
                // Search is handled in get_filtered_conversations
            }
        });
        
        ui.separator();
        
        // Conversation list
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let filtered = self.get_filtered_conversations();
                
                if filtered.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new(
                            if self.search_query.is_empty() {
                                "No conversations yet"
                            } else {
                                "No matching conversations"
                            }
                        ).color(theme.hint_text_color()));
                    });
                } else {
                    for conv in filtered {
                        self.render_conversation_item(ui, &conv, theme);
                    }
                }
            });
    }
    
    /// Render a single conversation item
    fn render_conversation_item(&mut self, ui: &mut Ui, conv: &ConversationInfo, theme: AppTheme) {
        let is_selected = self.selected_conversation == Some(conv.id);
        let is_editing = self.editing_conversation == Some(conv.id);
        
        // Create a selectable frame for the entire item
        let mut frame = Frame::none()
            .inner_margin(8.0)
            .rounding(4.0);
        
        if is_selected {
            frame = frame.fill(theme.accent_color().linear_multiply(0.2));
        }
        
        // Make the entire frame interactive
        let response = ui.allocate_response(
            ui.available_size(),
            egui::Sense::click()
        );
        
        // Handle click on conversation
        if response.clicked() && !is_editing {
            self.select_conversation(conv.id);
            self.pending_action = Some(PanelAction::SelectConversation(conv.id));
        }
        
        // Add hover effect
        if response.hovered() && !is_selected {
            frame = frame.fill(theme.accent_color().linear_multiply(0.1));
        }
        
        frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            // Title (editable if in edit mode)
                            if is_editing {
                                if ui.text_edit_singleline(&mut self.edit_buffer).lost_focus() {
                                    if !self.edit_buffer.trim().is_empty() {
                                        self.pending_action = Some(PanelAction::RenameConversation(
                                            conv.id,
                                            self.edit_buffer.trim().to_string()
                                        ));
                                    }
                                    self.editing_conversation = None;
                                }
                            } else {
                                ui.label(egui::RichText::new(&conv.title)
                                    .color(theme.text_color())
                                    .strong());
                            }
                            
                            // Preview
                            ui.label(egui::RichText::new(&conv.preview)
                                .color(theme.hint_text_color())
                                .size(12.0));
                            
                            // Time
                            let time_str = format_time_ago(conv.last_active);
                            ui.label(egui::RichText::new(time_str)
                                .color(theme.hint_text_color())
                                .size(10.0));
                        });
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Delete button
                            if ui.small_button(egui::RichText::new("🗑").color(theme.error_color()))
                                .on_hover_text("Delete conversation")
                                .clicked() 
                            {
                                self.show_delete_confirmation = Some(conv.id);
                            }
                            
                            // Edit button
                            if ui.small_button(egui::RichText::new("✏").color(theme.button_text_color()))
                                .on_hover_text("Rename conversation")
                                .clicked() 
                            {
                                self.editing_conversation = Some(conv.id);
                                self.edit_buffer = conv.title.clone();
                            }
                        });
                    });
                });
        
        // Handle delete confirmation dialog
        if self.show_delete_confirmation == Some(conv.id) {
            self.show_delete_dialog(ui, conv.id, theme);
        }
    }
    
    /// Show delete confirmation dialog
    fn show_delete_dialog(&mut self, ui: &mut Ui, conv_id: Uuid, theme: AppTheme) {
        egui::Window::new("Confirm Delete")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label("Are you sure you want to delete this conversation?");
                ui.label(egui::RichText::new("This action cannot be undone.")
                    .color(theme.error_color())
                    .size(12.0));
                
                ui.separator();
                
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_delete_confirmation = None;
                    }
                    
                    if ui.button(egui::RichText::new("Delete").color(theme.error_color())).clicked() {
                        self.pending_action = Some(PanelAction::DeleteConversation(conv_id));
                        self.show_delete_confirmation = None;
                    }
                });
            });
    }
    
    // Test helper methods
    
    pub fn default_width(&self) -> f32 {
        self.default_width
    }
    
    pub fn min_width(&self) -> f32 {
        self.min_width
    }
    
    pub fn is_resizable(&self) -> bool {
        self.resizable
    }
    
    pub fn get_background_color(&self, theme: &AppTheme) -> egui::Color32 {
        theme.panel_background()
    }
    
    pub fn get_text_color(&self, theme: &AppTheme) -> egui::Color32 {
        theme.text_color()
    }
    
    pub fn set_width(&mut self, width: f32) {
        self.current_width = width.clamp(self.min_width, self.max_width);
    }
    
    pub fn current_width(&self) -> f32 {
        self.current_width
    }
    
    pub fn set_conversations(&mut self, conversations: Vec<(&str, &str)>) {
        self.conversations = conversations.into_iter()
            .enumerate()
            .map(|(i, (title, preview))| ConversationInfo {
                id: Uuid::new_v4(),
                title: title.to_string(),
                preview: preview.to_string(),
                last_active: chrono::Utc::now() - chrono::Duration::minutes(i as i64 * 10),
            })
            .collect();
    }
    
    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.to_string();
    }
    
    pub fn get_filtered_conversations(&self) -> Vec<ConversationInfo> {
        if self.search_query.is_empty() {
            self.conversations.clone()
        } else {
            let query = self.search_query.to_lowercase();
            self.conversations.iter()
                .filter(|conv| {
                    conv.title.to_lowercase().contains(&query) ||
                    conv.preview.to_lowercase().contains(&query)
                })
                .cloned()
                .collect()
        }
    }
    
    pub fn add_conversation(&mut self, id: Uuid, title: &str, preview: &str) {
        self.conversations.push(ConversationInfo {
            id,
            title: title.to_string(),
            preview: preview.to_string(),
            last_active: chrono::Utc::now(),
        });
    }
    
    pub fn conversation_count(&self) -> usize {
        self.conversations.len()
    }
    
    pub fn delete_conversation(&mut self, id: Uuid) -> Result<(), String> {
        let initial_len = self.conversations.len();
        self.conversations.retain(|conv| conv.id != id);
        
        if self.conversations.len() < initial_len {
            if self.selected_conversation == Some(id) {
                self.selected_conversation = None;
            }
            Ok(())
        } else {
            Err("Conversation not found".to_string())
        }
    }
    
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
    
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }
    
    pub fn select_conversation(&mut self, id: Uuid) {
        self.selected_conversation = Some(id);
    }
    
    pub fn selected_conversation(&self) -> Option<Uuid> {
        self.selected_conversation
    }
    
    pub fn request_new_conversation(&mut self) {
        self.pending_new_conversation = true;
    }
    
    pub fn has_pending_new_conversation(&self) -> bool {
        self.pending_new_conversation
    }
    
    pub fn clear_pending_actions(&mut self) {
        self.pending_action = None;
        self.pending_new_conversation = false;
    }
    
    /// Get and clear the pending action
    pub fn take_pending_action(&mut self) -> Option<PanelAction> {
        self.pending_action.take()
    }
}

/// Format time for display
fn format_time_ago(time: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now - time;
    
    if duration.num_seconds() < 60 {
        "Just now".to_string()
    } else if duration.num_minutes() < 60 {
        let mins = duration.num_minutes();
        if mins == 1 {
            "1 min ago".to_string()
        } else {
            format!("{} mins ago", mins)
        }
    } else if duration.num_hours() < 24 {
        let hours = duration.num_hours();
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", hours)
        }
    } else {
        let days = duration.num_days();
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{} days ago", days)
        }
    }
}