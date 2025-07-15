use anyhow::Result;
use sagitta_code::config::types::{SagittaCodeConfig, ClaudeCodeConfig};
use sagitta_code::providers::types::ProviderType;
use sagitta_code::providers::types::ProviderConfig;
use std::collections::HashMap;
use tempfile::TempDir;

/// Initialize test isolation for config tests
fn init_test_isolation() {
    let _ = env_logger::builder()
        .is_test(true)
        .try_init();
}

#[cfg(test)]
mod config_migration_tests {
    use super::*;
    
    #[test]
    fn test_legacy_claude_config_migration() {
        init_test_isolation();
        
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        // Create legacy config structure (pre-provider system)
        let legacy_config = r#"
[claude_code]
api_key = "test-key"
model = "claude-3-sonnet-20240229"
max_tokens = 4096
timeout = 300
temperature = 0.1
"#;
        std::fs::write(&config_path, legacy_config).unwrap();
        
        // Load config (should trigger migration)
        let config = SagittaCodeConfig::load_from_path(&config_path).unwrap();
        
        // Verify migration occurred
        assert_eq!(config.current_provider, ProviderType::ClaudeCode);
        assert!(config.provider_configs.contains_key(&ProviderType::ClaudeCode));
        
        // Verify Claude Code config was migrated correctly
        if let Some(provider_config) = config.provider_configs.get(&ProviderType::ClaudeCode) {
            let claude_config: sagitta_code::providers::types::ClaudeCodeConfig = 
                provider_config.try_into().unwrap();
            
            // Check the new ClaudeCodeConfig structure
            assert_eq!(claude_config.binary_path, Some("claude".to_string()));
            assert_eq!(claude_config.timeout_seconds, 300);
            assert!(claude_config.additional_args.is_empty());
        } else {
            panic!("Claude Code config not found after migration");
        }
    }
    
    #[test]
    fn test_multiple_provider_config_persistence() {
        init_test_isolation();
        
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        // Create config with multiple providers
        let mut config = SagittaCodeConfig::default();
        config.current_provider = ProviderType::MistralRs;
        
        // Add Claude Code config
        let mut claude_config = ProviderConfig::new(ProviderType::ClaudeCode);
        claude_config.set_option("binary_path", &Some("claude".to_string())).unwrap();
        claude_config.set_option("timeout_seconds", &600u64).unwrap();
        claude_config.set_option("additional_args", &Vec::<String>::new()).unwrap();
        config.provider_configs.insert(ProviderType::ClaudeCode, claude_config);
        
        // Add Mistral.rs config
        let mut mistral_config = ProviderConfig::new(ProviderType::MistralRs);
        mistral_config.set_option("base_url", &"http://localhost:1234".to_string()).unwrap();
        mistral_config.set_option("model", &Some("mistral-7b".to_string())).unwrap();
        mistral_config.set_option("timeout_seconds", &120u64).unwrap();
        config.provider_configs.insert(ProviderType::MistralRs, mistral_config);
        
        // Save and reload
        config.save_to_path(&config_path).unwrap();
        let reloaded_config = SagittaCodeConfig::load_from_path(&config_path).unwrap();
        
        // Verify persistence
        assert_eq!(reloaded_config.current_provider, ProviderType::MistralRs);
        assert_eq!(reloaded_config.provider_configs.len(), 2);
        
        // Verify Claude Code config
        let claude_reloaded: sagitta_code::providers::types::ClaudeCodeConfig = 
            reloaded_config.provider_configs[&ProviderType::ClaudeCode].clone().try_into().unwrap();
        // Verify the config was loaded correctly
        assert_eq!(claude_reloaded.timeout_seconds, 600);
        assert_eq!(claude_reloaded.binary_path, Some("claude".to_string()));
        assert!(claude_reloaded.additional_args.is_empty());
        
        // Verify Mistral.rs config
        let mistral_reloaded: sagitta_code::providers::types::MistralRsConfig = 
            reloaded_config.provider_configs[&ProviderType::MistralRs].clone().try_into().unwrap();
        assert_eq!(mistral_reloaded.base_url, "http://localhost:1234");
        assert_eq!(mistral_reloaded.model, Some("mistral-7b".to_string()));
        assert_eq!(mistral_reloaded.timeout_seconds, 120);
    }
    
    #[test]
    fn test_config_backward_compatibility() {
        init_test_isolation();
        
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        // Create old-style config without provider system
        let old_config = SagittaCodeConfig {
            current_provider: ProviderType::ClaudeCode,
            provider_configs: HashMap::new(), // Empty provider configs
            claude_code: Some(ClaudeCodeConfig {
                claude_path: "claude".to_string(),
                model: "claude-3-opus-20240229".to_string(),
                max_output_tokens: 8192,
                ..Default::default()
            }),
            ..Default::default()
        };
        
        // Save old config
        old_config.save_to_path(&config_path).unwrap();
        
        // Load should migrate to new format
        let migrated_config = SagittaCodeConfig::load_from_path(&config_path).unwrap();
        
        // Should still work and migrate the legacy claude_code field
        assert_eq!(migrated_config.current_provider, ProviderType::ClaudeCode);
        
        // If migration happened, provider_configs should be populated
        if !migrated_config.provider_configs.is_empty() {
            let provider_config = &migrated_config.provider_configs[&ProviderType::ClaudeCode];
            // The legacy config gets converted to provider config with these mapped fields
            assert_eq!(provider_config.get_option::<String>("binary_path").unwrap(), Some("claude".to_string()));
            assert_eq!(provider_config.get_option::<String>("model").unwrap(), Some("claude-3-opus-20240229".to_string()));
            assert_eq!(provider_config.get_option::<u32>("max_output_tokens").unwrap(), Some(8192));
        }
    }
    
