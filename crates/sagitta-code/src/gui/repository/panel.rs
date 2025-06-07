use std::sync::Arc;
use anyhow::Result;
use egui::{Context, SidePanel, Vec2, Ui, RichText};
use tokio::sync::Mutex;
use super::manager::RepositoryManager;

use super::types::{RepoPanelState, RepoPanelTab};
use super::list::render_repo_list;
use super::add::render_add_repo;
use super::sync::render_sync_repo;
use super::query::render_query_repo;
use super::search::render_file_search;
use super::view::render_file_view;
use super::branches::render_branch_management;

/// Repository management panel
pub struct RepoPanel {
    state: Arc<Mutex<RepoPanelState>>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    is_open: bool,
}

impl RepoPanel {
    /// Create a new repository panel
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            state: Arc::new(Mutex::new(RepoPanelState::default())),
            repo_manager,
            is_open: false,
        }
    }

    /// Update repository list by spawning an async task
    pub fn refresh_repositories(&self) -> Result<()> {
        let state_clone = Arc::clone(&self.state);
        let repo_manager_clone = Arc::clone(&self.repo_manager);

        tokio::spawn(async move {
            log::debug!("RepoPanel: Starting background enhanced repository refresh...");
            
            // Try to get enhanced repository information first
            match repo_manager_clone.lock().await.get_enhanced_repository_list().await {
                Ok(enhanced_list) => {
                    let mut state_guard = state_clone.lock().await;
                    state_guard.enhanced_repositories = enhanced_list.repositories
                        .into_iter()
                        .map(|enhanced| enhanced.into())
                        .collect();
                    state_guard.use_enhanced_repos = true;
                    log::info!("RepoPanel: Successfully refreshed {} enhanced repositories.", state_guard.enhanced_repositories.len());
                }
                Err(e) => {
                    log::warn!("RepoPanel: Failed to get enhanced repository list: {}, falling back to basic list", e);
                    
                    // Fallback to basic repository listing
                    match repo_manager_clone.lock().await.list_repositories().await {
                        Ok(repositories) => {
                            let mut state_guard = state_clone.lock().await;
                            state_guard.repositories = repositories
                                .into_iter()
                                .map(|config| config.into())
                                .collect();
                            state_guard.use_enhanced_repos = false;
                            log::info!("RepoPanel: Successfully refreshed {} basic repositories.", state_guard.repositories.len());
                        }
                        Err(e) => {
                            log::error!("RepoPanel: Failed to refresh repositories: {}", e);
                        }
                    }
                }
            }
        });
        Ok(())
    }

    /// Toggle the panel visibility
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    /// Check if the panel is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Render the repository panel
    pub fn render(&mut self, ctx: &Context, theme: crate::gui::theme::AppTheme) {
        if !self.is_open {
            return;
        }

        let state_clone = Arc::clone(&self.state);
        let repo_manager_clone = Arc::clone(&self.repo_manager);

        SidePanel::right("repository_panel")
            .default_width(300.0)
            .min_width(250.0)
            .resizable(true)
            .frame(theme.side_panel_frame())
            .show(ctx, |ui| {
                let mut state_guard = match state_clone.try_lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        ui.label("State lock contention...");
                        return;
                    }
                };

                // Check if we need to refresh repositories
                if state_guard.is_loading_repos {
                    // Start a repository refresh
                    drop(state_guard); // Drop lock before spawning refresh task
                    let _ = self.refresh_repositories();
                    
                    // Re-acquire lock after starting refresh
                    state_guard = match state_clone.try_lock() {
                        Ok(guard) => guard,
                        Err(_) => {
                            ui.label("State lock contention after refresh trigger...");
                            return;
                        }
                    };
                    
                    // Reset loading flag
                    state_guard.is_loading_repos = false;
                }

                // Check if we need to load enhanced repositories for the first time
                if !state_guard.use_enhanced_repos && state_guard.enhanced_repositories.is_empty() && !state_guard.is_loading_repos && !state_guard.initial_load_attempted {
                    // Mark that we've attempted initial load to prevent infinite loops
                    state_guard.initial_load_attempted = true;
                    
                    // Start initial enhanced repository load
                    drop(state_guard); // Drop lock before spawning refresh task
                    let _ = self.refresh_repositories();
                    
                    // Re-acquire lock after starting refresh
                    state_guard = match state_clone.try_lock() {
                        Ok(guard) => guard,
                        Err(_) => {
                            ui.label("State lock contention during initial load...");
                            return;
                        }
                    };
                }

                self.render_header(ui);
                ui.separator();
                self.render_tabs(ui, &mut state_guard);
                ui.separator();

                match state_guard.active_tab {
                    RepoPanelTab::List => {
                        render_repo_list(ui, &mut state_guard, Arc::clone(&repo_manager_clone), theme);
                    }
                    RepoPanelTab::Add => {
                        render_add_repo(ui, &mut state_guard, Arc::clone(&repo_manager_clone), theme);
                    }
                    RepoPanelTab::CreateProject => {
                        // We need config and agent for project creation - these should be passed in
                        // For now, render a placeholder until we wire up the dependencies
                        ui.vertical_centered(|ui| {
                            ui.label("🆕 Project Creation");
                            ui.add_space(8.0);
                            ui.label("This feature requires access to the agent and config.");
                            ui.label("Implementation in progress...");
                        });
                    }
                    RepoPanelTab::Sync => {
                        // Auto-refresh repositories if sync tab is accessed with no repositories
                        let has_repos = if state_guard.use_enhanced_repos {
                            !state_guard.enhanced_repositories.is_empty()
                        } else {
                            !state_guard.repositories.is_empty()
                        };
                        
                        // Only trigger refresh if we have no repositories AND we're not already loading AND we haven't attempted initial load
                        if !has_repos && !state_guard.is_loading_repos && !state_guard.initial_load_attempted {
                            // Trigger a refresh if we have no repositories in the sync tab
                            state_guard.is_loading_repos = true;
                            state_guard.initial_load_attempted = true; // Mark that we've attempted to prevent loops
                            log::info!("Auto-triggering repository refresh for sync tab");
                        }
                        
                        render_sync_repo(ui, &mut state_guard, Arc::clone(&repo_manager_clone), theme);
                    }
                    RepoPanelTab::Query => {
                        render_query_repo(ui, &mut state_guard, Arc::clone(&repo_manager_clone), theme);
                    }
                    RepoPanelTab::SearchFile => {
                        render_file_search(ui, &mut state_guard, Arc::clone(&repo_manager_clone), theme);
                    }
                    RepoPanelTab::ViewFile => {
                        render_file_view(ui, &mut state_guard, Arc::clone(&repo_manager_clone), theme);
                    }
                    RepoPanelTab::Branches => {
                        render_branch_management(ui, &mut state_guard, Arc::clone(&repo_manager_clone), theme);
                    }
                }
            });
    }

    fn render_header(&mut self, ui: &mut Ui) {
        ui.heading("Repository Management");
    }

    fn render_tabs(&mut self, ui: &mut Ui, state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) {
        ui.horizontal(|ui| {
            // Add a bit of space to ensure proper tab spacing
            ui.spacing_mut().item_spacing.x = 10.0;
            
            self.tab_button(ui, RepoPanelTab::List, "List", state);
            self.tab_button(ui, RepoPanelTab::Add, "Add", state);
            self.tab_button(ui, RepoPanelTab::CreateProject, "🆕 Create", state);
            self.tab_button(ui, RepoPanelTab::Sync, "Sync", state);
            self.tab_button(ui, RepoPanelTab::Query, "Query", state);
            self.tab_button(ui, RepoPanelTab::SearchFile, "Files", state);
            self.tab_button(ui, RepoPanelTab::ViewFile, "View", state);
            self.tab_button(ui, RepoPanelTab::Branches, "Branches", state);
        });
    }

    fn tab_button(&mut self, ui: &mut Ui, tab: RepoPanelTab, label: &str, state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>) {
        let selected = state.active_tab == tab;
        
        // Use selectable_value to make the tab style more consistent
        if ui.selectable_label(selected, label).clicked() {
            state.active_tab = tab;
        }
    }

    /// Get the repository manager
    pub fn get_repo_manager(&self) -> Arc<Mutex<RepositoryManager>> {
        Arc::clone(&self.repo_manager)
    }
} 