// src/sync_progress.rs

use std::path::PathBuf;
use async_trait::async_trait;

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
}

/// Represents a progress update during repository synchronization.
#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub stage: SyncStage,
    // Potentially overall progress if calculable easily
    // pub overall_progress: Option<(usize, usize)>,
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