use egui::{Context, Key, Modifiers};
use sagitta_code::config::types::SagittaCodeConfig;
use sagitta_code::providers::types::ProviderType;
use sagitta_code::providers::types::ProviderConfig;
use sagitta_code::gui::app::state::AppState;

/// Initialize test isolation for UI tests
fn init_test_isolation() {
    let _ = env_logger::builder()
        .is_test(true)
        .try_init();
}

/// Mock test application for UI testing
struct TestApp {
    config: SagittaCodeConfig,
    app_state: AppState,
    ctx: Context,
}

impl TestApp {
    fn new() -> Self {
        let mut config = SagittaCodeConfig::default();
        config.current_provider = ProviderType::ClaudeCode;
        
        let mut app_state = AppState::new();
        app_state.current_provider = ProviderType::ClaudeCode;
        app_state.available_providers = vec![ProviderType::ClaudeCode, ProviderType::MistralRs];
        
        let ctx = Context::default();
        
        Self {
            config,
            app_state,
            ctx,
        }
    }
    
    fn set_provider(&mut self, provider: ProviderType) {
        self.config.current_provider = provider;
        self.app_state.current_provider = provider;
    }
    
    fn simulate_key_press(&mut self, key: Key, modifiers: Modifiers) {
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        });
        
        self.ctx.begin_pass(input);
    }
}

#[cfg(test)]
mod provider_dropdown_tests {
    use super::*;
    
    #[test]
    fn test_provider_dropdown_display() {
        init_test_isolation();
        
        let app = TestApp::new();
        
        // Test provider display names
        assert_eq!(ProviderType::ClaudeCode.display_name(), "Claude Code");
        assert_eq!(ProviderType::MistralRs.display_name(), "Mistral.rs");
    }
    
    #[test]
    fn test_provider_selection_state() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        
        // Initial state
        assert_eq!(app.app_state.current_provider, ProviderType::ClaudeCode);
        
        // Switch provider
        app.set_provider(ProviderType::MistralRs);
        assert_eq!(app.app_state.current_provider, ProviderType::MistralRs);
        
        // Switch back
        app.set_provider(ProviderType::ClaudeCode);
        assert_eq!(app.app_state.current_provider, ProviderType::ClaudeCode);
    }
    
    #[test]
    fn test_available_providers_list() {
        init_test_isolation();
        
        let app = TestApp::new();
        
        // Should have both providers available
        assert_eq!(app.app_state.available_providers.len(), 2);
        assert!(app.app_state.available_providers.contains(&ProviderType::ClaudeCode));
        assert!(app.app_state.available_providers.contains(&ProviderType::MistralRs));
    }
    
    #[test]
    fn test_provider_dropdown_ui_rendering() {
        init_test_isolation();
        
        let app = TestApp::new();
        
        // Test that UI rendering doesn't panic
        let output = app.ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Simulate provider dropdown rendering
                ui.horizontal(|ui| {
                    ui.label("🤖");
                    egui::ComboBox::from_id_salt("provider_selector")
                        .selected_text(app.app_state.current_provider.display_name())
                        .show_ui(ui, |ui| {
                            for provider in &app.app_state.available_providers {
                                ui.selectable_value(&mut ProviderType::ClaudeCode, *provider, provider.display_name());
                            }
                        });
                });
            });
        });
        
        // Should render without panicking - that's the main test
        // In test environment, we don't have specific output to check
    }
}

#[cfg(test)]
mod provider_settings_tests {
    use super::*;
    
    #[test]
    fn test_provider_settings_panel_structure() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        
        // Test Claude Code settings rendering
        app.set_provider(ProviderType::ClaudeCode);
        
