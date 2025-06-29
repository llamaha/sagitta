use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::types::SagittaCodeConfig;
use super::paths::get_sagitta_code_app_config_path;

const CONFIG_FILENAME: &str = "sagitta_code_config.json";

/// Get the path to the configuration file (deprecated - use get_sagitta_code_app_config_path)
pub fn get_config_path() -> Result<PathBuf> {
    get_sagitta_code_app_config_path()
}

/// Load configuration from the default location
pub fn load_config() -> Result<SagittaCodeConfig> {
    let config_path = get_sagitta_code_app_config_path()?;
    
    if config_path.exists() {
        load_config_from_path(&config_path)
    } else {
        // Return default config if file doesn't exist
        let default_config = SagittaCodeConfig::default();
        
        // Create directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Save the default config
        save_config(&default_config)?;
        
        Ok(default_config)
    }
}

/// Load configuration from a specific path
pub fn load_config_from_path(path: &Path) -> Result<SagittaCodeConfig> {
    let content = fs::read_to_string(path)?;
    let mut config: SagittaCodeConfig = toml::from_str(&content)?;
    
    // Validate configuration
    validate_config(&mut config)?;
    
    Ok(config)
}

/// Save configuration to the default location
pub fn save_config(config: &SagittaCodeConfig) -> Result<()> {
    // Use the standard path resolution which already handles test isolation
    let config_path = get_sagitta_code_app_config_path()?;
    
    // Create directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let content = toml::to_string_pretty(config)?;
    fs::write(config_path, content)?;
    
    Ok(())
}

/// Save configuration to a specific path
pub fn save_config_to_path(config: &SagittaCodeConfig, path: &Path) -> Result<()> {
    // Create directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let content = toml::to_string_pretty(config)?;
    fs::write(path, content)?;
    
    Ok(())
}

/// Load merged configuration from both sagitta-code and sagitta-search configs
pub fn load_merged_config() -> Result<SagittaCodeConfig> {
    // Load sagitta-code config first
    let mut config = load_config()?;
    
    // Try to load sagitta-search config for repository settings
    // Respect test isolation by checking for SAGITTA_TEST_CONFIG_PATH
    let sagitta_config_path = if let Ok(test_path) = std::env::var("SAGITTA_TEST_CONFIG_PATH") {
        Some(std::path::PathBuf::from(test_path))
    } else {
        None
    };
    
    if let Ok(sagitta_config) = sagitta_search::config::load_config(sagitta_config_path.as_ref()) {
        // If repositories_base_path is not set in sagitta-code config,
        // use the one from sagitta-search
        if config.sagitta.repositories_base_path.is_none() {
            if let Some(search_repos_path) = sagitta_config.repositories_base_path {
                config.sagitta.repositories_base_path = Some(search_repos_path.into());
            }
        }
        
        // Merge repository lists - convert RepositoryConfig to strings
        for repo_config in sagitta_config.repositories {
            let repo_name = repo_config.name;
            if !config.sagitta.repositories.contains(&repo_name) {
                config.sagitta.repositories.push(repo_name);
            }
        }
    }
    
    Ok(config)
}

/// Load all related configurations
pub fn load_all_configs() -> Result<(SagittaCodeConfig, Option<sagitta_search::config::AppConfig>)> {
    let sagitta_code_config = load_config()?;
    
    // Respect test isolation by checking for SAGITTA_TEST_CONFIG_PATH
    let sagitta_config_path = if let Ok(test_path) = std::env::var("SAGITTA_TEST_CONFIG_PATH") {
        Some(std::path::PathBuf::from(test_path))
    } else {
        None
    };
    
    let sagitta_search_config = sagitta_search::config::load_config(sagitta_config_path.as_ref()).ok();
    
    Ok((sagitta_code_config, sagitta_search_config))
}

