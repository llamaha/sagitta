use anyhow::{Result, Context};
use log::{debug, error, info, warn};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;

/// Configuration for the file watcher service
#[derive(Debug, Clone)]
pub struct FileWatcherConfig {
    /// Whether file watching is enabled
    pub enabled: bool,
    /// Debounce interval in milliseconds to avoid excessive triggers
    pub debounce_ms: u64,
    /// Patterns to exclude from watching (relative to repo root)
    pub exclude_patterns: Vec<String>,
    /// Maximum number of events to buffer before processing
    pub max_buffer_size: usize,
}

impl Default for FileWatcherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            debounce_ms: 2000, // 2 seconds debounce like aider
            exclude_patterns: vec![
                ".git/".to_string(),
                "target/".to_string(),
                "node_modules/".to_string(),
                ".cache/".to_string(),
                "build/".to_string(),
                "dist/".to_string(),
                ".next/".to_string(),
                "__pycache__/".to_string(),
                "*.tmp".to_string(),
                "*.temp".to_string(),
                "*.swp".to_string(),
                "*.swo".to_string(),
                "*~".to_string(),
                ".DS_Store".to_string(),
                "Thumbs.db".to_string(),
            ],
            max_buffer_size: 1000,
        }
    }
}

/// Represents a file change event after debouncing
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    /// The repository path where the change occurred
    pub repo_path: PathBuf,
    /// The specific file that changed (relative to repo root)
    pub file_path: PathBuf,
    /// The type of change (created, modified, removed)
    pub change_type: FileChangeType,
    /// Timestamp when the change was detected
    pub timestamp: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileChangeType {
    Created,
    Modified,
    Removed,
}

/// Internal event for tracking changes before debouncing
#[derive(Debug, Clone)]
struct PendingChange {
    repo_path: PathBuf,
    file_path: PathBuf,
    change_type: FileChangeType,
    last_seen: Instant,
}

/// File watcher service that monitors git repositories for changes
pub struct FileWatcherService {
    config: FileWatcherConfig,
    /// Map of repository path to watcher
    watchers: Arc<RwLock<HashMap<PathBuf, RecommendedWatcher>>>,
    /// Pending changes waiting for debounce
    pending_changes: Arc<RwLock<HashMap<PathBuf, PendingChange>>>,
    /// Channel for receiving file system events
    event_rx: Option<mpsc::UnboundedReceiver<notify::Result<Event>>>,
    event_tx: mpsc::UnboundedSender<notify::Result<Event>>,
    /// Channel for sending processed file change events
    change_tx: mpsc::UnboundedSender<FileChangeEvent>,
    change_rx: Option<mpsc::UnboundedReceiver<FileChangeEvent>>,
}

impl FileWatcherService {
    /// Create a new file watcher service
    pub fn new(config: FileWatcherConfig) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (change_tx, change_rx) = mpsc::unbounded_channel();

