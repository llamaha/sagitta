// Add the index module
pub mod index;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use qdrant_client::{
    qdrant::{ 
        CreateCollectionBuilder, Distance, FieldType, VectorParamsBuilder, PointStruct, 
        SearchPointsBuilder, Condition, Filter, PointsSelector, 
        UpdateStatus, DeletePoints, 
    },
    Payload,
    Qdrant,
};
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    collections::HashSet,
    path::PathBuf, 
    sync::Arc, 
    time::Duration,
};
use uuid::Uuid;
use walkdir::WalkDir;

use vectordb_core::config::AppConfig;
use vectordb_core::qdrant_client_trait::QdrantClientTrait;
use crate::cli::CliArgs;
use vectordb_core::syntax;
use crate::vectordb::embedding_logic::EmbeddingHandler;
use crate::cli::commands::{
    upsert_batch, BATCH_SIZE, LEGACY_INDEX_COLLECTION, // Import constants
    FIELD_CHUNK_CONTENT, FIELD_ELEMENT_TYPE, FIELD_END_LINE, FIELD_FILE_EXTENSION,
    FIELD_FILE_PATH, FIELD_LANGUAGE, FIELD_START_LINE, ensure_payload_index,
};

// Arguments for the main 'simple' command group
#[derive(Args, Debug, Clone)] 
pub struct SimpleArgs {
    #[command(subcommand)]
    pub command: SimpleCommand,
}

// Implement Default for SimpleArgs
impl Default for SimpleArgs {
    fn default() -> Self {
        Self {
            command: SimpleCommand::default(),
        }
    }
}

// Enum for the subcommands within 'simple'
#[derive(Subcommand, Debug, Clone)] 
pub enum SimpleCommand {
    /// Simple index of files under path. Uses a default collection.
    Index(SimpleIndexArgs),
    /// Query the default collection used by 'simple index'.
    Query(SimpleQueryArgs),
    /// Clear the default collection used by 'simple index'.
    Clear(SimpleClearArgs),
}

// Implement Default for SimpleCommand
impl Default for SimpleCommand {
    fn default() -> Self {
        Self::Query(SimpleQueryArgs {
            query: String::new(),
            limit: 10,
            lang: None,
            element_type: None,
            json: false,
        })
    }
}

// Args structs for subcommands
#[derive(Args, Debug, Clone)] 
pub struct SimpleIndexArgs {
    /// One or more directories or files to index into the default collection.
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,

    /// Optional file extensions to include (e.g., ".rs", ".py"). If omitted, all files are attempted.
    #[arg(short = 'e', long = "extension")]
    pub file_extensions: Option<Vec<String>>,
}

