use anyhow::{Result, Context};
use log::{debug, error, info};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Mutex};

use crate::config::types::AutoSyncConfig;
use crate::services::auto_commit::CommitResult;
use crate::services::file_watcher::{FileWatcherService, FileChangeEvent};
use crate::gui::repository::manager::RepositoryManager;

/// Represents the result of a sync operation
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// Repository path that was synced
    pub repo_path: PathBuf,
    /// Whether the sync was successful
    pub success: bool,
    /// Error message if sync failed
    pub error_message: Option<String>,
    /// Duration of the sync operation
    pub duration: Duration,
    /// Number of files processed during sync
    pub files_processed: Option<usize>,
    /// Timestamp when sync completed
    pub timestamp: Instant,
}

/// Tracks the sync status of repositories
#[derive(Debug, Clone)]
pub struct RepositorySyncStatus {
    /// Last successful sync timestamp
    pub last_sync: Option<Instant>,
    /// Last commit hash that was synced
    pub last_synced_commit: Option<String>,
    /// Whether the repository is currently being synced
    pub is_syncing: bool,
    /// Whether the repository is out of sync
    pub is_out_of_sync: bool,
    /// Last sync error, if any
    pub last_sync_error: Option<String>,
}

/// Orchestrates file watching, auto-commits, and repository syncing
pub struct SyncOrchestrator {
    config: AutoSyncConfig,
    /// Repository manager for sync operations
    repository_manager: Arc<Mutex<RepositoryManager>>,
    /// Repository sync statuses
    sync_statuses: Arc<RwLock<HashMap<PathBuf, RepositorySyncStatus>>>,
    /// Channel for sending sync results
    sync_result_tx: mpsc::UnboundedSender<SyncResult>,
    sync_result_rx: Option<mpsc::UnboundedReceiver<SyncResult>>,
    /// File watcher service
    file_watcher: Option<FileWatcherService>,
}

impl SyncOrchestrator {
    /// Create a new sync orchestrator
    pub fn new(config: AutoSyncConfig, repository_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        let (sync_result_tx, sync_result_rx) = mpsc::unbounded_channel();

        Self {
            config,
            repository_manager,
            sync_statuses: Arc::new(RwLock::new(HashMap::new())),
            sync_result_tx,
            sync_result_rx: Some(sync_result_rx),
            file_watcher: None,
        }
    }

    /// Start the sync orchestrator
    pub async fn start(&mut self) -> Result<mpsc::UnboundedReceiver<SyncResult>> {
        info!("Starting sync orchestrator");

        if !self.config.enabled {
            info!("Auto-sync is disabled");
            return Ok(self.sync_result_rx.take().unwrap());
        }

        // Initialize file watcher if enabled
        if self.config.file_watcher.enabled {
            let file_watcher_config = crate::services::file_watcher::FileWatcherConfig {
                enabled: self.config.file_watcher.enabled,
                debounce_ms: self.config.file_watcher.debounce_ms,
                exclude_patterns: self.config.file_watcher.exclude_patterns.clone(),
                max_buffer_size: self.config.file_watcher.max_buffer_size,
            };

            let mut file_watcher = FileWatcherService::new(file_watcher_config);
            let change_rx = file_watcher.start().await?;
            self.file_watcher = Some(file_watcher);

            // Start file change processing
            let sync_tx = self.sync_result_tx.clone();
            let repository_manager = Arc::clone(&self.repository_manager);
            let sync_statuses = Arc::clone(&self.sync_statuses);
            let config = self.config.clone();

            tokio::spawn(async move {
                Self::handle_file_changes(change_rx, sync_tx, repository_manager, sync_statuses, config).await;
            });
        }

        Ok(self.sync_result_rx.take().unwrap())
    }

    /// Handle commit results and trigger sync if needed
    pub async fn handle_commit_result(&self, commit_result: CommitResult) -> Result<()> {
        info!(
            "Handling commit result for {}: {} ({})",
            commit_result.repo_path.display(),
            commit_result.commit_message.lines().next().unwrap_or(""),
            &commit_result.commit_hash[..8]
        );

        // Update sync status to indicate out-of-sync
        {
            let mut statuses = self.sync_statuses.write().await;
            let status = statuses.entry(commit_result.repo_path.clone()).or_insert_with(|| {
                RepositorySyncStatus {
                    last_sync: None,
                    last_synced_commit: None,
                    is_syncing: false,
                    is_out_of_sync: true,
                    last_sync_error: None,
                }
            });
            status.is_out_of_sync = true;
        }

        // Trigger sync if configured
        if self.config.sync_after_commit {
            self.sync_repository(&commit_result.repo_path).await?;
        }

        Ok(())
    }

