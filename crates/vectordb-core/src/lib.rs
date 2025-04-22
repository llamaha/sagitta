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

// Moved modules needed internally by repo_add/repo_helpers (will move later)
// For now, vectordb-core needs access to these from the main crate/other deps
// This highlights the need to move these modules eventually
// pub mod qdrant_client_trait;
// pub mod embedding_logic;

// Re-export key items needed externally
pub use config::{AppConfig, RepositoryConfig, load_config, save_config, get_repo_base_path}; // Added get_repo_base_path
pub use error::VectorDBError;
pub use repo_add::{handle_repo_add, AddRepoArgs, AddRepoError};
pub use repo_helpers::{get_collection_name, ensure_repository_collection_exists}; // Re-export helpers
// pub use qdrant_client_trait::QdrantClientTrait; // Re-export trait when moved

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
