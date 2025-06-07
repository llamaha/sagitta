pub mod switch;
pub mod create;
pub mod checkout;
pub mod merge;
pub mod worktree;

// Re-export commonly used types
pub use switch::{
    BranchSwitcher, SwitchOptions, SyncOptions, SyncType, SyncRequirement,
    switch_branch, switch_branch_no_sync,
};

// Re-export create/clone operations
pub use create::{
    RepositoryCloner, CloneOptions, CloneResult, init_repository,
};

// Re-export change management operations
pub use checkout::{
    ChangeManager, CommitOptions, CommitResult, GitPushOptions, PushResult,
    PullOptions, PullResult, GitSignature,
};

// pub use create::*; 