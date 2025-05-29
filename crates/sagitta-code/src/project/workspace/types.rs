use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::agent::conversation::types::{WorkspaceSettings, ProjectType};

/// A project workspace that groups conversations by project context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWorkspace {
    /// Unique identifier for the workspace
    pub id: Uuid,
    
    /// Human-readable name for the workspace
    pub name: String,
    
    /// Root path of the project
    pub project_path: PathBuf,
    
    /// Detected project type
    pub project_type: ProjectType,
    
    /// Repository contexts associated with this workspace
    pub repository_contexts: Vec<String>,
    
    /// Conversation IDs belonging to this workspace
    pub conversation_ids: Vec<Uuid>,
    
    /// When the workspace was created
    pub created_at: DateTime<Utc>,
    
    /// Last activity timestamp
    pub last_active: DateTime<Utc>,
    
    /// Workspace-specific settings
    pub settings: WorkspaceSettings,
    
    /// Whether this workspace is currently active
    pub is_active: bool,
    
    /// Git repository information (if applicable)
    pub git_info: Option<GitInfo>,
    
    /// Environment variables specific to this workspace
    pub environment: std::collections::HashMap<String, String>,
}

/// Git repository information for a workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    /// Current branch name
    pub current_branch: String,
    
    /// Remote origin URL
    pub remote_origin: Option<String>,
    
    /// Last commit hash
    pub last_commit: Option<String>,
    
    /// Whether there are uncommitted changes
    pub has_uncommitted_changes: bool,
    
    /// List of modified files
    pub modified_files: Vec<PathBuf>,
}

/// Summary of a workspace for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub id: Uuid,
    pub name: String,
    pub project_path: PathBuf,
    pub project_type: ProjectType,
    pub conversation_count: usize,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub is_active: bool,
    pub git_branch: Option<String>,
}

impl ProjectWorkspace {
    /// Create a new workspace
    pub fn new(name: String, project_path: PathBuf) -> Self {
        let now = Utc::now();
        let project_type = ProjectType::detect_from_path(&project_path);
        
        Self {
            id: Uuid::new_v4(),
            name,
            project_path,
            project_type,
            repository_contexts: Vec::new(),
            conversation_ids: Vec::new(),
            created_at: now,
            last_active: now,
            settings: WorkspaceSettings::default(),
            is_active: false,
            git_info: None,
            environment: std::collections::HashMap::new(),
        }
    }
    
    /// Add a conversation to this workspace
    pub fn add_conversation(&mut self, conversation_id: Uuid) {
        if !self.conversation_ids.contains(&conversation_id) {
            self.conversation_ids.push(conversation_id);
            self.last_active = Utc::now();
        }
    }
    
    /// Remove a conversation from this workspace
    pub fn remove_conversation(&mut self, conversation_id: Uuid) {
        self.conversation_ids.retain(|&id| id != conversation_id);
        self.last_active = Utc::now();
    }
    
    /// Add a repository context to this workspace
    pub fn add_repository(&mut self, repository_name: String) {
        if !self.repository_contexts.contains(&repository_name) {
            self.repository_contexts.push(repository_name);
            self.last_active = Utc::now();
        }
    }
    
    /// Remove a repository context from this workspace
    pub fn remove_repository(&mut self, repository_name: &str) {
        self.repository_contexts.retain(|name| name != repository_name);
        self.last_active = Utc::now();
    }
    
    /// Update git information for this workspace
    pub fn update_git_info(&mut self, git_info: GitInfo) {
        self.git_info = Some(git_info);
        self.last_active = Utc::now();
    }
    
    /// Mark workspace as active
    pub fn activate(&mut self) {
        self.is_active = true;
        self.last_active = Utc::now();
    }
    
    /// Mark workspace as inactive
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }
    
    /// Create a summary of this workspace
    pub fn to_summary(&self) -> WorkspaceSummary {
        WorkspaceSummary {
            id: self.id,
            name: self.name.clone(),
            project_path: self.project_path.clone(),
            project_type: self.project_type.clone(),
            conversation_count: self.conversation_ids.len(),
            created_at: self.created_at,
            last_active: self.last_active,
            is_active: self.is_active,
            git_branch: self.git_info.as_ref().map(|g| g.current_branch.clone()),
        }
    }
    
    /// Check if this workspace matches a given path
    pub fn matches_path(&self, path: &PathBuf) -> bool {
        path.starts_with(&self.project_path)
    }
    
    /// Get relative path from workspace root
    pub fn get_relative_path(&self, path: &PathBuf) -> Option<PathBuf> {
        path.strip_prefix(&self.project_path).ok().map(|p| p.to_path_buf())
    }
}

