// src/sync_progress.rs

use std::path::PathBuf;
use async_trait::async_trait;
use std::time::{Duration, Instant};

/// Defines the different stages of a repository synchronization process.
#[derive(Debug, Clone)]
pub enum SyncStage {
    GitFetch { message: String, progress: Option<(u32, u32)> }, // (received_objects, total_objects)
    DiffCalculation { message: String },
    IndexFile { current_file: Option<PathBuf>, total_files: usize, current_file_num: usize, files_per_second: Option<f64>, message: Option<String> },
    DeleteFile { current_file: Option<PathBuf>, total_files: usize, current_file_num: usize, files_per_second: Option<f64>, message: Option<String> },
    CollectFiles { total_files: usize, message: String },
    QueryLanguages { message: String },
    VerifyingCollection { message: String },
    Completed { message: String },
    Error { message: String },
    Idle, // Default state or before sync starts
    Heartbeat { message: String }, // Periodic heartbeat to indicate progress is still happening
}

/// Represents a progress update during repository synchronization.
#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub stage: SyncStage,
    pub timestamp: Option<Instant>, // Timestamp when this progress was created
    // Potentially overall progress if calculable easily
    // pub overall_progress: Option<(usize, usize)>,
}

impl SyncProgress {
    /// Create a new progress update with current timestamp
    pub fn new(stage: SyncStage) -> Self {
        Self {
            stage,
            timestamp: Some(Instant::now()),
        }
    }
    
    /// Create a progress update without timestamp (for backwards compatibility)
    pub fn without_timestamp(stage: SyncStage) -> Self {
        Self {
            stage,
            timestamp: None,
        }
    }
}

/// Watchdog configuration for sync operations
#[derive(Debug, Clone)]
pub struct SyncWatchdogConfig {
    /// Maximum time without progress updates before considering sync stuck (default: 120 seconds)
    pub max_idle_duration: Duration,
    /// Interval for sending heartbeat updates during long operations (default: 30 seconds)
    pub heartbeat_interval: Duration,
    /// Whether watchdog is enabled (default: true)
    pub enabled: bool,
}

impl Default for SyncWatchdogConfig {
    fn default() -> Self {
        Self {
            max_idle_duration: Duration::from_secs(120), // 2 minutes without progress
            heartbeat_interval: Duration::from_secs(30), // Heartbeat every 30 seconds
            enabled: true,
        }
    }
}

/// Watchdog for monitoring sync progress and detecting stuck operations
#[derive(Debug)]
pub struct SyncWatchdog {
    config: SyncWatchdogConfig,
    last_progress_time: Option<Instant>,
    is_active: bool,
}

impl SyncWatchdog {
    /// Create a new sync watchdog with default configuration
    pub fn new() -> Self {
        Self::with_config(SyncWatchdogConfig::default())
    }
    
    /// Create a new sync watchdog with custom configuration
    pub fn with_config(config: SyncWatchdogConfig) -> Self {
        Self {
            config,
            last_progress_time: None,
            is_active: false,
        }
    }
    
    /// Start the watchdog
    pub fn start(&mut self) {
        self.is_active = true;
        self.last_progress_time = Some(Instant::now());
    }
    
    /// Stop the watchdog
    pub fn stop(&mut self) {
        self.is_active = false;
        self.last_progress_time = None;
    }
    
    /// Update the watchdog with new progress
    pub fn update_progress(&mut self, _progress: &SyncProgress) {
        if self.is_active {
            self.last_progress_time = Some(Instant::now());
        }
    }
    
    /// Check if the sync operation appears to be stuck
    pub fn is_stuck(&self) -> bool {
        if !self.config.enabled || !self.is_active {
            return false;
        }
        
        if let Some(last_progress) = self.last_progress_time {
            last_progress.elapsed() > self.config.max_idle_duration
        } else {
            false
        }
    }
    
    /// Get time since last progress update
    pub fn time_since_last_progress(&self) -> Option<Duration> {
        self.last_progress_time.map(|t| t.elapsed())
    }
    
    /// Check if a heartbeat should be sent
    pub fn should_send_heartbeat(&self) -> bool {
        if !self.config.enabled || !self.is_active {
            return false;
        }
        
        if let Some(last_progress) = self.last_progress_time {
            last_progress.elapsed() >= self.config.heartbeat_interval
        } else {
            false
        }
    }
}

impl Default for SyncWatchdog {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for reporting synchronization progress.
/// Implementors of this trait can decide how to display or log progress updates.
#[async_trait]
pub trait SyncProgressReporter: Send + Sync {
    /// Called by the sync process to report an update.
    async fn report(&self, progress: SyncProgress);
}

// Example of a No-Op reporter for when no specific reporter is provided.
// This can be useful for default behavior or in contexts where progress reporting is not needed.
#[derive(Debug, Clone)]
pub struct NoOpProgressReporter;

#[async_trait]
impl SyncProgressReporter for NoOpProgressReporter {
    async fn report(&self, _progress: SyncProgress) {
        // Does nothing
    }
}

/// Progress reporter that includes watchdog functionality
#[derive(Debug)]
pub struct WatchdogProgressReporter<T: SyncProgressReporter> {
    inner: T,
    watchdog: std::sync::Mutex<SyncWatchdog>,
}

impl<T: SyncProgressReporter> WatchdogProgressReporter<T> {
    /// Create a new watchdog progress reporter
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            watchdog: std::sync::Mutex::new(SyncWatchdog::new()),
        }
    }
    
    /// Create a new watchdog progress reporter with custom config
    pub fn with_config(inner: T, config: SyncWatchdogConfig) -> Self {
        Self {
            inner,
            watchdog: std::sync::Mutex::new(SyncWatchdog::with_config(config)),
        }
    }
    
    /// Start the watchdog
    pub fn start_watchdog(&self) {
        if let Ok(mut watchdog) = self.watchdog.lock() {
            watchdog.start();
        }
    }
    
    /// Stop the watchdog
    pub fn stop_watchdog(&self) {
        if let Ok(mut watchdog) = self.watchdog.lock() {
            watchdog.stop();
        }
    }
    
    /// Check if the operation appears stuck
    pub fn is_stuck(&self) -> bool {
        if let Ok(watchdog) = self.watchdog.lock() {
            watchdog.is_stuck()
        } else {
            false
        }
    }
    
    /// Get time since last progress
    pub fn time_since_last_progress(&self) -> Option<Duration> {
        if let Ok(watchdog) = self.watchdog.lock() {
            watchdog.time_since_last_progress()
        } else {
            None
        }
    }
}

#[async_trait]
impl<T: SyncProgressReporter> SyncProgressReporter for WatchdogProgressReporter<T> {
    async fn report(&self, progress: SyncProgress) {
        // Update watchdog
        if let Ok(mut watchdog) = self.watchdog.lock() {
            watchdog.update_progress(&progress);
        }
        
        // Forward to inner reporter
        self.inner.report(progress).await;
    }
} 