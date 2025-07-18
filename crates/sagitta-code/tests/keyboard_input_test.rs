// Tests for keyboard input handling in chat interface

#[cfg(test)]
mod tests {
    use sagitta_code::gui::chat::input::chat_input_ui;
    use sagitta_code::gui::theme::AppTheme;
    use sagitta_code::gui::repository::manager::RepositoryManager;
    use sagitta_code::gui::repository::git_controls::GitControls;
    use sagitta_search::config::AppConfig;
    use egui::{Context, Key, Modifiers, Event};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    
    #[test]
    fn test_ctrl_enter_newline_behavior() {
        let ctx = Context::default();
        let config = AppConfig::default();
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(Arc::new(Mutex::new(config)))));
        
        ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut input_buffer = String::from("Hello");
                let mut on_submit = false;
                let is_waiting = false;
                let theme = AppTheme::default();
                let mut show_hotkeys = false;
                let repo_context = None;
                let repos = vec![];
                let mut repo_change = None;
                let mut repo_refresh = false;
                let mut git_controls = GitControls::new(repo_manager.clone());
                let is_in_loop = false;
                let mut loop_break = false;
                let mut loop_inject_buffer = String::new();
                let mut show_loop_inject = false;
                let mut loop_inject_msg = None;
                let mut should_focus = false;
                let token_usage = None;
                let mut stop_requested = false;
                let mut tool_cards_collapsed = false;
                let mut tool_card_individual_states = std::collections::HashMap::new();
                
                // Initial state
                let initial_buffer = input_buffer.clone();
                
                // Add Ctrl+Enter event to the context before calling chat_input_ui
                ui.input_mut(|input| {
                    input.events.push(Event::Key {
                        key: Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers { ctrl: true, ..Default::default() },
                    });
                });
                
                chat_input_ui(
                    ui,
                    &mut input_buffer,
                    &mut on_submit,
                    is_waiting,
                    theme,
                    &mut show_hotkeys,
                    &repo_context,
                    &repos,
                    &mut repo_change,
                    &mut repo_refresh,
                    &sagitta_code::providers::types::ProviderType::ClaudeCode,
                    &[sagitta_code::providers::types::ProviderType::ClaudeCode],
                    &mut None,
                    &mut git_controls,
                    is_in_loop,
                    &mut loop_break,
                    &mut loop_inject_buffer,
                    &mut show_loop_inject,
                    &mut loop_inject_msg,
                    &mut should_focus,
                    &token_usage,
                    &mut stop_requested,
                    &mut tool_cards_collapsed,
                    &mut tool_card_individual_states,
                );
                
                // Verify that Ctrl+Enter added a newline but didn't submit
                assert!(input_buffer.contains('\n'), "Ctrl+Enter should add a newline");
                assert!(input_buffer.len() > initial_buffer.len(), "Buffer should be longer after Ctrl+Enter");
                assert!(!on_submit, "Ctrl+Enter should not trigger submission");
            });
        });
    }
    
    #[test]
    fn test_enter_without_ctrl_submits() {
        let ctx = Context::default();
        let config = AppConfig::default();
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(Arc::new(Mutex::new(config)))));
        
        ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut input_buffer = String::from("Hello world");
                let mut on_submit = false;
                let is_waiting = false;
                let theme = AppTheme::default();
                let mut show_hotkeys = false;
                let repo_context = None;
                let repos = vec![];
                let mut repo_change = None;
                let mut repo_refresh = false;
                let mut git_controls = GitControls::new(repo_manager.clone());
                let is_in_loop = false;
                let mut loop_break = false;
                let mut loop_inject_buffer = String::new();
                let mut show_loop_inject = false;
                let mut loop_inject_msg = None;
                let mut should_focus = false;
                let token_usage = None;
                let mut stop_requested = false;
                let mut tool_cards_collapsed = false;
                let mut tool_card_individual_states = std::collections::HashMap::new();
                
                // Add Enter event without modifiers to the context
                ui.input_mut(|input| {
                    input.events.push(Event::Key {
                        key: Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::default(),
                    });
                });
                
                // Note: This test can't easily verify submission behavior in a unit test
                // because egui's focus system requires a real UI context
                // But we can at least verify the code compiles and runs
                chat_input_ui(
                    ui,
                    &mut input_buffer,
                    &mut on_submit,
                    is_waiting,
                    theme,
                    &mut show_hotkeys,
                    &repo_context,
                    &repos,
                    &mut repo_change,
                    &mut repo_refresh,
                    &sagitta_code::providers::types::ProviderType::ClaudeCode,
                    &[sagitta_code::providers::types::ProviderType::ClaudeCode],
                    &mut None,
                    &mut git_controls,
                    is_in_loop,
                    &mut loop_break,
                    &mut loop_inject_buffer,
                    &mut show_loop_inject,
                    &mut loop_inject_msg,
                    &mut should_focus,
                    &token_usage,
                    &mut stop_requested,
                    &mut tool_cards_collapsed,
                    &mut tool_card_individual_states,
                );
                
                // In a real scenario with focus, this would be true, but in unit tests
                // we can't easily simulate the focus behavior
            });
        });
    }
    
    #[test]
    fn test_empty_buffer_no_submit() {
        let ctx = Context::default();
        let config = AppConfig::default();
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(Arc::new(Mutex::new(config)))));
        
        ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut input_buffer = String::new(); // Empty buffer
                let mut on_submit = false;
                let is_waiting = false;
                let theme = AppTheme::default();
                let mut show_hotkeys = false;
                let repo_context = None;
                let repos = vec![];
                let mut repo_change = None;
                let mut repo_refresh = false;
                let mut git_controls = GitControls::new(repo_manager.clone());
                let is_in_loop = false;
                let mut loop_break = false;
                let mut loop_inject_buffer = String::new();
                let mut show_loop_inject = false;
                let mut loop_inject_msg = None;
                let mut should_focus = false;
                let token_usage = None;
                let mut stop_requested = false;
                let mut tool_cards_collapsed = false;
                let mut tool_card_individual_states = std::collections::HashMap::new();
                
                chat_input_ui(
                    ui,
                    &mut input_buffer,
                    &mut on_submit,
                    is_waiting,
                    theme,
                    &mut show_hotkeys,
                    &repo_context,
                    &repos,
                    &mut repo_change,
                    &mut repo_refresh,
                    &sagitta_code::providers::types::ProviderType::ClaudeCode,
                    &[sagitta_code::providers::types::ProviderType::ClaudeCode],
                    &mut None,
                    &mut git_controls,
                    is_in_loop,
                    &mut loop_break,
                    &mut loop_inject_buffer,
                    &mut show_loop_inject,
                    &mut loop_inject_msg,
                    &mut should_focus,
                    &token_usage,
                    &mut stop_requested,
                    &mut tool_cards_collapsed,
                    &mut tool_card_individual_states,
                );
                
                // Empty buffer should never trigger submission regardless of Enter
                assert!(!on_submit, "Empty buffer should not allow submission");
            });
        });
    }
}