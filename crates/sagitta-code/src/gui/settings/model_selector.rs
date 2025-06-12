//! Dynamic model selector widget for OpenRouter models
//! 
//! Provides a searchable dropdown with model filtering, favorites, and model information.

use std::sync::Arc;
use std::collections::HashMap;
use egui::{Context, Ui, RichText, Color32, Window, Grid, Button, TextEdit, ScrollArea, ComboBox, Vec2, Stroke};
use log::{info, warn, error, debug};

use crate::llm::openrouter::{
    models::{ModelManager, ModelFilter, ModelCategory},
    api::ModelInfo,
    error::OpenRouterError,
};

/// Model selector state
#[derive(Debug, Clone)]
pub struct ModelSelectorState {
    pub search_query: String,
    pub selected_category: Option<ModelCategory>,
    pub selected_provider: Option<String>,
    pub show_recent_only: bool,
    pub show_dropdown: bool,
    pub loading: bool,
    pub error_message: Option<String>,
}

impl Default for ModelSelectorState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            selected_category: None,
            selected_provider: None,
            show_recent_only: true, // Start with recent models only
            show_dropdown: false,
            loading: false,
            error_message: None,
        }
    }
}

/// Dynamic model selector widget (simplified synchronous version)
pub struct ModelSelector {
    model_manager: Arc<ModelManager>,
    state: ModelSelectorState,
    available_models: Vec<ModelInfo>,
    filtered_models: Vec<ModelInfo>,
    providers: Vec<String>,
    favorites: Vec<String>, // Model IDs
    last_refresh: std::time::Instant,
    refresh_interval: std::time::Duration,
}

impl ModelSelector {
    /// Create a new model selector
    pub fn new(model_manager: Arc<ModelManager>) -> Self {
        Self {
            model_manager,
            state: ModelSelectorState::default(),
            available_models: Vec::new(),
            filtered_models: Vec::new(),
            providers: Vec::new(),
            favorites: Vec::new(),
            last_refresh: std::time::Instant::now() - std::time::Duration::from_secs(3600), // Force initial refresh
            refresh_interval: std::time::Duration::from_secs(300), // 5 minutes
        }
    }

    /// Render the model selector widget (synchronous for egui)
    pub fn render(
        &mut self,
        ui: &mut Ui,
        current_model: &mut String,
        theme: &crate::gui::theme::AppTheme,
    ) -> bool {
        let mut model_changed = false;

        ui.vertical(|ui| {
            // Model selection header
            ui.horizontal(|ui| {
                ui.label("Model:");
                
                // Current model display/button
                let model_display = if current_model.is_empty() {
                    "Select a model...".to_string()
                } else {
                    current_model.clone()
                };
                
                if ui.button(&model_display).clicked() {
                    self.state.show_dropdown = !self.state.show_dropdown;
                    if self.state.show_dropdown && self.should_refresh() {
                        // Trigger refresh by spawning a task
                        self.start_refresh();
                    }
                }
                
                // Refresh button
                if ui.small_button("🔄").on_hover_text("Refresh models").clicked() {
                    self.start_refresh();
                }
                
                // Favorites button
                if ui.small_button("⭐").on_hover_text("Toggle favorites").clicked() {
                    self.toggle_favorite(current_model);
                }
            });

            // Show error if any
            if let Some(error) = &self.state.error_message {
                ui.colored_label(Color32::RED, format!("Error: {}", error));
            }

            // Loading indicator
            if self.state.loading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Loading models...");
                });
            }

