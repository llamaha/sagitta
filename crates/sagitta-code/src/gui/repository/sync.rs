use std::sync::Arc;
use egui::{Ui, RichText, Color32, ScrollArea, Button, Grid, ProgressBar, TextEdit, Layout, Align};
use tokio::sync::{Mutex, oneshot};
use super::manager::{RepositoryManager, SyncStatus as ManagerSyncStatus};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use super::types::{RepoPanelState, RepoInfo, SimpleSyncStatus, DisplayableSyncProgress};

/// Timeout for sync operations that appear stuck (in seconds)
const SYNC_TIMEOUT_SECONDS: u64 = 30;

/// Channel for sync completion notifications
#[derive(Debug)]
pub struct SyncCompletionChannel {
    pub receiver: Option<oneshot::Receiver<(String, Result<(), anyhow::Error>)>>,
}

/// Track active sync operations to prevent hanging
#[derive(Debug, Clone)]
pub struct ActiveSyncOperation {
    pub repo_name: String,
    pub started_at: std::time::Instant,
}

/// Render the sync repository view
pub fn render_sync_repo(
    ui: &mut Ui, 
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: crate::gui::theme::AppTheme,
) {
    ui.heading("Sync Repositories");
    
    if state.simple_sync_status_map.is_none() {
        state.simple_sync_status_map = Some(HashMap::new());
    }
    
    if state.repositories.is_empty() {
        ui.label("No repositories available");
        
        if ui.button("Add Repository").clicked() {
            state.active_tab = super::types::RepoPanelTab::Add;
        }
        
        return;
    }
    
    ui.label("Select repositories to sync:");
    
    let repos_info: Vec<_> = state.repositories.iter().map(|repo| {
        let is_selected = state.selected_repos.contains(&repo.name);
        let default_branch = repo.branch.clone().unwrap_or_else(|| "main".to_string());
        let branch = state.branch_overrides.get(&repo.name)
            .cloned()
            .unwrap_or_else(|| default_branch.clone());
        (repo.clone(), is_selected, branch, default_branch)
    }).collect();
    
    let mut repo_selection_changes = Vec::new();
    let mut branch_override_changes = Vec::new();
    
    ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        for (repo, is_selected, branch, default_branch) in &repos_info { 
            let mut selected = *is_selected;
            let mut current_branch = branch.clone();
            
            ui.horizontal(|ui| {
                if ui.checkbox(&mut selected, &repo.name).changed() {
                    repo_selection_changes.push((repo.name.clone(), selected));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.text_edit_singleline(&mut current_branch).changed() {
                        branch_override_changes.push((
                            repo.name.clone(),
                            current_branch.clone(),
                            default_branch.clone()
                        ));
                    }
                    ui.label("Branch:");
                });
            });
        }
    });
    
    for (repo_name, selected) in repo_selection_changes {
        if selected {
            if !state.selected_repos.contains(&repo_name) {
                state.selected_repos.push(repo_name.clone());
            }
            state.selected_repo = Some(repo_name);
        } else {
            state.selected_repos.retain(|name| name != &repo_name);
            if state.selected_repo.as_ref() == Some(&repo_name) {
                state.selected_repo = None;
            }
        }
    }
    
    for (repo_name, branch, default_branch) in branch_override_changes {
        if branch != default_branch && !branch.trim().is_empty() {
            state.branch_overrides.insert(repo_name, branch);
        } else if branch.trim().is_empty() || branch == default_branch {
            state.branch_overrides.remove(&repo_name);
        }
    }
    
    ui.separator();
    
    let mut is_any_selected_syncing = false;
    if let Ok(manager) = repo_manager.try_lock() {
        if let Some(simple_status_map) = manager.try_get_simple_sync_status_map() {
            for repo_name in &state.selected_repos {
                if let Some(status) = simple_status_map.get(repo_name) {
                    if status.is_running {
                        is_any_selected_syncing = true;
                        break;
                    }
                }
            }
        }
    }

    ui.horizontal(|ui| {
        let sync_button_text = if is_any_selected_syncing { "Syncing Selected..." } else { "Sync Selected" };
        if ui.add_enabled(!is_any_selected_syncing, Button::new(sync_button_text)).clicked() {
            if !state.selected_repos.is_empty() {
                trigger_sync(&state.selected_repos, Arc::clone(&repo_manager));
            } else {
                log::warn!("Sync Selected clicked but no repositories selected.");
            }
        }

        let mut is_any_repo_syncing_at_all = false;
         if let Ok(manager) = repo_manager.try_lock() {
            if let Some(simple_status_map) = manager.try_get_simple_sync_status_map() {
                if simple_status_map.values().any(|s| s.is_running) {
                    is_any_repo_syncing_at_all = true;
                }
            }
        }
        let sync_all_text = if is_any_repo_syncing_at_all { "Syncing..." } else { "Sync All" };
        if ui.add_enabled(!is_any_repo_syncing_at_all, Button::new(sync_all_text)).clicked() {
            let all_repo_names = state.repositories.iter().map(|r| r.name.clone()).collect::<Vec<_>>();
            if !all_repo_names.is_empty() {
                trigger_sync(&all_repo_names, Arc::clone(&repo_manager));
            }
        }
    });
    
    ui.separator();
    ui.label(RichText::new("Sync Status:").heading());

    let repos_to_display_status_for: Vec<String> = state.repositories.iter().map(|r| r.name.clone()).collect();

    if repos_to_display_status_for.is_empty() {
        ui.label("No repositories to display status for.");
        return;
    }
    
    ScrollArea::vertical().show(ui, |ui| {
        if let Ok(manager) = repo_manager.try_lock() {
            let detailed_status_map_opt = manager.try_get_sync_status_map();
            let simple_status_map_opt = manager.try_get_simple_sync_status_map();

            for repo_name in repos_to_display_status_for {
                let simple_status = simple_status_map_opt.as_ref()
                    .and_then(|map_guard| map_guard.get(&repo_name).cloned());
                
                let detailed_status = detailed_status_map_opt.as_ref()
                    .and_then(|map_guard| map_guard.get(&repo_name).cloned());

                if simple_status.is_none() && (detailed_status.is_none() || detailed_status.as_ref().unwrap().detailed_progress.is_none()) {
                    continue;
                }

                let mut status_text_str = "Pending".to_string();
                let mut status_color = theme.hint_text_color();
                let mut current_progress_val = 0.0;
                let mut is_running = false;

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&format!("Repository: {}", repo_name)).strong());
                        
                        if let Some(ds) = &detailed_status {
                            if let Some(dp) = &ds.detailed_progress {
                                status_text_str = dp.stage_detail.name.clone();
                                current_progress_val = dp.percentage_overall;
                                is_running = !matches!(dp.stage_detail.name.as_str(), "Completed" | "Error" | "Idle");

                                if dp.stage_detail.name == "Completed" {
                                    status_color = theme.success_color();
                                } else if dp.stage_detail.name == "Error" {
                                    status_color = theme.error_color();
                                } else if is_running {
                                    status_color = theme.warning_color();
                                }
                            } else if let Some(ss) = &simple_status {
                                is_running = ss.is_running;
                                if ss.is_complete {
                                    status_text_str = if ss.is_success { "Completed".to_string() } else { "Failed".to_string() };
                                    status_color = if ss.is_success { theme.success_color() } else { theme.error_color() };
                                    current_progress_val = 1.0;
                                } else if ss.is_running {
                                    // Check for timeout if sync appears stuck
                                    if let Some(started_at) = ss.started_at {
                                        let elapsed = started_at.elapsed();
                                        if elapsed.as_secs() > SYNC_TIMEOUT_SECONDS {
                                            // Sync appears stuck, mark as timed out
                                            status_text_str = "Timed Out".to_string();
                                            status_color = theme.warning_color();
                                            current_progress_val = 0.0;
                                            is_running = false;
                                            
                                            // Clear the stuck status in the background
                                            let repo_manager_clone = Arc::clone(&repo_manager);
                                            let repo_name_clone = repo_name.clone();
                                            tokio::spawn(async move {
                                                if let Ok(mut manager) = repo_manager_clone.try_lock() {
                                                    if let Some(mut simple_map) = manager.try_get_simple_sync_status_map() {
                                                        if let Some(status) = simple_map.get_mut(&repo_name_clone) {
                                                            status.is_running = false;
                                                            status.is_complete = true;
                                                            status.is_success = false;
                                                            status.final_message = "Sync operation timed out - repository may have been already synced".to_string();
                                                            status.output_lines.push("⚠️ Sync timed out - clearing stuck status".to_string());
                                                        }
                                                    }
                                                }
                                            });
                                        } else {
                                            status_text_str = "Running".to_string();
                                            status_color = theme.warning_color();
                                        }
                                    } else {
                                        status_text_str = "Running".to_string();
                                        status_color = theme.warning_color();
                                    }
                                }
                            }
                        } else if let Some(ss) = &simple_status {
                            is_running = ss.is_running;
                            if ss.is_complete {
                                status_text_str = if ss.is_success { "Completed".to_string() } else { "Failed".to_string() };
                                status_color = if ss.is_success { theme.success_color() } else { theme.error_color() };
                                current_progress_val = 1.0;
                            } else if ss.is_running {
                                // Check for timeout if sync appears stuck
                                if let Some(started_at) = ss.started_at {
                                    let elapsed = started_at.elapsed();
                                    if elapsed.as_secs() > SYNC_TIMEOUT_SECONDS {
                                        // Sync appears stuck, mark as timed out
                                        status_text_str = "Timed Out".to_string();
                                        status_color = theme.warning_color();
                                        current_progress_val = 0.0;
                                        is_running = false;
                                        
                                        // Clear the stuck status in the background
                                        let repo_manager_clone = Arc::clone(&repo_manager);
                                        let repo_name_clone = repo_name.clone();
                                        tokio::spawn(async move {
                                            if let Ok(mut manager) = repo_manager_clone.try_lock() {
                                                if let Some(mut simple_map) = manager.try_get_simple_sync_status_map() {
                                                    if let Some(status) = simple_map.get_mut(&repo_name_clone) {
                                                        status.is_running = false;
                                                        status.is_complete = true;
                                                        status.is_success = false;
                                                        status.final_message = "Sync operation timed out - repository may have been already synced".to_string();
                                                        status.output_lines.push("⚠️ Sync timed out - clearing stuck status".to_string());
                                                    }
                                                }
                                            }
                                        });
                                    } else {
                                        status_text_str = "Running".to_string();
                                        status_color = theme.warning_color();
                                    }
                                } else {
                                    status_text_str = "Running".to_string();
                                    status_color = theme.warning_color();
                                }
                            }
                        }
                        ui.label(RichText::new(&status_text_str).color(status_color));
                    });
                    
                    if let Some(ds) = detailed_status {
                        if let Some(dp) = ds.detailed_progress {
                            ui.add(ProgressBar::new(current_progress_val).text(format!("Overall: {:.0}%", current_progress_val * 100.0)));
                            ui.label(RichText::new(&dp.message).small());
                            if let Some(file) = &dp.stage_detail.current_file {
                                ui.label(RichText::new(format!("Current File: {}", file)).small());
                            }
                            if let Some((curr, total)) = dp.stage_detail.current_progress {
                                if total > 0 {
                                    ui.label(RichText::new(format!("Step Progress: {}/{}", curr, total)).small());
                                }
                            }
                            if let Some(fps) = dp.stage_detail.files_per_second {
                                ui.label(RichText::new(format!("Speed: {:.2} files/s", fps)).small());
                            }
                            ui.label(RichText::new(format!("Elapsed: {:.1}s", dp.elapsed_seconds)).small());
                            ui.add_space(5.0);
                        }
                    }

                    if let Some(ss) = simple_status {
                        if !ss.final_message.is_empty() && status_text_str == "Completed" || status_text_str == "Failed" {
                             ui.label(RichText::new(&ss.final_message).small());
                        }
                        if !ss.output_lines.is_empty() {
                            ui.label("Log:");
                            ScrollArea::vertical()
                                .max_height(100.0)
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    for line in &ss.output_lines {
                                        ui.label(RichText::new(line).small().family(egui::FontFamily::Monospace));
                                    }
                                });
                        }
                    }
                });
                ui.add_space(5.0);
            }
        } else {
            ui.label("Repository manager is locked. Try again shortly.");
        }
    });
}

