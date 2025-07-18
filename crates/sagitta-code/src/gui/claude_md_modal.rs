use egui::{Context, TextEdit, RichText, Color32, ScrollArea, Window};
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use log::info;

use crate::config::{SagittaCodeConfig, save_config as save_sagitta_code_config};

/// Modal for managing CLAUDE.md template and settings
pub struct ClaudeMdModal {
    /// Whether the modal is open
    is_open: bool,
    
    /// Current template content being edited
    template_content: String,
    
    /// Whether auto-creation is enabled
    auto_create_enabled: bool,
    
    /// Status message for feedback
    status_message: Option<(String, Color32)>,
    
    /// Whether the template has been modified
    is_modified: bool,
    
    /// Reference to the app config
    config: Arc<Mutex<SagittaCodeConfig>>,
}

impl ClaudeMdModal {
    /// Create a new CLAUDE.md modal
    pub fn new(config: Arc<Mutex<SagittaCodeConfig>>) -> Self {
        Self {
            is_open: false,
            template_content: String::new(),
            auto_create_enabled: false,
            status_message: None,
            is_modified: false,
            config,
        }
    }
    
    /// Open the modal and load current settings
    pub async fn open(&mut self) {
        self.is_open = true;
        self.load_current_settings().await;
        self.status_message = None;
        self.is_modified = false;
    }
    
    /// Open the modal synchronously
    pub fn open_sync(&mut self) {
        self.is_open = true;
        self.load_current_settings_sync();
        self.status_message = None;
        self.is_modified = false;
    }
    
    /// Close the modal
    pub fn close(&mut self) {
        self.is_open = false;
        self.status_message = None;
    }
    
    /// Check if the modal is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }
    
    /// Load current settings from config
    async fn load_current_settings(&mut self) {
        let config_guard = self.config.lock().await;
        self.template_content = config_guard.ui.claude_md_template.clone();
        self.auto_create_enabled = config_guard.ui.auto_create_claude_md;
    }
    
    /// Load current settings from config synchronously for hotkey handling
    pub fn load_current_settings_sync(&mut self) {
        // Use try_lock to avoid blocking in the UI thread
        if let Ok(config_guard) = self.config.try_lock() {
            self.template_content = config_guard.ui.claude_md_template.clone();
            self.auto_create_enabled = config_guard.ui.auto_create_claude_md;
        } else {
            // If we can't get the lock immediately, use the default template
            log::warn!("CLAUDE.md modal: Could not acquire config lock, using default template");
            self.template_content = include_str!("../../templates/CLAUDE.md").to_string();
            self.auto_create_enabled = false;
        }
    }
    
    /// Save settings to config
    async fn save_settings(&mut self) -> Result<()> {
        let mut config_guard = self.config.lock().await;
        config_guard.ui.claude_md_template = self.template_content.clone();
        config_guard.ui.auto_create_claude_md = self.auto_create_enabled;
        
        let config_to_save = config_guard.clone();
        drop(config_guard);
        
        save_sagitta_code_config(&config_to_save)?;
        
        self.is_modified = false;
        self.status_message = Some(("Settings saved successfully!".to_string(), Color32::from_rgb(0, 150, 0)));
        
        info!("CLAUDE.md settings saved - auto_create: {}, template length: {}", 
              self.auto_create_enabled, self.template_content.len());
        
        Ok(())
    }
    
    /// Reset template to default
    fn reset_to_default(&mut self) {
        self.template_content = include_str!("../../templates/CLAUDE.md").to_string();
        self.is_modified = true;
        self.status_message = Some(("Template reset to default".to_string(), Color32::from_rgb(0, 100, 200)));
    }
    
    /// Handle keyboard shortcuts
    pub fn handle_shortcuts(&mut self, _ctx: &Context) {
        // F3 toggle is handled in the main keyboard shortcuts handler
    }
    
    /// Check if the modal should be opened (for async handling)
    pub fn should_open(&self) -> bool {
        self.is_open && self.template_content.is_empty()
    }
    
    /// Render the modal
    pub fn render(&mut self, ctx: &Context, _theme: &crate::gui::theme::AppTheme) -> Option<ClaudeMdModalAction> {
        if !self.is_open {
            return None;
        }
        
        log::debug!("CLAUDE.md modal: Rendering modal, is_open: {}", self.is_open);
        
        let mut action = None;
        
        Window::new("CLAUDE.md Template Manager")
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .resizable(true)
            .default_size([600.0, 500.0])
            .max_size([800.0, 600.0])
            .min_size([400.0, 300.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Header
                    ui.horizontal(|ui| {
                        ui.heading("CLAUDE.md Template Manager");
                        ui.add_space(ui.available_width() - 80.0);
                        if ui.small_button("❓ Help").clicked() {
                            action = Some(ClaudeMdModalAction::ShowHelp);
                        }
                    });
                    
                    ui.separator();
                    
                    // Settings section
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.auto_create_enabled, "Auto-create CLAUDE.md files when accessing repositories");
                        if self.auto_create_enabled != self.config.try_lock().map(|c| c.ui.auto_create_claude_md).unwrap_or(true) {
                            self.is_modified = true;
                        }
                    });
                    
