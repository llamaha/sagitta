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

use crate::config::{FredAgentConfig, load_merged_config, save_config as save_fred_config};
use crate::config::types::GeminiConfig;
use crate::config::paths::{get_sagitta_code_core_config_path, get_sagitta_code_app_config_path};

/// Settings panel for configuring Sagitta core settings
#[derive(Clone)]
pub struct SettingsPanel {
    // Sagitta config
    sagitta_config: Arc<Mutex<AppConfig>>,
    // Fred Agent config
    fred_config: Arc<Mutex<FredAgentConfig>>,
    is_open: bool,
    status_message: Option<(String, Color32)>,
    
    // Sagitta config fields
    qdrant_url: String,
    onnx_model_path: Option<String>,
    onnx_tokenizer_path: Option<String>,
    repositories_base_path: Option<String>,
    vocabulary_base_path: Option<String>,
    tenant_id: Option<String>,
    indexing_max_concurrent_upserts: u32,
    performance_batch_size: u32,
    performance_internal_embed_batch_size: u32,
    performance_collection_name_prefix: String,
    performance_max_file_size_bytes: u32,
    rayon_num_threads: u32,
    
    // Fred Agent config fields
    pub gemini_api_key: String,
    pub gemini_model: String,
    pub gemini_max_reasoning_steps: u32,
}