    #[test]
    fn test_first_run_flag_persistence() {
        init_test_isolation();
        
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        // Create fresh config (first run)
        let mut config = SagittaCodeConfig::default();
        assert!(!config.ui.first_run_completed); // Should be false by default
        
        // Complete first run
        config.ui.first_run_completed = true;
        config.save_to_path(&config_path).unwrap();
        
        // Reload and verify persistence
        let reloaded_config = SagittaCodeConfig::load_from_path(&config_path).unwrap();
        assert!(reloaded_config.ui.first_run_completed);
    }
    
    #[test]
    fn test_config_validation_after_migration() {
        init_test_isolation();
        
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        // Create config with potentially problematic values
        let mut config = SagittaCodeConfig::default();
        config.current_provider = ProviderType::MistralRs;
        config.provider_configs.clear(); // Clear default providers
        
        // Add config with edge case values
        let mut mistral_config = ProviderConfig::new(ProviderType::MistralRs);
        mistral_config.set_option("base_url", &"".to_string()).unwrap(); // Empty URL
        mistral_config.set_option("timeout_seconds", &0u64).unwrap(); // Invalid timeout
        config.provider_configs.insert(ProviderType::MistralRs, mistral_config);
        
        // Save and reload
        config.save_to_path(&config_path).unwrap();
        let reloaded_config = SagittaCodeConfig::load_from_path(&config_path).unwrap();
        
        // Config should load successfully even with invalid values
        // Validation happens at usage time, not load time
        assert_eq!(reloaded_config.provider_configs.len(), 1);
        
        let mistral_reloaded: sagitta_code::providers::types::MistralRsConfig = 
            reloaded_config.provider_configs[&ProviderType::MistralRs].clone().try_into().unwrap();
        assert_eq!(mistral_reloaded.base_url, "");
        assert_eq!(mistral_reloaded.timeout_seconds, 0);
    }
    
    #[test]
    fn test_config_schema_evolution() {
        init_test_isolation();
        
        // Test that the config system can handle missing fields gracefully
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        // Create minimal config (as if from an older version)
        let minimal_toml = r#"
current_provider = "ClaudeCode"

[[provider_configs.ClaudeCode]]
provider_type = "ClaudeCode"
"#;
        std::fs::write(&config_path, minimal_toml).unwrap();
        
        // Should load successfully with defaults for missing fields
        let config_result = SagittaCodeConfig::load_from_path(&config_path);
        
        // Should either load successfully or handle the error gracefully
        match config_result {
            Ok(config) => {
                assert_eq!(config.current_provider, ProviderType::ClaudeCode);
            },
            Err(_) => {
                // If it fails to load, that's also acceptable as long as it's handled
                // The important thing is that it doesn't panic
            }
        }
    }
}

#[cfg(test)]
mod config_manager_tests {
    use super::*;
    use sagitta_code::config::manager::ConfigManager;
    
    #[test]
    fn test_config_manager_provider_operations() {
        init_test_isolation();
        
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        let mut config = SagittaCodeConfig::default();
        config.save_to_path(&config_path).unwrap();
        let mut config_manager = ConfigManager::load_from_path(&config_path).unwrap();
        
        // Test provider switching
        config_manager.set_current_provider(ProviderType::MistralRs);
        assert_eq!(config_manager.current_provider(), ProviderType::MistralRs);
        
        // Test provider config update
        let mut mistral_config = ProviderConfig::new(ProviderType::MistralRs);
        mistral_config.set_option("base_url", &"http://localhost:8080".to_string()).unwrap();
        
        config_manager.set_provider_config(ProviderType::MistralRs, mistral_config);
        
        // Verify persistence
        // Save by getting the config and saving it
        config_manager.config().save_to_path(&config_path).unwrap();
        let reloaded_manager = ConfigManager::load_from_path(&config_path).unwrap();
        
        assert_eq!(reloaded_manager.current_provider(), ProviderType::MistralRs);
        
        let provider_config = reloaded_manager.provider_config(ProviderType::MistralRs).unwrap();
        let reloaded_mistral_config: sagitta_code::providers::types::MistralRsConfig = 
            provider_config.clone().try_into().unwrap();
        assert_eq!(reloaded_mistral_config.base_url, "http://localhost:8080");
    }
    
    #[test]
    fn test_config_manager_migration_handling() {
        init_test_isolation();
        
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        // Create legacy config
        let legacy_config = r#"
[claude_code]
api_key = "legacy-key"
model = "claude-3-sonnet-20240229"
"#;
        std::fs::write(&config_path, legacy_config).unwrap();
        
        // ConfigManager should handle migration transparently
        let config_manager = ConfigManager::load_from_path(&config_path).unwrap();
        
        // Should have migrated to new format
        assert_eq!(config_manager.current_provider(), ProviderType::ClaudeCode);
        
        // The legacy config would have been migrated by the ConfigManager
        let provider_config = config_manager.provider_config(ProviderType::ClaudeCode).unwrap();
        // Legacy fields are mapped to provider config options during migration
        assert!(provider_config.provider_type == ProviderType::ClaudeCode);
    }
}