use async_trait::async_trait;
use tokio::sync::mpsc;
use sagitta_search::sync_progress::{SyncProgress, SyncProgressReporter as CoreSyncProgressReporter};

// Using String for RepositoryId as per observations in manager.rs
pub type RepositoryId = String;

/// Message to send to the GUI thread containing progress information.
#[derive(Debug, Clone)]
pub struct GuiSyncReport {
    pub repo_id: RepositoryId,
    pub progress: SyncProgress, // This is sagitta_search::sync_progress::SyncProgress
}

/// Implements the SyncProgressReporter trait to send progress updates to a central handler.
pub struct GuiProgressReporter {
    progress_sender: mpsc::UnboundedSender<GuiSyncReport>,
    repo_id: RepositoryId,
}

impl GuiProgressReporter {
    pub fn new(progress_sender: mpsc::UnboundedSender<GuiSyncReport>, repo_id: RepositoryId) -> Self {
        Self { progress_sender, repo_id }
    }
}

#[async_trait]
impl CoreSyncProgressReporter for GuiProgressReporter {
    async fn report(&self, progress: SyncProgress) {
        let report = GuiSyncReport {
            repo_id: self.repo_id.clone(),
            progress,
        };
        if let Err(e) = self.progress_sender.send(report) {
            // TODO: Use sagitta-code's logging mechanism (e.g., log::error!)
            eprintln!("[GuiProgressReporter] Failed to send sync progress to GUI for repo '{}': {}", self.repo_id, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use sagitta_search::sync_progress::SyncStage;

    #[tokio::test]
    async fn test_gui_progress_reporter_sends_report() {
        let (tx, mut rx) = mpsc::unbounded_channel::<GuiSyncReport>();
        let repo_id = "test_repo".to_string();
        let reporter = GuiProgressReporter::new(tx, repo_id.clone());

        let core_progress = SyncProgress {
            stage: SyncStage::Idle,
        };

        reporter.report(core_progress.clone()).await;

        let received_report = rx.recv().await.expect("Should receive a report");

        assert_eq!(received_report.repo_id, repo_id);
        // Minimal check, as SyncProgress comparison can be tricky due to non-PartialEq fields in some stages.
        // We primarily care that *a* report for the correct repo_id was sent.
        // A more detailed check would involve asserting specific fields of received_report.progress
        // if SyncProgress derived PartialEq or by matching stage variants.
        match received_report.progress.stage {
            SyncStage::Idle => { /* Correct stage */ },
            _ => panic!("Incorrect stage received"),
        }
    }

    #[tokio::test]
    async fn test_gui_progress_reporter_handles_closed_channel() {
        let (tx, mut rx) = mpsc::unbounded_channel::<GuiSyncReport>();
        let repo_id = "test_repo_closed_channel".to_string();
        
        // Drop the receiver to simulate a closed channel
        drop(rx);
        
        let reporter = GuiProgressReporter::new(tx, repo_id.clone());

        let core_progress = SyncProgress {
            stage: SyncStage::GitFetch { message: "Fetching...".to_string(), progress: Some((10, 100)) },
        };

        // This should not panic, error will be printed to eprintln by the reporter
        reporter.report(core_progress).await;
        // Test passes if it doesn't panic and the error is logged (manually verifiable for now or by capturing stderr)
    }
} 