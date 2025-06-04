use std::sync::Arc;
use egui::{Ui, RichText, Color32, Grid, ScrollArea, Button};
use tokio::sync::Mutex;
use super::manager::RepositoryManager;
use super::types::{RepoPanelState, BranchSyncResult, RefTypeTab, BranchOperationResult, TagOperationResult, SwitchOperationResult, CreateBranchResult, DeleteBranchResult};
use git_manager::GitManager;
use log::{info, error, warn};

/// Render the branch management view
pub fn render_branch_management(
    ui: &mut Ui,
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: crate::gui::theme::AppTheme,
) {
    ui.heading("Git Reference Management");
    ui.separator();

    // Process async operation results first
    process_async_results(state);

    // Repository selection
    render_repository_selector(ui, state, repo_manager.clone());
    ui.separator();

    // Only show reference operations if a repository is selected
    if let Some(repo_name) = state.branch_management.selected_repo_for_branches.clone() {
        render_current_branch_info(ui, state, theme);
        ui.separator();
        
        // Reference type tabs
        render_ref_type_tabs(ui, state, theme);
        ui.separator();
        
        match state.branch_management.ref_type_tab {
            RefTypeTab::Branches => {
                render_branch_list(ui, state, repo_manager.clone(), &repo_name, theme);
            }
            RefTypeTab::Tags => {
                render_tag_list(ui, state, repo_manager.clone(), &repo_name, theme);
            }
            RefTypeTab::Manual => {
                render_manual_ref_input(ui, state, repo_manager.clone(), &repo_name, theme);
            }
        }
        
        ui.separator();
        render_branch_operations(ui, state, repo_manager, &repo_name, theme);
        
        // Show last sync result if available
        if let Some(ref sync_result) = state.branch_management.last_sync_result {
            ui.separator();
            render_sync_result(ui, sync_result, theme);
        }
    } else {
        ui.label("Select a repository to manage Git references");
    }
}

/// Process results from async operations and update UI state
fn process_async_results(state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) {
    let selected_repo = state.branch_management.selected_repo_for_branches.clone();
    let selected_repo_str = selected_repo.as_ref().map(|s| s.as_str()).unwrap_or("");

    // Collect branch results
    let mut branch_results = Vec::new();
    if let Some(ref mut receiver) = state.branch_management.branch_result_receiver {
        while let Ok(result) = receiver.try_recv() {
            if result.repo_name == selected_repo_str {
                branch_results.push(result);
            }
        }
    }

    // Process branch results
    for result in branch_results {
        state.branch_management.is_loading_branches = false;
        if result.success {
            state.branch_management.available_branches = result.branches;
            state.branch_management.current_branch = result.current_branch;
        } else {
            state.branch_management.switch_error = result.error_message;
        }
    }

    // Collect tag results
    let mut tag_results = Vec::new();
    if let Some(ref mut receiver) = state.branch_management.tag_result_receiver {
        while let Ok(result) = receiver.try_recv() {
            if result.repo_name == selected_repo_str {
                tag_results.push(result);
            }
        }
    }

    // Process tag results
    for result in tag_results {
        state.branch_management.is_loading_tags = false;
        if result.success {
            state.branch_management.available_tags = result.tags;
        } else {
            state.branch_management.switch_error = result.error_message;
        }
    }

    // Collect switch results
    let mut switch_results = Vec::new();
    if let Some(ref mut receiver) = state.branch_management.switch_result_receiver {
        while let Ok(result) = receiver.try_recv() {
            if result.repo_name == selected_repo_str {
                switch_results.push(result);
            }
        }
    }

    // Process switch results
    for result in switch_results {
        state.branch_management.is_switching_branch = false;
        if result.success {
            state.branch_management.switch_success = Some(format!("Successfully switched to '{}'", result.target_ref));
            state.branch_management.last_sync_result = result.sync_result;
            // Refresh current branch info
            state.branch_management.current_branch = Some(result.target_ref);
            // Clear any previous errors
            state.branch_management.switch_error = None;
        } else {
            state.branch_management.switch_error = result.error_message;
            state.branch_management.switch_success = None;
        }
    }

    // Collect create results
    let mut create_results = Vec::new();
    if let Some(ref mut receiver) = state.branch_management.create_result_receiver {
        while let Ok(result) = receiver.try_recv() {
            if result.repo_name == selected_repo_str {
                create_results.push(result);
            }
        }
    }

    // Process create results
    for result in create_results {
        state.branch_management.is_creating_branch = false;
        if result.success {
            state.branch_management.create_success = Some(format!("Successfully created branch '{}'", result.branch_name));
            state.branch_management.new_branch_name.clear();
            // Trigger branch list refresh
            state.branch_management.is_loading_branches = true;
            state.branch_management.create_error = None;
        } else {
            state.branch_management.create_error = result.error_message;
            state.branch_management.create_success = None;
        }
    }

    // Collect delete results
    let mut delete_results = Vec::new();
    if let Some(ref mut receiver) = state.branch_management.delete_result_receiver {
        while let Ok(result) = receiver.try_recv() {
            if result.repo_name == selected_repo_str {
                delete_results.push(result);
            }
        }
    }

    // Process delete results
    for result in delete_results {
        state.branch_management.is_deleting_branch = false;
        if result.success {
            state.branch_management.delete_success = Some(format!("Successfully deleted branch '{}'", result.branch_name));
            // Trigger branch list refresh
            state.branch_management.is_loading_branches = true;
            state.branch_management.delete_error = None;
        } else {
            state.branch_management.delete_error = result.error_message;
            state.branch_management.delete_success = None;
        }
    }
}

