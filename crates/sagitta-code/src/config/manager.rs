//! Configuration management utilities for provider system

use std::path::Path;
use crate::config::types::SagittaCodeConfig;
use crate::providers::types::{ProviderType, ProviderConfig, ConfigError};

/// Configuration manager for handling provider configurations
pub struct ConfigManager {
    config: SagittaCodeConfig,
}

impl ConfigManager {
    /// Creates a new configuration manager with the given config
    pub fn new(config: SagittaCodeConfig) -> Self {
        Self { config }
    }
    
    /// Creates a configuration manager by loading from the default path
    pub fn load_default() -> Result<Self, ConfigError> {
        let config_path = SagittaCodeConfig::default().config_path();
        Self::load_from_path(&config_path)
    }
    
    /// Creates a configuration manager by loading from a specific path
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let mut config = SagittaCodeConfig::load_from_path(path)
            .map_err(|e| ConfigError::InvalidValue(format!("Failed to load config: {}", e)))?;
        
        // Perform migration if needed
        config.migrate_legacy_config();
        
        Ok(Self::new(config))
    }
    
    /// Gets the current configuration
    pub fn config(&self) -> &SagittaCodeConfig {
        &self.config
    }
    
    /// Gets a mutable reference to the current configuration
    pub fn config_mut(&mut self) -> &mut SagittaCodeConfig {
        &mut self.config
    }
    
    /// Gets the current active provider type
    pub fn current_provider(&self) -> ProviderType {
        self.config.current_provider
    }
    
    /// Sets the current active provider
    pub fn set_current_provider(&mut self, provider_type: ProviderType) {
        self.config.set_current_provider(provider_type);
    }
    
    /// Gets the configuration for the current provider
    pub fn current_provider_config(&self) -> Option<&ProviderConfig> {
        self.config.get_current_provider_config()
    }
    
    /// Gets a mutable reference to the configuration for the current provider
    pub fn current_provider_config_mut(&mut self) -> Option<&mut ProviderConfig> {
        self.config.get_current_provider_config_mut()
    }
    
    /// Gets the configuration for a specific provider
    pub fn provider_config(&self, provider_type: ProviderType) -> Option<&ProviderConfig> {
        self.config.provider_configs.get(&provider_type)
    }
    
    /// Gets a mutable reference to the configuration for a specific provider
    pub fn provider_config_mut(&mut self, provider_type: ProviderType) -> Option<&mut ProviderConfig> {
        self.config.provider_configs.get_mut(&provider_type)
    }
    
    /// Adds or updates a provider configuration
    pub fn set_provider_config(&mut self, provider_type: ProviderType, config: ProviderConfig) {
        self.config.provider_configs.insert(provider_type, config);
    }
    
    /// Removes a provider configuration
    pub fn remove_provider_config(&mut self, provider_type: ProviderType) -> Option<ProviderConfig> {
        self.config.provider_configs.remove(&provider_type)
    }
    
    /// Gets all configured provider types
    pub fn configured_providers(&self) -> Vec<ProviderType> {
        self.config.provider_configs.keys().cloned().collect()
    }
    
    /// Gets all enabled provider types
    pub fn enabled_providers(&self) -> Vec<ProviderType> {
        self.config.provider_configs
            .iter()
            .filter(|(_, config)| config.enabled)
            .map(|(provider_type, _)| *provider_type)
            .collect()
    }
    
    /// Checks if a provider is configured and enabled
    pub fn is_provider_enabled(&self, provider_type: ProviderType) -> bool {
        self.config.provider_configs
            .get(&provider_type)
            .map(|config| config.enabled)
            .unwrap_or(false)
    }
    
    /// Enables or disables a provider
    pub fn set_provider_enabled(&mut self, provider_type: ProviderType, enabled: bool) -> Result<(), ConfigError> {
        match self.config.provider_configs.get_mut(&provider_type) {
            Some(config) => {
                config.enabled = enabled;
                Ok(())
            }
            None => Err(ConfigError::ProviderNotFound(format!("{:?}", provider_type)))
        }
    }
    
    /// Validates that the current provider is configured and enabled
    pub fn validate_current_provider(&self) -> Result<(), ConfigError> {
        let current = self.current_provider();
        
        match self.provider_config(current) {
            Some(config) if config.enabled => Ok(()),
            Some(_) => Err(ConfigError::ProviderDisabled(format!("{:?}", current))),
            None => Err(ConfigError::ProviderNotFound(format!("{:?}", current))),
        }
    }
    
    /// Saves the configuration to the default path
    pub fn save_default(&self) -> Result<(), ConfigError> {
        let config_path = self.config.config_path();
        self.save_to_path(&config_path)
    }
    
    /// Saves the configuration to a specific path
    pub fn save_to_path(&self, path: &Path) -> Result<(), ConfigError> {
        self.config.save_to_path(path)
            .map_err(|e| ConfigError::InvalidValue(format!("Failed to save config: {}", e)))
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        let mut config = SagittaCodeConfig::default();
        config.migrate_legacy_config();
        Self::new(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    
    #[test]
    fn test_config_manager_creation() {
        let manager = ConfigManager::default();
        
        // Should have Claude Code as default provider
        assert_eq!(manager.current_provider(), ProviderType::ClaudeCode);
        
        // Should have at least one provider configured
        assert!(!manager.configured_providers().is_empty());
        
        // Should be valid
        assert!(manager.validate_current_provider().is_ok());
    }
    
    #[test]
    fn test_provider_management() {
        let mut manager = ConfigManager::default();
        
        // Test setting current provider
        manager.set_current_provider(ProviderType::MistralRs);
        assert_eq!(manager.current_provider(), ProviderType::MistralRs);
        
        // Test getting provider configs
        assert!(manager.current_provider_config().is_some());
        
        // Test enabling/disabling
        assert!(manager.set_provider_enabled(ProviderType::ClaudeCode, false).is_ok());
        assert!(!manager.is_provider_enabled(ProviderType::ClaudeCode));
        
        assert!(manager.set_provider_enabled(ProviderType::ClaudeCode, true).is_ok());
        assert!(manager.is_provider_enabled(ProviderType::ClaudeCode));
    }
    
    #[test]
    fn test_config_persistence() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        
        // Create and save config
        let mut manager = ConfigManager::default();
        manager.set_current_provider(ProviderType::MistralRs);
        assert!(manager.save_to_path(&config_path).is_ok());
        
        // Verify file was created
        assert!(config_path.exists());
        
        // Load config and verify
        let loaded_manager = ConfigManager::load_from_path(&config_path).unwrap();
        assert_eq!(loaded_manager.current_provider(), ProviderType::MistralRs);
    }
    
    #[test]
    fn test_migration() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("legacy_config.toml");
        
        // Create a legacy config file
        let legacy_config = r#"
[claude_code]
claude_path = "custom-claude"
model = "claude-3-sonnet-20240229"
debug = true
        "#;
        
        fs::write(&config_path, legacy_config).unwrap();
        
        // Load and verify migration
        let manager = ConfigManager::load_from_path(&config_path).unwrap();
        assert_eq!(manager.current_provider(), ProviderType::ClaudeCode);
        assert!(manager.current_provider_config().is_some());
        
        // Verify legacy config was preserved
        assert!(manager.config().claude_code.is_some());
    }
}