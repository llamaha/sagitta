use std::sync::Arc;
use egui::{Ui, RichText, Color32, Grid, TextEdit, ScrollArea};
use tokio::sync::Mutex;
use super::manager::RepositoryManager;

use super::types::{RepoPanelState, RepoInfo};

/// Render the repository list view
pub fn render_repo_list(
    ui: &mut Ui, 
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: crate::gui::theme::AppTheme,
) {
    // Load repositories at the start of rendering if needed
    if state.repositories.is_empty() && !state.is_loading_repos {
        state.is_loading_repos = true;
        
        // Schedule the load operation
        let repo_manager_clone = Arc::clone(&repo_manager);
        let handle = tokio::runtime::Handle::current();
            
        handle.spawn(async move {
            let manager = repo_manager_clone.lock().await;
            if let Ok(repos) = manager.list_repositories().await {
                // We'll use a refresh flag to indicate that repos need to be updated
                // in the next frame, as we can't directly update state from here
                // The panel.rs render function will check this flag and update state
                let repos_info: Vec<RepoInfo> = repos.into_iter()
                    .map(RepoInfo::from)
                    .collect();
                
                // Pass to panel through channel or static state if available
                // For now, we'll assume the next render will handle refresh logic
            }
        });
    }
    
    ui.horizontal(|ui| {
        ui.label("Filter:");
        ui.text_edit_singleline(&mut state.repository_filter.search_term);
        
        if ui.button("Refresh").clicked() {
            // Set loading flag
            state.is_loading_repos = true;
            
            // Schedule the refresh using the runtime
            let repo_manager_clone = Arc::clone(&repo_manager);
            let handle = tokio::runtime::Handle::current();
            
            handle.spawn(async move {
                let manager = repo_manager_clone.lock().await;
                let _ = manager.list_repositories().await;
                // State will be updated on next render
            });
        }
    });
    
    ui.separator();
    
    ScrollArea::vertical().show(ui, |ui| {
        Grid::new("repositories_grid")
            .num_columns(3)
            .striped(true)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                // Header
                ui.label(RichText::new("Name").strong());
                ui.label(RichText::new("Source").strong());
                ui.label(RichText::new("Actions").strong());
                ui.end_row();
                
                let filtered_repos: Vec<RepoInfo> = if state.repository_filter.search_term.is_empty() {
                    state.repositories.clone()
                } else {
                    state.repositories.iter()
                        .filter(|r| r.name.to_lowercase().contains(&state.repository_filter.search_term.to_lowercase()))
                        .cloned()
                        .collect()
                };
                
                if filtered_repos.is_empty() {
                    ui.label("No repositories found");
                    ui.label("");
                    ui.label("");
                    ui.end_row();
                }
                
                for repo in filtered_repos {
                    // Name column
                    let is_selected = state.selected_repo.as_ref().map_or(false, |s| s == &repo.name);
                    
                    if ui.selectable_label(is_selected, &repo.name).clicked() {
                        if is_selected {
                            state.selected_repo = None;
                            // Also remove from selected_repos
                            state.selected_repos.retain(|name| name != &repo.name);
                        } else {
                            state.selected_repo = Some(repo.name.clone());
                            // Also add to selected_repos if not already there
                            if !state.selected_repos.contains(&repo.name) {
                                state.selected_repos.push(repo.name.clone());
                            }
                            
                            // Initialize options for other tabs
                            state.query_options = super::types::QueryOptions::new(repo.name.clone());
                            state.file_search_options = super::types::FileSearchOptions::new(repo.name.clone());
                            state.file_view_options = super::types::FileViewOptions::new(repo.name.clone());
                        }
                    }
                    
                    // Source column
                    let source_text = if let Some(remote) = &repo.remote {
                        if remote.is_empty() {
                            if let Some(path) = &repo.local_path {
                                path.to_string_lossy().to_string()
                            } else {
                                "Local".to_string()
                            }
                        } else {
                            remote.clone()
                        }
                    } else {
                        "Local".to_string()
                    };
                    
                    let source = if let Some(branch) = &repo.branch {
                        format!("{} ({})", source_text, branch)
                    } else {
                        source_text
                    };
                    
                    ui.label(source);
                    
                    // Actions column
                    ui.horizontal(|ui| {
                        if ui.button("Query").clicked() {
                            state.active_tab = super::types::RepoPanelTab::Query;
                            state.selected_repo = Some(repo.name.clone());
                            state.query_options = super::types::QueryOptions::new(repo.name.clone());
                        }
                        
                        if ui.button("Files").clicked() {
                            state.active_tab = super::types::RepoPanelTab::SearchFile;
                            state.selected_repo = Some(repo.name.clone());
                            state.file_search_options = super::types::FileSearchOptions::new(repo.name.clone());
                        }
                        
                        if ui.button("Remove").clicked() {
                            // Set up the remove
                            let repo_name = repo.name.clone();
                            let repo_name_for_async = repo_name.clone();
                            
                            // Schedule the remove operation
                            let repo_manager_clone = Arc::clone(&repo_manager);
                            let handle = tokio::runtime::Handle::current();
                            
                            handle.spawn(async move {
                                let mut manager = repo_manager_clone.lock().await;
                                let _ = manager.remove_repository(&repo_name_for_async).await;
                            });
                            
                            // Also remove from UI state immediately for responsiveness
                            state.repositories.retain(|r| r.name != repo_name);
                            if state.selected_repo.as_ref() == Some(&repo_name) {
                                state.selected_repo = None;
                            }
                            state.selected_repos.retain(|name| name != &repo_name);
                        }
                    });
                    
                    ui.end_row();
                }
            });
    });
} 