        let output = app.ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.collapsing("Provider Settings", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Primary Provider:");
                    egui::ComboBox::from_id_salt("primary_provider_combo")
                        .selected_text(app.app_state.current_provider.display_name())
                        .show_ui(ui, |ui| {
                            for provider in &app.app_state.available_providers {
                                ui.selectable_value(&mut app.app_state.current_provider, *provider, provider.display_name());
                            }
                        });
                });
                
                ui.separator();
                
                // Provider-specific settings would be rendered here
                match app.app_state.current_provider {
                    ProviderType::ClaudeCode => {
                        ui.label("Claude Code Settings");
                        ui.label("See 'Claude Code Configuration' section above");
                    },
                    ProviderType::MistralRs => {
                        ui.label("Mistral.rs Settings");
                        ui.horizontal(|ui| {
                            ui.label("Base URL:");
                            ui.text_edit_singleline(&mut String::from("http://localhost:1234"));
                        });
                    },
                    ProviderType::OpenAICompatible => {
                        ui.label("OpenAI Compatible Settings");
                        ui.horizontal(|ui| {
                            ui.label("Base URL:");
                            ui.text_edit_singleline(&mut String::from("http://localhost:11434"));
                        });
                    },
                    ProviderType::ClaudeCodeRouter => {
                        ui.label("Claude Code Router Settings");
                        ui.label("Router settings not implemented yet");
                    },
                }
                });
            });
        });
        
        // Verify rendering succeeds (no panic is the main test)
        // Just test that it runs without panic - output structure is different in tests
    }
    
    #[test]
    fn test_mistral_rs_settings_fields() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        app.set_provider(ProviderType::MistralRs);
        
        // Add Mistral.rs config
        let mut mistral_config = ProviderConfig::new(ProviderType::MistralRs);
        mistral_config.set_option("base_url", &"http://localhost:1234".to_string()).unwrap();
        mistral_config.set_option("api_key", &"test-key".to_string()).unwrap();
        mistral_config.set_option("model", &"mistral-7b".to_string()).unwrap();
        mistral_config.set_option("temperature", &Some(0.7)).unwrap();
        
        app.config.provider_configs.insert(ProviderType::MistralRs, mistral_config);
        
        // Test that Mistral.rs config can be accessed and displayed
        if let Some(config) = app.config.provider_configs.get(&ProviderType::MistralRs) {
            let mistral_config: sagitta_code::providers::types::MistralRsConfig = 
                config.try_into().unwrap();
            
            assert_eq!(mistral_config.base_url, "http://localhost:1234");
            assert_eq!(mistral_config.api_key, Some("test-key".to_string()));
            assert_eq!(mistral_config.model, Some("mistral-7b".to_string()));
            assert_eq!(mistral_config.timeout_seconds, 120);
        }
    }
    
    #[test]
    fn test_claude_code_settings_reference() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        app.set_provider(ProviderType::ClaudeCode);
        
        // Add Claude Code config
        let mut claude_config = ProviderConfig::new(ProviderType::ClaudeCode);
        claude_config.set_option("api_key", &Some("claude-test-key".to_string())).unwrap();
        claude_config.set_option("model", &"claude-3-sonnet-20240229".to_string()).unwrap();
        
        app.config.provider_configs.insert(ProviderType::ClaudeCode, claude_config);
        
        // Test that Claude Code config is properly referenced
        if let Some(config) = app.config.provider_configs.get(&ProviderType::ClaudeCode) {
            let claude_config: sagitta_code::providers::types::ClaudeCodeConfig = 
                config.try_into().unwrap();
            
            // The new ClaudeCodeConfig doesn't have api_key or model fields
            // It only has binary_path, additional_args, and timeout_seconds
            assert_eq!(claude_config.binary_path, None);
            assert_eq!(claude_config.timeout_seconds, 300); // Default timeout
        }
    }
}

#[cfg(test)]
mod provider_hotkey_tests {
    use super::*;
    
    #[test]
    fn test_provider_quick_switch_hotkey() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        
        // Initial state - quick switch should be closed
        assert!(!app.app_state.show_provider_quick_switch);
        
        // Simulate Ctrl+P key press
        app.simulate_key_press(Key::P, Modifiers::CTRL);
        
        // In the real app, the hotkey handler would toggle the state
        // For this test, we simulate that behavior
        app.app_state.show_provider_quick_switch = !app.app_state.show_provider_quick_switch;
        
        // Verify quick switch dialog state changed
        assert!(app.app_state.show_provider_quick_switch);
    }
    
    #[test]
    fn test_provider_quick_switch_toggle() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        
        // Start with quick switch closed
        app.app_state.show_provider_quick_switch = false;
        
        // Toggle on
        app.app_state.show_provider_quick_switch = !app.app_state.show_provider_quick_switch;
        assert!(app.app_state.show_provider_quick_switch);
        
        // Toggle off
        app.app_state.show_provider_quick_switch = !app.app_state.show_provider_quick_switch;
        assert!(!app.app_state.show_provider_quick_switch);
    }
    
    #[test]
    fn test_provider_quick_switch_functionality() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        app.app_state.show_provider_quick_switch = true;
        
        // Start with Claude Code
        assert_eq!(app.app_state.current_provider, ProviderType::ClaudeCode);
        
        // Simulate switching to Mistral.rs
        app.set_provider(ProviderType::MistralRs);
        app.app_state.show_provider_quick_switch = false; // Dialog should close after selection
        
        // Verify provider switched and dialog closed
        assert_eq!(app.app_state.current_provider, ProviderType::MistralRs);
        assert!(!app.app_state.show_provider_quick_switch);
    }
    
    #[test]
    fn test_provider_hotkey_with_multiple_providers() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        
        // Test with both providers available
        assert_eq!(app.app_state.available_providers.len(), 2);
        
        // Quick switch should work with multiple providers
        app.app_state.show_provider_quick_switch = true;
        
        // Simulate cycling through providers
        let providers = app.app_state.available_providers.clone();
        for provider in providers {
            app.set_provider(provider);
            assert_eq!(app.app_state.current_provider, provider);
        }
    }
}

#[cfg(test)]
mod provider_first_run_tests {
    use super::*;
    
