use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;
use serde_json;

use sagitta_code::gui::theme::{AppTheme, CustomThemeColors, set_custom_theme_colors, get_custom_theme_colors};
use sagitta_code::config::types::{SagittaCodeConfig, UiConfig};
use sagitta_code::config::{save_config, load_config_from_path};

mod common;

/// Test theme persistence across app restarts
#[tokio::test]
async fn test_theme_persistence_across_restarts() {
    common::init_test_isolation();
    
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("sagitta_code_config.toml");
    
    // Create custom theme colors
    let custom_colors = CustomThemeColors {
        panel_background: egui::Color32::from_rgb(50, 50, 50),
        text_color: egui::Color32::from_rgb(200, 200, 200),
        accent_color: egui::Color32::from_rgb(100, 150, 255),
        success_color: egui::Color32::from_rgb(0, 255, 0),
        error_color: egui::Color32::from_rgb(255, 0, 0),
        ..Default::default()
    };
    
    // Set up initial config with custom theme
    let mut config = SagittaCodeConfig::default();
    config.ui.theme = "custom".to_string();
    config.ui.custom_theme_path = Some(temp_dir.path().join("my_theme.sagitta-theme.json"));
    
    // Save the config
    std::env::set_var("SAGITTA_TEST_CONFIG_PATH", config_path.to_string_lossy().to_string());
    save_config(&config).expect("Should save config");
    
    // Save the custom theme colors to the theme file
    let theme_json = serde_json::to_string_pretty(&custom_colors).expect("Should serialize theme");
    fs::write(&config.ui.custom_theme_path.as_ref().unwrap(), theme_json).await.expect("Should write theme file");
    
    // Simulate app restart - load config from file
    let loaded_config = load_config_from_path(&config_path).expect("Should load config");
    
    // Verify theme settings persisted
    assert_eq!(loaded_config.ui.theme, "custom");
    assert!(loaded_config.ui.custom_theme_path.is_some());
    
    // Load and verify custom theme colors
    let theme_file_content = fs::read_to_string(loaded_config.ui.custom_theme_path.as_ref().unwrap()).await.expect("Should read theme file");
    let loaded_colors: CustomThemeColors = serde_json::from_str(&theme_file_content).expect("Should deserialize theme");
    
    assert_eq!(loaded_colors.panel_background, custom_colors.panel_background);
    assert_eq!(loaded_colors.text_color, custom_colors.text_color);
    assert_eq!(loaded_colors.accent_color, custom_colors.accent_color);
}

/// Test theme export functionality
#[tokio::test]
async fn test_theme_export() {
    let temp_dir = TempDir::new().unwrap();
    let export_path = temp_dir.path().join("exported_theme.sagitta-theme.json");
    
    // Create custom theme colors
    let custom_colors = CustomThemeColors {
        panel_background: egui::Color32::from_rgb(30, 30, 30),
        text_color: egui::Color32::from_rgb(220, 220, 220),
        accent_color: egui::Color32::from_rgb(255, 100, 100),
        success_color: egui::Color32::from_rgb(100, 255, 100),
        error_color: egui::Color32::from_rgb(255, 50, 50),
        button_background: egui::Color32::from_rgb(60, 60, 60),
        ..Default::default()
    };
    
    // Export theme to JSON
    let theme_json = serde_json::to_string_pretty(&custom_colors).expect("Should serialize theme");
    fs::write(&export_path, theme_json).await.expect("Should write exported theme");
    
    // Verify file was created and contains expected data
    assert!(export_path.exists());
    let file_content = fs::read_to_string(&export_path).await.expect("Should read exported file");
    let parsed_colors: CustomThemeColors = serde_json::from_str(&file_content).expect("Should parse exported theme");
    
    assert_eq!(parsed_colors.panel_background, custom_colors.panel_background);
    assert_eq!(parsed_colors.text_color, custom_colors.text_color);
    assert_eq!(parsed_colors.accent_color, custom_colors.accent_color);
    assert_eq!(parsed_colors.success_color, custom_colors.success_color);
    assert_eq!(parsed_colors.error_color, custom_colors.error_color);
    assert_eq!(parsed_colors.button_background, custom_colors.button_background);
}

