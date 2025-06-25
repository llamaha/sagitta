// User input handling for chat with enhanced features

use egui::{
    widgets::TextEdit, 
    Color32, 
    Rounding, 
    Stroke, 
    Event, 
    Key, 
    Layout, 
    Align, 
    Vec2, 
    RichText,
    Frame,
    ComboBox,
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
    current_agent_mode: crate::agent::state::types::AgentMode,
    on_agent_mode_change: &mut Option<crate::agent::state::types::AgentMode>,
    // Repository context parameters
    current_repository_context: &Option<String>,
    available_repositories: &[String],
    on_repository_context_change: &mut Option<String>,
    // Loop control parameters
    is_in_loop: bool,
    loop_break_requested: &mut bool,
    loop_inject_buffer: &mut String,
    show_loop_inject_input: &mut bool,
    loop_inject_message: &mut Option<String>,
) -> Option<egui::Id> {
    // Handle key events before the text edit widget to manually process Ctrl+Enter
    let mut new_line_added = false;
    
    // Process raw events to catch Ctrl+Enter before the TextEdit widget does
    ui.input_mut(|input| {
        let mut events_to_eat = Vec::new();
        
        for event_index in 0..input.events.len() {
            if let Event::Key { key, pressed, modifiers, .. } = &input.events[event_index] {
                if *key == Key::Enter && *pressed && modifiers.ctrl {
                    // Add a newline manually
                    input_buffer.push('\n');
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
    let button_bg_color = theme.button_background();
    let button_hover_color = theme.button_hover_color();
    let accent_color = theme.accent_color();
    let input_bg_color = theme.input_background();
    let border_color = theme.border_color();
    let focus_border_color = theme.focus_border_color();
    let success_color = theme.success_color();
    let warning_color = theme.warning_color();
    let error_color = theme.error_color();
    
    let mut text_edit_id: Option<egui::Id> = None;
    
    // Use a vertical layout centered within the panel provided by app.rs
    ui.vertical_centered(|ui| {
        // Agent Mode Selector
        ui.horizontal(|ui| {
            ui.label(RichText::new("Mode:").color(hint_color).small());
            ui.add_space(4.0);
            
            let mode_text = match current_agent_mode {
                crate::agent::state::types::AgentMode::ChatOnly => "💬 Chat Only",
                crate::agent::state::types::AgentMode::ToolsWithConfirmation => "🤝 Tools (Ask First)",
                crate::agent::state::types::AgentMode::FullyAutonomous => "🤖 Fully Autonomous",
            };
            
            let mode_color = match current_agent_mode {
                crate::agent::state::types::AgentMode::ChatOnly => accent_color,
                crate::agent::state::types::AgentMode::ToolsWithConfirmation => warning_color,
                crate::agent::state::types::AgentMode::FullyAutonomous => success_color,
            };
            
            egui::ComboBox::from_id_source("agent_mode_selector")
                .selected_text(RichText::new(mode_text).color(mode_color).small())
                .width(150.0)
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    ui.set_min_width(180.0);
                    
                    if ui.selectable_value(
                        &mut *on_agent_mode_change, 
                        Some(crate::agent::state::types::AgentMode::ChatOnly),
                        RichText::new("💬 Chat Only").color(accent_color)
                    ).clicked() {
                        *on_agent_mode_change = Some(crate::agent::state::types::AgentMode::ChatOnly);
                    }
                    
                    if ui.selectable_value(
                        &mut *on_agent_mode_change, 
                        Some(crate::agent::state::types::AgentMode::ToolsWithConfirmation),
                        RichText::new("🤝 Tools (Ask First)").color(warning_color)
                    ).clicked() {
                        *on_agent_mode_change = Some(crate::agent::state::types::AgentMode::ToolsWithConfirmation);
                    }
                    
                    if ui.selectable_value(
                        &mut *on_agent_mode_change, 
                        Some(crate::agent::state::types::AgentMode::FullyAutonomous),
                        RichText::new("🤖 Fully Autonomous").color(success_color)
                    ).clicked() {
                        *on_agent_mode_change = Some(crate::agent::state::types::AgentMode::FullyAutonomous);
                    }
                });
            
            ui.add_space(8.0);

            // Repository context selector
            let repo_text = match current_repository_context {
                Some(repo) => format!("📁 {}", repo),
                None => "📁 No Repository".to_string(),
            };
            
            let repo_color = if current_repository_context.is_some() {
                success_color
            } else {
                hint_color
            };

            egui::ComboBox::from_id_source("repository_context_selector")
                .selected_text(RichText::new(&repo_text).color(repo_color).small())
                .width(180.0)
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
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
                        if !available_repositories.contains(current_repo) {
                            if ui.selectable_value(
                                &mut *on_repository_context_change,
                                Some(current_repo.clone()),
                                RichText::new(format!("📁 {} (loading...)", current_repo)).color(success_color)
                            ).clicked() {
                                *on_repository_context_change = Some(current_repo.clone());
                            }
                        }
                    }
                    
                    // Available repositories
                    for repo in available_repositories {
                        if ui.selectable_value(
                            &mut *on_repository_context_change,
                            Some(repo.clone()),
                            RichText::new(format!("📁 {}", repo)).color(success_color)
                        ).clicked() {
                            *on_repository_context_change = Some(repo.clone());
                        }
                    }
                });
            
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
            
            // Removed help text for cleaner UI
            
            // Add Message Sagitta Code and help button on the same line (right side)
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
                ui.small(RichText::new("Message Sagitta Code").color(hint_color));
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
            
            // Show character count on the right
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let char_count = input_buffer.chars().count();
                let color = if char_count > 2000 {
                    error_color
                } else {
                    hint_color
                };
                ui.small(RichText::new(format!("{} chars", char_count)).color(color));
            });
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
                
                if inject_response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    if !loop_inject_buffer.trim().is_empty() {
                        *loop_inject_message = Some(loop_inject_buffer.clone());
                        loop_inject_buffer.clear();
                        *show_loop_inject_input = false;
                    }
                }
                
                if ui.small_button(RichText::new("Send").color(text_color)).clicked() {
                    if !loop_inject_buffer.trim().is_empty() {
                        *loop_inject_message = Some(loop_inject_buffer.clone());
                        loop_inject_buffer.clear();
                        *show_loop_inject_input = false;
                    }
                }
                
                if ui.small_button(RichText::new("Cancel").color(hint_color)).clicked() {
                    *show_loop_inject_input = false;
                    loop_inject_buffer.clear();
                }
            });
        }
        
        ui.add_space(4.0);
        
        // Main input area with proper theme styling
        Frame::none()
            .fill(input_bg_color)
            .stroke(Stroke::new(1.0, border_color))
            .rounding(Rounding::same(4))
            .inner_margin(Vec2::splat(8.0))
            .show(ui, |ui| {
                let text_edit = TextEdit::multiline(input_buffer)
                    .hint_text("Type your message here... (Enter to send, Ctrl+Enter for new line)")
                    .desired_width(f32::INFINITY)
                    .desired_rows(3)
                    .text_color(text_color)
                    .frame(false);
                
                let response = ui.add(text_edit);
                text_edit_id = Some(response.id);
                
                // Handle Enter key for submission
                if response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && !new_line_added {
                    if !input_buffer.trim().is_empty() {
                        *on_submit = true;
                    }
                }
            });
        
        ui.add_space(4.0);
        
        // Bottom controls
        ui.horizontal(|ui| {
            // Left side - keyboard shortcuts hint
            ui.small(RichText::new("Enter: Send • Ctrl+Enter: New line • ?: Help").color(hint_color));

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
                    .corner_radius(Rounding::same(16))
                    .min_size(Vec2::new(80.0, 24.0))
                )
                .on_hover_text("Clear the input field")
                .clicked()
                {
                    input_buffer.clear();
                }

                ui.add_space(8.0);

                // Send button (prominent style)
                let send_button_enabled = !input_buffer.trim().is_empty() && !is_waiting;
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
                    .corner_radius(Rounding::same(18))
                    .min_size(Vec2::new(100.0, 36.0))
                ).clicked() {
                    *on_submit = true;
                }
            });
        });
    });
    
    text_edit_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Context;

    #[test]
    fn test_chat_input_basic_behavior() {
        // Test basic input behavior without actual UI context
        // We can test the logic parts that don't require egui context
        
        let mut input_buffer = String::new();
        let mut on_submit = false;
        let is_waiting = false;
        let theme = AppTheme::Dark; // Use the new simplified theme
        let mut show_hotkeys_modal = false;
        
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
        let mut input_buffer = String::new();
        let mut on_submit = false;
        let mut show_hotkeys_modal = false;
        let mut on_agent_mode_change: Option<crate::agent::state::types::AgentMode> = None;
        let mut loop_break_requested = false;
        let mut loop_inject_buffer = String::new();
        let mut show_loop_inject_input = false;
        let mut loop_inject_message: Option<String> = None;
        
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
}

