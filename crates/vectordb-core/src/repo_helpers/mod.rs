pub mod git_utils;
pub mod qdrant_utils;
pub mod repo_indexing;

pub use self::git_utils::{is_supported_extension, create_fetch_options, merge_local_branch, collect_files_from_tree, switch_repository_branch};
pub use self::qdrant_utils::{get_collection_name, delete_points_for_files, custom_upsert_batch, ensure_repository_collection_exists, create_branch_filter};
pub use self::repo_indexing::{update_sync_status_and_languages, index_files, prepare_repository, delete_repository_data, sync_repository_branch};
pub use crate::constants::DEFAULT_VECTOR_DIMENSION; 