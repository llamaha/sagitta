use async_trait::async_trait;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::types::{ProjectWorkspace, WorkspaceSummary, GitInfo};
use super::detection::WorkspaceDetector;

/// Trait for managing project workspaces
#[async_trait]
pub trait WorkspaceManager: Send + Sync {
    /// Create a new workspace
    async fn create_workspace(&mut self, name: String, project_path: PathBuf) -> Result<Uuid>;
    
    /// Get a workspace by ID
    async fn get_workspace(&self, id: Uuid) -> Result<Option<ProjectWorkspace>>;
    
    /// Get workspace by project path
    async fn get_workspace_by_path(&self, path: &Path) -> Result<Option<ProjectWorkspace>>;
    
    /// Update an existing workspace
    async fn update_workspace(&mut self, workspace: ProjectWorkspace) -> Result<()>;
    
    /// Delete a workspace
    async fn delete_workspace(&mut self, id: Uuid) -> Result<()>;
    
    /// List all workspaces
    async fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>>;
    
    /// Detect workspace from current path
    async fn detect_workspace(&self, current_path: &Path) -> Result<Option<Uuid>>;
    
    /// Add a conversation to a workspace
    async fn add_conversation_to_workspace(&mut self, workspace_id: Uuid, conversation_id: Uuid) -> Result<()>;
    
    /// Remove a conversation from a workspace
    async fn remove_conversation_from_workspace(&mut self, workspace_id: Uuid, conversation_id: Uuid) -> Result<()>;
    
    /// Get the currently active workspace
    async fn get_active_workspace(&self) -> Result<Option<ProjectWorkspace>>;
    
    /// Set the active workspace
    async fn set_active_workspace(&mut self, workspace_id: Option<Uuid>) -> Result<()>;
    
    /// Auto-detect and create workspace if needed
    async fn auto_detect_workspace(&mut self, current_path: &Path) -> Result<Option<Uuid>>;
}

/// Implementation of the workspace manager
pub struct WorkspaceManagerImpl {
    /// In-memory cache of workspaces
    workspaces: Arc<RwLock<HashMap<Uuid, ProjectWorkspace>>>,
    
    /// Currently active workspace ID
    active_workspace_id: Arc<RwLock<Option<Uuid>>>,
    
    /// Workspace detector
    detector: WorkspaceDetector,
    
    /// Storage path for workspace data
    storage_path: PathBuf,
}

impl WorkspaceManagerImpl {
    /// Create a new workspace manager
    pub fn new(storage_path: PathBuf) -> Self {
        Self {
            workspaces: Arc::new(RwLock::new(HashMap::new())),
            active_workspace_id: Arc::new(RwLock::new(None)),
            detector: WorkspaceDetector::new(),
            storage_path,
        }
    }
    
    /// Load workspaces from storage
    pub async fn load_workspaces(&mut self) -> Result<()> {
        // TODO: Implement loading from disk
        // For now, this is a placeholder
        Ok(())
    }
    
    /// Save workspaces to storage
    async fn save_workspaces(&self) -> Result<()> {
        // TODO: Implement saving to disk
        // For now, this is a placeholder
        Ok(())
    }
    