    /// Sync a repository using the MCP repository sync tool
    pub async fn sync_repository(&self, repo_path: &Path) -> Result<SyncResult> {
        let start_time = Instant::now();
        let repo_path = repo_path.to_path_buf();

        info!("Starting sync for repository: {}", repo_path.display());

        // Mark as syncing
        {
            let mut statuses = self.sync_statuses.write().await;
            let status = statuses.entry(repo_path.clone()).or_insert_with(|| {
                RepositorySyncStatus {
                    last_sync: None,
                    last_synced_commit: None,
                    is_syncing: false,
                    is_out_of_sync: false,
                    last_sync_error: None,
                }
            });
            status.is_syncing = true;
            status.last_sync_error = None;
        }

        // Perform the sync using RepositoryManager
        let sync_result = match self.perform_repository_sync(&repo_path).await {
            Ok(files_processed) => {
                let duration = start_time.elapsed();
                info!(
                    "Successfully synced repository {} in {:?} ({} files processed)",
                    repo_path.display(),
                    duration,
                    files_processed
                );

                // Update sync status
                {
                    let mut statuses = self.sync_statuses.write().await;
                    if let Some(status) = statuses.get_mut(&repo_path) {
                        status.last_sync = Some(Instant::now());
                        status.is_syncing = false;
                        status.is_out_of_sync = false;
                        status.last_sync_error = None;
                        // TODO: Get actual commit hash from git
                        status.last_synced_commit = None;
                    }
                }

                SyncResult {
                    repo_path: repo_path.clone(),
                    success: true,
                    error_message: None,
                    duration,
                    files_processed: Some(files_processed),
                    timestamp: Instant::now(),
                }
            }
            Err(e) => {
                let duration = start_time.elapsed();
                error!("Failed to sync repository {}: {}", repo_path.display(), e);

                // Update sync status
                {
                    let mut statuses = self.sync_statuses.write().await;
                    if let Some(status) = statuses.get_mut(&repo_path) {
                        status.is_syncing = false;
                        status.last_sync_error = Some(e.to_string());
                    }
                }

                SyncResult {
                    repo_path: repo_path.clone(),
                    success: false,
                    error_message: Some(e.to_string()),
                    duration,
                    files_processed: None,
                    timestamp: Instant::now(),
                }
            }
        };

        // Send sync result
        if let Err(e) = self.sync_result_tx.send(sync_result.clone()) {
            error!("Failed to send sync result: {}", e);
        }

        Ok(sync_result)
    }

    /// Perform repository sync using RepositoryManager
    async fn perform_repository_sync(&self, repo_path: &Path) -> Result<usize> {
        let mut repo_manager = self.repository_manager.lock().await;

        // Get repository name from path
        let repo_name = repo_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid repository path"))?;

        // Perform the sync using RepositoryManager
        repo_manager
            .sync_repository_with_options(repo_name, false) // false = not force sync
            .await
            .context("Failed to sync repository")?;

        // Since RepositoryManager doesn't expose file count, return a default value
        // In a more sophisticated implementation, we could hook into the progress reporter
        // to get this information
        Ok(0)
    }

    /// Handle file changes from the file watcher
    async fn handle_file_changes(
        mut change_rx: mpsc::UnboundedReceiver<FileChangeEvent>,
        _sync_tx: mpsc::UnboundedSender<SyncResult>,
        _repository_manager: Arc<Mutex<RepositoryManager>>,
        sync_statuses: Arc<RwLock<HashMap<PathBuf, RepositorySyncStatus>>>,
        _config: AutoSyncConfig,
    ) {
        info!("Starting file change handler for sync orchestrator");

        while let Some(change_event) = change_rx.recv().await {
            debug!(
                "File change detected: {} in {}",
                change_event.file_path.display(),
                change_event.repo_path.display()
            );

            // Mark repository as potentially out of sync
            {
                let mut statuses = sync_statuses.write().await;
                let status = statuses.entry(change_event.repo_path.clone()).or_insert_with(|| {
                    RepositorySyncStatus {
                        last_sync: None,
                        last_synced_commit: None,
                        is_syncing: false,
                        is_out_of_sync: false,
                        last_sync_error: None,
                    }
                });
                status.is_out_of_sync = true;
            }
        }

        info!("File change handler for sync orchestrator stopped");
    }

