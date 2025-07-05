use async_trait::async_trait;
use sagitta_search::sync_progress::{SyncProgress, SyncProgressReporter, SyncStage};
use sagitta_search::sync_progress::{AddProgress, AddProgressReporter, RepoAddStage};
use log;
use std::sync::Mutex;

static PENDING_MESSAGES: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn take_pending_messages() -> Option<Vec<String>> {
    let mut messages = PENDING_MESSAGES.lock().unwrap();
    if messages.is_empty() {
        None
    } else {
        Some(std::mem::take(&mut *messages))
    }
}

pub struct LoggingProgressReporter;

#[async_trait]
impl SyncProgressReporter for LoggingProgressReporter {
    async fn report(&self, progress: SyncProgress) {
        match progress.stage {
            SyncStage::GitFetch { message, progress: Some((received, total)) } => {
                log::info!("[GitFetch] {message}: {received}/{total}");
            }
            SyncStage::GitFetch { message, progress: None } => {
                log::info!("[GitFetch] {message}");
            }
            SyncStage::DiffCalculation { message } => {
                log::info!("[DiffCalculation] {message}");
            }
            SyncStage::IndexFile { current_file, total_files, current_file_num, files_per_second, .. } => {
                let file_name = current_file.map_or_else(|| "N/A".to_string(), |p| p.to_string_lossy().to_string());
                if let Some(fps) = files_per_second {
                    log::info!("[IndexFile] Processing file {current_file_num}/{total_files} ({fps:.2} files/s): {file_name}");
                } else {
                    log::info!("[IndexFile] Processing file {current_file_num}/{total_files}: {file_name}");
                }
            }
            SyncStage::DeleteFile { current_file, total_files, current_file_num, files_per_second, .. } => {
                let file_name = current_file.map_or_else(|| "N/A".to_string(), |p| p.to_string_lossy().to_string());
                if let Some(fps) = files_per_second {
                    log::info!("[DeleteFile] Deleting file {current_file_num}/{total_files} ({fps:.2} files/s): {file_name}");
                } else {
                    log::info!("[DeleteFile] Deleting file {current_file_num}/{total_files}: {file_name}");
                }
            }
            SyncStage::CollectFiles { total_files, message } => {
                log::info!("[CollectFiles] {message}: {total_files} files");
            }
            SyncStage::QueryLanguages { message } => {
                log::info!("[QueryLanguages] {message}");
            }
            SyncStage::VerifyingCollection { message } => {
                log::info!("[VerifyingCollection] {message}");
            }
            SyncStage::Completed { message } => {
                log::info!("[Completed] {message}");
            }
            SyncStage::Error { message } => {
                log::error!("[Error] {message}");
            }
            SyncStage::Idle => {
                log::debug!("[Idle]");
            }
            SyncStage::Heartbeat { message } => {
                log::debug!("[Heartbeat] {message}");
            }
        }
    }
}