    /// Update git information for a workspace
    async fn update_workspace_git_info(&self, workspace_id: Uuid) -> Result<()> {
        let mut workspaces = self.workspaces.write().await;
        if let Some(workspace) = workspaces.get_mut(&workspace_id) {
            if let Ok(git_info) = GitInfo::from_repository(&workspace.project_path) {
                workspace.update_git_info(git_info);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl WorkspaceManager for WorkspaceManagerImpl {
    async fn create_workspace(&mut self, name: String, project_path: PathBuf) -> Result<Uuid> {
        let mut workspace = ProjectWorkspace::new(name, project_path);
        
        // Try to get git information
        if let Ok(git_info) = GitInfo::from_repository(&workspace.project_path) {
            workspace.update_git_info(git_info);
        }
        
        let id = workspace.id;
        
        // Add to memory cache
        {
            let mut workspaces = self.workspaces.write().await;
            workspaces.insert(id, workspace);
        }
        
        // Save to storage
        self.save_workspaces().await?;
        
        Ok(id)
    }
    
    async fn get_workspace(&self, id: Uuid) -> Result<Option<ProjectWorkspace>> {
        let workspaces = self.workspaces.read().await;
        Ok(workspaces.get(&id).cloned())
    }
    
    async fn get_workspace_by_path(&self, path: &Path) -> Result<Option<ProjectWorkspace>> {
        let workspaces = self.workspaces.read().await;
        for workspace in workspaces.values() {
            if workspace.matches_path(&path.to_path_buf()) {
                return Ok(Some(workspace.clone()));
            }
        }
        Ok(None)
    }
    
    async fn update_workspace(&mut self, workspace: ProjectWorkspace) -> Result<()> {
        let id = workspace.id;
        
        // Update memory cache
        {
            let mut workspaces = self.workspaces.write().await;
            workspaces.insert(id, workspace);
        }
        
        // Save to storage
        self.save_workspaces().await?;
        
        Ok(())
    }
    
    async fn delete_workspace(&mut self, id: Uuid) -> Result<()> {
        // Remove from memory cache
        {
            let mut workspaces = self.workspaces.write().await;
            workspaces.remove(&id);
        }
        
        // If this was the active workspace, clear it
        {
            let mut active_id = self.active_workspace_id.write().await;
            if *active_id == Some(id) {
                *active_id = None;
            }
        }
        
        // Save to storage
        self.save_workspaces().await?;
        
        Ok(())
    }
    
    async fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        let workspaces = self.workspaces.read().await;
        let mut summaries: Vec<WorkspaceSummary> = workspaces
            .values()
            .map(|workspace| workspace.to_summary())
            .collect();
        
        // Sort by last activity (most recent first)
        summaries.sort_by(|a, b| b.last_active.cmp(&a.last_active));
        
        Ok(summaries)
    }
    
    async fn detect_workspace(&self, current_path: &Path) -> Result<Option<Uuid>> {
        // First check if we already have a workspace for this path
        if let Some(workspace) = self.get_workspace_by_path(current_path).await? {
            return Ok(Some(workspace.id));
        }
        
        // Try to detect a workspace using the detector
        if let Some(_detected_workspace) = self.detector.detect_workspace(current_path)? {
            // We found a workspace but haven't created it yet
            // Return None to indicate detection is possible but not yet created
            return Ok(None);
        }
        
        Ok(None)
    }
    
    async fn add_conversation_to_workspace(&mut self, workspace_id: Uuid, conversation_id: Uuid) -> Result<()> {
        let mut workspaces = self.workspaces.write().await;
        if let Some(workspace) = workspaces.get_mut(&workspace_id) {
            workspace.add_conversation(conversation_id);
            drop(workspaces); // Release lock before saving
            self.save_workspaces().await?;
        } else {
            return Err(anyhow::anyhow!("Workspace not found: {}", workspace_id));
        }
        Ok(())
    }
    
    async fn remove_conversation_from_workspace(&mut self, workspace_id: Uuid, conversation_id: Uuid) -> Result<()> {
        let mut workspaces = self.workspaces.write().await;
        if let Some(workspace) = workspaces.get_mut(&workspace_id) {
            workspace.remove_conversation(conversation_id);
            drop(workspaces); // Release lock before saving
            self.save_workspaces().await?;
        } else {
            return Err(anyhow::anyhow!("Workspace not found: {}", workspace_id));
        }
        Ok(())
    }
    
    async fn get_active_workspace(&self) -> Result<Option<ProjectWorkspace>> {
        let active_id = self.active_workspace_id.read().await;
        if let Some(id) = *active_id {
            self.get_workspace(id).await
        } else {
            Ok(None)
        }
    }
    
    async fn set_active_workspace(&mut self, workspace_id: Option<Uuid>) -> Result<()> {
        // Validate workspace exists if ID is provided
        if let Some(id) = workspace_id {
            if self.get_workspace(id).await?.is_none() {
                return Err(anyhow::anyhow!("Workspace not found: {}", id));
            }
        }
        
        // Deactivate current workspace
        {
            let active_id = self.active_workspace_id.read().await;
            if let Some(current_id) = *active_id {
                let mut workspaces = self.workspaces.write().await;
                if let Some(workspace) = workspaces.get_mut(&current_id) {
                    workspace.deactivate();
                }
            }
        }
        
        // Set new active workspace
        {
            let mut active_id = self.active_workspace_id.write().await;
            *active_id = workspace_id;
        }
        
        // Activate new workspace
        if let Some(id) = workspace_id {
            let mut workspaces = self.workspaces.write().await;
            if let Some(workspace) = workspaces.get_mut(&id) {
                workspace.activate();
            }
        }
        
        // Update git info for the new active workspace
        if let Some(id) = workspace_id {
            self.update_workspace_git_info(id).await?;
        }
        
        self.save_workspaces().await?;
        
        Ok(())
    }
    
    async fn auto_detect_workspace(&mut self, current_path: &Path) -> Result<Option<Uuid>> {
        // First check if we already have a workspace for this path
        if let Some(workspace) = self.get_workspace_by_path(current_path).await? {
            // Set as active if not already
            if !workspace.is_active {
                self.set_active_workspace(Some(workspace.id)).await?;
            }
            return Ok(Some(workspace.id));
        }
        
        // Try to detect and create a new workspace
        if let Some(detected_workspace) = self.detector.detect_workspace(current_path)? {
            let workspace_id = self.create_workspace(detected_workspace.name, detected_workspace.project_path).await?;
            self.set_active_workspace(Some(workspace_id)).await?;
            return Ok(Some(workspace_id));
        }
        
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[tokio::test]
    async fn test_workspace_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();
        
        let manager = WorkspaceManagerImpl::new(storage_path);
        
        // Should start with no workspaces
        let workspaces = manager.list_workspaces().await.unwrap();
        assert!(workspaces.is_empty());
        
        // Should have no active workspace
        let active = manager.get_active_workspace().await.unwrap();
        assert!(active.is_none());
    }
    
    #[tokio::test]
    async fn test_create_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("storage");
        let project_path = temp_dir.path().join("project");
        fs::create_dir_all(&project_path).unwrap();
        
        let mut manager = WorkspaceManagerImpl::new(storage_path);
        
        let workspace_id = manager.create_workspace("Test Project".to_string(), project_path.clone()).await.unwrap();
        
        let workspace = manager.get_workspace(workspace_id).await.unwrap();
        assert!(workspace.is_some());
        
        let workspace = workspace.unwrap();
        assert_eq!(workspace.name, "Test Project");
        assert_eq!(workspace.project_path, project_path);
    }
    
    #[tokio::test]
    async fn test_get_workspace_by_path() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("storage");
        let project_path = temp_dir.path().join("project");
        fs::create_dir_all(&project_path).unwrap();
        
        let mut manager = WorkspaceManagerImpl::new(storage_path);
        
        let workspace_id = manager.create_workspace("Test Project".to_string(), project_path.clone()).await.unwrap();
        
        // Should find workspace by exact path
        let workspace = manager.get_workspace_by_path(&project_path).await.unwrap();
        assert!(workspace.is_some());
        assert_eq!(workspace.unwrap().id, workspace_id);
        
        // Should find workspace by subdirectory path
        let sub_path = project_path.join("src");
        let workspace = manager.get_workspace_by_path(&sub_path).await.unwrap();
        assert!(workspace.is_some());
        assert_eq!(workspace.unwrap().id, workspace_id);
        
        // Should not find workspace for unrelated path
        let other_path = temp_dir.path().join("other");
        let workspace = manager.get_workspace_by_path(&other_path).await.unwrap();
        assert!(workspace.is_none());
    }
    
    #[tokio::test]
    async fn test_active_workspace_management() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("storage");
        let project_path = temp_dir.path().join("project");
        fs::create_dir_all(&project_path).unwrap();
        
        let mut manager = WorkspaceManagerImpl::new(storage_path);
        
        // Should start with no active workspace
        let active = manager.get_active_workspace().await.unwrap();
        assert!(active.is_none());
        
        let workspace_id = manager.create_workspace("Test Project".to_string(), project_path).await.unwrap();
        
        // Set as active
        manager.set_active_workspace(Some(workspace_id)).await.unwrap();
        
        let active = manager.get_active_workspace().await.unwrap();
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, workspace_id);
        
