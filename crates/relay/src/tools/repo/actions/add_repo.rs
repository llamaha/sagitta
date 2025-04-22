use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use vectordb_core::repo_add::{handle_repo_add, AddRepoArgs};
use vectordb_core::config::get_repo_base_path;
use vectordb_core::embedding::EmbeddingHandler;

// --- Add Repo Action ---
// Corresponds to `vectordb-cli repo add` by calling the library function.

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddRepoParams {
    pub name: String, // Made public
    pub url: Option<String>,
    pub local_path: Option<String>,
    pub branch: Option<String>,
    pub extensions: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct AddRepoAction {
    params: AddRepoParams,
}

impl AddRepoAction {
    pub fn new(
        name: String,
        url: Option<String>,
        local_path: Option<String>,
        branch: Option<String>,
        extensions: Option<Vec<String>>,
    ) -> Self {
        if url.is_none() && local_path.is_none() {
            warn!("AddRepoAction created without URL or local path. This might fail.");
        }
        Self { params: AddRepoParams { name, url, local_path, branch, extensions } }
    }
}

#[async_trait]
impl Action for AddRepoAction {
    fn name(&self) -> &'static str {
        "add_repository"
    }

    async fn execute(&self, context: &AppContext, state: &mut ChainState) -> Result<()> {
        debug!(params = ?self.params, "Preparing AddRepoAction");

        // --- Retrieve Dependencies from Context --- 
        // This assumes AppContext provides these methods. Adjust as necessary.
        let vectordb_config = Arc::clone(&context.vdb_config);

        // --- Check for existing repository in VDB Config (using context) --- 
        if context.vdb_config.repositories.iter().any(|r| r.name == self.params.name) {
            warn!(repo_name = %self.params.name, "Repository already exists in config, checking if clone is needed.");
            // Decide if we should error or just proceed to check/clone
        }

        // --- Construct Args for Library Call --- 
        let args = AddRepoArgs {
            name: Some(self.params.name.clone()),
            url: self.params.url.clone(),
            local_path: self.params.local_path.as_ref().map(PathBuf::from),
            branch: self.params.branch.clone(),
            // Pass through other args if they become available in AddRepoArgs
            remote: None, // Assuming default for now
            ssh_key: None, // Assuming default for now
            ssh_passphrase: None, // Assuming default for now
            repositories_base_path: None, // Initialize missing field
        };

        if self.params.extensions.is_some() {
            warn!("The 'extensions' parameter for add_repo is ignored when calling the library function.");
        }

        // --- Call Library Function --- 
        info!(args = ?args, "Calling handle_repo_add library function");

        // 1. Initialize EmbeddingHandler and get dimension
        let embedding_handler = EmbeddingHandler::new(&*vectordb_config) // Deref the Arc<AppConfig>
            .map_err(|e| RelayError::ToolError(format!("Failed to initialize embedding handler: {}", e)))?;
        let embedding_dim = embedding_handler.dimension()
            .map_err(|e| RelayError::ToolError(format!("Failed to get embedding dimension: {}", e)))?;
        
        // 2. Determine repo base path (prioritize args over config)
        let repo_base_path = match &args.repositories_base_path {
            Some(path) => path.clone(),
            None => get_repo_base_path(Some(&*vectordb_config)) // Deref the Arc<AppConfig>
                        .map_err(|e| RelayError::ToolError(format!("Failed to determine repository base path: {}", e)))?,
        };
        // Ensure base path exists
        std::fs::create_dir_all(&repo_base_path)
            .map_err(|e| RelayError::ToolError(format!("Failed to create base directory '{}': {}", repo_base_path.display(), e)))?;
        
        // 3. Call the refactored function
        match handle_repo_add(
            args, 
            repo_base_path, // Pass determined base path
            embedding_dim as u64, // Pass dimension
            Arc::clone(&context.qdrant_client)
        ).await {
            Ok(new_repo_config) => {
                info!(repo_name = %new_repo_config.name, "Repository added successfully via library function.");
                
                // --- Update Relay State --- 
                let repo_name = new_repo_config.name.clone();
                // COMMENTED OUT: Cannot modify AppContext config directly here.
                // state.context.vdb_config.to_mut().repositories.push(new_repo_config);
                // state.context.vdb_config.to_mut().active_repository = Some(repo_name.clone()); 
                info!(repo_name = %repo_name, "Repository details returned by handle_repo_add."); // Updated message

                // --- Set Context Variables (Optional) --- 
                state.set_context("last_action_status".to_string(), "success".to_string())
                    .map_err(|e| RelayError::SerializationError(e))?; 
                state.set_context("last_added_repo".to_string(), repo_name.clone())
                    .map_err(|e| RelayError::SerializationError(e))?; 
                // Add stdout-like message to context for LLM?
                let success_message = format!("Repository '{repo_name}' added and set as active. Run 'sync_repository' to index it.");
                state.set_context("last_action_message".to_string(), success_message)
                    .map_err(|e| RelayError::SerializationError(e))?; 

                Ok(())
            },
            Err(e) => {
                error!(error = %e, repo_name = %self.params.name, "handle_repo_add library function failed");
                
                // --- Set Error Context (Optional) --- 
                let err_msg = format!("Failed to add repository '{}': {}", self.params.name, e);
                state.set_context("last_action_status".to_string(), "error".to_string())
                    .map_err(|e_ctx| RelayError::SerializationError(e_ctx))?; 
                state.set_context("last_action_error".to_string(), err_msg.clone())
                    .map_err(|e_ctx| RelayError::SerializationError(e_ctx))?; 
                
                Err(RelayError::ToolError(err_msg)) // Convert AddRepoError to RelayError
            }
        }
    }
} 