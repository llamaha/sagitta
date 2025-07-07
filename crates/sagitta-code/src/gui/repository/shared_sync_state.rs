// crates/sagitta-code/src/gui/repository/shared_sync_state.rs
use once_cell::sync::Lazy;
use dashmap::DashMap;

use super::types::{SimpleSyncStatus, DisplayableSyncProgress};

/// Simple 0/1 progress + log lines – what the panel already shows
pub static SIMPLE_STATUS: Lazy<DashMap<String, SimpleSyncStatus>> =
    Lazy::new(DashMap::new);

/// Detailed stage/percentage information
pub static DETAILED_STATUS: Lazy<DashMap<String, DisplayableSyncProgress>> =
    Lazy::new(DashMap::new);

/// Schedule cleanup of completed sync status after a delay
pub fn schedule_sync_status_cleanup(repo_name: String) {
    schedule_sync_status_cleanup_with_delay(repo_name, tokio::time::Duration::from_secs(10));
}

/// Schedule cleanup with a custom delay (useful for testing)
fn schedule_sync_status_cleanup_with_delay(repo_name: String, delay: tokio::time::Duration) {
    tokio::spawn(async move {
        // Wait for the specified delay before clearing
        tokio::time::sleep(delay).await;
        
        // Remove both simple and detailed status for this repository
        SIMPLE_STATUS.remove(&repo_name);
        DETAILED_STATUS.remove(&repo_name);
        
        log::debug!("Cleaned up sync status for repository: {}", repo_name);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::repository::types::SimpleSyncStatus;
    
    #[tokio::test]
    async fn test_sync_status_auto_cleanup() {
        let repo_name = "test-cleanup-repo";
        
        // Insert a completed sync status
        SIMPLE_STATUS.insert(repo_name.to_string(), SimpleSyncStatus {
            is_running: false,
            is_complete: true,
            is_success: true,
            output_lines: vec!["Test completed".to_string()],
            final_message: "✅ Completed".to_string(),
            started_at: Some(std::time::Instant::now()),
            final_elapsed_seconds: Some(1.0),
            last_progress_time: Some(std::time::Instant::now()),
        });
        
        // Verify it's there
        assert!(SIMPLE_STATUS.contains_key(repo_name));
        
        // Schedule cleanup with a very short delay for testing
        schedule_sync_status_cleanup_with_delay(repo_name.to_string(), tokio::time::Duration::from_millis(50));
        
        // Wait a bit less than cleanup time - should still be there
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        assert!(SIMPLE_STATUS.contains_key(repo_name));
        
        // Wait for cleanup to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(40)).await;
        assert!(!SIMPLE_STATUS.contains_key(repo_name));
    }
} 