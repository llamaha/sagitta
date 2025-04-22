use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
// Import necessary types
use vectordb_core::get_managed_repos_from_config;
use vectordb_core::config::{AppConfig, RepositoryConfig};
use anyhow::{Result as AnyhowResult};
use serde_json::json;
use std::sync::{Arc, Mutex};
use serde_json::Value;

// --- List Repositories Action ---
// Gets the list of repositories directly from the vectordb-lib config.

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ListRepositoriesParams {}

#[derive(Debug)]
pub struct ListRepositoriesAction {
    _params: ListRepositoriesParams,
}

impl ListRepositoriesAction {
    pub fn new(_params: ListRepositoriesParams) -> Self {
        Self { _params }
    }
}

#[async_trait]
impl Action for ListRepositoriesAction {
    fn name(&self) -> &'static str {
        "list_repositories"
    }

    async fn execute(&self, context: &AppContext, state: &mut ChainState) -> Result<()> {
        debug!("Executing ListRepositoriesAction using library function");

        // Access the AppConfig from the context
        let vdb_config = &context.vdb_config;

        // Call the library function to get structured data
        // This function is synchronous, so no .await needed.
        let managed_repos = get_managed_repos_from_config(vdb_config);

        // Extract names for the context list
        let repo_names: Vec<String> = managed_repos.repositories.iter().map(|r| r.name.clone()).collect();

        // Update ChainState
        state.active_repository = managed_repos.active_repository.clone();
        info!(active_repository = ?managed_repos.active_repository, "Updated active repository from config.");

        // Store the parsed list
        state.set_context("repository_list".to_string(), repo_names.clone())
             .map_err(|e| RelayError::ToolError(format!("Failed to set context for repository list: {}", e)))?;
        info!(repo_list = ?repo_names, "Set repository_list context from library data");

        // Optionally store the full structure if needed elsewhere, but the list and active name are key for format_action_result
        // state.set_context("managed_repositories_result".to_string(), managed_repos)
        //     .map_err(|e| RelayError::ToolError(format!("Failed to set context for managed_repositories_result: {}", e)))?;

        Ok(())
        // Note: No error handling needed here unless get_managed_repos_from_config could fail,
        // but its signature indicates it cannot (returns ManagedRepositories directly).
    }
} 