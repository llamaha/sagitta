pub mod analytics;
pub mod branching;
pub mod checkpoints;
pub mod clustering;
pub mod manager;
pub mod navigation;
pub mod persistence;
pub mod search;
pub mod search_service;
pub mod service;
pub mod tagging;
pub mod types;

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
pub use clustering::*;
pub use navigation::*;
pub use persistence::*;
pub use search::*;
pub use search_service::*;
pub use service::*;
pub use tagging::*; 