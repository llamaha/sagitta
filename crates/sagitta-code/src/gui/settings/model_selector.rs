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
    pub show_popular_only: bool,
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
            show_popular_only: true, // Start with popular models
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

            // Popular models toggle
            if ui.checkbox(&mut self.state.show_popular_only, "Popular only").changed() {
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
        
        // Use popular models as fallback for now
        let popular_models = self.get_popular_models_fallback();
        self.available_models = popular_models;
        self.apply_filters();
        self.update_providers();
        self.last_refresh = std::time::Instant::now();
        self.state.loading = false;
        
        // TODO: In a full implementation, this would spawn an async task
        // to call model_manager.get_available_models() and update the UI
        info!("Model refresh completed (using fallback popular models)");
    }

    /// Get popular models as fallback when API is not available
    fn get_popular_models_fallback(&self) -> Vec<ModelInfo> {
        let popular_models = vec![
            ("openai/gpt-4o", "OpenAI GPT-4o", "Multi-modal reasoning", "0.0025", "0.01", 128000),
            ("openai/gpt-4o-mini", "OpenAI GPT-4o Mini", "Fast and efficient", "0.0001", "0.0004", 128000),
            ("anthropic/claude-3-5-sonnet", "Claude 3.5 Sonnet", "Advanced reasoning", "0.003", "0.015", 200000),
            ("anthropic/claude-3-haiku", "Claude 3 Haiku", "Fast responses", "0.00025", "0.00125", 200000),
            ("meta-llama/llama-3.1-8b-instruct", "Llama 3.1 8B", "Open source", "0.0001", "0.0001", 128000),
            ("google/gemma-2-9b-it", "Gemma 2 9B", "Google's model", "0.0001", "0.0001", 8192),
        ];

        popular_models.into_iter().map(|(id, name, desc, prompt_price, completion_price, ctx_len)| {
            ModelInfo {
                id: id.to_string(),
                name: name.to_string(),
                created: 0,
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
        let mut models = if self.state.show_popular_only {
            // For popular models, use all available models since our fallback is already popular
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
} 