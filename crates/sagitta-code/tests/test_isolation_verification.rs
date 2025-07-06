mod common;

// Initialize test isolation for this integration test binary
#[ctor::ctor]
fn init_test_isolation() {
    common::init_test_isolation();
}

use std::env;
use std::fs;
use sagitta_code::config::{SagittaCodeConfig, save_config};

#[test]
fn test_save_config_uses_test_isolation() {
    // Verify that SAGITTA_TEST_CONFIG_PATH is set by our test isolation
    let test_config_path = env::var("SAGITTA_TEST_CONFIG_PATH")
        .expect("SAGITTA_TEST_CONFIG_PATH should be set by test isolation");
    
    println!("Test config path: {test_config_path}");
    
    // Create a test config
    let test_config = SagittaCodeConfig::default();
    
    // Save the config - this should use the test path
    save_config(&test_config).expect("Should save config successfully");
    
    // Verify that the config was saved to the test directory, not the user's real config
    // The test config path points to a file, so we need to get the parent directory
    let test_config_dir = std::path::Path::new(&test_config_path).parent()
        .expect("Test config path should have a parent directory");
    let expected_config_file = test_config_dir.join("sagitta_code_config.toml");
    assert!(expected_config_file.exists(), 
            "Config should be saved to test directory: {}", expected_config_file.display());
    
    // Verify that the user's real config directory is not affected
    let real_config_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("sagitta")
        .join("sagitta_code_config.toml");
    
    // If the real config exists, make sure it wasn't modified during this test
    if real_config_path.exists() {
        let real_config_metadata = fs::metadata(&real_config_path).unwrap();
        let test_start_time = std::time::SystemTime::now();
        
        // The real config should not have been modified recently (within the last few seconds)
        let time_since_modification = test_start_time
            .duration_since(real_config_metadata.modified().unwrap())
            .unwrap();
        
        assert!(time_since_modification.as_secs() > 5, 
                "Real config file should not have been modified during test");
    }
}

#[tokio::test]
async fn test_async_save_config_uses_test_isolation() {
    // This test verifies that async code paths also use test isolation
    let test_config_path = env::var("SAGITTA_TEST_CONFIG_PATH")
        .expect("SAGITTA_TEST_CONFIG_PATH should be set by test isolation");
    
    println!("Async test config path: {test_config_path}");
    
    // Create a test config
    let test_config = SagittaCodeConfig::default();
    
    // Spawn an async task that saves config (similar to what the GUI code does)
    let config_clone = test_config.clone();
    let save_task = tokio::spawn(async move {
        save_config(&config_clone)
    });
    
    // Wait for the task to complete
    save_task.await.unwrap().expect("Should save config successfully");
    
    // Verify that the config was saved to the test directory
    let test_config_dir = std::path::Path::new(&test_config_path).parent()
        .expect("Test config path should have a parent directory");
    let expected_config_file = test_config_dir.join("sagitta_code_config.toml");
    assert!(expected_config_file.exists(), 
            "Config should be saved to test directory: {}", expected_config_file.display());
} 