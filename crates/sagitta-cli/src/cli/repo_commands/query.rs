use clap::Args;
use anyhow::{anyhow, Context, Result, bail};
use std::sync::Arc;
use sagitta_search::{
    config::AppConfig,
    qdrant_client_trait::QdrantClientTrait,
    repo_helpers::{get_collection_name, get_branch_aware_collection_name},
    error::SagittaError,
    search_impl::search_collection,
    EmbeddingPool, EmbeddingProcessor,
    app_config_to_embedding_config,
    constants::{FIELD_BRANCH, FIELD_LANGUAGE, FIELD_ELEMENT_TYPE},
};
use qdrant_client::qdrant::{Filter, QueryResponse, Condition};

// Use config types from sagitta_search

use colored::*;
use std::fmt::Debug;

use crate::{
    cli::commands::{
        LEGACY_INDEX_COLLECTION,
    },
    cli::repo_commands::RepoCommand,
    cli::formatters::print_search_results,
};

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
    let cli_tenant_id = match cli_args.tenant_id.as_deref() {
        Some(id) => id,
        None => {
            bail!("--tenant-id is required to query a repository.");
        }
    };

    let repo_name = match args.name.as_ref().or(config.active_repository.as_ref()) {
        Some(name) => name.clone(),
        None => bail!("No repository specified and no active repository set. Use 'repo use <name>' or provide --name."),
    };

    let repo_config = config.repositories.iter()
        .find(|r| r.name == repo_name && r.tenant_id.as_deref() == Some(cli_tenant_id))
        .ok_or_else(|| anyhow!("Configuration for repository '{}' under tenant '{}' not found.", repo_name, cli_tenant_id))?;

    let branch_name = args.branch.clone()
        .or_else(|| repo_config.active_branch.clone())
        .unwrap_or_else(|| repo_config.default_branch.clone());

    let collection_name = get_branch_aware_collection_name(cli_tenant_id, &repo_name, &branch_name, config);

    let model_env_var = std::env::var("SAGITTA_ONNX_MODEL").ok();
    let tokenizer_env_var = std::env::var("SAGITTA_ONNX_TOKENIZER_DIR").ok();

    let _onnx_model_path_str = cli_args.onnx_model_path_arg.as_deref()
        .or(model_env_var.as_deref())
        .or(config.onnx_model_path.as_deref())
        .ok_or_else(|| anyhow!("ONNX model path not found. Provide via --onnx-model, env var, or config."))?;
    
    let _onnx_tokenizer_dir_str = cli_args.onnx_tokenizer_dir_arg.as_deref()
        .or(tokenizer_env_var.as_deref())
        .or(config.onnx_tokenizer_path.as_deref())
        .ok_or_else(|| anyhow!("ONNX tokenizer dir not found. Provide via --onnx-tokenizer-dir, env var, or config."))?;

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