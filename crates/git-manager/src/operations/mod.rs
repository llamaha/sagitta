pub mod switch;
pub mod create;
pub mod merge;
pub mod checkout;
pub mod worktree;

// Re-export commonly used types
pub use switch::{
    BranchSwitcher, SwitchOptions, SyncOptions, SyncType, SyncRequirement,
    switch_branch, switch_branch_no_sync,
};
// pub use create::*; 