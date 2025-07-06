use anyhow::Result;
use egui::{Button, ComboBox, Response, Ui};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use crate::gui::repository::manager::RepositoryManager;
use crate::gui::theme::AppTheme;
use crate::services::{SyncOrchestrator, RepositorySyncStatus};

/// Represents the type of git reference
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GitRefType {
    Branch,
    Tag,
    Commit,
}

/// Git reference information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRef {
    pub name: String,
    pub ref_type: GitRefType,
    pub hash: String,
    pub is_current: bool,
}

/// State for git workflow controls
#[derive(Debug, Clone)]
pub struct GitControlsState {
    /// Currently selected repository
    pub current_repository: Option<String>,
    /// Available git references for current repository
    pub available_refs: Vec<GitRef>,
    /// Currently selected reference
    pub current_ref: Option<GitRef>,
    /// Whether we're currently switching references
    pub is_switching: bool,
    /// Last error message
    pub last_error: Option<String>,
    /// Whether to show advanced controls
    pub show_advanced: bool,
    /// New branch name input
    pub new_branch_name: String,
    /// Sync status for repositories
    pub sync_statuses: HashMap<String, RepositorySyncStatus>,
}

impl Default for GitControlsState {
    fn default() -> Self {
        Self {
            current_repository: None,
            available_refs: Vec::new(),
            current_ref: None,
            is_switching: false,
            last_error: None,
            show_advanced: false,
            new_branch_name: String::new(),
            sync_statuses: HashMap::new(),
        }
    }
}

/// Commands for async operations
#[derive(Debug, Clone)]
pub enum GitCommand {
    SwitchToRef { git_ref: GitRef },
    CreateBranch { name: String, start_point: Option<String> },
    RefreshRefs { repo_name: String },
    ForceSync { repo_name: String },
    UpdateRepository { repo_name: Option<String> },
    UpdateSyncStatuses,
}

/// Git workflow controls component
pub struct GitControls {
    state: GitControlsState,
    repository_manager: Arc<Mutex<RepositoryManager>>,
    sync_orchestrator: Option<Arc<SyncOrchestrator>>,
    /// Channel for sending async commands
    command_tx: mpsc::UnboundedSender<GitCommand>,
    command_rx: Option<mpsc::UnboundedReceiver<GitCommand>>,
}

impl GitControls {
    /// Create new git controls
    pub fn new(repository_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        
        Self {
            state: GitControlsState::default(),
            repository_manager,
            sync_orchestrator: None,
            command_tx,
            command_rx: Some(command_rx),
        }
    }

    /// Set the sync orchestrator for status updates
    pub fn set_sync_orchestrator(&mut self, sync_orchestrator: Arc<SyncOrchestrator>) {
        self.sync_orchestrator = Some(sync_orchestrator);
    }
    
    /// Start the async command handler and return the receiver
    pub fn start_command_handler(&mut self) -> mpsc::UnboundedReceiver<GitCommand> {
        self.command_rx.take().expect("Command handler already started")
    }

