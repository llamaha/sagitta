use egui::{Context, TextEdit, RichText, Color32, Window, Grid, Vec2};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use anyhow::Result;
use log::info;

use crate::config::{SagittaCodeConfig, save_config as save_sagitta_code_config};
use crate::providers::types::{ProviderType, ProviderConfig};
use crate::gui::theme::AppTheme;

/// Modal dialog for first-run provider setup
pub struct ProviderSetupDialog {
    /// Whether the modal is open
    is_open: bool,
    
    /// Currently selected provider
    selected_provider: ProviderType,
    
    /// Provider-specific configurations
    provider_configs: HashMap<ProviderType, ProviderConfig>,
    
    /// Mistral.rs configuration fields
    mistral_base_url: String,
    mistral_api_key: String,
    
    /// Status message for feedback
    status_message: Option<(String, Color32)>,
    
    /// Reference to the app config
    config: Arc<Mutex<SagittaCodeConfig>>,
    
    /// Whether the setup is complete and valid
    setup_complete: bool,
    
    /// Whether to skip this dialog in the future
    dont_show_again: bool,
}

impl ProviderSetupDialog {
    /// Create a new provider setup dialog
    pub fn new(config: Arc<Mutex<SagittaCodeConfig>>) -> Self {
        Self {
            is_open: false,
            selected_provider: ProviderType::ClaudeCode,
            provider_configs: HashMap::new(),
            mistral_base_url: "http://localhost:1234".to_string(),
            mistral_api_key: String::new(),
            status_message: None,
            config,
            setup_complete: false,
            dont_show_again: false,
        }
    }
    
