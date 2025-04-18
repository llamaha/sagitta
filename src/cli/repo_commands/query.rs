use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::sync::Arc;
use colored::*;
use std::fmt::Debug;

use qdrant_client::qdrant::{Filter, Condition, SearchPointsBuilder};

use crate::{
    config::AppConfig,
    cli::repo_commands::helpers,
    cli::formatters::print_search_results,
    vectordb::embedding_logic::EmbeddingHandler,
    cli::commands::{FIELD_BRANCH, FIELD_LANGUAGE, FIELD_ELEMENT_TYPE},
    vectordb::qdrant_client_trait::QdrantClientTrait,
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
    let repo_name = args.name.as_ref().or(config.active_repository.as_ref())
        .ok_or_else(|| anyhow!("No active repository set and no repository name provided with --name."))?;

    let repo_config = config.repositories.iter()
        .find(|r| &r.name == repo_name)
        .ok_or_else(|| anyhow!("Configuration for repository '{}' not found.", repo_name))?;

    let collection_name = helpers::get_collection_name(repo_name);
    println!("Querying repository '{}' in collection '{}'...", repo_name.cyan(), collection_name.cyan());

    // Determine the branch to filter by
    let branch_filter = args.branch.as_ref().or(repo_config.active_branch.as_ref());
    if let Some(branch) = branch_filter {
        println!("Filtering by branch: {}", branch.yellow());
    } else {
         println!("{}", "Warning: No branch specified and repository has no active branch. Querying across all branches.".yellow());
    }

    // --- Embedding Setup ---
    let model_env_var = std::env::var("VECTORDB_ONNX_MODEL").ok();
    let tokenizer_env_var = std::env::var("VECTORDB_ONNX_TOKENIZER_DIR").ok();

    let onnx_model_path_str = cli_args.onnx_model_path_arg.as_ref()
        .or(model_env_var.as_ref())
        .or(config.onnx_model_path.as_ref())
        .ok_or_else(|| anyhow!("ONNX model path must be provided via --onnx-model, VECTORDB_ONNX_MODEL, or config"))?;
    let onnx_tokenizer_dir_str = cli_args.onnx_tokenizer_dir_arg.as_ref()
        .or(tokenizer_env_var.as_ref())
        .or(config.onnx_tokenizer_path.as_ref())
        .ok_or_else(|| anyhow!("ONNX tokenizer path must be provided via --onnx-tokenizer-dir, VECTORDB_ONNX_TOKENIZER_DIR, or config"))?;
    
    let embedding_handler = Arc::new(
        EmbeddingHandler::new(
            crate::vectordb::embedding::EmbeddingModelType::Onnx,
            Some(onnx_model_path_str.into()),
            Some(onnx_tokenizer_dir_str.into()),
        )
        .context("Failed to initialize embedding handler for query")?,
    );
    let embedding_dim = embedding_handler.dimension()?;
    
    // --- Create Query Vector ---
    let query_vector = embedding_handler.embed(&[&args.query])?.remove(0);
    if query_vector.len() != embedding_dim {
         return Err(anyhow!(
             "Query embedding dimension ({}) does not match model dimension ({}).", 
             query_vector.len(), embedding_dim
         ));
    }
    log::debug!("Query vector created with dimension: {}", query_vector.len());


    // --- Construct Qdrant Filter ---
    let mut filter_conditions = vec![
        // Filter by repository name (should always be present in repo collections)
        // Condition::matches(helpers::FIELD_REPO_NAME, repo_name.clone()),
        // Repo name is implicit in the collection name now
    ];

    if let Some(branch) = branch_filter {
        filter_conditions.push(Condition::matches(FIELD_BRANCH, branch.clone()));
    }
    
    // Add lang filter if provided
    if let Some(lang) = &args.lang {
        println!("Filtering by language: {}", lang.yellow());
        filter_conditions.push(Condition::matches(FIELD_LANGUAGE, lang.clone()));
    }

    // Add type filter if provided
    if let Some(el_type) = &args.element_type {
        println!("Filtering by element type: {}", el_type.yellow());
        filter_conditions.push(Condition::matches(FIELD_ELEMENT_TYPE, el_type.clone()));
    }

    let query_filter = Filter {
        must: filter_conditions,
        ..Default::default() // Use default for should, must_not, min_should
    };

    // --- Perform Search ---
    let search_request = SearchPointsBuilder::new(&collection_name, query_vector, args.limit)
        .filter(query_filter)
        .with_payload(true) // Request payload to display results
        .with_vectors(false); // Usually don't need vectors in results

    println!("Performing search...");
    let search_result = client.search_points(search_request.into())
        .await
        .context(format!("Failed to search points in collection '{}'", collection_name))?;

    println!("Search returned {} results.", search_result.result.len());

    if search_result.result.is_empty() {
        println!("{}", "No matching results found.".yellow());
        return Ok(());
    }
    
    // Removed call to print_summary_stats
    // print_summary_stats(&search_result);
    // Call the renamed formatter function
    print_search_results(&search_result.result, &args.query)?;

    Ok(())
} 