use vectordb_core::repo_helpers;
use vectordb_core::qdrant_client_trait::QdrantClientTrait;

// Use config types from vectordb_core
use vectordb_core::AppConfig;

use anyhow::{anyhow, Context, Result, bail};
use clap::Args;
use std::sync::Arc;
use colored::*;
use std::fmt::Debug;

use qdrant_client::qdrant::{Filter, Condition, SearchResponse, QueryResponse};

use crate::{
    cli::{
        self as cli, // Alias cli for clarity
        commands::{FIELD_BRANCH, FIELD_LANGUAGE, FIELD_ELEMENT_TYPE}, // Import re-exported constants
        formatters::print_search_results, // Correct path for formatters
    },
};

// Core imports
use vectordb_core::embedding::EmbeddingHandler;
use vectordb_core::error::VectorDBError;
use vectordb_core::search_collection;

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

    let collection_name = repo_helpers::get_collection_name(&repo_name);

    let model_env_var = std::env::var("VECTORDB_ONNX_MODEL").ok();
    let tokenizer_env_var = std::env::var("VECTORDB_ONNX_TOKENIZER_DIR").ok();

    let _onnx_model_path_str = cli_args.onnx_model_path_arg.as_deref()
        .or(model_env_var.as_deref())
        .or(config.onnx_model_path.as_deref())
        .ok_or_else(|| anyhow!("ONNX model path not found. Provide via --onnx-model, env var, or config."))?;
    
    let _onnx_tokenizer_dir_str = cli_args.onnx_tokenizer_dir_arg.as_deref()
        .or(tokenizer_env_var.as_deref())
        .or(config.onnx_tokenizer_path.as_deref())
        .ok_or_else(|| anyhow!("ONNX tokenizer dir not found. Provide via --onnx-tokenizer-dir, env var, or config."))?;

    let embedding_handler = EmbeddingHandler::new(config)?;

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
    let search_response_result: Result<QueryResponse, VectorDBError> = search_collection(
        client.clone(),
        &collection_name,
        &embedding_handler,
        &args.query,
        args.limit,
        Some(search_filter),
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