use async_trait::async_trait;
use sagitta_search::sync_progress::{SyncProgress, SyncProgressReporter as CoreSyncProgressReporter, SyncStage};
use sagitta_search::sync_progress::{AddProgress, AddProgressReporter as CoreAddProgressReporter, RepoAddStage};
use crate::gui::repository::shared_sync_state::{SIMPLE_STATUS, DETAILED_STATUS};
use crate::gui::repository::types::{SimpleSyncStatus, DisplayableSyncProgress, DisplayableAddProgress};

// Using String for RepositoryId as per observations in manager.rs
pub type RepositoryId = String;

/// Message to send to the GUI thread containing progress information.
#[derive(Debug, Clone)]
pub struct GuiSyncReport {
    pub repo_id: RepositoryId,
    pub progress: SyncProgress, // This is sagitta_search::sync_progress::SyncProgress
}

/// Implements the SyncProgressReporter trait to send progress updates to the global state.
pub struct GuiProgressReporter {
    repo_id: RepositoryId,
}

impl GuiProgressReporter {
    pub fn new(repo_id: RepositoryId) -> Self {
        Self { repo_id }
    }
}

#[async_trait]
impl CoreSyncProgressReporter for GuiProgressReporter {
    async fn report(&self, progress: SyncProgress) {
        // Convert core progress into the GUI flavour
        // We don't have elapsed time here, but the GUI can calculate it.
        let displayable = DisplayableSyncProgress::from_core_progress(&progress, 0.0);

        // Update the global maps
        DETAILED_STATUS.insert(self.repo_id.clone(), displayable.clone());

        // Compress into SimpleSyncStatus so the existing panel code can stay almost untouched
        let mut simple = SIMPLE_STATUS
            .entry(self.repo_id.clone())
            .or_insert_with(SimpleSyncStatus::default);

        simple.is_running = matches!(progress.stage, 
            SyncStage::GitFetch { .. } | 
            SyncStage::DiffCalculation { .. } | 
            SyncStage::IndexFile { .. } | 
            SyncStage::DeleteFile { .. } | 
            SyncStage::CollectFiles { .. } | 
            SyncStage::QueryLanguages { .. } | 
            SyncStage::VerifyingCollection { .. } |
            SyncStage::Heartbeat { .. }
        );
        simple.is_complete = matches!(progress.stage, SyncStage::Completed { .. } | SyncStage::Error { .. });
        simple.is_success  = matches!(progress.stage, SyncStage::Completed { .. });
        
        // Update last progress time for watchdog monitoring
        simple.last_progress_time = progress.timestamp.or_else(|| Some(std::time::Instant::now()));

        if simple.output_lines.len() > 100 { // Limit log lines
            simple.output_lines.remove(0);
        }
        simple.output_lines.push(displayable.message.clone());

        if simple.is_complete {
            if let Some(started_at) = simple.started_at {
                let duration = started_at.elapsed();
                let final_elapsed_seconds = duration.as_secs_f64();
                let final_message = format!(
                    "{} in {:.2}s",
                    if simple.is_success { "✅ Completed" } else { "❌ Failed" },
                    duration.as_secs_f32()
                );
                simple.final_message = final_message;
                simple.final_elapsed_seconds = Some(final_elapsed_seconds);
            } else {
                simple.final_message = if simple.is_success { "✅ Completed" } else { "❌ Failed" }.to_string();
                simple.final_elapsed_seconds = None; // No start time available
            }
        }
    }
}

#[async_trait]
impl CoreAddProgressReporter for GuiProgressReporter {
    async fn report(&self, progress: AddProgress) {
        // Convert add progress into a displayable format similar to sync progress
        let displayable = DisplayableAddProgress::from_core_progress(&progress, 0.0);

        // Update the global maps (reuse the same infrastructure as sync)
        DETAILED_STATUS.insert(self.repo_id.clone(), displayable.clone().into());

        // Compress into SimpleSyncStatus so the existing panel code can stay almost untouched
        let mut simple = SIMPLE_STATUS
            .entry(self.repo_id.clone())
            .or_insert_with(SimpleSyncStatus::default);

        simple.is_running = matches!(progress.stage, 
            RepoAddStage::Clone { .. } | 
            RepoAddStage::Fetch { .. } | 
            RepoAddStage::Checkout { .. }
        );
        simple.is_complete = matches!(progress.stage, RepoAddStage::Completed { .. } | RepoAddStage::Error { .. });
        simple.is_success  = matches!(progress.stage, RepoAddStage::Completed { .. });
        
        // Update last progress time for watchdog monitoring
        simple.last_progress_time = progress.timestamp.or_else(|| Some(std::time::Instant::now()));

        if simple.output_lines.len() > 100 { // Limit log lines
            simple.output_lines.remove(0);
        }
        simple.output_lines.push(displayable.message.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use sagitta_search::sync_progress::{SyncStage, SyncProgressReporter};
    use crate::gui::repository::shared_sync_state::{SIMPLE_STATUS, DETAILED_STATUS};

    #[tokio::test]
    async fn test_gui_progress_reporter_updates_global_state() {
        let repo_id = format!("test_repo_global_state_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let reporter = GuiProgressReporter::new(repo_id.clone());

        let core_progress = SyncProgress::new(SyncStage::Idle);

        SyncProgressReporter::report(&reporter, core_progress.clone()).await;

        // Simple checks that don't depend on complex state
        assert!(SIMPLE_STATUS.contains_key(&repo_id), "Simple status should be in the map");
        assert!(DETAILED_STATUS.contains_key(&repo_id), "Detailed status should be in the map");

        // Check basic state
        if let Some(simple_status) = SIMPLE_STATUS.get(&repo_id) {
            assert!(!simple_status.is_running, "Idle stage should not be considered running");
            assert!(!simple_status.is_complete, "Idle stage should not be considered complete");
        }

        // Cleanup immediately
        SIMPLE_STATUS.remove(&repo_id);
        DETAILED_STATUS.remove(&repo_id);
    }
} 