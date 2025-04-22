use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use vectordb_core::search_semantic;
use qdrant_client::qdrant::{Filter, Condition, r#match::MatchValue};
use std::sync::Arc;

// --- Semantic Search Action ---
// Corresponds to `vectordb-cli query [REPO_NAME]`

#[derive(Debug, Serialize, Deserialize)]
pub struct SemanticSearchParams {
    pub query: String,
    pub limit: Option<usize>,
    pub repo_name: Option<String>, // If present, use repo query, else use simple query
    pub lang: Option<String>,
    pub element_type: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug)]
pub struct SemanticSearchAction {
    params: SemanticSearchParams,
}

impl SemanticSearchAction {
    pub fn new(
        query: String,
        limit: Option<usize>,
        repo_name: Option<String>,
        lang: Option<String>,
        element_type: Option<String>,
        branch: Option<String>,
    ) -> Self {
        Self {
            params: SemanticSearchParams {
                query,
                limit,
                repo_name,
                lang,
                element_type,
                branch,
            },
        }
    }
}

#[async_trait]
impl Action for SemanticSearchAction {
    fn name(&self) -> &'static str {
        "semantic_search"
    }

    async fn execute(&self, context: &AppContext, state: &mut ChainState) -> Result<()> {
        debug!(params = ?self.params, "Preparing SemanticSearchAction");

        // --- Retrieve Dependencies from Context --- 
        let qdrant_client = Arc::clone(&context.qdrant_client);
        let vdb_app_config = Arc::clone(&context.vdb_config);

        // --- Determine Active Repository --- 
        let active_repo_name = match self.params.repo_name.as_ref().or(state.active_repository.as_ref()) {
            Some(name) => name.clone(),
            None => {
                let err_msg = "No active repository set and no repository specified in parameters.".to_string();
                error!(error = err_msg, "Cannot perform semantic search");
                return Err(RelayError::ToolError(err_msg));
            }
        };

        // Find the config for the active repository
        let repo_config = vdb_app_config.repositories.iter()
            .find(|r| r.name == active_repo_name)
            .ok_or_else(|| {
                let err_msg = format!("Configuration for repository '{}' not found.", active_repo_name);
                error!(error = err_msg);
                RelayError::ToolError(err_msg)
            })?;
        
        // --- Construct Filter (if branch specified) --- 
        let mut filter: Option<Filter> = None;
        // Note: Original handle_repo_query checked repo_config.active_branch. We should mimic that logic?
        // Or should the filter only apply if explicitly passed in params?
        // For now, only filter if params.branch is set.
        if let Some(branch_name) = &self.params.branch {
            debug!(branch = %branch_name, repo = %active_repo_name, "Creating branch filter");
            let condition = Condition::matches(
                vectordb_core::constants::FIELD_BRANCH.to_string(),
                MatchValue::Keyword(branch_name.clone()),
            );
            filter = Some(Filter {
                must: vec![condition],
                ..Default::default()
            });
        }

        // --- Call Library Search Function --- 
        info!(query = %self.params.query, repo = %active_repo_name, branch = ?self.params.branch, "Calling search_semantic library function");

        match search_semantic(
            &self.params.query,
            self.params.limit.unwrap_or(10) as usize, // Use param limit or default
            filter, // Pass the constructed filter
            &active_repo_name, // Pass repo name for collection name
            &vdb_app_config, // Pass full config for embedding handler
            qdrant_client, // Pass Qdrant client
        ).await {
            Ok(results) => {
                info!(count = results.len(), "Semantic search completed successfully.");
                let result_value = serde_json::to_value(&results)
                    .map_err(|e| RelayError::SerializationError(e))?;
                
                state.set_context("last_action_status".to_string(), "success".to_string())
                    .map_err(RelayError::SerializationError)?;
                state.set_context("search_results".to_string(), result_value)
                    .map_err(RelayError::SerializationError)?; 
                state.set_context("last_action_message".to_string(), format!("Found {} results.", results.len()))
                    .map_err(RelayError::SerializationError)?; 
                Ok(())
            },
            Err(e) => {
                error!(error = %e, query = %self.params.query, repo = %active_repo_name, "search_semantic failed");
                let err_msg = format!("Semantic search failed for query '{}' in repository '{}': {}", self.params.query, active_repo_name, e);
                state.set_context("last_action_status".to_string(), "error".to_string())
                    .map_err(RelayError::SerializationError)?;
                state.set_context("last_action_error".to_string(), err_msg.clone())
                    .map_err(RelayError::SerializationError)?;
                Err(RelayError::ToolError(err_msg))
            }
        }
    }
} 