/// Render repository selector dropdown
fn render_repository_selector(ui: &mut Ui, state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>, repo_manager: Arc<Mutex<RepositoryManager>>) {
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
                        // Repository changed, reset state and load data
                        reset_branch_state(state);
                        state.branch_management.selected_repo_for_branches = Some(repo.name.clone());
                        state.branch_management.is_loading_branches = true;
                        state.branch_management.is_loading_tags = true;
                        
                        // Initialize channels if not already done
                        initialize_channels_if_needed(state);
                        
                        // Start loading operations
                        load_branches(repo_manager.clone(), repo.name.clone(), get_branch_sender(state));
                        load_tags(repo_manager.clone(), repo.name.clone(), get_tag_sender(state));
                    }
                }
            });
            
        if ui.button("Refresh").clicked() {
            if let Some(repo_name) = state.branch_management.selected_repo_for_branches.clone() {
                state.branch_management.is_loading_branches = true;
                state.branch_management.is_loading_tags = true;
                clear_messages(state);
                
                // Initialize channels if not already done
                initialize_channels_if_needed(state);
                
                // Start loading operations
                load_branches(repo_manager.clone(), repo_name.clone(), get_branch_sender(state));
                load_tags(repo_manager.clone(), repo_name.clone(), get_tag_sender(state));
            }
        }
    });
}

/// Render reference type tabs
fn render_ref_type_tabs(ui: &mut Ui, state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>, theme: crate::gui::theme::AppTheme) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        
        let selected_branches = state.branch_management.ref_type_tab == RefTypeTab::Branches;
        if ui.selectable_label(selected_branches, "Branches").clicked() {
            state.branch_management.ref_type_tab = RefTypeTab::Branches;
        }
        
        let selected_tags = state.branch_management.ref_type_tab == RefTypeTab::Tags;
        if ui.selectable_label(selected_tags, "Tags").clicked() {
            state.branch_management.ref_type_tab = RefTypeTab::Tags;
        }
        
        let selected_manual = state.branch_management.ref_type_tab == RefTypeTab::Manual;
        if ui.selectable_label(selected_manual, "Manual Ref").clicked() {
            state.branch_management.ref_type_tab = RefTypeTab::Manual;
        }
    });
}