/// Test theme import functionality
#[tokio::test]
async fn test_theme_import() {
    let temp_dir = TempDir::new().unwrap();
    let import_path = temp_dir.path().join("import_theme.sagitta-theme.json");
    
    // Create a theme file to import
    let original_colors = CustomThemeColors {
        panel_background: egui::Color32::from_rgb(40, 40, 40),
        text_color: egui::Color32::from_rgb(240, 240, 240),
        accent_color: egui::Color32::from_rgb(0, 200, 255),
        success_color: egui::Color32::from_rgb(50, 255, 50),
        error_color: egui::Color32::from_rgb(255, 100, 100),
        border_color: egui::Color32::from_rgb(80, 80, 80),
        ..Default::default()
    };
    
    let theme_json = serde_json::to_string_pretty(&original_colors).expect("Should serialize theme");
    fs::write(&import_path, theme_json).await.expect("Should write theme file");
    
    // Import theme from JSON
    let file_content = fs::read_to_string(&import_path).await.expect("Should read theme file");
    let imported_colors: CustomThemeColors = serde_json::from_str(&file_content).expect("Should parse theme");
    
    // Verify imported colors match original
    assert_eq!(imported_colors.panel_background, original_colors.panel_background);
    assert_eq!(imported_colors.text_color, original_colors.text_color);
    assert_eq!(imported_colors.accent_color, original_colors.accent_color);
    assert_eq!(imported_colors.success_color, original_colors.success_color);
    assert_eq!(imported_colors.error_color, original_colors.error_color);
    assert_eq!(imported_colors.border_color, original_colors.border_color);
}

