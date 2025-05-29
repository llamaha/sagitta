// Workspace settings management
// TODO: Implement settings persistence and validation

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::agent::conversation::types::WorkspaceSettings;

/// Manager for workspace settings
pub struct WorkspaceSettingsManager {
    /// Settings storage path
    storage_path: PathBuf,
}

impl WorkspaceSettingsManager {
    /// Create a new settings manager
    pub fn new(storage_path: PathBuf) -> Self {
        Self { storage_path }
    }
    
    /// Load settings for a workspace
    pub async fn load_settings(&self, _workspace_id: uuid::Uuid) -> Result<WorkspaceSettings> {
        // TODO: Implement loading from disk
        Ok(WorkspaceSettings::default())
    }
    
    /// Save settings for a workspace
    pub async fn save_settings(&self, _workspace_id: uuid::Uuid, _settings: &WorkspaceSettings) -> Result<()> {
        // TODO: Implement saving to disk
        Ok(())
    }
    
    /// Get default settings template
    pub fn get_default_settings() -> WorkspaceSettings {
        WorkspaceSettings::default()
    }
    
    /// Validate settings
    pub fn validate_settings(settings: &WorkspaceSettings) -> Result<()> {
        // Validate max_conversations
        if let Some(max) = settings.max_conversations {
            if max == 0 {
                return Err(anyhow::anyhow!("max_conversations must be greater than 0"));
            }
        }
        
        // Validate auto_cleanup_days
        if let Some(days) = settings.auto_cleanup_days {
            if days == 0 {
                return Err(anyhow::anyhow!("auto_cleanup_days must be greater than 0"));
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_settings_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        
        let manager = WorkspaceSettingsManager::new(storage_path);
        
        // Should be able to load default settings
        let workspace_id = uuid::Uuid::new_v4();
        let settings = manager.load_settings(workspace_id).await.unwrap();
        
        assert!(settings.auto_context_loading);
        assert_eq!(settings.max_conversations, Some(100));
    }
    
    #[test]
    fn test_settings_validation() {
        let mut settings = WorkspaceSettings::default();
        
        // Valid settings should pass
        assert!(WorkspaceSettingsManager::validate_settings(&settings).is_ok());
        
        // Invalid max_conversations should fail
        settings.max_conversations = Some(0);
        assert!(WorkspaceSettingsManager::validate_settings(&settings).is_err());
        
        // Reset and test auto_cleanup_days
        settings.max_conversations = Some(100);
        settings.auto_cleanup_days = Some(0);
        assert!(WorkspaceSettingsManager::validate_settings(&settings).is_err());
    }
    
    #[test]
    fn test_default_settings() {
        let settings = WorkspaceSettingsManager::get_default_settings();
        
        assert!(settings.auto_context_loading);
        assert_eq!(settings.max_conversations, Some(100));
        assert_eq!(settings.auto_cleanup_days, Some(30));
        assert!(settings.auto_checkpoints);
        assert!(!settings.auto_branching);
        assert!(settings.default_tags.is_empty());
    }
} 