use async_trait::async_trait;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use sagitta_search::sync_progress::SyncProgressReporter;
use sagitta_search::sync_progress::{SyncProgress, SyncStage};

#[derive(Debug)]
struct StageProgress {
    pb: ProgressBar,
    message_template: String,
    // For stages like IndexFile, DeleteFile where we might want to show "File X of Y"
    current_item: Option<PathBuf>,
    total_items: Option<usize>,
    current_item_num: Option<usize>,
}

#[derive(Debug)]
pub struct IndicatifProgressReporter {
    multi_progress: Arc<MultiProgress>,
    stage_pbs: Arc<Mutex<HashMap<String, StageProgress>>>, // Keyed by a unique stage identifier
    overall_pb: Option<ProgressBar>,
}

impl IndicatifProgressReporter {
    pub fn new() -> Self {
        let multi_progress = Arc::new(MultiProgress::new());
        // Optional: Add an overall progress bar if desired
        // let overall_pb = multi_progress.add(ProgressBar::new(100));
        // overall_pb.set_style(ProgressStyle::default_bar()
        //     .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} ({eta}) {msg}")
        //     .unwrap()
        //     .progress_chars("#>-"));
        // overall_pb.set_message("Overall progress...");

        Self {
            multi_progress,
            stage_pbs: Arc::new(Mutex::new(HashMap::new())),
            overall_pb: None, // Some(overall_pb)
        }
    }

    fn get_stage_key(stage: &SyncStage) -> String {
        match stage {
            SyncStage::GitFetch { .. } => "git_fetch".to_string(),
            SyncStage::DiffCalculation { .. } => "diff_calc".to_string(),
            SyncStage::IndexFile { .. } => "index_file".to_string(),
            SyncStage::DeleteFile { .. } => "delete_file".to_string(),
            SyncStage::CollectFiles { .. } => "collect_files".to_string(),
            SyncStage::QueryLanguages { .. } => "query_langs".to_string(),
            SyncStage::VerifyingCollection { .. } => "verify_collection".to_string(),
            SyncStage::Completed { .. } => "completed".to_string(),
            SyncStage::Error { .. } => "error".to_string(),
            SyncStage::Idle => "idle".to_string(),
        }
    }

    async fn update_or_create_pb(&self, stage_key: &str, progress_info: &SyncProgress) {
        let mut stage_pbs_guard = self.stage_pbs.lock().await;

        let style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] {wide_bar:.cyan/blue} {pos:>7}/{len:7} ({per_sec}, {eta}) {msg}")
            .unwrap()
            .progress_chars("##-");
         let simple_style = ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap();

        let stage_progress = stage_pbs_guard.entry(stage_key.to_string()).or_insert_with(|| {
            let pb = self.multi_progress.add(ProgressBar::new(0)); // Length will be set later
            pb.set_style(style.clone());
            StageProgress {
                pb,
                message_template: "".to_string(), // Will be set based on stage
                current_item: None,
                total_items: None,
                current_item_num: None,
            }
        });

