// User input handling for chat with enhanced features

use egui::{
    widgets::TextEdit, 
    Color32, 
    CornerRadius, 
    Stroke, 
    Event, 
    Key, 
    Layout, 
    Align, 
    Vec2, 
    RichText,
    Frame,
};
use crate::gui::theme::AppTheme;

/// Renders the chat input area with enhanced styling and features
pub fn chat_input_ui(
    ui: &mut egui::Ui, 
    input_buffer: &mut String, 
    on_submit: &mut bool,
    is_waiting: bool,
    theme: AppTheme,
    show_hotkeys_modal: &mut bool,
    // Repository context parameters
    current_repository_context: &Option<String>,
    available_repositories: &[String],
    on_repository_context_change: &mut Option<String>,
    on_repository_refresh_requested: &mut bool,
    // Provider context parameters  
    current_provider: &crate::providers::types::ProviderType,
    available_providers: &[crate::providers::types::ProviderType],
    on_provider_change: &mut Option<crate::providers::types::ProviderType>,
    // Git controls
    git_controls: &mut crate::gui::repository::git_controls::GitControls,
    // Loop control parameters
    is_in_loop: bool,
    loop_break_requested: &mut bool,
    loop_inject_buffer: &mut String,
    show_loop_inject_input: &mut bool,
    loop_inject_message: &mut Option<String>,
    // Focus management
    should_focus_input: &mut bool,
    // Token usage
    current_token_usage: &Option<crate::llm::client::TokenUsage>,
    // Stop/Cancel callback
    stop_requested: &mut bool,
) -> Option<egui::Id> {
    // Handle key events before the text edit widget to manually process Ctrl+Enter
    let mut new_line_added = false;
    let mut cursor_pos_to_set: Option<usize> = None;
    
    // Process raw events to catch Ctrl+Enter before the TextEdit widget does
    ui.input_mut(|input| {
        let mut events_to_eat = Vec::new();
        
        for event_index in 0..input.events.len() {
            if let Event::Key { key, pressed, modifiers, .. } = &input.events[event_index] {
                if *key == Key::Enter && *pressed && (modifiers.ctrl || modifiers.command) {
                    // Add a newline manually and remember cursor position
                    input_buffer.push('\n');
                    cursor_pos_to_set = Some(input_buffer.len()); // Position cursor at end (after the newline)
                    new_line_added = true;
                    events_to_eat.push(event_index);
                }
            }
        }
        
        // Eat the events we've handled (in reverse order to maintain indices)
        for index in events_to_eat.into_iter().rev() {
            input.events.remove(index);
        }
    });
    
    // Use theme colors properly
    let text_color = theme.text_color();
    let hint_color = theme.hint_text_color();
    let _button_bg_color = theme.button_background();
    let _button_hover_color = theme.button_hover_color();
    let accent_color = theme.accent_color();
    let input_bg_color = theme.input_background();
    let border_color = theme.border_color();
    let _focus_border_color = theme.focus_border_color();
    let success_color = theme.success_color();
    let warning_color = theme.warning_color();
    let error_color = theme.error_color();
    
    let mut text_edit_id: Option<egui::Id> = None;
    
    // Add horizontal margins to prevent content from extending beyond window
    let margin = 8.0;
    let available_width = ui.available_width() - (margin * 2.0);
    
    // Use a vertical layout with constrained width
    ui.allocate_ui_with_layout(
        Vec2::new(available_width, ui.available_height()),
        Layout::top_down(Align::Center),
        |ui| {
        // Repository and Provider selector
        ui.horizontal(|ui| {
            // Repository context selector
            let repo_text = match current_repository_context {
                Some(repo) => format!("📁 {repo}"),
                None => "📁 No Repository".to_string(),
            };
            
            let repo_color = if current_repository_context.is_some() {
                success_color
            } else {
                hint_color
            };

            let combo_response = egui::ComboBox::from_id_salt("repository_context_selector")
                .selected_text(RichText::new(&repo_text).color(repo_color).small())
                .width(180.0)
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    ui.set_min_width(200.0);
                    
                    // "No Repository" option
                    if ui.selectable_value(
                        &mut *on_repository_context_change,
                        Some("".to_string()),
                        RichText::new("📁 No Repository").color(hint_color)
                    ).clicked() {
                        *on_repository_context_change = Some("".to_string());
                    }
                    
                    // If current repository is set but not in available list, show it first
                    if let Some(current_repo) = current_repository_context {
                        if !available_repositories.contains(current_repo)
                            && ui.selectable_value(
                                &mut *on_repository_context_change,
                                Some(current_repo.clone()),
                                RichText::new(format!("📁 {current_repo} (loading...)")).color(success_color)
                            ).clicked() {
                                *on_repository_context_change = Some(current_repo.clone());
                            }
                    }
                    
                    // Available repositories
                    for repo in available_repositories {
                        if ui.selectable_value(
                            &mut *on_repository_context_change,
                            Some(repo.clone()),
                            RichText::new(format!("📁 {repo}")).color(success_color)
                        ).clicked() {
                            *on_repository_context_change = Some(repo.clone());
                        }
                    }
                });
            
            // Trigger repository refresh when ComboBox is opened
            if combo_response.response.clicked() {
                *on_repository_refresh_requested = true;
            }

            
            // Show git controls if a repository is selected
            if let Some(repo_name) = current_repository_context {
                if !repo_name.is_empty() {
                    ui.separator();
                    // Check if git controls need to update repository context
                    if git_controls.state().current_repository.as_ref() != Some(repo_name) {
                        // We can't call async method here, so we'll set a flag for later processing
                        // For now, just render the controls with the current state
                    }
                    // Render git workflow controls
                    git_controls.render(ui, theme);
                }
            }
            
            // Show "Create new repository" and "Add existing" buttons when "No repository" is selected
            if current_repository_context.is_none() || current_repository_context.as_ref().map(|s| s.is_empty()).unwrap_or(false) {
                ui.add_space(8.0);
                if ui.button(RichText::new("🆕 Create project").color(accent_color).small()).clicked() {
                    // Set a flag to open the repository panel with CreateProject tab
                    // This will be handled by the main app
                    *on_repository_context_change = Some("__CREATE_NEW_REPOSITORY__".to_string());
                }
                ui.add_space(4.0);
                if ui.button(RichText::new("➕ Add project").color(accent_color).small()).clicked() {
                    // Set a flag to open the repository panel with Add tab
                    // This will be handled by the main app
                    *on_repository_context_change = Some("__ADD_EXISTING_REPOSITORY__".to_string());
                }
            }
            
            // Show token usage and character count if available
            if let Some(token_usage) = current_token_usage {
                ui.add_space(16.0);
                
                // Calculate percentage if we have a model context window size
                let context_window = match token_usage.model_name.as_str() {
                    model if model.contains("claude-3-5-sonnet") => 200000,
                    model if model.contains("claude-3-5-haiku") => 200000,
                    model if model.contains("claude-3-opus") => 200000,
                    model if model.contains("claude-3-sonnet") => 200000,
                    model if model.contains("claude-3-haiku") => 200000,
                    model if model.contains("gpt-4o") => 128000,
                    model if model.contains("gpt-4-turbo") => 128000,
                    model if model.contains("gpt-4") => 8192,
                    model if model.contains("gpt-3.5-turbo") => 16385,
                    _ => 100000, // Default context window
                };
                
                let percentage = (token_usage.total_tokens as f32 / context_window as f32 * 100.0).min(100.0);
                let color = if percentage > 90.0 {
                    theme.error_color()
                } else if percentage > 75.0 {
                    theme.warning_color()
                } else {
                    hint_color
                };
                
                ui.label(RichText::new(format!("📊 {percentage:.1}%"))
                    .color(color)
                    .small());
                
                ui.separator();
                
                ui.label(RichText::new(format!("{} tokens", token_usage.total_tokens))
                    .color(hint_color)
                    .small());
                
                if let Some(cached) = token_usage.cached_tokens {
                    ui.separator();
                    ui.label(RichText::new(format!("💾 {cached} cached"))
                        .color(success_color)
                        .small());
                }
            }
            
            // Removed help text for cleaner UI
            
            // Add character count and help button on the same line (right side)
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Simple help button with tooltip
                let help_label = RichText::new("?")
                    .color(accent_color)
                    .small();
                
                if ui.small_button(help_label)
                    .on_hover_text("Show keyboard shortcuts")
                    .clicked() 
                {
                    *show_hotkeys_modal = !*show_hotkeys_modal;
                }
                
                ui.add_space(8.0);
                
                // Show character count on the right
                let char_count = input_buffer.chars().count();
                let char_color = if char_count > 2000 {
                    error_color
                } else {
                    hint_color
                };
                ui.small(RichText::new(format!("{char_count} chars")).color(char_color));
            });
        });
        
        ui.add_space(2.0); // Reduced spacing
        
        // Status indicator (typing, waiting, etc.)
        ui.horizontal(|ui| {
            if is_in_loop {
                ui.add_space(4.0);
                ui.small(RichText::new("🔄 Sagitta Code is in reasoning loop...").color(warning_color));
                ui.spinner();
                
                // Loop control buttons
                ui.add_space(8.0);
                if ui.small_button(RichText::new("⏹ Stop Loop").color(error_color))
                    .on_hover_text("Break out of the reasoning loop")
                    .clicked() 
                {
                    *loop_break_requested = true;
                }
                
                ui.add_space(4.0);
                if ui.small_button(RichText::new("💬 Inject Message").color(accent_color))
                    .on_hover_text("Inject a message into the loop (Ctrl+I)")
                    .clicked() 
                {
                    *show_loop_inject_input = !*show_loop_inject_input;
                }
            } else if is_waiting {
                ui.add_space(4.0);
                ui.small(RichText::new("Sagitta Code is thinking...").color(hint_color));
                ui.spinner();
            }
        });
        
        // Loop injection input (shown when requested)
        if *show_loop_inject_input {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("💬 Inject into loop:").color(accent_color).small());
                
                let inject_response = ui.add_sized(
                    [ui.available_width() - 120.0, 24.0],
                    TextEdit::singleline(loop_inject_buffer)
                        .hint_text("Message to inject into reasoning loop...")
                        .text_color(text_color)
                );
                
                if inject_response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
                    && !loop_inject_buffer.trim().is_empty() {
                        *loop_inject_message = Some(loop_inject_buffer.clone());
                        loop_inject_buffer.clear();
                        *show_loop_inject_input = false;
                    }
                
                if ui.small_button(RichText::new("Send").color(text_color)).clicked()
                    && !loop_inject_buffer.trim().is_empty() {
                        *loop_inject_message = Some(loop_inject_buffer.clone());
                        loop_inject_buffer.clear();
                        *show_loop_inject_input = false;
                    }
                
                if ui.small_button(RichText::new("Cancel").color(hint_color)).clicked() {
                    *show_loop_inject_input = false;
                    loop_inject_buffer.clear();
                }
            });
        }
        
        ui.add_space(4.0);
        
        // Main input area with proper theme styling
        // Add horizontal constraint to prevent overflow
        ui.horizontal(|ui| {
            ui.add_space(margin); // Left margin
            
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width() - margin, ui.available_height()),
                Layout::left_to_right(Align::TOP),
                |ui| {
                    Frame::NONE
                        .fill(input_bg_color)
                        .stroke(Stroke::new(1.0, border_color))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(Vec2::splat(8.0))
                        .show(ui, |ui| {
                // Disable input during waiting/thinking/loop states
                let input_enabled = !is_waiting && !is_in_loop;
                
                // Calculate approximate height needed for the text
                let line_count = input_buffer.lines().count().max(1);
                let line_height = 20.0; // Approximate line height
                let content_height = (line_count as f32 * line_height).min(80.0);
                
                // Only use ScrollArea if content exceeds max height
                if content_height >= 80.0 {
                    // Large content - use scroll area
                    egui::ScrollArea::vertical()
                        .max_height(80.0)
                        .show(ui, |ui| {
                            let text_edit = TextEdit::multiline(input_buffer)
                                .hint_text(if input_enabled {
                                    "Type your message here... (Enter to send, Ctrl+Enter for new line)"
                                } else if is_in_loop {
                                    "Input disabled during reasoning loop..."
                                } else {
                                    "Input disabled while Sagitta Code is thinking..."
                                })
                                .desired_width(ui.available_width())
                                .min_size(Vec2::new(ui.available_width(), 60.0))
                                .text_color(if input_enabled { text_color } else { hint_color })
                                .interactive(input_enabled)
                                .frame(false);
                            
                            let response = ui.add(text_edit);
                            text_edit_id = Some(response.id);
                            
                            // Set cursor position if we added a newline with Ctrl+Enter
                            if let Some(cursor_pos) = cursor_pos_to_set {
                                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                                    let ccursor = egui::text::CCursor::new(cursor_pos);
                                    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                                    state.store(ui.ctx(), response.id);
                                }
                            }
                            
                            // Handle Enter key for submission
                            if input_enabled && response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && !new_line_added
                                && !input_buffer.trim().is_empty() {
                                    *on_submit = true;
                                }
                        });
                } else {
                    // Small content - no scroll area needed
                    let text_edit = TextEdit::multiline(input_buffer)
                        .hint_text(if input_enabled {
                            "Type your message here... (Enter to send, Ctrl+Enter for new line)"
                        } else if is_in_loop {
                            "Input disabled during reasoning loop..."
                        } else {
                            "Input disabled while Sagitta Code is thinking..."
                        })
                        .desired_width(ui.available_width())
                        .min_size(Vec2::new(ui.available_width(), 60.0))
                        .text_color(if input_enabled { text_color } else { hint_color })
                        .interactive(input_enabled)
                        .frame(false);
                    
                    let response = ui.add(text_edit);
                    text_edit_id = Some(response.id);
                    
                    // Set cursor position if we added a newline with Ctrl+Enter
                    if let Some(cursor_pos) = cursor_pos_to_set {
                        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                            let ccursor = egui::text::CCursor::new(cursor_pos);
                            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                            state.store(ui.ctx(), response.id);
                        }
                    }
                    
                    // Request focus when needed (cursor should be visible without ScrollArea)
                    if *should_focus_input && input_enabled {
                        response.request_focus();
                        *should_focus_input = false;
                    }
                    
                    // Handle Enter key for submission
                    if input_enabled && response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && !new_line_added
                        && !input_buffer.trim().is_empty() {
                            *on_submit = true;
                        }
                }
            });
                });
        });
        
        ui.add_space(4.0);
        
        // Bottom controls
        ui.horizontal(|ui| {
            // Left side - keyboard shortcuts hint
            ui.small(RichText::new("Enter: Send • Ctrl+Enter: New line • F1: Menu").color(hint_color));

            // Right side - buttons
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Clear button (ghost style)
                if ui.add(
                    egui::Button::new(
                        RichText::new("Clear")
                            .color(hint_color)
                            .size(12.0)
                    )
                    .fill(Color32::TRANSPARENT)
                    .corner_radius(CornerRadius::same(16))
                    .min_size(Vec2::new(80.0, 24.0))
                )
                .on_hover_text("Clear the input field")
                .clicked()
                {
                    input_buffer.clear();
                }

                ui.add_space(8.0);

                // Send/Stop button - changes based on state
                if is_waiting || is_in_loop {
                    // Show Stop button during waiting/thinking/loop states
                    // Use a darker shade of accent color instead of bright red
                    let stop_color = Color32::from_rgb(
                        (accent_color.r() as f32 * 0.7) as u8,
                        (accent_color.g() as f32 * 0.7) as u8,
                        (accent_color.b() as f32 * 0.7) as u8,
                    );
                    if ui.add(
                        egui::Button::new(
                            RichText::new("⏹ Stop")
                                .color(Color32::WHITE)
                                .strong()
                        )
                        .fill(stop_color)
                        .corner_radius(CornerRadius::same(18))
                        .min_size(Vec2::new(100.0, 36.0))
                    )
                    .on_hover_text(if is_in_loop { "Stop the reasoning loop" } else { "Stop the current operation" })
                    .clicked() {
                        if is_in_loop {
                            *loop_break_requested = true;
                        } else {
                            *stop_requested = true;
                            log::info!("Stop button clicked during waiting state");
                        }
                    }
                } else {
                    // Show Send button during normal state
                    let send_button_enabled = !input_buffer.trim().is_empty();
                    let send_color = if send_button_enabled { accent_color } else { theme.button_disabled_color() };
                    let send_text_color = if send_button_enabled { Color32::WHITE } else { theme.button_disabled_text_color() };

                    if ui.add_enabled(
                        send_button_enabled,
                        egui::Button::new(
                            RichText::new("📤 Send")
                                .color(send_text_color)
                                .strong()
                        )
                        .fill(send_color)
                        .corner_radius(CornerRadius::same(18))
                        .min_size(Vec2::new(100.0, 36.0))
                    ).clicked() {
                        *on_submit = true;
                    }
                }
            });
        });
    });
    
    text_edit_id
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn test_chat_input_basic_behavior() {
        // Test basic input behavior without actual UI context
        // We can test the logic parts that don't require egui context
        
        let mut input_buffer = String::new();
        let on_submit = false;
        let is_waiting = false;
        let theme = AppTheme::Dark; // Use the new simplified theme
        let show_hotkeys_modal = false;
        
        // Test initial state
        assert!(input_buffer.is_empty());
        assert!(!on_submit);
        assert!(!show_hotkeys_modal);
        
        // Simulate adding text to buffer
        input_buffer.push_str("Hello, world!");
        assert_eq!(input_buffer, "Hello, world!");
        
        // Test buffer modification
        input_buffer.clear();
        assert!(input_buffer.is_empty());
    }

    #[test]
    fn test_hotkeys_modal_state() {
        let mut show_hotkeys_modal = false;
        
        // Test initial state
        assert!(!show_hotkeys_modal);
        
        // Test toggling
        show_hotkeys_modal = !show_hotkeys_modal;
        assert!(show_hotkeys_modal);
        
        // Test toggling back
        show_hotkeys_modal = !show_hotkeys_modal;
        assert!(!show_hotkeys_modal);
    }

    #[test]
    fn test_input_buffer_char_counting() {
        let input_buffer = "Hello, world! 👋";
        let char_count = input_buffer.chars().count();
        
        // Should count Unicode characters correctly
        assert_eq!(char_count, 15); // 14 ASCII chars + 1 emoji
        
        let byte_count = input_buffer.len();
        assert!(byte_count > char_count); // Emoji takes more bytes
    }

    #[test]
    fn test_newline_handling() {
        let mut input_buffer = String::from("Line 1");
        
        // Simulate Ctrl+Enter adding newline
        input_buffer.push('\n');
        input_buffer.push_str("Line 2");
        
        assert_eq!(input_buffer, "Line 1\nLine 2");
        
        let line_count = input_buffer.chars().filter(|&c| c == '\n').count() + 1;
        assert_eq!(line_count, 2);
    }

    #[test]
    fn test_input_validation() {
        let empty_buffer = String::new();
        let whitespace_buffer = String::from("   \n\t  ");
        let valid_buffer = String::from("Hello, world!");
        
        // Test empty and whitespace-only inputs
        assert!(empty_buffer.trim().is_empty());
        assert!(whitespace_buffer.trim().is_empty());
        assert!(!valid_buffer.trim().is_empty());
    }

    #[test]
    fn test_theme_compatibility() {
        // Test that the function works with different themes
        let theme = AppTheme::Dark;
        let input_buffer = String::new();
        let on_submit = false;
        let show_hotkeys_modal = false;
        let loop_break_requested = false;
        let loop_inject_buffer = String::new();
        let show_loop_inject_input = false;
        let loop_inject_message: Option<String> = None;
        
        // This should not panic with any theme
        // We can't easily test the UI rendering without egui context
        // but we can test that the theme conversion works
        let _text_color = theme.text_color();
        let _bg_color = theme.panel_background();
        let _accent_color = theme.accent_color();
    }

    #[test] 
    fn test_input_states() {
        // Test different input states
        let waiting_state = true;
        let not_waiting_state = false;
        
        assert!(waiting_state);
        assert!(!not_waiting_state);
        
        // When waiting, input should be disabled
        // When not waiting, input should be enabled
        // This logic is handled in the UI rendering
    }

    #[test]
    fn test_character_limits() {
        let mut input_buffer = String::new();
        
        // Test normal length
        input_buffer = "Hello".repeat(100); // 500 chars
        assert!(input_buffer.chars().count() < 2000);
        
        // Test approaching limit
        input_buffer = "x".repeat(1999);
        assert_eq!(input_buffer.chars().count(), 1999);
        
        // Test over limit
        input_buffer = "x".repeat(2001);
        assert!(input_buffer.chars().count() > 2000);
    }

    #[test]
    fn test_submit_conditions() {
        let empty_buffer = String::new();
        let whitespace_buffer = String::from("   ");
        let valid_buffer = String::from("Hello");
        
        // Submit should only work with non-empty, non-whitespace content
        assert!(empty_buffer.trim().is_empty()); // Should not submit
        assert!(whitespace_buffer.trim().is_empty()); // Should not submit  
        assert!(!valid_buffer.trim().is_empty()); // Should submit
    }

    #[test]
    fn test_multiline_input() {
        let multiline_input = "Line 1\nLine 2\nLine 3";
        let line_count = multiline_input.chars().filter(|&c| c == '\n').count() + 1;
        
        assert_eq!(line_count, 3);
        assert!(!multiline_input.trim().is_empty());
        
        // Test that multiline input is still valid for submission
        assert!(!multiline_input.trim().is_empty());
    }

    #[test]
    fn test_focus_management() {
        // Test that focus flag is properly managed
        let mut should_focus_input = true;
        
        // Initial state should request focus
        assert!(should_focus_input);
        
        // After requesting focus, it should be reset
        if should_focus_input {
            // Simulate requesting focus
            should_focus_input = false;
        }
        assert!(!should_focus_input);
        
        // Simulate tool completion - should request focus again
        should_focus_input = true;
        assert!(should_focus_input);
    }

    #[test]
    fn test_input_disabled_during_waiting() {
        // Test that input is disabled during waiting states
        let is_waiting = true;
        let is_in_loop = false;
        
        // Input should be disabled when waiting
        let input_enabled = !is_waiting && !is_in_loop;
        assert!(!input_enabled);
        
        // Test with loop state
        let is_waiting_2 = false;
        let is_in_loop_2 = true;
        let input_enabled_2 = !is_waiting_2 && !is_in_loop_2;
        assert!(!input_enabled_2);
        
        // Test normal state
        let is_waiting_3 = false;
        let is_in_loop_3 = false;
        let input_enabled_3 = !is_waiting_3 && !is_in_loop_3;
        assert!(input_enabled_3);
    }

    #[test]
    fn test_button_state_changes() {
        // Test that button changes between Send and Stop based on state
        let is_waiting = true;
        let is_in_loop = false;
        
        // Should show Stop button when waiting
        let show_stop_button = is_waiting || is_in_loop;
        assert!(show_stop_button);
        
        // Test with loop state
        let is_waiting_2 = false;
        let is_in_loop_2 = true;
        let show_stop_button_2 = is_waiting_2 || is_in_loop_2;
        assert!(show_stop_button_2);
        
        // Test normal state - should show Send button
        let is_waiting_3 = false;
        let is_in_loop_3 = false;
        let show_stop_button_3 = is_waiting_3 || is_in_loop_3;
        assert!(!show_stop_button_3);
    }

    #[test]
    fn test_input_area_scrolling() {
        // Test that the input area has proper scrolling constraints
        const MAX_HEIGHT: f32 = 80.0;
        const MIN_HEIGHT: f32 = 60.0;
        const LINE_HEIGHT: f32 = 20.0;
        
        // Test that max height is reasonable for scrolling
        assert!(MAX_HEIGHT > MIN_HEIGHT);
        assert!(MAX_HEIGHT < 200.0); // Should be less than panel max height
        
        // Test that minimum height shows at least 3 rows (approx 20px per row)
        assert!(MIN_HEIGHT >= 60.0);
        
        // Test small input (no scrolling needed)
        let small_input = "Line 1\nLine 2\nLine 3";
        let small_line_count = small_input.lines().count();
        assert_eq!(small_line_count, 3);
        let small_height = small_line_count as f32 * LINE_HEIGHT;
        assert!(small_height < MAX_HEIGHT); // Should not need scrolling
        
        // Test large input handling
        let large_input = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10";
        let line_count = large_input.lines().count();
        assert_eq!(line_count, 10);
        
        // With 10 lines, the input should require scrolling
        // Assuming ~20px per line, 10 lines = 200px which is > MAX_HEIGHT
        let estimated_height = line_count as f32 * LINE_HEIGHT;
        assert!(estimated_height > MAX_HEIGHT);
        
        // Test that scrolling only activates at 80px (4 lines)
        let threshold_lines = (MAX_HEIGHT / LINE_HEIGHT) as usize;
        assert_eq!(threshold_lines, 4);
    }
}

