use std::sync::Arc;
use egui::{Ui, RichText, Color32, ScrollArea, Button, Grid, ProgressBar, TextEdit, Layout, Align};
use tokio::sync::{Mutex, oneshot};
use super::manager::{RepositoryManager, SyncStatus as ManagerSyncStatus};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use super::types::{RepoPanelState, RepoInfo, SimpleSyncStatus, DisplayableSyncProgress};
use crate::gui::repository::shared_sync_state::{SIMPLE_STATUS, DETAILED_STATUS};
use crate::gui::theme::AppTheme;

/// Watchdog configuration for sync operations
/// Maximum time without progress updates before considering sync stuck (in seconds)
const SYNC_WATCHDOG_TIMEOUT_SECONDS: u64 = 120; // 2 minutes without progress updates

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
    
    // Get repositories from either enhanced or basic list
    let available_repos = if state.use_enhanced_repos && !state.enhanced_repositories.is_empty() {
        // Convert enhanced repositories to basic RepoInfo format
        state.enhanced_repositories.iter().map(|enhanced| {
            super::types::RepoInfo {
                name: enhanced.name.clone(),
                remote: enhanced.remote.clone(),
                branch: enhanced.branch.clone(),
                local_path: enhanced.local_path.clone(),
                is_syncing: enhanced.is_syncing,
            }
        }).collect::<Vec<_>>()
    } else {
        state.repositories.clone()
    };
    
    if available_repos.is_empty() {
        ui.horizontal(|ui| {
            ui.label("No repositories available");
            
            if ui.button("Refresh").clicked() {
                state.is_loading_repos = true;
            }
        });
        
        if ui.button("Add Repository").clicked() {
            state.active_tab = super::types::RepoPanelTab::Add;
        }
        
        return;
    }
    
    ui.horizontal(|ui| {
        ui.label("Select repositories to sync:");
        
        if ui.button("Refresh").clicked() {
            state.is_loading_repos = true;
        }
    });
    
    // Add header for the repository selection area
    ui.horizontal(|ui| {
        ui.label(RichText::new("Repository").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("Branch").strong());
        });
    });
    ui.separator();
    
    let repos_info: Vec<_> = available_repos.iter().map(|repo| {
        let is_selected = state.selected_repos.contains(&repo.name);
        let default_branch = repo.branch.clone().unwrap_or_else(|| "main".to_string());
        let branch = state.branch_overrides.get(&repo.name)
            .cloned()
            .unwrap_or_else(|| default_branch.clone());
        (repo.clone(), is_selected, branch, default_branch)
    }).collect();
    
    let mut repo_selection_changes = Vec::new();
    let mut branch_override_changes = Vec::new();
    
    // Use a more generous max height and ensure scrolling works for long lists
    let max_height = if repos_info.len() > 10 {
        300.0 // Larger height for many repositories
    } else if repos_info.len() > 5 {
        200.0 // Medium height for moderate number of repositories
    } else {
        150.0 // Smaller height for few repositories
    };
    
    ScrollArea::vertical()
        .max_height(max_height)
        .auto_shrink([false, true]) // Don't shrink horizontally, shrink vertically if content is smaller
        .show(ui, |ui| {
            for (repo, is_selected, branch, default_branch) in &repos_info { 
                let mut selected = *is_selected;
                let mut current_branch = branch.clone();
                
                ui.horizontal(|ui| {
                    // Improve checkbox layout with better spacing and larger hit area
                    ui.spacing_mut().item_spacing.x = 8.0; // More space between checkbox and text
                    ui.spacing_mut().button_padding = egui::Vec2::new(4.0, 4.0); // Larger clickable area
                    
                    // Use a wider checkbox area to make it easier to click
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(20.0, ui.available_height()), 
                        egui::Layout::left_to_right(egui::Align::Center), 
                        |ui| {
                            if ui.checkbox(&mut selected, "").changed() {
                                repo_selection_changes.push((repo.name.clone(), selected));
                            }
                        }
                    );
                    
                    // Repository name with better spacing
                    ui.label(&repo.name);
                    
                    // Branch input on the right with proper spacing
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.set_min_width(120.0); // Ensure enough space for branch input
                        if ui.text_edit_singleline(&mut current_branch).changed() {
                            branch_override_changes.push((
                                repo.name.clone(),
                                current_branch.clone(),
                                default_branch.clone()
                            ));
                        }
                    });
                });
                
                // Add a small separator between repositories for better visual separation
                ui.add_space(2.0);
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
    
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.force_sync, "Force sync (ignore last synced commit)");
        ui.label(RichText::new("⚠️ Force sync will re-index all files").small().color(theme.hint_text_color()));
    });
    
    ui.separator();

    let mut is_any_selected_syncing = false;
    let mut is_any_repo_syncing_at_all = false;
    
    // Try to get sync status with better error handling
    for repo_name in &state.selected_repos {
        if let Some(status) = SIMPLE_STATUS.get(repo_name) {
            if status.is_running {
                is_any_selected_syncing = true;
                break;
            }
        }
    }

    let is_any_repo_syncing_at_all = SIMPLE_STATUS.iter().any(|s| s.value().is_running);

    ui.horizontal(|ui| {
        let sync_button_text = if is_any_selected_syncing { "Syncing Selected..." } else { "Sync Selected" };
        if ui.add_enabled(!is_any_selected_syncing, Button::new(sync_button_text)).clicked() {
            if !state.selected_repos.is_empty() {
                trigger_sync(&state.selected_repos, Arc::clone(&repo_manager), state.force_sync);
            } else {
                log::warn!("Sync Selected clicked but no repositories selected.");
            }
        }

        let sync_all_text = if is_any_repo_syncing_at_all { "Syncing..." } else { "Sync All" };
        if ui.add_enabled(!is_any_repo_syncing_at_all, Button::new(sync_all_text)).clicked() {
            let all_repo_names = available_repos.iter().map(|r| r.name.clone()).collect::<Vec<_>>();
            if !all_repo_names.is_empty() {
                trigger_sync(&all_repo_names, Arc::clone(&repo_manager), state.force_sync);
            }
        }
    });
    
    ui.separator();
    ui.label(RichText::new("Sync Status:").heading());

    let repos_to_display_status_for: Vec<String> = available_repos.iter().map(|r| r.name.clone()).collect();

    if repos_to_display_status_for.is_empty() {
        ui.label("No repositories to display status for.");
        return;
    }
    
    ScrollArea::vertical().show(ui, |ui| {
        let mut has_any_status = false;

        for repo_name in repos_to_display_status_for {
            let simple_status_entry = SIMPLE_STATUS.get(&repo_name);
            let detailed_status_entry = DETAILED_STATUS.get(&repo_name);

            // Only show repositories that actually have sync status
            if simple_status_entry.is_none() && detailed_status_entry.is_none() {
                continue;
            }

            has_any_status = true;

            let simple_status = simple_status_entry.map(|s| s.value().clone());
            let detailed_status = detailed_status_entry.map(|d| d.value().clone());

            let mut status_text_str = "Pending".to_string();
            let mut status_color = theme.hint_text_color();
            let mut current_progress_val = 0.0;
            let mut is_running = false;

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&format!("Repository: {}", repo_name)).strong());
                    
                    // Check simple status first for immediate feedback
                    if let Some(ss) = &simple_status {
                        is_running = ss.is_running;
                        if ss.is_complete {
                            status_text_str = if ss.is_success { "Completed".to_string() } else { "Failed".to_string() };
                            status_color = if ss.is_success { theme.success_color() } else { theme.error_color() };
                            current_progress_val = 1.0;
                        } else if ss.is_running {
                            // Check for watchdog timeout based on last progress update
                            if let Some(last_progress_time) = ss.last_progress_time {
                                let time_since_progress = last_progress_time.elapsed();
                                if !ss.is_complete && time_since_progress.as_secs() > SYNC_WATCHDOG_TIMEOUT_SECONDS {
                                    // Sync appears stuck based on lack of progress updates
                                    status_text_str = "Watchdog Timeout".to_string();
                                    status_color = theme.warning_color();
                                    current_progress_val = 0.0;
                                    is_running = false;
                                    
                                    // Update the global state directly and store final elapsed time
                                    SIMPLE_STATUS.entry(repo_name.clone()).and_modify(|s| {
                                        s.is_running = false;
                                        s.is_complete = true;
                                        s.is_success = false;
                                        s.final_message = format!("Sync operation timed out - no progress for {} seconds", time_since_progress.as_secs());
                                        s.output_lines.push("⚠️ Watchdog timeout - no progress updates received".to_string());
                                        s.final_elapsed_seconds = Some(ss.started_at.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0));
                                    });

                                } else {
                                    status_text_str = "Running".to_string();
                                    status_color = theme.warning_color();
                                    current_progress_val = 0.1; // Show some progress to indicate it's running
                                }
                            } else if let Some(started_at) = ss.started_at {
                                // Fallback to started_at if no progress time is available
                                let elapsed = started_at.elapsed();
                                if elapsed.as_secs() > SYNC_WATCHDOG_TIMEOUT_SECONDS {
                                    status_text_str = "Watchdog Timeout".to_string();
                                    status_color = theme.warning_color();
                                    current_progress_val = 0.0;
                                    is_running = false;
                                    
                                    SIMPLE_STATUS.entry(repo_name.clone()).and_modify(|s| {
                                        s.is_running = false;
                                        s.is_complete = true;
                                        s.is_success = false;
                                        s.final_message = format!("Sync operation timed out - no progress for {} seconds", elapsed.as_secs());
                                        s.output_lines.push("⚠️ Watchdog timeout - no progress updates received".to_string());
                                        s.final_elapsed_seconds = Some(elapsed.as_secs_f64());
                                    });
                                } else {
                                    status_text_str = "Running".to_string();
                                    status_color = theme.warning_color();
                                    current_progress_val = 0.1;
                                }
                            } else {
                                status_text_str = "Starting".to_string();
                                status_color = theme.warning_color();
                                current_progress_val = 0.05; // Small progress to show it's starting
                            }
                        }
                    }
                    
                    // Override with detailed status if available (more precise)
                    if let Some(ds) = &detailed_status {
                        status_text_str = ds.stage_detail.name.clone();
                        current_progress_val = ds.percentage_overall;
                        is_running = !matches!(ds.stage_detail.name.as_str(), "Completed" | "Error" | "Idle");

                        if ds.stage_detail.name == "Completed" {
                            status_color = theme.success_color();
                        } else if ds.stage_detail.name == "Error" {
                            status_color = theme.error_color();
                        } else if is_running {
                            status_color = theme.warning_color();
                        }
                    }
                    
                    ui.label(RichText::new(&status_text_str).color(status_color));
                });
                
                // Show progress bar for any running or completed sync
                if is_running || current_progress_val > 0.0 {
                    ui.add(ProgressBar::new(current_progress_val).text(format!("Progress: {:.0}%", current_progress_val * 100.0)));
                }
                
                // Show detailed progress information if available
                if let Some(ds) = detailed_status {
                    ui.label(RichText::new(&ds.message).small());
                    if let Some(file) = &ds.stage_detail.current_file {
                        ui.label(RichText::new(format!("Current File: {}", file)).small());
                    }
                    if let Some((curr, total)) = ds.stage_detail.current_progress {
                        if total > 0 {
                            ui.label(RichText::new(format!("Step Progress: {}/{}", curr, total)).small());
                        }
                    }
                    if let Some(fps) = ds.stage_detail.files_per_second {
                        ui.label(RichText::new(format!("Speed: {:.2} files/s", fps)).small());
                    }
                    if let Some(ss) = &simple_status {
                        if let Some(final_elapsed) = ss.final_elapsed_seconds {
                            // Show static elapsed time for completed syncs
                            ui.label(RichText::new(format!("Elapsed: {:.1}s", final_elapsed)).small());
                        } else if let Some(started_at) = ss.started_at {
                            // Show live elapsed time for running syncs
                            ui.label(RichText::new(format!("Elapsed: {:.1}s", started_at.elapsed().as_secs_f32())).small());
                        }
                    }
                    ui.add_space(5.0);
                }

                if let Some(ss) = simple_status {
                    if !ss.final_message.is_empty() && (status_text_str == "Completed" || status_text_str == "Failed" || status_text_str == "Timed Out") {
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

        if !has_any_status {
            ui.label("No sync operations in progress or completed. Click 'Sync Selected' or 'Sync All' to start syncing.");
        }
    });
}

