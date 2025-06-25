use std::sync::Arc;
use egui::{Ui, RichText, Color32, Grid, TextEdit, ScrollArea, Response, Sense, Rect, Pos2};
use tokio::sync::Mutex;
use super::manager::RepositoryManager;

use super::types::{RepoPanelState, RepoInfo, EnhancedRepoInfo, SyncState};

/// Format bytes in a human-readable format
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;
    
    if bytes == 0 {
        return "0 B".to_string();
    }
    
    let bytes_f = bytes as f64;
    let unit_index = (bytes_f.log(THRESHOLD).floor() as usize).min(UNITS.len() - 1);
    let size = bytes_f / THRESHOLD.powf(unit_index as f64);
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Get status icon and color for sync state
fn get_sync_status_indicator(sync_state: &SyncState) -> (String, Color32) {
    match sync_state {
        SyncState::UpToDate => ("✅".to_string(), Color32::from_rgb(46, 160, 67)),
        SyncState::NeedsSync => ("🔄".to_string(), Color32::from_rgb(255, 193, 7)),
        SyncState::NeverSynced => ("❓".to_string(), Color32::from_rgb(108, 117, 125)),
        SyncState::Unknown => ("⚠️".to_string(), Color32::from_rgb(220, 53, 69)),
    }
}

