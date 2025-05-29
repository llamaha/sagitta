pub mod types;
pub mod manager;
pub mod detection;
pub mod settings;

// Re-export key types for easy access
pub use types::{ProjectWorkspace, WorkspaceSummary};
pub use manager::{WorkspaceManager, WorkspaceManagerImpl};
pub use detection::WorkspaceDetector; 