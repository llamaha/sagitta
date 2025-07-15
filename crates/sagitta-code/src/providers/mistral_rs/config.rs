use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MistralRsConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stream: bool,
    pub top_k: Option<u32>,
    pub min_p: Option<f32>,
}

impl Default for MistralRsConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:1234".to_string(),
            api_key: None,
            model: "default".to_string(),
            max_tokens: None,
            temperature: None,
            stream: true,
            top_k: None,
            min_p: None,
        }
    }
}

impl MistralRsConfig {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            ..Default::default()
        }
    }

    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    pub fn with_temperature(mut self, temperature: Option<f32>) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: Option<u32>) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.base_url.is_empty() {
            return Err("Base URL cannot be empty".to_string());
        }

        if !self.base_url.starts_with("http://") && !self.base_url.starts_with("https://") {
            return Err("Base URL must start with http:// or https://".to_string());
        }

        if let Some(temp) = self.temperature {
            if temp < 0.0 || temp > 2.0 {
                return Err("Temperature must be between 0.0 and 2.0".to_string());
            }
        }

        if let Some(max_tokens) = self.max_tokens {
            if max_tokens == 0 {
                return Err("Max tokens must be greater than 0".to_string());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MistralRsConfig::default();
        assert_eq!(config.base_url, "http://localhost:1234");
        assert_eq!(config.model, "default");
        assert!(config.stream);
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_config_validation() {
        let config = MistralRsConfig::default();
        assert!(config.validate().is_ok());

        let invalid_config = MistralRsConfig {
            base_url: "invalid_url".to_string(),
            ..Default::default()
        };
        assert!(invalid_config.validate().is_err());

        let invalid_temp_config = MistralRsConfig {
            temperature: Some(3.0),
            ..Default::default()
        };
        assert!(invalid_temp_config.validate().is_err());
    }

    #[test]
    fn test_builder_methods() {
        let config = MistralRsConfig::new("http://localhost:8080".to_string())
            .with_api_key(Some("test_key".to_string()))
            .with_model("gpt-4".to_string())
            .with_temperature(Some(0.7))
            .with_max_tokens(Some(1000));

        assert_eq!(config.base_url, "http://localhost:8080");
        assert_eq!(config.api_key, Some("test_key".to_string()));
        assert_eq!(config.model, "gpt-4");
        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.max_tokens, Some(1000));
    }
}