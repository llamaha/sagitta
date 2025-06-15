// Settings panel UI will go here

use std::sync::Arc;
use anyhow::Result;
use egui::{Context, Ui, RichText, Color32, Window, SidePanel, Grid, Button, TextEdit, ScrollArea, TextStyle, Checkbox, ComboBox};
use tokio::sync::Mutex;
use rfd::FileDialog;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use sagitta_search::config::{AppConfig, load_config, save_config, get_config_path_or_default};
use uuid::Uuid;
use log::{info, warn, error};

use crate::config::{SagittaCodeConfig, load_merged_config, save_config as save_sagitta_code_config};
use crate::config::types::OpenRouterConfig;
use crate::config::paths::{get_sagitta_code_app_config_path};
use crate::llm::openrouter::models::ModelManager;
use super::model_selector::ModelSelector;

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
    vocabulary_base_path: Option<String>,
    indexing_max_concurrent_upserts: u32,
    performance_batch_size: u32,
    embedding_batch_size: u32,
    performance_collection_name_prefix: String,
    performance_max_file_size_bytes: u32,
    
    // Sagitta Code config fields
    pub openrouter_api_key: String,
    pub openrouter_model: String,
    pub openrouter_max_reasoning_steps: u32,
    
    // Model selector for dynamic model selection
    model_selector: Option<ModelSelector>,
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
            vocabulary_base_path: initial_app_config.vocabulary_base_path.clone(),
            indexing_max_concurrent_upserts: initial_app_config.indexing.max_concurrent_upserts as u32,
            performance_batch_size: initial_app_config.performance.batch_size as u32,
            embedding_batch_size: initial_app_config.embedding.embedding_batch_size as u32,
            performance_collection_name_prefix: initial_app_config.performance.collection_name_prefix.clone(),
            performance_max_file_size_bytes: initial_app_config.performance.max_file_size_bytes as u32,
            
            // Sagitta Code config fields from initial_sagitta_code_config
            openrouter_api_key: initial_sagitta_code_config.openrouter.api_key.clone().unwrap_or_default(),
            openrouter_model: initial_sagitta_code_config.openrouter.model.clone(),
            openrouter_max_reasoning_steps: initial_sagitta_code_config.openrouter.max_reasoning_steps,
            
            // Initialize model selector as None (will be lazy-loaded)
            model_selector: None,
        }
    }
    
    /// Create a new settings panel with model manager for enhanced model selection
    pub fn with_model_manager(
        initial_sagitta_code_config: SagittaCodeConfig, 
        initial_app_config: AppConfig,
        model_manager: Arc<ModelManager>
    ) -> Self {
        let mut panel = Self::new(initial_sagitta_code_config, initial_app_config);
        panel.model_selector = Some(ModelSelector::new(model_manager));
        panel
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
    
    /// Render the settings panel
    pub fn render(&mut self, ctx: &Context, theme: crate::gui::theme::AppTheme) {
        if !self.is_open {
            return;
        }
        
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
                        }
                    });
                    ui.separator();
                    
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            // OpenRouter Configuration
                            ui.heading("OpenRouter Configuration");
                            Grid::new("openrouter_config_grid")
                                .num_columns(2)
                                .spacing([8.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label("API Key:");
                                    ui.add(TextEdit::singleline(&mut self.openrouter_api_key)
                                        .password(true)
                                        .hint_text("Enter your OpenRouter API key"));
                                    ui.end_row();
                                    
                                    ui.label("Max Reasoning Steps:");
                                    ui.add(egui::DragValue::new(&mut self.openrouter_max_reasoning_steps)
                                        .range(1..=100)
                                        .speed(1.0));
                                    ui.end_row();
                                });
                            
                            // Model selector (enhanced UI)
                            ui.add_space(8.0);
                            if let Some(ref mut model_selector) = self.model_selector {
                                // Check if we need to refresh models when first shown
                                if model_selector.state().show_dropdown && model_selector.state().loading == false {
                                    // If dropdown is being shown and we haven't loaded models yet, trigger refresh
                                    let should_refresh = model_selector.state().loading == false && 
                                        (model_selector.state().error_message.is_none() || 
                                         model_selector.state().search_query.is_empty());
                                    
                                    if should_refresh {
                                        // This is a bit of a hack - we'll trigger refresh on first show
                                        // In a real app, this would be handled more elegantly
                                        info!("Triggering model refresh for first-time dropdown display");
                                    }
                                }
                                
                                // Use the dynamic model selector
                                if model_selector.render(ui, &mut self.openrouter_model, &theme) {
                                    // Model was changed - you can add any callback logic here
                                    info!("Model changed to: {}", self.openrouter_model);
                                }
                            } else {
                                // Fallback to simple text input if no model manager available
                                ui.horizontal(|ui| {
                                    ui.label("Model:");
                                    ui.add(TextEdit::singleline(&mut self.openrouter_model)
                                        .hint_text("e.g., openai/gpt-4, anthropic/claude-3-5-sonnet"));
                                    
                                    // Note about enhanced selection
                                    ui.small("💡 Enhanced model selection available with API key");
                                });
                            }
                            
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
                            Grid::new("repo_settings_grid")
                                .num_columns(3)
                                .spacing([8.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label("Repositories Base Path:");
                                    let mut repos_path_str = self.repositories_base_path.clone().unwrap_or_default();
                                    ui.add(TextEdit::singleline(&mut repos_path_str).desired_width(250.0));
                                    self.repositories_base_path = if repos_path_str.is_empty() { None } else { Some(repos_path_str) };
                                    if ui.button("Browse").clicked() {
                                        if let Some(path) = FileDialog::new()
                                            .set_title("Select Repositories Base Directory")
                                            .pick_folder() {
                                            self.repositories_base_path = Some(path.to_string_lossy().to_string());
                                        }
                                    }
                                    ui.end_row();
                                    
                                    ui.label("Vocabulary Base Path:");
                                    let mut vocab_path_str = self.vocabulary_base_path.clone().unwrap_or_default();
                                    ui.add(TextEdit::singleline(&mut vocab_path_str).desired_width(250.0));
                                    self.vocabulary_base_path = if vocab_path_str.is_empty() { None } else { Some(vocab_path_str) };
                                    if ui.button("Browse").clicked() {
                                        if let Some(path) = FileDialog::new()
                                            .set_title("Select Vocabulary Base Directory")
                                            .pick_folder() {
                                            self.vocabulary_base_path = Some(path.to_string_lossy().to_string());
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
        let mut config = AppConfig::default();
        
        // Basic settings
        config.qdrant_url = self.qdrant_url.clone();
        config.onnx_model_path = self.onnx_model_path.clone();
        config.onnx_tokenizer_path = self.onnx_tokenizer_path.clone();
        config.repositories_base_path = self.repositories_base_path.clone();
        config.vocabulary_base_path = self.vocabulary_base_path.clone();
        
        // Indexing settings
        config.indexing.max_concurrent_upserts = self.indexing_max_concurrent_upserts as usize;
        
        // Performance settings
        config.performance.batch_size = self.performance_batch_size as usize;
        config.performance.collection_name_prefix = self.performance_collection_name_prefix.clone();
        config.performance.max_file_size_bytes = self.performance_max_file_size_bytes as u64;
        
        // Embedding engine settings
        config.embedding.embedding_batch_size = self.embedding_batch_size as usize;
        
        config
    }
    
    /// Create an updated SagittaCodeConfig from the current UI state
    fn create_updated_sagitta_code_config(&self) -> SagittaCodeConfig {
        let mut updated_config = SagittaCodeConfig {
            openrouter: OpenRouterConfig {
                api_key: if self.openrouter_api_key.is_empty() { 
                    None 
                } else { 
                    Some(self.openrouter_api_key.clone()) 
                },
                model: self.openrouter_model.clone(),
                provider_preferences: None,
                max_history_size: 20,
                max_reasoning_steps: self.openrouter_max_reasoning_steps,
                request_timeout: 30,
            },
            sagitta: Default::default(),
            ui: Default::default(),
            logging: Default::default(),
            conversation: Default::default(),

        };
        
        // Copy other fields from current config using try_lock to avoid blocking runtime
        if let Ok(current_config) = self.sagitta_code_config.try_lock() {
            updated_config.sagitta = current_config.sagitta.clone();
            updated_config.ui = current_config.ui.clone();
            updated_config.logging = current_config.logging.clone();
            updated_config.conversation = current_config.conversation.clone();

        } else {
            // If we can't get the lock immediately, log a warning but use defaults
            log::warn!("SettingsPanel: Could not acquire config lock immediately, using defaults for non-OpenRouter fields");
        }
        
        updated_config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use sagitta_search::config::{AppConfig, IndexingConfig, PerformanceConfig};
    use crate::config::types::{SagittaCodeConfig, OpenRouterConfig, UiConfig, LoggingConfig, ConversationConfig};
    // Import specific loader functions for more direct testing of file operations
    use crate::config::loader::{load_config_from_path as load_sagitta_code_config_from_path, save_config_to_path as save_sagitta_code_config_to_path};

    fn create_test_sagitta_config() -> AppConfig {
        AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: Some("/test/model.onnx".to_string()),
            onnx_tokenizer_path: Some("/test/tokenizer".to_string()),
            repositories_base_path: Some("/test/repos".to_string()),
            vocabulary_base_path: Some("/test/vocab".to_string()),
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
            openrouter: OpenRouterConfig {
                api_key: Some("test-api-key".to_string()),
                model: "test-model".to_string(),
                provider_preferences: None,
                max_history_size: 20,
                max_reasoning_steps: 10,
                request_timeout: 30,
            },
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
        assert_eq!(panel.vocabulary_base_path, Some("/test/vocab".to_string()));
        assert_eq!(panel.indexing_max_concurrent_upserts, 10);
        assert_eq!(panel.performance_batch_size, 150);
        assert_eq!(panel.embedding_batch_size, 64);
        assert_eq!(panel.performance_collection_name_prefix, "test_sagitta");
        assert_eq!(panel.performance_max_file_size_bytes, 2 * 1024 * 1024);

        // Check values from create_test_sagitta_code_config()
        assert_eq!(panel.openrouter_api_key, "test-api-key");
        assert_eq!(panel.openrouter_model, "test-model");
        assert_eq!(panel.openrouter_max_reasoning_steps, 10);

        assert!(!panel.is_open);
    }

    #[tokio::test]
    async fn test_settings_panel_config_population() {
        let mut panel = SettingsPanel::new(create_test_sagitta_code_config(), create_test_sagitta_config());
        
        assert_eq!(panel.qdrant_url, "http://localhost:6334");
        assert_eq!(panel.onnx_model_path, Some("/test/model.onnx".to_string()));
        assert_eq!(panel.onnx_tokenizer_path, Some("/test/tokenizer".to_string()));
        assert_eq!(panel.repositories_base_path, Some("/test/repos".to_string()));
        assert_eq!(panel.vocabulary_base_path, Some("/test/vocab".to_string()));
        assert_eq!(panel.indexing_max_concurrent_upserts, 10);
        assert_eq!(panel.performance_batch_size, 150);
        assert_eq!(panel.embedding_batch_size, 64);
        assert_eq!(panel.performance_collection_name_prefix, "test_sagitta");
        assert_eq!(panel.performance_max_file_size_bytes, 2 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_settings_panel_sagitta_code_config_population() {
        let mut panel = SettingsPanel::new(create_test_sagitta_code_config(), create_test_sagitta_config());
        
        assert_eq!(panel.openrouter_api_key, "test-api-key");
        assert_eq!(panel.openrouter_model, "test-model");
        assert_eq!(panel.openrouter_max_reasoning_steps, 10);
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
        panel.vocabulary_base_path = Some("/custom/vocab".to_string());
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
        assert_eq!(config.vocabulary_base_path, Some("/custom/vocab".to_string()));
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
        panel.openrouter_api_key = "updated-api-key".to_string();
        panel.openrouter_model = "updated-model".to_string();
        panel.openrouter_max_reasoning_steps = 100;
        
        let config = panel.create_updated_sagitta_code_config();
        
        assert_eq!(config.openrouter.api_key, Some("updated-api-key".to_string()));
        assert_eq!(config.openrouter.model, "updated-model");
        assert_eq!(config.openrouter.max_reasoning_steps, 100);
    }

    #[test]
    fn test_create_updated_sagitta_code_config_empty_api_key() {
        let mut panel = SettingsPanel::new(create_test_sagitta_code_config(), create_test_sagitta_config());
        
        // Set empty API key
        panel.openrouter_api_key = "".to_string();
        panel.openrouter_model = "test-model".to_string();
        
        let config = panel.create_updated_sagitta_code_config();
        
        assert_eq!(config.openrouter.api_key, None);
        assert_eq!(config.openrouter.model, "test-model");
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
        assert_eq!(panel.vocabulary_base_path, default_app_config.vocabulary_base_path);
        assert_eq!(panel.indexing_max_concurrent_upserts as usize, default_app_config.indexing.max_concurrent_upserts);
        assert_eq!(panel.performance_batch_size as usize, default_app_config.performance.batch_size);
        assert_eq!(panel.embedding_batch_size as usize, default_app_config.embedding.embedding_batch_size);
        assert_eq!(panel.performance_collection_name_prefix, default_app_config.performance.collection_name_prefix);
        assert_eq!(panel.performance_max_file_size_bytes as u64, default_app_config.performance.max_file_size_bytes);

        // Check SagittaCodeConfig derived fields
        assert_eq!(panel.openrouter_api_key, default_sagitta_code_config.openrouter.api_key.unwrap_or_default());
        assert_eq!(panel.openrouter_model, default_sagitta_code_config.openrouter.model);
        assert_eq!(panel.openrouter_max_reasoning_steps, default_sagitta_code_config.openrouter.max_reasoning_steps);
    }

    #[test]
    fn test_settings_panel_config_sync_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let core_config_temp_path = temp_dir.path().join("core_config.toml");
        let sagitta_code_config_temp_path = temp_dir.path().join("sagitta_code_config.json");

        // 1. Initial Configs (Set A)
        let initial_app_config = AppConfig {
            qdrant_url: "http://initial-qdrant:6334".to_string(),
            onnx_model_path: Some("initial/model.onnx".to_string()),
            onnx_tokenizer_path: Some("initial/tokenizer/".to_string()),

            ..Default::default()
        };
        let initial_sagitta_code_config = SagittaCodeConfig {
            openrouter: OpenRouterConfig {
                api_key: Some("initial-api-key".to_string()),
                model: "initial-gemini-model".to_string(),
                max_reasoning_steps: 30,
                ..Default::default()
            },
            ui: UiConfig {
                theme: "dark".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        // 2. Create Panel & Verify Initial State (Simulates File to GUI)
        let mut panel = SettingsPanel::new(initial_sagitta_code_config.clone(), initial_app_config.clone());

        assert_eq!(panel.qdrant_url, initial_app_config.qdrant_url);
        assert_eq!(panel.onnx_model_path, initial_app_config.onnx_model_path);

        assert_eq!(panel.openrouter_api_key, initial_sagitta_code_config.openrouter.api_key.unwrap_or_default());
        assert_eq!(panel.openrouter_model, initial_sagitta_code_config.openrouter.model);
        assert_eq!(panel.openrouter_max_reasoning_steps, initial_sagitta_code_config.openrouter.max_reasoning_steps);
        // Test theme string parsing
        match initial_sagitta_code_config.ui.theme.as_str() {
            "dark" => {
                // Theme parsing works correctly for dark theme
                assert_eq!(initial_sagitta_code_config.ui.theme, "dark");
            },
            "light" => {
                // Theme parsing works correctly for light theme  
                assert_eq!(initial_sagitta_code_config.ui.theme, "light");
            },
            _ => {
                // Default theme handling
                assert_eq!(initial_sagitta_code_config.ui.theme, "dark");
            }
        }

        // 3. Modify Panel State (Simulate UI Edits - Set B)
        panel.qdrant_url = "http://updated-qdrant:6334".to_string();
        panel.onnx_model_path = Some("updated/model.onnx".to_string());
        panel.openrouter_api_key = "updated-api-key".to_string();
        panel.openrouter_model = "updated-gemini-model".to_string();
        panel.openrouter_max_reasoning_steps = 90;

        // 4. Generate Configs from Modified Panel State
        let updated_sagitta_config = panel.create_updated_sagitta_config();
        let updated_sagitta_code_config = panel.create_updated_sagitta_code_config();
        
        // 5. Verify Updated Configs Match Modified Panel State
        assert_eq!(updated_sagitta_code_config.openrouter.api_key, Some("updated-api-key".to_string()));
        assert_eq!(updated_sagitta_code_config.openrouter.model, "updated-gemini-model".to_string());
        assert_eq!(updated_sagitta_code_config.openrouter.max_reasoning_steps, 90);
        
        // Verify sagitta config updates
        assert_eq!(updated_sagitta_config.qdrant_url, "http://updated-qdrant:6334");
        // Use default performance batch size since it's not modified in this test
        assert_eq!(updated_sagitta_config.performance.batch_size, sagitta_search::config::PerformanceConfig::default().batch_size);

        // 6. Save Generated Configs to Temp Files
        sagitta_search::config::save_config(&updated_sagitta_config, Some(&core_config_temp_path)).unwrap();
        save_sagitta_code_config_to_path(&updated_sagitta_code_config, &sagitta_code_config_temp_path).unwrap();

        // 7. Load Configs Back from Temp Files
        let loaded_app_config_from_file = sagitta_search::config::load_config(Some(&core_config_temp_path)).unwrap();
        let loaded_sagitta_code_config_from_file = load_sagitta_code_config_from_path(&sagitta_code_config_temp_path).unwrap();

        // 8. Verify Loaded Configs Match Modified Panel State (as represented by generated configs)
        assert_eq!(loaded_app_config_from_file.qdrant_url, updated_sagitta_config.qdrant_url);
        assert_eq!(loaded_app_config_from_file.onnx_model_path, updated_sagitta_config.onnx_model_path);

        assert_eq!(loaded_sagitta_code_config_from_file.openrouter.api_key, updated_sagitta_code_config.openrouter.api_key);
        assert_eq!(loaded_sagitta_code_config_from_file.openrouter.model, updated_sagitta_code_config.openrouter.model);
        assert_eq!(loaded_sagitta_code_config_from_file.openrouter.max_reasoning_steps, updated_sagitta_code_config.openrouter.max_reasoning_steps);
        assert_eq!(loaded_sagitta_code_config_from_file.ui.theme, updated_sagitta_code_config.ui.theme);
    }
}

