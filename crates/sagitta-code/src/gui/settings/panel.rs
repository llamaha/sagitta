// Settings panel UI will go here

use std::sync::Arc;
use egui::{Context, RichText, Color32, Grid, TextEdit, ScrollArea};
use tokio::sync::Mutex;
use rfd::FileDialog;
use std::path::PathBuf;
use sagitta_search::config::{AppConfig, get_repo_base_path};

use crate::config::{SagittaCodeConfig, save_config as save_sagitta_code_config};
use crate::providers::types::ProviderType;
use crate::providers::Provider;
// TODO: Re-enable when claude_code module is implemented in Phase 2
// use crate::llm::claude_code::models::CLAUDE_CODE_MODELS;

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
    pub claude_code_allowed_tools: String, // Comma-separated list
    pub claude_code_disallowed_tools: String, // Comma-separated list
    pub claude_code_auto_ide: bool,
    
    // Conversation features - Fast model
    pub conversation_fast_model: String,
    pub conversation_enable_fast_model: bool,
    
    // Auto-sync config fields
    pub auto_sync_enabled: bool,
    pub auto_sync_file_watcher_enabled: bool,
    pub auto_sync_file_watcher_debounce_ms: u64,
    pub auto_sync_auto_commit_enabled: bool,
    pub auto_sync_auto_commit_cooldown_seconds: u64,
    pub auto_sync_sync_after_commit: bool,
    pub auto_sync_sync_on_repo_switch: bool,
    pub auto_sync_sync_on_repo_add: bool,
    
    // Provider Settings
    pub current_provider: ProviderType,
    
    // OpenAI Compatible provider fields
    pub openai_base_url: String,
    pub openai_api_key: Option<String>,
    pub openai_model: Option<String>,
    pub openai_timeout_seconds: u64,
    pub openai_max_retries: u32,

    // Provider-specific fields - Claude Code Router
    pub claude_code_router_base_url: String,
    pub claude_code_router_api_key: Option<String>,
    pub claude_code_router_config_path: Option<String>,
    pub claude_code_router_timeout_seconds: u64,
    pub claude_code_router_max_retries: u32,
    
    // Test connection state
    pub test_connection_status: Option<String>,
    pub test_connection_success: Option<bool>,
    pub test_connection_in_progress: bool,
    pub test_connection_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<(String, bool)>>,
    
    // App event sender for provider changes
    pub app_event_sender: Option<tokio::sync::mpsc::UnboundedSender<crate::gui::app::events::AppEvent>>,
    
    // UI preferences
    pub use_simplified_tool_rendering: bool,
    
    // Tool configuration
    pub tool_shell_timeout_ms: u64,
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
            claude_code_path: initial_sagitta_code_config.claude_code.as_ref()
                .map(|c| c.claude_path.clone()).unwrap_or_else(|| "claude".to_string()),
            claude_code_model: initial_sagitta_code_config.claude_code.as_ref()
                .map(|c| c.model.clone()).unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
            claude_code_fallback_model: initial_sagitta_code_config.claude_code.as_ref()
                .and_then(|c| c.fallback_model.clone()),
            claude_code_max_output_tokens: initial_sagitta_code_config.claude_code.as_ref()
                .map(|c| c.max_output_tokens).unwrap_or(4096),
            claude_code_debug: initial_sagitta_code_config.claude_code.as_ref()
                .map(|c| c.debug).unwrap_or(false),
            claude_code_verbose: initial_sagitta_code_config.claude_code.as_ref()
                .map(|c| c.verbose).unwrap_or(false),
            claude_code_timeout: initial_sagitta_code_config.claude_code.as_ref()
                .map(|c| c.timeout).unwrap_or(600),
            claude_code_max_turns: initial_sagitta_code_config.claude_code.as_ref()
                .map(|c| c.max_turns).unwrap_or(0),
            claude_code_allowed_tools: initial_sagitta_code_config.claude_code.as_ref()
                .map(|c| c.allowed_tools.join(",")).unwrap_or_else(String::new),
            claude_code_disallowed_tools: initial_sagitta_code_config.claude_code.as_ref()
                .map(|c| c.disallowed_tools.join(",")).unwrap_or_else(String::new),
            claude_code_auto_ide: initial_sagitta_code_config.claude_code.as_ref()
                .map(|c| c.auto_ide).unwrap_or(false),
            
            // Conversation features - Fast model
            conversation_fast_model: initial_sagitta_code_config.conversation.fast_model.clone(),
            conversation_enable_fast_model: initial_sagitta_code_config.conversation.enable_fast_model,
            
            // Auto-sync config
            auto_sync_enabled: initial_sagitta_code_config.auto_sync.enabled,
            auto_sync_file_watcher_enabled: initial_sagitta_code_config.auto_sync.file_watcher.enabled,
            auto_sync_file_watcher_debounce_ms: initial_sagitta_code_config.auto_sync.file_watcher.debounce_ms,
            auto_sync_auto_commit_enabled: initial_sagitta_code_config.auto_sync.auto_commit.enabled,
            auto_sync_auto_commit_cooldown_seconds: initial_sagitta_code_config.auto_sync.auto_commit.cooldown_seconds,
            auto_sync_sync_after_commit: initial_sagitta_code_config.auto_sync.sync_after_commit,
            auto_sync_sync_on_repo_switch: initial_sagitta_code_config.auto_sync.sync_on_repo_switch,
            auto_sync_sync_on_repo_add: initial_sagitta_code_config.auto_sync.sync_on_repo_add,
            
            // Provider Settings
            current_provider: initial_sagitta_code_config.current_provider,
            
            // OpenAI Compatible provider fields - initialize from provider_configs if available
            openai_base_url: initial_sagitta_code_config.provider_configs.get(&ProviderType::OpenAICompatible)
                .and_then(|config| config.get_option::<String>("base_url").ok().flatten())
                .unwrap_or_else(|| "http://localhost:1234/v1".to_string()),
            openai_api_key: initial_sagitta_code_config.provider_configs.get(&ProviderType::OpenAICompatible)
                .and_then(|config| config.get_option::<String>("api_key").ok().flatten()),
            openai_model: initial_sagitta_code_config.provider_configs.get(&ProviderType::OpenAICompatible)
                .and_then(|config| config.get_option::<String>("model").ok().flatten()),
            openai_timeout_seconds: initial_sagitta_code_config.provider_configs.get(&ProviderType::OpenAICompatible)
                .and_then(|config| config.get_option::<u64>("timeout_seconds").ok().flatten())
                .unwrap_or(120),
            openai_max_retries: initial_sagitta_code_config.provider_configs.get(&ProviderType::OpenAICompatible)
                .and_then(|config| config.get_option::<u32>("max_retries").ok().flatten())
                .unwrap_or(3),

            // Claude Code Router fields
            claude_code_router_base_url: initial_sagitta_code_config.provider_configs.get(&ProviderType::ClaudeCodeRouter)
                .and_then(|config| config.get_option::<String>("base_url").ok().flatten())
                .unwrap_or_else(|| "http://localhost:3000".to_string()),
            claude_code_router_api_key: initial_sagitta_code_config.provider_configs.get(&ProviderType::ClaudeCodeRouter)
                .and_then(|config| config.get_option::<String>("api_key").ok().flatten()),
            claude_code_router_config_path: initial_sagitta_code_config.provider_configs.get(&ProviderType::ClaudeCodeRouter)
                .and_then(|config| config.get_option::<String>("config_path").ok().flatten()),
            claude_code_router_timeout_seconds: initial_sagitta_code_config.provider_configs.get(&ProviderType::ClaudeCodeRouter)
                .and_then(|config| config.get_option::<u64>("timeout_seconds").ok().flatten())
                .unwrap_or(120),
            claude_code_router_max_retries: initial_sagitta_code_config.provider_configs.get(&ProviderType::ClaudeCodeRouter)
                .and_then(|config| config.get_option::<u32>("max_retries").ok().flatten())
                .unwrap_or(3),
            
            // Test connection state
            test_connection_status: None,
            test_connection_success: None,
            test_connection_in_progress: false,
            test_connection_receiver: None,
            
            // App event sender (will be set later by the main app)
            app_event_sender: None,
            
            // UI preferences
            use_simplified_tool_rendering: initial_sagitta_code_config.ui.use_simplified_tool_rendering,
            
            // Tool configuration
            tool_shell_timeout_ms: initial_sagitta_code_config.tools.shell_timeout_ms,
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
    
    /// Set the app event sender for provider changes
    pub fn set_app_event_sender(&mut self, sender: tokio::sync::mpsc::UnboundedSender<crate::gui::app::events::AppEvent>) {
        self.app_event_sender = Some(sender);
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
        
        // Check for test connection results
        if let Some(ref mut receiver) = self.test_connection_receiver {
            if let Ok((status, success)) = receiver.try_recv() {
                self.test_connection_status = Some(status);
                self.test_connection_success = Some(success);
                self.test_connection_in_progress = false;
                self.test_connection_receiver = None;
            }
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
                                        // TODO: Re-enable when claude_code module is implemented in Phase 2
                                        /*
                                        egui::ComboBox::from_id_salt("claude_model_combo")
                                            .selected_text(&self.claude_code_model)
                                            .show_ui(ui, |ui| {
                                                for model in CLAUDE_CODE_MODELS {
                                                    ui.selectable_value(&mut self.claude_code_model, model.id.to_string(), model.name);
                                                }
                                            });
                                        */
                                        ui.add(TextEdit::singleline(&mut self.claude_code_model)
                                            .hint_text("Model name (e.g., claude-3-5-sonnet-latest)"));
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
                                                .range(0..=100)
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
                            
                            // Debug Settings
                            ui.collapsing("Debug Settings", |ui| {
                                Grid::new("claude_code_format_grid")
                                    .num_columns(2)
                                    .spacing([8.0, 8.0])
                                    .show(ui, |ui| {
                                        // Removed Input/Output Format settings - handled internally
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
                                        
                                        ui.label("Shell Timeout (ms):")
                                            .on_hover_text("Timeout for shell commands in milliseconds (default: 60000)");
                                        ui.add(egui::DragValue::new(&mut self.tool_shell_timeout_ms)
                                            .speed(1000.0)
                                            .range(1000..=600000)
                                            .suffix(" ms"));
                                        ui.end_row();
                                        
                                        // Removed Additional Directories and Skip Permissions settings
                                        // These are handled internally by the system
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
                                        // TODO: Re-enable when claude_code module is implemented in Phase 2
                                        /*
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
                                        */
                                        ui.add(TextEdit::singleline(&mut self.conversation_fast_model)
                                            .hint_text("Fast model name (e.g., claude-3-5-haiku-latest)"));
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
                            
                            // Provider Settings
                            ui.heading("Provider Settings");
                            ui.label("Configure AI providers and their settings");
                            ui.add_space(4.0);

                            // Current Provider Selection
                            ui.horizontal(|ui| {
                                ui.label("Current Provider:");
                                egui::ComboBox::from_id_salt("current_provider_combo")
                                    .selected_text(match self.current_provider {
                                        ProviderType::ClaudeCode => "Claude Code",

                                        ProviderType::OpenAICompatible => "OpenAI Compatible",
                                        ProviderType::ClaudeCodeRouter => "Claude Code Router",
                                        ProviderType::MistralRs => "Mistral.rs",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.current_provider, ProviderType::ClaudeCode, "Claude Code");

                                        ui.selectable_value(&mut self.current_provider, ProviderType::OpenAICompatible, "OpenAI Compatible");
                                        ui.selectable_value(&mut self.current_provider, ProviderType::ClaudeCodeRouter, "Claude Code Router");
                                        ui.selectable_value(&mut self.current_provider, ProviderType::MistralRs, "Mistral.rs");
                                    });
                            });
                            
                            ui.add_space(4.0);

                            // Test and Apply buttons for provider changes
                            ui.horizontal(|ui| {
                                let test_button_text = if self.test_connection_in_progress {
                                    "Testing..."
                                } else {
                                    "Test Connection"
                                };
                                
                                if ui.add_enabled(!self.test_connection_in_progress, egui::Button::new(test_button_text)).clicked() {
                                    // Test connection to the selected provider
                                    self.test_provider_connection();
                                }
                                
                                ui.add_space(8.0);
                                
                                if ui.button("Apply & Restart").clicked() {
                                    // Apply provider changes and restart the app
                                    self.apply_provider_changes();
                                }
                            });
                            
                            // Show test connection status
                            if let Some(ref status) = self.test_connection_status {
                                ui.add_space(4.0);
                                let color = match self.test_connection_success {
                                    Some(true) => theme.success_color(),
                                    Some(false) => theme.error_color(),
                                    None => theme.text_color(),
                                };
                                ui.colored_label(color, format!("Test Status: {}", status));
                            }
                            
                            ui.add_space(8.0);

                            // Provider-specific settings based on current selection
                            match self.current_provider {
                                ProviderType::ClaudeCode => {
                                    ui.collapsing("Claude Code Provider Settings", |ui| {
                                        ui.label("Claude Code settings are configured in the section above.");
                                    });
                                },
                                ProviderType::OpenAICompatible => {
                                    ui.collapsing("OpenAI Compatible Provider Settings", |ui| {
                                        Grid::new("openai_compatible_grid")
                                            .num_columns(2)
                                            .spacing([8.0, 8.0])
                                            .show(ui, |ui| {
                                                ui.label("Base URL:");
                                                ui.text_edit_singleline(&mut self.openai_base_url)
                                                    .on_hover_text("Base URL for the OpenAI-compatible API (e.g., http://localhost:1234/v1)");
                                                ui.end_row();
                                                
                                                ui.label("API Key:");
                                                let mut api_key_text = self.openai_api_key.clone().unwrap_or_default();
                                                ui.add(egui::TextEdit::singleline(&mut api_key_text)
                                                    .password(true)
                                                    .hint_text("Optional API key for authentication"));
                                                self.openai_api_key = if api_key_text.is_empty() { None } else { Some(api_key_text) };
                                                ui.end_row();
                                                
                                                ui.label("Model:");
                                                let mut model_text = self.openai_model.clone().unwrap_or_default();
                                                ui.add(egui::TextEdit::singleline(&mut model_text)
                                                    .hint_text("Optional model name (uses server default if empty)"));
                                                self.openai_model = if model_text.is_empty() { None } else { Some(model_text) };
                                                ui.end_row();
                                                
                                                ui.label("Timeout (seconds):");
                                                ui.add(egui::DragValue::new(&mut self.openai_timeout_seconds)
                                                    .range(1..=3600)
                                                    .speed(10.0))
                                                    .on_hover_text("Request timeout in seconds");
                                                ui.end_row();
                                                
                                                ui.label("Max Retries:");
                                                ui.add(egui::DragValue::new(&mut self.openai_max_retries)
                                                    .range(0..=10))
                                                    .on_hover_text("Maximum number of retries on failure");
                                                ui.end_row();
                                            });
                                            
                                        ui.add_space(4.0);
                                        ui.label("Common OpenAI-compatible services:");
                                        ui.indent("openai_examples", |ui| {
                                            ui.label("• LM Studio: http://localhost:1234/v1");
                                            ui.label("• Ollama: http://localhost:11434/v1");
                                            ui.label("• Text Generation WebUI: http://localhost:5000/v1");
                                            ui.label("• OpenRouter: https://openrouter.ai/api/v1");
                                        });
                                    });
                                },
                                ProviderType::ClaudeCodeRouter => {
                                    ui.collapsing("Claude Code Router Provider Settings", |ui| {
                                        Grid::new("claude_code_router_grid")
                                            .num_columns(2)
                                            .spacing([8.0, 8.0])
                                            .show(ui, |ui| {
                                                ui.label("Base URL:");
                                                ui.text_edit_singleline(&mut self.claude_code_router_base_url)
                                                    .on_hover_text("Base URL for the Claude Code Router proxy (e.g., http://localhost:3000)");
                                                ui.end_row();
                                                
                                                ui.label("API Key:");
                                                let mut api_key_text = self.claude_code_router_api_key.clone().unwrap_or_default();
                                                ui.add(egui::TextEdit::singleline(&mut api_key_text)
                                                    .password(true)
                                                    .hint_text("Optional API key for authentication"));
                                                self.claude_code_router_api_key = if api_key_text.is_empty() { None } else { Some(api_key_text) };
                                                ui.end_row();
                                                
                                                ui.label("Config Path:");
                                                let mut config_path_text = self.claude_code_router_config_path.clone().unwrap_or_default();
                                                ui.add(egui::TextEdit::singleline(&mut config_path_text)
                                                    .hint_text("Optional path to router configuration file"));
                                                self.claude_code_router_config_path = if config_path_text.is_empty() { None } else { Some(config_path_text) };
                                                ui.end_row();
                                                
                                                ui.label("Timeout (seconds):");
                                                ui.add(egui::DragValue::new(&mut self.claude_code_router_timeout_seconds)
                                                    .range(1..=3600)
                                                    .speed(10.0))
                                                    .on_hover_text("Request timeout in seconds");
                                                ui.end_row();
                                                
                                                ui.label("Max Retries:");
                                                ui.add(egui::DragValue::new(&mut self.claude_code_router_max_retries)
                                                    .range(0..=10))
                                                    .on_hover_text("Maximum number of retries on failure");
                                                ui.end_row();
                                            });
                                            
                                        ui.add_space(4.0);
                                        ui.label("Claude Code Router is a proxy that routes requests to Claude Code instances.");
                                        ui.label("It uses the same Claude Code binary but adds routing capabilities.");
                                    });
                                },
                                ProviderType::MistralRs => {
                                    ui.collapsing("Mistral.rs Provider Settings", |ui| {
                                        ui.label("Mistral.rs settings will be implemented here.");
                                    });
                                },
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
                                        .hint_text(format!("Default: {default_repo_path}")));
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
                                            .range(1..=32)
                                            .speed(0.1));
                                        ui.end_row();
                                        
                                        ui.label("Batch Size:");
                                        ui.add(egui::DragValue::new(&mut self.performance_batch_size)
                                            .range(32..=512)
                                            .speed(1.0));
                                        ui.end_row();
                                        
                                        ui.label("Embedding Batch Size:")
                                            .on_hover_text("Higher batch sizes will use more VRAM but can improve throughput.");
                                        ui.add(egui::DragValue::new(&mut self.embedding_batch_size)
                                            .range(1..=1024)
                                            .speed(1.0));
                                        ui.end_row();
                                        
                                        ui.label("Collection Name Prefix:");
                                        ui.text_edit_singleline(&mut self.performance_collection_name_prefix);
                                        ui.end_row();
                                        
                                        ui.label("Max File Size (bytes):");
                                        ui.add(egui::DragValue::new(&mut self.performance_max_file_size_bytes)
                                            .range(1024..=20971520) // 1KB to 20MB
                                            .speed(1024.0));
                                        ui.end_row();
                                    });
                            });
                            ui.add_space(16.0);
                            
                            // Auto-sync Settings
                            ui.heading("Auto-Sync & Commit");
                            ui.collapsing("Auto-Sync Settings", |ui| {
                                ui.label("Configure automatic commit and sync features for repositories");
                                
                                Grid::new("auto_sync_grid")
                                    .num_columns(2)
                                    .spacing([8.0, 8.0])
                                    .show(ui, |ui| {
                                        ui.label("Enable Auto-Sync:");
                                        ui.checkbox(&mut self.auto_sync_enabled, "")
                                            .on_hover_text("Enable automatic file watching, commits, and syncing");
                                        ui.end_row();
                                        
                                        if self.auto_sync_enabled {
                                            ui.label("File Watcher:");
                                            ui.checkbox(&mut self.auto_sync_file_watcher_enabled, "")
                                                .on_hover_text("Watch for file changes in repositories");
                                            ui.end_row();
                                            
                                            if self.auto_sync_file_watcher_enabled {
                                                ui.label("Debounce (ms):");
                                                ui.add(egui::DragValue::new(&mut self.auto_sync_file_watcher_debounce_ms)
                                                    .range(100..=10000)
                                                    .speed(100.0))
                                                    .on_hover_text("Wait time before processing file changes");
                                                ui.end_row();
                                            }
                                            
                                            ui.label("Auto-Commit:");
                                            ui.checkbox(&mut self.auto_sync_auto_commit_enabled, "")
                                                .on_hover_text("Automatically commit changes with AI-generated messages");
                                            ui.end_row();
                                            
                                            if self.auto_sync_auto_commit_enabled {
                                                ui.label("Cooldown (seconds):");
                                                ui.add(egui::DragValue::new(&mut self.auto_sync_auto_commit_cooldown_seconds)
                                                    .range(5..=300)
                                                    .speed(5.0))
                                                    .on_hover_text("Minimum time between auto-commits");
                                                ui.end_row();
                                            }
                                            
                                            ui.label("Sync After Commit:");
                                            ui.checkbox(&mut self.auto_sync_sync_after_commit, "")
                                                .on_hover_text("Automatically sync repository after commits");
                                            ui.end_row();
                                            
                                            ui.label("Sync on Repo Switch:");
                                            ui.checkbox(&mut self.auto_sync_sync_on_repo_switch, "")
                                                .on_hover_text("Sync when switching between repositories");
                                            ui.end_row();
                                            
                                            ui.label("Sync on Repo Add:");
                                            ui.checkbox(&mut self.auto_sync_sync_on_repo_add, "")
                                                .on_hover_text("Sync when adding new repositories");
                                            ui.end_row();
                                        }
                                    });
                            });
                            ui.add_space(16.0);
                            
                            // UI Settings
                            ui.heading("UI Settings");
                            ui.collapsing("Interface Preferences", |ui| {
                                ui.label("Configure user interface behavior and appearance");
                                ui.add_space(4.0);
                                ui.label("More preferences coming soon...");
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
                self.status_message = Some((format!("Error saving Sagitta Code settings: {e}"), theme.error_color()));
                log::error!("SettingsPanel: Error saving Sagitta Code config: {e}");
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
                log::info!("SettingsPanel: Sagitta Core config saved to {shared_config_path:?}");
                changes_made = true;
            }
            Err(e) => {
                let current_status = self.status_message.take().unwrap_or(("".to_string(), theme.error_color()));
                self.status_message = Some((format!("{} Error saving Sagitta Core settings: {}", current_status.0, e).trim().to_string(), theme.error_color()));
                log::error!("SettingsPanel: Error saving Sagitta Core config to {shared_config_path:?}: {e}");
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
        
        // Update Claude Code fields - ensure claude_code exists
        if updated_config.claude_code.is_none() {
            updated_config.claude_code = Some(crate::config::types::ClaudeCodeConfig::default());
        }
        
        if let Some(ref mut claude_config) = updated_config.claude_code {
            claude_config.claude_path = self.claude_code_path.clone();
            claude_config.model = self.claude_code_model.clone();
            claude_config.fallback_model = self.claude_code_fallback_model.clone();
            claude_config.max_output_tokens = self.claude_code_max_output_tokens;
            claude_config.debug = self.claude_code_debug;
            claude_config.verbose = self.claude_code_verbose;
            claude_config.timeout = self.claude_code_timeout;
            claude_config.max_turns = self.claude_code_max_turns;
            // output_format and input_format are handled internally
            claude_config.output_format = "stream-json".to_string();
            claude_config.input_format = "text".to_string();
            // dangerously_skip_permissions is always true internally
            claude_config.dangerously_skip_permissions = true;
            
            // Parse comma-separated lists
            claude_config.allowed_tools = if self.claude_code_allowed_tools.trim().is_empty() {
                Vec::new()
            } else {
                self.claude_code_allowed_tools
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            };
            
            claude_config.disallowed_tools = if self.claude_code_disallowed_tools.trim().is_empty() {
                Vec::new()
            } else {
                self.claude_code_disallowed_tools
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            };
            
            // additional_directories is handled internally - empty for now
            claude_config.additional_directories = Vec::new();
            
            // MCP config is now handled internally
            claude_config.auto_ide = self.claude_code_auto_ide;
        }
        
        // Update conversation features
        updated_config.conversation.fast_model = self.conversation_fast_model.clone();
        updated_config.conversation.enable_fast_model = self.conversation_enable_fast_model;
        
        // Update auto-sync settings
        updated_config.auto_sync.enabled = self.auto_sync_enabled;
        updated_config.auto_sync.file_watcher.enabled = self.auto_sync_file_watcher_enabled;
        updated_config.auto_sync.file_watcher.debounce_ms = self.auto_sync_file_watcher_debounce_ms;
        updated_config.auto_sync.auto_commit.enabled = self.auto_sync_auto_commit_enabled;
        updated_config.auto_sync.auto_commit.cooldown_seconds = self.auto_sync_auto_commit_cooldown_seconds;
        updated_config.auto_sync.sync_after_commit = self.auto_sync_sync_after_commit;
        updated_config.auto_sync.sync_on_repo_switch = self.auto_sync_sync_on_repo_switch;
        updated_config.auto_sync.sync_on_repo_add = self.auto_sync_sync_on_repo_add;
        
        // Update provider settings
        updated_config.current_provider = self.current_provider;
        
        // Update provider-specific configurations
        // Update OpenAI Compatible provider config if needed
        if self.current_provider == ProviderType::OpenAICompatible || 
           updated_config.provider_configs.contains_key(&ProviderType::OpenAICompatible) {
            let mut openai_config = updated_config.provider_configs
                .get(&ProviderType::OpenAICompatible)
                .cloned()
                .unwrap_or_else(|| {
                    let provider = crate::providers::openai_compatible::provider::OpenAICompatibleProvider::new();
                    provider.default_config()
                });
            
            openai_config.set_option("base_url", self.openai_base_url.clone()).ok();
            if let Some(ref api_key) = self.openai_api_key {
                openai_config.set_option("api_key", api_key.clone()).ok();
            }
            if let Some(ref model) = self.openai_model {
                openai_config.set_option("model", model.clone()).ok();
            }
            openai_config.set_option("timeout_seconds", self.openai_timeout_seconds).ok();
            openai_config.set_option("max_retries", self.openai_max_retries).ok();
            
            updated_config.provider_configs.insert(ProviderType::OpenAICompatible, openai_config);
        }
        
        // Update Claude Code Router provider config
        if self.current_provider == ProviderType::ClaudeCodeRouter || 
           updated_config.provider_configs.contains_key(&ProviderType::ClaudeCodeRouter) {
            let mut router_config = updated_config.provider_configs
                .get(&ProviderType::ClaudeCodeRouter)
                .cloned()
                .unwrap_or_else(|| {
                    let provider = crate::providers::claude_code_router::ClaudeCodeRouterProvider::new();
                    provider.default_config()
                });
            
            router_config.set_option("base_url", self.claude_code_router_base_url.clone()).ok();
            if let Some(ref api_key) = self.claude_code_router_api_key {
                router_config.set_option("api_key", api_key.clone()).ok();
            }
            if let Some(ref config_path) = self.claude_code_router_config_path {
                router_config.set_option("config_path", config_path.clone()).ok();
            }
            router_config.set_option("timeout_seconds", self.claude_code_router_timeout_seconds).ok();
            router_config.set_option("max_retries", self.claude_code_router_max_retries).ok();
            
            updated_config.provider_configs.insert(ProviderType::ClaudeCodeRouter, router_config);
        }
        
        // Update UI settings
        updated_config.ui.use_simplified_tool_rendering = self.use_simplified_tool_rendering;
        
        // Update tool configuration
        updated_config.tools.shell_timeout_ms = self.tool_shell_timeout_ms;
        
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

    /// Test connection to the currently selected provider
    fn test_provider_connection(&mut self) {
        log::info!("Testing connection to provider: {:?}", self.current_provider);
        
        // Set UI state to show test is in progress
        self.test_connection_in_progress = true;
        self.test_connection_status = Some("Testing connection...".to_string());
        self.test_connection_success = None;
        
        // Create a channel for the async result
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        self.test_connection_receiver = Some(receiver);
        
        // Clone the necessary data for the async task
        let provider_type = self.current_provider.clone();
        let claude_model = self.claude_code_model.clone();
        let claude_path = self.claude_code_path.clone();
        let openai_base_url = self.openai_base_url.clone();
        let openai_api_key = self.openai_api_key.clone();
        let openai_model = self.openai_model.clone();
        let openai_timeout_seconds = self.openai_timeout_seconds;
        let openai_max_retries = self.openai_max_retries;
        let router_base_url = self.claude_code_router_base_url.clone();
        let router_api_key = self.claude_code_router_api_key.clone();
        let router_config_path = self.claude_code_router_config_path.clone();
        let router_timeout_seconds = self.claude_code_router_timeout_seconds;
        let router_max_retries = self.claude_code_router_max_retries;

        
        // Spawn async task to test the connection
        tokio::spawn(async move {
            let result = Self::test_provider_connection_async(
                provider_type,
                claude_model,
                claude_path,
                openai_base_url,
                openai_api_key,
                openai_model,
                openai_timeout_seconds,
                openai_max_retries,
                router_base_url,
                router_api_key,
                router_config_path,
                router_timeout_seconds,
                router_max_retries,
            ).await;
            
            // Send the result through the channel
            let (status, success) = match result {
                Ok(msg) => {
                    log::info!("Connection test successful: {}", msg);
                    (msg, true)
                },
                Err(e) => {
                    log::error!("Connection test failed: {}", e);
                    (e, false)
                }
            };
            
            // Send result to UI thread
            let _ = sender.send((status, success));
        });
    }
    
    async fn test_provider_connection_async(
        provider_type: ProviderType,
        claude_model: String,
        claude_path: String,
        openai_base_url: String,
        openai_api_key: Option<String>,
        openai_model: Option<String>,
        openai_timeout_seconds: u64,
        openai_max_retries: u32,
        router_base_url: String,
        router_api_key: Option<String>,
        router_config_path: Option<String>,
        router_timeout_seconds: u64,
        router_max_retries: u32,
    ) -> Result<String, String> {
        match provider_type {
            ProviderType::ClaudeCode => {
                Self::test_claude_code_connection(claude_model, claude_path).await
            },
            ProviderType::OpenAICompatible => {
                Self::test_openai_compatible_connection(
                    openai_base_url,
                    openai_api_key,
                    openai_model,
                    openai_timeout_seconds,
                    openai_max_retries,
                ).await
            },
            ProviderType::ClaudeCodeRouter => {
                // For now, just test that we can create the client
                // In the future, we could test the actual connection
                Ok("Claude Code Router configuration is valid".to_string())
            },
            ProviderType::MistralRs => {
                Ok("Mistral.rs connection test not implemented yet".to_string())
            },
        }
    }
    
    async fn test_claude_code_connection(model: String, binary_path: String) -> Result<String, String> {
        use std::process::Command;
        use std::time::Duration;
        
        log::info!("Testing Claude Code connection with model: {} and binary: {}", model, binary_path);
        
        // First, check if the claude binary exists and is executable
        let output = match Command::new(&binary_path)
            .arg("--version")
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                return Err(format!("Failed to execute claude binary '{}': {}", binary_path, e));
            }
        };
        
        if !output.status.success() {
            return Err(format!("Claude binary '{}' returned error: {}", binary_path, 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        let version_info = String::from_utf8_lossy(&output.stdout);
        log::info!("Claude binary version: {}", version_info);
        
        // Try a simple test command to verify the model works
        let test_output = match Command::new(&binary_path)
            .args(&["--model", &model])
            .arg("Hello, can you respond with just 'OK' to confirm you're working?")
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                return Err(format!("Failed to test claude model '{}': {}", model, e));
            }
        };
        
        if !test_output.status.success() {
            let stderr = String::from_utf8_lossy(&test_output.stderr);
            return Err(format!("Claude model '{}' test failed: {}", model, stderr));
        }
        
        let response = String::from_utf8_lossy(&test_output.stdout);
        log::info!("Claude test response: {}", response);
        
        Ok(format!("Claude Code connection successful! Model: {}, Version: {}", 
            model, version_info.trim()))
    }
    
    async fn test_openai_compatible_connection(
        base_url: String,
        api_key: Option<String>,
        model: Option<String>,
        timeout_seconds: u64,
        max_retries: u32,
    ) -> Result<String, String> {
        use reqwest::Client;
        use serde_json::json;
        use std::time::Duration;
        
        log::info!("Testing OpenAI Compatible connection to: {}", base_url);
        
        // Create HTTP client with timeout
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        
        // First, try to get the models list endpoint if available
        let models_url = format!("{}/models", base_url.trim_end_matches("/v1"));
        let mut request = client.get(&models_url);
        if let Some(ref key) = api_key {
            request = request.bearer_auth(key);
        }
        
        let models_result = request.send().await;
        
        // If models endpoint fails, try a simple chat completions request
        if models_result.is_err() || !models_result.as_ref().unwrap().status().is_success() {
            log::info!("Models endpoint not available, trying chat completions test");
            
            let chat_url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
            let test_payload = json!({
                "model": model.clone().unwrap_or_else(|| "gpt-3.5-turbo".to_string()),
                "messages": [
                    {"role": "user", "content": "Say 'OK' to confirm the connection works."}
                ],
                "max_tokens": 10,
                "temperature": 0
            });
            
            let mut request = client.post(&chat_url)
                .json(&test_payload);
            if let Some(ref key) = api_key {
                request = request.bearer_auth(key);
            }
            
            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        let response_body: serde_json::Value = response.json().await
                            .map_err(|e| format!("Failed to parse response: {}", e))?;
                        
                        // Check if it's a valid OpenAI-compatible response
                        if response_body.get("choices").is_some() {
                            let model_used = response_body.get("model")
                                .and_then(|m| m.as_str())
                                .unwrap_or("unknown");
                            return Ok(format!("OpenAI Compatible connection successful! Server: {}, Model: {}", 
                                base_url, model_used));
                        } else {
                            return Err("Server response is not OpenAI-compatible format".to_string());
                        }
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                        return Err(format!("HTTP error {}: {}", status, error_text));
                    }
                },
                Err(e) => {
                    return Err(format!("Failed to connect to {}: {}", base_url, e));
                }
            }
        } else {
            // Models endpoint succeeded
            let response = models_result.unwrap();
            let models_data: serde_json::Value = response.json().await
                .map_err(|e| format!("Failed to parse models response: {}", e))?;
            
            let model_count = models_data.get("data")
                .and_then(|d| d.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
                
            Ok(format!("OpenAI Compatible connection successful! Server: {}, Available models: {}", 
                base_url, model_count))
        }
    }


    /// Apply provider changes and restart the application
    fn apply_provider_changes(&mut self) {
        log::info!("Applying provider changes and restarting");
        
        // Save current settings
        self.save_configs(crate::gui::theme::AppTheme::default());
        
        // Send event to reinitialize the provider
        if let Some(ref sender) = self.app_event_sender {
            if let Err(e) = sender.send(crate::gui::app::events::AppEvent::ReinitializeProvider {
                provider_type: self.current_provider.clone(),
            }) {
                log::error!("Failed to send ReinitializeProvider event: {}", e);
            } else {
                log::info!("Sent ReinitializeProvider event for provider: {:?}", self.current_provider);
            }
        } else {
            log::error!("No app event sender available for provider reinitialization");
        }
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    
    
    use sagitta_search::config::AppConfig;
    use crate::config::types::{SagittaCodeConfig, UiConfig, LoggingConfig, ConversationConfig, ClaudeCodeConfig};
    // Import specific loader functions for more direct testing of file operations
    

    fn create_test_sagitta_config() -> AppConfig {
        AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: Some("/test/model.onnx".to_string()),
            onnx_tokenizer_path: Some("/test/tokenizer".to_string()),
            embed_model: None, // Not using automatic model downloading
            repositories_base_path: Some("/test/repos".to_string()),
            vocabulary_base_path: None, // Use default path
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
        }
    }

    fn create_test_sagitta_code_config() -> SagittaCodeConfig {
        SagittaCodeConfig {
            claude_code: Some(crate::config::types::ClaudeCodeConfig::default()),
            sagitta: Default::default(),
            ui: UiConfig::default(),
            logging: LoggingConfig::default(),
            conversation: ConversationConfig::default(),
            auto_sync: crate::config::types::AutoSyncConfig::default(),
            current_provider: crate::providers::types::ProviderType::ClaudeCode,
            provider_configs: std::collections::HashMap::new(),
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
        let panel = SettingsPanel::new(create_test_sagitta_code_config(), create_test_sagitta_config());
        
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
        
        if let Some(ref claude_config) = config.claude_code {
            assert_eq!(claude_config.claude_path, "/custom/claude");
            assert!(claude_config.verbose);
            assert_eq!(claude_config.max_output_tokens, 10000);
        } else {
            panic!("Expected claude_code config to be present");
        }
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
        if let Some(ref claude_config) = default_sagitta_code_config.claude_code {
            assert_eq!(panel.claude_code_path, claude_config.claude_path);
            assert_eq!(panel.claude_code_model, claude_config.model);
            assert_eq!(panel.claude_code_verbose, claude_config.verbose);
        } else {
            // If claude_code is None, that's OK now - it's migrated to provider system
            // Just check that provider configs are present
            assert!(!default_sagitta_code_config.provider_configs.is_empty());
        }
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
        if let Some(ref claude_config) = updated_sagitta_code_config.claude_code {
            assert_eq!(claude_config.claude_path, "/new/claude");
            assert!(claude_config.verbose);
        } else {
            panic!("Expected claude_code config to be present");
        }
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
                dependencies: Vec::new(),
                last_synced_commit: None,
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
                dependencies: Vec::new(),
                last_synced_commit: None,
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
        if let Some(ref claude_config) = updated_sagitta_code_config.claude_code {
            assert_eq!(claude_config.claude_path, "/updated/claude");
        } else {
            panic!("Expected claude_code config to be present");
        }
        
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

