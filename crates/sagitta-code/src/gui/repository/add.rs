use std::sync::Arc;
use std::path::PathBuf;
use egui::{Ui, RichText, Color32, Grid, TextEdit, Button, Checkbox, Layout, Align, Spinner};
use tokio::sync::Mutex;
use super::manager::RepositoryManager;
use rfd::FileDialog;

use super::types::RepoPanelState;

/// Render the add repository form
pub fn render_add_repo(
    ui: &mut Ui, 
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: crate::gui::theme::AppTheme,
) {
    ui.heading("Add Repository");
    
    // Form grid
    Grid::new("add_repo_form")
        .num_columns(2)
        .spacing([8.0, 8.0])
        .show(ui, |ui| {
            ui.label("Name:");
            ui.add_enabled(!state.add_repo_form.adding, TextEdit::singleline(&mut state.add_repo_form.name));
            ui.end_row();
            
            ui.label("Use local repository:");
            ui.add_enabled(!state.add_repo_form.adding, Checkbox::new(&mut state.add_repo_form.use_local, ""));
            ui.end_row();
            
            if state.add_repo_form.use_local {
                ui.label("Local path:");
                ui.horizontal(|ui| {
                    ui.add_enabled(!state.add_repo_form.adding, TextEdit::singleline(&mut state.add_repo_form.local_path));
                    
                    if ui.add_enabled(!state.add_repo_form.adding, Button::new("Browse")).clicked() {
                        if let Some(path) = FileDialog::new()
                            .set_title("Select Repository Directory")
                            .pick_folder() {
                                state.add_repo_form.local_path = path.to_string_lossy().to_string();
                            }
                    }
                });
                ui.end_row();
            } else {
                ui.label("Repository URL:");
                ui.add_enabled(!state.add_repo_form.adding, TextEdit::singleline(&mut state.add_repo_form.url));
                ui.end_row();
                
                ui.label("Branch (optional):");
                ui.add_enabled(!state.add_repo_form.adding, TextEdit::singleline(&mut state.add_repo_form.branch));
                ui.end_row();
                
                ui.label("Target ref (optional):");
                ui.horizontal(|ui| {
                    ui.add_enabled(!state.add_repo_form.adding, TextEdit::singleline(&mut state.add_repo_form.target_ref));
                    ui.label(RichText::new("e.g., tag, commit hash").color(theme.hint_text_color()).small());
                });
                ui.end_row();
            }
        });
    
    // Status message
    if let Some(status) = &state.add_repo_form.status_message {
        ui.label(RichText::new(status).color(theme.success_color()));
    }
    
    // Error message
    if let Some(error) = &state.add_repo_form.error_message {
        ui.label(RichText::new(error).color(theme.error_color()));
    }

    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            // Show spinner when adding
            if state.add_repo_form.adding {
                ui.add(Spinner::new());
                ui.label("Adding repository...");
            }
            
            let button_text = if state.add_repo_form.adding {
                "Adding..."
            } else {
                "Add Repository"
            };
            
            if ui.add_enabled(!state.add_repo_form.adding, Button::new(button_text)).clicked() {
                // Clear previous messages
                state.add_repo_form.error_message = None;
                state.add_repo_form.status_message = None;
                state.add_repo_form.adding = true;
                
                // Validate form
                if state.add_repo_form.name.is_empty() {
                    state.add_repo_form.error_message = Some("Repository name is required".to_string());
                    state.add_repo_form.adding = false;
                    return;
                }
                
                if state.add_repo_form.use_local {
                    if state.add_repo_form.local_path.is_empty() {
                        state.add_repo_form.error_message = Some("Local path is required".to_string());
                        state.add_repo_form.adding = false;
                        return;
                    }
                } else {
                    if state.add_repo_form.url.is_empty() {
                        state.add_repo_form.error_message = Some("Repository URL is required".to_string());
                        state.add_repo_form.adding = false;
                        return;
                    }
                }
                
                // Clone form data for the async operation
                let form = state.add_repo_form.clone();
                let repo_manager_clone = Arc::clone(&repo_manager);
                
                // Create a channel for progress updates
                let (tx, rx) = std::sync::mpsc::channel();
                
                // Store the receiver in the form for polling
                state.add_repo_form.result_receiver = Some(rx);
                
                // Schedule the add operation
                let handle = tokio::runtime::Handle::current();
                handle.spawn(async move {
                    let mut manager = repo_manager_clone.lock().await;
                    let result: Result<(), anyhow::Error> = if form.use_local {
                        manager.add_local_repository(&form.name, &form.local_path).await
                    } else {
                        let branch_str = if form.branch.is_empty() { None } else { Some(form.branch.as_str()) };
                        let target_ref_str = if form.target_ref.is_empty() { None } else { Some(form.target_ref.as_str()) };
                        manager.add_repository(&form.name, &form.url, branch_str, target_ref_str).await
                    };
                    let _ = tx.send(result.map(|_| form.name));
                });
            }
        });
    });
    
    // Poll for completion if we have a receiver
    if let Some(ref rx) = state.add_repo_form.result_receiver {
        if let Ok(res) = rx.try_recv() {
            // Clear the receiver since we got a result
            state.add_repo_form.result_receiver = None;
            
            match res {
                Ok(name) => {
                    state.add_repo_form.status_message = Some(format!("Repository '{}' added successfully", name));
                    state.add_repo_form = super::types::AddRepoForm::default();
                    state.active_tab = super::types::RepoPanelTab::List;
                    // Trigger repository list refresh
                    state.is_loading_repos = true;
                }
                Err(e) => {
                    state.add_repo_form.error_message = Some(format!("Failed to add repository: {}", e));
                    state.add_repo_form.adding = false;
                }
            }
        }
    }
} 