#[async_trait]
impl AddProgressReporter for LoggingProgressReporter {
    async fn report(&self, progress: AddProgress) {
        match progress.stage {
            RepoAddStage::Clone { message, progress: Some((received, total)) } => {
                log::info!("[Clone] {message}: {received}/{total}");
            }
            RepoAddStage::Clone { message, progress: None } => {
                log::info!("[Clone] {message}");
            }
            RepoAddStage::Fetch { message, progress: Some((received, total)) } => {
                log::info!("[Fetch] {message}: {received}/{total}");
            }
            RepoAddStage::Fetch { message, progress: None } => {
                log::info!("[Fetch] {message}");
            }
            RepoAddStage::Checkout { message } => {
                log::info!("[Checkout] {message}");
            }
            RepoAddStage::Completed { message } => {
                log::info!("[Completed] {message}");
            }
            RepoAddStage::Error { message } => {
                log::error!("[Error] {message}");
            }
            RepoAddStage::Idle => {
                log::debug!("[Idle]");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sagitta_search::sync_progress::{SyncProgress, SyncStage, AddProgress, RepoAddStage, SyncProgressReporter, AddProgressReporter};
    use std::path::PathBuf;
    use env_logger;

    // Helper to setup logging for tests
    fn setup_test_logger() {
        // Use try_init to avoid panic if logger is already set (e.g. by another test)
        // Not capturing logs to a buffer for now, relying on manual inspection with --nocapture
        let _ = env_logger::builder().is_test(true).filter_level(log::LevelFilter::Debug).try_init();
    }

    #[tokio::test]
    async fn test_report_git_fetch_with_progress() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::GitFetch {
            message: "Fetching objects".to_string(),
            progress: Some((50, 100)),
        });
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [GitFetch] Fetching objects: 50/100
    }

    #[tokio::test]
    async fn test_report_git_fetch_no_progress() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::GitFetch {
            message: "Receiving objects".to_string(),
            progress: None,
        });
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [GitFetch] Receiving objects
    }

    #[tokio::test]
    async fn test_report_index_file_with_fps() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::IndexFile {
            current_file: Some(PathBuf::from("src/module/file.rs")),
            total_files: 200,
            current_file_num: 25,
            files_per_second: Some(15.756),
            message: None,
        });
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [IndexFile] Processing file 25/200 (15.76 files/s): src/module/file.rs
    }

    #[tokio::test]
    async fn test_report_index_file_no_fps() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::IndexFile {
            current_file: Some(PathBuf::from("README.md")),
            total_files: 5,
            current_file_num: 1,
            files_per_second: None,
            message: None,
        });
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [IndexFile] Processing file 1/5: README.md
    }

    #[tokio::test]
    async fn test_report_delete_file_with_fps() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::DeleteFile {
            current_file: Some(PathBuf::from("old_file.txt")),
            total_files: 10,
            current_file_num: 2,
            files_per_second: Some(5.123),
            message: None,
        });
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [DeleteFile] Deleting file 2/10 (5.12 files/s): old_file.txt
    }

    #[tokio::test]
    async fn test_report_error() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::Error { message: "A critical error happened".to_string() });
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: ERROR [sagitta_mcp::progress] [Error] A critical error happened
    }

    #[tokio::test]
    async fn test_report_completed() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::Completed { message: "All operations finished.".to_string() });
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [Completed] All operations finished.
    }

    #[tokio::test]
    async fn test_report_idle() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::Idle);
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: DEBUG [sagitta_mcp::progress] [Idle]
    }

     #[tokio::test]
    async fn test_report_diff_calculation() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::DiffCalculation { message: "Calculating differences...".to_string() });
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [DiffCalculation] Calculating differences...
    }

    #[tokio::test]
    async fn test_report_collect_files() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::CollectFiles { total_files: 150, message: "Gathering files for indexing".to_string() });
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [CollectFiles] Gathering files for indexing: 150 files
    }

    #[tokio::test]
    async fn test_report_query_languages() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::QueryLanguages { message: "Identifying languages in repository".to_string() });
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [QueryLanguages] Identifying languages in repository
    }

    #[tokio::test]
    async fn test_report_verifying_collection() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = SyncProgress::new(SyncStage::VerifyingCollection { message: "Ensuring collection exists and is ready".to_string() });
        SyncProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [VerifyingCollection] Ensuring collection exists and is ready
    }

    // Add progress reporter tests
    #[tokio::test]
    async fn test_report_add_clone() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = AddProgress::new(RepoAddStage::Clone {
            message: "Cloning repository".to_string(),
            progress: Some((30, 100)),
        });
        AddProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [Clone] Cloning repository: 30/100
    }

    #[tokio::test]
    async fn test_report_add_completed() {
        setup_test_logger();
        let reporter = LoggingProgressReporter;
        let progress = AddProgress::new(RepoAddStage::Completed {
            message: "Repository added successfully".to_string(),
        });
        AddProgressReporter::report(&reporter, progress).await;
        // Expected log: INFO [sagitta_mcp::progress] [Completed] Repository added successfully
    }
}