        match &progress_info.stage {
            SyncStage::GitFetch { message, progress } => {
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Git Fetch] {}", message);
                if let Some((current, total)) = progress {
                    stage_progress.pb.set_length(*total as u64);
                    stage_progress.pb.set_position(*current as u64);
                    stage_progress.message_template = format!("[Git Fetch] {}: {}/{}", message, current, total);
                } else {
                    stage_progress.pb.set_length(1); // Indeterminate
                    stage_progress.pb.set_position(0);
                    stage_progress.pb.tick();
                }
            }
            SyncStage::DiffCalculation { message } => {
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Diff Calc] {}", message);
                stage_progress.pb.set_length(1);
                stage_progress.pb.set_position(0);
                stage_progress.pb.tick();

            }
            SyncStage::IndexFile { current_file, total_files, current_file_num, files_per_second, message }
            | SyncStage::DeleteFile { current_file, total_files, current_file_num, files_per_second, message } => {
                stage_progress.pb.set_style(style.clone());
                let action = if matches!(progress_info.stage, SyncStage::IndexFile {..}) { "Indexing" } else { "Deleting" };
                stage_progress.total_items = Some(*total_files);
                stage_progress.current_item_num = Some(*current_file_num);
                stage_progress.current_item = current_file.clone();

                stage_progress.pb.set_length(*total_files as u64);
                stage_progress.pb.set_position(*current_file_num as u64);

                // Create clearer messaging based on whether this is file processing or chunk processing
                let mut msg_parts = vec![];
                
                if let Some(ref message) = message {
                    if message.contains("chunk") || message.contains("embedding") {
                        // This is chunk/embedding processing - ensure it uses progress bar style
                        stage_progress.pb.set_style(style.clone()); // Force progress bar style
                        msg_parts.push(format!("[Embedding]"));
                        if message.contains("Starting embedding generation") {
                            msg_parts.push("Starting...".to_string());
                        } else if message.contains("Generating embeddings") {
                            msg_parts.push(message.clone());
                        } else if message.contains("completed") {
                            msg_parts.push("Completed".to_string());
                        } else {
                            msg_parts.push(message.clone());
                        }
                    } else {
                        // This is file processing
                        msg_parts.push(format!("[{}]", action));
                        if let Some(f) = current_file {
                            msg_parts.push(format!("File: {}", f.file_name().unwrap_or_default().to_string_lossy()));
                        }
                    }
                } else {
                    // Fallback to original behavior
                    msg_parts.push(format!("[{}]", action));
                    if let Some(f) = current_file {
                        msg_parts.push(format!("File: {}", f.file_name().unwrap_or_default().to_string_lossy()));
                    }
                }
                
                if let Some(fps) = files_per_second {
                    let unit = if message.as_ref().map_or(false, |m| m.contains("chunk")) {
                        "chunks/s"
                    } else {
                        "files/s"
                    };
                    stage_progress.pb.set_message(format!("{} {:.2} {}", msg_parts.join(" "), fps, unit));
                } else {
                    stage_progress.pb.set_message(msg_parts.join(" "));
                }
                 return; // Message set directly, no need for template
            }
            SyncStage::CollectFiles { total_files, message } => {
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Collect Files] {}", message);
                stage_progress.total_items = Some(*total_files);
                stage_progress.pb.set_length(*total_files as u64); // Or 1 if just a message
                stage_progress.pb.set_position(0); // Or tick if indeterminate
                 stage_progress.pb.tick();
            }
            SyncStage::QueryLanguages { message } => {
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Query Languages] {}", message);
                stage_progress.pb.set_length(1);
                stage_progress.pb.set_position(0);
                stage_progress.pb.tick();
            }
            SyncStage::VerifyingCollection { message } => {
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Verify Collection] {}", message);
                stage_progress.pb.set_length(1);
                stage_progress.pb.set_position(0);
                stage_progress.pb.tick();
            }
            SyncStage::Completed { message } => {
                stage_progress.pb.finish_with_message(format!("[Completed] {}", message));
                // Optionally remove from map or keep for final display
                return;
            }
            SyncStage::Error { message } => {
                stage_progress.pb.abandon_with_message(format!("[Error] {}", message));
                // Optionally remove from map
                return;
            }
            SyncStage::Idle => {
                // Don't create progress bars for Idle state - it's not useful
                return;
            }
        }
        stage_progress.pb.set_message(stage_progress.message_template.clone());
    }
}

