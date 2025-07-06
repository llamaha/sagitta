// src/sync_progress.rs

use std::path::PathBuf;
use async_trait::async_trait;
use std::time::{Duration, Instant};

/// Defines the different stages of a repository synchronization process.
#[derive(Debug, Clone)]
pub enum SyncStage {
    /// Git fetch operation in progress
    GitFetch { 
        /// Status message describing the current fetch operation
        message: String, 
        /// Progress as (received_objects, total_objects) if available
        progress: Option<(u32, u32)> 
    },
    /// Calculating differences between local and remote repository
    DiffCalculation { 
        /// Status message describing the diff calculation
        message: String 
    },
    /// Indexing individual files
    IndexFile { 
        /// Path of the file currently being indexed
        current_file: Option<PathBuf>, 
        /// Total number of files to index
        total_files: usize, 
        /// Index of the current file being processed (1-based)
        current_file_num: usize, 
        /// Current indexing speed in files per second
        files_per_second: Option<f64>, 
        /// Optional status message
        message: Option<String> 
    },
    /// Deleting files from the index
    DeleteFile { 
        /// Path of the file currently being deleted
        current_file: Option<PathBuf>, 
        /// Total number of files to delete
        total_files: usize, 
        /// Index of the current file being deleted (1-based)
        current_file_num: usize, 
        /// Current deletion speed in files per second
        files_per_second: Option<f64>, 
        /// Optional status message
        message: Option<String> 
    },
    /// Collecting files from the repository
    CollectFiles { 
        /// Number of files collected so far
        total_files: usize, 
        /// Status message describing the collection process
        message: String 
    },
    /// Querying languages from the indexed data
    QueryLanguages { 
        /// Status message describing the query operation
        message: String 
    },
    /// Verifying the integrity of the collection
    VerifyingCollection { 
        /// Status message describing the verification process
        message: String 
    },
    /// Synchronization completed successfully
    Completed { 
        /// Completion message with summary information
        message: String 
    },
    /// An error occurred during synchronization
    Error { 
        /// Error message describing what went wrong
        message: String 
    },
    /// Default state before sync starts or when idle
    Idle,
    /// Periodic heartbeat to indicate progress is still happening
    Heartbeat { 
        /// Heartbeat message indicating current activity
        message: String 
    },
}

/// Represents a progress update during repository synchronization.
#[derive(Debug, Clone)]
pub struct SyncProgress {
    /// Current stage of the synchronization process
    pub stage: SyncStage,
    /// Timestamp when this progress update was created
    pub timestamp: Option<Instant>,
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

/// Example of a No-Op reporter for when no specific reporter is provided.
/// This can be useful for default behavior or in contexts where progress reporting is not needed.
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

/// Defines the different stages of a repository addition process.
#[derive(Debug, Clone)]
pub enum RepoAddStage {
    /// Repository clone operation in progress
    Clone { 
        /// Status message describing the clone operation
        message: String, 
        /// Progress as (received_objects, total_objects) if available
        progress: Option<(u32, u32)> 
    },
    /// Repository fetch operation in progress
    Fetch { 
        /// Status message describing the fetch operation
        message: String, 
        /// Progress as (received_objects, total_objects) if available
        progress: Option<(u32, u32)> 
    },
    /// Checking out the repository to specific branch or commit
    Checkout { 
        /// Status message describing the checkout operation
        message: String 
    },
    /// Repository addition completed successfully
    Completed { 
        /// Completion message with summary information
        message: String 
    },
    /// An error occurred during repository addition
    Error { 
        /// Error message describing what went wrong
        message: String 
    },
    /// Default state before add starts or when idle
    Idle,
}

/// Represents a progress update during repository addition.
#[derive(Debug, Clone)]
pub struct AddProgress {
    /// Current stage of the repository addition process
    pub stage: RepoAddStage,
    /// Timestamp when this progress update was created
    pub timestamp: Option<Instant>,
}

impl AddProgress {
    /// Create a new progress update with current timestamp
    pub fn new(stage: RepoAddStage) -> Self {
        Self {
            stage,
            timestamp: Some(Instant::now()),
        }
    }
    
