pub mod repository;
pub mod branch;
pub mod state;
pub mod credentials;
pub mod remote;

// Re-export commonly used types
pub use state::{BranchState, RepositoryState, StateManager};
pub use repository::{GitRepository, RepositoryInfo};
pub use branch::*; 