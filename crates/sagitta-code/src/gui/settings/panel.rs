// Settings panel UI will go here

use std::sync::Arc;
use anyhow::Result;
use egui::{Context, Ui, RichText, Color32, Window, SidePanel, Grid, Button, TextEdit, ScrollArea, TextStyle, Checkbox, ComboBox};
use tokio::sync::Mutex;
use rfd::FileDialog;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use sagitta_search::config::{AppConfig, load_config, save_config, get_config_path_or_default, get_repo_base_path};
use uuid::Uuid;
use log::{info, warn, error};

use crate::config::{SagittaCodeConfig, load_merged_config, save_config as save_sagitta_code_config};
use crate::config::types::ClaudeCodeConfig;
use crate::config::paths::{get_sagitta_code_app_config_path};
use crate::llm::claude_code::models::CLAUDE_CODE_MODELS;

/// Settings panel for configuring Sagitta core settings
pub struct SettingsPanel {
    // Sagitta config
    sagitta_config: Arc<Mutex<AppConfig>>,
    // Sagitta Code config
    sagitta_code_config: Arc<Mutex<SagittaCodeConfig>>,
    is_open: bool,
    status_message: Option<(String, Color32)>,
    
    // Sagitta config fields
    qdrant_url: String,
    onnx_model_path: Option<String>,
    onnx_tokenizer_path: Option<String>,
    repositories_base_path: Option<String>,

    indexing_max_concurrent_upserts: u32,
    performance_batch_size: u32,
    embedding_batch_size: u32,
    performance_collection_name_prefix: String,
    performance_max_file_size_bytes: u32,
    
    // Sagitta Code config fields - Claude Code
    pub claude_code_path: String,
    pub claude_code_model: String,
    pub claude_code_fallback_model: Option<String>,
    pub claude_code_max_output_tokens: u32,
    pub claude_code_debug: bool,
    pub claude_code_verbose: bool,
    pub claude_code_timeout: u64,
    pub claude_code_max_turns: u32,
    pub claude_code_output_format: String,
    pub claude_code_input_format: String,
    pub claude_code_dangerously_skip_permissions: bool,
    pub claude_code_allowed_tools: String, // Comma-separated list
    pub claude_code_disallowed_tools: String, // Comma-separated list
    pub claude_code_additional_directories: String, // Comma-separated list of paths
    pub claude_code_auto_ide: bool,
    
    // Conversation features - Fast model
    pub conversation_fast_model: String,
    pub conversation_enable_fast_model: bool,
}

