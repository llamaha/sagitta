use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use anyhow::Result;
use log::{debug, info, warn, error};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebouncedEvent, FileIdCache};
use serde::{Serialize, Deserialize};

use crate::vectordb::VectorDB;
use crate::utils::git::GitRepo;

/// Configuration for auto-sync of a repository
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AutoSyncConfig {
    /// Whether auto-sync is enabled for this repository
    pub enabled: bool,
    /// Minimum interval between syncs (in seconds)
    pub min_interval: u64,
}

impl Default for AutoSyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_interval: 60, // Default to 60 seconds between syncs
        }
    }
}

/// AutoSync daemon that monitors repositories and triggers syncs
pub struct AutoSyncDaemon {
    /// Thread handle for the daemon
    thread_handle: Option<thread::JoinHandle<()>>,
    /// Flag to control daemon thread lifecycle
    running: Arc<AtomicBool>,
    /// Last sync time for each repository
    last_sync: Arc<Mutex<HashMap<String, Instant>>>,
    /// Database instance for syncing
    db: Arc<Mutex<VectorDB>>,
    /// Signal to stop the auto-sync daemon
    stop_tx: Option<mpsc::Sender<()>>,
}

impl AutoSyncDaemon {
    /// Create a new auto-sync daemon
    pub fn new(db: VectorDB) -> Self {
        Self {
            thread_handle: None,
            running: Arc::new(AtomicBool::new(false)),
            last_sync: Arc::new(Mutex::new(HashMap::new())),
            db: Arc::new(Mutex::new(db)),
            stop_tx: None,
        }
    }

    /// Start the auto-sync daemon in a background thread
    pub fn start(&mut self) -> Result<()> {
        // Check if already running
        if self.running.load(Ordering::SeqCst) {
            return Ok(()); // Already running
        }

        debug!("Starting auto-sync daemon");
        
        // Mark as running
        self.running.store(true, Ordering::SeqCst);
        
        // Create channel for stopping the daemon
        let (stop_tx, stop_rx) = mpsc::channel();
        self.stop_tx = Some(stop_tx);
        
        // Clone shared state for thread
        let running = self.running.clone();
        let last_sync = self.last_sync.clone();
        let db = self.db.clone();
        
        // Start daemon thread
        let handle = thread::spawn(move || {
            Self::daemon_loop(running, last_sync, db, stop_rx);
        });
        
        self.thread_handle = Some(handle);
        
        info!("Auto-sync daemon started");
        Ok(())
    }

