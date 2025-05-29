pub mod manager;
pub mod rules;
pub mod workspace;

// Re-export key types
pub use manager::ProjectManager;
pub use workspace::{ProjectWorkspace, WorkspaceManager, WorkspaceDetector}; 