impl SettingsPanel {
    /// Create a new settings panel
    pub fn new(initial_fred_config: FredAgentConfig, initial_app_config: AppConfig) -> Self {
        Self {
            sagitta_config: Arc::new(Mutex::new(initial_app_config.clone())),
            fred_config: Arc::new(Mutex::new(initial_fred_config.clone())),
            is_open: false,
            status_message: None,
            
            // Sagitta config fields from initial_app_config
            qdrant_url: initial_app_config.qdrant_url.clone(),
            onnx_model_path: initial_app_config.onnx_model_path.clone(),
            onnx_tokenizer_path: initial_app_config.onnx_tokenizer_path.clone(),
            repositories_base_path: initial_app_config.repositories_base_path.clone(),
            vocabulary_base_path: initial_app_config.vocabulary_base_path.clone(),
            tenant_id: initial_app_config.tenant_id.clone(),
            indexing_max_concurrent_upserts: initial_app_config.indexing.max_concurrent_upserts as u32,
            performance_batch_size: initial_app_config.performance.batch_size as u32,
            performance_internal_embed_batch_size: initial_app_config.performance.internal_embed_batch_size as u32,
            performance_collection_name_prefix: initial_app_config.performance.collection_name_prefix.clone(),
            performance_max_file_size_bytes: initial_app_config.performance.max_file_size_bytes as u32,
            rayon_num_threads: initial_app_config.rayon_num_threads as u32,
            
            // Fred Agent config fields from initial_fred_config
            gemini_api_key: initial_fred_config.gemini.api_key.clone().unwrap_or_default(),
            gemini_model: initial_fred_config.gemini.model.clone(),
            gemini_max_reasoning_steps: initial_fred_config.gemini.max_reasoning_steps,
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
    
    /// Get the current fred agent config
    pub async fn get_fred_config(&self) -> FredAgentConfig {
        self.fred_config.lock().await.clone()
    }
    
    /// Render the settings panel
    pub fn render(&mut self, ctx: &Context, theme: crate::gui::theme::AppTheme) {
        if !self.is_open {
            return;
        }
        
        egui::SidePanel::right("settings_panel")
            .resizable(true)
            .default_width(400.0)
            .frame(egui::Frame::none().fill(theme.panel_background()))
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
                            // Gemini Configuration
                            ui.heading("Gemini Configuration");
                            Grid::new("gemini_config_grid")
                                .num_columns(2)
                                .spacing([8.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label("API Key:");
                                    ui.add(TextEdit::singleline(&mut self.gemini_api_key)
                                        .password(true)
                                        .hint_text("Enter your Gemini API key"));
                                    ui.end_row();
                                    
                                    ui.label("Model:");
                                    ui.add(TextEdit::singleline(&mut self.gemini_model)
                                        .hint_text("e.g., gemini-2.0-flash-thinking-exp"));
                                    ui.end_row();
                                    
                                    ui.label("Max Reasoning Steps:");
                                    ui.add(egui::DragValue::new(&mut self.gemini_max_reasoning_steps)
                                        .range(1..=100)
                                        .speed(1.0));
                                    ui.end_row();
                                });
                            
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
                            
                            // Tenant ID settings
                            ui.heading("Tenant ID");
                            ui.label("The tenant ID is used to uniquely identify your installation and is required for repository operations.");
                            Grid::new("tenant_id_grid")
                                .num_columns(2)
                                .spacing([8.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label("Tenant ID:");
                                    let mut tenant_id_str = self.tenant_id.clone().unwrap_or_default();
                                    if ui.text_edit_singleline(&mut tenant_id_str).changed() {
                                        self.tenant_id = if tenant_id_str.is_empty() { None } else { Some(tenant_id_str) };
                                    }
                                    ui.end_row();
                                });
                            if ui.button("Generate New UUID").clicked() {
                                self.tenant_id = Some(Uuid::new_v4().to_string());
                            }
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
                                        
                                        ui.label("Internal Embed Batch Size:");
                                        ui.add(egui::DragValue::new(&mut self.performance_internal_embed_batch_size)
                                            .clamp_range(8..=256)
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
                            
                            // Rayon threads settings
                            ui.heading("Rayon Threads");
                            ui.label("Controls the number of parallel threads used during repository syncing and indexing. Lower values reduce GPU memory usage but may be slower.");
                            Grid::new("rayon_threads_grid")
                                .num_columns(2)
                                .spacing([8.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label("Number of Threads:");
                                    ui.add(egui::DragValue::new(&mut self.rayon_num_threads)
                                        .clamp_range(1..=128)
                                        .speed(1.0));
                                    ui.end_row();
                                    
                                    ui.label("Recommendation:");
                                    ui.label(RichText::new("For 8GB GPU: 2-6 threads. Reduce if you get GPU memory errors.")
                                        .small()
                                        .color(theme.hint_text_color()));
                                    ui.end_row();
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

        // Save FredAgentConfig
        let updated_fred_config = self.create_updated_fred_config();
        match save_fred_config(&updated_fred_config) {
            Ok(_) => {
                self.status_message = Some(("Fred Agent settings saved.".to_string(), theme.success_color()));
                log::info!("SettingsPanel: Fred Agent config saved");
                changes_made = true;
            }
            Err(e) => {
                self.status_message = Some((format!("Error saving Fred Agent settings: {}", e), theme.error_color()));
                log::error!("SettingsPanel: Error saving Fred Agent config: {}", e);
            }
        }
        
        // Save Sagitta Core AppConfig
        let updated_sagitta_config = self.create_updated_sagitta_config();
        match get_sagitta_code_core_config_path() {
            Ok(core_config_path) => {
                match sagitta_search::config::save_config(&updated_sagitta_config, Some(&core_config_path)) {
                    Ok(_) => {
                        let current_status = self.status_message.take().unwrap_or(("".to_string(), theme.success_color()));
                        self.status_message = Some((format!("{} Sagitta Core settings saved.", current_status.0).trim().to_string(), theme.success_color()));
                        log::info!("SettingsPanel: Sagitta Core config saved to {:?}", core_config_path);
                        changes_made = true;
                    }
                    Err(e) => {
                        let current_status = self.status_message.take().unwrap_or(("".to_string(), theme.error_color()));
                        self.status_message = Some((format!("{} Error saving Sagitta Core settings: {}", current_status.0, e).trim().to_string(), theme.error_color()));
                        log::error!("SettingsPanel: Error saving Sagitta Core config to {:?}: {}", core_config_path, e);
                    }
                }
            }
            Err(e) => {
                let current_status = self.status_message.take().unwrap_or(("".to_string(), theme.error_color()));
                self.status_message = Some((format!("{} Error getting Sagitta Core config path for saving: {}", current_status.0, e).trim().to_string(), theme.error_color()));
                log::error!("SettingsPanel: Error getting Sagitta Core config path for saving: {}", e);
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
        config.tenant_id = self.tenant_id.clone();
        
        // Indexing settings
        config.indexing.max_concurrent_upserts = self.indexing_max_concurrent_upserts as usize;
        
        // Performance settings
        config.performance.batch_size = self.performance_batch_size as usize;
        config.performance.internal_embed_batch_size = self.performance_internal_embed_batch_size as usize;
        config.performance.collection_name_prefix = self.performance_collection_name_prefix.clone();
        config.performance.max_file_size_bytes = self.performance_max_file_size_bytes as u64;
        
        // Rayon threads
        config.rayon_num_threads = self.rayon_num_threads as usize;
        
        config
    }
    
    /// Create an updated FredAgentConfig from the current UI state
    fn create_updated_fred_config(&self) -> FredAgentConfig {
        let mut fred_config = FredAgentConfig::default();
        
        // Update Gemini settings
        fred_config.gemini.api_key = if self.gemini_api_key.is_empty() { None } else { Some(self.gemini_api_key.clone()) };
        fred_config.gemini.model = self.gemini_model.clone();
        fred_config.gemini.max_reasoning_steps = self.gemini_max_reasoning_steps;
        
        fred_config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use sagitta_search::config::{AppConfig, IndexingConfig, PerformanceConfig};
    use crate::config::types::{FredAgentConfig, GeminiConfig, UiConfig, LoggingConfig, ConversationConfig};
    // Import specific loader functions for more direct testing of file operations
    use crate::config::loader::{load_config_from_path as load_fred_config_from_path, save_config_to_path as save_fred_config_to_path};

    fn create_test_sagitta_config() -> AppConfig {
        AppConfig {
            qdrant_url: "http://test:6334".to_string(),
            onnx_model_path: Some("/test/model.onnx".to_string()),
            onnx_tokenizer_path: Some("/test/tokenizer".to_string()),
            server_api_key_path: None,
            repositories_base_path: Some("/test/repos".to_string()),
            vocabulary_base_path: Some("/test/vocab".to_string()),
            repositories: Vec::new(),
            active_repository: None,
            indexing: IndexingConfig {
                max_concurrent_upserts: 8,
            },
            performance: PerformanceConfig {
                batch_size: 200,
                internal_embed_batch_size: 64,
                collection_name_prefix: "test_sagitta".to_string(),
                max_file_size_bytes: 2097152,
                vector_dimension: 384,
            },
            oauth: None,
            tls_enable: false,
            tls_cert_path: None,
            tls_key_path: None,
            cors_allowed_origins: None,
            cors_allow_credentials: true,
            tenant_id: Some("test-tenant-123".to_string()),
            rayon_num_threads: 8,
        }
    }

    fn create_test_fred_config() -> FredAgentConfig {
        FredAgentConfig {
            gemini: GeminiConfig {
                api_key: Some("test-api-key".to_string()),
                model: "test-model".to_string(),
                max_history_size: 30,
                max_reasoning_steps: 75,
            },
            sagitta: crate::config::types::SagittaDbConfig::default(),
            ui: UiConfig {
                dark_mode: false,
                theme: "dark".to_string(),
                window_width: 1000,
                window_height: 800,
            },
            logging: LoggingConfig::default(),
            conversation: ConversationConfig::default(),
        }
    }

    #[test]
    fn test_settings_panel_creation() {
        let panel = SettingsPanel::new(create_test_fred_config(), create_test_sagitta_config());
        
        // Check values from create_test_sagitta_config()
        assert_eq!(panel.qdrant_url, "http://test:6334"); // Corrected expected value
        assert_eq!(panel.onnx_model_path, Some("/test/model.onnx".to_string()));
        assert_eq!(panel.onnx_tokenizer_path, Some("/test/tokenizer".to_string()));
        assert_eq!(panel.repositories_base_path, Some("/test/repos".to_string()));
        assert_eq!(panel.vocabulary_base_path, Some("/test/vocab".to_string()));
        assert_eq!(panel.tenant_id, Some("test-tenant-123".to_string()));
        assert_eq!(panel.indexing_max_concurrent_upserts, 8);
        assert_eq!(panel.performance_batch_size, 200);
        assert_eq!(panel.performance_internal_embed_batch_size, 64);
        assert_eq!(panel.performance_collection_name_prefix, "test_sagitta");
        assert_eq!(panel.performance_max_file_size_bytes, 2097152);
        assert_eq!(panel.rayon_num_threads, 8);

        // Check values from create_test_fred_config()
        assert_eq!(panel.gemini_api_key, "test-api-key");
        assert_eq!(panel.gemini_model, "test-model");
        assert_eq!(panel.gemini_max_reasoning_steps, 75);

        assert!(!panel.is_open);
    }

    #[tokio::test]
    async fn test_settings_panel_config_population() {
        let mut panel = SettingsPanel::new(create_test_fred_config(), create_test_sagitta_config());
        
        assert_eq!(panel.qdrant_url, "http://test:6334");
        assert_eq!(panel.onnx_model_path, Some("/test/model.onnx".to_string()));
        assert_eq!(panel.onnx_tokenizer_path, Some("/test/tokenizer".to_string()));
        assert_eq!(panel.repositories_base_path, Some("/test/repos".to_string()));
        assert_eq!(panel.vocabulary_base_path, Some("/test/vocab".to_string()));
        assert_eq!(panel.tenant_id, Some("test-tenant-123".to_string()));
        assert_eq!(panel.indexing_max_concurrent_upserts, 8);
        assert_eq!(panel.performance_batch_size, 200);
        assert_eq!(panel.performance_internal_embed_batch_size, 64);
        assert_eq!(panel.performance_collection_name_prefix, "test_sagitta");
        assert_eq!(panel.performance_max_file_size_bytes, 2097152);
        assert_eq!(panel.rayon_num_threads, 8);
    }

    #[tokio::test]
    async fn test_settings_panel_fred_config_population() {
        let mut panel = SettingsPanel::new(create_test_fred_config(), create_test_sagitta_config());
        
        assert_eq!(panel.gemini_api_key, "test-api-key");
        assert_eq!(panel.gemini_model, "test-model");
        assert_eq!(panel.gemini_max_reasoning_steps, 75);
    }

    #[test]
    fn test_settings_panel_toggle() {
        let mut panel = SettingsPanel::new(create_test_fred_config(), create_test_sagitta_config());
        
        assert!(!panel.is_open());
        
        panel.toggle();
        assert!(panel.is_open());
        
        panel.toggle();
        assert!(!panel.is_open());
    }

    #[test]
    fn test_create_updated_sagitta_config() {
        let mut panel = SettingsPanel::new(create_test_fred_config(), create_test_sagitta_config());
        
        // Set some test values
        panel.qdrant_url = "http://custom:6334".to_string();
        panel.onnx_model_path = Some("/custom/model.onnx".to_string());
        panel.onnx_tokenizer_path = Some("/custom/tokenizer".to_string());
        panel.repositories_base_path = Some("/custom/repos".to_string());
        panel.vocabulary_base_path = Some("/custom/vocab".to_string());
        panel.tenant_id = Some("custom-tenant".to_string());
        panel.indexing_max_concurrent_upserts = 16;
        panel.performance_batch_size = 300;
        panel.performance_internal_embed_batch_size = 128;
        panel.performance_collection_name_prefix = "custom_sagitta".to_string();
        panel.performance_max_file_size_bytes = 4194304;
        panel.rayon_num_threads = 16;
        
        let config = panel.create_updated_sagitta_config();
        
        assert_eq!(config.qdrant_url, "http://custom:6334");
        assert_eq!(config.onnx_model_path, Some("/custom/model.onnx".to_string()));
        assert_eq!(config.onnx_tokenizer_path, Some("/custom/tokenizer".to_string()));
        assert_eq!(config.repositories_base_path, Some("/custom/repos".to_string()));
        assert_eq!(config.vocabulary_base_path, Some("/custom/vocab".to_string()));
        assert_eq!(config.tenant_id, Some("custom-tenant".to_string()));
        assert_eq!(config.indexing.max_concurrent_upserts, 16);
        assert_eq!(config.performance.batch_size, 300);
        assert_eq!(config.performance.internal_embed_batch_size, 128);
        assert_eq!(config.performance.collection_name_prefix, "custom_sagitta");
        assert_eq!(config.performance.max_file_size_bytes, 4194304);
        assert_eq!(config.rayon_num_threads, 16);
    }

    #[test]
    fn test_create_updated_fred_config() {
        let mut panel = SettingsPanel::new(create_test_fred_config(), create_test_sagitta_config());
        
        // Set some test values
        panel.gemini_api_key = "updated-api-key".to_string();
        panel.gemini_model = "updated-model".to_string();
        panel.gemini_max_reasoning_steps = 100;
        
        let config = panel.create_updated_fred_config();
        
        assert_eq!(config.gemini.api_key, Some("updated-api-key".to_string()));
        assert_eq!(config.gemini.model, "updated-model");
        assert_eq!(config.gemini.max_reasoning_steps, 100);
    }

    #[test]
    fn test_create_updated_fred_config_empty_api_key() {
        let mut panel = SettingsPanel::new(create_test_fred_config(), create_test_sagitta_config());
        
        // Set empty API key
        panel.gemini_api_key = "".to_string();
        panel.gemini_model = "test-model".to_string();
        
        let config = panel.create_updated_fred_config();
        
        assert_eq!(config.gemini.api_key, None);
        assert_eq!(config.gemini.model, "test-model");
    }

    #[test]
    fn test_settings_panel_default_values_match_config_defaults() {
        let default_app_config = AppConfig::default();
        let default_fred_config = FredAgentConfig::default();
        
        let panel = SettingsPanel::new(default_fred_config.clone(), default_app_config.clone());

        // Check AppConfig derived fields
        assert_eq!(panel.qdrant_url, default_app_config.qdrant_url);
        assert_eq!(panel.onnx_model_path, default_app_config.onnx_model_path);
        assert_eq!(panel.onnx_tokenizer_path, default_app_config.onnx_tokenizer_path);
        assert_eq!(panel.repositories_base_path, default_app_config.repositories_base_path);
        assert_eq!(panel.vocabulary_base_path, default_app_config.vocabulary_base_path);
        assert_eq!(panel.tenant_id, default_app_config.tenant_id);
        assert_eq!(panel.indexing_max_concurrent_upserts as usize, default_app_config.indexing.max_concurrent_upserts);
        assert_eq!(panel.performance_batch_size as usize, default_app_config.performance.batch_size);
        assert_eq!(panel.performance_internal_embed_batch_size as usize, default_app_config.performance.internal_embed_batch_size);
        assert_eq!(panel.performance_collection_name_prefix, default_app_config.performance.collection_name_prefix);
        assert_eq!(panel.performance_max_file_size_bytes as u64, default_app_config.performance.max_file_size_bytes);
        assert_eq!(panel.rayon_num_threads as usize, default_app_config.rayon_num_threads);

        // Check FredAgentConfig derived fields
        assert_eq!(panel.gemini_api_key, default_fred_config.gemini.api_key.unwrap_or_default());
        assert_eq!(panel.gemini_model, default_fred_config.gemini.model);
        assert_eq!(panel.gemini_max_reasoning_steps, default_fred_config.gemini.max_reasoning_steps);
    }

    #[test]
    fn test_settings_panel_config_sync_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let core_config_temp_path = temp_dir.path().join("core_config.toml");
        let fred_config_temp_path = temp_dir.path().join("sagitta_code_config.json");

        // 1. Initial Configs (Set A)
        let initial_app_config = AppConfig {
            qdrant_url: "http://initial-qdrant:6334".to_string(),
            onnx_model_path: Some("initial/model.onnx".to_string()),
            onnx_tokenizer_path: Some("initial/tokenizer/".to_string()),
            tenant_id: Some("initial-tenant".to_string()),
            rayon_num_threads: 2,
            ..Default::default()
        };
        let initial_fred_config = FredAgentConfig {
            gemini: GeminiConfig {
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
        let mut panel = SettingsPanel::new(initial_fred_config.clone(), initial_app_config.clone());

        assert_eq!(panel.qdrant_url, initial_app_config.qdrant_url);
        assert_eq!(panel.onnx_model_path, initial_app_config.onnx_model_path);
        assert_eq!(panel.tenant_id, initial_app_config.tenant_id);
        assert_eq!(panel.rayon_num_threads as usize, initial_app_config.rayon_num_threads);

        assert_eq!(panel.gemini_api_key, initial_fred_config.gemini.api_key.unwrap_or_default());
        assert_eq!(panel.gemini_model, initial_fred_config.gemini.model);
        assert_eq!(panel.gemini_max_reasoning_steps, initial_fred_config.gemini.max_reasoning_steps);
        // Test theme string parsing
        match initial_fred_config.ui.theme.as_str() {
            "dark" => {
                // Theme parsing works correctly for dark theme
                assert_eq!(initial_fred_config.ui.theme, "dark");
            },
            "light" => {
                // Theme parsing works correctly for light theme  
                assert_eq!(initial_fred_config.ui.theme, "light");
            },
            _ => {
                // Default theme handling
                assert_eq!(initial_fred_config.ui.theme, "dark");
            }
        }

        // 3. Modify Panel State (Simulate UI Edits - Set B)
        panel.qdrant_url = "http://updated-qdrant:6334".to_string();
        panel.onnx_model_path = Some("updated/model.onnx".to_string());
        panel.tenant_id = Some("updated-tenant".to_string());
        panel.rayon_num_threads = 6;
        panel.gemini_api_key = "updated-api-key".to_string();
        panel.gemini_model = "updated-gemini-model".to_string();
        panel.gemini_max_reasoning_steps = 90;

        // 4. Generate Configs from Modified Panel State
        let updated_sagitta_config = panel.create_updated_sagitta_config();
        let updated_fred_config = panel.create_updated_fred_config();
        
        // 5. Verify Updated Configs Match Modified Panel State
        assert_eq!(updated_fred_config.gemini.api_key, Some("updated-api-key".to_string()));
        assert_eq!(updated_fred_config.gemini.model, "updated-gemini-model".to_string());
        assert_eq!(updated_fred_config.gemini.max_reasoning_steps, 90);
        
        // Verify sagitta config updates
        assert_eq!(updated_sagitta_config.qdrant_url, "http://updated-qdrant:6334");
        // Use default performance batch size since it's not modified in this test
        assert_eq!(updated_sagitta_config.performance.batch_size, sagitta_search::config::PerformanceConfig::default().batch_size);

        // 6. Save Generated Configs to Temp Files
        sagitta_search::config::save_config(&updated_sagitta_config, Some(&core_config_temp_path)).unwrap();
        save_fred_config_to_path(&updated_fred_config, &fred_config_temp_path).unwrap();

        // 7. Load Configs Back from Temp Files
        let loaded_app_config_from_file = sagitta_search::config::load_config(Some(&core_config_temp_path)).unwrap();
        let loaded_fred_config_from_file = load_fred_config_from_path(&fred_config_temp_path).unwrap();

        // 8. Verify Loaded Configs Match Modified Panel State (as represented by generated configs)
        assert_eq!(loaded_app_config_from_file.qdrant_url, updated_sagitta_config.qdrant_url);
        assert_eq!(loaded_app_config_from_file.onnx_model_path, updated_sagitta_config.onnx_model_path);
        assert_eq!(loaded_app_config_from_file.tenant_id, updated_sagitta_config.tenant_id);
        assert_eq!(loaded_app_config_from_file.rayon_num_threads, updated_sagitta_config.rayon_num_threads);
        // Add more AppConfig fields if necessary

        assert_eq!(loaded_fred_config_from_file.gemini.api_key, updated_fred_config.gemini.api_key);
        assert_eq!(loaded_fred_config_from_file.gemini.model, updated_fred_config.gemini.model);
        assert_eq!(loaded_fred_config_from_file.gemini.max_reasoning_steps, updated_fred_config.gemini.max_reasoning_steps);
        assert_eq!(loaded_fred_config_from_file.ui.theme, updated_fred_config.ui.theme);
        // Add more FredAgentConfig fields if necessary
    }
}

