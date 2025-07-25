// Simplified sidebar rendering
use super::simple_types::{SimpleConversationSidebar, SimpleSidebarAction, SimpleConversationItem};
use crate::gui::theme::AppTheme;
use egui::{RichText, ScrollArea, TextEdit, Ui};
use uuid::Uuid;

impl SimpleConversationSidebar {
    /// Render the simplified sidebar
    pub fn render(&mut self, ui: &mut Ui, theme: &AppTheme) {
        let theme = theme.clone();
        
        // Header
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Conversations").color(theme.text_color()));
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // New conversation button
                if ui.button(RichText::new("➕").color(theme.button_text_color()))
                    .on_hover_text("Create new conversation")
                    .clicked() 
                {
                    self.pending_action = Some(SimpleSidebarAction::CreateNewConversation);
                }
                
                // Refresh button
                if ui.button(RichText::new("🔄").color(theme.button_text_color()))
                    .on_hover_text("Refresh conversations")
                    .clicked() 
                {
                    self.pending_action = Some(SimpleSidebarAction::RefreshList);
                }
            });
        });
        
        ui.separator();
        
        // Search box
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").color(theme.hint_text_color()));
            let response = ui.add(
                TextEdit::singleline(&mut self.search_query)
                    .hint_text("Search conversations...")
                    .desired_width(ui.available_width())
            );
            
            // Clear search button
            if !self.search_query.is_empty() && response.has_focus() {
                if ui.small_button(RichText::new("✕").color(theme.hint_text_color())).clicked() {
                    self.search_query.clear();
                }
            }
        });
        
        ui.separator();
        
        // Conversation list
        ScrollArea::vertical()
            .id_salt("conversation_list")
            .show(ui, |ui| {
                // Clone the filtered conversations to avoid borrow conflict
                let filtered: Vec<SimpleConversationItem> = self.filtered_conversations()
                    .into_iter()
                    .cloned()
                    .collect();
                
                if filtered.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new(
                            if self.search_query.is_empty() {
                                "No conversations yet"
                            } else {
                                "No matching conversations"
                            }
                        ).color(theme.hint_text_color()));
                    });
                } else {
                    for conv in filtered {
                        self.render_conversation_item(ui, &theme, conv.id, &conv.title, conv.last_active, conv.is_selected);
                    }
                }
            });
    }
    
    /// Render a single conversation item
    fn render_conversation_item(
        &mut self, 
        ui: &mut Ui, 
        theme: &AppTheme, 
        id: Uuid, 
        title: &str,
        last_active: chrono::DateTime<chrono::Utc>,
        is_selected: bool
    ) {
        let bg_color = if is_selected {
            theme.accent_color().gamma_multiply(0.2) // Darker version of accent color
        } else {
            theme.panel_background()
        };
        
        let text_color = if is_selected {
            theme.accent_color() // Use accent color for selected text
        } else {
            theme.text_color()
        };
        
        // Check if we're editing this conversation
        let is_editing = self.editing_conversation.as_ref()
            .map(|(edit_id, _)| *edit_id == id)
            .unwrap_or(false);
        
        ui.group(|ui| {
            ui.style_mut().visuals.widgets.noninteractive.bg_fill = bg_color;
            
            if is_editing {
                // Edit mode - extract values to avoid borrow issues
                let (should_submit, should_cancel, new_title) = if let Some((_, edit_buffer)) = &mut self.editing_conversation {
                    let result = ui.horizontal(|ui| {
                        let response = ui.add(
                            TextEdit::singleline(edit_buffer)
                                .desired_width(ui.available_width() - 60.0)
                        );
                        
                        let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let escape_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
                        let check_clicked = ui.small_button("✓").clicked();
                        let cancel_clicked = ui.small_button("✕").clicked();
                        
                        let should_submit = enter_pressed || check_clicked;
                        let should_cancel = escape_pressed || cancel_clicked;
                        let new_title = edit_buffer.trim().to_string();
                        
                        (should_submit, should_cancel, new_title)
                    });
                    result.inner
                } else {
                    (false, false, String::new())
                };
                
                // Handle actions after the borrow ends
                if should_submit && !new_title.is_empty() {
                    self.pending_action = Some(SimpleSidebarAction::RenameConversation(id, new_title));
                    self.stop_editing();
                } else if should_cancel {
                    self.stop_editing();
                }
            } else {
                // Normal display mode
                let response = ui.allocate_response(
                    ui.available_size(),
                    egui::Sense::click()
                );
                
                if response.clicked() {
                    self.pending_action = Some(SimpleSidebarAction::SwitchToConversation(id));
                }
                
                // Right-click context menu
                response.context_menu(|ui| {
                    if ui.button("Rename").clicked() {
                        self.start_editing(id, title.to_string());
                        ui.close_menu();
                    }
                    
                    ui.separator();
                    
                    if ui.button(RichText::new("Delete").color(theme.error_color())).clicked() {
                        self.pending_action = Some(SimpleSidebarAction::DeleteConversation(id));
                        ui.close_menu();
                    }
                });
                
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).color(text_color).strong());
                    
                    // Time display
                    let time_ago = format_time_ago(last_active);
                    ui.label(RichText::new(time_ago).color(theme.hint_text_color()).small());
                });
            }
        });
    }
}

/// Format time as relative time ago
fn format_time_ago(time: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(time);
    
    if duration.num_seconds() < 60 {
        "Just now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{} min ago", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{} hours ago", duration.num_hours())
    } else if duration.num_days() < 7 {
        format!("{} days ago", duration.num_days())
    } else if duration.num_weeks() < 4 {
        format!("{} weeks ago", duration.num_weeks())
    } else {
        time.format("%b %d, %Y").to_string()
    }
}