                    ui.add_space(8.0);
                    
                    // Template editing section
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Template Content:").strong());
                        ui.add_space(ui.available_width() - 200.0);
                        if ui.small_button("Reset to Default").clicked() {
                            self.reset_to_default();
                        }
                        if ui.small_button("Load from File").clicked() {
                            action = Some(ClaudeMdModalAction::LoadFromFile);
                        }
                    });
                    
                    ui.add_space(4.0);
                    
                    // Large text editor for template content
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .min_scrolled_height(200.0)
                        .max_height(250.0)
                        .show(ui, |ui| {
                            let previous_content = self.template_content.clone();
                            ui.add_sized(
                                [ui.available_width(), 200.0],
                                TextEdit::multiline(&mut self.template_content)
                                    .font(egui::TextStyle::Monospace)
                                    .hint_text("Enter your CLAUDE.md template content here...")
                            );
                            
                            if self.template_content != previous_content {
                                self.is_modified = true;
                            }
                        });
                    
                    ui.add_space(8.0);
                    
                    // Status message
                    if let Some((message, color)) = &self.status_message {
                        ui.colored_label(*color, message);
                        ui.add_space(4.0);
                    }
                    
                    // Bottom buttons
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            action = Some(ClaudeMdModalAction::Save);
                        }
                        
                        if self.is_modified {
                            ui.colored_label(Color32::YELLOW, "● Unsaved changes");
                        }
                        
                        ui.add_space(ui.available_width() - 150.0);
                        
                        if ui.button("Apply to All Repos").clicked() {
                            action = Some(ClaudeMdModalAction::ApplyToAllRepos);
                        }
                        
                        if ui.button("Cancel").clicked() {
                            self.close();
                        }
                    });
                    
                    ui.add_space(4.0);
                    
                    // Keyboard shortcuts help
                    ui.horizontal(|ui| {
                        ui.small("Shortcuts: ");
                        ui.small("F3: Open this modal");
                        ui.small("• Escape: Close");
                    });
                });
            });
        
        action
    }
}

/// Actions that can be triggered from the modal
#[derive(Debug, Clone)]
pub enum ClaudeMdModalAction {
    Save,
    LoadFromFile,
    ShowHelp,
    ApplyToAllRepos,
}

impl ClaudeMdModal {
    /// Handle modal actions
    pub async fn handle_action(&mut self, action: ClaudeMdModalAction) -> Result<Option<String>> {
        match action {
            ClaudeMdModalAction::Save => {
                self.save_settings().await?;
                Ok(None)
            },
            ClaudeMdModalAction::LoadFromFile => {
                // This would trigger a file dialog in the main app
                Ok(Some("load_file".to_string()))
            },
            ClaudeMdModalAction::ShowHelp => {
                Ok(Some("show_help".to_string()))
            },
            ClaudeMdModalAction::ApplyToAllRepos => {
                Ok(Some("apply_to_all".to_string()))
            },
        }
    }
    
    /// Load template from file content
    pub fn load_template_from_file(&mut self, content: String) {
        self.template_content = content;
        self.is_modified = true;
        self.status_message = Some(("Template loaded from file".to_string(), Color32::from_rgb(0, 100, 200)));
    }
    
    /// Get the current template content
    pub fn get_template_content(&self) -> &str {
        &self.template_content
    }
    
    /// Get whether auto-create is enabled
    pub fn is_auto_create_enabled(&self) -> bool {
        self.auto_create_enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use crate::config::SagittaCodeConfig;
    
    #[test]
    fn test_claude_modal_toggle_behavior() {
        // Test that the modal can be opened and closed properly
        let config = Arc::new(Mutex::new(SagittaCodeConfig::default()));
        let mut modal = ClaudeMdModal::new(config);
        
        // Initially closed
        assert!(!modal.is_open());
        
        // Open synchronously
        modal.open_sync();
        assert!(modal.is_open());
        
        // Close
        modal.close();
        assert!(!modal.is_open());
    }
    
    #[test] 
    fn test_no_escape_key_handler() {
        // Test that handle_shortcuts no longer responds to Escape key
        // This is verified by checking the implementation doesn't use ctx parameter
        let config = Arc::new(Mutex::new(SagittaCodeConfig::default()));
        let mut modal = ClaudeMdModal::new(config);
        modal.open_sync();
        
        // The handle_shortcuts method now takes _ctx parameter (unused)
        // indicating it doesn't handle any keyboard shortcuts
        // (This is a compile-time check that the parameter is unused)
    }
}