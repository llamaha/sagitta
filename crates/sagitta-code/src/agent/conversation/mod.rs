pub mod types;
pub mod manager;
pub mod persistence;
pub mod search;
pub mod branching;
pub mod clustering;
pub mod analytics;
pub mod navigation;
pub mod checkpoints;

// Re-export key types for easy access
pub use types::{
    Conversation, ConversationBranch, ConversationCheckpoint, ConversationMetadata,
    ConversationSummary, ConversationQuery, ConversationSearchResult, BranchStatus,
    ProjectContext, ContextSnapshot, WorkspaceSettings,
};
pub use manager::{ConversationManager, ConversationManagerImpl};
pub use analytics::*;
pub use branching::*;
pub use checkpoints::*; 