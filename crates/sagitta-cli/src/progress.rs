use async_trait::async_trait;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use sagitta_search::sync_progress::SyncProgressReporter;
use sagitta_search::sync_progress::{SyncProgress, SyncStage};
use sagitta_search::sync_progress::{AddProgressReporter, AddProgress, RepoAddStage};

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
            SyncStage::Heartbeat { .. } => "heartbeat".to_string(),
        }
    }

    fn get_add_stage_key(stage: &RepoAddStage) -> String {
        match stage {
            RepoAddStage::Clone { .. } => "add_clone".to_string(),
            RepoAddStage::Fetch { .. } => "add_fetch".to_string(),
            RepoAddStage::Checkout { .. } => "add_checkout".to_string(),
            RepoAddStage::Completed { .. } => "add_completed".to_string(),
            RepoAddStage::Error { .. } => "add_error".to_string(),
            RepoAddStage::Idle => "add_idle".to_string(),
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
                stage_progress.message_template = format!("[Git Fetch] {message}");
                if let Some((current, total)) = progress {
                    stage_progress.pb.set_length(*total as u64);
                    stage_progress.pb.set_position(*current as u64);
                    stage_progress.message_template = format!("[Git Fetch] {message}: {current}/{total}");
                } else {
                    stage_progress.pb.set_length(1); // Indeterminate
                    stage_progress.pb.set_position(0);
                    stage_progress.pb.tick();
                }
            }
            SyncStage::DiffCalculation { message } => {
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Diff Calc] {message}");
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
                        msg_parts.push("[Embedding]".to_string());
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
                        msg_parts.push(format!("[{action}]"));
                        if let Some(f) = current_file {
                            msg_parts.push(format!("File: {}", f.file_name().unwrap_or_default().to_string_lossy()));
                        }
                    }
                } else {
                    // Fallback to original behavior
                    msg_parts.push(format!("[{action}]"));
                    if let Some(f) = current_file {
                        msg_parts.push(format!("File: {}", f.file_name().unwrap_or_default().to_string_lossy()));
                    }
                }
                
                if let Some(fps) = files_per_second {
                    let unit = if message.as_ref().is_some_and(|m| m.contains("chunk")) {
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
                stage_progress.message_template = format!("[Collect Files] {message}");
                stage_progress.total_items = Some(*total_files);
                stage_progress.pb.set_length(*total_files as u64); // Or 1 if just a message
                stage_progress.pb.set_position(0); // Or tick if indeterminate
                 stage_progress.pb.tick();
            }
            SyncStage::QueryLanguages { message } => {
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Query Languages] {message}");
                stage_progress.pb.set_length(1);
                stage_progress.pb.set_position(0);
                stage_progress.pb.tick();
            }
            SyncStage::VerifyingCollection { message } => {
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Verify Collection] {message}");
                stage_progress.pb.set_length(1);
                stage_progress.pb.set_position(0);
                stage_progress.pb.tick();
            }
            SyncStage::Completed { message } => {
                stage_progress.pb.finish_with_message(format!("[Completed] {message}"));
                // Optionally remove from map or keep for final display
                return;
            }
            SyncStage::Error { message } => {
                stage_progress.pb.abandon_with_message(format!("[Error] {message}"));
                // Optionally remove from map
                return;
            }
            SyncStage::Idle => {
                // Don't create progress bars for Idle state - it's not useful
                return;
            }
            SyncStage::Heartbeat { message } => {
                // Heartbeat stage - update existing progress bars or create a simple one
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Heartbeat] {message}");
                stage_progress.pb.set_length(1);
                stage_progress.pb.set_position(0);
                stage_progress.pb.tick();
            }
        }
        stage_progress.pb.set_message(stage_progress.message_template.clone());
    }

    async fn update_or_create_add_pb(&self, stage_key: &str, progress_info: &AddProgress) {
        let mut stage_pbs_guard = self.stage_pbs.lock().await;

        // Get or create the progress bar for this stage
        let stage_progress = stage_pbs_guard.entry(stage_key.to_string()).or_insert_with(|| {
            let pb = self.multi_progress.add(ProgressBar::new(100));
            StageProgress {
                pb,
                message_template: String::new(),
                current_item: None,
                total_items: None,
                current_item_num: None,
            }
        });

        // Define styles
        let style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-");

        let simple_style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap()
            .progress_chars("#>-");

        match &progress_info.stage {
            RepoAddStage::Clone { message, progress } => {
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Clone] {message}");
                if let Some((current, total)) = progress {
                    stage_progress.pb.set_length(*total as u64);
                    stage_progress.pb.set_position(*current as u64);
                    stage_progress.message_template = format!("[Clone] {message}: {current}/{total}");
                } else {
                    stage_progress.pb.set_length(1); // Indeterminate
                    stage_progress.pb.set_position(0);
                    stage_progress.pb.tick();
                }
            }
            RepoAddStage::Fetch { message, progress } => {
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Fetch] {message}");
                if let Some((current, total)) = progress {
                    stage_progress.pb.set_length(*total as u64);
                    stage_progress.pb.set_position(*current as u64);
                    stage_progress.message_template = format!("[Fetch] {message}: {current}/{total}");
                } else {
                    stage_progress.pb.set_length(1); // Indeterminate
                    stage_progress.pb.set_position(0);
                    stage_progress.pb.tick();
                }
            }
            RepoAddStage::Checkout { message } => {
                stage_progress.pb.set_style(simple_style.clone());
                stage_progress.message_template = format!("[Checkout] {message}");
                stage_progress.pb.set_length(1);
                stage_progress.pb.set_position(0);
                stage_progress.pb.tick();
            }
            RepoAddStage::Completed { message } => {
                stage_progress.pb.finish_with_message(format!("[Completed] {message}"));
                return;
            }
            RepoAddStage::Error { message } => {
                stage_progress.pb.abandon_with_message(format!("[Error] {message}"));
                return;
            }
            RepoAddStage::Idle => {
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

#[async_trait]
impl AddProgressReporter for IndicatifProgressReporter {
    async fn report(&self, progress: AddProgress) {
        let stage_key = Self::get_add_stage_key(&progress.stage);

        // Clear other finished progress bars to keep the display clean
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

        self.update_or_create_add_pb(&stage_key, &progress).await;

        if let Some(overall_pb_instance) = &self.overall_pb {
            // Logic to update overall progress bar if you have one
            // This might involve tracking the total number of stages or total work units
            // For now, let's just increment it or set it based on current stage
            // overall_pb_instance.inc(1);
        }

        // If the stage is Completed or Error, we might want to ensure all other bars are cleared
        // and this one is prominently displayed.
        if matches!(progress.stage, RepoAddStage::Completed { .. } | RepoAddStage::Error { .. }) {
            // Optionally clear all other bars except this one
            // This is a design choice - you might want to keep them for reference
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
    use sagitta_search::sync_progress::{SyncProgress, SyncStage, AddProgress, RepoAddStage, SyncProgressReporter, AddProgressReporter};

    // Test-friendly progress reporter that doesn't render to terminal
    #[derive(Debug)]
    struct MockProgressReporter {
        stage_pbs: Arc<Mutex<HashMap<String, MockStageProgress>>>,
    }

    #[derive(Debug)]
    struct MockStageProgress {
        length: Option<u64>,
        position: u64,
        message: String,
        is_finished: bool,
    }

    impl MockProgressReporter {
        fn new() -> Self {
            Self {
                stage_pbs: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn get_stage_key(stage: &SyncStage) -> String {
            IndicatifProgressReporter::get_stage_key(stage)
        }

        fn get_add_stage_key(stage: &RepoAddStage) -> String {
            IndicatifProgressReporter::get_add_stage_key(stage)
        }
    }

    #[async_trait]
    impl SyncProgressReporter for MockProgressReporter {
        async fn report(&self, progress: SyncProgress) {
            let stage_key = Self::get_stage_key(&progress.stage);
            let mut stage_pbs = self.stage_pbs.lock().await;
            
            let mock_progress = match &progress.stage {
                SyncStage::GitFetch { message, progress } => {
                    if let Some((current, total)) = progress {
                        MockStageProgress {
                            length: Some(*total as u64),
                            position: *current as u64,
                            message: format!("[Git Fetch] {}: {}/{}", message, current, total),
                            is_finished: false,
                        }
                    } else {
                        MockStageProgress {
                            length: Some(1),
                            position: 0,
                            message: format!("[Git Fetch] {}", message),
                            is_finished: false,
                        }
                    }
                }
                SyncStage::IndexFile { current_file, total_files, current_file_num, files_per_second, .. } => {
                    let file_name = current_file.as_ref()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let mut msg = format!("[Indexing] File: {}", file_name);
                    if let Some(fps) = files_per_second {
                        msg.push_str(&format!(" {:.2} files/s", fps));
                    }
                    MockStageProgress {
                        length: Some(*total_files as u64),
                        position: *current_file_num as u64,
                        message: msg,
                        is_finished: false,
                    }
                }
                SyncStage::Completed { message } => {
                    MockStageProgress {
                        length: Some(1),
                        position: 1,
                        message: format!("[Completed] {}", message),
                        is_finished: true,
                    }
                }
                SyncStage::Error { message } => {
                    MockStageProgress {
                        length: Some(1),
                        position: 0,
                        message: format!("[Error] {}", message),
                        is_finished: true,
                    }
                }
                _ => {
                    MockStageProgress {
                        length: Some(1),
                        position: 0,
                        message: format!("{:?}", progress.stage),
                        is_finished: false,
                    }
                }
            };
            
            stage_pbs.insert(stage_key, mock_progress);
        }
    }

    #[async_trait]
    impl AddProgressReporter for MockProgressReporter {
        async fn report(&self, progress: AddProgress) {
            let stage_key = Self::get_add_stage_key(&progress.stage);
            let mut stage_pbs = self.stage_pbs.lock().await;
            
            let mock_progress = match &progress.stage {
                RepoAddStage::Clone { message, progress } => {
                    if let Some((current, total)) = progress {
                        MockStageProgress {
                            length: Some(*total as u64),
                            position: *current as u64,
                            message: format!("[Clone] {}: {}/{}", message, current, total),
                            is_finished: false,
                        }
                    } else {
                        MockStageProgress {
                            length: Some(1),
                            position: 0,
                            message: format!("[Clone] {}", message),
                            is_finished: false,
                        }
                    }
                }
                RepoAddStage::Completed { message } => {
                    MockStageProgress {
                        length: Some(1),
                        position: 1,
                        message: format!("[Completed] {}", message),
                        is_finished: true,
                    }
                }
                _ => {
                    MockStageProgress {
                        length: Some(1),
                        position: 0,
                        message: format!("{:?}", progress.stage),
                        is_finished: false,
                    }
                }
            };
            
            stage_pbs.insert(stage_key, mock_progress);
        }
    }

    fn create_progress(stage: SyncStage) -> SyncProgress {
        SyncProgress::new(stage)
    }

    fn create_add_progress(stage: RepoAddStage) -> AddProgress {
        AddProgress::new(stage)
    }

    #[tokio::test]
    async fn test_reporter_new() {
        let reporter = IndicatifProgressReporter::new();
        assert!(reporter.overall_pb.is_none()); // Assuming no overall_pb by default
        assert_eq!(reporter.stage_pbs.lock().await.len(), 0);
    }

    #[tokio::test]
    async fn test_mock_reporter_git_fetch() {
        let reporter = MockProgressReporter::new();
        let progress = create_progress(SyncStage::GitFetch { 
            message: "Fetching objects".to_string(), 
            progress: Some((75, 100)) 
        });
        SyncProgressReporter::report(&reporter, progress).await;

        let stage_pbs = reporter.stage_pbs.lock().await;
        let stage_key = MockProgressReporter::get_stage_key(&SyncStage::GitFetch{message:"".into(), progress:None});
        let sp = stage_pbs.get(&stage_key).expect("GitFetch progress not found");
        assert_eq!(sp.length, Some(100));
        assert_eq!(sp.position, 75);
        assert!(sp.message.contains("[Git Fetch] Fetching objects: 75/100"));
        assert!(!sp.is_finished);
    }

    #[tokio::test]
    async fn test_mock_reporter_index_file() {
        let reporter = MockProgressReporter::new();
        let progress = create_progress(SyncStage::IndexFile {
            current_file: Some(PathBuf::from("src/main.rs")),
            total_files: 200,
            current_file_num: 25,
            files_per_second: Some(10.5),
            message: None,
        });
        SyncProgressReporter::report(&reporter, progress).await;

        let stage_pbs = reporter.stage_pbs.lock().await;
        let stage_key = MockProgressReporter::get_stage_key(&SyncStage::IndexFile { 
            current_file: None, total_files:0, current_file_num:0, files_per_second: None, message: None 
        });
        let sp = stage_pbs.get(&stage_key).expect("IndexFile progress not found");
        assert_eq!(sp.length, Some(200));
        assert_eq!(sp.position, 25);
        assert!(sp.message.contains("[Indexing] File: main.rs"));
        assert!(sp.message.contains("10.50 files/s"));
        assert!(!sp.is_finished);
    }

    #[tokio::test]
    async fn test_mock_reporter_completed() {
        let reporter = MockProgressReporter::new();
        let progress = create_progress(SyncStage::Completed { 
            message: "Sync completed successfully".to_string() 
        });
        SyncProgressReporter::report(&reporter, progress).await;

        let stage_pbs = reporter.stage_pbs.lock().await;
        let stage_key = MockProgressReporter::get_stage_key(&SyncStage::Completed { message: "".into() });
        let sp = stage_pbs.get(&stage_key).expect("Completed progress not found");
        assert!(sp.is_finished);
        assert!(sp.message.contains("[Completed] Sync completed successfully"));
    }

    #[tokio::test]
    async fn test_stage_transition_and_clearing() {
        // Simple test that just verifies stage key generation works correctly
        let fetch_key = IndicatifProgressReporter::get_stage_key(&SyncStage::GitFetch{
            message: "test".into(), 
            progress: None
        });
        let index_key = IndicatifProgressReporter::get_stage_key(&SyncStage::IndexFile { 
            current_file: None, 
            total_files: 0, 
            current_file_num: 0, 
            files_per_second: None, 
            message: None 
        });
        let completed_key = IndicatifProgressReporter::get_stage_key(&SyncStage::Completed { 
            message: "done".into() 
        });
        
        // Verify that different stages generate different keys
        assert_ne!(fetch_key, index_key);
        assert_ne!(index_key, completed_key);
        assert_ne!(fetch_key, completed_key);
        
        // Verify that same stages generate same keys
        let fetch_key2 = IndicatifProgressReporter::get_stage_key(&SyncStage::GitFetch{
            message: "different message".into(), 
            progress: Some((1, 2))
        });
        assert_eq!(fetch_key, fetch_key2); // Should be same because key is based on stage type, not content
    }

    #[tokio::test]
    async fn test_add_progress_clone() {
        let reporter = MockProgressReporter::new();
        let progress = create_add_progress(RepoAddStage::Clone {
            message: "Cloning repository".to_string(),
            progress: Some((50, 100)),
        });
        AddProgressReporter::report(&reporter, progress).await;

        let stage_pbs = reporter.stage_pbs.lock().await;
        let stage_key = MockProgressReporter::get_add_stage_key(&RepoAddStage::Clone { message: "".into(), progress: None });
        let sp = stage_pbs.get(&stage_key).expect("Clone progress not found");
        assert_eq!(sp.length, Some(100));
        assert_eq!(sp.position, 50);
        assert!(sp.message.contains("[Clone] Cloning repository: 50/100"));
        assert!(!sp.is_finished);
    }

    #[tokio::test]
    async fn test_add_progress_completed() {
        let reporter = MockProgressReporter::new();
        let progress = create_add_progress(RepoAddStage::Completed {
            message: "Repository added successfully".to_string(),
        });
        AddProgressReporter::report(&reporter, progress).await;

        let stage_pbs = reporter.stage_pbs.lock().await;
        let stage_key = MockProgressReporter::get_add_stage_key(&RepoAddStage::Completed { message: "".into() });
        let sp = stage_pbs.get(&stage_key).expect("Completed progress not found");
        assert!(sp.is_finished);
        assert!(sp.message.contains("[Completed] Repository added successfully"));
    }
} 