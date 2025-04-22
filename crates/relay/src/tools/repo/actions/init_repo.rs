use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use git2::Repository;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, error, info};

// --- Init Repo Action ---
// Note: Uses git2 library to initialize a new repository.

#[derive(Debug, Serialize, Deserialize)]
pub struct InitRepoParams {
    pub path: String, // Directory to initialize
}

#[derive(Debug)]
pub struct InitRepoAction {
    params: InitRepoParams,
}

impl InitRepoAction {
    pub fn new(path: String) -> Self {
        Self { params: InitRepoParams { path } }
    }
}

#[async_trait]
impl Action for InitRepoAction {
    fn name(&self) -> &'static str {
        "init_repo"
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        let path_str = &self.params.path;
        let repo_path = Path::new(path_str);
        debug!(path = %path_str, "Executing InitRepoAction");

        // Ensure the directory exists first
        // Using tokio::fs for async file operations
        tokio::fs::create_dir_all(repo_path).await.map_err(|e| {
            RelayError::ToolError(format!("Failed to create directory '{}' for repo init: {}", path_str, e))
        })?;

        // Initialize the git repository
        match Repository::init(repo_path) {
            Ok(repo) => {
                info!(path = ?repo.path(), "Successfully initialized Git repository.");
                // Update state with the path
                state.current_directory = Some(path_str.to_string());
                state.set_context("repo_init_path".to_string(), path_str.to_string()) // Ensure value is owned String
                    .map_err(|e| RelayError::ToolError(format!("Failed to set context for repo init: {}", e)))?;
                Ok(())
            }
            Err(e) => {
                error!(path = %path_str, error = %e, "Failed to initialize Git repository");
                Err(RelayError::ToolError(format!("Failed to initialize git repository at '{}': {}", path_str, e)))
            }
        }
    }
} 