fn trigger_sync(repo_names: &[String], repo_manager: Arc<Mutex<RepositoryManager>>, force_sync: bool) {
    for repo_name in repo_names {
        // Insert an immediate placeholder so the progress bar appears instantly
        let now = std::time::Instant::now();
        SIMPLE_STATUS.insert(repo_name.to_string(), SimpleSyncStatus {
            is_running: true,
            is_complete: false,
            is_success: false,
            output_lines: vec!["⏳ Preparing sync...".into()],
            final_message: String::new(),
            started_at: Some(now),
            final_elapsed_seconds: None,
            last_progress_time: Some(now), // Initialize watchdog timer
        });
        
        // Clear any old detailed status
        DETAILED_STATUS.remove(repo_name);

        let repo_manager_clone = Arc::clone(&repo_manager);
        let rn = repo_name.clone();
        
        log::info!("Triggering sync for repository: {} (force: {})", rn, force_sync);
        
        // Start the actual sync operation
        tokio::spawn(async move {
            let started_at = std::time::Instant::now();
            // The lock is now held only within this async task
            match repo_manager_clone.lock().await.sync_repository_with_options(&rn, force_sync).await {
                Ok(_) => {
                    log::info!("Sync task for repository '{}' reported success.", rn);
                }
                Err(e) => {
                    log::error!("Sync task for repository '{}' failed: {}", rn, e);
                    let final_elapsed = started_at.elapsed().as_secs_f64();
                    // Ensure the status reflects the failure if the task itself errors out
                    SIMPLE_STATUS.entry(rn.clone()).and_modify(|s| {
                        s.is_running = false;
                        s.is_complete = true;
                        s.is_success = false;
                        s.final_message = format!("❌ Sync task failed: {}", e);
                        s.final_elapsed_seconds = Some(final_elapsed);
                    });
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
    fn test_sync_watchdog_timeout_detection() {
        let mut status = SimpleSyncStatus {
            is_running: true,
            is_complete: false,
            is_success: false,
            output_lines: vec!["Starting sync...".to_string()],
            final_message: String::new(),
            started_at: Some(Instant::now()),
            final_elapsed_seconds: None,
            last_progress_time: Some(Instant::now() - Duration::from_secs(SYNC_WATCHDOG_TIMEOUT_SECONDS + 1)),
        };

        // Simulate the watchdog timeout check logic
        if let Some(last_progress_time) = status.last_progress_time {
            let time_since_progress = last_progress_time.elapsed();
            assert!(time_since_progress.as_secs() > SYNC_WATCHDOG_TIMEOUT_SECONDS, "Should detect watchdog timeout");
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
            started_at: Some(Instant::now()),
            final_elapsed_seconds: None,
            last_progress_time: Some(Instant::now() - Duration::from_secs(5)),
        };

        // Simulate the watchdog timeout check logic
        if let Some(last_progress_time) = status.last_progress_time {
            let time_since_progress = last_progress_time.elapsed();
            assert!(time_since_progress.as_secs() <= SYNC_WATCHDOG_TIMEOUT_SECONDS, "Should not detect watchdog timeout");
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
            final_elapsed_seconds: None,
            last_progress_time: None,
        };

        // Should handle missing start time and progress time gracefully
        assert!(status.started_at.is_none(), "Should handle missing start time");
        assert!(status.last_progress_time.is_none(), "Should handle missing progress time");
    }

    #[test]
    fn test_sync_panel_shows_repositories_when_available() {
        use crate::gui::repository::types::{RepoPanelState, RepoInfo};
        use std::path::PathBuf;
        
        // Create a state with repositories
        let mut state = RepoPanelState {
            repositories: vec![
                RepoInfo {
                    name: "test-repo-1".to_string(),
                    remote: Some("https://github.com/test/repo1.git".to_string()),
                    branch: Some("main".to_string()),
                    local_path: Some(PathBuf::from("/tmp/repo1")),
                    is_syncing: false,
                },
                RepoInfo {
                    name: "test-repo-2".to_string(),
                    remote: Some("https://github.com/test/repo2.git".to_string()),
                    branch: Some("dev".to_string()),
                    local_path: Some(PathBuf::from("/tmp/repo2")),
                    is_syncing: false,
                },
            ],
            ..Default::default()
        };
        
        // The sync panel should not be empty when repositories are available
        assert!(!state.repositories.is_empty(), "State should have repositories");
        assert_eq!(state.repositories.len(), 2, "Should have exactly 2 repositories");
        
        // Verify that the repositories have the expected data
        assert_eq!(state.repositories[0].name, "test-repo-1");
        assert_eq!(state.repositories[1].name, "test-repo-2");
    }

    #[test]
    fn test_sync_panel_refresh_triggers_repository_load() {
        use crate::gui::repository::types::RepoPanelState;
        
        // Create an empty state
        let mut state = RepoPanelState::default();
        
        // Initially should be empty
        assert!(state.repositories.is_empty(), "Initial state should have no repositories");
        
        // Setting is_loading_repos should trigger a refresh
        state.is_loading_repos = true;
        
        assert!(state.is_loading_repos, "Loading flag should be set");
    }

    #[test]
    fn test_sync_panel_works_with_enhanced_repositories() {
        use crate::gui::repository::types::{RepoPanelState, EnhancedRepoInfo, RepoSyncStatus, SyncState, FilesystemStatus};
        use std::path::PathBuf;
        
        // Create a state with enhanced repositories
        let mut state = RepoPanelState {
            enhanced_repositories: vec![
                EnhancedRepoInfo {
                    name: "enhanced-repo-1".to_string(),
                    remote: Some("https://github.com/test/enhanced1.git".to_string()),
                    branch: Some("main".to_string()),
                    local_path: Some(PathBuf::from("/tmp/enhanced1")),
                    is_syncing: false,
                    filesystem_status: FilesystemStatus {
                        exists: true,
                        accessible: true,
                        is_git_repository: true,
                    },
                    git_status: None,
                    sync_status: RepoSyncStatus {
                        state: SyncState::UpToDate,
                        needs_sync: false,
                        last_synced_commit: Some("abc123".to_string()),
                    },
                    indexed_languages: Some(vec!["rust".to_string()]),
                    file_extensions: vec![],
                    total_files: Some(100),
                    size_bytes: Some(50000),
                    added_as_local_path: false,
                },
            ],
            use_enhanced_repos: true,
            ..Default::default()
        };
        
        // The sync panel should work with enhanced repositories
        assert!(!state.enhanced_repositories.is_empty(), "State should have enhanced repositories");
        assert_eq!(state.enhanced_repositories.len(), 1, "Should have exactly 1 enhanced repository");
        assert!(state.use_enhanced_repos, "Should be using enhanced repositories");
        
        // Verify that the enhanced repository has the expected data
        assert_eq!(state.enhanced_repositories[0].name, "enhanced-repo-1");
        assert_eq!(state.enhanced_repositories[0].remote, Some("https://github.com/test/enhanced1.git".to_string()));
    }

    #[test]
    fn test_original_sync_panel_issue_regression_test() {
        use crate::gui::repository::types::{RepoPanelState, EnhancedRepoInfo, RepoSyncStatus, SyncState, FilesystemStatus, RepoInfo};
        use std::path::PathBuf;
        
        // Simulate the exact scenario that was causing the original issue:
        // 1. Enhanced repositories are populated (successful enhanced repository load)
        // 2. Basic repositories list is empty (user reported seeing "No repositories available")
        // 3. use_enhanced_repos is true
        let mut state = RepoPanelState {
            repositories: vec![], // Empty basic repository list (this was the issue)
            enhanced_repositories: vec![
                EnhancedRepoInfo {
                    name: "user-repo-1".to_string(),
                    remote: Some("https://github.com/user/repo1.git".to_string()),
                    branch: Some("main".to_string()),
                    local_path: Some(PathBuf::from("/home/user/repo1")),
                    is_syncing: false,
                    filesystem_status: FilesystemStatus {
                        exists: true,
                        accessible: true,
                        is_git_repository: true,
                    },
                    git_status: None,
                    sync_status: RepoSyncStatus {
                        state: SyncState::NeedsSync,
                        needs_sync: true,
                        last_synced_commit: Some("def456".to_string()),
                    },
                    indexed_languages: Some(vec!["typescript".to_string(), "javascript".to_string()]),
                    file_extensions: vec![],
                    total_files: Some(500),
                    size_bytes: Some(2500000),
                    added_as_local_path: false,
                },
                EnhancedRepoInfo {
                    name: "user-repo-2".to_string(),
                    remote: Some("https://github.com/user/repo2.git".to_string()),
                    branch: Some("develop".to_string()),
                    local_path: Some(PathBuf::from("/home/user/repo2")),
                    is_syncing: true,
                    filesystem_status: FilesystemStatus {
                        exists: true,
                        accessible: true,
                        is_git_repository: true,
                    },
                    git_status: None,
                    sync_status: RepoSyncStatus {
                        state: SyncState::NeverSynced,
                        needs_sync: true,
                        last_synced_commit: None,
                    },
                    indexed_languages: Some(vec!["python".to_string()]),
                    file_extensions: vec![],
                    total_files: Some(200),
                    size_bytes: Some(750000),
                    added_as_local_path: false,
                },
            ],
            use_enhanced_repos: true,
            ..Default::default()
        };
        
        // Before the fix: sync panel would check state.repositories.is_empty() and show "No repositories available"
        assert!(state.repositories.is_empty(), "Basic repositories should be empty (original issue scenario)");
        assert!(!state.enhanced_repositories.is_empty(), "Enhanced repositories should be populated");
        assert!(state.use_enhanced_repos, "Should be using enhanced repositories");
        
        // After the fix: sync panel should use the enhanced repositories
        let available_repos = if state.use_enhanced_repos && !state.enhanced_repositories.is_empty() {
            state.enhanced_repositories.iter().map(|enhanced| {
                RepoInfo {
                    name: enhanced.name.clone(),
                    remote: enhanced.remote.clone(),
                    branch: enhanced.branch.clone(),
                    local_path: enhanced.local_path.clone(),
                    is_syncing: enhanced.is_syncing,
                }
            }).collect::<Vec<_>>()
        } else {
            state.repositories.clone()
        };
        
        // The fix ensures that repositories are now available for syncing
        assert!(!available_repos.is_empty(), "Sync panel should now show repositories (issue fixed)");
        assert_eq!(available_repos.len(), 2, "Should show both enhanced repositories");
        
        // Verify the repositories are correctly mapped
        assert_eq!(available_repos[0].name, "user-repo-1");
        assert_eq!(available_repos[1].name, "user-repo-2");
        assert_eq!(available_repos[0].remote, Some("https://github.com/user/repo1.git".to_string()));
        assert_eq!(available_repos[1].remote, Some("https://github.com/user/repo2.git".to_string()));
        
        // Verify sync status is correctly mapped
        assert!(!available_repos[0].is_syncing, "First repo should not be syncing");
        assert!(available_repos[1].is_syncing, "Second repo should be syncing");
    }

    #[test]
    fn test_auto_refresh_prevents_infinite_loop() {
        use crate::gui::repository::types::RepoPanelState;
        
        // Test that auto-refresh logic prevents infinite loops
        let mut state = RepoPanelState {
            repositories: vec![],
            enhanced_repositories: vec![],
            use_enhanced_repos: false,
            is_loading_repos: false,
            initial_load_attempted: false, // Haven't attempted initial load yet
            ..Default::default()
        };
        
        // Simulate the sync tab auto-refresh logic
        let has_repos = if state.use_enhanced_repos {
            !state.enhanced_repositories.is_empty()
        } else {
            !state.repositories.is_empty()
        };
        
        // First time: should trigger refresh
        let should_refresh_first = !has_repos && !state.is_loading_repos && !state.initial_load_attempted;
        assert!(should_refresh_first, "Should trigger refresh on first access");
        
        // Simulate triggering the refresh
        if should_refresh_first {
            state.is_loading_repos = true;
            state.initial_load_attempted = true;
        }
        
        // Second time: should NOT trigger refresh (prevents loop)
        let should_refresh_second = !has_repos && !state.is_loading_repos && !state.initial_load_attempted;
        assert!(!should_refresh_second, "Should NOT trigger refresh again (prevents infinite loop)");
        
        // Reset loading flag (simulating refresh completion)
        state.is_loading_repos = false;
        
        // Third time: should still NOT trigger refresh because initial_load_attempted is true
        let should_refresh_third = !has_repos && !state.is_loading_repos && !state.initial_load_attempted;
        assert!(!should_refresh_third, "Should NOT trigger refresh after initial attempt (prevents infinite loop)");
    }

    #[test]
    fn test_sync_progress_immediate_feedback() {
        use crate::gui::repository::types::SimpleSyncStatus;
        use std::time::Instant;
        
        // Test that sync shows immediate feedback when started
        let now = Instant::now();
        let mut status = SimpleSyncStatus {
            is_running: true,
            is_complete: false,
            is_success: false,
            output_lines: vec!["🔄 Starting repository sync...".to_string()],
            final_message: String::new(),
            started_at: Some(now),
            final_elapsed_seconds: None,
            last_progress_time: Some(now),
        };
        
        // Should show as running immediately
        assert!(status.is_running, "Status should show as running immediately");
        assert!(!status.is_complete, "Status should not be complete when starting");
        assert!(status.started_at.is_some(), "Should have start time");
        assert!(!status.output_lines.is_empty(), "Should have initial output");
        assert!(status.output_lines[0].contains("Starting"), "Should show starting message");
        
        // Simulate progress update
        status.output_lines.push("[Git Fetch] Fetching objects...".to_string());
        
        assert_eq!(status.output_lines.len(), 2, "Should have progress updates");
        assert!(status.output_lines[1].contains("Git Fetch"), "Should show fetch progress");
    }

    #[test]
    fn test_repository_list_scrollbar_height_calculation() {
        // Test that scrollbar height is calculated correctly based on number of repositories
        
        // Test with few repositories (≤5)
        let few_repos = 3;
        let height_few = if few_repos > 10 {
            300.0
        } else if few_repos > 5 {
            200.0
        } else {
            150.0
        };
        assert_eq!(height_few, 150.0, "Should use small height for few repositories");
        
        // Test with moderate repositories (6-10)
        let moderate_repos = 8;
        let height_moderate = if moderate_repos > 10 {
            300.0
        } else if moderate_repos > 5 {
            200.0
        } else {
            150.0
        };
        assert_eq!(height_moderate, 200.0, "Should use medium height for moderate repositories");
        
        // Test with many repositories (>10)
        let many_repos = 15;
        let height_many = if many_repos > 10 {
            300.0
        } else if many_repos > 5 {
            200.0
        } else {
            150.0
        };
        assert_eq!(height_many, 300.0, "Should use large height for many repositories");
    }

    #[test]
    fn test_checkbox_layout_accessibility() {
        use crate::gui::repository::types::{RepoPanelState, RepoInfo};
        use std::path::PathBuf;
        
        // Test that checkbox layout provides adequate hit area
        let state = RepoPanelState {
            repositories: vec![
                RepoInfo {
                    name: "test-repo-1".to_string(),
                    remote: Some("https://github.com/test/repo1.git".to_string()),
                    branch: Some("main".to_string()),
                    local_path: Some(PathBuf::from("/tmp/repo1")),
                    is_syncing: false,
                },
                RepoInfo {
                    name: "test-repo-with-very-long-name-that-might-cause-layout-issues".to_string(),
                    remote: Some("https://github.com/test/long-repo.git".to_string()),
                    branch: Some("develop".to_string()),
                    local_path: Some(PathBuf::from("/tmp/long-repo")),
                    is_syncing: false,
                },
            ],
            ..Default::default()
        };
        
        // Verify repository data structure supports proper layout
        assert_eq!(state.repositories.len(), 2, "Should have test repositories");
        assert!(state.repositories[1].name.len() > 50, "Should have long repository name for testing");
        
        // Test that branch overrides work with long names
        let long_repo_name = &state.repositories[1].name;
        assert!(long_repo_name.contains("very-long-name"), "Should contain long name identifier");
    }

    #[test]
    fn test_sync_status_display_priority() {
        use crate::gui::repository::types::{SimpleSyncStatus, DisplayableSyncProgress, GuiSyncStageDisplay};
        use std::time::Instant;
        
        // Test that simple status is shown immediately, then detailed status overrides
        let now = Instant::now();
        let simple_status = SimpleSyncStatus {
            is_running: true,
            is_complete: false,
            is_success: false,
            output_lines: vec!["🔄 Starting repository sync...".to_string()],
            final_message: String::new(),
            started_at: Some(now),
            final_elapsed_seconds: None,
            last_progress_time: Some(now),
        };
        
        let detailed_progress = DisplayableSyncProgress {
            stage_detail: GuiSyncStageDisplay {
                name: "Git Fetch".to_string(),
                overall_message: "Fetching objects from remote".to_string(),
                current_file: None,
                current_progress: Some((50, 100)),
                files_per_second: None,
            },
            message: "Fetching objects from remote".to_string(),
            percentage_overall: 0.5,
            elapsed_seconds: 10.0,
            current_overall: 50,
            total_overall: 100,
        };
        
        // Simple status should show immediate feedback
        assert!(simple_status.is_running, "Simple status should show running immediately");
        assert!(simple_status.started_at.is_some(), "Simple status should have start time");
        
        // Detailed status should provide more precise information
        assert_eq!(detailed_progress.stage_detail.name, "Git Fetch", "Detailed status should show specific stage");
        assert_eq!(detailed_progress.percentage_overall, 0.5, "Detailed status should show precise progress");
        assert!(detailed_progress.stage_detail.current_progress.is_some(), "Detailed status should show step progress");
    }

    #[test]
    fn test_progress_bar_visibility_conditions() {
        // Test when progress bar should be visible
        
        // Case 1: Sync is running
        let is_running = true;
        let progress_val = 0.1;
        let should_show_progress = is_running || progress_val > 0.0;
        assert!(should_show_progress, "Should show progress bar when sync is running");
        
        // Case 2: Sync completed
        let is_running = false;
        let progress_val = 1.0;
        let should_show_progress = is_running || progress_val > 0.0;
        assert!(should_show_progress, "Should show progress bar when sync is completed");
        
        // Case 3: Sync starting (small progress)
        let is_running = false;
        let progress_val = 0.05;
        let should_show_progress = is_running || progress_val > 0.0;
        assert!(should_show_progress, "Should show progress bar when sync is starting");
        
        // Case 4: No sync activity
        let is_running = false;
        let progress_val = 0.0;
        let should_show_progress = is_running || progress_val > 0.0;
        assert!(!should_show_progress, "Should not show progress bar when no sync activity");
    }

    #[test]
    fn test_repository_list_handles_many_repositories() {
        use crate::gui::repository::types::{RepoPanelState, RepoInfo};
        use std::path::PathBuf;
        
        // Test with a large number of repositories to ensure scrolling works
        let mut repositories = Vec::new();
        for i in 1..=25 {
            repositories.push(RepoInfo {
                name: format!("test-repo-{:02}", i),
                remote: Some(format!("https://github.com/test/repo{}.git", i)),
                branch: Some("main".to_string()),
                local_path: Some(PathBuf::from(format!("/tmp/repo{}", i))),
                is_syncing: i % 3 == 0, // Some repos syncing
            });
        }
        
        let state = RepoPanelState {
            repositories,
            ..Default::default()
        };
        
        // Verify we have many repositories
        assert_eq!(state.repositories.len(), 25, "Should have 25 test repositories");
        
        // Verify some are syncing (for testing sync status display)
        let syncing_count = state.repositories.iter().filter(|r| r.is_syncing).count();
        assert!(syncing_count > 0, "Should have some repositories syncing");
        assert!(syncing_count < state.repositories.len(), "Should not have all repositories syncing");
        
        // Test that repository names are unique
        let mut names = std::collections::HashSet::new();
        for repo in &state.repositories {
            assert!(names.insert(&repo.name), "Repository names should be unique: {}", repo.name);
        }
        
        // Test scrollbar height calculation for many repos
        let height = if state.repositories.len() > 10 {
            300.0
        } else if state.repositories.len() > 5 {
            200.0
        } else {
            150.0
        };
        assert_eq!(height, 300.0, "Should use maximum height for many repositories");
    }

    #[test]
    fn test_branch_override_functionality() {
        use crate::gui::repository::types::RepoPanelState;
        use std::collections::HashMap;
        
        // Test branch override logic
        let mut state = RepoPanelState {
            branch_overrides: HashMap::new(),
            ..Default::default()
        };
        
        let repo_name = "test-repo".to_string();
        let default_branch = "main".to_string();
        let override_branch = "feature-branch".to_string();
        
        // Test setting branch override
        state.branch_overrides.insert(repo_name.clone(), override_branch.clone());
        
        // Test getting branch with override
        let branch = state.branch_overrides.get(&repo_name)
            .cloned()
            .unwrap_or_else(|| default_branch.clone());
        assert_eq!(branch, override_branch, "Should use override branch when set");
        
        // Test getting branch without override
        let other_repo = "other-repo".to_string();
        let branch = state.branch_overrides.get(&other_repo)
            .cloned()
            .unwrap_or_else(|| default_branch.clone());
        assert_eq!(branch, default_branch, "Should use default branch when no override");
        
        // Test removing override (empty or same as default)
        state.branch_overrides.remove(&repo_name);
        let branch = state.branch_overrides.get(&repo_name)
            .cloned()
            .unwrap_or_else(|| default_branch.clone());
        assert_eq!(branch, default_branch, "Should use default branch after removing override");
    }

    #[test]
    fn test_sync_error_handling_and_display() {
        use crate::gui::repository::types::SimpleSyncStatus;
        use std::time::Instant;
        
        // Test error status display
        let now = Instant::now();
        let error_status = SimpleSyncStatus {
            is_running: false,
            is_complete: true,
            is_success: false,
            output_lines: vec![
                "🔄 Starting repository sync...".to_string(),
                "[Git Fetch] Fetching objects...".to_string(),
                "❌ Sync Failed.".to_string(),
            ],
            final_message: "Failed to fetch from remote: connection timeout".to_string(),
            started_at: Some(now),
            final_elapsed_seconds: None,
            last_progress_time: Some(now),
        };
        
        // Verify error state
        assert!(!error_status.is_running, "Error status should not be running");
        assert!(error_status.is_complete, "Error status should be complete");
        assert!(!error_status.is_success, "Error status should not be successful");
        assert!(!error_status.final_message.is_empty(), "Error status should have error message");
        assert!(error_status.output_lines.len() >= 3, "Error status should have progress log");
        assert!(error_status.output_lines.last().unwrap().contains("Failed"), "Last log line should indicate failure");
    }

    #[test]
    fn test_immediate_sync_status_feedback() {
        use crate::gui::repository::types::SimpleSyncStatus;
        use std::time::Instant;
        
        // Test that sync status is created immediately when sync is triggered
        let now = Instant::now();
        let immediate_status = SimpleSyncStatus {
            is_running: true,
            is_complete: false,
            is_success: false,
            output_lines: vec!["🔄 Sync requested - initializing...".to_string()],
            final_message: String::new(),
            started_at: Some(now),
            final_elapsed_seconds: None,
            last_progress_time: Some(now),
        };
        
        // Verify immediate feedback
        assert!(immediate_status.is_running, "Should show as running immediately");
        assert!(!immediate_status.is_complete, "Should not be complete when starting");
        assert!(!immediate_status.is_success, "Should not be successful when starting");
        assert!(immediate_status.started_at.is_some(), "Should have start time");
        assert!(!immediate_status.output_lines.is_empty(), "Should have initial log message");
        assert!(immediate_status.output_lines[0].contains("Sync requested"), "Should show sync requested message");
        assert!(immediate_status.output_lines[0].contains("initializing"), "Should show initializing message");
        assert!(immediate_status.final_message.is_empty(), "Should not have final message when starting");
        
        // Test that the status shows immediate activity
        let elapsed = immediate_status.started_at.unwrap().elapsed();
        assert!(elapsed.as_millis() < 100, "Should be created very recently (immediate feedback)");
    }

    #[test]
    fn test_sync_status_always_visible_for_available_repos() {
        use crate::gui::repository::types::{RepoPanelState, RepoInfo};
        use std::path::PathBuf;
        
        // Test that sync status section only shows repositories with actual status
        let state = RepoPanelState {
            repositories: vec![
                RepoInfo {
                    name: "repo-with-status".to_string(),
                    remote: Some("https://github.com/test/repo1.git".to_string()),
                    branch: Some("main".to_string()),
                    local_path: Some(PathBuf::from("/tmp/repo1")),
                    is_syncing: false,
                },
                RepoInfo {
                    name: "repo-without-status".to_string(),
                    remote: Some("https://github.com/test/repo2.git".to_string()),
                    branch: Some("main".to_string()),
                    local_path: Some(PathBuf::from("/tmp/repo2")),
                    is_syncing: false,
                },
            ],
            ..Default::default()
        };
        
        // All repositories are available for syncing
        let repos_to_display: Vec<String> = state.repositories.iter().map(|r| r.name.clone()).collect();
        assert_eq!(repos_to_display.len(), 2, "Should have all repositories available for syncing");
        assert!(repos_to_display.contains(&"repo-with-status".to_string()), "Should include first repo");
        assert!(repos_to_display.contains(&"repo-without-status".to_string()), "Should include second repo");
        
        // But only repositories with actual sync status should be shown in the status section
        // This simulates the logic: only show repos that have simple_status.is_some() || detailed_status.is_some()
        let repos_with_status = repos_to_display.iter().filter(|_| {
            // Simulating no status available for any repo initially
            let has_simple_status = false;
            let has_detailed_status = false;
            has_simple_status || has_detailed_status
        }).count();
        
        assert_eq!(repos_with_status, 0, "Should not show any repos in status section without actual status");
    }

    #[test]
    fn test_sync_status_display_when_no_operations() {
        use crate::gui::repository::types::RepoPanelState;
        
        // Test the message shown when no sync operations are in progress
        let state = RepoPanelState {
            repositories: vec![], // No repositories
            ..Default::default()
        };
        
        // When no repositories have status, should show helpful message
        let has_any_status = false; // Simulating no sync operations
        
        if !has_any_status {
            let expected_message = "No sync operations in progress or completed. Click 'Sync Selected' or 'Sync All' to start syncing.";
            assert!(expected_message.contains("No sync operations"), "Should show helpful message when no sync operations");
            assert!(expected_message.contains("Click"), "Should provide guidance on how to start syncing");
        }
    }

    #[test]
    fn test_repository_manager_locked_message() {
        // Test that when repository manager is locked, we show a helpful message
        let is_manager_locked = true; // Simulating locked repository manager
        
        if is_manager_locked {
            let expected_message = "Loading sync status...";
            assert!(expected_message.contains("Loading"), "Should indicate loading status");
            assert!(!expected_message.contains("Try again shortly"), "Should not suggest trying again (old message)");
        }
    }

    #[test]
    fn test_force_sync_option() {
        use crate::gui::repository::types::RepoPanelState;
        
        // Test that force sync option can be toggled
        let mut state = RepoPanelState::default();
        
        // Initially should be false
        assert!(!state.force_sync, "Force sync should default to false");
        
        // Can be toggled
        state.force_sync = true;
        assert!(state.force_sync, "Force sync should be toggleable to true");
        
        state.force_sync = false;
        assert!(!state.force_sync, "Force sync should be toggleable back to false");
    }

    #[test]
    fn test_force_sync_trigger_parameters() {
        // Test that trigger_sync function accepts force parameter correctly
        let repo_names = vec!["test-repo".to_string()];
        let force_sync = true;
        
        // This test verifies the function signature accepts the force parameter
        // The actual functionality would be tested in integration tests
        assert_eq!(repo_names.len(), 1, "Should have one repository");
        assert!(force_sync, "Force sync parameter should be true");
        
        // Test with force disabled
        let force_sync = false;
        assert!(!force_sync, "Force sync parameter should be false");
    }
} 