impl GitInfo {
    /// Create new git info by reading from a repository
    pub fn from_repository(repo_path: &PathBuf) -> Result<Self, git2::Error> {
        let repo = git2::Repository::open(repo_path)?;
        
        // Get current branch
        let head = repo.head()?;
        let current_branch = head.shorthand().unwrap_or("unknown").to_string();
        
        // Get remote origin
        let remote_origin = repo.find_remote("origin")
            .ok()
            .and_then(|remote| remote.url().map(|url| url.to_string()));
        
        // Get last commit
        let last_commit = head.target().map(|oid| oid.to_string());
        
        // Check for uncommitted changes
        let statuses = repo.statuses(None)?;
        let has_uncommitted_changes = !statuses.is_empty();
        
        // Get modified files
        let mut modified_files = Vec::new();
        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                modified_files.push(PathBuf::from(path));
            }
        }
        
        Ok(GitInfo {
            current_branch,
            remote_origin,
            last_commit,
            has_uncommitted_changes,
            modified_files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_workspace_creation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        
        let workspace = ProjectWorkspace::new("Test Workspace".to_string(), path.clone());
        
        assert_eq!(workspace.name, "Test Workspace");
        assert_eq!(workspace.project_path, path);
        assert_eq!(workspace.project_type, ProjectType::Unknown);
        assert!(workspace.conversation_ids.is_empty());
        assert!(workspace.repository_contexts.is_empty());
        assert!(!workspace.is_active);
    }
    
    #[test]
    fn test_workspace_project_type_detection() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        
        // Create Cargo.toml to make it a Rust project
        fs::write(path.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        
        let workspace = ProjectWorkspace::new("Rust Project".to_string(), path);
        
        assert_eq!(workspace.project_type, ProjectType::Rust);
    }
    
    #[test]
    fn test_workspace_conversation_management() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut workspace = ProjectWorkspace::new("Test".to_string(), path);
        
        let conv_id = Uuid::new_v4();
        let original_time = workspace.last_active;
        
        // Small delay to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(1));
        
        // Add conversation
        workspace.add_conversation(conv_id);
        assert_eq!(workspace.conversation_ids.len(), 1);
        assert!(workspace.conversation_ids.contains(&conv_id));
        assert!(workspace.last_active > original_time);
        
        // Try to add same conversation again (should not duplicate)
        workspace.add_conversation(conv_id);
        assert_eq!(workspace.conversation_ids.len(), 1);
        
        // Remove conversation
        workspace.remove_conversation(conv_id);
        assert!(workspace.conversation_ids.is_empty());
    }
    
    #[test]
    fn test_workspace_repository_management() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut workspace = ProjectWorkspace::new("Test".to_string(), path);
        
        let repo_name = "test-repo".to_string();
        
        // Add repository
        workspace.add_repository(repo_name.clone());
        assert_eq!(workspace.repository_contexts.len(), 1);
        assert!(workspace.repository_contexts.contains(&repo_name));
        
        // Try to add same repository again (should not duplicate)
        workspace.add_repository(repo_name.clone());
        assert_eq!(workspace.repository_contexts.len(), 1);
        
        // Remove repository
        workspace.remove_repository(&repo_name);
        assert!(workspace.repository_contexts.is_empty());
    }
    
    #[test]
    fn test_workspace_activation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut workspace = ProjectWorkspace::new("Test".to_string(), path);
        
        assert!(!workspace.is_active);
        
        workspace.activate();
        assert!(workspace.is_active);
        
        workspace.deactivate();
        assert!(!workspace.is_active);
    }
    
    #[test]
    fn test_workspace_path_matching() {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path().to_path_buf();
        let workspace = ProjectWorkspace::new("Test".to_string(), root_path.clone());
        
        // Test exact match
        assert!(workspace.matches_path(&root_path));
        
        // Test subdirectory match
        let sub_path = root_path.join("src").join("main.rs");
        assert!(workspace.matches_path(&sub_path));
        
        // Test non-matching path
        let other_path = PathBuf::from("/completely/different/path");
        assert!(!workspace.matches_path(&other_path));
    }
    
    #[test]
    fn test_workspace_relative_path() {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path().to_path_buf();
        let workspace = ProjectWorkspace::new("Test".to_string(), root_path.clone());
        
        // Test relative path calculation
        let sub_path = root_path.join("src").join("main.rs");
        let relative = workspace.get_relative_path(&sub_path);
        
        assert!(relative.is_some());
        assert_eq!(relative.unwrap(), PathBuf::from("src").join("main.rs"));
        
        // Test non-matching path
        let other_path = PathBuf::from("/completely/different/path");
        let relative = workspace.get_relative_path(&other_path);
        assert!(relative.is_none());
    }
    
    #[test]
    fn test_workspace_summary() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let mut workspace = ProjectWorkspace::new("Test Workspace".to_string(), path.clone());
        
        workspace.add_conversation(Uuid::new_v4());
        workspace.add_conversation(Uuid::new_v4());
        workspace.activate();
        
        let summary = workspace.to_summary();
        
        assert_eq!(summary.name, "Test Workspace");
        assert_eq!(summary.project_path, path);
        assert_eq!(summary.conversation_count, 2);
        assert!(summary.is_active);
        assert!(summary.git_branch.is_none());
    }
    
    #[test]
    fn test_git_info_creation() {
        let git_info = GitInfo {
            current_branch: "main".to_string(),
            remote_origin: Some("https://github.com/user/repo.git".to_string()),
            last_commit: Some("abc123".to_string()),
            has_uncommitted_changes: false,
            modified_files: Vec::new(),
        };
        
        assert_eq!(git_info.current_branch, "main");
        assert_eq!(git_info.remote_origin, Some("https://github.com/user/repo.git".to_string()));
        assert!(!git_info.has_uncommitted_changes);
    }
} 