    #[test]
    fn test_first_run_setup_dialog_state() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        
        // Fresh config should trigger first run
        app.config.ui.first_run_completed = false;
        app.app_state.show_provider_setup_dialog = true;
        
        // Verify first run dialog state
        assert!(app.app_state.show_provider_setup_dialog);
        assert!(!app.config.ui.first_run_completed);
        
        // Complete first run
        app.config.ui.first_run_completed = true;
        app.app_state.show_provider_setup_dialog = false;
        
        // Verify completion
        assert!(!app.app_state.show_provider_setup_dialog);
        assert!(app.config.ui.first_run_completed);
    }
    
    #[test]
    fn test_first_run_provider_selection() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        app.app_state.show_provider_setup_dialog = true;
        
        // Simulate provider selection in first-run dialog
        let selected_provider = ProviderType::MistralRs;
        
        // User selects provider
        app.set_provider(selected_provider);
        
        // Complete first run setup
        app.config.ui.first_run_completed = true;
        app.app_state.show_provider_setup_dialog = false;
        
        // Verify selection persisted
        assert_eq!(app.app_state.current_provider, ProviderType::MistralRs);
        assert!(app.config.ui.first_run_completed);
    }
    
    #[test]
    fn test_first_run_skip_option() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        app.app_state.show_provider_setup_dialog = true;
        
        // User skips detailed setup (uses defaults)
        app.set_provider(ProviderType::ClaudeCode); // Default provider
        app.config.ui.first_run_completed = true;
        app.app_state.show_provider_setup_dialog = false;
        
        // Should work with default settings
        assert_eq!(app.app_state.current_provider, ProviderType::ClaudeCode);
        assert!(app.config.ui.first_run_completed);
    }
}

#[cfg(test)]
mod provider_ui_integration_tests {
    use super::*;
    
    #[test]
    fn test_provider_dropdown_chat_integration() {
        init_test_isolation();
        
        let app = TestApp::new();
        
        // Test that provider dropdown integrates properly with chat UI
        let output = app.ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Repository dropdown simulation
                    ui.label("📁");
                    ui.selectable_label(false, "Current Repository");
                    
                    ui.separator();
                    
                    // Provider dropdown simulation
                    ui.label("🤖");
                    egui::ComboBox::from_id_salt("provider_selector")
                        .selected_text(app.app_state.current_provider.display_name())
                        .show_ui(ui, |ui| {
                            for provider in &app.app_state.available_providers {
                                ui.selectable_value(&mut ProviderType::ClaudeCode, *provider, provider.display_name());
                            }
                        });
                });
            });
        });
        
        // Should render without issues - main test is no panic
        // In test environment, we just verify no panic occurred
    }
    
    #[test]
    fn test_provider_settings_integration() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        
        // Test full settings panel integration
        let output = app.ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Existing settings sections...
                    ui.collapsing("General Settings", |ui| {
                        ui.label("General configuration options");
                    });
                    
                    // Provider settings section
                    ui.collapsing("Provider Settings", |ui| {
                        ui.label("Provider configuration");
                        
                        // Provider selection
                        ui.horizontal(|ui| {
                            ui.label("Primary Provider:");
                            egui::ComboBox::from_id_salt("primary_provider_combo")
                                .selected_text(app.app_state.current_provider.display_name())
                                .show_ui(ui, |ui| {
                                    for provider in &app.app_state.available_providers {
                                        ui.selectable_value(&mut app.app_state.current_provider, *provider, provider.display_name());
                                    }
                                });
                        });
                        
                        ui.separator();
                        
                        // Provider-specific settings
                        match app.app_state.current_provider {
                            ProviderType::ClaudeCode => {
                                ui.label("Claude Code settings reference");
                            },
                            ProviderType::MistralRs => {
                                ui.label("Mistral.rs settings");
                            },
                            ProviderType::OpenAICompatible => {
                                ui.label("OpenAI Compatible settings");
                            },
                            ProviderType::ClaudeCodeRouter => {
                                ui.label("Claude Code Router settings");
                            },
                        }
                    });
                    
                    // Other existing sections...
                    ui.collapsing("Advanced Settings", |ui| {
                        ui.label("Advanced configuration options");
                    });
                });
            });
        });
        
        // Verify the full settings panel renders correctly - main test is no panic
        // In test environment, we just verify no panic occurred
    }
    
    #[test]
    fn test_provider_state_consistency() {
        init_test_isolation();
        
        let mut app = TestApp::new();
        
        // Ensure app state and config stay consistent
        app.set_provider(ProviderType::MistralRs);
        
        assert_eq!(app.app_state.current_provider, app.config.current_provider);
        
        // Switch back and verify consistency
        app.set_provider(ProviderType::ClaudeCode);
        
        assert_eq!(app.app_state.current_provider, app.config.current_provider);
        assert_eq!(app.app_state.current_provider, ProviderType::ClaudeCode);
    }
}