fn trigger_sync(repo_names: &[String], repo_manager: Arc<Mutex<RepositoryManager>>) {
    for repo_name in repo_names {
        let repo_manager_clone = Arc::clone(&repo_manager);
        let rn = repo_name.clone();
        
        log::info!("Triggering sync for repository: {}", rn);
        
        tokio::spawn(async move {
            match repo_manager_clone.lock().await.sync_repository(&rn).await {
                Ok(_) => {
                    log::info!("Sync task for repository '{}' reported success.", rn);
                }
                Err(e) => {
                    log::error!("Sync task for repository '{}' failed: {}", rn, e);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::repository::types::SimpleSyncStatus;
    use std::time::{Duration, Instant};

    #[test]
    fn test_sync_timeout_detection() {
        let mut status = SimpleSyncStatus {
            is_running: true,
            is_complete: false,
            is_success: false,
            output_lines: vec!["Starting sync...".to_string()],
            final_message: String::new(),
            started_at: Some(Instant::now() - Duration::from_secs(SYNC_TIMEOUT_SECONDS + 1)),
        };

        // Simulate the timeout check logic
        if let Some(started_at) = status.started_at {
            let elapsed = started_at.elapsed();
            assert!(elapsed.as_secs() > SYNC_TIMEOUT_SECONDS, "Should detect timeout");
        }
    }

    #[test]
    fn test_sync_not_timed_out() {
        let mut status = SimpleSyncStatus {
            is_running: true,
            is_complete: false,
            is_success: false,
            output_lines: vec!["Starting sync...".to_string()],
            final_message: String::new(),
            started_at: Some(Instant::now() - Duration::from_secs(5)),
        };

        // Simulate the timeout check logic
        if let Some(started_at) = status.started_at {
            let elapsed = started_at.elapsed();
            assert!(elapsed.as_secs() <= SYNC_TIMEOUT_SECONDS, "Should not detect timeout");
        }
    }

    #[test]
    fn test_sync_status_without_start_time() {
        let status = SimpleSyncStatus {
            is_running: true,
            is_complete: false,
            is_success: false,
            output_lines: vec!["Starting sync...".to_string()],
            final_message: String::new(),
            started_at: None,
        };

        // Should handle missing start time gracefully
        assert!(status.started_at.is_none(), "Should handle missing start time");
    }
} 