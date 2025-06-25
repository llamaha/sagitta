use std::sync::Arc;
use egui::{Ui, RichText, Color32, Grid, TextEdit, ScrollArea, ComboBox, Button, text::LayoutJob, TextStyle, TextFormat, FontId};
use tokio::sync::Mutex;
use super::manager::RepositoryManager;
use egui_code_editor::CodeEditor;

use super::types::{RepoPanelState, FileViewResult};

/// Render the file view component
pub fn render_file_view(
    ui: &mut Ui, 
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: crate::gui::theme::AppTheme,
) {
    ui.heading("View File");
    
    // Sync file view options with selected repository if needed
    if let Some(selected_repo) = &state.selected_repo {
        if state.file_view_options.repo_name != *selected_repo {
            state.file_view_options.repo_name = selected_repo.clone();
        }
    }
    
    // Check for file content updates from async task
    if let Some(channel) = &mut state.file_view_result.channel {
        if let Ok(result) = channel.receiver.try_recv() {
            // Update file view result
            state.file_view_result.is_loading = result.is_loading;
            state.file_view_result.error_message = result.error_message;
            state.file_view_result.content = result.content;
        }
    }
    
    if state.selected_repo.is_none() {
        ui.label("No repository selected");
        
        // Repository selector dropdown
        let repo_names = state.repo_names();
        if !repo_names.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Select repository:");
                ComboBox::from_id_source("view_no_repo_selector")
                    .selected_text("Choose repository...")
                    .show_ui(ui, |ui| {
                        for name in repo_names {
                            if ui.selectable_value(
                                &mut state.selected_repo,
                                Some(name.clone()),
                                &name
                            ).clicked() {
                                state.file_view_options.repo_name = name;
                            }
                        }
                    });
            });
        } else {
            ui.label("No repositories available");
            if ui.button("Go to Repository List").clicked() {
                state.active_tab = super::types::RepoPanelTab::List;
            }
        }
        
        return;
    }
    
    // File view options
    Grid::new("file_view_options_grid")
        .num_columns(2)
        .spacing([8.0, 8.0])
        .show(ui, |ui| {
            ui.label("Repository:");
            let repo_names: Vec<String> = state.repositories.iter().map(|r| r.name.clone()).collect();
            let selected_text = state.selected_repo.as_ref().unwrap_or(&state.file_view_options.repo_name);
            ComboBox::from_id_source("repository_select_file_view")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for name in repo_names {
                        if ui.selectable_value(
                            &mut state.file_view_options.repo_name, 
                            name.clone(),
                            &name
                        ).clicked() {
                            // Also update the selected_repo to maintain consistency
                            state.selected_repo = Some(name.clone());
                        }
                    }
                });
            ui.end_row();
            
            ui.label("File Path:");
            ui.text_edit_singleline(&mut state.file_view_options.file_path);
            ui.end_row();
            
            ui.label("Start Line (optional):");
            ui.horizontal(|ui| {
                let mut start_line_str = if let Some(line) = state.file_view_options.start_line {
                    line.to_string()
                } else {
                    String::new()
                };
                
                if ui.text_edit_singleline(&mut start_line_str).changed() {
                    state.file_view_options.start_line = start_line_str.parse().ok();
                }
            });
            ui.end_row();
            
            ui.label("End Line (optional):");
            ui.horizontal(|ui| {
                let mut end_line_str = if let Some(line) = state.file_view_options.end_line {
                    line.to_string()
                } else {
                    String::new()
                };
                
                if ui.text_edit_singleline(&mut end_line_str).changed() {
                    state.file_view_options.end_line = end_line_str.parse().ok();
                }
            });
            ui.end_row();
        });
    
    // View button
    ui.vertical_centered(|ui| {
        if ui.button("View File").clicked() {
            if state.file_view_options.file_path.is_empty() {
                return;
            }
            
            // Set loading state
            state.file_view_result.is_loading = true;
            state.file_view_result.error_message = None;
            state.file_view_result.content = String::new();
            
            // Clone view options for async operation
            let options = state.file_view_options.clone();
            let repo_manager_clone = Arc::clone(&repo_manager);
            
            // Get sender clone for async operation
            let sender = state.file_view_result.channel.as_ref().map(|ch| ch.sender.clone());
            
            // Schedule the view operation
            let handle = tokio::runtime::Handle::current();
            handle.spawn(async move {
                let manager = repo_manager_clone.lock().await;
                
                // Call the actual view method
                let result = manager.view_file(
                    &options.repo_name,
                    &options.file_path,
                    options.start_line.map(|l| l as u32),
                    options.end_line.map(|l| l as u32),
                ).await;
                
                // Send result back to UI thread through channel
                if let Some(sender) = sender {
                    match result {
                        Ok(content) => {
                            let _ = sender.try_send(FileViewResult {
                                is_loading: false,
                                error_message: None,
                                content,
                                channel: None,
                            });
                        },
                        Err(e) => {
                            let _ = sender.try_send(FileViewResult {
                                is_loading: false,
                                error_message: Some(e.to_string()),
                                content: String::new(),
                                channel: None,
                            });
                        }
                    }
                }
            });
        }
    });
    
    ui.separator();
    
    // Show loading indicator or error message
    if state.file_view_result.is_loading {
        ui.label(RichText::new("Loading file...").color(theme.warning_color()));
    } else if let Some(error) = &state.file_view_result.error_message {
        ui.label(RichText::new(format!("Error: {}", error)).color(theme.error_color()));
    }
    
    // File content
    ui.label("File Content:");
    
    // Clone needed data
    let file_path = state.file_view_options.file_path.clone();
    let content = state.file_view_result.content.clone();
    let is_loading = state.file_view_result.is_loading;
    
    ScrollArea::vertical()
        .max_height(400.0)
        .show(ui, |ui| {
            // Display file content with syntax highlighting
            if file_path.is_empty() {
                ui.label("No file selected");
            } else if content.is_empty() && !is_loading {
                ui.label("No content available");
            } else {
                // Create a copy of the content to satisfy the TextEdit API
                let mut content_copy = content;
                
                let text_style = egui::TextStyle::Monospace;
                let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                    let mut layout_job = egui::text::LayoutJob::default();
                    layout_job.append(
                        text,
                        0.0,
                        egui::text::TextFormat {
                            font_id: text_style.resolve(ui.style()),
                            color: ui.visuals().text_color(),
                            ..Default::default()
                        },
                    );
                    ui.fonts(|f| f.layout_job(layout_job))
                };
                
                ui.add(
                    egui::TextEdit::multiline(&mut content_copy)
                        .font(egui::TextStyle::Monospace)
                        .code_editor()
                        .desired_rows(10)
                        .lock_focus(true)
                        .desired_width(f32::INFINITY)
                        .layouter(&mut layouter),
                );
            }
        });
} 