    /// Add a repository to watch and sync
    pub async fn add_repository(&mut self, repo_path: &Path) -> Result<()> {
        info!("Adding repository to sync orchestrator: {}", repo_path.display());

        // Add to file watcher if available
        if let Some(ref file_watcher) = self.file_watcher {
            file_watcher.watch_repository(repo_path).await?;
        }

        // Initialize sync status
        {
            let mut statuses = self.sync_statuses.write().await;
            statuses.insert(
                repo_path.to_path_buf(),
                RepositorySyncStatus {
                    last_sync: None,
                    last_synced_commit: None,
                    is_syncing: false,
                    is_out_of_sync: true, // New repositories start as out-of-sync
                    last_sync_error: None,
                },
            );
        }

        // Trigger initial sync if configured
        if self.config.sync_on_repo_add {
            self.sync_repository(repo_path).await?;
        }

        Ok(())
    }

    /// Remove a repository from watching and syncing
    pub async fn remove_repository(&mut self, repo_path: &Path) -> Result<()> {
        info!("Removing repository from sync orchestrator: {}", repo_path.display());

        // Remove from file watcher if available
        if let Some(ref file_watcher) = self.file_watcher {
            file_watcher.unwatch_repository(repo_path).await?;
        }

        // Remove sync status
        {
            let mut statuses = self.sync_statuses.write().await;
            statuses.remove(&repo_path.to_path_buf());
        }

        Ok(())
    }

    /// Switch to a different repository (used when user changes repo in UI)
    pub async fn switch_repository(&self, repo_path: &Path) -> Result<()> {
        info!("Switching to repository: {}", repo_path.display());

        // Trigger sync if configured
        if self.config.sync_on_repo_switch {
            self.sync_repository(repo_path).await?;
        }

        Ok(())
    }

    /// Get sync status for a repository
    pub async fn get_sync_status(&self, repo_path: &Path) -> Option<RepositorySyncStatus> {
        let statuses = self.sync_statuses.read().await;
        statuses.get(&repo_path.to_path_buf()).cloned()
    }

    /// Get sync statuses for all repositories
    pub async fn get_all_sync_statuses(&self) -> HashMap<PathBuf, RepositorySyncStatus> {
        self.sync_statuses.read().await.clone()
    }

    /// Check if any repository is out of sync
    pub async fn has_out_of_sync_repositories(&self) -> bool {
        let statuses = self.sync_statuses.read().await;
        statuses.values().any(|status| status.is_out_of_sync && !status.is_syncing)
    }

    /// Force sync all repositories
    pub async fn sync_all_repositories(&self) -> Result<Vec<SyncResult>> {
        let repo_paths: Vec<PathBuf> = {
            let statuses = self.sync_statuses.read().await;
            statuses.keys().cloned().collect()
        };

        let mut results = Vec::new();
        for repo_path in repo_paths {
            match self.sync_repository(&repo_path).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Failed to sync repository {}: {}", repo_path.display(), e);
                    results.push(SyncResult {
                        repo_path,
                        success: false,
                        error_message: Some(e.to_string()),
                        duration: Duration::from_secs(0),
                        files_processed: None,
                        timestamp: Instant::now(),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Update configuration
    pub fn update_config(&mut self, config: AutoSyncConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &AutoSyncConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_get_sync_status_nonexistent() {
        let config = AutoSyncConfig::default();
        // Mock repository manager - in real tests you'd create a proper mock
        let repository_manager = Arc::new(Mutex::new(
            RepositoryManager::new(Default::default())
        ));
        
        let orchestrator = SyncOrchestrator::new(config, repository_manager);
        let temp_dir = TempDir::new().unwrap();
        
        let status = orchestrator.get_sync_status(temp_dir.path()).await;
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_has_out_of_sync_repositories_empty() {
        let config = AutoSyncConfig::default();
        // Mock repository manager - in real tests you'd create a proper mock
        let repository_manager = Arc::new(Mutex::new(
            RepositoryManager::new(Default::default())
        ));
        
        let orchestrator = SyncOrchestrator::new(config, repository_manager);
        
        let has_out_of_sync = orchestrator.has_out_of_sync_repositories().await;
        assert!(!has_out_of_sync);
    }
}