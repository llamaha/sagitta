use std::sync::Arc;
use egui::{Ui, RichText, Grid, ScrollArea, ComboBox};
use tokio::sync::Mutex;
use super::manager::RepositoryManager;

use super::types::{RepoPanelState, FileSearchResult};

/// Render the file search view
pub fn render_file_search(
    ui: &mut Ui, 
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: crate::gui::theme::AppTheme,
) {
    ui.heading("Search Files");
    
    // Sync file search options with selected repository if needed
    if let Some(selected_repo) = &state.selected_repo {
        if state.file_search_options.repo_name != *selected_repo {
            state.file_search_options.repo_name = selected_repo.clone();
        }
    }
    
    // Check for file search updates from async task
    if let Some(channel) = &mut state.file_search_result.channel {
        if let Ok(result) = channel.receiver.try_recv() {
            // Update file search result
            state.file_search_result.is_loading = result.is_loading;
            state.file_search_result.error_message = result.error_message;
            state.file_search_result.files = result.files;
        }
    }
    
    if state.selected_repo.is_none() {
        ui.label("No repository selected");
        
        // Repository selector dropdown
        let repo_names = state.repo_names();
        if !repo_names.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Select repository:");
                ComboBox::from_id_salt("search_no_repo_selector")
                    .selected_text("Choose repository...")
                    .show_ui(ui, |ui| {
                        for name in repo_names {
                            if ui.selectable_value(
                                &mut state.selected_repo,
                                Some(name.clone()),
                                &name
                            ).clicked() {
                                state.file_search_options.repo_name = name;
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
    
    // Search options
    Grid::new("file_search_options_grid")
        .num_columns(2)
        .spacing([8.0, 8.0])
        .show(ui, |ui| {
            ui.label("Repository:");
            let repo_names = state.repo_names();
            let selected_text = state.selected_repo.as_ref().unwrap_or(&state.file_search_options.repo_name);
            ComboBox::from_id_salt("repository_select_file_search")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for name in repo_names {
                        if ui.selectable_value(
                            &mut state.file_search_options.repo_name, 
                            name.clone(),
                            &name
                        ).clicked() {
                            // Also update the selected_repo to maintain consistency
                            state.selected_repo = Some(name.clone());
                            
                            // Write the repository state file for MCP server
                            let repo_manager_clone = repo_manager.clone();
                            let repo_name_for_state = name.clone();
                            
                            tokio::spawn(async move {
                                let repo_manager_guard = repo_manager_clone.lock().await;
                                if let Ok(repositories) = repo_manager_guard.list_repositories().await {
                                    if let Some(repo_config) = repositories.iter().find(|r| r.name == repo_name_for_state) {
                                        // Write the current repository path to state file
                                        let mut state_path = dirs::config_dir().unwrap_or_default();
                                        state_path.push("sagitta-code");
                                        
                                        // Ensure directory exists
                                        if let Err(e) = tokio::fs::create_dir_all(&state_path).await {
                                            log::warn!("Failed to create state directory: {e}");
                                        } else {
                                            state_path.push("current_repository.txt");
                                            if let Err(e) = tokio::fs::write(&state_path, repo_config.local_path.to_string_lossy().as_bytes()).await {
                                                log::warn!("Failed to write repository state file: {e}");
                                            } else {
                                                log::debug!("Wrote current repository path to state file: {}", state_path.display());
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                });
            ui.end_row();
            
            ui.label("Pattern:");
            ui.text_edit_singleline(&mut state.file_search_options.pattern);
            ui.end_row();
            
            ui.label("Case Sensitive:");
            ui.checkbox(&mut state.file_search_options.case_sensitive, "");
            ui.end_row();
        });
    
    // Search button
    ui.vertical_centered(|ui| {
        if ui.button("Search Files").clicked() {
            if state.file_search_options.pattern.is_empty() {
                return;
            }
            
            // Set loading state
            state.file_search_result.is_loading = true;
            state.file_search_result.error_message = None;
            state.file_search_result.files.clear();
            
            // Clone search options for async operation
            let options = state.file_search_options.clone();
            let repo_manager_clone = Arc::clone(&repo_manager);
            
            // Get a clone of the sender if available
            let sender = state.file_search_result.channel
                .as_ref()
                .map(|ch| ch.sender.clone());
            
            // Schedule the search operation
            let handle = tokio::runtime::Handle::current();
            handle.spawn(async move {
                let manager = repo_manager_clone.lock().await;
                
                // Call the actual search method
                let result = manager.search_file(
                    &options.repo_name,
                    &options.pattern,
                    options.case_sensitive
                ).await;
                
                // Send result back to UI thread through channel if available
                if let Some(sender) = sender {
                    match result {
                        Ok(files) => {
                            log::info!("File search found {} files", files.len());
                            let _ = sender.try_send(FileSearchResult {
                                is_loading: false,
                                error_message: None,
                                files,
                                channel: None,
                            });
                        },
                        Err(e) => {
                            log::error!("File search error: {e}");
                            let _ = sender.try_send(FileSearchResult {
                                is_loading: false,
                                error_message: Some(e.to_string()),
                                files: Vec::new(),
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
    if state.file_search_result.is_loading {
        ui.label(RichText::new("Searching...").color(theme.warning_color()));
    } else if let Some(error) = &state.file_search_result.error_message {
        ui.label(RichText::new(format!("Error: {error}")).color(theme.error_color()));
    }
    
    // Search results
    ui.label("Search Results:");
    
    // Clone the data we need to avoid borrow checker issues
    let files = state.file_search_result.files.clone();
    let repo_name = state.file_search_options.repo_name.clone();
    let is_loading = state.file_search_result.is_loading;
    
    ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            if !is_loading && files.is_empty() {
                ui.label("No files found matching pattern");
                return;
            }
            
            for file_path in files {
                if ui.selectable_label(false, &file_path).clicked() {
                    // Set up file view options when a file is clicked
                    state.file_view_options.repo_name = repo_name.clone();
                    state.file_view_options.file_path = file_path;
                    state.file_view_options.start_line = None;
                    state.file_view_options.end_line = None;
                    
                    // Switch to file view tab
                    state.active_tab = super::types::RepoPanelTab::ViewFile;
                }
            }
        });
} 