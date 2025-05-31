use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use anyhow::Result;

use crate::agent::conversation::types::ProjectType;
use super::types::{ProjectWorkspace, GitInfo};

/// Workspace detector for automatically identifying project workspaces
pub struct WorkspaceDetector {
    /// Maximum depth to search for project markers
    max_depth: usize,
    
    /// Whether to include git information
    include_git_info: bool,
}

impl Default for WorkspaceDetector {
    fn default() -> Self {
        Self {
            max_depth: 3,
            include_git_info: true,
        }
    }
}

impl WorkspaceDetector {
    /// Create a new workspace detector
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set maximum search depth
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }
    
    /// Set whether to include git information
    pub fn with_git_info(mut self, include: bool) -> Self {
        self.include_git_info = include;
        self
    }
    
    /// Detect workspace from a given path
    pub fn detect_workspace(&self, path: &Path) -> Result<Option<ProjectWorkspace>> {
        if let Some(project_root) = self.find_project_root(path)? {
            let name = self.generate_workspace_name(&project_root);
            let mut workspace = ProjectWorkspace::new(name, project_root.clone());
            
            // Update git information if requested
            if self.include_git_info {
                if let Ok(git_info) = GitInfo::from_repository(&project_root) {
                    workspace.update_git_info(git_info);
                }
            }
            
            Ok(Some(workspace))
        } else {
            Ok(None)
        }
    }
    
    /// Find the project root by looking for project markers
    pub fn find_project_root(&self, start_path: &Path) -> Result<Option<PathBuf>> {
        let mut current = start_path.to_path_buf();
        
        // Walk up the directory tree
        loop {
            if self.is_project_root(&current) {
                return Ok(Some(current));
            }
            
            // Move to parent directory
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                break;
            }
        }
        
        // If no project root found walking up, try searching down
        self.search_project_roots_down(start_path)
    }
    
    /// Check if a directory is a project root
    fn is_project_root(&self, path: &Path) -> bool {
        // Check for common project markers
        let markers = [
            "Cargo.toml",           // Rust
            "package.json",         // Node.js/JavaScript/TypeScript
            "requirements.txt",     // Python
            "pyproject.toml",       // Python (modern)
            "setup.py",             // Python (legacy)
            "go.mod",               // Go
            "pom.xml",              // Java (Maven)
            "build.gradle",         // Java/Kotlin (Gradle)
            "CMakeLists.txt",       // C/C++ (CMake)
            "Makefile",             // C/C++/Make
            "*.csproj",             // C#
            "*.sln",                // C# Solution
            ".git",                 // Git repository
            ".gitignore",           // Git repository (alternative)
        ];
        
        for marker in &markers {
            if marker.contains('*') {
                // Handle glob patterns - check for proper file extension match
                let extension = marker.strip_prefix("*.").unwrap_or("");
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.ends_with(&format!(".{}", extension)) {
                                return true;
                            }
                        }
                    }
                }
            } else if path.join(marker).exists() {
                return true;
            }
        }
        
        false
    }
    
    /// Search for project roots in subdirectories
    fn search_project_roots_down(&self, start_path: &Path) -> Result<Option<PathBuf>> {
        for entry in WalkDir::new(start_path)
            .max_depth(self.max_depth)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() && self.is_project_root(entry.path()) {
                return Ok(Some(entry.path().to_path_buf()));
            }
        }
        
        Ok(None)
    }
    
    /// Generate a workspace name from the project path
    fn generate_workspace_name(&self, project_path: &Path) -> String {
        // Try to get name from project files
        if let Some(name) = self.extract_name_from_project_files(project_path) {
            return name;
        }
        
        // Fall back to directory name
        project_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Unknown Project")
            .to_string()
    }
    
    /// Extract project name from project-specific files
    fn extract_name_from_project_files(&self, project_path: &Path) -> Option<String> {
        // Try Cargo.toml for Rust projects
        if let Ok(cargo_content) = std::fs::read_to_string(project_path.join("Cargo.toml")) {
            if let Some(name) = self.extract_toml_name(&cargo_content) {
                return Some(name);
            }
        }
        
        // Try package.json for Node.js projects
        if let Ok(package_content) = std::fs::read_to_string(project_path.join("package.json")) {
            if let Ok(package_json) = serde_json::from_str::<serde_json::Value>(&package_content) {
                if let Some(name) = package_json.get("name").and_then(|n| n.as_str()) {
                    return Some(name.to_string());
                }
            }
        }
        
        // Try pyproject.toml for Python projects
        if let Ok(pyproject_content) = std::fs::read_to_string(project_path.join("pyproject.toml")) {
            if let Some(name) = self.extract_toml_name(&pyproject_content) {
                return Some(name);
            }
        }
        
        // Try go.mod for Go projects
        if let Ok(go_mod_content) = std::fs::read_to_string(project_path.join("go.mod")) {
            if let Some(first_line) = go_mod_content.lines().next() {
                if first_line.starts_with("module ") {
                    let module_name = first_line.strip_prefix("module ").unwrap_or("");
                    if let Some(last_part) = module_name.split('/').last() {
                        return Some(last_part.to_string());
                    }
                }
            }
        }
        
        None
    }
    
    /// Extract name from TOML content (simple parser)
    fn extract_toml_name(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("name") && line.contains('=') {
                let parts: Vec<&str> = line.split('=').collect();
                if parts.len() == 2 {
                    let name = parts[1].trim().trim_matches('"').trim_matches('\'');
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
        None
    }
    
    /// Detect all workspaces in a directory tree
    pub fn detect_all_workspaces(&self, root_path: &Path) -> Result<Vec<ProjectWorkspace>> {
        let mut workspaces = Vec::new();
        let mut visited_roots = std::collections::HashSet::new();
        
        for entry in WalkDir::new(root_path)
            .max_depth(self.max_depth)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() && self.is_project_root(entry.path()) {
                let project_root = entry.path().to_path_buf();
                
                // Avoid duplicate workspaces for the same root
                if visited_roots.insert(project_root.clone()) {
                    let name = self.generate_workspace_name(&project_root);
                    let mut workspace = ProjectWorkspace::new(name, project_root.clone());
                    
                    // Update git information if requested
                    if self.include_git_info {
                        if let Ok(git_info) = GitInfo::from_repository(&project_root) {
                            workspace.update_git_info(git_info);
                        }
                    }
                    
                    workspaces.push(workspace);
                }
            }
        }
        
        Ok(workspaces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_workspace_detector_creation() {
        let detector = WorkspaceDetector::new();
        assert_eq!(detector.max_depth, 3);
        assert!(detector.include_git_info);
    }
    
    #[test]
    fn test_workspace_detector_configuration() {
        let detector = WorkspaceDetector::new()
            .with_max_depth(5)
            .with_git_info(false);
        
        assert_eq!(detector.max_depth, 5);
        assert!(!detector.include_git_info);
    }
    
    #[test]
    fn test_is_project_root_rust() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let detector = WorkspaceDetector::new();
        
        // Not a project root initially
        assert!(!detector.is_project_root(path));
        
        // Create Cargo.toml
        fs::write(path.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        assert!(detector.is_project_root(path));
    }
    
    #[test]
    fn test_is_project_root_nodejs() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let detector = WorkspaceDetector::new();
        
        // Create package.json
        fs::write(path.join("package.json"), r#"{"name": "test"}"#).unwrap();
        assert!(detector.is_project_root(path));
    }
    
    #[test]
    fn test_is_project_root_python() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let detector = WorkspaceDetector::new();
        
        // Create requirements.txt
        fs::write(path.join("requirements.txt"), "requests==2.25.1").unwrap();
        assert!(detector.is_project_root(path));
        
        // Remove and try pyproject.toml
        fs::remove_file(path.join("requirements.txt")).unwrap();
        fs::write(path.join("pyproject.toml"), "[project]\nname = \"test\"").unwrap();
        assert!(detector.is_project_root(path));
    }
    
    #[test]
    fn test_is_project_root_git() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let detector = WorkspaceDetector::new();
        
        // Create .git directory
        fs::create_dir(path.join(".git")).unwrap();
        assert!(detector.is_project_root(path));
    }
    
    #[test]
    fn test_extract_name_from_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let detector = WorkspaceDetector::new();
        
        let cargo_content = r#"
[package]
name = "my-awesome-project"
version = "0.1.0"
"#;
        fs::write(path.join("Cargo.toml"), cargo_content).unwrap();
        
        let name = detector.extract_name_from_project_files(path);
        assert_eq!(name, Some("my-awesome-project".to_string()));
    }
    
    #[test]
    fn test_extract_name_from_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let detector = WorkspaceDetector::new();
        
        let package_content = r#"
{
  "name": "my-node-project",
  "version": "1.0.0"
}
"#;
        fs::write(path.join("package.json"), package_content).unwrap();
        
        let name = detector.extract_name_from_project_files(path);
        assert_eq!(name, Some("my-node-project".to_string()));
    }
    
    #[test]
    fn test_extract_name_from_go_mod() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let detector = WorkspaceDetector::new();
        
        let go_mod_content = "module github.com/user/my-go-project\n\ngo 1.19\n";
        fs::write(path.join("go.mod"), go_mod_content).unwrap();
        
        let name = detector.extract_name_from_project_files(path);
        assert_eq!(name, Some("my-go-project".to_string()));
    }
    
    #[test]
    fn test_generate_workspace_name_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let detector = WorkspaceDetector::new();
        
        // No project files, should fall back to directory name
        let name = detector.generate_workspace_name(path);
        // tempfile creates dirs with patterns like ".tmpXXXXXX"
        assert!(name.starts_with(".tmp") || name.starts_with("tmp") || name.contains("temp"));
    }
    
    #[test]
    fn test_find_project_root_walking_up() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let detector = WorkspaceDetector::new();
        
        // Create nested directory structure
        let nested_path = root.join("src").join("deep").join("nested");
        fs::create_dir_all(&nested_path).unwrap();
        
        // Create Cargo.toml at root
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        
        // Should find root when starting from nested path
        let found_root = detector.find_project_root(&nested_path).unwrap();
        assert_eq!(found_root, Some(root.to_path_buf()));
    }
    
    #[test]
    fn test_detect_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let detector = WorkspaceDetector::new().with_git_info(false);
        
        // Create a Rust project
        fs::write(path.join("Cargo.toml"), "[package]\nname = \"test-project\"").unwrap();
        
        let workspace = detector.detect_workspace(path).unwrap();
        assert!(workspace.is_some());
        
        let workspace = workspace.unwrap();
        assert_eq!(workspace.name, "test-project");
        assert_eq!(workspace.project_path, path);
        assert_eq!(workspace.project_type, ProjectType::Rust);
    }
    
    #[test]
    fn test_detect_workspace_no_project() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let detector = WorkspaceDetector::new();
        
        // Empty directory, no project markers
        let workspace = detector.detect_workspace(path).unwrap();
        assert!(workspace.is_none());
    }
    
    #[test]
    fn test_detect_all_workspaces() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let detector = WorkspaceDetector::new().with_git_info(false);
        
        // Create multiple projects
        let rust_project = root.join("rust-project");
        fs::create_dir_all(&rust_project).unwrap();
        fs::write(rust_project.join("Cargo.toml"), "[package]\nname = \"rust-proj\"").unwrap();
        
        let node_project = root.join("node-project");
        fs::create_dir_all(&node_project).unwrap();
        fs::write(node_project.join("package.json"), r#"{"name": "node-proj"}"#).unwrap();
        
        let workspaces = detector.detect_all_workspaces(root).unwrap();
        assert_eq!(workspaces.len(), 2);
        
        let names: Vec<String> = workspaces.iter().map(|w| w.name.clone()).collect();
        assert!(names.contains(&"rust-proj".to_string()));
        assert!(names.contains(&"node-proj".to_string()));
    }
} 