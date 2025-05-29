// Gemini model definitions will go here

use serde::{Deserialize, Serialize};

/// Default model to use for Gemini
pub const DEFAULT_MODEL: &str = "gemini-2.5-flash-preview-05-20";

/// Available Gemini models
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GeminiModel {
    /// Gemini 1.5 Flash - faster, more economical model
    Flash,
    /// Gemini 1.5 Pro - more capable model
    Pro,
    /// Gemini 1.0 Pro
    Pro1,
    /// Gemini 2.5 Pro Preview - latest and most capable model
    Pro25Preview,
    /// Gemini 2.5 Flash Preview - with thinking mode support
    Flash25Preview,
}

impl GeminiModel {
    /// Get the model ID for API requests
    pub fn model_id(&self) -> &'static str {
        match self {
            GeminiModel::Flash => "gemini-1.5-flash-latest",
            GeminiModel::Pro => "gemini-1.5-pro-latest",
            GeminiModel::Pro1 => "gemini-1.0-pro",
            GeminiModel::Pro25Preview => "gemini-2.5-pro-preview-05-06",
            GeminiModel::Flash25Preview => "gemini-2.5-flash-preview-05-20",
        }
    }
    
    /// Get a model from its ID
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "gemini-1.5-flash-latest" => Some(GeminiModel::Flash),
            "gemini-1.5-flash" => Some(GeminiModel::Flash),
            "gemini-1.5-pro-latest" => Some(GeminiModel::Pro),
            "gemini-1.5-pro" => Some(GeminiModel::Pro),
            "gemini-1.0-pro" => Some(GeminiModel::Pro1),
            "gemini-2.5-pro-preview-05-06" => Some(GeminiModel::Pro25Preview),
            "gemini-2.5-flash-preview-05-20" => Some(GeminiModel::Flash25Preview),
            _ => None,
        }
    }
    
    /// Check if this model supports thinking mode
    pub fn supports_thinking(&self) -> bool {
        match self {
            GeminiModel::Pro25Preview | GeminiModel::Flash25Preview => true,
            _ => false,
        }
    }
    
    /// Get the default parameters for this model
    pub fn default_parameters(&self) -> ModelParameters {
        match self {
            GeminiModel::Flash => ModelParameters {
                temperature: 0.4,
                top_p: 0.95,
                top_k: 40,
                max_output_tokens: 8192,
                response_mime_type: None,
            },
            GeminiModel::Pro => ModelParameters {
                temperature: 0.4,
                top_p: 0.95,
                top_k: 40,
                max_output_tokens: 8192,
                response_mime_type: None,
            },
            GeminiModel::Pro1 => ModelParameters {
                temperature: 0.7,
                top_p: 0.95,
                top_k: 40,
                max_output_tokens: 2048,
                response_mime_type: None,
            },
            GeminiModel::Pro25Preview => ModelParameters {
                temperature: 0.4,
                top_p: 0.95,
                top_k: 40,
                max_output_tokens: 8192,
                response_mime_type: None,
            },
            GeminiModel::Flash25Preview => ModelParameters {
                temperature: 0.4,
                top_p: 0.95,
                top_k: 40,
                max_output_tokens: 8192,
                response_mime_type: None,
            },
        }
    }
    
    /// Get the context window size for this model
    pub fn context_window_size(&self) -> usize {
        match self {
            GeminiModel::Flash => 1_000_000,
            GeminiModel::Pro => 1_000_000,
            GeminiModel::Pro1 => 30720,
            GeminiModel::Pro25Preview => 2_000_000, // Gemini 2.5 has larger context window
            GeminiModel::Flash25Preview => 1_000_000, // Gemini 2.5 Flash
        }
    }
}

/// Parameters for model generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParameters {
    /// Temperature controls randomness in output (0.0-1.0)
    /// Higher values = more random, lower values = more deterministic
    pub temperature: f32,
    
    /// Nucleus sampling: only consider tokens with this cumulative probability (0.0-1.0)
    /// Higher = more diverse
    pub top_p: f32,
    
    /// Only consider this many most likely next tokens
    pub top_k: i32,
    
    /// Maximum number of tokens to generate
    pub max_output_tokens: i32,
    
    /// Optional response MIME type
    pub response_mime_type: Option<String>,
}

