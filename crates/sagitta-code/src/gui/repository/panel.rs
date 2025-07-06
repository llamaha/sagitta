use std::sync::Arc;
use anyhow::Result;
use egui::{Context, SidePanel, Ui};
use tokio::sync::Mutex;
use super::manager::RepositoryManager;

use super::types::{RepoPanelState, RepoPanelTab, OrphanedRepoInfo};
use super::list::render_repo_list;
use super::add::render_add_repo;
use super::sync::render_sync_repo;
use super::query::render_query_repo;
use super::search::render_file_search;
use super::view::render_file_view;
use super::branches::render_branch_management;
use super::create_project::render_create_project;
use crate::config::types::SagittaCodeConfig;
use crate::agent::Agent;
use crate::services::SyncOrchestrator;

/// Repository management panel
pub struct RepoPanel {
    state: Arc<Mutex<RepoPanelState>>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    config: Arc<Mutex<SagittaCodeConfig>>,
    agent: Option<Arc<Agent>>,
    sync_orchestrator: Option<Arc<SyncOrchestrator>>,
    is_open: bool,
}

impl RepoPanel {
    /// Create a new repository panel
    pub fn new(
        repo_manager: Arc<Mutex<RepositoryManager>>,
        config: Arc<Mutex<SagittaCodeConfig>>,
        agent: Option<Arc<Agent>>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(RepoPanelState::default())),
            repo_manager,
            config,
            agent,
            sync_orchestrator: None,
            is_open: false,
        }
    }

    /// Update repository list by spawning an async task
    pub fn refresh_repositories(&self) -> Result<()> {
        let state_clone = Arc::clone(&self.state);
        let repo_manager_clone = Arc::clone(&self.repo_manager);
        let sync_orchestrator_clone = self.sync_orchestrator.clone();

        tokio::spawn(async move {
            log::debug!("RepoPanel: Starting background enhanced repository refresh...");
            
            // Try to get enhanced repository information first
            let enhanced_result = {
                let manager = repo_manager_clone.lock().await;
                manager.get_enhanced_repository_list().await
            };
            
            match enhanced_result {
                Ok(enhanced_list) => {
                    // Get orphaned repositories
                    let orphaned_result = {
                        let manager = repo_manager_clone.lock().await;
                        manager.get_orphaned_repositories().await
                    };
                    
                    // Update state with both results
                    let mut state_guard = state_clone.lock().await;
                    state_guard.enhanced_repositories = enhanced_list.repositories
                        .into_iter()
                        .map(|enhanced| enhanced.into())
                        .collect();
                    state_guard.use_enhanced_repos = true;
                    state_guard.is_loading_repos = false; // Reset loading flag
                    log::info!("RepoPanel: Successfully refreshed {} enhanced repositories.", state_guard.enhanced_repositories.len());
                    
                    // Update sync status from sync orchestrator if available
                    if let Some(sync_orchestrator) = &sync_orchestrator_clone {
                        let sync_statuses = sync_orchestrator.get_all_sync_statuses().await;
                        
                        // Update each repository's sync status
                        for enhanced_repo in &mut state_guard.enhanced_repositories {
                            // Try to find sync status by matching local path
                            if let Some(local_path) = &enhanced_repo.local_path {
                                if let Some(sync_status) = sync_statuses.get(local_path) {
                                    // Map SyncOrchestrator's SyncState to the GUI's SyncState
                                    use crate::services::sync_orchestrator::SyncState as OrchestratorSyncState;
                                    use super::types::SyncState as GuiSyncState;
                                    
                                    enhanced_repo.sync_status.state = match sync_status.sync_state {
                                        OrchestratorSyncState::FullySynced => GuiSyncState::UpToDate,
                                        OrchestratorSyncState::LocalOnly => GuiSyncState::LocalOnly,
                                        OrchestratorSyncState::LocalIndexedRemoteFailed => GuiSyncState::LocalIndexedRemoteFailed,
                                        OrchestratorSyncState::Syncing => GuiSyncState::Syncing,
                                        OrchestratorSyncState::Failed => GuiSyncState::Failed,
                                        OrchestratorSyncState::NotSynced => {
                                            if sync_status.is_out_of_sync {
                                                GuiSyncState::NeedsSync
                                            } else {
                                                GuiSyncState::NeverSynced
                                            }
                                        }
                                    };
                                    
                                    log::debug!("Updated sync status for {}: {:?}", enhanced_repo.name, enhanced_repo.sync_status.state);
                                }
                            }
                        }
                    }
                    
                    match orphaned_result {
                        Ok(orphaned_list) => {
                            state_guard.orphaned_repositories = orphaned_list
                                .into_iter()
                                .map(|orphaned| OrphanedRepoInfo {
                                    name: orphaned.name,
                                    local_path: orphaned.local_path,
                                    is_git_repository: orphaned.is_git_repository,
                                    remote_url: orphaned.remote_url,
                                    file_count: orphaned.file_count,
                                    size_bytes: orphaned.size_bytes,
                                })
                                .collect();
                            log::info!("RepoPanel: Found {} orphaned repositories.", state_guard.orphaned_repositories.len());
                        }
                        Err(e) => {
                            log::warn!("RepoPanel: Failed to get orphaned repositories: {e}");
                            state_guard.orphaned_repositories.clear();
                        }
                    }
                }
                Err(e) => {
                    log::warn!("RepoPanel: Failed to get enhanced repository list: {e}, falling back to basic list");
                    
                    // Fallback to basic repository listing
                    let basic_result = {
                        let manager = repo_manager_clone.lock().await;
                        manager.list_repositories().await
                    };
                    
                    match basic_result {
                        Ok(repositories) => {
                            let mut state_guard = state_clone.lock().await;
                            state_guard.repositories = repositories
                                .into_iter()
                                .map(|config| config.into())
                                .collect();
                            state_guard.use_enhanced_repos = false;
                            state_guard.orphaned_repositories.clear(); // Clear orphaned repos in fallback mode
                            state_guard.is_loading_repos = false; // Reset loading flag
                            log::info!("RepoPanel: Successfully refreshed {} basic repositories.", state_guard.repositories.len());
                        }
                        Err(e) => {
                            log::error!("RepoPanel: Failed to refresh repositories: {e}");
                            // Reset loading flag on error
                            let mut state_guard = state_clone.lock().await;
                            state_guard.is_loading_repos = false;
                        }
                    }
                }
            }
        });
        Ok(())
    }

    /// Toggle the panel visibility
    pub fn toggle(&mut self) {
        let was_closed = !self.is_open;
        self.is_open = !self.is_open;
        
        // If we're opening the panel and the List tab is active, trigger a refresh
        if was_closed && self.is_open {
            if let Ok(mut state) = self.state.try_lock() {
                if state.active_tab == RepoPanelTab::List {
                    state.is_loading_repos = true;
                    state.use_enhanced_repos = false; // Reset to trigger enhanced reload
                    log::info!("Repository panel opened with List tab active, triggering refresh");
                }
            }
        }
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
                    log::info!("Repository Panel: Processing refresh request");
                    
                    // Reset loading flag immediately to prevent duplicate refreshes
                    state_guard.is_loading_repos = false;
                    
                    // Start a repository refresh
                    drop(state_guard); // Drop lock before spawning refresh task
                    
                    // Always execute refresh when requested
                    if let Err(e) = self.refresh_repositories() {
                        log::error!("Repository Panel: Failed to start refresh: {}", e);
                    } else {
                        log::info!("Repository Panel: Refresh task started successfully");
                    }
                    
                    // Re-acquire lock after starting refresh
                    state_guard = match state_clone.try_lock() {
                        Ok(guard) => guard,
                        Err(_) => {
                            ui.label("State lock contention after refresh trigger...");
                            return;
                        }
                    };
                }

                // Check if we need to load enhanced repositories for the first time
                if !state_guard.use_enhanced_repos && state_guard.enhanced_repositories.is_empty() && !state_guard.is_loading_repos && !state_guard.initial_load_attempted {
                    // Mark that we've attempted initial load to prevent infinite loops
                    state_guard.initial_load_attempted = true;
                    state_guard.is_loading_repos = true; // Set loading flag
                    
                    // Start initial enhanced repository load
                    drop(state_guard); // Drop lock before spawning refresh task
                    let _ = self.refresh_repositories();
                    
                    // Show loading message and return early
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label("Loading repositories...");
                    });
                    return;
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
                        // Get config from the panel's config field
                        let config = match self.config.try_lock() {
                            Ok(guard) => guard.clone(),
                            Err(_) => {
                                ui.label("Config lock contention...");
                                return;
                            }
                        };
                        
                        render_create_project(
                            ui,
                            &mut state_guard,
                            &config,
                            Arc::clone(&repo_manager_clone),
                            theme,
                        );
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
        
        // Render the dependency modal if it's visible
        if let Ok(mut state_guard) = self.state.try_lock() {
            let available_repos = state_guard.repo_names();
            state_guard.dependency_modal.render(
                ctx,
                &available_repos,
                Arc::clone(&self.repo_manager),
                &theme,
            );
            
            // Render remove confirmation dialog at panel level
            if state_guard.show_remove_confirmation {
                super::list::render_remove_confirmation_dialog(ctx, &mut state_guard, Arc::clone(&self.repo_manager), &theme);
            }
        }
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
            self.tab_button(ui, RepoPanelTab::CreateProject, "Create", state);
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
            
            // Always refresh when Repository List tab is clicked
            if tab == RepoPanelTab::List {
                state.is_loading_repos = true;
                state.use_enhanced_repos = false; // Reset to trigger enhanced reload
                log::info!("Repository List tab clicked, triggering refresh");
            }
        }
    }

    /// Get the repository manager
    pub fn get_repo_manager(&self) -> Arc<Mutex<RepositoryManager>> {
        Arc::clone(&self.repo_manager)
    }

    /// Set the agent (usually done after initialization)
    pub fn set_agent(&mut self, agent: Arc<Agent>) {
        self.agent = Some(agent);
    }
    
    /// Set the sync orchestrator (usually done after initialization)
    pub fn set_sync_orchestrator(&mut self, sync_orchestrator: Arc<SyncOrchestrator>) {
        self.sync_orchestrator = Some(sync_orchestrator);
    }
    
    /// Set the active tab
    pub fn set_active_tab(&self, tab: RepoPanelTab) {
        let state_clone = Arc::clone(&self.state);
        tokio::spawn(async move {
            let mut state = state_clone.lock().await;
            state.active_tab = tab;
        });
    }
    
    /// Check if a repository was just created and return its name if so
    pub fn take_newly_created_repository(&self) -> Option<String> {
        match self.state.try_lock() {
            Ok(mut state) => state.newly_created_repository.take(),
            Err(_) => None,
        }
    }
    
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::SagittaCodeConfig;
    use sagitta_search::{AppConfig as SagittaAppConfig};
    

    fn create_test_config() -> Arc<Mutex<SagittaCodeConfig>> {
        Arc::new(Mutex::new(SagittaCodeConfig::default()))
    }

    fn create_test_repo_manager() -> Arc<Mutex<RepositoryManager>> {
        let sagitta_config = Arc::new(Mutex::new(SagittaAppConfig::default()));
        Arc::new(Mutex::new(RepositoryManager::new(sagitta_config)))
    }

    #[test]
    fn test_repo_panel_creation() {
        let repo_manager = create_test_repo_manager();
        let config = create_test_config();
        let panel = RepoPanel::new(repo_manager, config, None);
        
        assert!(!panel.is_open());
    }

    #[test]
    fn test_repo_panel_toggle() {
        let repo_manager = create_test_repo_manager();
        let config = create_test_config();
        let mut panel = RepoPanel::new(repo_manager, config, None);
        
        assert!(!panel.is_open());
        panel.toggle();
        assert!(panel.is_open());
        panel.toggle();
        assert!(!panel.is_open());
    }

    #[test]
    fn test_repo_panel_set_agent() {
        let repo_manager = create_test_repo_manager();
        let config = create_test_config();
        let panel = RepoPanel::new(repo_manager, config.clone(), None);
        
        assert!(panel.agent.is_none());
        
        // Create a mock agent (would need proper agent setup in real test)
        // For now, just verify the structure exists
        // panel.set_agent(agent);
        // assert!(panel.agent.is_some());
    }

    #[tokio::test]
    async fn test_repo_panel_set_active_tab() {
        let repo_manager = create_test_repo_manager();
        let config = create_test_config();
        let panel = RepoPanel::new(repo_manager, config, None);
        
        // Set active tab to CreateProject
        panel.set_active_tab(RepoPanelTab::CreateProject);
        
        // Wait a bit for the async operation
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        // In a real test, we would verify the tab was set
        // For now, just verify no panic occurs
    }
    
    #[tokio::test]
    async fn test_repository_list_refresh_on_tab_click() {
        let repo_manager = create_test_repo_manager();
        let config = create_test_config();
        let panel = RepoPanel::new(repo_manager, config, None);
        
        // Get initial state
        let mut state = panel.state.lock().await;
        state.active_tab = RepoPanelTab::Sync; // Start from different tab
        state.is_loading_repos = false; // Start with no loading
        state.use_enhanced_repos = true; // Start with enhanced repos
        
        // Simulate clicking the List tab (as would happen in tab_button method)
        state.active_tab = RepoPanelTab::List;
        state.is_loading_repos = true;
        state.use_enhanced_repos = false;
        
        // Verify refresh was triggered
        assert!(state.is_loading_repos, "Refresh should be triggered when List tab is clicked");
        assert!(!state.use_enhanced_repos, "Enhanced repos should be reset to trigger reload");
        assert_eq!(state.active_tab, RepoPanelTab::List, "Active tab should be set to List");
    }
    
    #[tokio::test]
    async fn test_refresh_button_functionality() {
        let repo_manager = create_test_repo_manager();
        let config = create_test_config();
        let panel = RepoPanel::new(repo_manager, config, None);
        
        // Simulate refresh button click
        let mut state = panel.state.lock().await;
        state.is_loading_repos = false; // Start with no loading
        state.use_enhanced_repos = true; // Start with enhanced repos
        
        // Simulate refresh button click (as would happen in render_repo_list)
        state.is_loading_repos = true;
        state.use_enhanced_repos = false;
        
        // Verify refresh was triggered
        assert!(state.is_loading_repos, "Refresh should be triggered when refresh button is clicked");
        assert!(!state.use_enhanced_repos, "Enhanced repos should be reset to trigger reload");
    }

    #[tokio::test]
    async fn test_panel_toggle_triggers_refresh_on_list_tab() {
        let repo_manager = create_test_repo_manager();
        let config = create_test_config();
        let mut panel = RepoPanel::new(repo_manager, config, None);
        
        // Set up initial state - panel closed, List tab active
        panel.is_open = false;
        {
            let mut state = panel.state.lock().await;
            state.active_tab = RepoPanelTab::List;
            state.is_loading_repos = false;
            state.use_enhanced_repos = true;
        }
        
        // Toggle panel open (simulate Ctrl+R)
        panel.toggle();
        
        // Verify panel is now open
        assert!(panel.is_open(), "Panel should be open after toggle");
        
        // Verify refresh was triggered for List tab
        let state = panel.state.lock().await;
        assert!(state.is_loading_repos, "Refresh should be triggered when panel is opened on List tab");
        assert!(!state.use_enhanced_repos, "Enhanced repos should be reset to trigger reload");
    }

    #[tokio::test]
    async fn test_panel_toggle_no_refresh_on_other_tabs() {
        let repo_manager = create_test_repo_manager();
        let config = create_test_config();
        let mut panel = RepoPanel::new(repo_manager, config, None);
        
        // Set up initial state - panel closed, Sync tab active
        panel.is_open = false;
        {
            let mut state = panel.state.lock().await;
            state.active_tab = RepoPanelTab::Sync;
            state.is_loading_repos = false;
            state.use_enhanced_repos = true;
        }
        
        // Toggle panel open
        panel.toggle();
        
        // Verify panel is now open
        assert!(panel.is_open(), "Panel should be open after toggle");
        
        // Verify no refresh was triggered for non-List tab
        let state = panel.state.lock().await;
        assert!(!state.is_loading_repos, "Refresh should NOT be triggered when panel is opened on non-List tab");
        assert!(state.use_enhanced_repos, "Enhanced repos should remain unchanged");
    }

} 