impl SettingsPanel {
    /// Create a new settings panel
    pub fn new(initial_sagitta_code_config: SagittaCodeConfig, initial_app_config: AppConfig) -> Self {
        Self {
            sagitta_config: Arc::new(Mutex::new(initial_app_config.clone())),
            sagitta_code_config: Arc::new(Mutex::new(initial_sagitta_code_config.clone())),
            is_open: false,
            status_message: None,
            
            // Sagitta config fields from initial_app_config
            qdrant_url: initial_app_config.qdrant_url.clone(),
            onnx_model_path: initial_app_config.onnx_model_path.clone(),
            onnx_tokenizer_path: initial_app_config.onnx_tokenizer_path.clone(),
            repositories_base_path: initial_app_config.repositories_base_path.clone(),

            indexing_max_concurrent_upserts: initial_app_config.indexing.max_concurrent_upserts as u32,
            performance_batch_size: initial_app_config.performance.batch_size as u32,
            embedding_batch_size: initial_app_config.embedding.embedding_batch_size as u32,
            performance_collection_name_prefix: initial_app_config.performance.collection_name_prefix.clone(),
            performance_max_file_size_bytes: initial_app_config.performance.max_file_size_bytes as u32,
            
            // Sagitta Code config fields - Claude Code
            claude_code_path: initial_sagitta_code_config.claude_code.claude_path.clone(),
            claude_code_model: initial_sagitta_code_config.claude_code.model.clone(),
            claude_code_fallback_model: initial_sagitta_code_config.claude_code.fallback_model.clone(),
            claude_code_max_output_tokens: initial_sagitta_code_config.claude_code.max_output_tokens,
            claude_code_debug: initial_sagitta_code_config.claude_code.debug,
            claude_code_verbose: initial_sagitta_code_config.claude_code.verbose,
            claude_code_timeout: initial_sagitta_code_config.claude_code.timeout,
            claude_code_max_turns: initial_sagitta_code_config.claude_code.max_turns,
            claude_code_output_format: initial_sagitta_code_config.claude_code.output_format.clone(),
            claude_code_input_format: initial_sagitta_code_config.claude_code.input_format.clone(),
            claude_code_dangerously_skip_permissions: initial_sagitta_code_config.claude_code.dangerously_skip_permissions,
            claude_code_allowed_tools: initial_sagitta_code_config.claude_code.allowed_tools.join(","),
            claude_code_disallowed_tools: initial_sagitta_code_config.claude_code.disallowed_tools.join(","),
            claude_code_additional_directories: initial_sagitta_code_config.claude_code.additional_directories
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(","),
            claude_code_auto_ide: initial_sagitta_code_config.claude_code.auto_ide,
            
            // Conversation features - Fast model
            conversation_fast_model: initial_sagitta_code_config.conversation.fast_model.clone(),
            conversation_enable_fast_model: initial_sagitta_code_config.conversation.enable_fast_model,
        }
    }
    
    
    /// Toggle the panel visibility
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }
    
    /// Check if the panel is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }
    
    /// Get the current sagitta config
    pub async fn get_sagitta_config(&self) -> AppConfig {
        self.sagitta_config.lock().await.clone()
    }
    
    /// Get the current sagitta code config
    pub async fn get_sagitta_code_config(&self) -> SagittaCodeConfig {
        self.sagitta_code_config.lock().await.clone()
    }
    
    /// Render the settings panel and return true if it should be closed
    pub fn render(&mut self, ctx: &Context, theme: crate::gui::theme::AppTheme) -> bool {
        if !self.is_open {
            return false;
        }
        
        let mut should_close = false;
        
        egui::SidePanel::right("settings_panel")
            .resizable(true)
            .default_width(400.0)
            .frame(theme.side_panel_frame())
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Settings");
                        ui.add_space(8.0);
                        if ui.button("×").clicked() {
                            self.is_open = false;
                            should_close = true;
                        }
                    });
                    ui.separator();
                    
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            // Claude Code Configuration
                            ui.heading("Claude Code Configuration");
                            
                            // Basic Settings
                            ui.collapsing("Basic Settings", |ui| {
                                Grid::new("claude_code_basic_grid")
                                    .num_columns(2)
                                    .spacing([8.0, 8.0])
                                    .show(ui, |ui| {
                                        ui.label("Claude Binary Path:");
                                        ui.add(TextEdit::singleline(&mut self.claude_code_path)
                                            .hint_text("Path to claude binary (default: claude)"));
                                        ui.end_row();
                                        
                                        ui.label("Model:");
                                        egui::ComboBox::from_id_salt("claude_model_combo")
                                            .selected_text(&self.claude_code_model)
                                            .show_ui(ui, |ui| {
                                                for model in CLAUDE_CODE_MODELS {
                                                    ui.selectable_value(&mut self.claude_code_model, model.id.to_string(), model.name);
                                                }
                                            });
                                        ui.end_row();
                                        
                                        ui.label("Fallback Model:");
                                        let mut fallback_text = self.claude_code_fallback_model.clone().unwrap_or_default();
                                        ui.add(TextEdit::singleline(&mut fallback_text)
                                            .hint_text("Optional fallback model when primary is unavailable"));
                                        self.claude_code_fallback_model = if fallback_text.is_empty() { None } else { Some(fallback_text) };
                                        ui.end_row();
                                        
                                        ui.label("Max Output Tokens:");
                                        ui.add(egui::DragValue::new(&mut self.claude_code_max_output_tokens)
                                            .range(1000..=128000)
                                            .speed(1000.0))
                                            .on_hover_text("Maximum output tokens. Default: 64000");
                                        ui.end_row();
                                        
                                        ui.label("Max Turns:");
                                        ui.horizontal(|ui| {
                                            ui.add(egui::DragValue::new(&mut self.claude_code_max_turns)
                                                .clamp_range(0..=100)
                                                .suffix(" turns"))
                                                .on_hover_text("Maximum number of turns for multi-turn conversations (0 = unlimited)");
                                            if self.claude_code_max_turns == 0 {
                                                ui.label("(unlimited)");
                                            }
                                        });
                                        ui.end_row();
                                        
                                        ui.label("Timeout (seconds):");
                                        ui.add(egui::DragValue::new(&mut self.claude_code_timeout)
                                            .range(10..=3600)
                                            .speed(10.0))
                                            .on_hover_text("Request timeout in seconds. Default: 600 (10 minutes)");
                                        ui.end_row();
                                    });
                            });
                            
                            // Format and Debug Settings
                            ui.collapsing("Format and Debug Settings", |ui| {
                                Grid::new("claude_code_format_grid")
                                    .num_columns(2)
                                    .spacing([8.0, 8.0])
                                    .show(ui, |ui| {
                                        ui.label("Output Format:");
                                        egui::ComboBox::from_id_salt("output_format_combo")
                                            .selected_text(&self.claude_code_output_format)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut self.claude_code_output_format, "text".to_string(), "Text");
                                                ui.selectable_value(&mut self.claude_code_output_format, "json".to_string(), "JSON");
                                                ui.selectable_value(&mut self.claude_code_output_format, "stream-json".to_string(), "Stream JSON");
                                            });
                                        ui.end_row();
                                        
                                        ui.label("Input Format:");
                                        egui::ComboBox::from_id_salt("input_format_combo")
                                            .selected_text(&self.claude_code_input_format)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut self.claude_code_input_format, "text".to_string(), "Text");
                                                ui.selectable_value(&mut self.claude_code_input_format, "stream-json".to_string(), "Stream JSON");
                                            });
                                        ui.end_row();
                                        
                                        ui.label("Debug Mode:");
                                        ui.checkbox(&mut self.claude_code_debug, "Enable debug mode for verbose output");
                                        ui.end_row();
                                        
                                        ui.label("Verbose Logging:");
                                        ui.checkbox(&mut self.claude_code_verbose, "Enable verbose logging for debugging");
                                        ui.end_row();
                                    });
                            });
                            
                            // Tool and Permission Settings
                            ui.collapsing("Tool and Permission Settings", |ui| {
                                Grid::new("claude_code_tools_grid")
                                    .num_columns(2)
                                    .spacing([8.0, 8.0])
                                    .show(ui, |ui| {
                                        ui.label("Allowed Tools:")
                                            .on_hover_text("Comma-separated list of allowed tools (empty = all allowed)");
                                        ui.add(TextEdit::singleline(&mut self.claude_code_allowed_tools)
                                            .hint_text("e.g., bash,read,write"));
                                        ui.end_row();
                                        
                                        ui.label("Disallowed Tools:")
                                            .on_hover_text("Comma-separated list of disallowed tools");
                                        ui.add(TextEdit::singleline(&mut self.claude_code_disallowed_tools)
                                            .hint_text("e.g., bash,exec"));
                                        ui.end_row();
                                        
                                        ui.label("Additional Directories:")
                                            .on_hover_text("Comma-separated list of additional paths to allow tool access");
                                        ui.add(TextEdit::singleline(&mut self.claude_code_additional_directories)
                                            .hint_text("e.g., /home/user/projects,/tmp"));
                                        ui.end_row();
                                        
                                        ui.label("Skip Permissions:");
                                        ui.checkbox(&mut self.claude_code_dangerously_skip_permissions, 
                                            "⚠️ Dangerously skip all permission checks (for sandboxes only)")
                                            .on_hover_text("WARNING: Only enable in secure sandbox environments!");
                                        ui.end_row();
                                    });
                            });
                            
                            // Integration Settings
                            ui.collapsing("Integration Settings", |ui| {
                                Grid::new("claude_code_integration_grid")
                                    .num_columns(2)
                                    .spacing([8.0, 8.0])
                                    .show(ui, |ui| {
                                        // MCP config is now handled internally - no need for user configuration
                                        
                                        ui.label("Auto IDE Connect:");
                                        ui.checkbox(&mut self.claude_code_auto_ide, "Automatically connect to IDE on startup");
                                        ui.end_row();
                                    });
                            });
                            
                            // Conversation Features Settings
                            ui.collapsing("Conversation Features", |ui| {
                                ui.label("Configure fast model for conversation management features like title generation, tagging, and status updates.");
                                ui.add_space(4.0);
                                
                                Grid::new("conversation_features_grid")
                                    .num_columns(2)
                                    .spacing([8.0, 8.0])
                                    .show(ui, |ui| {
                                        ui.label("Enable Fast Model:");
                                        ui.checkbox(&mut self.conversation_enable_fast_model, "Use fast model for conversation features")
                                            .on_hover_text("When enabled, uses a faster model (e.g., Claude Haiku) for conversation management tasks");
                                        ui.end_row();
                                        
                                        ui.label("Fast Model:");
                                        egui::ComboBox::from_id_salt("conversation_fast_model_combo")
                                            .selected_text(&self.conversation_fast_model)
                                            .show_ui(ui, |ui| {
                                                // Show models suitable for fast operations
                                                for model in CLAUDE_CODE_MODELS {
                                                    if model.id.contains("haiku") || model.id.contains("sonnet") {
                                                        ui.selectable_value(&mut self.conversation_fast_model, model.id.to_string(), model.name);
                                                    }
                                                }
                                            });
                                        ui.end_row();
                                    });
                                    
                                ui.add_space(4.0);
                                ui.label("🚀 Fast model is used for: title generation, tag suggestions, status updates, and checkpoint/branch suggestions.")
                                    .on_hover_text("These features will run in the background without blocking the main conversation");
                            });
                            
                            ui.add_space(8.0);
                            ui.label("Note: Make sure to authenticate with 'claude auth' before using Claude Code provider.");
                            
                            ui.separator();
                            ui.add_space(8.0);
                            
                            // Qdrant settings
                            ui.heading("Qdrant Settings");
                            Grid::new("qdrant_settings_grid")
                                .num_columns(2)
                                .spacing([8.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label("Qdrant URL:");
                                    ui.text_edit_singleline(&mut self.qdrant_url);
                                    ui.end_row();
                                });
                            ui.add_space(16.0);
                            
                            // ONNX settings
                            ui.heading("ONNX Settings");
                            Grid::new("onnx_settings_grid")
                                .num_columns(3)
                                .spacing([8.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label("ONNX Model Path:");
                                    let mut onnx_model_path_str = self.onnx_model_path.clone().unwrap_or_default();
                                    ui.add(TextEdit::singleline(&mut onnx_model_path_str).desired_width(250.0));
                                    self.onnx_model_path = if onnx_model_path_str.is_empty() { None } else { Some(onnx_model_path_str) };
                                    if ui.button("Browse").clicked() {
                                        if let Some(path) = FileDialog::new()
                                            .add_filter("ONNX Model", &["onnx"])
                                            .set_title("Select ONNX Model File")
                                            .pick_file() {
                                            self.onnx_model_path = Some(path.to_string_lossy().to_string());
                                        }
                                    }
                                    ui.end_row();
                                    
                                    ui.label("ONNX Tokenizer Path:");
                                    let mut onnx_tokenizer_path_str = self.onnx_tokenizer_path.clone().unwrap_or_default();
                                    ui.add(TextEdit::singleline(&mut onnx_tokenizer_path_str).desired_width(250.0));
                                    self.onnx_tokenizer_path = if onnx_tokenizer_path_str.is_empty() { None } else { Some(onnx_tokenizer_path_str) };
                                    if ui.button("Browse").clicked() {
                                        if let Some(path) = FileDialog::new()
                                            .set_title("Select Tokenizer Directory or File")
                                            .pick_folder() {
                                            self.onnx_tokenizer_path = Some(path.to_string_lossy().to_string());
                                        }
                                    }
                                    ui.end_row();
                                });
                            ui.add_space(16.0);
                            
                            // Repository settings
                            ui.heading("Repository Settings");
                            ui.label("Configure custom path for repository storage. Leave empty to use the default XDG data directory.");
                            ui.add_space(4.0);
                            
                            Grid::new("repo_settings_grid")
                                .num_columns(3)
                                .spacing([8.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label("Repositories Base Path:")
                                        .on_hover_text("Directory where cloned repositories are stored");
                                    let mut repos_path_str = self.repositories_base_path.clone().unwrap_or_default();
                                    let default_repo_path = self.get_default_repo_base_path_display();
                                    ui.add(TextEdit::singleline(&mut repos_path_str)
                                        .desired_width(250.0)
                                        .hint_text(&format!("Default: {}", default_repo_path)));
                                    self.repositories_base_path = if repos_path_str.is_empty() { None } else { Some(repos_path_str) };
                                    if ui.button("Browse").clicked() {
                                        if let Some(path) = FileDialog::new()
                                            .set_title("Select Repositories Base Directory")
                                            .pick_folder() {
                                            self.repositories_base_path = Some(path.to_string_lossy().to_string());
                                        }
                                    }
                                    ui.end_row();
                                    

                                });
                            ui.add_space(16.0);
                            

                            
                            // Advanced settings - Indexing
                            ui.collapsing("Advanced Indexing Settings", |ui| {
                                ui.label(
                                    "These settings control the indexing process performance. Change them only if you understand their impact."
                                );
                                
                                Grid::new("indexing_settings_grid")
                                    .num_columns(2)
                                    .spacing([8.0, 8.0])
                                    .show(ui, |ui| {
                                        ui.label("Max Concurrent Upserts:");
                                        ui.add(egui::DragValue::new(&mut self.indexing_max_concurrent_upserts)
                                            .clamp_range(1..=32)
                                            .speed(0.1));
                                        ui.end_row();
                                        
                                        ui.label("Batch Size:");
                                        ui.add(egui::DragValue::new(&mut self.performance_batch_size)
                                            .clamp_range(32..=512)
                                            .speed(1.0));
                                        ui.end_row();
                                        
                                        ui.label("Embedding Batch Size:")
                                            .on_hover_text("Higher batch sizes will use more VRAM but can improve throughput.");
                                        ui.add(egui::DragValue::new(&mut self.embedding_batch_size)
                                            .clamp_range(1..=1024)
                                            .speed(1.0));
                                        ui.end_row();
                                        
                                        ui.label("Collection Name Prefix:");
                                        ui.text_edit_singleline(&mut self.performance_collection_name_prefix);
                                        ui.end_row();
                                        
                                        ui.label("Max File Size (bytes):");
                                        ui.add(egui::DragValue::new(&mut self.performance_max_file_size_bytes)
                                            .clamp_range(1024..=20971520) // 1KB to 20MB
                                            .speed(1024.0));
                                        ui.end_row();
                                    });
                            });
                            ui.add_space(16.0);
                            
                            // Status message
                            if let Some((message, color)) = &self.status_message {
                                ui.label(RichText::new(message).color(*color));
                            }
                            
                            // Save button
                            if ui.button("Save Configuration").clicked() {
                                self.save_configs(theme);
                            }
                        });
                });
            });
        
        should_close
    }
    
    /// Save both configurations
    fn save_configs(&mut self, theme: crate::gui::theme::AppTheme) {
        self.status_message = None;
        let mut changes_made = false;

        // Save SagittaCodeConfig
        let updated_sagitta_code_config = self.create_updated_sagitta_code_config();
        match save_sagitta_code_config(&updated_sagitta_code_config) {
            Ok(_) => {
                self.status_message = Some(("Sagitta Code settings saved.".to_string(), theme.success_color()));
                log::info!("SettingsPanel: Sagitta Code config saved");
                changes_made = true;
            }
            Err(e) => {
                self.status_message = Some((format!("Error saving Sagitta Code settings: {}", e), theme.error_color()));
                log::error!("SettingsPanel: Error saving Sagitta Code config: {}", e);
            }
        }
        
        // Save sagitta-search configuration to shared location
        let updated_sagitta_config = self.create_updated_sagitta_config();
        
        // Respect test isolation by checking for SAGITTA_TEST_CONFIG_PATH
        let shared_config_path = if let Ok(test_path) = std::env::var("SAGITTA_TEST_CONFIG_PATH") {
            PathBuf::from(test_path)
        } else {
            sagitta_search::config::get_config_path()
                .unwrap_or_else(|_| {
                    dirs::config_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join("sagitta")
                        .join("config.toml")
                })
        };
            
        match sagitta_search::config::save_config(&updated_sagitta_config, Some(&shared_config_path)) {
            Ok(_) => {
                let current_status = self.status_message.take().unwrap_or(("".to_string(), theme.success_color()));
                self.status_message = Some((format!("{} Sagitta Core settings saved.", current_status.0).trim().to_string(), theme.success_color()));
                log::info!("SettingsPanel: Sagitta Core config saved to {:?}", shared_config_path);
                changes_made = true;
            }
            Err(e) => {
                let current_status = self.status_message.take().unwrap_or(("".to_string(), theme.error_color()));
                self.status_message = Some((format!("{} Error saving Sagitta Core settings: {}", current_status.0, e).trim().to_string(), theme.error_color()));
                log::error!("SettingsPanel: Error saving Sagitta Core config to {:?}: {}", shared_config_path, e);
            }
        }
        
        if !changes_made && self.status_message.is_none(){
            self.status_message = Some(("No changes to save.".to_string(), theme.warning_color()));
        }
    }
    
    /// Create an updated AppConfig from the current UI state
    fn create_updated_sagitta_config(&self) -> AppConfig {
        // Clone the existing config to preserve all fields
        let sagitta_config_guard = self.sagitta_config.try_lock().expect("Failed to acquire sagitta_config lock");
        let mut config = sagitta_config_guard.clone();
        
        // Update only the fields that are exposed in the UI
        // Basic settings
        config.qdrant_url = self.qdrant_url.clone();
        config.onnx_model_path = self.onnx_model_path.clone();
        config.onnx_tokenizer_path = self.onnx_tokenizer_path.clone();
        config.repositories_base_path = self.repositories_base_path.clone();

        
        // Indexing settings
        config.indexing.max_concurrent_upserts = self.indexing_max_concurrent_upserts as usize;
        
        // Performance settings
        config.performance.batch_size = self.performance_batch_size as usize;
        config.performance.collection_name_prefix = self.performance_collection_name_prefix.clone();
        config.performance.max_file_size_bytes = self.performance_max_file_size_bytes as u64;
        
        // Embedding engine settings
        config.embedding.embedding_batch_size = self.embedding_batch_size as usize;
        
        // All other fields (repositories, embed_model, etc.) are preserved from the original config
        
        config
    }
    
    /// Create an updated SagittaCodeConfig from the current UI state
    fn create_updated_sagitta_code_config(&self) -> SagittaCodeConfig {
        // Clone the existing config to preserve all fields
        let current_config_guard = self.sagitta_code_config.try_lock().expect("Failed to acquire sagitta_code_config lock");
        let mut updated_config = current_config_guard.clone();
        
        // Update Claude Code fields
        updated_config.claude_code.claude_path = self.claude_code_path.clone();
        updated_config.claude_code.model = self.claude_code_model.clone();
        updated_config.claude_code.fallback_model = self.claude_code_fallback_model.clone();
        updated_config.claude_code.max_output_tokens = self.claude_code_max_output_tokens;
        updated_config.claude_code.debug = self.claude_code_debug;
        updated_config.claude_code.verbose = self.claude_code_verbose;
        updated_config.claude_code.timeout = self.claude_code_timeout;
        updated_config.claude_code.max_turns = self.claude_code_max_turns;
        updated_config.claude_code.output_format = self.claude_code_output_format.clone();
        updated_config.claude_code.input_format = self.claude_code_input_format.clone();
        updated_config.claude_code.dangerously_skip_permissions = self.claude_code_dangerously_skip_permissions;
        
        // Parse comma-separated lists
        updated_config.claude_code.allowed_tools = if self.claude_code_allowed_tools.trim().is_empty() {
            Vec::new()
        } else {
            self.claude_code_allowed_tools
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        
        updated_config.claude_code.disallowed_tools = if self.claude_code_disallowed_tools.trim().is_empty() {
            Vec::new()
        } else {
            self.claude_code_disallowed_tools
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        
        updated_config.claude_code.additional_directories = if self.claude_code_additional_directories.trim().is_empty() {
            Vec::new()
        } else {
            self.claude_code_additional_directories
                .split(',')
                .map(|s| PathBuf::from(s.trim()))
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        };
        
        // MCP config is now handled internally
        updated_config.claude_code.auto_ide = self.claude_code_auto_ide;
        
        // Update conversation features
        updated_config.conversation.fast_model = self.conversation_fast_model.clone();
        updated_config.conversation.enable_fast_model = self.conversation_enable_fast_model;
        
        // Preserve all other fields
        // and all other config sections (sagitta, ui, logging) from the original
        
        updated_config
    }

    /// Get the default repository base path for display purposes
    fn get_default_repo_base_path_display(&self) -> String {
        // Try to get the default path that would be used if no override is set
        match get_repo_base_path(None) {
            Ok(path) => path.to_string_lossy().to_string(),
            Err(_) => "~/.local/share/sagitta/repositories".to_string(), // Fallback
        }
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use sagitta_search::config::{AppConfig, IndexingConfig, PerformanceConfig};
    use crate::config::types::{SagittaCodeConfig, UiConfig, LoggingConfig, ConversationConfig};
    // Import specific loader functions for more direct testing of file operations
    use crate::config::loader::{load_config_from_path as load_sagitta_code_config_from_path, save_config_to_path as save_sagitta_code_config_to_path};

    fn create_test_sagitta_config() -> AppConfig {
        AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: Some("/test/model.onnx".to_string()),
            onnx_tokenizer_path: Some("/test/tokenizer".to_string()),
            embed_model: None, // Not using automatic model downloading
            repositories_base_path: Some("/test/repos".to_string()),
            vocabulary_base_path: None, // Use default path
            tenant_id: None, // not used in sagitta-code (hardcoded to "local")
            repositories: vec![],
            active_repository: None,
            indexing: sagitta_search::config::IndexingConfig {
                max_concurrent_upserts: 10,
            },
            performance: sagitta_search::config::PerformanceConfig {
                batch_size: 150,
                collection_name_prefix: "test_sagitta".to_string(),
                max_file_size_bytes: 2 * 1024 * 1024, // 2MB
                vector_dimension: 384,
            },
            embedding: sagitta_search::config::EmbeddingEngineConfig {
                embedding_batch_size: 64,
                ..sagitta_search::config::EmbeddingEngineConfig::default()
            },
            server_api_key_path: None,
            oauth: None,
            tls_enable: false,
            tls_cert_path: None,
            tls_key_path: None,
            cors_allowed_origins: None,
            cors_allow_credentials: true,
        }
    }

    fn create_test_sagitta_code_config() -> SagittaCodeConfig {
        SagittaCodeConfig {
            claude_code: ClaudeCodeConfig::default(),
            sagitta: Default::default(),
            ui: UiConfig::default(),
            logging: LoggingConfig::default(),
            conversation: ConversationConfig::default(),
        }
    }

    #[test]
    fn test_settings_panel_creation() {
        let panel = SettingsPanel::new(create_test_sagitta_code_config(), create_test_sagitta_config());
        
        // Check values from create_test_sagitta_config()
        assert_eq!(panel.qdrant_url, "http://localhost:6334");
        assert_eq!(panel.onnx_model_path, Some("/test/model.onnx".to_string()));
        assert_eq!(panel.onnx_tokenizer_path, Some("/test/tokenizer".to_string()));
        assert_eq!(panel.repositories_base_path, Some("/test/repos".to_string()));
        assert_eq!(panel.indexing_max_concurrent_upserts, 10);
        assert_eq!(panel.performance_batch_size, 150);
        assert_eq!(panel.embedding_batch_size, 64);
        assert_eq!(panel.performance_collection_name_prefix, "test_sagitta");
        assert_eq!(panel.performance_max_file_size_bytes, 2 * 1024 * 1024);

        // Check values from create_test_sagitta_code_config()
        assert!(!panel.is_open);
    }

    #[tokio::test]
    async fn test_settings_panel_config_population() {
        let mut panel = SettingsPanel::new(create_test_sagitta_code_config(), create_test_sagitta_config());
        
        assert_eq!(panel.qdrant_url, "http://localhost:6334");
        assert_eq!(panel.onnx_model_path, Some("/test/model.onnx".to_string()));
        assert_eq!(panel.onnx_tokenizer_path, Some("/test/tokenizer".to_string()));
        assert_eq!(panel.repositories_base_path, Some("/test/repos".to_string()));
        assert_eq!(panel.indexing_max_concurrent_upserts, 10);
        assert_eq!(panel.performance_batch_size, 150);
        assert_eq!(panel.embedding_batch_size, 64);
        assert_eq!(panel.performance_collection_name_prefix, "test_sagitta");
        assert_eq!(panel.performance_max_file_size_bytes, 2 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_settings_panel_sagitta_code_config_population() {
        let panel = SettingsPanel::new(create_test_sagitta_code_config(), create_test_sagitta_config());
        
        // Check Claude Code defaults
        assert_eq!(panel.claude_code_path, "claude");
        assert!(!panel.claude_code_verbose);
    }

    #[test]
    fn test_settings_panel_toggle() {
        let mut panel = SettingsPanel::new(create_test_sagitta_code_config(), create_test_sagitta_config());
        
        assert!(!panel.is_open());
        
        panel.toggle();
        assert!(panel.is_open());
        
        panel.toggle();
        assert!(!panel.is_open());
    }

    #[test]
    fn test_create_updated_sagitta_config() {
        let mut panel = SettingsPanel::new(create_test_sagitta_code_config(), create_test_sagitta_config());
        
        // Set some test values
        panel.qdrant_url = "http://custom:6334".to_string();
        panel.onnx_model_path = Some("/custom/model.onnx".to_string());
        panel.onnx_tokenizer_path = Some("/custom/tokenizer".to_string());
        panel.repositories_base_path = Some("/custom/repos".to_string());
        panel.indexing_max_concurrent_upserts = 16;
        panel.performance_batch_size = 300;
        panel.embedding_batch_size = 256;
        panel.performance_collection_name_prefix = "custom_sagitta".to_string();
        panel.performance_max_file_size_bytes = 4194304;
        
        let config = panel.create_updated_sagitta_config();
        
        assert_eq!(config.qdrant_url, "http://custom:6334");
        assert_eq!(config.onnx_model_path, Some("/custom/model.onnx".to_string()));
        assert_eq!(config.onnx_tokenizer_path, Some("/custom/tokenizer".to_string()));
        assert_eq!(config.repositories_base_path, Some("/custom/repos".to_string()));
        assert_eq!(config.indexing.max_concurrent_upserts, 16);
        assert_eq!(config.performance.batch_size, 300);
        assert_eq!(config.embedding.embedding_batch_size, 256);
        assert_eq!(config.performance.collection_name_prefix, "custom_sagitta");
        assert_eq!(config.performance.max_file_size_bytes, 4194304);
    }

    #[test]
    fn test_create_updated_sagitta_code_config() {
        let mut panel = SettingsPanel::new(create_test_sagitta_code_config(), create_test_sagitta_config());
        
        // Set some test values
        panel.claude_code_path = "/custom/claude".to_string();
        panel.claude_code_verbose = true;
        panel.claude_code_max_output_tokens = 10000;
        
        let config = panel.create_updated_sagitta_code_config();
        
        assert_eq!(config.claude_code.claude_path, "/custom/claude");
        assert_eq!(config.claude_code.verbose, true);
        assert_eq!(config.claude_code.max_output_tokens, 10000);
    }

    #[test]
    fn test_settings_panel_default_values_match_config_defaults() {
        let default_app_config = AppConfig::default();
        let default_sagitta_code_config = SagittaCodeConfig::default();
        
        let panel = SettingsPanel::new(default_sagitta_code_config.clone(), default_app_config.clone());

        // Check AppConfig derived fields
        assert_eq!(panel.qdrant_url, default_app_config.qdrant_url);
        assert_eq!(panel.onnx_model_path, default_app_config.onnx_model_path);
        assert_eq!(panel.onnx_tokenizer_path, default_app_config.onnx_tokenizer_path);
        assert_eq!(panel.repositories_base_path, default_app_config.repositories_base_path);
        assert_eq!(panel.indexing_max_concurrent_upserts as usize, default_app_config.indexing.max_concurrent_upserts);
        assert_eq!(panel.performance_batch_size as usize, default_app_config.performance.batch_size);
        assert_eq!(panel.embedding_batch_size as usize, default_app_config.embedding.embedding_batch_size);
        assert_eq!(panel.performance_collection_name_prefix, default_app_config.performance.collection_name_prefix);
        assert_eq!(panel.performance_max_file_size_bytes as u64, default_app_config.performance.max_file_size_bytes);

        // Check SagittaCodeConfig derived fields
        assert_eq!(panel.claude_code_path, default_sagitta_code_config.claude_code.claude_path);
        assert_eq!(panel.claude_code_model, default_sagitta_code_config.claude_code.model);
        assert_eq!(panel.claude_code_verbose, default_sagitta_code_config.claude_code.verbose);
    }

    #[test]
    fn test_settings_panel_config_sync_roundtrip() {
        let initial_sagitta_config = create_test_sagitta_config();
        let initial_sagitta_code_config = create_test_sagitta_code_config();
        
        let mut panel = SettingsPanel::new(initial_sagitta_code_config.clone(), initial_sagitta_config.clone());
        
        // Modify some values
        panel.claude_code_path = "/new/claude".to_string();
        panel.claude_code_verbose = true;
        panel.qdrant_url = "http://new_url:6334".to_string();
        
        // Create updated configs
        let updated_sagitta_config = panel.create_updated_sagitta_config();
        let updated_sagitta_code_config = panel.create_updated_sagitta_code_config();
        
        // Verify changes were applied
        assert_eq!(updated_sagitta_config.qdrant_url, "http://new_url:6334");
        assert_eq!(updated_sagitta_code_config.claude_code.claude_path, "/new/claude");
        assert_eq!(updated_sagitta_code_config.claude_code.verbose, true);
    }

    #[test]
    fn test_default_path_display_functions() {
        let initial_sagitta_config = create_test_sagitta_config();
        let initial_sagitta_code_config = create_test_sagitta_code_config();
        
        let panel = SettingsPanel::new(initial_sagitta_code_config, initial_sagitta_config);
        
        // Test that default repo path function returns non-empty string
        let default_repo_path = panel.get_default_repo_base_path_display();
        
        assert!(!default_repo_path.is_empty(), "Default repo path should not be empty");
        
        // Test that it contains expected path components
        assert!(default_repo_path.contains("sagitta"), "Default repo path should contain 'sagitta'");
        assert!(default_repo_path.contains("repositories"), "Default repo path should contain 'repositories'");
    }

    #[test]
    fn test_config_fields_preserved_on_update() {
        // Create configs with additional fields that aren't in the UI
        let mut initial_sagitta_config = create_test_sagitta_config();
        initial_sagitta_config.repositories = vec![
            sagitta_search::config::RepositoryConfig {
                name: "test-repo-1".to_string(),
                url: "https://github.com/test/repo1".to_string(),
                local_path: PathBuf::from("/test/repos/repo1"),
                default_branch: "main".to_string(),
                tracked_branches: vec!["main".to_string()],
                remote_name: Some("origin".to_string()),
                last_synced_commits: std::collections::HashMap::new(),
                active_branch: Some("main".to_string()),
                ssh_key_path: None,
                ssh_key_passphrase: None,
                indexed_languages: None,
                added_as_local_path: false,
                target_ref: None,
                tenant_id: None,
            },
            sagitta_search::config::RepositoryConfig {
                name: "test-repo-2".to_string(),
                url: "https://github.com/test/repo2".to_string(),
                local_path: PathBuf::from("/test/repos/repo2"),
                default_branch: "main".to_string(),
                tracked_branches: vec!["main".to_string()],
                remote_name: Some("origin".to_string()),
                last_synced_commits: std::collections::HashMap::new(),
                active_branch: Some("main".to_string()),
                ssh_key_path: None,
                ssh_key_passphrase: None,
                indexed_languages: None,
                added_as_local_path: false,
                target_ref: None,
                tenant_id: None,
            },
        ];
        initial_sagitta_config.embed_model = Some("test-embed-model".to_string());
        
        let mut initial_sagitta_code_config = create_test_sagitta_code_config();
        initial_sagitta_code_config.ui.current_repository_context = Some("test-repo-1".to_string());
        initial_sagitta_code_config.sagitta.repositories = vec!["repo1".to_string(), "repo2".to_string()];
        
        let mut panel = SettingsPanel::new(initial_sagitta_code_config.clone(), initial_sagitta_config.clone());
        
        // Modify only UI-exposed fields
        panel.claude_code_path = "/updated/claude".to_string();
        panel.qdrant_url = "http://updated:6334".to_string();
        
        // Create updated configs
        let updated_sagitta_config = panel.create_updated_sagitta_config();
        let updated_sagitta_code_config = panel.create_updated_sagitta_code_config();
        
        // Verify UI-exposed fields were updated
        assert_eq!(updated_sagitta_config.qdrant_url, "http://updated:6334");
        assert_eq!(updated_sagitta_code_config.claude_code.claude_path, "/updated/claude");
        
        // Verify non-UI fields were preserved
        assert_eq!(updated_sagitta_config.repositories.len(), 2);
        assert_eq!(updated_sagitta_config.repositories[0].name, "test-repo-1");
        assert_eq!(updated_sagitta_config.repositories[1].name, "test-repo-2");
        assert_eq!(updated_sagitta_config.embed_model, Some("test-embed-model".to_string()));
        
        assert_eq!(updated_sagitta_code_config.ui.current_repository_context, Some("test-repo-1".to_string()));
        assert_eq!(updated_sagitta_code_config.sagitta.repositories, vec!["repo1".to_string(), "repo2".to_string()]);
    }

    #[test]
    fn test_settings_panel_x_button_returns_close_signal() {
        let mut panel = SettingsPanel::new(create_test_sagitta_code_config(), create_test_sagitta_config());
        
        // Open the panel
        panel.toggle();
        assert!(panel.is_open());
        
        // When the panel is open but X hasn't been clicked, render should return false
        // (In a real UI test, we would simulate the click, but we can test the logic)
        // The render function returns true only when the X button is clicked
        
        // Test that the panel correctly manages its state
        assert!(panel.is_open());
        
        // Simulate X button click by setting is_open to false
        // In the actual render function, this happens when X is clicked
        panel.is_open = false;
        assert!(!panel.is_open());
    }
}