    /// Create a progress update without timestamp (for backwards compatibility)
    pub fn without_timestamp(stage: RepoAddStage) -> Self {
        Self {
            stage,
            timestamp: None,
        }
    }
}

/// Trait for reporting repository addition progress.
/// Implementors of this trait can decide how to display or log progress updates.
#[async_trait]
pub trait AddProgressReporter: Send + Sync {
    /// Called by the add process to report an update.
    async fn report(&self, progress: AddProgress);
}

/// Example of a No-Op reporter for when no specific reporter is provided.
#[derive(Debug, Clone)]
pub struct NoOpAddProgressReporter;

#[async_trait]
impl AddProgressReporter for NoOpAddProgressReporter {
    async fn report(&self, _progress: AddProgress) {
        // Does nothing
    }
}

#[cfg(test)]
mod add_progress_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::collections::VecDeque;

    /// Test progress reporter that captures progress updates for verification
    #[derive(Debug)]
    struct TestAddProgressReporter {
        progress_updates: Arc<Mutex<VecDeque<AddProgress>>>,
    }

    impl TestAddProgressReporter {
        fn new() -> Self {
            Self {
                progress_updates: Arc::new(Mutex::new(VecDeque::new())),
            }
        }

        fn get_updates(&self) -> Vec<AddProgress> {
            self.progress_updates.lock().unwrap().iter().cloned().collect()
        }

        fn get_update_count(&self) -> usize {
            self.progress_updates.lock().unwrap().len()
        }

        fn get_last_update(&self) -> Option<AddProgress> {
            self.progress_updates.lock().unwrap().back().cloned()
        }
    }

    #[async_trait]
    impl AddProgressReporter for TestAddProgressReporter {
        async fn report(&self, progress: AddProgress) {
            self.progress_updates.lock().unwrap().push_back(progress);
        }
    }

    #[tokio::test]
    async fn test_add_progress_reporter_basic() {
        let reporter = TestAddProgressReporter::new();
        
        // Test clone stage
        let clone_progress = AddProgress::new(RepoAddStage::Clone {
            message: "Cloning repository".to_string(),
            progress: Some((50, 100)),
        });
        reporter.report(clone_progress).await;

        // Test checkout stage
        let checkout_progress = AddProgress::new(RepoAddStage::Checkout {
            message: "Checking out branch main".to_string(),
        });
        reporter.report(checkout_progress).await;

        // Test completion
        let completed_progress = AddProgress::new(RepoAddStage::Completed {
            message: "Repository successfully added".to_string(),
        });
        reporter.report(completed_progress).await;

        // Verify updates
        assert_eq!(reporter.get_update_count(), 3);
        
        let updates = reporter.get_updates();
        assert!(matches!(updates[0].stage, RepoAddStage::Clone { .. }));
        assert!(matches!(updates[1].stage, RepoAddStage::Checkout { .. }));
        assert!(matches!(updates[2].stage, RepoAddStage::Completed { .. }));
    }

    #[tokio::test]
    async fn test_add_progress_with_error() {
        let reporter = TestAddProgressReporter::new();
        
        // Test error stage
        let error_progress = AddProgress::new(RepoAddStage::Error {
            message: "Failed to clone repository: permission denied".to_string(),
        });
        reporter.report(error_progress).await;

        // Verify error was captured
        assert_eq!(reporter.get_update_count(), 1);
        
        let last_update = reporter.get_last_update().unwrap();
        if let RepoAddStage::Error { message } = last_update.stage {
            assert!(message.contains("permission denied"));
        } else {
            panic!("Expected error stage");
        }
    }

    #[tokio::test]
    async fn test_add_progress_timestamps() {
        let reporter = TestAddProgressReporter::new();
        
        let progress = AddProgress::new(RepoAddStage::Clone {
            message: "Test".to_string(),
            progress: None,
        });
        
        // Verify timestamp is set
        assert!(progress.timestamp.is_some());
        
        reporter.report(progress).await;
        
        let updates = reporter.get_updates();
        assert!(updates[0].timestamp.is_some());
    }

    #[test]
    fn test_add_progress_without_timestamp() {
        let progress = AddProgress::without_timestamp(RepoAddStage::Idle);
        assert!(progress.timestamp.is_none());
    }

    #[tokio::test]
    async fn test_noop_add_progress_reporter() {
        let reporter = NoOpAddProgressReporter;
        
        // Should not panic or cause issues
        let progress = AddProgress::new(RepoAddStage::Clone {
            message: "Test".to_string(),
            progress: None,
        });
        
        reporter.report(progress).await;
        // No way to verify NoOp behavior other than it doesn't crash
    }
} 