    /// Stop the auto-sync daemon
    pub fn stop(&mut self) -> Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(()); // Not running
        }
        
        debug!("Stopping auto-sync daemon");
        
        // Signal thread to stop
        self.running.store(false, Ordering::SeqCst);
        
        // Send stop signal
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        
        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
        
        info!("Auto-sync daemon stopped");
        Ok(())
    }

    /// Main daemon loop
    fn daemon_loop(
        running: Arc<AtomicBool>,
        last_sync: Arc<Mutex<HashMap<String, Instant>>>,
        db: Arc<Mutex<VectorDB>>,
        stop_rx: mpsc::Receiver<()>
    ) {
        info!("Auto-sync daemon running");
        
        // Create channel for watcher events
        let (tx, rx) = mpsc::channel();
        
        // Create debouncer for file events
        let mut debouncer = match new_debouncer(
            Duration::from_secs(2), // Debounce file events for 2 seconds
            None,
            tx
        ) {
            Ok(debouncer) => debouncer,
            Err(e) => {
                error!("Failed to create file watcher: {}", e);
                return;
            }
        };
        
        // Track currently watched repositories
        let mut watched_repos: HashMap<String, PathBuf> = HashMap::new();
        
        // Update watchers periodically
        let mut next_watcher_update = Instant::now();
        
        // Main event loop
        while running.load(Ordering::SeqCst) {
            // Update watchers every 30 seconds
            if Instant::now() > next_watcher_update {
                if let Err(e) = Self::update_watchers(&mut debouncer, &mut watched_repos, &db) {
                    error!("Failed to update watchers: {}", e);
                }
                next_watcher_update = Instant::now() + Duration::from_secs(30);
            }
            
            // Handle file events with a timeout
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Ok(events)) => {
                    Self::handle_file_events(events, &watched_repos, &last_sync, &db);
                },
                Ok(Err(e)) => {
                    error!("File watcher error: {:?}", e);
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Timeout is normal, continue with the next iteration
                },
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    error!("File watcher disconnected");
                    break;
                },
            }
            
            // Check for stop signal
            if stop_rx.try_recv().is_ok() {
                debug!("Stop signal received by auto-sync daemon");
                break;
            }
        }
        
        // Clean up watchers
        for (_, path) in watched_repos {
            let _ = debouncer.watcher().unwatch(&path);
        }
        info!("Auto-sync daemon shutdown complete");
    }
    
    /// Update watchers for repositories with auto-sync enabled
    fn update_watchers<T: FileIdCache>(
        debouncer: &mut notify_debouncer_full::Debouncer<RecommendedWatcher, T>,
        watched_repos: &mut HashMap<String, PathBuf>,
        db: &Arc<Mutex<VectorDB>>
    ) -> Result<()> {
        // Lock database
        let db_locked = db.lock().unwrap();
        
        // Get repos with auto-sync enabled
        let auto_sync_repos: Vec<_> = db_locked.repo_manager.list_repositories().iter()
            .filter(|repo| repo.active && repo.auto_sync.enabled)
            .map(|repo| (repo.id.clone(), repo.path.clone()))
            .collect();
        
        // Remove watchers for repositories that no longer need watching
        let mut to_remove = Vec::new();
        for (id, _) in watched_repos.iter() {
            if !auto_sync_repos.iter().any(|(r_id, _)| r_id == id) {
                to_remove.push(id.clone());
            }
        }
        
        for id in to_remove {
            if let Some(path) = watched_repos.remove(&id) {
                debug!("Removing watcher for repository: {}", path.display());
                if let Err(e) = debouncer.watcher().unwatch(&path) {
                    warn!("Failed to unwatch repository {}: {}", path.display(), e);
                }
            }
        }
        
        // Add watchers for new repositories
        for (id, path) in auto_sync_repos {
            if !watched_repos.contains_key(&id) && path.exists() {
                debug!("Adding watcher for repository: {}", path.display());
                match debouncer.watcher().watch(&path, RecursiveMode::Recursive) {
                    Ok(_) => {
                        watched_repos.insert(id.clone(), path.clone());
                    },
                    Err(e) => {
                        error!("Failed to watch repository {}: {}", path.display(), e);
                    }
                }
            }
        }
        
        debug!("Now watching {} repositories", watched_repos.len());
        Ok(())
    }
    
    /// Handle file events from the watcher
    fn handle_file_events(
        events: Vec<DebouncedEvent>,
        watched_repos: &HashMap<String, PathBuf>,
        last_sync: &Arc<Mutex<HashMap<String, Instant>>>,
        db: &Arc<Mutex<VectorDB>>
    ) {
        // Group events by repository
        let mut repo_events: HashMap<String, Vec<PathBuf>> = HashMap::new();
        
        for event in events {
            // Each event might have multiple paths affected
            for event_path in &event.event.paths {
                // Find which repository this event belongs to
                for (id, repo_path) in watched_repos {
                    if event_path.starts_with(repo_path) {
                        repo_events.entry(id.clone())
                            .or_insert_with(Vec::new)
                            .push(event_path.clone());
                        break;
                    }
                }
            }
        }
        
        // Process events for each repository
        for (repo_id, _) in repo_events {
            // Check if we need to sync based on time interval
            let should_sync = {
                let mut last_sync_map = last_sync.lock().unwrap();
                let now = Instant::now();
                
                let db_locked = match db.lock() {
                    Ok(db) => db,
                    Err(e) => {
                        error!("Failed to lock database: {}", e);
                        continue;
                    }
                };
                
                let repo = match db_locked.repo_manager.get_repository(&repo_id) {
                    Some(repo) => repo,
                    None => continue,
                };
                
                let min_interval = repo.auto_sync.min_interval;
                
                if let Some(last_time) = last_sync_map.get(&repo_id) {
                    if now.duration_since(*last_time) < Duration::from_secs(min_interval) {
                        debug!("Skipping sync for {} due to interval limit", repo_id);
                        false
                    } else {
                        last_sync_map.insert(repo_id.clone(), now);
                        true
                    }
                } else {
                    last_sync_map.insert(repo_id.clone(), now);
                    true
                }
            };
            
            if !should_sync {
                continue;
            }
            
            // Perform sync in a separate thread to avoid blocking the watcher
            let db_clone = db.clone();
            let repo_id_clone = repo_id.clone();
            
            thread::spawn(move || {
                debug!("Auto-syncing repository: {}", repo_id_clone);
                
                let mut db_locked = match db_clone.lock() {
                    Ok(db) => db,
                    Err(e) => {
                        error!("Failed to lock database for sync: {}", e);
                        return;
                    }
                };
                
                // Get repository information
                let (repo_path, repo_name, branch) = {
                    let repo = match db_locked.repo_manager.get_repository(&repo_id_clone) {
                        Some(repo) => repo,
                        None => {
                            error!("Repository no longer exists: {}", repo_id_clone);
                            return;
                        }
                    };
                    
                    (repo.path.clone(), repo.name.clone(), repo.active_branch.clone())
                };
                
                // Check if Git HEAD has actually changed
                match GitRepo::new(repo_path.clone()) {
                    Ok(git_repo) => {
                        // Get current and indexed commit hashes
                        let current_commit = match git_repo.get_commit_hash(&branch) {
                            Ok(hash) => hash,
                            Err(e) => {
                                error!("Failed to get current commit hash: {}", e);
                                return;
                            }
                        };
                        
                        let repo = match db_locked.repo_manager.get_repository(&repo_id_clone) {
                            Some(repo) => repo,
                            None => return,
                        };
                        
                        let needs_sync = match repo.get_indexed_commit(&branch) {
                            Some(indexed_commit) => indexed_commit != &current_commit,
                            None => true, // Never indexed before
                        };
                        
                        if needs_sync {
                            info!("Auto-syncing repository {} ({}), branch {}", repo_name, repo_id_clone, branch);
                            
                            // Perform incremental sync
                            if let Err(e) = db_locked.index_repository_changes(&repo_id_clone, &branch) {
                                error!("Failed to auto-sync repository {}: {}", repo_id_clone, e);
                            } else {
                                info!("Auto-sync completed for repository {}", repo_id_clone);
                            }
                        } else {
                            debug!("No Git changes detected, skipping auto-sync for {}", repo_id_clone);
                        }
                    },
                    Err(e) => {
                        error!("Failed to access Git repository {}: {}", repo_id_clone, e);
                    }
                }
            });
        }
    }
}

impl Clone for AutoSyncDaemon {
    fn clone(&self) -> Self {
        // For cloning, we create a new daemon that isn't running
        // The running daemon remains with the original instance
        Self {
            thread_handle: None,
            running: Arc::new(AtomicBool::new(false)),
            last_sync: Arc::new(Mutex::new(HashMap::new())),
            db: self.db.clone(),
            stop_tx: None,
        }
    }
} 