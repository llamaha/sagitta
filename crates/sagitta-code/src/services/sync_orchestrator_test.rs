use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use tempfile::TempDir;
use tokio::sync::{Mutex, mpsc};
use tokio::time::timeout;

use crate::config::types::AutoSyncConfig;
use crate::gui::repository::manager::RepositoryManager;
use crate::gui::app::events::{AppEvent, SyncNotificationType};
use super::SyncOrchestrator;
use super::file_watcher::{FileWatcherService, FileWatcherConfig, FileChangeEvent};

/// Test helper to create a mock repository
async fn create_test_repository(temp_dir: &TempDir) -> Result<PathBuf> {
    let repo_path = temp_dir.path().join("test_repo");
    tokio::fs::create_dir_all(&repo_path).await?;
    
    // Initialize git repository
    let repo = git2::Repository::init(&repo_path)?;
    
    // Create initial commit
    let sig = git2::Signature::now("Test User", "test@example.com")?;
    let tree_id = {
        let mut index = repo.index()?;
        index.write_tree()?
    };
    let tree = repo.find_tree(tree_id)?;
    
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "Initial commit",
        &tree,
        &[],
    )?;
    
    Ok(repo_path)
}

/// Test helper to create a repository manager
fn create_test_repository_manager() -> Arc<Mutex<RepositoryManager>> {
    let core_config = Arc::new(Mutex::new(sagitta_search::AppConfig::default()));
    Arc::new(Mutex::new(RepositoryManager::new(core_config)))
}

#[tokio::test]
async fn test_sync_orchestrator_initialization() -> Result<()> {
    let config = AutoSyncConfig::default();
    let repo_manager = create_test_repository_manager();
    
    let mut sync_orchestrator = SyncOrchestrator::new(config, repo_manager);
    
    // Test that sync orchestrator can be started
    let (app_event_tx, mut app_event_rx) = mpsc::unbounded_channel();
    sync_orchestrator.set_app_event_sender(app_event_tx);
    
    let _result_rx = sync_orchestrator.start().await?;
    
    // Verify that no initial events are sent
    match timeout(Duration::from_millis(100), app_event_rx.recv()).await {
        Ok(Some(_)) => panic!("Unexpected event received during initialization"),
        Ok(None) => panic!("Channel closed unexpectedly"),
        Err(_) => (), // Expected timeout
    }
    
    Ok(())
}

#[tokio::test]
async fn test_sync_orchestrator_repository_addition() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo_path = create_test_repository(&temp_dir).await?;
    
    let config = AutoSyncConfig::default();
    let repo_manager = create_test_repository_manager();
    
    let mut sync_orchestrator = SyncOrchestrator::new(config, repo_manager);
    
    let (app_event_tx, mut app_event_rx) = mpsc::unbounded_channel();
    sync_orchestrator.set_app_event_sender(app_event_tx);
    
    let _result_rx = sync_orchestrator.start().await?;
    
    // Add repository to sync orchestrator
    sync_orchestrator.add_repository(&repo_path).await?;
    
    // Verify sync status is initialized
    let status = sync_orchestrator.get_sync_status(&repo_path).await;
    assert!(status.is_some(), "Sync status should be initialized after adding repository");
    
    let status = status.unwrap();
    assert!(status.is_local_only, "Test repository should be detected as local-only");
    
    Ok(())
}

#[tokio::test]
async fn test_sync_orchestrator_repository_switch() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo_path = create_test_repository(&temp_dir).await?;
    
    let mut config = AutoSyncConfig::default();
    config.sync_on_repo_switch = true;
    
    let repo_manager = create_test_repository_manager();
    let mut sync_orchestrator = SyncOrchestrator::new(config, repo_manager);
    
    let (app_event_tx, mut app_event_rx) = mpsc::unbounded_channel();
    sync_orchestrator.set_app_event_sender(app_event_tx);
    
    let _result_rx = sync_orchestrator.start().await?;
    
    // Add repository first
    sync_orchestrator.add_repository(&repo_path).await?;
    
    // Switch to repository (should trigger sync)
    sync_orchestrator.switch_repository(&repo_path).await?;
    
    // Should receive a sync notification
    let event = timeout(Duration::from_secs(5), app_event_rx.recv()).await?;
    
    match event {
        Some(AppEvent::ShowSyncNotification { repository, message: _, notification_type }) => {
            assert_eq!(repository, repo_path.file_name().unwrap().to_string_lossy());
            assert!(matches!(notification_type, SyncNotificationType::Success | SyncNotificationType::Info | SyncNotificationType::Error));
        }
        _ => panic!("Expected ShowSyncNotification event, got: {:?}", event),
    }
    
    Ok(())
}