impl Default for ModelParameters {
    fn default() -> Self {
        GeminiModel::Pro25Preview.default_parameters()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model_constant() {
        assert_eq!(DEFAULT_MODEL, "gemini-2.5-flash-preview-05-20");
    }

    #[test]
    fn test_gemini_model_ids() {
        assert_eq!(GeminiModel::Flash.model_id(), "gemini-1.5-flash-latest");
        assert_eq!(GeminiModel::Pro.model_id(), "gemini-1.5-pro-latest");
        assert_eq!(GeminiModel::Pro1.model_id(), "gemini-1.0-pro");
        assert_eq!(GeminiModel::Pro25Preview.model_id(), "gemini-2.5-pro-preview-05-06");
        assert_eq!(GeminiModel::Flash25Preview.model_id(), "gemini-2.5-flash-preview-05-20");
    }

    #[test]
    fn test_gemini_model_from_id() {
        // Test exact matches
        assert_eq!(GeminiModel::from_id("gemini-1.5-flash-latest"), Some(GeminiModel::Flash));
        assert_eq!(GeminiModel::from_id("gemini-1.5-flash"), Some(GeminiModel::Flash));
        assert_eq!(GeminiModel::from_id("gemini-1.5-pro-latest"), Some(GeminiModel::Pro));
        assert_eq!(GeminiModel::from_id("gemini-1.5-pro"), Some(GeminiModel::Pro));
        assert_eq!(GeminiModel::from_id("gemini-1.0-pro"), Some(GeminiModel::Pro1));
        assert_eq!(GeminiModel::from_id("gemini-2.5-pro-preview-05-06"), Some(GeminiModel::Pro25Preview));
        assert_eq!(GeminiModel::from_id("gemini-2.5-flash-preview-05-20"), Some(GeminiModel::Flash25Preview));
        
        // Test unknown model
        assert_eq!(GeminiModel::from_id("unknown-model"), None);
        assert_eq!(GeminiModel::from_id(""), None);
        assert_eq!(GeminiModel::from_id("gemini-2.0-ultra"), None);
    }

    #[test]
    fn test_gemini_model_roundtrip() {
        // Test that model_id() and from_id() are consistent
        let models = [GeminiModel::Flash, GeminiModel::Pro, GeminiModel::Pro1, GeminiModel::Pro25Preview, GeminiModel::Flash25Preview];
        
        for model in models {
            let id = model.model_id();
            let parsed = GeminiModel::from_id(id);
            assert_eq!(parsed, Some(model), "Roundtrip failed for model: {:?}", model);
        }
    }

    #[test]
    fn test_gemini_model_default_parameters() {
        let flash_params = GeminiModel::Flash.default_parameters();
        assert_eq!(flash_params.temperature, 0.4);
        assert_eq!(flash_params.top_p, 0.95);
        assert_eq!(flash_params.top_k, 40);
        assert_eq!(flash_params.max_output_tokens, 8192);
        assert!(flash_params.response_mime_type.is_none());

        let pro_params = GeminiModel::Pro.default_parameters();
        assert_eq!(pro_params.temperature, 0.4);
        assert_eq!(pro_params.top_p, 0.95);
        assert_eq!(pro_params.top_k, 40);
        assert_eq!(pro_params.max_output_tokens, 8192);
        assert!(pro_params.response_mime_type.is_none());

        let pro1_params = GeminiModel::Pro1.default_parameters();
        assert_eq!(pro1_params.temperature, 0.7);
        assert_eq!(pro1_params.top_p, 0.95);
        assert_eq!(pro1_params.top_k, 40);
        assert_eq!(pro1_params.max_output_tokens, 2048);
        assert!(pro1_params.response_mime_type.is_none());

        let pro25_params = GeminiModel::Pro25Preview.default_parameters();
        assert_eq!(pro25_params.temperature, 0.4);
        assert_eq!(pro25_params.top_p, 0.95);
        assert_eq!(pro25_params.top_k, 40);
        assert_eq!(pro25_params.max_output_tokens, 8192);
        assert!(pro25_params.response_mime_type.is_none());

        let flash25_params = GeminiModel::Flash25Preview.default_parameters();
        assert_eq!(flash25_params.temperature, 0.4);
        assert_eq!(flash25_params.top_p, 0.95);
        assert_eq!(flash25_params.top_k, 40);
        assert_eq!(flash25_params.max_output_tokens, 8192);
        assert!(flash25_params.response_mime_type.is_none());
    }

    #[test]
    fn test_gemini_model_context_window_sizes() {
        assert_eq!(GeminiModel::Flash.context_window_size(), 1_000_000);
        assert_eq!(GeminiModel::Pro.context_window_size(), 1_000_000);
        assert_eq!(GeminiModel::Pro1.context_window_size(), 30720);
        assert_eq!(GeminiModel::Pro25Preview.context_window_size(), 2_000_000);
        assert_eq!(GeminiModel::Flash25Preview.context_window_size(), 1_000_000);
    }

    #[test]
    fn test_model_parameters_serialization() {
        let params = ModelParameters {
            temperature: 0.8,
            top_p: 0.9,
            top_k: 50,
            max_output_tokens: 4096,
            response_mime_type: Some("application/json".to_string()),
        };

        let serialized = serde_json::to_string(&params).unwrap();
        let deserialized: ModelParameters = serde_json::from_str(&serialized).unwrap();

        assert_eq!(params.temperature, deserialized.temperature);
        assert_eq!(params.top_p, deserialized.top_p);
        assert_eq!(params.top_k, deserialized.top_k);
        assert_eq!(params.max_output_tokens, deserialized.max_output_tokens);
        assert_eq!(params.response_mime_type, deserialized.response_mime_type);
    }

    #[test]
    fn test_model_parameters_default() {
        let default_params = ModelParameters::default();
        let pro25_params = GeminiModel::Pro25Preview.default_parameters();

        assert_eq!(default_params.temperature, pro25_params.temperature);
        assert_eq!(default_params.top_p, pro25_params.top_p);
        assert_eq!(default_params.top_k, pro25_params.top_k);
        assert_eq!(default_params.max_output_tokens, pro25_params.max_output_tokens);
        assert_eq!(default_params.response_mime_type, pro25_params.response_mime_type);
    }

    #[test]
    fn test_gemini_model_enum_properties() {
        // Test Debug trait
        let flash = GeminiModel::Flash;
        assert!(format!("{:?}", flash).contains("Flash"));

        // Test Clone trait
        let pro = GeminiModel::Pro;
        let pro_clone = pro.clone();
        assert_eq!(pro, pro_clone);

        // Test PartialEq trait
        assert_eq!(GeminiModel::Flash, GeminiModel::Flash);
        assert_ne!(GeminiModel::Flash, GeminiModel::Pro);

        // Test Copy trait (implicit through usage)
        let pro1 = GeminiModel::Pro1;
        let pro1_copy = pro1; // This works because Copy is implemented
        assert_eq!(pro1, pro1_copy);
    }

    #[test]
    fn test_model_parameters_with_none_mime_type() {
        let params = ModelParameters {
            temperature: 0.5,
            top_p: 0.8,
            top_k: 30,
            max_output_tokens: 1024,
            response_mime_type: None,
        };

        let serialized = serde_json::to_string(&params).unwrap();
        let deserialized: ModelParameters = serde_json::from_str(&serialized).unwrap();

        assert!(deserialized.response_mime_type.is_none());
    }

    #[test]
    fn test_model_parameters_edge_values() {
        // Test with edge values
        let params = ModelParameters {
            temperature: 0.0,
            top_p: 1.0,
            top_k: 1,
            max_output_tokens: 1,
            response_mime_type: Some("text/plain".to_string()),
        };

        assert_eq!(params.temperature, 0.0);
        assert_eq!(params.top_p, 1.0);
        assert_eq!(params.top_k, 1);
        assert_eq!(params.max_output_tokens, 1);
        assert_eq!(params.response_mime_type, Some("text/plain".to_string()));
    }

    #[test]
    fn test_all_models_have_valid_parameters() {
        let models = [GeminiModel::Flash, GeminiModel::Pro, GeminiModel::Pro1, GeminiModel::Pro25Preview, GeminiModel::Flash25Preview];
        
        for model in models {
            let params = model.default_parameters();
            
            // Validate parameter ranges
            assert!(params.temperature >= 0.0 && params.temperature <= 1.0, 
                "Temperature out of range for {:?}: {}", model, params.temperature);
            assert!(params.top_p >= 0.0 && params.top_p <= 1.0, 
                "Top_p out of range for {:?}: {}", model, params.top_p);
            assert!(params.top_k > 0, 
                "Top_k should be positive for {:?}: {}", model, params.top_k);
            assert!(params.max_output_tokens > 0, 
                "Max output tokens should be positive for {:?}: {}", model, params.max_output_tokens);
        }
    }

    #[test]
    fn test_model_context_windows_are_reasonable() {
        let models = [GeminiModel::Flash, GeminiModel::Pro, GeminiModel::Pro1, GeminiModel::Pro25Preview, GeminiModel::Flash25Preview];
        
        for model in models {
            let context_size = model.context_window_size();
            assert!(context_size > 1000, "Context window too small for {:?}: {}", model, context_size);
            assert!(context_size <= 2_000_000, "Context window suspiciously large for {:?}: {}", model, context_size);
        }
    }

    #[test]
    fn test_model_id_consistency() {
        // Ensure model IDs follow expected patterns
        assert!(GeminiModel::Flash.model_id().contains("flash"));
        assert!(GeminiModel::Pro.model_id().contains("pro"));
        assert!(GeminiModel::Pro1.model_id().contains("pro"));
        assert!(GeminiModel::Pro25Preview.model_id().contains("pro"));
        assert!(GeminiModel::Flash25Preview.model_id().contains("flash"));
        
        // Ensure all model IDs start with "gemini"
        let models = [GeminiModel::Flash, GeminiModel::Pro, GeminiModel::Pro1, GeminiModel::Pro25Preview, GeminiModel::Flash25Preview];
        for model in models {
            assert!(model.model_id().starts_with("gemini"), 
                "Model ID should start with 'gemini': {}", model.model_id());
        }
    }

    #[test]
    fn test_thinking_mode_support() {
        // Test that only 2.5 models support thinking
        assert!(!GeminiModel::Flash.supports_thinking());
        assert!(!GeminiModel::Pro.supports_thinking());
        assert!(!GeminiModel::Pro1.supports_thinking());
        assert!(GeminiModel::Pro25Preview.supports_thinking());
        assert!(GeminiModel::Flash25Preview.supports_thinking());
    }
}

