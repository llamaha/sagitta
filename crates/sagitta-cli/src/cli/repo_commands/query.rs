use clap::Args;
use anyhow::{anyhow, Result, bail};
use std::sync::Arc;
use sagitta_search::{
    config::AppConfig,
    qdrant_client_trait::QdrantClientTrait,
    repo_helpers::get_branch_aware_collection_name,
    error::SagittaError,
    search_impl::search_collection,
    EmbeddingPool,
    app_config_to_embedding_config,
    constants::{FIELD_BRANCH, FIELD_LANGUAGE, FIELD_ELEMENT_TYPE},
};
use qdrant_client::qdrant::{Filter, QueryResponse, Condition};

// Use config types from sagitta_search

use colored::*;
use std::fmt::Debug;

use crate::cli::formatters::print_search_results;

#[derive(Args, Debug, Clone)]
pub struct RepoQueryArgs {
    /// The search query string.
    #[arg(required = true)]
    pub query: String,

    /// Optional: Name of the repository to query (defaults to active).
    #[arg(short, long)]
    pub name: Option<String>,

    /// Optional: Filter by specific branch (defaults to active branch if repo is active).
    #[arg(short, long)]
    pub branch: Option<String>,

    /// Maximum number of results to return.
    #[arg(short, long, default_value_t = 10)]
    pub limit: u64,

    /// Optional: Filter by specific language (e.g., "rust", "python").
    #[arg(long)]
    pub lang: Option<String>,

    /// Optional: Filter by specific code element type (e.g., "function", "struct", "impl").
    #[arg(long = "type")]
    pub element_type: Option<String>,

    /// Output results in JSON format.
    #[arg(long)]
    pub json: bool,
}

pub async fn handle_repo_query<C>(
    args: RepoQueryArgs,
    config: &AppConfig,
    client: Arc<C>,
    cli_args: &crate::cli::CliArgs,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let repo_name = match args.name.as_ref().or(config.active_repository.as_ref()) {
        Some(name) => name.clone(),
        None => bail!("No repository specified and no active repository set. Use 'repo use <name>' or provide --name."),
    };

    let repo_config = config.repositories.iter()
        .find(|r| r.name == repo_name)
        .ok_or_else(|| anyhow!("Configuration for repository '{}' not found.", repo_name))?;

    let branch_name = args.branch.clone()
        .or_else(|| repo_config.active_branch.clone())
        .unwrap_or_else(|| repo_config.default_branch.clone());

    let collection_name = get_branch_aware_collection_name(&repo_name, &branch_name, config);

    // Check if embedding is configured (either via embed_model or ONNX paths)
    let has_embed_model = config.embed_model.is_some();
    let has_onnx_paths = config.onnx_model_path.is_some() && config.onnx_tokenizer_path.is_some();
    let has_cli_onnx = cli_args.onnx_model_path_arg.is_some() && cli_args.onnx_tokenizer_dir_arg.is_some();
    let has_env_onnx = std::env::var("SAGITTA_ONNX_MODEL").is_ok() && std::env::var("SAGITTA_ONNX_TOKENIZER_DIR").is_ok();
    
    if !has_embed_model && !has_onnx_paths && !has_cli_onnx && !has_env_onnx {
        return Err(anyhow!("Embedding model not configured. Provide 'embed_model' in config, or ONNX paths via config/CLI/env vars."));
    }

    let embedding_config = app_config_to_embedding_config(config);
    let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config)?;

    println!(
        "Querying repository '{}' (collection: '{}', branch: '{}')...",
        repo_name.cyan(),
        collection_name.cyan(),
        branch_name.cyan()
    );

    let mut filter_conditions = vec![Condition::matches(FIELD_BRANCH, branch_name)];
    if let Some(lang) = &args.lang {
        filter_conditions.push(Condition::matches(FIELD_LANGUAGE, lang.clone()));
    }
    if let Some(element_type) = &args.element_type {
        filter_conditions.push(Condition::matches(FIELD_ELEMENT_TYPE, element_type.clone()));
    }
    let search_filter = Filter::must(filter_conditions);

    log::debug!("Calling core search_collection...");
    let search_response_result: Result<QueryResponse, SagittaError> = search_collection(
        client,
        &collection_name,
        &embedding_pool,
        &args.query,
        args.limit,
        Some(search_filter),
        config,
        None,
    ).await;

    let search_response = match search_response_result {
        Ok(resp) => resp,
        Err(e) => {
            return Err(anyhow!(e).context("Search operation failed"));
        }
    };

    print_search_results(&search_response.result, &args.query, args.json)?;

    Ok(())
} 