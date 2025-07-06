//! Repository helper modules providing utilities for git operations, Qdrant interactions, and indexing.

/// Utilities for working with Qdrant collections and operations.
pub mod qdrant_utils;
/// Functions for indexing repository files and managing sync operations.
pub mod repo_indexing;
/// Git repository utilities for file collection and fetch operations.
pub mod git_utils;
/// Edge case handling for git operations including ref resolution and validation.
pub mod git_edge_cases;
/// Recovery mechanisms for handling corrupted or interrupted operations.
pub mod recovery;
/// Collection validation utilities for ensuring data integrity.
pub mod collection_validation;

// Re-export commonly used functions
pub use qdrant_utils::{
    get_collection_name,
    get_branch_aware_collection_name,
    collection_exists_for_branch,
    delete_points_for_files,
    get_branch_sync_metadata,
    should_sync_branch,
    create_branch_filter,
    BranchSyncMetadata,
};

pub use repo_indexing::{
    prepare_repository,
    index_files,
    delete_repository_data,
    sync_repository_branch,
    update_sync_status_and_languages,
    IndexFilesParams,
    PrepareRepositoryParams,
};

pub use git_utils::{
    collect_files_from_tree,
    create_fetch_options,
};

pub use git_edge_cases::{
    resolve_git_ref,
    validate_ref_name,
    detect_default_branch,
    check_working_tree_clean,
    get_current_branch,
};