        // Clear active workspace
        manager.set_active_workspace(None).await.unwrap();
        
        let active = manager.get_active_workspace().await.unwrap();
        assert!(active.is_none());
    }
    
    #[tokio::test]
    async fn test_conversation_management() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("storage");
        let project_path = temp_dir.path().join("project");
        fs::create_dir_all(&project_path).unwrap();
        
        let mut manager = WorkspaceManagerImpl::new(storage_path);
        
        let workspace_id = manager.create_workspace("Test Project".to_string(), project_path).await.unwrap();
        let conversation_id = Uuid::new_v4();
        
        // Add conversation to workspace
        manager.add_conversation_to_workspace(workspace_id, conversation_id).await.unwrap();
        
        let workspace = manager.get_workspace(workspace_id).await.unwrap().unwrap();
        assert_eq!(workspace.conversation_ids.len(), 1);
        assert!(workspace.conversation_ids.contains(&conversation_id));
        
        // Remove conversation from workspace
        manager.remove_conversation_from_workspace(workspace_id, conversation_id).await.unwrap();
        
        let workspace = manager.get_workspace(workspace_id).await.unwrap().unwrap();
        assert!(workspace.conversation_ids.is_empty());
    }
    
    #[tokio::test]
    async fn test_auto_detect_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("storage");
        let project_path = temp_dir.path().join("project");
        fs::create_dir_all(&project_path).unwrap();
        
        // Create a Rust project
        fs::write(project_path.join("Cargo.toml"), "[package]\nname = \"test-project\"").unwrap();
        
        let mut manager = WorkspaceManagerImpl::new(storage_path);
        
        // Should auto-detect and create workspace
        let workspace_id = manager.auto_detect_workspace(&project_path).await.unwrap();
        assert!(workspace_id.is_some());
        
        let workspace = manager.get_workspace(workspace_id.unwrap()).await.unwrap().unwrap();
        assert_eq!(workspace.name, "test-project");
        assert!(workspace.is_active);
        
        // Second call should return same workspace
        let workspace_id2 = manager.auto_detect_workspace(&project_path).await.unwrap();
        assert_eq!(workspace_id, workspace_id2);
    }
    
    #[tokio::test]
    async fn test_list_workspaces() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("storage");
        
        let mut manager = WorkspaceManagerImpl::new(storage_path);
        
        // Create multiple workspaces
        let project1 = temp_dir.path().join("project1");
        let project2 = temp_dir.path().join("project2");
        fs::create_dir_all(&project1).unwrap();
        fs::create_dir_all(&project2).unwrap();
        
        manager.create_workspace("Project 1".to_string(), project1).await.unwrap();
        manager.create_workspace("Project 2".to_string(), project2).await.unwrap();
        
        let workspaces = manager.list_workspaces().await.unwrap();
        assert_eq!(workspaces.len(), 2);
        
        let names: Vec<String> = workspaces.iter().map(|w| w.name.clone()).collect();
        assert!(names.contains(&"Project 1".to_string()));
        assert!(names.contains(&"Project 2".to_string()));
    }
} 