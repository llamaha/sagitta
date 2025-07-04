pub mod qdrant_utils;
pub mod repo_indexing;
pub mod git_utils;
pub mod git_edge_cases;
pub mod recovery;
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