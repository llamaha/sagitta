use std::sync::Arc;
use egui::{Ui, RichText, Color32, Grid, ScrollArea, Button};
use tokio::sync::Mutex;
use super::manager::RepositoryManager;
use super::types::{RepoPanelState, BranchSyncResult};
use git_manager::GitManager;
use log::{info, error, warn};

/// Render the branch management view
pub fn render_branch_management(
    ui: &mut Ui,
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: crate::gui::theme::AppTheme,
) {
    ui.heading("Branch Management");
    ui.separator();

    // Repository selection
    render_repository_selector(ui, state);
    ui.separator();

    // Only show branch operations if a repository is selected
    if let Some(repo_name) = state.branch_management.selected_repo_for_branches.clone() {
        render_current_branch_info(ui, state, theme);
        ui.separator();
        
        render_branch_list(ui, state, repo_manager.clone(), &repo_name, theme);
        ui.separator();
        
        render_branch_operations(ui, state, repo_manager, &repo_name, theme);
        
        // Show last sync result if available
        if let Some(ref sync_result) = state.branch_management.last_sync_result {
            ui.separator();
            render_sync_result(ui, sync_result, theme);
        }
    } else {
        ui.label("Select a repository to manage branches");
    }
}

/// Render repository selector dropdown
fn render_repository_selector(ui: &mut Ui, state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) {
    ui.horizontal(|ui| {
        ui.label("Repository:");
        
        let selected_text = state.branch_management.selected_repo_for_branches
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("Select repository...");
            
        let repositories = state.repositories.clone(); // Clone to avoid borrow conflicts
            
        egui::ComboBox::from_label("")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for repo in &repositories {
                    let is_selected = state.branch_management.selected_repo_for_branches
                        .as_ref()
                        .map_or(false, |selected| selected == &repo.name);
                        
                    if ui.selectable_value(
                        &mut state.branch_management.selected_repo_for_branches,
                        Some(repo.name.clone()),
                        &repo.name
                    ).clicked() && !is_selected {
                        // Repository changed, reset branch state and load branches
                        reset_branch_state(state);
                        state.branch_management.selected_repo_for_branches = Some(repo.name.clone());
                        state.branch_management.is_loading_branches = true;
                    }
                }
            });
            
        if ui.button("Refresh Branches").clicked() {
            if state.branch_management.selected_repo_for_branches.is_some() {
                state.branch_management.is_loading_branches = true;
                clear_messages(state);
            }
        }
    });
}

/// Render current branch information
fn render_current_branch_info(ui: &mut Ui, state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>, theme: crate::gui::theme::AppTheme) {
    ui.horizontal(|ui| {
        ui.label("Current Branch:");
        
        if let Some(current_branch) = &state.branch_management.current_branch {
            ui.label(RichText::new(current_branch).color(theme.success_color()).strong());
        } else if state.branch_management.is_loading_branches {
            ui.spinner();
            ui.label("Loading...");
        } else {
            ui.label(RichText::new("Unknown").color(theme.hint_text_color()));
        }
    });
}

/// Render the list of available branches with switch buttons
fn render_branch_list(
    ui: &mut Ui,
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    repo_name: &str,
    theme: crate::gui::theme::AppTheme,
) {
    ui.label(RichText::new("Available Branches:").strong());
    
    if state.branch_management.is_loading_branches {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Loading branches...");
        });
        
        // Trigger branch loading
        load_branches(repo_manager, repo_name.to_string());
        return;
    }
    
    if state.branch_management.available_branches.is_empty() {
        ui.label("No branches found");
        return;
    }
    
    ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
            Grid::new("branches_grid")
                .num_columns(3)
                .striped(true)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    // Header
                    ui.label(RichText::new("Branch").strong());
                    ui.label(RichText::new("Status").strong());
                    ui.label(RichText::new("Actions").strong());
                    ui.end_row();
                    
                    for branch in &state.branch_management.available_branches.clone() {
                        // Branch name
                        ui.label(branch);
                        
                        // Status
                        if branch == state.branch_management.current_branch.as_ref().unwrap_or(&String::new()) {
                            ui.label(RichText::new("Current").color(theme.success_color()));
                        } else {
                            ui.label("");
                        }
                        
                        // Actions
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Delete").clicked() {
                                state.branch_management.branch_to_delete = Some(branch.clone());
                            }
                            
                            if ui.button("Switch").clicked() {
                                // Trigger switch branch operation
                                switch_branch(
                                    repo_manager.clone(),
                                    repo_name.to_string(),
                                    branch.clone(),
                                );
                                state.branch_management.is_switching_branch = true;
                                clear_messages(state);
                            }
                        });
                        
                        ui.end_row();
                    }
                });
        });
}

