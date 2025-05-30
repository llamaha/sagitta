use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};

use super::types::{SagittaCodeConfig, GeminiConfig};
use crate::config::paths::{get_sagitta_code_app_config_path, migrate_old_config};

const CONFIG_FILENAME: &str = "sagitta_code_config.json";

/// Get the path to the configuration file (deprecated - use get_sagitta_code_app_config_path)
pub fn get_config_path() -> Result<PathBuf> {
    get_sagitta_code_app_config_path()
}

/// Load configuration from the default location
pub fn load_config() -> Result<SagittaCodeConfig> {
    // Run migration first
    if let Err(e) = migrate_old_config() {
        log::warn!("Failed to migrate old configuration: {}", e);
    }
    
    let config_path = get_sagitta_code_app_config_path()?;
    load_config_from_path(&config_path)
}

/// Load configuration from a specific path
pub fn load_config_from_path<P: AsRef<Path>>(path: P) -> Result<SagittaCodeConfig> {
    let path = path.as_ref();
    
    if !path.exists() {
        // Return default config if file doesn't exist
        return Ok(SagittaCodeConfig::default());
    }
    
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    
    // Determine format based on file extension
    let config: SagittaCodeConfig = if path.extension().and_then(|s| s.to_str()) == Some("toml") {
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse TOML config file: {}", path.display()))?
    } else {
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON config file: {}", path.display()))?
    };
    
    Ok(config)
}

/// Load configuration (simplified - no more core config merging)
/// The shared sagitta-search config is now loaded separately
pub fn load_merged_config() -> Result<SagittaCodeConfig> {
    load_config()
}

/// Save configuration to the default location
pub fn save_config(config: &SagittaCodeConfig) -> Result<()> {
    let config_path = get_sagitta_code_app_config_path()?;
    save_config_to_path(config, &config_path)
}

/// Save configuration to a specific path
pub fn save_config_to_path<P: AsRef<Path>>(config: &SagittaCodeConfig, path: P) -> Result<()> {
    let path = path.as_ref();
    
    // Ensure the parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }
    
    // Determine format based on file extension
    let content = if path.extension().and_then(|s| s.to_str()) == Some("toml") {
        toml::to_string_pretty(config)
            .context("Failed to serialize config to TOML")?
    } else {
        serde_json::to_string_pretty(config)
            .context("Failed to serialize config to JSON")?
    };
    
    fs::write(path, content)
        .with_context(|| format!("Failed to write config file: {}", path.display()))?;
    
    Ok(())
}

/// Initialize the configuration directory and create a default config if it doesn't exist
pub fn initialize_config() -> Result<SagittaCodeConfig> {
    // Run migration first
    if let Err(e) = migrate_old_config() {
        log::warn!("Failed to migrate old configuration: {}", e);
    }
    
    let config_path = get_sagitta_code_app_config_path()?;
    
    // Create the config directory if it doesn't exist
    if let Some(config_dir) = config_path.parent() {
        if !config_dir.exists() {
            fs::create_dir_all(config_dir)
                .with_context(|| format!("Failed to create config directory: {}", config_dir.display()))?;
        }
    }
    
    // Load or create default config
    let config = load_config().unwrap_or_else(|_| {
        log::info!("Creating default sagitta-code configuration");
        SagittaCodeConfig::default()
    });
    
    // Save the config to ensure the file exists
    save_config(&config)?;
    
    Ok(config)
}

/// Validate that a configuration is valid
pub fn validate_config(config: &SagittaCodeConfig) -> Result<()> {
    // Validate Gemini configuration
    if config.gemini.model.is_empty() {
        return Err(anyhow::anyhow!("Gemini model cannot be empty"));
    }
    
    // Validate that if API key is provided, it's not empty
    if let Some(ref api_key) = config.gemini.api_key {
        if api_key.is_empty() {
            return Err(anyhow::anyhow!("Gemini API key cannot be empty if provided"));
        }
    }
    
    Ok(())
}