/// Test export/import round-trip
#[tokio::test]
async fn test_theme_export_import_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let roundtrip_path = temp_dir.path().join("roundtrip_theme.sagitta-theme.json");
    
    // Create original theme with all colors set
    let original_colors = CustomThemeColors {
        panel_background: egui::Color32::from_rgb(25, 25, 25),
        input_background: egui::Color32::from_rgb(35, 35, 35),
        button_background: egui::Color32::from_rgb(55, 55, 55),
        code_background: egui::Color32::from_rgb(30, 30, 30),
        thinking_background: egui::Color32::from_rgb(40, 40, 40),
        
        text_color: egui::Color32::from_rgb(210, 210, 210),
        hint_text_color: egui::Color32::from_rgb(120, 120, 120),
        code_text_color: egui::Color32::from_rgb(190, 190, 190),
        thinking_text_color: egui::Color32::from_rgb(170, 170, 170),
        timestamp_color: egui::Color32::from_rgb(110, 110, 110),
        
        accent_color: egui::Color32::from_rgb(90, 140, 230),
        success_color: egui::Color32::from_rgb(40, 200, 40),
        warning_color: egui::Color32::from_rgb(250, 210, 0),
        error_color: egui::Color32::from_rgb(250, 60, 0),
        
        border_color: egui::Color32::from_rgb(50, 50, 50),
        focus_border_color: egui::Color32::from_rgb(90, 140, 230),
        
        button_hover_color: egui::Color32::from_rgb(70, 70, 70),
        button_disabled_color: egui::Color32::from_rgb(50, 50, 50),
        button_text_color: egui::Color32::WHITE,
        button_disabled_text_color: egui::Color32::from_rgb(170, 170, 170),
        
        user_color: egui::Color32::from_rgb(250, 250, 250),
        agent_color: egui::Color32::from_rgb(0, 250, 0),
        system_color: egui::Color32::from_rgb(250, 0, 0),
        tool_color: egui::Color32::from_rgb(250, 250, 0),
        
        streaming_color: egui::Color32::from_rgb(140, 250, 140),
        thinking_indicator_color: egui::Color32::from_rgb(90, 140, 250),
        complete_color: egui::Color32::from_rgb(90, 250, 90),
        
        diff_added_bg: egui::Color32::from_rgb(0, 70, 0),
        diff_removed_bg: egui::Color32::from_rgb(70, 0, 0),
        diff_added_text: egui::Color32::from_rgb(90, 190, 90),
        diff_removed_text: egui::Color32::from_rgb(190, 90, 90),
    };
    
    // Export to JSON
    let exported_json = serde_json::to_string_pretty(&original_colors).expect("Should export theme");
    fs::write(&roundtrip_path, &exported_json).await.expect("Should write exported theme");
    
    // Import from JSON
    let imported_json = fs::read_to_string(&roundtrip_path).await.expect("Should read theme file");
    let imported_colors: CustomThemeColors = serde_json::from_str(&imported_json).expect("Should import theme");
    
    // Verify all colors survived the round-trip
    assert_eq!(imported_colors.panel_background, original_colors.panel_background);
    assert_eq!(imported_colors.input_background, original_colors.input_background);
    assert_eq!(imported_colors.button_background, original_colors.button_background);
    assert_eq!(imported_colors.code_background, original_colors.code_background);
    assert_eq!(imported_colors.thinking_background, original_colors.thinking_background);
    
    assert_eq!(imported_colors.text_color, original_colors.text_color);
    assert_eq!(imported_colors.hint_text_color, original_colors.hint_text_color);
    assert_eq!(imported_colors.code_text_color, original_colors.code_text_color);
    assert_eq!(imported_colors.thinking_text_color, original_colors.thinking_text_color);
    assert_eq!(imported_colors.timestamp_color, original_colors.timestamp_color);
    
    assert_eq!(imported_colors.accent_color, original_colors.accent_color);
    assert_eq!(imported_colors.success_color, original_colors.success_color);
    assert_eq!(imported_colors.warning_color, original_colors.warning_color);
    assert_eq!(imported_colors.error_color, original_colors.error_color);
    
    assert_eq!(imported_colors.border_color, original_colors.border_color);
    assert_eq!(imported_colors.focus_border_color, original_colors.focus_border_color);
    
    assert_eq!(imported_colors.button_hover_color, original_colors.button_hover_color);
    assert_eq!(imported_colors.button_disabled_color, original_colors.button_disabled_color);
    assert_eq!(imported_colors.button_text_color, original_colors.button_text_color);
    assert_eq!(imported_colors.button_disabled_text_color, original_colors.button_disabled_text_color);
    
    assert_eq!(imported_colors.user_color, original_colors.user_color);
    assert_eq!(imported_colors.agent_color, original_colors.agent_color);
    assert_eq!(imported_colors.system_color, original_colors.system_color);
    assert_eq!(imported_colors.tool_color, original_colors.tool_color);
    
    assert_eq!(imported_colors.streaming_color, original_colors.streaming_color);
    assert_eq!(imported_colors.thinking_indicator_color, original_colors.thinking_indicator_color);
    assert_eq!(imported_colors.complete_color, original_colors.complete_color);
    
    assert_eq!(imported_colors.diff_added_bg, original_colors.diff_added_bg);
    assert_eq!(imported_colors.diff_removed_bg, original_colors.diff_removed_bg);
    assert_eq!(imported_colors.diff_added_text, original_colors.diff_added_text);
    assert_eq!(imported_colors.diff_removed_text, original_colors.diff_removed_text);
}

/// Test automatic theme saving when user changes theme
#[tokio::test]
async fn test_automatic_theme_saving() {
    common::init_test_isolation();
    
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("sagitta_code_config.toml");
    
    // Set up test environment
    std::env::set_var("SAGITTA_TEST_CONFIG_PATH", config_path.to_string_lossy().to_string());
    
    // Create initial config
    let mut config = SagittaCodeConfig::default();
    config.ui.theme = "dark".to_string();
    save_config(&config).expect("Should save initial config");
    
    // Simulate user changing theme to custom
    config.ui.theme = "custom".to_string();
    config.ui.custom_theme_path = Some(temp_dir.path().join("auto_saved_theme.sagitta-theme.json"));
    
    // Save config (simulating automatic save)
    save_config(&config).expect("Should save updated config");
    
    // Verify config was saved
    let loaded_config = load_config_from_path(&config_path).expect("Should load saved config");
    assert_eq!(loaded_config.ui.theme, "custom");
    assert!(loaded_config.ui.custom_theme_path.is_some());
}