#[async_trait]
impl SyncProgressReporter for IndicatifProgressReporter {
    async fn report(&self, progress: SyncProgress) {
        let stage_key = IndicatifProgressReporter::get_stage_key(&progress.stage);

        // Clear other finished progress bars to keep the display clean
        // (except for the current stage, completed, or error)
        let mut stage_pbs_guard = self.stage_pbs.lock().await;
        let keys_to_clear: Vec<String> = stage_pbs_guard
            .iter()
            .filter_map(|(key, sp)| {
                // Only clear bars that are truly not useful anymore
                if key != &stage_key && (
                    key == "idle" || // Always clear idle bars
                    (sp.pb.is_finished() && sp.pb.message().contains("[Error]")) // Clear error bars
                ) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        for key_to_clear in keys_to_clear {
            if let Some(sp_to_clear) = stage_pbs_guard.remove(&key_to_clear) {
                sp_to_clear.pb.finish_and_clear(); // Clears the bar from the MultiProgress
            }
        }
        drop(stage_pbs_guard); // Release lock before calling another async method on self

        self.update_or_create_pb(&stage_key, &progress).await;

        if let Some(overall_pb_instance) = &self.overall_pb {
            // Logic to update overall progress bar if you have one
            // This might involve tracking the total number of stages or total work units
            // For now, let's just increment it or set it based on current stage
            // overall_pb_instance.inc(1);
        }

        // If the stage is Completed or Error, we might want to ensure all other bars are cleared
        // and this one is prominently displayed.
        if matches!(progress.stage, SyncStage::Completed { .. } | SyncStage::Error { .. }) {
            // Don't clear other progress bars on completion - they show useful information
            // about file processing speed and embedding speed that users want to see
            // Only clear bars that are truly not useful (like Idle bars)
            let mut stage_pbs_guard = self.stage_pbs.lock().await;
            let keys_to_remove: Vec<String> = stage_pbs_guard
                .keys()
                .filter(|key| {
                    // Only remove idle or error bars, keep useful progress information
                    *key == "idle" || (*key != &stage_key && key.starts_with("error"))
                })
                .cloned()
                .collect();
            
            for key in keys_to_remove {
                if let Some(sp) = stage_pbs_guard.remove(&key) {
                    sp.pb.finish_and_clear();
                }
            }
            // Don't wait for MultiProgress to join - let the bars remain visible
        }
    }
}

impl Default for IndicatifProgressReporter {
    fn default() -> Self {
        Self::new()
    }
}

// Required to be able to use Arc<dyn SyncProgressReporter>
unsafe impl Send for IndicatifProgressReporter {}
unsafe impl Sync for IndicatifProgressReporter {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;
    use sagitta_search::sync_progress::{SyncProgress, SyncStage};

    fn create_progress(stage: SyncStage) -> SyncProgress {
        SyncProgress { stage }
    }

    #[tokio::test]
    async fn test_reporter_new() {
        let reporter = IndicatifProgressReporter::new();
        assert!(reporter.overall_pb.is_none()); // Assuming no overall_pb by default
        assert_eq!(reporter.stage_pbs.lock().await.len(), 0);
        // We can't easily assert multi_progress internal state without more direct access
        // or capturing output, so we trust it's initialized.
    }

    #[tokio::test]
    async fn test_report_git_fetch_indeterminate() {
        let reporter = IndicatifProgressReporter::new();
        let progress = create_progress(SyncStage::GitFetch {
            message: "Fetching... ".to_string(),
            progress: None,
        });
        reporter.report(progress).await;
        sleep(Duration::from_millis(50)).await; // Allow progress bar to update

        let stage_pbs = reporter.stage_pbs.lock().await;
        assert_eq!(stage_pbs.len(), 1);
        let stage_key = IndicatifProgressReporter::get_stage_key(&SyncStage::GitFetch { message: "".into(), progress: None });
        let sp = stage_pbs.get(&stage_key).expect("GitFetch progress bar not found");
        assert_eq!(sp.pb.length(), Some(1)); // Indeterminate
        assert!(sp.pb.message().contains("[Git Fetch] Fetching..."));
        assert!(!sp.pb.is_finished());
    }

    #[tokio::test]
    async fn test_report_git_fetch_determinate() {
        let reporter = IndicatifProgressReporter::new();
        let progress = create_progress(SyncStage::GitFetch {
            message: "Receiving objects".to_string(),
            progress: Some((50, 100)),
        });
        reporter.report(progress).await;
        sleep(Duration::from_millis(50)).await;

        let stage_pbs = reporter.stage_pbs.lock().await;
        let stage_key = IndicatifProgressReporter::get_stage_key(&SyncStage::GitFetch { message: "".into(), progress: None });
        let sp = stage_pbs.get(&stage_key).expect("GitFetch progress bar not found");
        assert_eq!(sp.pb.length(), Some(100));
        assert_eq!(sp.pb.position(), 50);
        assert!(sp.pb.message().contains("[Git Fetch] Receiving objects: 50/100"));
        assert!(!sp.pb.is_finished());
    }

    #[tokio::test]
    async fn test_report_index_file() {
        let reporter = IndicatifProgressReporter::new();
        let progress = create_progress(SyncStage::IndexFile {
            current_file: Some(PathBuf::from("src/main.rs")),
            total_files: 200,
            current_file_num: 25,
            files_per_second: Some(10.5),
            message: None,
        });
        reporter.report(progress).await;
        sleep(Duration::from_millis(50)).await;

        let stage_pbs = reporter.stage_pbs.lock().await;
        let stage_key = IndicatifProgressReporter::get_stage_key(&SyncStage::IndexFile { current_file: None, total_files:0, current_file_num:0, files_per_second: None, message: None });
        let sp = stage_pbs.get(&stage_key).expect("IndexFile progress bar not found");
        assert_eq!(sp.pb.length(), Some(200));
        assert_eq!(sp.pb.position(), 25);
        assert!(sp.pb.message().contains("[Indexing] File: main.rs"));
        assert!(sp.pb.message().contains("10.50 files/s"));
        assert!(!sp.pb.is_finished());
    }
    
    #[tokio::test]
    async fn test_report_delete_file() {
        let reporter = IndicatifProgressReporter::new();
        let progress = create_progress(SyncStage::DeleteFile {
            current_file: Some(PathBuf::from("test/old.txt")),
            total_files: 50,
            current_file_num: 5,
            files_per_second: None,
            message: None,
        });
        reporter.report(progress).await;
        sleep(Duration::from_millis(50)).await;

        let stage_pbs = reporter.stage_pbs.lock().await;
        let stage_key = IndicatifProgressReporter::get_stage_key(&SyncStage::DeleteFile { current_file: None, total_files:0, current_file_num:0, files_per_second: None, message: None });
        let sp = stage_pbs.get(&stage_key).expect("DeleteFile progress bar not found");
        assert_eq!(sp.pb.length(), Some(50));
        assert_eq!(sp.pb.position(), 5);
        assert!(sp.pb.message().contains("[Deleting] File: old.txt"));
        assert!(!sp.pb.is_finished());
    }

    #[tokio::test]
    async fn test_report_completed() {
        let reporter = IndicatifProgressReporter::new();
        let progress = create_progress(SyncStage::Completed { message: "Sync successful".to_string() });
        reporter.report(progress).await;
        sleep(Duration::from_millis(50)).await;

        let stage_pbs = reporter.stage_pbs.lock().await;
        let stage_key = IndicatifProgressReporter::get_stage_key(&SyncStage::Completed { message: "".into() });
        let sp = stage_pbs.get(&stage_key).expect("Completed progress bar not found");
        assert!(sp.pb.is_finished());
        assert!(sp.pb.message().contains("[Completed] Sync successful"));
    }

    #[tokio::test]
    async fn test_report_error() {
        let reporter = IndicatifProgressReporter::new();
        let progress = create_progress(SyncStage::Error { message: "Sync failed".to_string() });
        reporter.report(progress).await;
        sleep(Duration::from_millis(50)).await; // Allow progress bar to update and potentially be abandoned

        let stage_pbs = reporter.stage_pbs.lock().await;
        let stage_key = IndicatifProgressReporter::get_stage_key(&SyncStage::Error { message: "".into() });
        let sp = stage_pbs.get(&stage_key).expect("Error progress bar not found");
        // is_finished is true for abandoned bars as well. Message is key.
        assert!(sp.pb.is_finished()); 
        assert!(sp.pb.message().contains("[Error] Sync failed"));
    }

    #[tokio::test]
    async fn test_stage_transition_and_clearing() {
        let reporter = IndicatifProgressReporter::new();

        // Stage 1: GitFetch
        let fetch_progress = create_progress(SyncStage::GitFetch { message: "Fetching".to_string(), progress: Some((10,10)) });
        reporter.report(fetch_progress).await;
        sleep(Duration::from_millis(50)).await;
        let fetch_key = IndicatifProgressReporter::get_stage_key(&SyncStage::GitFetch{message:"S".into(), progress:None});
        {
            let stage_pbs = reporter.stage_pbs.lock().await;
            assert_eq!(stage_pbs.len(), 1);
            assert!(stage_pbs.contains_key(&fetch_key));
            let fetch_pb = &stage_pbs.get(&fetch_key).unwrap().pb;
            assert_eq!(fetch_pb.position(), 10);
            assert_eq!(fetch_pb.length(), Some(10));
            // Manually finish it to simulate core logic completing a step before sending next stage.
            // In real use, the `Completed` stage for fetch might not exist, it just moves to next.
            // Here we rely on the reporter's own clearing logic when a *new* stage arrives.
            // For testing the clearing logic, we need a bar that *would* be cleared.
            // The current logic clears bars that are `is_finished()` OR have `[Completed]` or `[Error]` in message.
            // So, let's simulate a Completed message for GitFetch first, then move to Indexing.
        }

        // Stage 1.5: Simulate GitFetch finishing by sending a Completed stage for it (or similar)
        // This is a bit artificial as core might not send a specific "GitFetchCompleted" SyncStage.
        // Instead, let's send a GitFetch that IS finished and then a new stage.
        let fetch_done_progress = create_progress(SyncStage::GitFetch { message: "Fetch complete".to_string(), progress: Some((10,10)) });
        // To make it look "finished" to the cleanup logic, we'd need to call .finish() on it or have a Completed stage
        // Let's manually mark the existing one as finished via a `Completed` stage for test purposes
        let complete_fetch_stage = create_progress(SyncStage::Completed { message: "Fetch part done".to_string() });
        // We need to report *this* to the *existing* git_fetch progress bar for it to be marked finished
        // This is tricky because `report` creates/updates based on `stage_key` from the *new* progress event.
        // The cleanup logic is based on iterating existing bars.
        // So, let's make the fetch bar appear finished by setting its message.
        {
            let mut stage_pbs = reporter.stage_pbs.lock().await;
            if let Some(sp) = stage_pbs.get_mut(&fetch_key) {
                sp.pb.finish_with_message("[Completed] Fetch part done");
            }
        }
        sleep(Duration::from_millis(50)).await; 

        // Stage 2: IndexFile - this should clear the "finished" GitFetch bar
        let index_progress = create_progress(SyncStage::IndexFile { current_file: None, total_files: 100, current_file_num: 1, files_per_second: None, message: None });
        reporter.report(index_progress).await;
        sleep(Duration::from_millis(100)).await; // More sleep for multi_progress drawing and clearing

        let index_key = IndicatifProgressReporter::get_stage_key(&SyncStage::IndexFile{current_file:None, total_files:0, current_file_num:0, files_per_second:None, message: None});
        {
            let stage_pbs = reporter.stage_pbs.lock().await;
            // With our new logic, we preserve useful progress bars, so both might be present
            // The key thing is that the new IndexFile bar exists
            assert!(stage_pbs.len() >= 1, "Expected at least one active progress bar (IndexFile)");
            assert!(stage_pbs.contains_key(&index_key), "IndexFile progress bar should exist");
            // Note: We no longer automatically clear the fetch bar since it might contain useful info
        }

        // Stage 3: Completed (overall)
        let final_complete_progress = create_progress(SyncStage::Completed { message: "All done".to_string() });
        reporter.report(final_complete_progress).await;
        sleep(Duration::from_millis(50)).await;

        let completed_key = IndicatifProgressReporter::get_stage_key(&SyncStage::Completed{message:"".into()});
        {
            let stage_pbs = reporter.stage_pbs.lock().await;
            // With our new logic, we preserve useful progress information, so multiple bars may remain
            assert!(stage_pbs.len() >= 1, "Expected at least the final Completed bar");
            assert!(stage_pbs.contains_key(&completed_key), "Completed progress bar should exist");
            let sp = stage_pbs.get(&completed_key).unwrap();
            assert!(sp.pb.is_finished());
        }
    }
} 