#[derive(Args, Debug, Clone)]
pub struct SimpleQueryArgs {
    /// The search query string for the default collection.
    #[arg(required = true)]
    pub query: String,

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

#[derive(Args, Debug, Clone)] 
pub struct SimpleClearArgs {
    // No args needed
}


// --- Main Handler for 'simple' commands ---

pub async fn handle_simple_command(
    args: SimpleArgs,
    cli_args: &CliArgs,
    config: AppConfig,
    client: Arc<Qdrant>,
) -> Result<()> {
    match args.command {
        SimpleCommand::Index(index_args) => handle_simple_index(&index_args, cli_args, &config, client).await,
        SimpleCommand::Query(query_args) => handle_simple_query(&query_args, cli_args, &config, client).await,
        SimpleCommand::Clear(clear_args) => handle_simple_clear(&clear_args, &config, client).await,
    }
}

// --- Simple Index Handler ---

async fn handle_simple_index(
    cmd_args: &SimpleIndexArgs,
    cli_args: &CliArgs,
    config: &AppConfig,
    client: Arc<Qdrant>,
) -> Result<()> {
    log::info!("Starting simple indexing process...");

    let collection_name = LEGACY_INDEX_COLLECTION;
    log::info!("Indexing into default collection: '{}'", collection_name);

    for path in &cmd_args.paths {
        if !path.exists() {
             bail!("Input path does not exist: {}", path.display());
        }
    }
    log::info!("Processing input paths: {:?}", cmd_args.paths);

    // --- Enforce Config-Only ONNX Paths for Simple Index ---
    if cli_args.onnx_model_path_arg.is_some() || std::env::var("VECTORDB_ONNX_MODEL").is_ok() {
        return Err(anyhow!("For 'simple index', ONNX model path must be provided solely via the configuration file, not CLI arguments or environment variables."));
    }
    if cli_args.onnx_tokenizer_dir_arg.is_some() || std::env::var("VECTORDB_ONNX_TOKENIZER_DIR").is_ok() {
         return Err(anyhow!("For 'simple index', ONNX tokenizer path must be provided solely via the configuration file, not CLI arguments or environment variables."));
    }

    let onnx_model_path_str = config.onnx_model_path.as_ref()
        .ok_or_else(|| anyhow!("ONNX model path must be set in the configuration file when using 'simple index'"))?;
    let onnx_tokenizer_dir_str = config.onnx_tokenizer_path.as_ref()
        .ok_or_else(|| anyhow!("ONNX tokenizer path must be set in the configuration file when using 'simple index'"))?;
    let _onnx_model_path = PathBuf::from(onnx_model_path_str);
    let _onnx_tokenizer_path = PathBuf::from(onnx_tokenizer_dir_str);
    if !_onnx_model_path.exists() {
        return Err(anyhow!("Resolved ONNX model path does not exist: {}", _onnx_model_path.display()));
    }
    if !_onnx_tokenizer_path.is_dir() {
        return Err(anyhow!("Resolved ONNX tokenizer path is not a directory: {}", _onnx_tokenizer_path.display()));
    }
    let tokenizer_file = _onnx_tokenizer_path.join("tokenizer.json");
    if !tokenizer_file.exists() {
        return Err(anyhow!("tokenizer.json not found in the ONNX tokenizer directory: {}", _onnx_tokenizer_path.display()));
    }
    log::info!("Using resolved ONNX model: {}", _onnx_model_path.display());
    log::info!("Using resolved ONNX tokenizer directory: {}", _onnx_tokenizer_path.display());

    // Create EmbeddingHandler using the validated config paths
    let embedding_handler = EmbeddingHandler::new(config)
        .context("Failed to initialize embedding handler for simple index")?;
    let embedding_dim = embedding_handler
        .dimension()
        .context("Failed to get embedding dimension")?;
    log::info!("Embedding dimension: {}", embedding_dim);

    // Ensure collection exists with the correct embedding dimension
    ensure_legacy_collection_exists(&client, collection_name, embedding_dim as u64).await?;
    if !client.collection_exists(collection_name.to_string()).await? {
        bail!("Collection '{}' check failed after creation attempt.", collection_name);
    }

    // Gather all files to index (flatten directories, apply extension filter)
    let mut files_to_process = Vec::new();
    let file_types_set: Option<HashSet<String>> = cmd_args
        .file_extensions
        .as_ref()
        .map(|ft_vec| {
            ft_vec
                .iter()
                .map(|s| s.trim_start_matches('.').to_lowercase())
                .collect()
        });
    for path_arg in &cmd_args.paths {
        let absolute_path_arg = path_arg.canonicalize().with_context(|| format!("Failed to get absolute path for: {}", path_arg.display()))?;
        if absolute_path_arg.is_file() {
            let should_process = match &file_types_set {
                Some(filter_set) => {
                    let extension = absolute_path_arg
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|s| s.to_lowercase())
                        .unwrap_or_default();
                    filter_set.contains(&extension)
                }
                None => true,
            };
            if should_process {
                files_to_process.push(absolute_path_arg);
            }
        } else if absolute_path_arg.is_dir() {
            for entry_result in WalkDir::new(&absolute_path_arg).into_iter().filter_map(|e| e.ok()) {
                let entry_path = entry_result.path();
                if !entry_path.is_file() {
                    continue;
                }
                let should_process = match &file_types_set {
                    Some(filter_set) => {
                        let extension = entry_path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .map(|s| s.to_lowercase())
                            .unwrap_or_default();
                        filter_set.contains(&extension)
                    }
                    None => true,
                };
                if should_process {
                    files_to_process.push(entry_path.to_path_buf());
                }
            }
        }
    }