/// Test loading custom theme on startup
#[tokio::test]
async fn test_custom_theme_loading_on_startup() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("sagitta_code_config.toml");
    let theme_path = temp_dir.path().join("startup_theme.sagitta-theme.json");
    
    // Create custom theme colors
    let custom_colors = CustomThemeColors {
        panel_background: egui::Color32::from_rgb(60, 60, 60),
        text_color: egui::Color32::from_rgb(180, 180, 180),
        accent_color: egui::Color32::from_rgb(120, 180, 255),
        ..Default::default()
    };
    
    // Save theme file
    let theme_json = serde_json::to_string_pretty(&custom_colors).expect("Should serialize theme");
    fs::write(&theme_path, theme_json).await.expect("Should write theme file");
    
    // Create config pointing to the theme file
    let mut config = SagittaCodeConfig::default();
    config.ui.theme = "custom".to_string();
    config.ui.custom_theme_path = Some(theme_path.clone());
    
    std::env::set_var("SAGITTA_TEST_CONFIG_PATH", config_path.to_string_lossy().to_string());
    save_config(&config).expect("Should save config");
    
    // Simulate app startup - load config and theme
    let loaded_config = load_config_from_path(&config_path).expect("Should load config");
    assert_eq!(loaded_config.ui.theme, "custom");
    
    if let Some(theme_file_path) = &loaded_config.ui.custom_theme_path {
        let theme_content = fs::read_to_string(theme_file_path).await.expect("Should read theme file");
        let loaded_colors: CustomThemeColors = serde_json::from_str(&theme_content).expect("Should parse theme");
        
        // Verify theme was loaded correctly
        assert_eq!(loaded_colors.panel_background, custom_colors.panel_background);
        assert_eq!(loaded_colors.text_color, custom_colors.text_color);
        assert_eq!(loaded_colors.accent_color, custom_colors.accent_color);
    } else {
        panic!("Custom theme path should be present");
    }
}

/// Test theme file validation
#[tokio::test]
async fn test_theme_file_validation() {
    let temp_dir = TempDir::new().unwrap();
    
    // Test valid theme file
    let valid_theme_path = temp_dir.path().join("valid_theme.sagitta-theme.json");
    let valid_colors = CustomThemeColors::default();
    let valid_json = serde_json::to_string_pretty(&valid_colors).expect("Should serialize");
    fs::write(&valid_theme_path, valid_json).await.expect("Should write valid theme");
    
    // Should parse successfully
    let content = fs::read_to_string(&valid_theme_path).await.expect("Should read file");
    let parsed: Result<CustomThemeColors, _> = serde_json::from_str(&content);
    assert!(parsed.is_ok(), "Valid theme file should parse successfully");
    
    // Test invalid theme file
    let invalid_theme_path = temp_dir.path().join("invalid_theme.sagitta-theme.json");
    let invalid_json = r#"{"invalid": "json", "missing": "required_fields"}"#;
    fs::write(&invalid_theme_path, invalid_json).await.expect("Should write invalid theme");
    
    // Should fail to parse
    let invalid_content = fs::read_to_string(&invalid_theme_path).await.expect("Should read file");
    let invalid_parsed: Result<CustomThemeColors, _> = serde_json::from_str(&invalid_content);
    assert!(invalid_parsed.is_err(), "Invalid theme file should fail to parse");
}

/// Test theme file extension validation
#[test]
fn test_theme_file_extension() {
    let valid_extensions = [
        "my_theme.sagitta-theme.json",
        "dark_theme.sagitta-theme.json",
        "custom.sagitta-theme.json",
    ];
    
    for filename in &valid_extensions {
        assert!(filename.ends_with(".sagitta-theme.json"), "Should have correct extension: {}", filename);
    }
    
    let invalid_extensions = [
        "theme.json",
        "theme.txt",
        "theme.sagitta",
        "theme.sagitta-theme",
    ];
    
    for filename in &invalid_extensions {
        assert!(!filename.ends_with(".sagitta-theme.json"), "Should not have correct extension: {}", filename);
    }
} 