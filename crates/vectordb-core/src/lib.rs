// crates/vectordb-core/src/lib.rs

// Declare modules moved from main crate
pub mod config;
pub mod error;
pub mod repo_add;
pub mod repo_helpers;
pub mod constants; // Declare the new constants module
pub mod qdrant_client_trait; // Declare the new trait module
pub mod embedding; // Declare the new embedding module
pub mod syntax; // Declare the new syntax module
pub mod git_helpers; // Declare the new git helpers module

// Declare modules needed for search and edit actions
// pub mod search; // Keep commented until code is moved
pub mod search; // Declare moved module
pub mod edit;

// Moved modules needed internally by repo_add/repo_helpers (will move later)
// For now, vectordb-core needs access to these from the main crate/other deps
// This highlights the need to move these modules eventually
// pub mod qdrant_client_trait;
// pub mod embedding_logic;

// Re-export key items needed externally
pub use config::{AppConfig, RepositoryConfig, load_config, save_config, get_repo_base_path, get_config_path, ManagedRepositories, get_managed_repos_from_config};
pub use error::VectorDBError;
pub use repo_add::{handle_repo_add, AddRepoArgs, AddRepoError};
pub use repo_helpers::{
    delete_repository_data, // For remove_repository action
    sync_repository_branch, // For sync_repository action
    switch_repository_branch, // For use_repository action
    // get_managed_repos_from_config, // Now exported via config
    get_collection_name, 
    ensure_repository_collection_exists
}; // Re-export helpers
// pub use search::{search_semantic, SearchResult}; // Keep commented until code is moved
pub use search::{search_semantic, SearchResult}; // Export moved search items
pub use edit::{
    apply_edit, 
    validate_edit, 
    EditTarget, 
    EngineEditOptions, 
    EngineValidationIssue,
    EngineValidationSeverity
};
pub use qdrant_client_trait::QdrantClientTrait; // Re-export trait

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
