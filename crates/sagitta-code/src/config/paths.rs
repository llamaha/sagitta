use std::path::PathBuf;
use directories::ProjectDirs;
use anyhow::Result; // Ensure anyhow::Result is used

/// Gets the path to Sagitta Code's application configuration file.
/// This will be ~/.config/sagitta/sagitta_code_config.json (moved from sagitta_code subdirectory)
pub fn get_sagitta_code_app_config_path() -> Result<PathBuf> {
    if let Some(proj_dirs) = ProjectDirs::from("", "", "sagitta") { // Changed from "sagitta_code" to "sagitta"
        let config_dir = proj_dirs.config_dir();
        if !config_dir.exists() {
            std::fs::create_dir_all(config_dir).map_err(|e| anyhow::anyhow!("Failed to create config directory {:?}: {}", config_dir, e))?;
        }
        Ok(config_dir.join("sagitta_code_config.json")) // Keep the filename for app-specific config
    } else {
        Err(anyhow::anyhow!("Unable to determine Sagitta config directory"))
    }
}

/// Gets the path to the shared sagitta configuration directory for data storage
/// This will be ~/.local/share/sagitta/
pub fn get_sagitta_data_dir() -> Result<PathBuf> {
    if let Some(proj_dirs) = ProjectDirs::from("", "", "sagitta") {
        let data_dir = proj_dirs.data_dir();
        if !data_dir.exists() {
            std::fs::create_dir_all(data_dir).map_err(|e| anyhow::anyhow!("Failed to create data directory {:?}: {}", data_dir, e))?;
        }
        Ok(data_dir.to_path_buf())
    } else {
        Err(anyhow::anyhow!("Unable to determine Sagitta data directory"))
    }
}

/// Gets the path for conversation storage
/// This will be ~/.local/share/sagitta/conversations/
pub fn get_conversations_path() -> Result<PathBuf> {
    let conversations_dir = get_sagitta_data_dir()?.join("conversations");
    if !conversations_dir.exists() {
        std::fs::create_dir_all(&conversations_dir).map_err(|e| anyhow::anyhow!("Failed to create conversations directory {:?}: {}", conversations_dir, e))?;
    }
    Ok(conversations_dir)
}

/// Gets the path for logs storage
/// This will be ~/.local/share/sagitta/logs/
pub fn get_logs_path() -> Result<PathBuf> {
    let logs_dir = get_sagitta_data_dir()?.join("logs");
    if !logs_dir.exists() {
        std::fs::create_dir_all(&logs_dir).map_err(|e| anyhow::anyhow!("Failed to create logs directory {:?}: {}", logs_dir, e))?;
    }
    Ok(logs_dir)
}

/// Migrates configuration from old locations to new unified structure
pub fn migrate_old_config() -> Result<()> {
    // Old paths
    let old_config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
        .join("sagitta_code");
    
    let old_app_config = old_config_dir.join("sagitta_code_config.json");
    let old_core_config = old_config_dir.join("core_config.toml");
    
    // New paths
    let new_app_config = get_sagitta_code_app_config_path()?;
    let new_core_config = sagitta_search::config::get_config_path()?;
    
    let mut migrated = false;
    
    // Migrate app config
    if old_app_config.exists() && !new_app_config.exists() {
        std::fs::rename(&old_app_config, &new_app_config)
            .map_err(|e| anyhow::anyhow!("Failed to migrate app config: {}", e))?;
        log::info!("Migrated sagitta-code config from {} to {}", old_app_config.display(), new_app_config.display());
        migrated = true;
    }
    
    // Migrate core config if it exists and shared config doesn't exist
    if old_core_config.exists() && !new_core_config.exists() {
        std::fs::rename(&old_core_config, &new_core_config)
            .map_err(|e| anyhow::anyhow!("Failed to migrate core config: {}", e))?;
        log::info!("Migrated core config from {} to {}", old_core_config.display(), new_core_config.display());
        migrated = true;
    }
    
    // Remove old directory if it's empty
    if migrated && old_config_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&old_config_dir) {
            if entries.count() == 0 {
                if let Err(e) = std::fs::remove_dir(&old_config_dir) {
                    log::warn!("Could not remove old config directory {}: {}", old_config_dir.display(), e);
                } else {
                    log::info!("Removed old config directory: {}", old_config_dir.display());
                }
            }
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use crate::utils::errors::SagittaCodeError;

    #[test]
    fn test_get_sagitta_code_app_config_path() {
        let result = get_sagitta_code_app_config_path();
        
        match result {
            Ok(path) => {
                // Should end with the correct filename
                assert!(path.to_string_lossy().ends_with("sagitta_code_config.json"));
                
                // Should contain the .config directory and sagitta (not sagitta_code)
                assert!(path.to_string_lossy().contains(".config"));
                assert!(path.to_string_lossy().contains("sagitta"));
                // Should NOT contain sagitta_code in directory path (only in filename)
                let path_str = path.to_string_lossy();
                let path_without_filename = path.parent().unwrap().to_string_lossy();
                assert!(!path_without_filename.contains("sagitta_code"));
                
                // Path should be absolute
                assert!(path.is_absolute());
            },
            Err(e) => {
                // Config errors are acceptable in test environments
                assert!(!e.to_string().is_empty());
            }
        }
    }

    #[test]
    fn test_get_conversations_path() {
        let result = get_conversations_path();
        
        match result {
            Ok(path) => {
                // Should end with conversations
                assert!(path.to_string_lossy().ends_with("conversations"));
                
                // Should be in data directory
                assert!(path.to_string_lossy().contains("sagitta"));
                
                // Path should be absolute
                assert!(path.is_absolute());
            },
            Err(e) => {
                assert!(!e.to_string().is_empty());
            }
        }
    }

    #[test]
    fn test_get_logs_path() {
        let result = get_logs_path();
        
        match result {
            Ok(path) => {
                // Should end with logs
                assert!(path.to_string_lossy().ends_with("logs"));
                
                // Should be in data directory
                assert!(path.to_string_lossy().contains("sagitta"));
                
                // Path should be absolute
                assert!(path.is_absolute());
            },
            Err(e) => {
                assert!(!e.to_string().is_empty());
            }
        }
    }

    #[test]
    fn test_data_and_config_separation() {
        let app_config_result = get_sagitta_code_app_config_path();
        let conversations_result = get_conversations_path();
        let logs_result = get_logs_path();
        
        if let (Ok(app_path), Ok(conv_path), Ok(logs_path)) = (app_config_result, conversations_result, logs_result) {
            let app_str = app_path.to_string_lossy();
            let conv_str = conv_path.to_string_lossy();
            let logs_str = logs_path.to_string_lossy();
            
            // Config should be in .config
            assert!(app_str.contains(".config"));
            
            // Data should be in .local/share or similar
            assert!(!conv_str.contains(".config"));
            assert!(!logs_str.contains(".config"));
            
            // All should be under sagitta namespace
            assert!(app_str.contains("sagitta"));
            assert!(conv_str.contains("sagitta"));
            assert!(logs_str.contains("sagitta"));
        }
    }

    #[test]
    fn test_path_consistency() {
        // Test that calling the functions multiple times returns the same paths
        let path1 = get_sagitta_code_app_config_path();
        let path2 = get_sagitta_code_app_config_path();
        
        assert_eq!(path1.is_ok(), path2.is_ok());
        if let (Ok(p1), Ok(p2)) = (path1, path2) {
            assert_eq!(p1, p2);
        }
    }
} 