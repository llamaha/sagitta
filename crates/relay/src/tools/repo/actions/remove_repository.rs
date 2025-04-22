use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use std::io::{self, Write};
use std::sync::Arc;
use anyhow::Context as AnyhowContext;

// --- Add vectordb-core imports ---
use vectordb_core::config::{AppConfig as VectorDBAppConfig, RepositoryConfig, load_config, save_config, get_config_path};
use vectordb_core::repo_helpers::{delete_repository_data};
use vectordb_core::error::VectorDBError;
// --- End vectordb-core imports ---

// --- Remove Repository Action ---
// Calls vectordb_core::repo_helpers::delete_repository_data

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveRepositoryParams {
    pub name: String,
}

#[derive(Debug)]
pub struct RemoveRepositoryAction {
    params: RemoveRepositoryParams,
}

impl RemoveRepositoryAction {
    pub fn new(params: RemoveRepositoryParams) -> Self {
        Self { params }
    }
}

#[async_trait]
impl Action for RemoveRepositoryAction {
    fn name(&self) -> &'static str {
        "remove_repository"
    }

    async fn execute(&self, context: &AppContext, state: &mut ChainState) -> Result<()> {
        let repo_name = &self.params.name;
        debug!(name = %repo_name, "Preparing RemoveRepositoryAction using library function");

        // --- Confirmation Logic (Placeholder) ---
        // TODO: Confirmation should ideally happen *before* calling the action.
        // If confirmation is needed here, it requires a different mechanism
        // (e.g., returning a specific result type asking for confirmation).
        // For now, proceeding without explicit confirmation within the action.
        warn!("Executing remove_repository without explicit user confirmation within the action.");

        // --- Retrieve Dependencies ---
        let qdrant_client = Arc::clone(&context.qdrant_client);
        let config_path = get_config_path()
            .map_err(|e| RelayError::ToolError(format!("Failed to get config path: {}", e)))?;

        // --- Load Current Config ---
        // Must load the mutable config here to find the repo and later remove it
        let mut vdb_config = load_config(Some(&config_path))
            .map_err(|e| RelayError::ToolError(format!("Failed to load VectorDB config: {}", e)))?;

        // --- Find Repository Config ---
        let repo_config_to_remove = vdb_config.repositories.iter()
            .find(|r| r.name == *repo_name)
            .cloned() // Clone the config to use it after the borrow ends
            .ok_or_else(|| RelayError::ToolError(format!("Repository '{}' not found in configuration.", repo_name)))?;

        info!(name = %repo_name, "Found repository config. Proceeding with deletion.");

        // --- Call Library Function to Delete Data & Files ---
        match delete_repository_data(&repo_config_to_remove, Arc::clone(&qdrant_client)).await {
            Ok(_) => {
                info!(repo_name = %repo_name, "Successfully deleted repository data and local files via library function.");

                // --- Remove from Config & Save ---
                vdb_config.repositories.retain(|r| r.name != *repo_name);
                // Also update active repo in config if it was the one removed
                if vdb_config.active_repository.as_deref() == Some(repo_name) {
                    vdb_config.active_repository = None;
                    info!(repo_name = %repo_name, "Cleared active repository in config as it was removed.");
                }
                save_config(&vdb_config, Some(&config_path))
                    .map_err(|e| RelayError::ToolError(format!("Failed to save updated config after removing repository '{}': {}", repo_name, e)))?;

                info!(repo_name = %repo_name, "Successfully removed repository from config file.");

                // --- Update ChainState ---
                if state.active_repository.as_deref() == Some(repo_name) {
                    state.active_repository = None;
                    info!(repo_name = %repo_name, "Cleared active repository in chain state.");
                }

                // Set success context
                let success_msg = format!("Successfully removed repository '{}'.", repo_name);
                state.set_context("last_action_status".to_string(), "success".to_string())
                    .map_err(RelayError::SerializationError)?;
                state.set_context("last_action_message".to_string(), success_msg)
                    .map_err(RelayError::SerializationError)?;

                Ok(())
            },
            Err(e) => {
                error!(error = %e, repo_name = %repo_name, "delete_repository_data library function failed");
                let err_msg = format!("Failed to delete repository data for '{}': {}", repo_name, e);
                // Set error context
                state.set_context("last_action_status".to_string(), "error".to_string())
                     .map_err(RelayError::SerializationError)?;
                state.set_context("last_action_error".to_string(), err_msg.clone())
                     .map_err(RelayError::SerializationError)?;
                Err(RelayError::ToolError(err_msg))
            }
        }
    }
} 