/// Render branch operations (create new branch)
fn render_branch_operations(
    ui: &mut Ui,
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    repo_name: &str,
    theme: crate::gui::theme::AppTheme,
) {
    ui.label(RichText::new("Branch Operations:").strong());
    
    // Create new branch
    ui.horizontal(|ui| {
        ui.label("New branch:");
        ui.text_edit_singleline(&mut state.branch_management.new_branch_name);
        
        let create_button = Button::new("Create");
        let is_creating = state.branch_management.is_creating_branch;
        let has_name = !state.branch_management.new_branch_name.trim().is_empty();
        
        if ui.add_enabled(!is_creating && has_name, create_button).clicked() {
            create_branch(
                repo_manager.clone(),
                repo_name.to_string(),
                state.branch_management.new_branch_name.trim().to_string(),
            );
            state.branch_management.is_creating_branch = true;
            clear_messages(state);
        }
        
        if is_creating {
            ui.spinner();
            ui.label("Creating...");
        }
    });
    
    // Show operation status messages
    render_status_messages(ui, state, theme);
    
    // Show delete confirmation dialog
    render_delete_confirmation(ui, state, repo_manager, repo_name, theme);
}

/// Render status messages for branch operations
fn render_status_messages(
    ui: &mut Ui,
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    theme: crate::gui::theme::AppTheme,
) {
    // Display messages
    if let Some(ref error) = state.branch_management.switch_error {
        ui.label(RichText::new(format!("Switch Error: {}", error)).color(theme.error_color()));
    }
    
    if let Some(ref success) = state.branch_management.switch_success {
        ui.label(RichText::new(format!("Switch Success: {}", success)).color(theme.success_color()));
    }
    
    if let Some(ref error) = state.branch_management.create_error {
        ui.label(RichText::new(format!("Create Error: {}", error)).color(theme.error_color()));
    }
    
    if let Some(ref success) = state.branch_management.create_success {
        ui.label(RichText::new(format!("Create Success: {}", success)).color(theme.success_color()));
    }
    
    if let Some(ref error) = state.branch_management.delete_error {
        ui.label(RichText::new(format!("Delete Error: {}", error)).color(theme.error_color()));
    }
    
    if let Some(ref success) = state.branch_management.delete_success {
        ui.label(RichText::new(format!("Delete Success: {}", success)).color(theme.success_color()));
    }
}

/// Render delete confirmation dialog
fn render_delete_confirmation(
    ui: &mut Ui,
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    repo_name: &str,
    theme: crate::gui::theme::AppTheme,
) {
    if state.branch_management.show_delete_confirmation {
        if let Some(ref branch_to_delete) = state.branch_management.branch_to_delete.clone() {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(format!("Delete branch '{}'?", branch_to_delete));
                if ui.button(RichText::new("Confirm Delete").color(theme.error_color()))
                    .clicked() {
                    delete_branch(
                        repo_manager.clone(),
                        repo_name.to_string(),
                        branch_to_delete.clone(),
                    );
                    state.branch_management.is_deleting_branch = true;
                    state.branch_management.show_delete_confirmation = false;
                    state.branch_management.branch_to_delete = None;
                    clear_messages(state);
                }
                
                if ui.button("Cancel").clicked() {
                    state.branch_management.show_delete_confirmation = false;
                    state.branch_management.branch_to_delete = None;
                }
            });
        }
    }
}

