use anyhow::{Context, Result};
use log;
use serde::Deserialize;
use std::fs;

const APP_NAME: &str = "vectordb-cli";
const CONFIG_FILE_NAME: &str = "config.toml";
const DEFAULT_QDRANT_URL: &str = "http://localhost:6334";

#[derive(Deserialize, Debug, Clone)]
/// Represents the application configuration, loaded from a `config.toml` file.
pub struct AppConfig {
    /// The URL for the Qdrant gRPC endpoint.
    #[serde(default = "default_qdrant_url")]
    pub qdrant_url: String,
    /// Optional path to the ONNX model file (e.g., `/path/to/model.onnx`).
    /// If set, overrides the `VECTORDB_ONNX_MODEL` environment variable.
    pub onnx_model_path: Option<String>,
    /// Optional path to the directory containing the ONNX tokenizer files (e.g., `tokenizer.json`).
    /// If set, overrides the `VECTORDB_ONNX_TOKENIZER_DIR` environment variable.
    pub onnx_tokenizer_path: Option<String>,
    // Add other configuration options here as needed
}

fn default_qdrant_url() -> String {
    DEFAULT_QDRANT_URL.to_string()
}

/// Loads the application configuration from the XDG config directory.
///
/// If the config file or directory doesn't exist, it returns the default configuration.
/// If the file exists but cannot be parsed, it logs an error and returns default configuration.
pub fn load_config() -> Result<AppConfig> {
    match dirs::config_dir() {
        Some(config_dir) => {
            let app_config_dir = config_dir.join(APP_NAME);
            let config_file_path = app_config_dir.join(CONFIG_FILE_NAME);

            if !config_file_path.exists() {
                log::info!(
                    "Config file not found at '{}'. Using default settings.",
                    config_file_path.display()
                );
                // Optionally, create the directory and a default config file here
                // fs::create_dir_all(&app_config_dir).context("Failed to create config directory")?;
                // let default_config_content = toml::to_string(&AppConfig::default()).unwrap();
                // fs::write(&config_file_path, default_config_content).context("Failed to write default config file")?;
                return Ok(AppConfig::default());
            }

            log::info!("Loading config from '{}'", config_file_path.display());
            let config_content = fs::read_to_string(&config_file_path)
                .with_context(|| format!("Failed to read config file at '{}'", config_file_path.display()))?;

            match toml::from_str(&config_content) {
                Ok(config) => Ok(config),
                Err(e) => {
                    log::error!(
                        "Failed to parse config file at '{}': {}. Using default settings.",
                        config_file_path.display(),
                        e
                    );
                    // Return default config on parse error
                    Ok(AppConfig::default())
                }
            }
        }
        None => {
            log::warn!("Could not find XDG config directory. Using default settings.");
            Ok(AppConfig::default())
        }
    }
}

// Implement Default for AppConfig to easily get defaults
impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            qdrant_url: default_qdrant_url(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_config_defaults() {
        // Simulate no config directory found (difficult with dirs crate directly)
        // We rely on the log messages and the default return path for now.
        // A more robust test would mock the dirs::config_dir function.

        // Test case where config dir exists but file doesn't
        let temp_dir = tempdir().unwrap();
        let fake_config_parent = temp_dir.path();
        let fake_app_config_dir = fake_config_parent.join(APP_NAME);
        // Don't create the file

        // Mocking dirs::config_dir is complex, so we test the logic flow assuming it returns our temp path
        // This manual simulation isn't ideal but tests parts of the logic.
        
        // Let's test the scenario where the file exists but is invalid TOML
        fs::create_dir_all(&fake_app_config_dir).unwrap();
        let config_file_path = fake_app_config_dir.join(CONFIG_FILE_NAME);
        fs::write(&config_file_path, "this is not valid toml {}{").unwrap();

        // We can't directly call load_config and intercept dirs::config_dir easily.
        // Instead, let's focus on testing the default implementation directly.
        let default_cfg = AppConfig::default();
        assert_eq!(default_cfg.qdrant_url, DEFAULT_QDRANT_URL);
        
        // We could test the parsing logic more directly if needed by creating a test function
        // that takes a path, reads, and parses, bypassing the dirs crate for the test.
    }
    
    // Example of testing parsing logic separately:
    fn parse_toml_content(content: &str) -> Result<AppConfig, toml::de::Error> {
        toml::from_str(content)
    }

    #[test]
    fn test_toml_parsing() {
        let valid_toml = r#"
            qdrant_url = "http://custom-qdrant:1234"
            onnx_model_path = "/path/to/model.onnx"
            onnx_tokenizer_path = "/path/to/tokenizer/"
        "#;
        let config = parse_toml_content(valid_toml).unwrap();
        assert_eq!(config.qdrant_url, "http://custom-qdrant:1234");
        assert_eq!(config.onnx_model_path, Some("/path/to/model.onnx".to_string()));
        assert_eq!(config.onnx_tokenizer_path, Some("/path/to/tokenizer/".to_string()));

        let valid_toml_default = r#"
            # No qdrant_url specified, should use default
        "#;
        let config_default = parse_toml_content(valid_toml_default).unwrap();
        assert_eq!(config_default.qdrant_url, DEFAULT_QDRANT_URL);
        assert_eq!(config_default.onnx_model_path, None);
        assert_eq!(config_default.onnx_tokenizer_path, None);


        let invalid_toml = "qdrant_url = http://no-quotes";
        let result = parse_toml_content(invalid_toml);
        assert!(result.is_err());
    }
} 