    /// Update the current repository
    pub async fn set_current_repository(&mut self, repository_name: Option<String>) -> Result<()> {
        if self.state.current_repository == repository_name {
            return Ok(());
        }

        self.state.current_repository = repository_name.clone();
        self.state.available_refs.clear();
        self.state.current_ref = None;
        self.state.last_error = None;

        if let Some(repo_name) = repository_name {
            if let Err(e) = self.refresh_git_refs(&repo_name).await {
                // Check if this is an UnbornBranch error (repository with no commits)
                if e.to_string().contains("UnbornBranch") || e.to_string().contains("reference 'refs/heads/master' not found") {
                    info!("Repository {} has no commits yet, using default branch", repo_name);
                    // Set default state for new repository
                    self.state.available_refs.clear();
                    self.state.current_ref = Some(GitRef {
                        name: "main".to_string(),
                        ref_type: GitRefType::Branch,
                        hash: "0000000".to_string(),
                        is_current: true,
                    });
                    self.state.last_error = None;
                } else {
                    error!("Failed to refresh git refs for {}: {}", repo_name, e);
                    self.state.last_error = Some(format!("Failed to load git refs: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Refresh git references for the current repository
    async fn refresh_git_refs(&mut self, repository_name: &str) -> Result<()> {
        let repo_manager = self.repository_manager.lock().await;
        
        // Get available branches, tags, and refs
        let refs = repo_manager.list_git_references(repository_name).await?;
        let current_ref = repo_manager.get_current_git_ref(repository_name).await?;

        self.state.available_refs = refs;
        self.state.current_ref = current_ref;

        Ok(())
    }

    /// Switch to a different git reference
    pub async fn switch_to_ref(&mut self, git_ref: &GitRef) -> Result<()> {
        let repo_name = match &self.state.current_repository {
            Some(name) => name.clone(),
            None => return Err(anyhow::anyhow!("No repository selected")),
        };

        info!("Switching to git ref: {} in repository: {}", git_ref.name, repo_name);

        self.state.is_switching = true;
        self.state.last_error = None;

        let result = {
            let mut repo_manager = self.repository_manager.lock().await;
            repo_manager.switch_to_ref(&repo_name, &git_ref.name, true).await
        };

        match result {
            Ok(_) => {
                info!("Successfully switched to ref: {}", git_ref.name);
                
                // Trigger sync if orchestrator is available
                if let Some(sync_orchestrator) = &self.sync_orchestrator {
                    if let Some(repo_path) = self.get_repository_path(&repo_name).await {
                        if let Err(e) = sync_orchestrator.switch_repository(&repo_path).await {
                            warn!("Failed to trigger sync after ref switch: {}", e);
                        }
                    }
                }

                // Refresh refs to update current state
                if let Err(e) = self.refresh_git_refs(&repo_name).await {
                    warn!("Failed to refresh git refs after switch: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to switch to ref {}: {}", git_ref.name, e);
                self.state.last_error = Some(format!("Switch failed: {}", e));
            }
        }

        self.state.is_switching = false;
        Ok(())
    }

    /// Create a new branch
    pub async fn create_branch(&mut self, branch_name: &str, start_point: Option<&str>) -> Result<()> {
        let repo_name = match &self.state.current_repository {
            Some(name) => name.clone(),
            None => return Err(anyhow::anyhow!("No repository selected")),
        };

        info!("Creating new branch: {} in repository: {}", branch_name, repo_name);

        let result = {
            let repo_manager = self.repository_manager.lock().await;
            repo_manager.create_branch(&repo_name, branch_name, false).await
        };

        match result {
            Ok(_) => {
                info!("Successfully created branch: {}", branch_name);
                // Refresh refs to show the new branch
                if let Err(e) = self.refresh_git_refs(&repo_name).await {
                    warn!("Failed to refresh git refs after branch creation: {}", e);
                }
                // Clear the input
                self.state.new_branch_name.clear();
            }
            Err(e) => {
                error!("Failed to create branch {}: {}", branch_name, e);
                self.state.last_error = Some(format!("Branch creation failed: {}", e));
            }
        }

        Ok(())
    }

    /// Get repository path for sync operations
    async fn get_repository_path(&self, repository_name: &str) -> Option<std::path::PathBuf> {
        let repo_manager = self.repository_manager.lock().await;
        repo_manager.get_repository_path(repository_name).await.ok()
    }

    /// Update sync statuses
    pub async fn update_sync_statuses(&mut self) {
        if let Some(sync_orchestrator) = &self.sync_orchestrator {
            let statuses = sync_orchestrator.get_all_sync_statuses().await;
            self.state.sync_statuses = statuses.into_iter()
                .map(|(path, status)| (path.to_string_lossy().to_string(), status))
                .collect();
        }
    }

    /// Get sync status for current repository
    pub fn get_current_sync_status(&self) -> Option<&RepositorySyncStatus> {
        self.state.current_repository.as_ref()
            .and_then(|repo_name| self.state.sync_statuses.get(repo_name))
    }

    /// Render the git controls UI
    pub fn render(&mut self, ui: &mut Ui, theme: AppTheme) -> Response {
        let mut response = ui.horizontal(|ui| {
            // Repository sync status indicator
            if let Some(sync_status) = self.get_current_sync_status() {
                self.render_sync_status_indicator(ui, sync_status, theme);
                ui.separator();
            }

            // Current reference display
            if let Some(current_ref) = &self.state.current_ref {
                ui.label("📍");
                ui.label(format!("{} ({})", current_ref.name, 
                    match current_ref.ref_type {
                        GitRefType::Branch => "branch",
                        GitRefType::Tag => "tag",
                        GitRefType::Commit => "commit",
                    }
                ));
                ui.separator();
            }

            // Reference selection combo box
            if !self.state.available_refs.is_empty() {
                self.render_ref_selector(ui, theme);
                ui.separator();
            }

            // Advanced controls toggle
            if ui.button(if self.state.show_advanced { "🔧 Less" } else { "🔧 More" }).clicked() {
                self.state.show_advanced = !self.state.show_advanced;
            }

            // Advanced controls
            if self.state.show_advanced {
                ui.separator();
                self.render_advanced_controls(ui, theme);
            }

            // Error display
            if let Some(error) = &self.state.last_error {
                ui.separator();
                ui.colored_label(theme.error_color(), format!("⚠ {}", error));
                if ui.small_button("✕").clicked() {
                    self.state.last_error = None;
                }
            }
        }).response;

        // Handle async operations
        if self.state.is_switching {
            response = response.on_hover_text("Switching git reference...");
        }

        response
    }

    /// Render sync status indicator
    fn render_sync_status_indicator(&self, ui: &mut Ui, sync_status: &RepositorySyncStatus, theme: AppTheme) {
        use crate::services::SyncState;
        
        let (icon, color, tooltip) = match sync_status.sync_state {
            SyncState::FullySynced => {
                ("✅", theme.success_color(), "Fully synced with remote repository")
            }
            SyncState::LocalOnly => {
                ("📁", theme.info_color(), "Local repository (no remote configured)")
            }
            SyncState::LocalIndexedRemoteFailed => {
                let error_detail = match &sync_status.sync_error_type {
                    Some(crate::services::SyncErrorType::AuthenticationFailed) => {
                        "Authentication failed - check your SSH keys or credentials"
                    }
                    Some(crate::services::SyncErrorType::NetworkError) => {
                        "Network error - check your internet connection"
                    }
                    _ => "Remote sync failed, but local indexing succeeded"
                };
                ("📡", theme.warning_color(), error_detail)
            }
            SyncState::Syncing => {
                ("⏳", theme.accent_color(), "Repository is currently syncing...")
            }
            SyncState::Failed => {
                let error_msg = sync_status.last_sync_error.as_deref().unwrap_or("Sync failed");
                ("❌", theme.error_color(), error_msg)
            }
            SyncState::NotSynced => {
                if sync_status.is_out_of_sync {
                    ("🔄", theme.warning_color(), "Repository has changes that need syncing")
                } else {
                    ("❓", theme.muted_color(), "Repository not yet synced")
                }
            }
        };

        let mut response = ui.colored_label(color, icon);
        
        // Show detailed tooltip
        response = response.on_hover_ui(|ui| {
            ui.label(tooltip);
            
            if let Some(error) = &sync_status.last_sync_error {
                ui.separator();
                ui.colored_label(theme.error_color(), format!("Error: {}", error));
            }
            
            if sync_status.is_local_only {
                ui.separator();
                ui.label("💡 This is a local-only repository. To sync with a remote:");
                ui.label("1. Add a remote: git remote add origin <url>");
                ui.label("2. Push your changes: git push -u origin main");
            }
        });
    }

    /// Render reference selector
    fn render_ref_selector(&mut self, ui: &mut Ui, _theme: AppTheme) {
        let current_ref_name = self.state.current_ref.as_ref().map(|r| r.name.clone()).unwrap_or_default();
        
        ComboBox::from_label("Git Ref")
            .selected_text(&current_ref_name)
            .show_ui(ui, |ui| {
                // Group by type
                let mut branches = Vec::new();
                let mut tags = Vec::new();
                let mut commits = Vec::new();

                for git_ref in &self.state.available_refs {
                    match git_ref.ref_type {
                        GitRefType::Branch => branches.push(git_ref),
                        GitRefType::Tag => tags.push(git_ref),
                        GitRefType::Commit => commits.push(git_ref),
                    }
                }

                // Render branches
                if !branches.is_empty() {
                    ui.label("📋 Branches");
                    for branch in branches {
                        let text = if branch.is_current {
                            format!("• {} (current)", branch.name)
                        } else {
                            format!("  {}", branch.name)
                        };
                        
                        if ui.selectable_label(branch.is_current, text).clicked() && !self.state.is_switching {
                            let _ = self.command_tx.send(GitCommand::SwitchToRef { git_ref: branch.clone() });
                        }
                    }
                    ui.separator();
                }

                // Render tags
                if !tags.is_empty() {
                    ui.label("🏷 Tags");
                    for tag in tags {
                        if ui.selectable_label(false, format!("  {}", tag.name)).clicked() && !self.state.is_switching {
                            let _ = self.command_tx.send(GitCommand::SwitchToRef { git_ref: tag.clone() });
                        }
                    }
                }
            });
    }

    /// Render advanced git controls
    fn render_advanced_controls(&mut self, ui: &mut Ui, _theme: AppTheme) {
        ui.horizontal(|ui| {
            ui.label("New branch:");
            ui.text_edit_singleline(&mut self.state.new_branch_name);
            
            let create_enabled = !self.state.new_branch_name.trim().is_empty() && !self.state.is_switching;
            
            if ui.add_enabled(create_enabled, Button::new("Create")).clicked() {
                let branch_name = self.state.new_branch_name.trim().to_string();
                let _ = self.command_tx.send(GitCommand::CreateBranch { 
                    name: branch_name, 
                    start_point: None 
                });
            }
        });

        ui.horizontal(|ui| {
            if ui.button("🔄 Refresh").clicked() {
                if let Some(repo_name) = &self.state.current_repository {
                    let _ = self.command_tx.send(GitCommand::RefreshRefs { 
                        repo_name: repo_name.clone() 
                    });
                }
            }

            if ui.button("🔄 Force Sync").clicked() {
                if let Some(repo_name) = &self.state.current_repository {
                    let _ = self.command_tx.send(GitCommand::ForceSync { 
                        repo_name: repo_name.clone() 
                    });
                }
            }
        });
    }

    /// Get the current state for external access
    pub fn state(&self) -> &GitControlsState {
        &self.state
    }
    
    /// Send a command to the git controls
    pub fn send_command(&self, command: GitCommand) {
        let _ = self.command_tx.send(command);
    }
    
    /// Handle async commands (should be called from an async context)
    pub async fn handle_commands(&mut self, mut command_rx: mpsc::UnboundedReceiver<GitCommand>) {
        while let Some(command) = command_rx.recv().await {
            match command {
                GitCommand::SwitchToRef { git_ref } => {
                    if let Err(e) = self.switch_to_ref(&git_ref).await {
                        error!("Failed to switch to ref {}: {}", git_ref.name, e);
                    }
                }
                GitCommand::CreateBranch { name, start_point } => {
                    if let Err(e) = self.create_branch(&name, start_point.as_deref()).await {
                        error!("Failed to create branch {}: {}", name, e);
                    }
                }
                GitCommand::RefreshRefs { repo_name } => {
                    if let Err(e) = self.refresh_git_refs(&repo_name).await {
                        error!("Failed to refresh git refs for {}: {}", repo_name, e);
                    }
                }
                GitCommand::ForceSync { repo_name } => {
                    if let Some(sync_orchestrator) = &self.sync_orchestrator {
                        if let Some(repo_path) = self.get_repository_path(&repo_name).await {
                            if let Err(e) = sync_orchestrator.sync_repository(&repo_path).await {
                                error!("Failed to force sync repository {}: {}", repo_name, e);
                            }
                        }
                    }
                }
                GitCommand::UpdateRepository { repo_name } => {
                    if let Err(e) = self.set_current_repository(repo_name).await {
                        error!("Failed to update repository context: {}", e);
                    }
                }
                GitCommand::UpdateSyncStatuses => {
                    self.update_sync_statuses().await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Mutex;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_git_controls_creation() {
        let repo_manager = Arc::new(Mutex::new(
            RepositoryManager::new(Default::default())
        ));
        
        let git_controls = GitControls::new(repo_manager);
        assert!(git_controls.state.current_repository.is_none());
        assert!(git_controls.state.available_refs.is_empty());
    }

    #[tokio::test]
    async fn test_set_current_repository() {
        let repo_manager = Arc::new(Mutex::new(
            RepositoryManager::new(Default::default())
        ));
        
        let mut git_controls = GitControls::new(repo_manager);
        
        // This will fail because the repository doesn't exist, but we test the state change
        let _ = git_controls.set_current_repository(Some("test-repo".to_string())).await;
        assert_eq!(git_controls.state.current_repository, Some("test-repo".to_string()));
    }

    #[test]
    fn test_git_ref_type_serialization() {
        let branch_ref = GitRef {
            name: "main".to_string(),
            ref_type: GitRefType::Branch,
            hash: "abc123".to_string(),
            is_current: true,
        };

        let serialized = serde_json::to_string(&branch_ref).unwrap();
        let deserialized: GitRef = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.name, "main");
        assert_eq!(deserialized.ref_type, GitRefType::Branch);
        assert!(deserialized.is_current);
    }
}