    /// Open the modal for first-run setup
    pub fn open(&mut self) {
        self.is_open = true;
        self.status_message = None;
        self.setup_complete = false;
        self.initialize_default_configs();
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
    
    /// Check if setup is complete
    pub fn is_setup_complete(&self) -> bool {
        self.setup_complete
    }
    
    /// Get the selected provider and configuration
    pub fn get_selected_provider(&self) -> (ProviderType, ProviderConfig) {
        let config = match self.selected_provider {
            ProviderType::ClaudeCode => {
                // Use default Claude Code configuration
                ProviderConfig::default_for_provider(ProviderType::ClaudeCode)
            },
            ProviderType::MistralRs => {
                let mistral_config = crate::providers::types::MistralRsConfig {
                    base_url: self.mistral_base_url.clone(),
                    api_token: if self.mistral_api_key.trim().is_empty() { 
                        None 
                    } else { 
                        Some(self.mistral_api_key.clone()) 
                    },
                    model: None,
                    timeout_seconds: 120,
                };
                mistral_config.into()
            },
            ProviderType::OpenAICompatible => {
                // For now, treat OpenAICompatible the same as MistralRs
                let openai_config = crate::providers::types::OpenAICompatibleConfig {
                    base_url: self.mistral_base_url.clone(),
                    api_key: if self.mistral_api_key.trim().is_empty() { 
                        None 
                    } else { 
                        Some(self.mistral_api_key.clone()) 
                    },
                    model: None,
                    timeout_seconds: 120,
                    max_retries: 3,
                };
                openai_config.into()
            },
        };
        
        (self.selected_provider, config)
    }
    
    /// Initialize default configurations for all providers
    fn initialize_default_configs(&mut self) {
        // Claude Code - use defaults (no special configuration needed)
        self.provider_configs.insert(
            ProviderType::ClaudeCode,
            ProviderConfig::default_for_provider(ProviderType::ClaudeCode)
        );
        
        // Mistral.rs - use current UI values
        let mistral_config = crate::providers::types::MistralRsConfig {
            base_url: self.mistral_base_url.clone(),
            api_token: if self.mistral_api_key.trim().is_empty() { None } else { Some(self.mistral_api_key.clone()) },
            model: None,
            timeout_seconds: 120,
        };
        self.provider_configs.insert(ProviderType::MistralRs, mistral_config.into());
    }
    
    /// Validate the current configuration
    fn validate_configuration(&self) -> Result<(), String> {
        match self.selected_provider {
            ProviderType::ClaudeCode => {
                // Claude Code requires authentication via `claude auth`
                // We assume it's properly configured if selected
                Ok(())
            },
            ProviderType::MistralRs | ProviderType::OpenAICompatible => {
                if self.mistral_base_url.trim().is_empty() {
                    return Err("Base URL is required".to_string());
                }
                
                // Basic URL validation
                if !self.mistral_base_url.starts_with("http://") && !self.mistral_base_url.starts_with("https://") {
                    return Err("Base URL must start with http:// or https://".to_string());
                }
                
                Ok(())
            },
        }
    }
    
    /// Save the selected provider configuration
    pub async fn save_configuration(&mut self) -> Result<()> {
        // Validate configuration first
        if let Err(error) = self.validate_configuration() {
            self.status_message = Some((error, Color32::RED));
            return Ok(()); // Don't close dialog on validation error
        }
        
        let mut config_guard = self.config.lock().await;
        
        // Set the current provider
        config_guard.current_provider = self.selected_provider;
        
        // Update provider configuration
        let (provider_type, provider_config) = self.get_selected_provider();
        config_guard.provider_configs.insert(provider_type, provider_config);
        
        // Mark first run as completed
        config_guard.ui.first_run_completed = true;
        
        // Update the dialog preference based on the checkbox
        config_guard.ui.dialog_preferences.show_provider_setup = !self.dont_show_again;
        
        let config_to_save = config_guard.clone();
        drop(config_guard);
        
        // Save to file
        save_sagitta_code_config(&config_to_save)?;
        
        self.setup_complete = true;
        self.status_message = Some(("Configuration saved successfully!".to_string(), Color32::GREEN));
        
        info!("First-run provider setup completed: {:?}", self.selected_provider);
        
        Ok(())
    }
    
    /// Render the provider setup dialog
    pub fn show(&mut self, ctx: &Context, theme: AppTheme) -> bool {
        if !self.is_open {
            return false;
        }
        
        let mut keep_open = true;
        let mut save_requested = false;
        
        Window::new("✦ Welcome to Sagitta Code!")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(500.0);
                
                // Welcome message
                ui.vertical(|ui| {
                    ui.label(RichText::new("Choose your AI provider to get started:")
                        .size(16.0)
                        .color(theme.text_color()));
                    
                    ui.add_space(12.0);
                    
                    // Provider selection
                    ui.group(|ui| {
                        ui.label(RichText::new("Available Providers:")
                            .size(14.0)
                            .strong()
                            .color(theme.text_color()));
                        
                        ui.add_space(8.0);
                        
                        // Claude Code option
                        let claude_selected = ui.selectable_value(
                            &mut self.selected_provider,
                            ProviderType::ClaudeCode,
                            "◉ Claude Code"
                        ).clicked();
                        
                        if claude_selected {
                            self.initialize_default_configs();
                        }
                        
                        ui.label(RichText::new("• Uses Anthropic's Claude via the claude-code CLI")
                            .size(12.0)
                            .color(theme.hint_text_color()));
                        ui.label(RichText::new("• Requires authentication: run 'claude auth'")
                            .size(12.0)
                            .color(theme.hint_text_color()));
                        
                        ui.add_space(8.0);
                        
                        // Mistral.rs option  
                        let mistral_selected = ui.selectable_value(
                            &mut self.selected_provider,
                            ProviderType::MistralRs,
                            "◈ Mistral.rs (Local)"
                        ).clicked();
                        
                        if mistral_selected {
                            self.initialize_default_configs();
                        }
                        
                        ui.label(RichText::new("• Local AI server with OpenAI-compatible API")
                            .size(12.0)
                            .color(theme.hint_text_color()));
                        ui.label(RichText::new("• Run your own models locally")
                            .size(12.0)
                            .color(theme.hint_text_color()));
                    });
                    
                    ui.add_space(12.0);
                    
                    // Provider-specific configuration
                    ui.group(|ui| {
                        ui.label(RichText::new("Configuration:")
                            .size(14.0)
                            .strong()
                            .color(theme.text_color()));
                        
                        ui.add_space(8.0);
                        
                        match self.selected_provider {
                            ProviderType::ClaudeCode => {
                                ui.label(RichText::new("✅ Claude Code uses your global authentication.")
                                    .color(theme.success_color()));
                                ui.label(RichText::new("Make sure you've run 'claude auth' in your terminal.")
                                    .size(12.0)
                                    .color(theme.hint_text_color()));
                            },
                            ProviderType::MistralRs | ProviderType::OpenAICompatible => {
                                Grid::new("mistral_setup_grid")
                                    .num_columns(2)
                                    .spacing([8.0, 8.0])
                                    .show(ui, |ui| {
                                        ui.label("Base URL:");
                                        ui.add(TextEdit::singleline(&mut self.mistral_base_url)
                                            .hint_text("http://localhost:1234"));
                                        ui.end_row();
                                        
                                        ui.label("API Key (Optional):");
                                        ui.add(TextEdit::singleline(&mut self.mistral_api_key)
                                            .password(true)
                                            .hint_text("Leave empty if no auth required"));
                                        ui.end_row();
                                        
                                    });
                            },
                        }
                    });
                    
                    ui.add_space(12.0);
                    
                    // Status message
                    if let Some((message, color)) = &self.status_message {
                        ui.label(RichText::new(message).color(*color));
                        ui.add_space(8.0);
                    }
                    
                    // Don't show again checkbox
                    ui.checkbox(&mut self.dont_show_again, "Don't show this dialog again");
                    
                    ui.add_space(8.0);
                    
                    // Buttons
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Continue").size(14.0)).clicked() {
                            save_requested = true;
                        }
                        
                        ui.add_space(8.0);
                        
                        if ui.button(RichText::new("Skip for now").size(14.0)).clicked() {
                            // Use Claude Code as default and complete setup
                            self.selected_provider = ProviderType::ClaudeCode;
                            self.initialize_default_configs();
                            save_requested = true;
                        }
                    });
                });
            });
        
        // Handle save request
        if save_requested {
            // Save configuration synchronously (we'll update the config directly)
            if let Ok(mut config_guard) = self.config.try_lock() {
                // Update configuration
                config_guard.current_provider = self.selected_provider;
                let (provider_type, provider_config) = self.get_selected_provider();
                config_guard.provider_configs.insert(provider_type, provider_config);
                config_guard.ui.first_run_completed = true;
                config_guard.ui.dialog_preferences.show_provider_setup = !self.dont_show_again;
                
                let config_to_save = config_guard.clone();
                drop(config_guard);
                
                // Save to file
                match save_sagitta_code_config(&config_to_save) {
                    Ok(_) => {
                        self.setup_complete = true;
                        self.status_message = Some(("Configuration saved successfully!".to_string(), Color32::GREEN));
                        info!("First-run provider setup completed: {:?}", self.selected_provider);
                        keep_open = false;
                    }
                    Err(e) => {
                        self.status_message = Some((format!("Failed to save: {}", e), Color32::RED));
                        log::error!("Failed to save provider configuration: {}", e);
                    }
                }
            } else {
                self.status_message = Some(("Config is locked, please try again".to_string(), Color32::YELLOW));
            }
        }
        
        if !keep_open {
            self.close();
        }
        
        self.is_open
    }
}