/// Validate configuration and apply defaults where needed
fn validate_config(config: &mut SagittaCodeConfig) -> Result<()> {
    // Validate Claude Code configuration
    if config.claude_code.claude_path.is_empty() {
        return Err(anyhow!("Claude binary path cannot be empty"));
    }
    
    if config.claude_code.model.is_empty() {
        return Err(anyhow!("Claude model cannot be empty"));
    }
    
    // Validate timeout value
    if config.claude_code.timeout == 0 {
        return Err(anyhow!("Claude timeout must be greater than 0"));
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{TempDir, NamedTempFile};
    use std::fs;

    fn create_test_config() -> SagittaCodeConfig {
        let mut config = SagittaCodeConfig::default();
        config.claude_code.model = "claude-sonnet-4-20250514".to_string();
        config.claude_code.claude_path = "claude".to_string();
        config
    }

    #[test]
    fn test_load_config_from_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.toml");
        
        let result = load_config_from_path(&config_path);
        
        // Should return an error for nonexistent files
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No such file or directory"));
    }

    #[test]
    fn test_load_config_from_valid_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
[claude_code]
claude_path = "claude"
model = "claude-sonnet-4-20250514"
max_output_tokens = 8192
timeout = 600

[sagitta]

[ui]
theme = "default"
window_width = 900
window_height = 700

[logging]
log_level = "info"
log_to_file = false

[conversation]
"#;
        
        fs::write(temp_file.path(), config_content).unwrap();
        
        let result = load_config_from_path(temp_file.path()).unwrap();
        
        assert_eq!(result.claude_code.claude_path, "claude");
        assert_eq!(result.claude_code.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_load_config_from_invalid_toml() {
        let temp_file = NamedTempFile::new().unwrap();
        let invalid_content = "invalid toml content [[[";
        
        fs::write(temp_file.path(), invalid_content).unwrap();
        
        let result = load_config_from_path(temp_file.path());
        assert!(result.is_err());
        // The actual error comes from TOML parsing
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("expected") || error_msg.contains("TOML") || error_msg.contains("parse"));
    }

    #[test]
    fn test_load_config_from_unreadable_file() {
        // Test with a path that doesn't exist and can't be read
        let result = load_config_from_path(Path::new("/nonexistent/path/config.toml"));
        
        // Should return an error for unreadable files
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("No such file or directory") || error_msg.contains("Permission denied"));
    }

    #[test]
    fn test_save_config_to_path() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        let test_config = create_test_config();
        
        let result = save_config_to_path(&test_config, &config_path);
        assert!(result.is_ok());
        
        // Verify the file was created and contains the expected content
        assert!(config_path.exists());
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("claude-sonnet-4-20250514"));
        assert!(content.contains("claude"));
    }

    #[test]
    fn test_save_config_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir.path().join("nested").join("config.toml");
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
        test_config.claude_code.model = "claude-sonnet-4-20250514".to_string();
        test_config.claude_code.claude_path = "claude".to_string();
        
        save_config_to_path(&test_config, temp_file.path()).unwrap();
        
        let content = fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.contains("claude-sonnet-4-20250514"));
        assert!(content.contains("claude"));
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
        assert_eq!(original_config.claude_code.model, loaded_config.claude_code.model);
        assert_eq!(original_config.claude_code.claude_path, loaded_config.claude_code.claude_path);
    }

    #[test]
    fn test_validate_config_valid() {
        let mut valid_config = create_test_config();
        let result = validate_config(&mut valid_config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_config_empty_model() {
        let mut invalid_config = SagittaCodeConfig::default();
        invalid_config.claude_code.model = "".to_string();
        
        let result = validate_config(&mut invalid_config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Claude model cannot be empty"));
    }

    #[test]
    fn test_validate_config_empty_claude_path() {
        let mut invalid_config = SagittaCodeConfig::default();
        invalid_config.claude_code.claude_path = "".to_string();
        
        let result = validate_config(&mut invalid_config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Claude binary path cannot be empty"));
    }

    #[test]
    fn test_validate_config_zero_timeout() {
        let mut invalid_config = SagittaCodeConfig::default();
        invalid_config.claude_code.timeout = 0;
        
        let result = validate_config(&mut invalid_config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Claude timeout must be greater than 0"));
    }

    #[test]
    fn test_load_all_configs() {
        // This test should not call the actual load_all_configs() function during tests
        // as it would access the user's real config file and potentially overwrite it.
        // Instead, we test the function's behavior in a controlled way.
        
        // Test that the function exists and has the correct signature
        // by checking it compiles, but don't actually call it in tests
        let _function_exists: fn() -> Result<(SagittaCodeConfig, Option<sagitta_search::config::AppConfig>)> = load_all_configs;
        
        // The actual functionality is tested through integration tests
        // that properly set up test isolation
    }

    #[test]
    fn test_config_with_special_characters() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut config_with_special_chars = SagittaCodeConfig::default();
        config_with_special_chars.claude_code.model = "claude-model-with-dashes_underscores".to_string();
        config_with_special_chars.claude_code.claude_path = "/path/with/special-chars!@#".to_string();
        
        // Save and load
        save_config_to_path(&config_with_special_chars, temp_file.path()).unwrap();
        let loaded = load_config_from_path(temp_file.path()).unwrap();
        
        assert_eq!(config_with_special_chars.claude_code.model, loaded.claude_code.model);
        assert_eq!(config_with_special_chars.claude_code.claude_path, loaded.claude_code.claude_path);
    }

    #[test]
    fn test_config_with_unicode() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut config_with_unicode = SagittaCodeConfig::default();
        config_with_unicode.claude_code.model = "claude-with-émojis-🤖".to_string();
        config_with_unicode.claude_code.claude_path = "/path/with/unicode-🚀".to_string();
        
        // Save and load
        save_config_to_path(&config_with_unicode, temp_file.path()).unwrap();
        let loaded = load_config_from_path(temp_file.path()).unwrap();
        
        assert_eq!(config_with_unicode.claude_code.model, loaded.claude_code.model);
        assert_eq!(config_with_unicode.claude_code.claude_path, loaded.claude_code.claude_path);
    }

    #[test]
    fn test_partial_config_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let partial_content = r#"
[claude_code]
model = "claude-sonnet-4-20250514"
"#;
        
        fs::write(temp_file.path(), partial_content).unwrap();
        
        let result = load_config_from_path(temp_file.path()).unwrap();
        
        assert_eq!(result.claude_code.model, "claude-sonnet-4-20250514");
        // claude_path should use default
        assert_eq!(result.claude_code.claude_path, "claude");
    }

    #[test]
    fn test_config_with_extra_fields() {
        let temp_file = NamedTempFile::new().unwrap();
        let content_with_extra = r#"
[claude_code]
claude_path = "claude"
model = "claude-sonnet-4-20250514"
extra_field = "this should be ignored"

[unknown_section]
unknown_field = "also ignored"
"#;
        
        fs::write(temp_file.path(), content_with_extra).unwrap();
        
        // Should still load successfully, ignoring unknown fields
        let result = load_config_from_path(temp_file.path()).unwrap();
        
        assert_eq!(result.claude_code.claude_path, "claude");
        assert_eq!(result.claude_code.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_save_and_load_repository_context() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut config = SagittaCodeConfig::default();
        
        // Set repository context
        config.ui.current_repository_context = Some("my-test-repo".to_string());
        
        // Save the config
        save_config_to_path(&config, temp_file.path()).unwrap();
        
        // Load it back
        let loaded_config = load_config_from_path(temp_file.path()).unwrap();
        
        // Verify repository context was preserved
        assert_eq!(loaded_config.ui.current_repository_context, Some("my-test-repo".to_string()));
    }

    #[test]
    fn test_load_config_without_repository_context() {
        // Create a config file without the repository context field
        let temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
[claude_code]
claude_path = "claude"
model = "claude-sonnet-4-20250514"

[ui]
theme = "dark"
window_width = 1200
window_height = 800
# Note: no current_repository_context field

[sagitta]

[logging]

[conversation]
"#;
        
        fs::write(temp_file.path(), config_content).unwrap();
        
        let loaded_config = load_config_from_path(temp_file.path()).unwrap();
        
        // Should default to None
        assert_eq!(loaded_config.ui.current_repository_context, None);
    }
}