/// Render current branch information
fn render_current_branch_info(ui: &mut Ui, state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>, theme: crate::gui::theme::AppTheme) {
    ui.horizontal(|ui| {
        ui.label("Current Branch/Ref:");
        
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
        load_branches(repo_manager, repo_name.to_string(), get_branch_sender(state));
        return;
    }
    
    if state.branch_management.available_branches.is_empty() {
        ui.label("No branches found");
        return;
    }
    
    render_ref_grid(ui, state, repo_manager, repo_name, &state.branch_management.available_branches.clone(), "Branch", theme);
}

/// Render the list of available tags with switch buttons
fn render_tag_list(
    ui: &mut Ui,
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    repo_name: &str,
    theme: crate::gui::theme::AppTheme,
) {
    ui.label(RichText::new("Available Tags:").strong());
    
    if state.branch_management.is_loading_tags {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Loading tags...");
        });
        
        // Trigger tag loading
        load_tags(repo_manager, repo_name.to_string(), get_tag_sender(state));
        return;
    }
    
    if state.branch_management.available_tags.is_empty() {
        ui.label("No tags found");
        return;
    }
    
    render_ref_grid(ui, state, repo_manager, repo_name, &state.branch_management.available_tags.clone(), "Tag", theme);
}

/// Render manual reference input
fn render_manual_ref_input(
    ui: &mut Ui,
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    repo_name: &str,
    theme: crate::gui::theme::AppTheme,
) {
    ui.label(RichText::new("Manual Git Reference:").strong());
    ui.label("Enter any valid Git reference (commit hash, remote branch, etc.):");
    
    ui.horizontal(|ui| {
        ui.label("Ref:");
        ui.text_edit_singleline(&mut state.branch_management.manual_ref_input);
        
        let switch_button = Button::new("Switch to Ref");
        let is_switching = state.branch_management.is_switching_branch;
        let has_ref = !state.branch_management.manual_ref_input.trim().is_empty();
        
        if ui.add_enabled(!is_switching && has_ref, switch_button).clicked() {
            // Trigger switch to manual ref
            switch_to_ref(
                repo_manager.clone(),
                repo_name.to_string(),
                state.branch_management.manual_ref_input.trim().to_string(),
                get_switch_sender(state),
            );
            state.branch_management.is_switching_branch = true;
            clear_messages(state);
        }
        
        if is_switching {
            ui.spinner();
            ui.label("Switching...");
        }
    });
    
    ui.separator();
    ui.label(RichText::new("Examples:").strong());
    ui.label("• Commit hash: abc123def456789...");
    ui.label("• Tag: v1.0.0, release-2023-01");
    ui.label("• Remote branch: origin/feature-branch");
    ui.label("• Short commit: abc123d");
}

