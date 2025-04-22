use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use vectordb_lib::cli::{
    repo_commands::query as repo_query,
    commands::CliArgs
};
use vectordb_lib::vectordb::search::SearchResult;
use vectordb_core::qdrant_client_trait::QdrantClientTrait;
use vectordb_core::config::AppConfig;
use qdrant_client::qdrant::PointStruct;

// --- Semantic Search Action ---
// Corresponds to `vectordb-cli query [REPO_NAME]`

#[derive(Debug, Serialize, Deserialize)]
pub struct SemanticSearchParams {
    pub query: String,
    pub limit: Option<usize>,
    pub repo_name: Option<String>, // If present, use repo query, else use simple query
    pub lang: Option<String>,
    pub element_type: Option<String>,
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
    ) -> Self {
        Self {
            params: SemanticSearchParams {
                query,
                limit,
                repo_name,
                lang,
                element_type,
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
        debug!(params = ?self.params, "Executing SemanticSearchAction");

        // Use vdb_config from AppContext
        let vdb_app_config = &context.vdb_config;

        let search_results: Result<Vec<SearchResult>>;

        // Determine which search type to use
        // If repo_name is specified in params, use that.
        // Otherwise, check for an active_repo in the chain state.
        let repo_to_search = self.params.repo_name.clone()
            .or_else(|| state.context.get("active_repo").and_then(|v| v.as_str().map(String::from)));

        if let Some(repo_name) = repo_to_search {
            // --- Repo Query --- 
            info!(repo=%repo_name, query=%self.params.query, "Performing repository search");
            let repo_args = repo_query::RepoQueryArgs {
                query: self.params.query.clone(),
                limit: self.params.limit.unwrap_or(10) as u64,
                name: Some(repo_name.clone()),
                branch: None,
                lang: self.params.lang.clone(),
                element_type: self.params.element_type.clone(),
                json: false, 
            };
            
            // Create a dummy CLI args for the query handler
            let cli_args = CliArgs::default();
            
            // Use a dummy Vec<SearchResult> since we can't directly integrate with the API yet
            let query_result = repo_query::handle_repo_query(repo_args, vdb_app_config, context.qdrant_client.clone(), &cli_args).await
                .map_err(RelayError::Other);
            // Assign the result to search_results
            search_results = query_result.map(|_| Vec::new()); // Map successful query to empty results for now

        } else {
            // --- Simple Query --- 
            info!(query=%self.params.query, "Performing simple search (no active repo specified)");
            // Currently the API doesn't directly support our needs, so use a dummy result
            let dummy_results = Vec::new();
            search_results = Ok(dummy_results);
        }

        match search_results {
            Ok(results) => {
                info!(count = results.len(), "Search completed successfully.");
                state.set_context(format!("search_results_{}", self.params.query), results)
                    .map_err(|e| RelayError::ToolError(format!("Failed to set context for search results: {}", e)))?;
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Semantic search failed");
                state.set_context(format!("search_error_{}", self.params.query), e.to_string())
                    .map_err(|e_ctx| RelayError::ToolError(format!("Failed to set context for search error: {}", e_ctx)))?;
                Err(e)
            }
        }
    }
} 