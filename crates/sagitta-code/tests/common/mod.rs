use std::sync::Once;
use tempfile::TempDir;
use std::path::PathBuf;

static INIT: Once = Once::new();
static mut TEST_CONFIG_DIR: Option<TempDir> = None;

/// Initialize test isolation by setting SAGITTA_TEST_CONFIG_PATH to a temporary directory.
/// This ensures that all config operations during tests use a temporary location instead
/// of the user's real config directory.
pub fn init_test_isolation() {
    INIT.call_once(|| {
        unsafe {
            let temp_dir = TempDir::new().expect("Failed to create temporary directory for test isolation");
            let config_path = temp_dir.path().join("config.toml");
            
            // Set the environment variable that sagitta-search config system respects
            std::env::set_var("SAGITTA_TEST_CONFIG_PATH", config_path.to_str().unwrap());
            
            // Store the temp_dir to keep it alive for the duration of the test run
            TEST_CONFIG_DIR = Some(temp_dir);
            
            log::debug!("Test isolation initialized: config redirected to {}", config_path.display());
        }
    });
}

/// Get the test config path if test isolation is active
pub fn get_test_config_path() -> Option<PathBuf> {
    std::env::var("SAGITTA_TEST_CONFIG_PATH")
        .ok()
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_sets_env_var() {
        init_test_isolation();
        
        let config_path = std::env::var("SAGITTA_TEST_CONFIG_PATH");
        assert!(config_path.is_ok(), "SAGITTA_TEST_CONFIG_PATH should be set after init_test_isolation");
        
        let path = PathBuf::from(config_path.unwrap());
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert!(filename == "config.toml" || filename == "dummy.toml", 
                "Config path should end with config.toml or dummy.toml, got: {filename}");
    }

    #[test]
    fn test_get_test_config_path() {
        init_test_isolation();
        
        let path = get_test_config_path();
        assert!(path.is_some(), "Should return test config path when isolation is active");
        
        let path = path.unwrap();
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert!(filename == "config.toml" || filename == "dummy.toml", 
                "Should point to config.toml or dummy.toml, got: {filename}");
    }
} 