/// Render the result of the last branch switch with sync details
fn render_sync_result(ui: &mut Ui, sync_result: &BranchSyncResult, theme: crate::gui::theme::AppTheme) {
    ui.label(RichText::new("Last Branch Switch Result:").strong());
    
    let status_color = if sync_result.success {
        theme.success_color()
    } else {
        theme.error_color()
    };
    
    ui.label(RichText::new(format!(
        "Switched from '{}' to '{}' - {} ({})",
        sync_result.previous_branch,
        sync_result.new_branch,
        if sync_result.success { "Success" } else { "Failed" },
        sync_result.sync_type
    )).color(status_color));
    
    if sync_result.files_processed > 0 {
        ui.label(format!("Files processed: {}", sync_result.files_processed));
    }
    
    if let Some(ref error) = sync_result.error_message {
        ui.label(RichText::new(format!("Error: {}", error)).color(theme.error_color()));
    }
}

/// Helper functions for async operations

fn reset_branch_state(state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) {
    state.branch_management.available_branches.clear();
    state.branch_management.current_branch = None;
    state.branch_management.is_loading_branches = false;
    clear_messages(state);
}

fn clear_messages(state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) {
    state.branch_management.switch_error = None;
    state.branch_management.switch_success = None;
    state.branch_management.create_error = None;
    state.branch_management.create_success = None;
    state.branch_management.delete_error = None;
    state.branch_management.delete_success = None;
}

/// Async operations (these will be implemented to work with the repository manager)

fn load_branches(repo_manager: Arc<Mutex<RepositoryManager>>, repo_name: String) {
    tokio::spawn(async move {
        info!("Loading branches for repository: {}", repo_name);
        
        match repo_manager.lock().await.list_branches(&repo_name).await {
            Ok(branches) => {
                info!("Successfully loaded {} branches for repository '{}'", branches.len(), repo_name);
                // TODO: Update UI state with branches
                // This would require a channel or callback mechanism to update the UI state
            }
            Err(e) => {
                error!("Failed to load branches for repository '{}': {}", repo_name, e);
                // TODO: Update UI state with error
            }
        }
    });
}

fn switch_branch(repo_manager: Arc<Mutex<RepositoryManager>>, repo_name: String, branch_name: String) {
    tokio::spawn(async move {
        info!("Switching to branch '{}' in repository '{}'", branch_name, repo_name);
        
        match repo_manager.lock().await.switch_branch(&repo_name, &branch_name, true).await {
            Ok(result) => {
                info!("Successfully switched to branch '{}' in repository '{}'. Sync type: {}, Files processed: {}", 
                      branch_name, repo_name, result.sync_type, result.files_processed);
                // TODO: Update UI state with success result
            }
            Err(e) => {
                error!("Failed to switch to branch '{}' in repository '{}': {}", branch_name, repo_name, e);
                // TODO: Update UI state with error
            }
        }
    });
}

fn create_branch(repo_manager: Arc<Mutex<RepositoryManager>>, repo_name: String, branch_name: String) {
    tokio::spawn(async move {
        info!("Creating branch '{}' in repository '{}'", branch_name, repo_name);
        
        match repo_manager.lock().await.create_branch(&repo_name, &branch_name, false).await {
            Ok(()) => {
                info!("Successfully created branch '{}' in repository '{}'", branch_name, repo_name);
                // TODO: Update UI state with success and refresh branch list
            }
            Err(e) => {
                error!("Failed to create branch '{}' in repository '{}': {}", branch_name, repo_name, e);
                // TODO: Update UI state with error
            }
        }
    });
}

fn delete_branch(repo_manager: Arc<Mutex<RepositoryManager>>, repo_name: String, branch_name: String) {
    tokio::spawn(async move {
        info!("Deleting branch '{}' in repository '{}'", branch_name, repo_name);
        
        match repo_manager.lock().await.delete_branch(&repo_name, &branch_name, false).await {
            Ok(()) => {
                info!("Successfully deleted branch '{}' in repository '{}'", branch_name, repo_name);
                // TODO: Update UI state with success and refresh branch list
            }
            Err(e) => {
                error!("Failed to delete branch '{}' in repository '{}': {}", branch_name, repo_name, e);
                // TODO: Update UI state with error
            }
        }
    });
} 