    // Convert files_to_process to relative paths from a dummy root (for repo_indexer compatibility)
    // We'll use the current directory as the root, and store relative paths
    let cwd = std::env::current_dir().context("Failed to get current working directory")?;
    let mut relative_paths = Vec::new();
    for abs_path in &files_to_process {
        let rel = abs_path.strip_prefix(&cwd).unwrap_or(abs_path).to_path_buf();
        relative_paths.push(rel);
    }

    // Call the repo indexer with dummy branch/commit values
    let branch_name = "main";
    let commit_hash = "simple-index";
    let total_chunks_indexed = vectordb_core::repo_helpers::repo_indexing::index_files(
        client.clone(),
        None,
        None,
        config,
        &cwd,
        &relative_paths,
        collection_name,
        branch_name,
        commit_hash,
    ).await.map_err(|e| anyhow!("Repo indexer failed: {}", e))?;

    println!("\nSimple Indexing Summary for Collection '{}':", collection_name);
    println!("  Files Scanned:       {}", files_to_process.len());
    println!("  Files Processed:     {}", files_to_process.len()); // No skip logic here
    println!("  Files Skipped:       0");
    println!("  Chunks Indexed:      {}", total_chunks_indexed);

    Ok(())
}

// --- Simple Query Handler ---

async fn handle_simple_query(
    args: &SimpleQueryArgs,
    cli_args: &CliArgs,
    config: &AppConfig,
    client: Arc<Qdrant>,
) -> Result<()> {
    log::info!("Starting simple query process...");

    let collection_name = LEGACY_INDEX_COLLECTION;
    log::info!("Querying default collection: '{}'", collection_name);

    if !client.collection_exists(collection_name).await? {
        println!("Default collection '{}' does not exist or has not been indexed yet.", collection_name);
        println!("Try running 'simple index <path>' first.");
        return Ok(());
    }

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
    let _onnx_model_path = PathBuf::from(onnx_model_path_str);
    let _onnx_tokenizer_path = PathBuf::from(onnx_tokenizer_dir_str);
    let embedding_handler = EmbeddingHandler::new(config)
        .context("Failed to initialize embedding handler for simple query")?;

    let embedding_results = embedding_handler.create_embedding_model()?.embed_batch(&[&args.query])?;
    let query_embedding = embedding_results.into_iter().next()
        .ok_or_else(|| anyhow!("Failed to generate embedding for query"))?;
    log::info!("Query embedding generated.");

    let mut filter_conditions = Vec::new();
    if let Some(lang_name) = &args.lang {
        filter_conditions.push(Condition::matches(FIELD_LANGUAGE, lang_name.clone()));
        log::info!("Filtering by language: {}", lang_name);
    }
    if let Some(element_type) = &args.element_type {
        filter_conditions.push(Condition::matches(FIELD_ELEMENT_TYPE, element_type.clone()));
        log::info!("Filtering by element type: {}", element_type);
    }
    let search_filter = if filter_conditions.is_empty() { None } else { Some(Filter::must(filter_conditions)) };

    log::info!("Executing search against collection: '{}'...", collection_name);
    
    let mut builder = SearchPointsBuilder::new(collection_name, query_embedding, args.limit)
        .with_payload(true);
    if let Some(filter) = search_filter {
            builder = builder.filter(filter);
    }
    let search_request = builder.build();
    
    let search_response = client.search_points(search_request).await
        .with_context(|| format!("Qdrant search failed for collection '{}'", collection_name))?;

    log::info!("Search returned {} results from collection {}", search_response.result.len(), collection_name);
    
    crate::cli::formatters::print_search_results(&search_response.result, &args.query, args.json)?;

    Ok(())
}