/// Common function to render a grid of refs (branches or tags)
fn render_ref_grid(
    ui: &mut Ui,
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    repo_name: &str,
    refs: &[String],
    ref_type: &str,
    theme: crate::gui::theme::AppTheme,
) {
    ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            Grid::new(format!("{}_grid", ref_type.to_lowercase()))
                .num_columns(3)
                .striped(true)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    // Header
                    ui.label(RichText::new(ref_type).strong());
                    ui.label(RichText::new("Status").strong());
                    ui.label(RichText::new("Actions").strong());
                    ui.end_row();
                    
                    for git_ref in refs {
                        // Ref name
                        ui.label(git_ref);
                        
                        // Status
                        if git_ref == state.branch_management.current_branch.as_ref().unwrap_or(&String::new()) {
                            ui.label(RichText::new("Current").color(theme.success_color()));
                        } else {
                            ui.label("");
                        }
                        
                        // Actions
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Only show delete for branches, not tags
                            if ref_type == "Branch" && ui.button("Delete").clicked() {
                                state.branch_management.branch_to_delete = Some(git_ref.clone());
                                state.branch_management.show_delete_confirmation = true;
                            }
                            
                            if ui.button("Switch").clicked() {
                                // Trigger switch to ref operation
                                switch_to_ref(
                                    repo_manager.clone(),
                                    repo_name.to_string(),
                                    git_ref.clone(),
                                    get_switch_sender(state),
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
                get_create_sender(state),
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
                        get_delete_sender(state),
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
    state.branch_management.available_tags.clear();
    state.branch_management.current_branch = None;
    state.branch_management.is_loading_branches = false;
    state.branch_management.is_loading_tags = false;
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

fn load_branches(repo_manager: Arc<Mutex<RepositoryManager>>, repo_name: String, sender: Option<Sender<BranchOperationResult>>) {
    tokio::spawn(async move {
        info!("Loading branches for repository: {}", repo_name);
        
        match repo_manager.lock().await.list_branches(&repo_name).await {
            Ok(branches) => {
                info!("Successfully loaded {} branches for repository '{}'", branches.len(), repo_name);
                
                if let Some(sender) = sender {
                    let current_branch = branches.first().cloned();
                    if let Err(e) = sender.send(BranchOperationResult {
                        repo_name,
                        success: true,
                        branches,
                        current_branch,
                        error_message: None,
                    }) {
                        warn!("Failed to send branch loading result: receiver dropped");
                    }
                }
            }
            Err(e) => {
                error!("Failed to load branches for repository '{}': {}", repo_name, e);
                
                if let Some(sender) = sender {
                    if let Err(_) = sender.send(BranchOperationResult {
                        repo_name,
                        success: false,
                        branches: Vec::new(),
                        current_branch: None,
                        error_message: Some(e.to_string()),
                    }) {
                        warn!("Failed to send branch loading error: receiver dropped");
                    }
                }
            }
        }
    });
}

fn load_tags(repo_manager: Arc<Mutex<RepositoryManager>>, repo_name: String, sender: Option<Sender<TagOperationResult>>) {
    tokio::spawn(async move {
        info!("Loading tags for repository: {}", repo_name);
        
        match repo_manager.lock().await.list_tags(&repo_name).await {
            Ok(tags) => {
                info!("Successfully loaded {} tags for repository '{}'", tags.len(), repo_name);
                // TODO: Update UI state with tags
                // This would require a channel or callback mechanism to update the UI state
                if let Some(sender) = sender {
                    if let Err(_) = sender.send(TagOperationResult {
                        repo_name,
                        success: true,
                        tags,
                        error_message: None,
                    }) {
                        warn!("Failed to send tag loading result: receiver dropped");
                    }
                }
            }
            Err(e) => {
                error!("Failed to load tags for repository '{}': {}", repo_name, e);
                // TODO: Update UI state with error
                if let Some(sender) = sender {
                    if let Err(_) = sender.send(TagOperationResult {
                        repo_name,
                        success: false,
                        tags: Vec::new(),
                        error_message: Some(e.to_string()),
                    }) {
                        warn!("Failed to send tag loading error: receiver dropped");
                    }
                }
            }
        }
    });
}

fn switch_to_ref(repo_manager: Arc<Mutex<RepositoryManager>>, repo_name: String, ref_name: String, sender: Option<Sender<SwitchOperationResult>>) {
    tokio::spawn(async move {
        info!("Switching to ref '{}' in repository '{}'", ref_name, repo_name);
        
        match repo_manager.lock().await.switch_to_ref(&repo_name, &ref_name, true).await {
            Ok(result) => {
                info!("Successfully switched to ref '{}' in repository '{}'. Sync type: {}, Files processed: {}", 
                      ref_name, repo_name, result.sync_type, result.files_processed);
                
                if let Some(sender) = sender {
                    let _ = sender.send(SwitchOperationResult {
                        repo_name,
                        target_ref: ref_name,
                        success: true,
                        sync_result: Some(result),
                        error_message: None,
                    });
                }
            }
            Err(e) => {
                error!("Failed to switch to ref '{}' in repository '{}': {}", ref_name, repo_name, e);
                
                if let Some(sender) = sender {
                    let _ = sender.send(SwitchOperationResult {
                        repo_name,
                        target_ref: ref_name,
                        success: false,
                        sync_result: None,
                        error_message: Some(e.to_string()),
                    });
                }
            }
        }
    });
}

fn create_branch(repo_manager: Arc<Mutex<RepositoryManager>>, repo_name: String, branch_name: String, sender: Option<Sender<CreateBranchResult>>) {
    tokio::spawn(async move {
        info!("Creating branch '{}' in repository '{}'", branch_name, repo_name);
        
        match repo_manager.lock().await.create_branch(&repo_name, &branch_name, false).await {
            Ok(()) => {
                info!("Successfully created branch '{}' in repository '{}'", branch_name, repo_name);
                
                if let Some(sender) = sender {
                    let _ = sender.send(CreateBranchResult {
                        repo_name,
                        branch_name,
                        success: true,
                        error_message: None,
                    });
                }
            }
            Err(e) => {
                error!("Failed to create branch '{}' in repository '{}': {}", branch_name, repo_name, e);
                
                if let Some(sender) = sender {
                    let _ = sender.send(CreateBranchResult {
                        repo_name,
                        branch_name,
                        success: false,
                        error_message: Some(e.to_string()),
                    });
                }
            }
        }
    });
}

fn delete_branch(repo_manager: Arc<Mutex<RepositoryManager>>, repo_name: String, branch_name: String, sender: Option<Sender<DeleteBranchResult>>) {
    tokio::spawn(async move {
        info!("Deleting branch '{}' in repository '{}'", branch_name, repo_name);
        
        match repo_manager.lock().await.delete_branch(&repo_name, &branch_name, false).await {
            Ok(()) => {
                info!("Successfully deleted branch '{}' in repository '{}'", branch_name, repo_name);
                
                if let Some(sender) = sender {
                    let _ = sender.send(DeleteBranchResult {
                        repo_name,
                        branch_name,
                        success: true,
                        error_message: None,
                    });
                }
            }
            Err(e) => {
                error!("Failed to delete branch '{}' in repository '{}': {}", branch_name, repo_name, e);
                
                if let Some(sender) = sender {
                    let _ = sender.send(DeleteBranchResult {
                        repo_name,
                        branch_name,
                        success: false,
                        error_message: Some(e.to_string()),
                    });
                }
            }
        }
    });
}

/// Helper functions for channel management

fn initialize_channels_if_needed(state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) {
    if state.branch_management.branch_result_receiver.is_none() {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        state.branch_management.branch_result_receiver = Some(receiver);
        // Store sender in a way that can be accessed later - we'll use a different approach
    }
    
    if state.branch_management.tag_result_receiver.is_none() {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        state.branch_management.tag_result_receiver = Some(receiver);
    }
    
    if state.branch_management.switch_result_receiver.is_none() {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        state.branch_management.switch_result_receiver = Some(receiver);
    }
    
    if state.branch_management.create_result_receiver.is_none() {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        state.branch_management.create_result_receiver = Some(receiver);
    }
    
    if state.branch_management.delete_result_receiver.is_none() {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        state.branch_management.delete_result_receiver = Some(receiver);
    }
}

// We need to store senders in the state as well
type Sender<T> = tokio::sync::mpsc::UnboundedSender<T>;

fn get_branch_sender(state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) -> Option<Sender<BranchOperationResult>> {
    // For now, create a new channel each time - this is not ideal but will work
    if state.branch_management.branch_result_receiver.is_none() {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        state.branch_management.branch_result_receiver = Some(receiver);
        Some(sender)
    } else {
        // We can't easily get the sender from the receiver, so we'll create a new channel
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        state.branch_management.branch_result_receiver = Some(receiver);
        Some(sender)
    }
}

fn get_tag_sender(state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) -> Option<Sender<TagOperationResult>> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    state.branch_management.tag_result_receiver = Some(receiver);
    Some(sender)
}

fn get_switch_sender(state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) -> Option<Sender<SwitchOperationResult>> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    state.branch_management.switch_result_receiver = Some(receiver);
    Some(sender)
}

fn get_create_sender(state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) -> Option<Sender<CreateBranchResult>> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    state.branch_management.create_result_receiver = Some(receiver);
    Some(sender)
}

fn get_delete_sender(state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) -> Option<Sender<DeleteBranchResult>> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    state.branch_management.delete_result_receiver = Some(receiver);
    Some(sender)
} 