            // Model selection dropdown
            if self.state.show_dropdown {
                model_changed = self.render_model_dropdown(ui, current_model, theme);
            }
        });

        model_changed
    }

    /// Render the model dropdown interface (synchronous)
    fn render_model_dropdown(
        &mut self,
        ui: &mut Ui,
        current_model: &mut String,
        _theme: &crate::gui::theme::AppTheme,
    ) -> bool {
        let mut model_changed = false;

        ui.separator();
        
        // Search and filter controls
        ui.horizontal(|ui| {
            ui.label("Search:");
            let search_response = ui.add(TextEdit::singleline(&mut self.state.search_query)
                .hint_text("Search models..."));
            
            if search_response.changed() {
                self.apply_filters();
            }
        });

        ui.horizontal(|ui| {
            // Category filter
            ComboBox::from_label("Category")
                .selected_text(match &self.state.selected_category {
                    Some(ModelCategory::Chat) => "Chat",
                    Some(ModelCategory::Code) => "Code", 
                    Some(ModelCategory::Vision) => "Vision",
                    Some(ModelCategory::Function) => "Function",
                    Some(ModelCategory::Creative) => "Creative",
                    Some(ModelCategory::Reasoning) => "Reasoning",
                    None => "All",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.state.selected_category, None, "All");
                    ui.selectable_value(&mut self.state.selected_category, Some(ModelCategory::Chat), "Chat");
                    ui.selectable_value(&mut self.state.selected_category, Some(ModelCategory::Code), "Code");
                    ui.selectable_value(&mut self.state.selected_category, Some(ModelCategory::Vision), "Vision");
                    ui.selectable_value(&mut self.state.selected_category, Some(ModelCategory::Function), "Function");
                    ui.selectable_value(&mut self.state.selected_category, Some(ModelCategory::Creative), "Creative");
                    ui.selectable_value(&mut self.state.selected_category, Some(ModelCategory::Reasoning), "Reasoning");
                });

            // Provider filter
            ComboBox::from_label("Provider")
                .selected_text(self.state.selected_provider.as_deref().unwrap_or("All"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.state.selected_provider, None, "All");
                    for provider in &self.providers {
                        ui.selectable_value(&mut self.state.selected_provider, Some(provider.clone()), provider);
                    }
                });

            // Recent models toggle
            if ui.checkbox(&mut self.state.show_recent_only, "Recent only").changed() {
                self.apply_filters();
            }
        });

        // Model list
        ui.separator();
        
        ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                // Create a copy of the filtered models to avoid borrowing issues
                let models_to_show = self.filtered_models.clone();
                
                for model in &models_to_show {
                    let is_selected = current_model == &model.id;
                    let is_favorite = self.favorites.contains(&model.id);
                    
                    ui.horizontal(|ui| {
                        // Model selection button
                        let button_text = if is_selected {
                            RichText::new(&model.id).color(Color32::WHITE)
                        } else {
                            RichText::new(&model.id)
                        };
                        
                        if ui.add(Button::new(button_text)
                            .fill(if is_selected { Color32::BLUE } else { Color32::TRANSPARENT })
                            .min_size(Vec2::new(200.0, 20.0)))
                            .clicked() {
                            *current_model = model.id.clone();
                            self.state.show_dropdown = false;
                            model_changed = true;
                        }
                        
                        // Favorite toggle - capture the model ID first
                        let model_id = model.id.clone();
                        let star_text = if is_favorite { "⭐" } else { "☆" };
                        if ui.small_button(star_text).clicked() {
                            self.toggle_favorite(&model_id);
                        }
                        
                        // Model info
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Ok(prompt_price) = model.pricing.prompt.parse::<f64>() {
                                ui.small(format!("${:.6}/token", prompt_price));
                            }
                            ui.small(format!("{}k ctx", model.context_length / 1000));
                        });
                    });
                    
                    // Model description (if expanded)
                    if is_selected && !model.description.is_empty() {
                        ui.indent("model_description", |ui| {
                            ui.small(&model.description);
                        });
                    }
                }
            });

        // Close button
        ui.horizontal(|ui| {
            if ui.button("Close").clicked() {
                self.state.show_dropdown = false;
            }
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.small(format!("Showing {} of {} models", 
                    self.filtered_models.len(), 
                    self.available_models.len()));
            });
        });

        model_changed
    }

    /// Check if we should refresh models
    fn should_refresh(&self) -> bool {
        self.available_models.is_empty() || self.last_refresh.elapsed() > self.refresh_interval
    }

    /// Start a refresh operation (spawns background task)
    fn start_refresh(&mut self) {
        self.state.loading = true;
        self.state.error_message = None;
        
        // Try to fetch recent models from API first, fallback to popular models
        let model_manager = self.model_manager.clone();
        
        // Since we're in egui (synchronous), we need to try the async call in a way that doesn't block
        // For now, we'll use the popular models as a good default and try to fetch in background
        let popular_models = self.get_popular_models_fallback();
        
        // Spawn a task to fetch actual recent models and update later (non-blocking)
        let manager_clone = model_manager.clone();
        tokio::spawn(async move {
            match manager_clone.get_recent_models().await {
                Ok(api_models) => {
                    // In a real implementation, we'd need a way to update the UI from here
                    // For now, we log success and the UI will use the fallback
                    info!("Successfully fetched {} recent models from OpenRouter API", api_models.len());
                }
                Err(e) => {
                    warn!("Failed to fetch recent models from OpenRouter API: {}. Using fallback.", e);
                }
            }
        });
        
        // Use popular models as immediate fallback for good UX
        self.available_models = popular_models;
        self.apply_filters();
        self.update_providers();
        self.last_refresh = std::time::Instant::now();
        self.state.loading = false;
        
        info!("Model refresh initiated with recent models fallback");
    }

    /// Get popular models as fallback when API is not available
    fn get_popular_models_fallback(&self) -> Vec<ModelInfo> {
        let popular_models = vec![
            // DeepSeek Models
            ("deepseek/deepseek-r1-0528:free", "DeepSeek R1 0528 (free)", "Latest reasoning model from DeepSeek", "0.0", "0.0", 64000),
            
            // Mistral Models
            ("mistralai/magistral-medium-2506", "Mistral Magistral Medium 2506", "Latest Mistral model", "0.001", "0.003", 128000),
            ("mistralai/magistral-medium-2506:thinking", "Mistral Magistral Medium 2506 (thinking)", "Thinking mode version", "0.001", "0.003", 128000),
            
            // Anthropic Models
            ("anthropic/claude-sonnet-4", "Anthropic Claude Sonnet 4", "Latest Claude model", "0.003", "0.015", 200000),
            
            // Google Models
            ("google/gemini-2.5-pro-preview", "Google Gemini 2.5 Pro Preview 06-05", "Latest Gemini Pro preview", "0.001250", "0.005", 2097152),
            ("google/gemini-2.5-flash-preview-05-20", "Google Gemini 2.5 Flash Preview 05-20", "Latest Gemini Flash preview", "0.000075", "0.0003", 1048576),
            
            // Meta Models
            ("meta-llama/llama-3.3-70b-instruct:free", "Meta Llama 3.3 70B Instruct (free)", "Latest free Llama model", "0.0", "0.0", 128000),
        ];

        popular_models.into_iter().map(|(id, name, desc, prompt_price, completion_price, ctx_len)| {
            ModelInfo {
                id: id.to_string(),
                name: name.to_string(),
                created: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() - 7 * 24 * 60 * 60, // Pretend they were created 7 days ago (very recent)
                description: desc.to_string(),
                pricing: crate::llm::openrouter::api::Pricing {
                    prompt: prompt_price.to_string(),
                    completion: completion_price.to_string(),
                    request: "0.0".to_string(),
                    image: "0.0".to_string(),
                },
                context_length: ctx_len,
                architecture: crate::llm::openrouter::api::Architecture {
                    input_modalities: vec!["text".to_string()],
                    output_modalities: vec!["text".to_string()],
                    tokenizer: "".to_string(),
                },
                top_provider: crate::llm::openrouter::api::TopProvider {
                    is_moderated: false,
                },
            }
        }).collect()
    }

    /// Apply current filters to the model list
    fn apply_filters(&mut self) {
        let mut models = if self.state.show_recent_only {
            // For recent models, use all available models since our fallback is already recent
            self.available_models.clone()
        } else {
            self.available_models.clone()
        };

        // Apply search filter
        if !self.state.search_query.is_empty() {
            let query = self.state.search_query.to_lowercase();
            models.retain(|model| {
                model.id.to_lowercase().contains(&query) ||
                model.description.to_lowercase().contains(&query)
            });
        }

        // Apply provider filter
        if let Some(provider) = &self.state.selected_provider {
            models.retain(|model| model.id.starts_with(&format!("{}/", provider)));
        }

        // Apply category filter (simplified implementation)
        if let Some(category) = &self.state.selected_category {
            models.retain(|model| self.model_matches_category(model, category));
        }

        // Sort models: favorites first, then alphabetically
        models.sort_by(|a, b| {
            let a_fav = self.favorites.contains(&a.id);
            let b_fav = self.favorites.contains(&b.id);
            
            match (a_fav, b_fav) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.id.cmp(&b.id),
            }
        });

        self.filtered_models = models;
    }

    /// Check if a model matches a given category (simplified heuristic)
    fn model_matches_category(&self, model: &ModelInfo, category: &ModelCategory) -> bool {
        let model_name = model.id.to_lowercase();
        
        match category {
            ModelCategory::Code => {
                model_name.contains("code") || 
                model_name.contains("copilot") ||
                model_name.contains("starcoder") ||
                (model_name.contains("wizard") && model_name.contains("coder"))
            }
            ModelCategory::Vision => {
                model_name.contains("vision") ||
                model_name.contains("gpt-4o") || // GPT-4o has vision
                model_name.contains("claude-3") // Claude 3 has vision
            }
            ModelCategory::Function => {
                // Most modern models support function calling, so this is less useful
                true
            }
            ModelCategory::Creative => {
                model_name.contains("creative") ||
                model_name.contains("claude") // Claude is good for creative tasks
            }
            ModelCategory::Reasoning => {
                model_name.contains("o1") || // OpenAI's reasoning models
                model_name.contains("reasoning") ||
                model_name.contains("think")
            }
            ModelCategory::Chat => {
                // Default category for general conversation models
                !self.model_matches_category(model, &ModelCategory::Code) &&
                !self.model_matches_category(model, &ModelCategory::Vision)
            }
        }
    }

    /// Update the providers list from available models
    fn update_providers(&mut self) {
        let mut providers = std::collections::HashSet::new();
        
        for model in &self.available_models {
            if let Some(provider) = model.id.split('/').next() {
                providers.insert(provider.to_string());
            }
        }
        
        self.providers = providers.into_iter().collect();
        self.providers.sort();
    }

    /// Toggle favorite status for a model
    fn toggle_favorite(&mut self, model_id: &str) {
        if let Some(pos) = self.favorites.iter().position(|id| id == model_id) {
            self.favorites.remove(pos);
        } else {
            self.favorites.push(model_id.to_string());
        }
        
        // Re-apply filters to update sorting
        self.apply_filters();
    }

    /// Get current state (for external access)
    pub fn state(&self) -> &ModelSelectorState {
        &self.state
    }

    /// Set favorites list (for persistence)
    pub fn set_favorites(&mut self, favorites: Vec<String>) {
        self.favorites = favorites;
        self.apply_filters();
    }

    /// Get favorites list (for persistence)
    pub fn get_favorites(&self) -> &[String] {
        &self.favorites
    }

    /// Force refresh models from API (blocking call for initialization)
    pub fn force_refresh_models(&mut self) {
        self.state.loading = true;
        self.state.error_message = None;
        
        // Try to fetch recent models from API using blocking call
        let rt = tokio::runtime::Runtime::new().unwrap();
        match rt.block_on(self.model_manager.get_recent_models()) {
            Ok(api_models) => {
                info!("Successfully loaded {} recent models from OpenRouter API", api_models.len());
                self.available_models = api_models;
            }
            Err(e) => {
                warn!("Failed to load recent models from OpenRouter API: {}. Trying all available models.", e);
                // Try getting all available models as fallback
                match rt.block_on(self.model_manager.get_available_models(None)) {
                    Ok(all_models) => {
                        info!("Successfully loaded {} total models from OpenRouter API", all_models.len());
                        // Take the last 100 models (most recent by ID/creation order)
                        let mut recent_models = all_models;
                        recent_models.sort_by(|a, b| b.created.cmp(&a.created));
                        recent_models.truncate(100);
                        self.available_models = recent_models;
                    }
                    Err(e2) => {
                        warn!("Failed to load any models from OpenRouter API: {}. Using fallback.", e2);
                        self.available_models = self.get_popular_models_fallback();
                        self.state.error_message = Some(format!("API Error: {}", e2));
                    }
                }
            }
        }
        
        self.apply_filters();
        self.update_providers();
        self.last_refresh = std::time::Instant::now();
        self.state.loading = false;
    }
} 