/// Show repository status tooltip
fn show_repo_status_tooltip(ui: &mut Ui, enhanced_repo: &EnhancedRepoInfo) {
    let tooltip_text = format!(
        "Repository Status:\n\n\
        📁 Status: {}\n\
        🌿 Branch: {}\n\
        📍 Commit: {}\n\
        🔄 Sync: {}\n\
        📊 Files: {}\n\
        💾 Size: {}\n\
        🔤 Languages: {}",
        if enhanced_repo.filesystem_status.exists {
            if enhanced_repo.filesystem_status.is_git_repository {
                "Git repository"
            } else {
                "Directory (no git)"
            }
        } else {
            "Missing from filesystem"
        },
        enhanced_repo.branch.as_deref().unwrap_or("unknown"),
        enhanced_repo.git_status.as_ref()
            .map(|git| {
                let commit_short = &git.current_commit[..8.min(git.current_commit.len())];
                if git.is_detached_head {
                    format!("{} (detached HEAD, {})", commit_short, if git.is_clean { "clean" } else { "dirty" })
                } else {
                    format!("{} ({})", commit_short, if git.is_clean { "clean" } else { "dirty" })
                }
            })
            .unwrap_or_else(|| "unknown".to_string()),
        match enhanced_repo.sync_status.state {
            SyncState::UpToDate => "up-to-date",
            SyncState::NeedsSync => "needs sync",
            SyncState::NeverSynced => "never synced",
            SyncState::Unknown => "unknown",
        },
        enhanced_repo.total_files
            .map(|count| count.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        enhanced_repo.size_bytes
            .map(|size| format_bytes(size))
            .unwrap_or_else(|| "unknown".to_string()),
        enhanced_repo.indexed_languages.as_ref()
            .map(|langs| langs.join(", "))
            .unwrap_or_else(|| "none detected".to_string())
    );
    
    ui.label(&tooltip_text);
}

/// Render the repository list view
pub fn render_repo_list(
    ui: &mut Ui, 
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: crate::gui::theme::AppTheme,
) {
    ui.horizontal(|ui| {
        ui.label("Filter:");
        ui.text_edit_singleline(&mut state.repository_filter.search_term);
        
        if ui.button("Refresh").clicked() {
            // Set loading flag to trigger refresh in panel.rs
            state.is_loading_repos = true;
            state.use_enhanced_repos = false; // Reset to trigger enhanced reload
        }
    });
    
    ui.separator();
    
    ScrollArea::vertical().show(ui, |ui| {
        Grid::new("repositories_grid")
            .num_columns(4)
            .striped(true)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                // Header
                ui.label(RichText::new("Name").strong());
                ui.label(RichText::new("Source").strong());
                ui.label(RichText::new("Status").strong());
                ui.label(RichText::new("Actions").strong());
                ui.end_row();
                
                // Choose which repositories to display based on availability
                let repos_to_display: Vec<EnhancedRepoInfo> = if state.use_enhanced_repos && !state.enhanced_repositories.is_empty() {
                    if state.repository_filter.search_term.is_empty() {
                        state.enhanced_repositories.clone()
                    } else {
                        state.enhanced_repositories.iter()
                            .filter(|r| r.name.to_lowercase().contains(&state.repository_filter.search_term.to_lowercase()))
                            .cloned()
                            .collect()
                    }
                } else {
                    // Fallback to basic repositories if enhanced are not available
                    let basic_repos: Vec<RepoInfo> = if state.repository_filter.search_term.is_empty() {
                        state.repositories.clone()
                    } else {
                        state.repositories.iter()
                            .filter(|r| r.name.to_lowercase().contains(&state.repository_filter.search_term.to_lowercase()))
                            .cloned()
                            .collect()
                    };
                    
                    // Convert basic repos to enhanced format for display
                    return render_basic_repos(ui, basic_repos, state, Arc::clone(&repo_manager));
                };
                
                if repos_to_display.is_empty() {
                    ui.label("No repositories found");
                    ui.label("");
                    ui.label("");
                    ui.label("");
                    ui.end_row();
                }
                
                for enhanced_repo in repos_to_display {
                    // Name column with enable/disable toggle
                    ui.horizontal(|ui| {
                        // Enable/disable checkbox for LLM context
                        let is_enabled = state.enabled_as_dependencies.contains(&enhanced_repo.name);
                        let mut checkbox_enabled = is_enabled;
                        if ui.checkbox(&mut checkbox_enabled, "")
                            .on_hover_text("Enable this repository as a dependency context for the LLM")
                            .changed() {
                            if checkbox_enabled {
                                state.enabled_as_dependencies.insert(enhanced_repo.name.clone());
                            } else {
                                state.enabled_as_dependencies.remove(&enhanced_repo.name);
                            }
                        }
                        
                        let is_selected = state.selected_repo.as_ref().map_or(false, |s| s == &enhanced_repo.name);
                        
                        // Style the name differently if the repository is missing
                        let name_text = if !enhanced_repo.filesystem_status.exists {
                            RichText::new(&enhanced_repo.name).color(Color32::from_rgb(220, 53, 69))
                        } else {
                            RichText::new(&enhanced_repo.name)
                        };
                        
                        if ui.selectable_label(is_selected, name_text).clicked() {
                            if is_selected {
                                state.selected_repo = None;
                                // Also remove from selected_repos
                                state.selected_repos.retain(|name| name != &enhanced_repo.name);
                            } else {
                                state.selected_repo = Some(enhanced_repo.name.clone());
                                // Also add to selected_repos if not already there
                                if !state.selected_repos.contains(&enhanced_repo.name) {
                                    state.selected_repos.push(enhanced_repo.name.clone());
                                }
                                
                                // Initialize options for other tabs
                                state.query_options = super::types::QueryOptions::new(enhanced_repo.name.clone());
                                state.file_search_options = super::types::FileSearchOptions::new(enhanced_repo.name.clone());
                                state.file_view_options = super::types::FileViewOptions::new(enhanced_repo.name.clone());
                            }
                        }
                    });
                    
                    // Source column
                    let source_text = if let Some(remote) = &enhanced_repo.remote {
                        if remote.is_empty() {
                            if let Some(path) = &enhanced_repo.local_path {
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
                    
                    let source = if let Some(branch) = &enhanced_repo.branch {
                        format!("{} ({})", source_text, branch)
                    } else {
                        source_text
                    };
                    
                    ui.label(source);
                    
                    // Status column with enhanced information
                    ui.horizontal(|ui| {
                        let (status_icon, status_color) = get_sync_status_indicator(&enhanced_repo.sync_status.state);
                        
                        // Draw the status icon and get its response for hover detection
                        let status_response = ui.colored_label(status_color, &status_icon);
                        
                        // Show tooltip when hovering over the status icon
                        if status_response.hovered() {
                            egui::show_tooltip_at_pointer(ui.ctx(), egui::layers::LayerId::debug(), egui::Id::new("repo_status_tooltip"), |ui| {
                                show_repo_status_tooltip(ui, &enhanced_repo);
                            });
                        }
                        
                        // Show basic status info inline
                        if let Some(files) = enhanced_repo.total_files {
                            let size_text = enhanced_repo.size_bytes
                                .map(|size| format!(" ({})", format_bytes(size)))
                                .unwrap_or_default();
                            ui.label(format!("{} files{}", files, size_text));
                        }
                    });
                    
                    // Actions column
                    ui.horizontal(|ui| {
                        if !enhanced_repo.filesystem_status.exists {
                            // Repository is missing - show reclone button
                            if !enhanced_repo.added_as_local_path {
                                if ui.button("Reclone").clicked() {
                                    let repo_name = enhanced_repo.name.clone();
                                    let repo_manager_clone = Arc::clone(&repo_manager);
                                    let handle = tokio::runtime::Handle::current();
                                    
                                    handle.spawn(async move {
                                        let mut manager = repo_manager_clone.lock().await;
                                        let _ = manager.reclone_repository(&repo_name).await;
                                    });
                                }
                            } else {
                                // Repository was added as local path - can't reclone
                                ui.label("Local path");
                            }
                        } else {
                            // Repository exists - show normal actions
                            if ui.button("Query").clicked() {
                                state.active_tab = super::types::RepoPanelTab::Query;
                                state.selected_repo = Some(enhanced_repo.name.clone());
                                state.query_options = super::types::QueryOptions::new(enhanced_repo.name.clone());
                            }
                            
                            if ui.button("Files").clicked() {
                                state.active_tab = super::types::RepoPanelTab::SearchFile;
                                state.selected_repo = Some(enhanced_repo.name.clone());
                                state.file_search_options = super::types::FileSearchOptions::new(enhanced_repo.name.clone());
                            }
                            
                            if ui.button("Sync").clicked() {
                                state.active_tab = super::types::RepoPanelTab::Sync;
                                state.selected_repo = Some(enhanced_repo.name.clone());
                                state.sync_options.repository_name = enhanced_repo.name.clone();
                            }
                        }
                        
                        // Remove button is always available
                        if ui.button("Remove").clicked() {
                            // Set up the remove
                            let repo_name = enhanced_repo.name.clone();
                            let repo_name_for_async = repo_name.clone();
                            
                            // Schedule the remove operation
                            let repo_manager_clone = Arc::clone(&repo_manager);
                            let handle = tokio::runtime::Handle::current();
                            
                            handle.spawn(async move {
                                let mut manager = repo_manager_clone.lock().await;
                                let _ = manager.remove_repository(&repo_name_for_async).await;
                            });
                            
                            // Also remove from UI state immediately for responsiveness
                            state.enhanced_repositories.retain(|r| r.name != repo_name);
                            state.repositories.retain(|r| r.name != repo_name);
                            if state.selected_repo.as_ref() == Some(&repo_name) {
                                state.selected_repo = None;
                            }
                            state.selected_repos.retain(|name| name != &repo_name);
                            state.enabled_as_dependencies.remove(&repo_name);
                        }
                    });
                    
                    ui.end_row();
                }
            });
    });
    
    // Display orphaned repositories if any
    if !state.orphaned_repositories.is_empty() {
        ui.separator();
        ui.heading("Orphaned Repositories");
        ui.label("These directories exist on the filesystem but are not in your configuration:");
        
        ScrollArea::vertical().show(ui, |ui| {
            Grid::new("orphaned_repositories_grid")
                .num_columns(4)
                .striped(true)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    // Header
                    ui.label(RichText::new("Name").strong());
                    ui.label(RichText::new("Path").strong());
                    ui.label(RichText::new("Info").strong());
                    ui.label(RichText::new("Actions").strong());
                    ui.end_row();
                    
                    let orphaned_repos = state.orphaned_repositories.clone();
                    let mut to_remove = Vec::new();
                    
                    for orphan in &orphaned_repos {
                        // Name column
                        ui.label(RichText::new(&orphan.name).color(Color32::from_rgb(255, 193, 7)));
                        
                        // Path column
                        ui.label(orphan.local_path.to_string_lossy().to_string());
                        
                        // Info column
                        ui.horizontal(|ui| {
                            if orphan.is_git_repository {
                                ui.colored_label(Color32::from_rgb(46, 160, 67), "Git");
                            }
                            if let Some(file_count) = orphan.file_count {
                                ui.label(format!("{} files", file_count));
                            }
                            if let Some(size) = orphan.size_bytes {
                                ui.label(format_bytes(size));
                            }
                        });
                        
                        // Actions column
                        ui.horizontal(|ui| {
                            if ui.button("Add").clicked() {
                                let orphan_clone = orphan.clone();
                                let repo_manager_clone = Arc::clone(&repo_manager);
                                let handle = tokio::runtime::Handle::current();
                                
                                handle.spawn(async move {
                                    let manager = repo_manager_clone.lock().await;
                                    let orphaned_repo = sagitta_search::OrphanedRepository {
                                        name: orphan_clone.name,
                                        local_path: orphan_clone.local_path,
                                        is_git_repository: orphan_clone.is_git_repository,
                                        remote_url: orphan_clone.remote_url,
                                        file_count: orphan_clone.file_count,
                                        size_bytes: orphan_clone.size_bytes,
                                    };
                                    let _ = manager.add_orphaned_repository(&orphaned_repo).await;
                                });
                                
                                // Mark for removal
                                to_remove.push(orphan.name.clone());
                            }
                            
                            if ui.button("Remove").clicked() {
                                let orphan_clone = orphan.clone();
                                let repo_manager_clone = Arc::clone(&repo_manager);
                                let handle = tokio::runtime::Handle::current();
                                
                                handle.spawn(async move {
                                    let manager = repo_manager_clone.lock().await;
                                    let orphaned_repo = sagitta_search::OrphanedRepository {
                                        name: orphan_clone.name,
                                        local_path: orphan_clone.local_path,
                                        is_git_repository: orphan_clone.is_git_repository,
                                        remote_url: orphan_clone.remote_url,
                                        file_count: orphan_clone.file_count,
                                        size_bytes: orphan_clone.size_bytes,
                                    };
                                    let _ = manager.remove_orphaned_repository(&orphaned_repo).await;
                                });
                                
                                // Mark for removal
                                to_remove.push(orphan.name.clone());
                            }
                        });
                        
                        ui.end_row();
                    }
                    
                    // Remove clicked items from the state
                    for name in to_remove {
                        state.orphaned_repositories.retain(|o| o.name != name);
                    }
                });
        });
    }
}

