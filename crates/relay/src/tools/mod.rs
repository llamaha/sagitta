// Placeholder for tools module 

pub mod file;
pub mod repo; // Add repo module
pub mod search; // Declare search module
pub mod edit; // Add edit module
pub mod command; // Add command module
pub mod git; // Add git module

// Re-export actions or relevant types if needed
pub use file::{ReadFileAction, WriteFileAction, CreateDirectoryAction, LineEditAction};
pub use repo::actions::{InitRepoAction, AddRepoAction, UseRepoAction, SyncRepositoryAction, ListRepositoriesAction, RemoveRepositoryAction};
pub use search::SemanticSearchAction;
pub use edit::SemanticEditAction; // Add semantic edit action
pub use command::RunCommandAction; // Add command action 
pub use git::{GitStatusAction, GitAddAction, GitCommitAction}; // Add git actions 