#[tokio::test]
async fn test_file_watcher_integration() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo_path = create_test_repository(&temp_dir).await?;
    
    let watcher_config = FileWatcherConfig {
        enabled: true,
        debounce_ms: 100, // Short debounce for testing
        exclude_patterns: vec![],
        max_buffer_size: 1000,
    };
    
    let mut file_watcher = FileWatcherService::new(watcher_config);
    let mut change_rx = file_watcher.start().await?;
    
    // Add repository to watch
    file_watcher.watch_repository(&repo_path).await?;
    
    // Create a test file
    let test_file = repo_path.join("test.txt");
    tokio::fs::write(&test_file, "Hello, world!").await?;
    
    // Should receive a file change event
    let change_event = timeout(Duration::from_secs(2), change_rx.recv()).await?;
    
    match change_event {
        Some(event) => {
            assert_eq!(event.repo_path, repo_path);
            assert_eq!(event.file_path, PathBuf::from("test.txt"));
        }
        None => panic!("Expected file change event"),
    }
    
    Ok(())
}

#[tokio::test]
async fn test_sync_orchestrator_with_file_watcher() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo_path = create_test_repository(&temp_dir).await?;
    
    let mut config = AutoSyncConfig::default();
    config.file_watcher.enabled = true;
    config.file_watcher.debounce_ms = 100;
    config.sync_after_commit = true;
    
    let repo_manager = create_test_repository_manager();
    let mut sync_orchestrator = SyncOrchestrator::new(config.clone(), repo_manager);
    
    let (app_event_tx, mut app_event_rx) = mpsc::unbounded_channel();
    sync_orchestrator.set_app_event_sender(app_event_tx);
    
    // Create and configure file watcher
    let watcher_config = crate::services::file_watcher::FileWatcherConfig {
        enabled: config.file_watcher.enabled,
        debounce_ms: config.file_watcher.debounce_ms,
        exclude_patterns: config.file_watcher.exclude_patterns.clone(),
        max_buffer_size: 1000,
    };
    
    let mut file_watcher = FileWatcherService::new(watcher_config);
    let _change_rx = file_watcher.start().await?;
    let file_watcher = Arc::new(file_watcher);
    
    // Set file watcher on sync orchestrator
    sync_orchestrator.set_file_watcher(file_watcher.clone()).await;
    
    let _result_rx = sync_orchestrator.start().await?;
    
    // Add repository
    sync_orchestrator.add_repository(&repo_path).await?;
    
    // Verify that the file watcher is watching the repository
    // This is tested indirectly by ensuring no errors occur during setup
    
    Ok(())
}

#[tokio::test]
async fn test_sync_notification_content() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo_path = create_test_repository(&temp_dir).await?;
    
    let config = AutoSyncConfig::default();
    let repo_manager = create_test_repository_manager();
    let mut sync_orchestrator = SyncOrchestrator::new(config, repo_manager);
    
    let (app_event_tx, mut app_event_rx) = mpsc::unbounded_channel();
    sync_orchestrator.set_app_event_sender(app_event_tx);
    
    let _result_rx = sync_orchestrator.start().await?;
    
    // Add and sync repository
    sync_orchestrator.add_repository(&repo_path).await?;
    sync_orchestrator.queue_repository_sync(&repo_path).await?;
    
    // Should receive sync notification
    let event = timeout(Duration::from_secs(10), app_event_rx.recv()).await?;
    
    match event {
        Some(AppEvent::ShowSyncNotification { repository, message, notification_type }) => {
            assert!(!repository.is_empty(), "Repository name should not be empty");
            assert!(!message.is_empty(), "Message should not be empty");
            assert!(matches!(
                notification_type, 
                SyncNotificationType::Success | SyncNotificationType::Info | SyncNotificationType::Warning | SyncNotificationType::Error
            ));
            
            println!("Received sync notification: {} - {}", repository, message);
        }
        other => panic!("Expected ShowSyncNotification event, got: {:?}", other),
    }
    
    Ok(())
}