        Self {
            config,
            watchers: Arc::new(RwLock::new(HashMap::new())),
            pending_changes: Arc::new(RwLock::new(HashMap::new())),
            event_rx: Some(event_rx),
            event_tx,
            change_tx,
            change_rx: Some(change_rx),
        }
    }

    /// Start the file watcher service
    pub async fn start(&mut self) -> Result<mpsc::UnboundedReceiver<FileChangeEvent>> {
        if !self.config.enabled {
            info!("File watcher service is disabled");
            return Ok(self.change_rx.take().unwrap());
        }

        info!("Starting file watcher service with debounce: {}ms", self.config.debounce_ms);

        // Take the receiver so we can move it into the task
        let event_rx = self.event_rx.take().context("Event receiver already taken")?;
        
        // Clone necessary data for the background task
        let pending_changes = Arc::clone(&self.pending_changes);
        let change_tx = self.change_tx.clone();
        let config = self.config.clone();

        // Start the event processing task
        tokio::spawn(async move {
            Self::process_events(event_rx, pending_changes, change_tx, config).await;
        });

        // Start the debounce processing task
        let pending_changes_debounce = Arc::clone(&self.pending_changes);
        let change_tx_debounce = self.change_tx.clone();
        let debounce_interval = Duration::from_millis(self.config.debounce_ms);

        tokio::spawn(async move {
            Self::process_debounce(pending_changes_debounce, change_tx_debounce, debounce_interval).await;
        });

        Ok(self.change_rx.take().unwrap())
    }

    /// Add a repository to watch for changes
    pub async fn watch_repository(&self, repo_path: &Path) -> Result<()> {
        if !self.config.enabled {
            debug!("File watching disabled, not watching repository: {}", repo_path.display());
            return Ok(());
        }

        let repo_path = repo_path.canonicalize()
            .context("Failed to canonicalize repository path")?;

        info!("Adding file watcher for repository: {}", repo_path.display());

        let event_tx = self.event_tx.clone();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                if let Err(e) = event_tx.send(res) {
                    error!("Failed to send file watcher event: {}", e);
                }
            },
            Config::default(),
        ).context("Failed to create file watcher")?;

        watcher.watch(&repo_path, RecursiveMode::Recursive)
            .context("Failed to start watching repository")?;

        // Store the watcher
        let mut watchers = self.watchers.write().await;
        if let Some(_old_watcher) = watchers.insert(repo_path.clone(), watcher) {
            debug!("Replaced existing watcher for repository: {}", repo_path.display());
        }

        Ok(())
    }

    /// Remove a repository from being watched
    pub async fn unwatch_repository(&self, repo_path: &Path) -> Result<()> {
        let repo_path = repo_path.canonicalize()
            .context("Failed to canonicalize repository path")?;

        info!("Removing file watcher for repository: {}", repo_path.display());

        let mut watchers = self.watchers.write().await;
        if watchers.remove(&repo_path).is_some() {
            debug!("Successfully removed watcher for repository: {}", repo_path.display());
        } else {
            warn!("No watcher found for repository: {}", repo_path.display());
        }

        // Also clean up any pending changes for this repository
        let mut pending = self.pending_changes.write().await;
        pending.retain(|path, _| !path.starts_with(&repo_path));

        Ok(())
    }

    /// Check if a file should be watched based on exclude patterns
    fn should_watch_file(&self, file_path: &Path, repo_path: &Path) -> bool {
        let relative_path = match file_path.strip_prefix(repo_path) {
            Ok(path) => path,
            Err(_) => return false, // File is not under the repository
        };

        let relative_str = relative_path.to_string_lossy();

        // Check exclude patterns
        for pattern in &self.config.exclude_patterns {
            if pattern.ends_with('/') {
                // Directory pattern
                if relative_str.starts_with(pattern) {
                    return false;
                }
            } else if pattern.contains('*') {
                // Glob pattern - simple implementation
                if pattern.starts_with("*.") {
                    let extension = &pattern[2..];
                    if relative_str.ends_with(&format!(".{}", extension)) {
                        return false;
                    }
                } else if pattern.ends_with("*") {
                    let prefix = &pattern[..pattern.len() - 1];
                    if relative_str.starts_with(prefix) {
                        return false;
                    }
                }
            } else {
                // Exact match
                if relative_str == *pattern || relative_str.ends_with(&format!("/{}", pattern)) {
                    return false;
                }
            }
        }

        true
    }

    /// Process incoming file system events
    async fn process_events(
        mut event_rx: mpsc::UnboundedReceiver<notify::Result<Event>>,
        pending_changes: Arc<RwLock<HashMap<PathBuf, PendingChange>>>,
        _change_tx: mpsc::UnboundedSender<FileChangeEvent>,
        config: FileWatcherConfig,
    ) {
        while let Some(event_result) = event_rx.recv().await {
            match event_result {
                Ok(event) => {
                    if let Err(e) = Self::handle_file_event(event, &pending_changes, &config).await {
                        error!("Error handling file event: {}", e);
                    }
                }
                Err(e) => {
                    error!("File watcher error: {}", e);
                }
            }
        }
        
        info!("File event processing task ended");
    }

    /// Handle a single file system event
    async fn handle_file_event(
        event: Event,
        pending_changes: &Arc<RwLock<HashMap<PathBuf, PendingChange>>>,
        config: &FileWatcherConfig,
    ) -> Result<()> {
        for path in event.paths {
            // Find the repository root for this file
            let repo_path = Self::find_repository_root(&path)?;
            
            // Create FileWatcherService instance to access should_watch_file
            let temp_service = FileWatcherService {
                config: config.clone(),
                watchers: Arc::new(RwLock::new(HashMap::new())),
                pending_changes: Arc::new(RwLock::new(HashMap::new())),
                event_rx: None,
                event_tx: mpsc::unbounded_channel().0,
                change_tx: mpsc::unbounded_channel().0,
                change_rx: None,
            };

            if !temp_service.should_watch_file(&path, &repo_path) {
                continue;
            }

            let change_type = match event.kind {
                EventKind::Create(_) => FileChangeType::Created,
                EventKind::Modify(_) => FileChangeType::Modified,
                EventKind::Remove(_) => FileChangeType::Removed,
                _ => continue, // Ignore other event types
            };

            let file_path = path.strip_prefix(&repo_path)?.to_path_buf();
            let change_key = path.clone();

            let pending_change = PendingChange {
                repo_path: repo_path.clone(),
                file_path,
                change_type,
                last_seen: Instant::now(),
            };

            let mut pending = pending_changes.write().await;
            if pending.len() >= config.max_buffer_size {
                warn!("Pending changes buffer is full, dropping oldest changes");
                // Keep only the most recent changes
                let cutoff = pending.len() - config.max_buffer_size / 2;
                let mut changes: Vec<_> = pending.drain().collect();
                changes.sort_by_key(|(_, change)| change.last_seen);
                changes.truncate(changes.len() - cutoff);
                pending.extend(changes);
            }
            
            pending.insert(change_key, pending_change);
        }

        Ok(())
    }

    /// Process debounced changes and emit file change events
    async fn process_debounce(
        pending_changes: Arc<RwLock<HashMap<PathBuf, PendingChange>>>,
        change_tx: mpsc::UnboundedSender<FileChangeEvent>,
        debounce_interval: Duration,
    ) {
        let mut interval = interval(Duration::from_millis(500)); // Check every 500ms

        loop {
            interval.tick().await;

            let now = Instant::now();
            let mut to_emit = Vec::new();

            {
                let mut pending = pending_changes.write().await;
                pending.retain(|path, change| {
                    if now.duration_since(change.last_seen) >= debounce_interval {
                        // This change has been stable long enough, emit it
                        to_emit.push((path.clone(), change.clone()));
                        false // Remove from pending
                    } else {
                        true // Keep in pending
                    }
                });
            }

            // Emit the debounced changes
            for (_, change) in to_emit {
                let event = FileChangeEvent {
                    repo_path: change.repo_path,
                    file_path: change.file_path,
                    change_type: change.change_type,
                    timestamp: change.last_seen,
                };

                if let Err(e) = change_tx.send(event) {
                    error!("Failed to send file change event: {}", e);
                    break; // Receiver is gone
                }
            }
        }
    }

    /// Find the repository root for a given file path
    fn find_repository_root(file_path: &Path) -> Result<PathBuf> {
        let mut current = file_path;
        
        loop {
            if current.join(".git").exists() {
                return Ok(current.to_path_buf());
            }
            
            match current.parent() {
                Some(parent) => current = parent,
                None => return Err(anyhow::anyhow!("No git repository found for path: {}", file_path.display())),
            }
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &FileWatcherConfig {
        &self.config
    }

    /// Update the configuration
    pub fn update_config(&mut self, config: FileWatcherConfig) {
        self.config = config;
    }
}