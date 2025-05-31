/// Utilities for Git operations.
pub mod git_utils;
/// Utilities for interacting with Qdrant.
pub mod qdrant_utils;
/// Functions related to repository indexing logic.
pub mod repo_indexing;

pub use self::git_utils::{is_supported_extension, create_fetch_options, collect_files_from_tree};
pub use self::qdrant_utils::{
    get_collection_name, 
    get_branch_aware_collection_name,
    collection_exists_for_branch,
    get_branch_sync_metadata,
    should_sync_branch,
    BranchSyncMetadata,
    delete_points_for_files, 
    ensure_repository_collection_exists, 
    create_branch_filter
};
pub use self::repo_indexing::{update_sync_status_and_languages, index_files, prepare_repository, delete_repository_data}; 