// --- Simple Clear Handler ---

async fn handle_simple_clear(
    _args: &SimpleClearArgs,
    _config: &AppConfig,
    client: Arc<Qdrant>,
) -> Result<()> {
    let collection_name = LEGACY_INDEX_COLLECTION;
    log::info!("Starting simple clear process for collection: '{}'", collection_name);

    if !client.collection_exists(collection_name).await? {
        println!("Default collection '{}' does not exist. Nothing to clear.", collection_name);
        return Ok(());
    }

    println!("Clearing all data from default collection '{}'...", collection_name);
    log::info!("Deleting all points from collection '{}'...", collection_name);

    let delete_filter: Option<Filter> = None; 
    let points_selector = PointsSelector {
        points_selector_one_of: Some(qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Filter(delete_filter.unwrap_or_default())),
    };

    let delete_request = DeletePoints {
        collection_name: collection_name.to_string(),
        wait: Some(true),
        points: Some(points_selector),
        ordering: None,
        shard_key_selector: None,
    };
    
    match client.delete_points(delete_request).await {
        Ok(response) => {
             if let Some(result) = response.result {
                 match UpdateStatus::try_from(result.status) {
                     Ok(UpdateStatus::Completed) => {
                         println!("Successfully cleared all data from collection '{}'.", collection_name);
                         log::info!("Cleared all points from collection '{}'.", collection_name);
                     }
                     Ok(status) => {
                         println!("Clear operation finished with status: {:?}", status);
                         log::warn!("Clear operation for '{}' finished with status: {:?}", collection_name, status);
                     }
                     Err(_) => {
                         println!("Clear operation finished with unknown status: {}", result.status);
                         log::warn!("Clear operation for '{}' finished with unknown status: {}", collection_name, result.status);
                     }
                 }
             } else {
                 println!("Clear operation response did not contain a result.");
                 log::warn!("Clear operation response for '{}' did not contain a result.", collection_name);
             }
        }
        Err(e) => {
            let anyhow_err: anyhow::Error = anyhow!(e);
            return Err(anyhow_err.context(format!("Failed to delete points from collection '{}'", collection_name)));
        }
    }

    Ok(())
}


// --- Helper Function to Ensure Legacy Collection Exists ---
async fn ensure_legacy_collection_exists(
    client: &Qdrant,
    collection_name: &str,
    embedding_dimension: u64,
) -> Result<()> {
    if client.collection_exists(collection_name).await? {
        log::info!("Collection '{}' already exists.", collection_name);
    } else {
        log::info!("Collection '{}' not found. Creating...", collection_name);
        let create_request = CreateCollectionBuilder::new(collection_name)
            .vectors_config(VectorParamsBuilder::new(embedding_dimension, Distance::Cosine));
        
        client.create_collection(create_request).await?;
        log::info!("Collection '{}' created successfully.", collection_name);
        
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    ensure_payload_index(client, collection_name, FIELD_FILE_PATH, FieldType::Keyword, true, None).await?;
    ensure_payload_index(client, collection_name, FIELD_LANGUAGE, FieldType::Keyword, true, None).await?;
    ensure_payload_index(client, collection_name, FIELD_ELEMENT_TYPE, FieldType::Keyword, true, None).await?;
    ensure_payload_index(client, collection_name, FIELD_START_LINE, FieldType::Integer, false, None).await?;
    ensure_payload_index(client, collection_name, FIELD_END_LINE, FieldType::Integer, false, None).await?;
    ensure_payload_index(client, collection_name, FIELD_FILE_EXTENSION, FieldType::Keyword, true, None).await?;

    Ok(())
} 