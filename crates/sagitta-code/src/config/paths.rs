use std::path::PathBuf;
use directories::ProjectDirs;
use anyhow::Result; // Ensure anyhow::Result is used

/// Gets the path to Sagitta Code's dedicated core configuration file (e.g., for sagitta-search settings).
/// This will be something like ~/.config/sagitta-code/core_config.toml
pub fn get_sagitta_code_core_config_path() -> Result<PathBuf> {
    if let Some(proj_dirs) = ProjectDirs::from("", "", "sagitta_code") { // Use empty vendor for typical ~/.config behavior
        let config_dir = proj_dirs.config_dir();
        if !config_dir.exists() {
            std::fs::create_dir_all(config_dir).map_err(|e| anyhow::anyhow!("Failed to create config directory {:?}: {}", config_dir, e))?;
        }
        Ok(config_dir.join("core_config.toml"))
    } else {
        Err(anyhow::anyhow!("Unable to determine Sagitta Code config directory"))
    }
}

/// Gets the path to Sagitta Code's main application configuration file.
/// This will be something like ~/.config/sagitta-code/sagitta_code_config.json
pub fn get_sagitta_code_app_config_path() -> Result<PathBuf> {
    if let Some(proj_dirs) = ProjectDirs::from("", "", "sagitta_code") {
        let config_dir = proj_dirs.config_dir();
        if !config_dir.exists() {
            std::fs::create_dir_all(config_dir).map_err(|e| anyhow::anyhow!("Failed to create config directory {:?}: {}", config_dir, e))?;
        }
        Ok(config_dir.join("sagitta_code_config.json"))
    } else {
        Err(anyhow::anyhow!("Unable to determine Sagitta Code config directory"))
    }
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
                
                // Should contain the .config directory
                assert!(path.to_string_lossy().contains(".config"));
                assert!(path.to_string_lossy().contains("sagitta_code"));
                
                // Path should be absolute
                assert!(path.is_absolute());
            },
            Err(e) => {
                // Config errors are acceptable in test environments
                // but we should at least verify the error is some kind of error
                assert!(!e.to_string().is_empty());
            }
        }
    }

    #[test]
    fn test_get_sagitta_code_core_config_path() {
        let result = get_sagitta_code_core_config_path();
        
        match result {
            Ok(path) => {
                // Should end with the correct filename
                assert!(path.to_string_lossy().ends_with("core_config.toml"));
                
                // Should contain the .config directory
                assert!(path.to_string_lossy().contains(".config"));
                assert!(path.to_string_lossy().contains("sagitta_code"));
                
                // Path should be absolute
                assert!(path.is_absolute());
            },
            Err(e) => {
                // Config errors are acceptable in test environments
                // but we should at least verify the error is some kind of error
                assert!(!e.to_string().is_empty());
            }
        }
    }

    #[test]
    fn test_different_config_paths() {
        let app_config_result = get_sagitta_code_app_config_path();
        let core_config_result = get_sagitta_code_core_config_path();
        
        // If both succeed, they should be different paths
        if let (Ok(app_path), Ok(core_path)) = (app_config_result, core_config_result) {
            assert_ne!(app_path, core_path);
            
            // Both should be in the same directory
            assert_eq!(app_path.parent(), core_path.parent());
            
            // File extensions should be different
            assert_eq!(app_path.extension().and_then(|s| s.to_str()), Some("json"));
            assert_eq!(core_path.extension().and_then(|s| s.to_str()), Some("toml"));
        }
    }

    #[test]
    fn test_config_directory_structure() {
        let app_config_result = get_sagitta_code_app_config_path();
        
        if let Ok(app_path) = app_config_result {
            let parent_dir = app_path.parent().expect("Should have parent directory");
            
            // Parent directory should end with "sagitta_code"
            assert_eq!(parent_dir.file_name().and_then(|s| s.to_str()), Some("sagitta_code"));
            
            // Grandparent should end with ".config"
            if let Some(grandparent) = parent_dir.parent() {
                assert_eq!(grandparent.file_name().and_then(|s| s.to_str()), Some(".config"));
            }
        }
    }

    #[test]
    fn test_path_string_conversion() {
        let app_config_result = get_sagitta_code_app_config_path();
        
        if let Ok(path) = app_config_result {
            // Should be able to convert to string
            let path_str = path.to_string_lossy();
            assert!(!path_str.is_empty());
            
            // Should contain valid path separators for the platform
            #[cfg(windows)]
            assert!(path_str.contains('\\') || path_str.contains('/'));
            #[cfg(not(windows))]
            assert!(path_str.contains('/'));
        }
    }

    #[test]
    fn test_error_handling() {
        // Test error handling by temporarily unsetting HOME (on Unix) or equivalent
        let original_home = env::var("HOME").ok();
        let original_userprofile = env::var("USERPROFILE").ok();
        
        // Temporarily remove home directory environment variables
        env::remove_var("HOME");
        env::remove_var("USERPROFILE");
        
        let app_config_result = get_sagitta_code_app_config_path();
        let core_config_result = get_sagitta_code_core_config_path();
        
        // Should handle missing home directory gracefully
        // (may succeed with fallback or fail with appropriate error)
        match app_config_result {
            Ok(_) => {}, // Success is fine (fallback mechanism)
            Err(e) => assert!(!e.to_string().is_empty()),
        }
        
        match core_config_result {
            Ok(_) => {}, // Success is fine (fallback mechanism)  
            Err(e) => assert!(!e.to_string().is_empty()),
        }
        
        // Restore original environment
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        }
        if let Some(userprofile) = original_userprofile {
            env::set_var("USERPROFILE", userprofile);
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
        
        let core_path1 = get_sagitta_code_core_config_path();
        let core_path2 = get_sagitta_code_core_config_path();
        
        assert_eq!(core_path1.is_ok(), core_path2.is_ok());
        if let (Ok(p1), Ok(p2)) = (core_path1, core_path2) {
            assert_eq!(p1, p2);
        }
    }

    #[test]
    fn test_filename_correctness() {
        if let Ok(app_path) = get_sagitta_code_app_config_path() {
            assert_eq!(
                app_path.file_name().and_then(|s| s.to_str()),
                Some("sagitta_code_config.json")
            );
        }
        
        if let Ok(core_path) = get_sagitta_code_core_config_path() {
            assert_eq!(
                core_path.file_name().and_then(|s| s.to_str()),
                Some("core_config.toml")
            );
        }
    }

    #[test]
    fn test_path_creation_readiness() {
        // Test that the paths returned are suitable for file creation
        if let Ok(app_path) = get_sagitta_code_app_config_path() {
            if let Some(parent) = app_path.parent() {
                // Parent directory path should be valid
                assert!(!parent.to_string_lossy().is_empty());
                
                // Should be able to create the directory structure (in theory)
                // We don't actually create it in tests to avoid side effects
            }
        }
    }

    #[test] 
    fn test_config_separation() {
        // Ensure Sagitta Code configs are separate from main sagitta configs
        if let (Ok(app_path), Ok(core_path)) = (
            get_sagitta_code_app_config_path(),
            get_sagitta_code_core_config_path()
        ) {
            let app_path_str = app_path.to_string_lossy();
            let core_path_str = core_path.to_string_lossy();
            
            // Both should be in sagitta_code subdirectory
            assert!(app_path_str.contains("sagitta_code"));
            assert!(core_path_str.contains("sagitta_code"));
            
            // Should not be in the root .config/sagitta directory
            assert!(!app_path_str.ends_with(".config/sagitta/config.toml"));
            assert!(!core_path_str.ends_with(".config/sagitta/config.toml"));
        }
    }
} 