/// Fallback function to render basic repositories when enhanced data is not available
fn render_basic_repos(
    ui: &mut Ui,
    repos: Vec<RepoInfo>,
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
) {
    for repo in repos {
        // Name column with enable/disable toggle
        ui.horizontal(|ui| {
            // Enable/disable checkbox for LLM context
            let is_enabled = state.enabled_as_dependencies.contains(&repo.name);
            let mut checkbox_enabled = is_enabled;
            if ui.checkbox(&mut checkbox_enabled, "")
                .on_hover_text("Enable this repository as a dependency context for the LLM")
                .changed() {
                if checkbox_enabled {
                    state.enabled_as_dependencies.insert(repo.name.clone());
                } else {
                    state.enabled_as_dependencies.remove(&repo.name);
                }
            }
            
            let is_selected = state.selected_repo.as_ref().map_or(false, |s| s == &repo.name);
            
            if ui.selectable_label(is_selected, &repo.name).clicked() {
                if is_selected {
                    state.selected_repo = None;
                    state.selected_repos.retain(|name| name != &repo.name);
                } else {
                    state.selected_repo = Some(repo.name.clone());
                    if !state.selected_repos.contains(&repo.name) {
                        state.selected_repos.push(repo.name.clone());
                    }
                    
                    state.query_options = super::types::QueryOptions::new(repo.name.clone());
                    state.file_search_options = super::types::FileSearchOptions::new(repo.name.clone());
                    state.file_view_options = super::types::FileViewOptions::new(repo.name.clone());
                }
            }
        });
        
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
        
        // Basic status column (no enhanced data available)
        ui.label("Loading...");
        
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
                let repo_name = repo.name.clone();
                let repo_name_for_async = repo_name.clone();
                
                let repo_manager_clone = Arc::clone(&repo_manager);
                let handle = tokio::runtime::Handle::current();
                
                handle.spawn(async move {
                    let mut manager = repo_manager_clone.lock().await;
                    let _ = manager.remove_repository(&repo_name_for_async).await;
                });
                
                state.repositories.retain(|r| r.name != repo_name);
                if state.selected_repo.as_ref() == Some(&repo_name) {
                    state.selected_repo = None;
                }
                state.selected_repos.retain(|name| name != &repo_name);
                state.enabled_as_dependencies.remove(&repo_name);
            }
        });
        
        ui.end_row();
    }
} 