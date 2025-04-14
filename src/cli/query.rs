use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use futures::future::join_all;
use qdrant_client::{
    qdrant::{SearchPointsBuilder, Filter, Condition, PointStruct, ScoredPoint},
    Qdrant,
};
use std::{
    sync::Arc,
    path::PathBuf,
};

use crate::{
    config::AppConfig,
    vectordb::{embedding, embedding_logic::EmbeddingHandler},
    cli::repo_commands::get_collection_name,
    cli::formatters::print_search_results,
    cli::commands::{FIELD_LANGUAGE, FIELD_ELEMENT_TYPE, FIELD_BRANCH},
};
use super::commands::CliArgs;


#[derive(Args, Debug)]
pub struct QueryArgs {
    /// The search query string
    #[arg(required = true)]
    pub query: String,

    /// Maximum number of results to return
    #[arg(short, long, default_value_t = 10)]
    pub limit: u64,

    /// Optional repository name(s) to search within. Can be specified multiple times.
    /// If omitted, searches the active repository.
    /// Conflicts with --all-repos.
    #[arg(short, long, conflicts_with = "all_repos")]
    pub repo: Option<Vec<String>>,

    /// Search across all configured repositories.
    /// Conflicts with --repo.
    #[arg(long)]
    pub all_repos: bool,

    /// Optional branch name to filter results within the specified repository/repositories.
    #[arg(short, long)]
    pub branch: Option<String>,

    /// Optional: Filter by specific language (e.g., "rust", "python")
    #[arg(long)]
    pub lang: Option<String>,

    /// Optional: Filter by specific code element type (e.g., "function", "struct", "impl")
    #[arg(long = "type")]
    pub element_type: Option<String>,

    // Removed context arg as we show the full chunk now
    // /// Context lines before and after the matched line in the snippet
    // #[arg(long, default_value_t = 2)]
    // pub context: usize,
}

/// Handles the `query` command.
pub async fn handle_query(
    args: &QueryArgs,
    cli_args: &CliArgs,
    config: AppConfig,
    client: Arc<Qdrant>,
) -> Result<()> {
    log::info!("Starting query process...");

    // --- 1. Determine Target Repositories/Collections --- 
    let target_repos: Vec<String> = match (&args.repo, args.all_repos) {
        (Some(repo_names), _) => { // Specific repos requested
            // Validate that requested repos exist in config
            for name in repo_names {
                if !config.repositories.iter().any(|r| r.name == *name) {
                    bail!("Repository '{}' not found in configuration.", name);
                }
            }
            repo_names.clone()
        }
        (None, true) => { // All repos requested
            config.repositories.iter().map(|r| r.name.clone()).collect()
        }
        (None, false) => { // Default: use active repo
            vec![config.active_repository.clone().ok_or_else(|| {
                 anyhow!("No active repository set and no specific repository requested via --repo or --all-repos. Use 'repo use <name>' first.")
             })?]
        }
    };

    if target_repos.is_empty() {
        println!("No repositories configured or specified to search.");
        return Ok(());
    }

    log::info!("Target repositories: {:?}", target_repos);
    let collection_names: Vec<String> = target_repos.iter().map(|name| get_collection_name(name)).collect();

    // --- 2. Initialize Embedding Handler (Same as before) --- 
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
    let onnx_model_path = PathBuf::from(onnx_model_path_str);
    let onnx_tokenizer_path = PathBuf::from(onnx_tokenizer_dir_str);
    let embedding_handler = EmbeddingHandler::new(
        embedding::EmbeddingModelType::Onnx,
        Some(onnx_model_path),
        Some(onnx_tokenizer_path),
    )
    .context("Failed to initialize embedding handler")?;

    // --- 3. Generate Query Embedding --- 
    let query_embedding = embedding_handler.create_embedding_model()?
        .embed(&args.query)?;
    log::info!("Query embedding generated.");

    // --- 4. Build Search Filter --- 
    let mut filter_conditions = Vec::new();
    if let Some(branch_name) = &args.branch {
        // Add branch condition only if querying repo collections (implied by target_repos having entries)
        if !target_repos.is_empty() {
            filter_conditions.push(Condition::matches(FIELD_BRANCH, branch_name.clone()));
            log::info!("Filtering by branch: {}", branch_name);
        } else {
             log::warn!("Branch filter specified but no repository target found (this shouldn't happen). Ignoring filter.");
        }
    }
    if let Some(lang_name) = &args.lang {
        filter_conditions.push(Condition::matches(FIELD_LANGUAGE, lang_name.clone()));
        log::info!("Filtering by language: {}", lang_name);
    }
    if let Some(element_type) = &args.element_type {
        filter_conditions.push(Condition::matches(FIELD_ELEMENT_TYPE, element_type.clone()));
        log::info!("Filtering by element type: {}", element_type);
    }
    let search_filter = if filter_conditions.is_empty() { None } else { Some(Filter::must(filter_conditions)) };

    // --- 5. Execute Searches in Parallel --- 
    log::info!("Executing search against collections: {:?}...", collection_names);
    let search_futures: Vec<_> = collection_names.into_iter().map(|collection_name| {
        let client = Arc::clone(&client);
        let query_embedding_clone = query_embedding.clone();
        let search_filter_clone = search_filter.clone();
        let limit = args.limit;
        
        tokio::spawn(async move {
            let mut builder = SearchPointsBuilder::new(&collection_name, query_embedding_clone, limit)
                .with_payload(true);
            if let Some(filter) = search_filter_clone {
                 builder = builder.filter(filter);
            }
            let search_request = builder.build();
            client.search_points(search_request).await
        })
    }).collect();

    let search_results = join_all(search_futures).await;

    // --- 6. Aggregate and Sort Results --- 
    let mut all_scored_points = Vec::new();
    let mut errors = Vec::new();

    for (i, result) in search_results.into_iter().enumerate() {
        match result {
            Ok(Ok(search_response)) => {
                log::debug!("Search returned {} results from collection {}", search_response.result.len(), target_repos[i]);
                all_scored_points.extend(search_response.result);
            }
            Ok(Err(e)) => {
                let err_msg = format!("Qdrant search failed for repo '{}': {}", target_repos[i], e);
                log::error!("{}", err_msg);
                errors.push(err_msg);
            }
            Err(e) => { // JoinError
                let err_msg = format!("Task panicked for repo '{}': {}", target_repos[i], e);
                log::error!("{}", err_msg);
                errors.push(err_msg);
            }
        }
    }

    if !errors.is_empty() {
         eprintln!("Warning: Some searches failed:\n - {}", errors.join("\n - "));
    }

    // Sort by score (descending)
    all_scored_points.sort_unstable_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Limit results
    all_scored_points.truncate(args.limit as usize);

    log::info!("Total unique results after aggregation: {}", all_scored_points.len());

    // --- 7. Format and Print Results --- 
    print_search_results(&all_scored_points, &args.query)?;

    Ok(())
}