// TODO: Implement model discovery and caching in Phase 2
// This is a placeholder to make the code compile

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::api::ModelInfo;
use super::error::OpenRouterError;

/// Model cache entry with timestamp
#[derive(Debug, Clone)]
struct CachedModel {
    model: ModelInfo,
    cached_at: Instant,
}

/// Model categories for filtering
#[derive(Debug, Clone, PartialEq)]
pub enum ModelCategory {
    Chat,          // General conversation models
    Code,          // Code-specialized models  
    Vision,        // Multi-modal models that support images
    Function,      // Function calling models
    Creative,      // Creative writing models
    Reasoning,     // Reasoning/analysis models
}

/// Model filtering criteria
#[derive(Debug, Clone, Default)]
pub struct ModelFilter {
    pub provider: Option<String>,
    pub category: Option<ModelCategory>, 
    pub max_price_per_token: Option<f64>,
    pub min_context_length: Option<u64>,
    pub search_query: Option<String>,
}

/// Manages OpenRouter model discovery, caching, and filtering
pub struct ModelManager {
    cache: Arc<Mutex<HashMap<String, CachedModel>>>,
    cache_duration: Duration,
    http_client: reqwest::Client,
    base_url: String,
}

impl ModelManager {
    /// Create a new model manager
    pub fn new(http_client: reqwest::Client, base_url: String) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_duration: Duration::from_secs(300), // 5 minutes
            http_client,
            base_url,
        }
    }

    /// Get all available models with optional filtering
    pub async fn get_available_models(&self, filter: Option<ModelFilter>) -> Result<Vec<ModelInfo>, OpenRouterError> {
        let models = self.fetch_models().await?;
        
        if let Some(filter) = filter {
            Ok(self.filter_models(models, &filter))
        } else {
            Ok(models)
        }
    }

    /// Get model by ID
    pub async fn get_model_by_id(&self, model_id: &str) -> Result<Option<ModelInfo>, OpenRouterError> {
        let models = self.fetch_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    /// Search models by name or description
    pub async fn search_models(&self, query: &str) -> Result<Vec<ModelInfo>, OpenRouterError> {
        let filter = ModelFilter {
            search_query: Some(query.to_lowercase()),
            ..Default::default()
        };
        self.get_available_models(Some(filter)).await
    }

    /// Get popular models (commonly used ones)
    pub async fn get_popular_models(&self) -> Result<Vec<ModelInfo>, OpenRouterError> {
        // Instead of hardcoded popular models, get recent models from the last year
        self.get_recent_models().await
    }

    /// Get recent models from the last year (more useful than hardcoded popular list)
    pub async fn get_recent_models(&self) -> Result<Vec<ModelInfo>, OpenRouterError> {
        let all_models = self.fetch_models().await?;
        
        // Calculate cutoff date (1 year ago)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let one_year_ago = now.saturating_sub(365 * 24 * 60 * 60); // 1 year in seconds
        
        // Filter models to those created in the last year
        let mut recent_models: Vec<ModelInfo> = all_models.into_iter()
            .filter(|model| model.created >= one_year_ago)
            .collect();
        
        // Sort by creation date (newest first) and take the first 100
        recent_models.sort_by(|a, b| b.created.cmp(&a.created));
        recent_models.truncate(100);
        
        // If we don't have enough recent models, include some high-quality models
        if recent_models.len() < 20 {
            log::warn!("Only found {} recent models, including additional recommended models", recent_models.len());
            recent_models.extend(self.get_recommended_models().await?);
            
            // Remove duplicates and re-sort
            recent_models.sort_by(|a, b| a.id.cmp(&b.id));
            recent_models.dedup_by(|a, b| a.id == b.id);
            recent_models.sort_by(|a, b| b.created.cmp(&a.created));
            recent_models.truncate(100);
        }
        
        Ok(recent_models)
    }

    /// Get recommended models (fallback when not enough recent models)
    async fn get_recommended_models(&self) -> Result<Vec<ModelInfo>, OpenRouterError> {
        let all_models = self.fetch_models().await?;
        
        // Define recommended model patterns (current 2024/2025 models)
        let recommended_patterns = [
            "deepseek-r1-0528",
            "magistral-medium-2506",
            "claude-sonnet-4",
            "gemini-2.5-pro-preview",
            "gemini-2.5-flash-preview",
            "llama-3.3-70b-instruct"
        ];
        
        let recommended_models: Vec<ModelInfo> = all_models.into_iter()
            .filter(|model| {
                let model_id_lower = model.id.to_lowercase();
                recommended_patterns.iter().any(|pattern| model_id_lower.contains(pattern))
            })
            .collect();
            
        Ok(recommended_models)
    }

    /// Refresh model cache
    pub async fn refresh_cache(&self) -> Result<(), OpenRouterError> {
        let _ = self.fetch_models_from_api().await?;
        Ok(())
    }

    /// Clear model cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    /// Get models from cache or fetch if needed
    async fn fetch_models(&self) -> Result<Vec<ModelInfo>, OpenRouterError> {
        // Check cache first
        if let Ok(cache) = self.cache.lock() {
            if !cache.is_empty() {
                let now = Instant::now();
                let cached_models: Vec<ModelInfo> = cache.values()
                    .filter(|cached| now.duration_since(cached.cached_at) < self.cache_duration)
                    .map(|cached| cached.model.clone())
                    .collect();

                if !cached_models.is_empty() {
                    return Ok(cached_models);
                }
            }
        }

        // Cache miss or expired, fetch from API
        self.fetch_models_from_api().await
    }

    /// Fetch models from OpenRouter API and update cache
    async fn fetch_models_from_api(&self) -> Result<Vec<ModelInfo>, OpenRouterError> {
        let url = format!("{}/models", self.base_url);
        log::debug!("Fetching models from OpenRouter API: {}", url);
        
        let response = self.http_client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            log::error!("OpenRouter API error: HTTP {} - {}", status, error_text);
            return Err(OpenRouterError::HttpError(
                format!("HTTP {}: {}", status, error_text)
            ));
        }

        let models_response: super::api::ModelsResponse = response.json().await?;
        let models = models_response.data;
        
        log::info!("Fetched {} models from OpenRouter API", models.len());
        if log::log_enabled!(log::Level::Debug) {
            // Log first few model names for debugging
            for (i, model) in models.iter().take(10).enumerate() {
                log::debug!("Model {}: {} (created: {})", i, model.id, model.created);
            }
            if models.len() > 10 {
                log::debug!("... and {} more models", models.len() - 10);
            }
        }

        // Update cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
            let now = Instant::now();
            for model in &models {
                cache.insert(model.id.clone(), CachedModel {
                    model: model.clone(),
                    cached_at: now,
                });
            }
            log::debug!("Updated model cache with {} entries", cache.len());
        }

        Ok(models)
    }

    /// Filter models based on criteria
    fn filter_models(&self, models: Vec<ModelInfo>, filter: &ModelFilter) -> Vec<ModelInfo> {
        models.into_iter()
            .filter(|model| {
                // Provider filter
                if let Some(provider) = &filter.provider {
                    if !model.id.starts_with(&format!("{}/", provider)) {
                        return false;
                    }
                }

                // Category filter
                if let Some(category) = &filter.category {
                    if !self.model_matches_category(model, category) {
                        return false;
                    }
                }

                // Price filter (parse pricing strings)
                if let Some(max_price) = filter.max_price_per_token {
                    if let Ok(prompt_price) = model.pricing.prompt.parse::<f64>() {
                        if prompt_price > max_price {
                            return false;
                        }
                    }
                }

                // Context length filter
                if let Some(min_context) = filter.min_context_length {
                    if model.context_length < min_context {
                        return false;
                    }
                }

                // Search query filter
                if let Some(query) = &filter.search_query {
                    let model_text = format!("{} {} {}", 
                        model.id.to_lowercase(), 
                        model.name.to_lowercase(), 
                        model.description.to_lowercase()
                    );
                    if !model_text.contains(query) {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Check if model matches a category
    fn model_matches_category(&self, model: &ModelInfo, category: &ModelCategory) -> bool {
        let model_id = model.id.to_lowercase();
        let model_name = model.name.to_lowercase();
        let description = model.description.to_lowercase();

        match category {
            ModelCategory::Chat => {
                // Most models are chat models, exclude specific specialized ones
                !model_id.contains("code") && !model_id.contains("vision") 
            }
            ModelCategory::Code => {
                model_id.contains("code") || model_name.contains("code") || 
                description.contains("code") || description.contains("programming")
            }
            ModelCategory::Vision => {
                model.architecture.input_modalities.contains(&"image".to_string()) ||
                model_id.contains("vision") || description.contains("vision") ||
                description.contains("image") || description.contains("visual")
            }
            ModelCategory::Function => {
                description.contains("function") || description.contains("tool") ||
                model_name.contains("function")
            }
            ModelCategory::Creative => {
                description.contains("creative") || description.contains("writing") ||
                model_name.contains("creative")
            }
            ModelCategory::Reasoning => {
                description.contains("reasoning") || description.contains("analysis") ||
                model_id.contains("reasoning") || model_name.contains("reasoning")
            }
        }
    }

    /// Get providers available
    pub async fn get_providers(&self) -> Result<Vec<String>, OpenRouterError> {
        let models = self.fetch_models().await?;
        let mut providers: Vec<String> = models.iter()
            .filter_map(|model| {
                model.id.split('/').next().map(|s| s.to_string())
            })
            .collect();
        
        providers.sort();
        providers.dedup();
        Ok(providers)
    }

    /// Get model statistics
    pub async fn get_model_stats(&self) -> Result<ModelStats, OpenRouterError> {
        let models = self.fetch_models().await?;
        
        let total_models = models.len();
        let providers = self.get_providers().await?.len();
        
        let vision_models = models.iter()
            .filter(|m| self.model_matches_category(m, &ModelCategory::Vision))
            .count();
            
        let code_models = models.iter()
            .filter(|m| self.model_matches_category(m, &ModelCategory::Code))
            .count();

        Ok(ModelStats {
            total_models,
            total_providers: providers,
            vision_models,
            code_models,
        })
    }
}

/// Model statistics
#[derive(Debug, Clone)]
pub struct ModelStats {
    pub total_models: usize,
    pub total_providers: usize,
    pub vision_models: usize,
    pub code_models: usize,
} 