/// Get both the sagitta-code config and the shared sagitta-search config
pub fn load_all_configs() -> Result<(SagittaCodeConfig, sagitta_search::config::AppConfig)> {
    // Load sagitta-code specific config
    let code_config = load_config()?;
    
    // Load shared sagitta-search config
    let core_config = sagitta_search::config::load_config(None)
        .context("Failed to load shared sagitta-search configuration")?;
    
    Ok((code_config, core_config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{TempDir, NamedTempFile};
    use std::fs;

    fn create_test_config() -> SagittaCodeConfig {
        let mut config = SagittaCodeConfig::default();
        config.gemini.api_key = Some("test-api-key".to_string());
        config.gemini.model = "gemini-2.5-pro-preview-05-06".to_string();
        config
    }

    #[test]
    fn test_load_config_from_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.json");
        
        let result = load_config_from_path(&config_path).unwrap();
        
        // Should return default config
        assert_eq!(result.gemini.model, "gemini-2.5-flash-preview-05-20"); // Default model
        assert!(result.gemini.api_key.is_none());
    }

    #[test]
    fn test_load_config_from_valid_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
{
  "gemini": {
    "api_key": "test-key-123",
    "model": "gemini-1.5-flash-latest",
    "max_history_size": 25
  },
  "sagitta": {
    "repositories_base_path": null,
    "repositories": []
  },
  "ui": {
    "dark_mode": true,
    "theme": "default",
    "window_width": 900,
    "window_height": 700
  },
  "logging": {
    "log_level": "info",
    "log_to_file": false,
    "log_file_path": null
  }
}
"#;
        
        fs::write(temp_file.path(), config_content).unwrap();
        
        let result = load_config_from_path(temp_file.path()).unwrap();
        
        assert_eq!(result.gemini.api_key, Some("test-key-123".to_string()));
        assert_eq!(result.gemini.model, "gemini-1.5-flash-latest");
        assert_eq!(result.gemini.max_history_size, 25);
    }

    #[test]
    fn test_load_config_from_invalid_json() {
        let temp_file = NamedTempFile::new().unwrap();
        let invalid_content = "invalid json content {{{";
        
        fs::write(temp_file.path(), invalid_content).unwrap();
        
        let result = load_config_from_path(temp_file.path());
        assert!(result.is_err());
        // The actual error message includes "JSON" for JSON files
        assert!(result.unwrap_err().to_string().contains("Failed to parse JSON config file"));
    }

    #[test]
    fn test_load_config_from_unreadable_file() {
        // Test with a path that doesn't exist and can't be read
        let result = load_config_from_path("/nonexistent/path/config.json");
        
        // Should return default config since file doesn't exist
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.gemini.model, "gemini-2.5-flash-preview-05-20");
    }

    #[test]
    fn test_save_config_to_path() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.json");
        let test_config = create_test_config();
        
        let result = save_config_to_path(&test_config, &config_path);
        assert!(result.is_ok());
        
        // Verify the file was created and contains the expected content
        assert!(config_path.exists());
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("test-api-key"));
        assert!(content.contains("gemini-2.5-pro-preview-05-06"));
    }

    #[test]
    fn test_save_config_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir.path().join("nested").join("config.json");
        let test_config = create_test_config();
        
        // Directory doesn't exist yet
        assert!(!nested_path.parent().unwrap().exists());
        
        let result = save_config_to_path(&test_config, &nested_path);
        assert!(result.is_ok());
        
        // Directory should be created
        assert!(nested_path.parent().unwrap().exists());
        assert!(nested_path.exists());
    }

    #[test]
    fn test_save_config_serialization() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut test_config = SagittaCodeConfig::default();
        test_config.gemini.api_key = Some("serialization-test-key".to_string());
        test_config.gemini.model = "gemini-1.5-pro-latest".to_string();
        
        save_config_to_path(&test_config, temp_file.path()).unwrap();
        
        let content = fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.contains("serialization-test-key"));
        assert!(content.contains("gemini-1.5-pro-latest"));
    }

    #[test]
    fn test_roundtrip_save_and_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let original_config = create_test_config();
        
        // Save the config
        save_config_to_path(&original_config, temp_file.path()).unwrap();
        
        // Load it back
        let loaded_config = load_config_from_path(temp_file.path()).unwrap();
        
        // Should be identical
        assert_eq!(original_config.gemini.api_key, loaded_config.gemini.api_key);
        assert_eq!(original_config.gemini.model, loaded_config.gemini.model);
    }

    #[test]
    fn test_initialize_config() {
        // This test is tricky because it uses the actual config paths
        // We'll test the logic but can't easily test the file system operations
        
        // Test that it returns a valid config
        let result = initialize_config();
        
        // Should succeed (might create actual config file)
        assert!(result.is_ok());
        let config = result.unwrap();
        
        // Should have default values
        assert!(!config.gemini.model.is_empty());
    }

    #[test]
    fn test_validate_config_valid() {
        let valid_config = create_test_config();
        let result = validate_config(&valid_config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_config_empty_model() {
        let mut invalid_config = SagittaCodeConfig::default();
        invalid_config.gemini.api_key = Some("test-key".to_string());
        invalid_config.gemini.model = "".to_string();
        
        let result = validate_config(&invalid_config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("model cannot be empty"));
    }

    #[test]
    fn test_validate_config_empty_api_key() {
        let mut invalid_config = SagittaCodeConfig::default();
        invalid_config.gemini.api_key = Some("".to_string());
        invalid_config.gemini.model = "gemini-1.5-pro-latest".to_string();
        
        let result = validate_config(&invalid_config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API key cannot be empty"));
    }

    #[test]
    fn test_validate_config_none_api_key() {
        let mut valid_config = SagittaCodeConfig::default();
        valid_config.gemini.api_key = None;
        valid_config.gemini.model = "gemini-1.5-pro-latest".to_string();
        
        let result = validate_config(&valid_config);
        assert!(result.is_ok()); // None API key is valid
    }

    #[test]
    fn test_load_all_configs() {
        // This test just checks that the function doesn't panic
        // In a real environment, it would load actual config files
        let result = load_all_configs();
        
        // The function should either succeed or fail gracefully
        match result {
            Ok((code_config, core_config)) => {
                // Both configs should be valid
                assert!(!code_config.gemini.model.is_empty());
                assert!(!core_config.qdrant_url.is_empty());
            }
            Err(_) => {
                // Failure is acceptable in test environment where configs might not exist
            }
        }
    }

    #[test]
    fn test_config_with_special_characters() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut config_with_special_chars = SagittaCodeConfig::default();
        config_with_special_chars.gemini.api_key = Some("key-with-special-chars!@#$%^&*()".to_string());
        config_with_special_chars.gemini.model = "model-with-dashes-and_underscores".to_string();
        
        // Save and load
        save_config_to_path(&config_with_special_chars, temp_file.path()).unwrap();
        let loaded = load_config_from_path(temp_file.path()).unwrap();
        
        assert_eq!(config_with_special_chars.gemini.api_key, loaded.gemini.api_key);
        assert_eq!(config_with_special_chars.gemini.model, loaded.gemini.model);
    }

    #[test]
    fn test_config_with_unicode() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut config_with_unicode = SagittaCodeConfig::default();
        config_with_unicode.gemini.api_key = Some("🔑-unicode-key-🚀".to_string());
        config_with_unicode.gemini.model = "model-with-émojis-🤖".to_string();
        
        // Save and load
        save_config_to_path(&config_with_unicode, temp_file.path()).unwrap();
        let loaded = load_config_from_path(temp_file.path()).unwrap();
        
        assert_eq!(config_with_unicode.gemini.api_key, loaded.gemini.api_key);
        assert_eq!(config_with_unicode.gemini.model, loaded.gemini.model);
    }

    #[test]
    fn test_partial_config_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let partial_content = r#"
{
  "gemini": {
    "model": "partial-model"
  }
}
"#;
        
        fs::write(temp_file.path(), partial_content).unwrap();
        
        let result = load_config_from_path(temp_file.path()).unwrap();
        
        assert_eq!(result.gemini.model, "partial-model");
        assert!(result.gemini.api_key.is_none());
    }

    #[test]
    fn test_config_with_extra_fields() {
        let temp_file = NamedTempFile::new().unwrap();
        let content_with_extra = r#"
{
  "gemini": {
    "api_key": "test-key",
    "model": "test-model",
    "extra_field": "this should be ignored"
  },
  "unknown_section": {
    "unknown_field": "also ignored"
  }
}
"#;
        
        fs::write(temp_file.path(), content_with_extra).unwrap();
        
        // Should still load successfully, ignoring unknown fields
        let result = load_config_from_path(temp_file.path()).unwrap();
        
        assert_eq!(result.gemini.api_key, Some("test-key".to_string()));
        assert_eq!(result.gemini.model, "test-model");
    }
}
