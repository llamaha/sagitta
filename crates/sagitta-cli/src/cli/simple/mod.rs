// Third-party imports
use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use qdrant_client::{
    qdrant::{ 
        Condition, Filter, 
        UpdateStatus, SearchResponse,
        QueryResponse,
    },
    Qdrant,
};
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    collections::HashSet,
    path::PathBuf, 
    sync::Arc,
    time::Duration,
};

use sagitta_search::config::AppConfig;
use crate::cli::CliArgs;
use sagitta_search::EmbeddingPool; // Use re-export from main crate
use sagitta_search::EmbeddingProcessor; // For dimension() method
use crate::cli::commands::{ // Only import necessary items from commands
    LEGACY_INDEX_COLLECTION, 
    FIELD_CHUNK_CONTENT, FIELD_ELEMENT_TYPE, FIELD_END_LINE,
    FIELD_FILE_PATH, FIELD_LANGUAGE, FIELD_START_LINE,
};
use sagitta_search::search_collection;
use sagitta_search::qdrant_ops::{delete_collection_by_name, delete_all_points};
use sagitta_search::error::SagittaError; // Import SagittaError
use sagitta_search::config::load_config; // Import load_config
use sagitta_search::indexing::ensure_collection_exists; // Import ensure_collection_exists
use colored::Colorize; // Added import for Colorize trait
use sagitta_search::sync_progress::{SyncProgress, SyncStage, SyncProgressReporter};
use async_trait::async_trait;

// Local module imports
// pub mod index; // Remove index module
// use index::handle_file_processing; // Remove usage
pub mod search_file; // Added
pub mod view_file; // Added

// Constants
// const SIMPLE_INDEX_COLLECTION: &str = \"simple_index\"; // REMOVED - Unused

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
    /// Search for files within the current directory (or specified paths) using glob patterns.
    SearchFile(search_file::SearchFileArgs),
    /// View the content of a specific file.
    ViewFile(view_file::ViewFileArgs),
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

// A simple progress reporter for the CLI that wraps an Indicatif ProgressBar
#[derive(Debug)]
struct IndicatifCliReporter {
    pb: ProgressBar,
}

impl IndicatifCliReporter {
    fn new(pb: ProgressBar) -> Self {
        IndicatifCliReporter { pb }
    }
}

#[async_trait]
impl SyncProgressReporter for IndicatifCliReporter {
    async fn report(&self, progress: SyncProgress) {
        match progress.stage {
            SyncStage::IndexFile { current_file, total_files, current_file_num, files_per_second, .. } => {
                self.pb.set_length(total_files as u64);
                self.pb.set_position(current_file_num as u64);
                let file_name = current_file.map_or_else(|| "".to_string(), |p| p.file_name().unwrap_or_default().to_string_lossy().to_string());
                let mut msg = format!("Indexing {}/{} {}", current_file_num, total_files, file_name);
                if let Some(fps) = files_per_second {
                    msg.push_str(&format!(" ({:.1} files/s)", fps));
                }
                self.pb.set_message(msg);
            }
            SyncStage::CollectFiles { total_files, message } => {
                self.pb.set_length(total_files as u64); // Or some other appropriate length
                self.pb.set_message(format!("Collect: {} ({})", message, total_files));
            }
            SyncStage::Completed { message } => {
                self.pb.finish_with_message(format!("Completed: {}", message));
            }
            SyncStage::Error { message } => {
                self.pb.abandon_with_message(format!("Error: {}", message));
            }
            _ => { // For other stages, just tick or set a generic message
                self.pb.tick();
                self.pb.set_message(format!("{:?}", progress.stage));
            }
        }
    }
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
        SimpleCommand::SearchFile(search_args) => search_file::handle_simple_search_file(&search_args).await,
        SimpleCommand::ViewFile(view_args) => view_file::handle_simple_view_file(&view_args).await,
    }
}

// --- Simple Index Handler (Refactored) ---

async fn handle_simple_index(
    cmd_args: &SimpleIndexArgs,
    cli_args: &CliArgs,
    config: &AppConfig,
    client: Arc<Qdrant>,
) -> Result<()> {
    log::info!("Starting simple indexing process...");

    let collection_name = LEGACY_INDEX_COLLECTION;
    log::info!("Using default collection: '{}'", collection_name);

    // --- Validate Config for ONNX paths for simple index FIRST ---
    if cli_args.onnx_model_path_arg.is_some() || std::env::var("SAGITTA_ONNX_MODEL").is_ok() {
        return Err(anyhow!("For 'simple index', ONNX model path must be provided solely via the configuration file, not CLI arguments or environment variables."));
    }
    if cli_args.onnx_tokenizer_dir_arg.is_some() || std::env::var("SAGITTA_ONNX_TOKENIZER_DIR").is_ok() {
         return Err(anyhow!("For 'simple index', ONNX tokenizer path must be provided solely via the configuration file, not CLI arguments or environment variables."));
    }
    if config.onnx_model_path.is_none() || config.onnx_tokenizer_path.is_none() {
         return Err(anyhow!("ONNX model and tokenizer paths must be set in the configuration file when using 'simple index'"));
    }
    log::info!("Using ONNX paths from configuration file for simple index.");

    // --- Ensure Collection Exists with Correct Dimension --- 
    let vector_dim = config.performance.vector_dimension as u64;

    ensure_collection_exists(client.clone(), collection_name, vector_dim)
        .await
        .with_context(|| format!("Failed to ensure collection '{}' exists with dimension {}", collection_name, vector_dim))?;
    log::info!("Ensured collection '{}' exists with dimension {}.", collection_name, vector_dim);

    // --- Clear existing points from collection before indexing (optional, original behavior) ---
    // If ensure_collection_exists recreates on dimension mismatch, this might not be strictly necessary
    // but clearing points ensures a fresh index if the collection already existed with the correct dimension.
    println!("Clearing points from default collection '{}' before indexing...", collection_name);
    match delete_all_points(client.clone(), collection_name).await {
        Ok(_) => {
            log::info!("Successfully cleared points from collection '{}'.", collection_name);
            println!("Collection points cleared.");
        }
        Err(e) => {
            log::error!("Failed to clear points from collection '{}': {}. Proceeding anyway.", collection_name, e);
            eprintln!("Warning: Failed to clear points from collection '{}': {}. Proceeding anyway.", collection_name, e);
        }
    }

    // Validate input paths exist (basic check)
    for path in &cmd_args.paths {
        if !path.exists() {
            bail!("Path does not exist: {}", path.display());
        }
    }
    log::debug!("Input paths: {:?}", cmd_args.paths);

    let embedding_config = sagitta_search::app_config_to_embedding_config(config);
    let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config)
        .context("Failed to initialize embedding pool")?;

    // --- Prepare Filters ---
    let file_extensions_set: Option<HashSet<String>> = cmd_args
        .file_extensions
        .as_ref()
        .map(|ft_vec| {
            ft_vec
                .iter()
                .map(|s| s.trim_start_matches('.').to_lowercase())
                .collect()
        });

    // --- Setup Progress Bar ---
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["-", "\\", "|", "/", "-", "\\", "|", "/"])
            .template("{spinner} {elapsed_precise} [{bar:40.cyan/blue}] {pos}/{len} ({msg})")?
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message("Gathering files...");

    // --- Call Core Indexing Logic ---
    let index_result = sagitta_search::indexing::index_paths(
        &cmd_args.paths, 
        file_extensions_set,
        collection_name,
        client.clone(),
        &embedding_pool,
        Some(Arc::new(IndicatifCliReporter::new(pb.clone()))),
        config, // Use the passed config instead of reloading
    ).await;

    // --- Handle Result ---
    pb.finish(); // Ensure spinner stops
    match index_result {
        Ok(_) => {
            log::info!("Simple indexing completed successfully.");
            println!("Indexing finished.");
            Ok(())
        }
        Err(e) => {
            log::error!("Simple indexing failed: {}", e);
            match e {
                sagitta_search::error::SagittaError::ConfigurationError(ref msg) => {
                    eprintln!("Configuration Error: {}", msg);
                }
                sagitta_search::error::SagittaError::EmbeddingError(ref msg) => {
                    eprintln!("Embedding Error: {}", msg);
                }
                sagitta_search::error::SagittaError::QdrantError(ref msg) => {
                    eprintln!("Database Error: {}", msg);
                }
                 sagitta_search::error::SagittaError::IOError(ref io_err) => {
                     eprintln!("I/O Error during indexing: {}", io_err);
                 }
                _ => {
                    eprintln!("An unexpected error occurred during indexing: {}", e);
                }
            }
            // Convert the specific error back to a generic anyhow::Error for the CLI handler return type
            Err(anyhow!(e))
        }
    }
}

// --- Simple Query Handler (Refactored) ---

async fn handle_simple_query(
    args: &SimpleQueryArgs,
    cli_args: &CliArgs,
    config: &AppConfig,
    client: Arc<Qdrant>,
) -> Result<()> {
    let collection_name = LEGACY_INDEX_COLLECTION;
    log::info!("Searching collection '{}' for query: '{}', limit: {}, filter: {:?}", 
        collection_name, args.query, args.limit, 
        (args.lang.as_ref(), args.element_type.as_ref()));

    // Ensure collection exists before querying
    if !client.collection_exists(collection_name).await? {
        log::warn!("Collection '{}' doesn't exist. Creating it before searching.", collection_name);
        let vector_dim = config.performance.vector_dimension as u64;
        ensure_collection_exists(client.clone(), collection_name, vector_dim).await?;
        println!("No results found (new collection created).");
        return Ok(());
    }
    
    // Build search filters based on args
    let mut filter_conditions = Vec::new();
    if let Some(lang_name) = &args.lang {
        filter_conditions.push(Condition::matches(FIELD_LANGUAGE, lang_name.clone()));
        log::debug!("Filtering by language: {}", lang_name);
    }
    if let Some(element_type) = &args.element_type {
        filter_conditions.push(Condition::matches(FIELD_ELEMENT_TYPE, element_type.clone()));
        log::debug!("Filtering by element type: {}", element_type);
    }
    let filter = if filter_conditions.is_empty() { None } else { Some(Filter::must(filter_conditions)) };

    log::info!(
        "Searching collection '{}' for query: '{}', limit: {}, filter: {:?}",
        collection_name,
        args.query,
        args.limit,
        filter
    );

    // 5. Perform Search
    // Use the already loaded config from the parameter
    let embedding_config = sagitta_search::app_config_to_embedding_config(config);
    let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config)
        .context("Failed to initialize embedding pool")?;
    
    let start_time = std::time::Instant::now(); // Define start_time here
    let search_response_result: Result<QueryResponse, SagittaError> = search_collection(
        client.clone(),
        collection_name,
        &embedding_pool,
        &args.query,
        args.limit,
        filter,
        config, // <-- Pass the loaded config
        None, // Use default search configuration
    ).await;

    let duration = start_time.elapsed();

    match search_response_result {
        Ok(response) => {
            if args.json {
                // Output JSON
                let hits: Vec<_> = response.result.into_iter()
                    .map(|point| {
                        let payload_json = serde_json::to_value(point.payload)
                            .unwrap_or(serde_json::Value::Null);
                        serde_json::json!({
                            "id": point.id.map(|id| format!("{:?}", id)).unwrap_or_default(),
                            "score": point.score,
                            "payload": payload_json
                        })
                    }).collect();
                
                // Wrap the hits in a "results" field
                let output_json = serde_json::json!({ "results": hits });
                println!("{}", serde_json::to_string_pretty(&output_json)?);
            } else {
                // Output human-readable
                if response.result.is_empty() {
                    println!("No results found.");
                } else {
                    println!("Search Results:");
                    for (i, point) in response.result.iter().enumerate() {
                        println!("--- Result {} (Score: {:.4}) ---", i + 1, point.score);
                        // Pretty print payload fields
                        if let Some(path) = point.payload.get(FIELD_FILE_PATH).and_then(|v| v.as_str()) {
                            println!("  File: {}", path);
                        }
                        if let Some(start) = point.payload.get(FIELD_START_LINE).and_then(|v| v.as_integer()) {
                            if let Some(end) = point.payload.get(FIELD_END_LINE).and_then(|v| v.as_integer()) {
                                println!("  Lines: {}-{}", start, end);
                            }
                        }
                        if let Some(lang) = point.payload.get(FIELD_LANGUAGE).and_then(|v| v.as_str()) {
                            println!("  Lang: {}", lang);
                        }
                        if let Some(elem_type) = point.payload.get(FIELD_ELEMENT_TYPE).and_then(|v| v.as_str()) {
                            println!("  Type: {}", elem_type);
                        }
                        if let Some(content) = point.payload.get(FIELD_CHUNK_CONTENT).and_then(|v| v.as_str()) {
                            println!("  Content:\n    {}", content.trim().replace('\n', "\n    "));
                        }
                    }
                }
            }
        }
        Err(e) => {
            // Handle specific SagittaError variants if needed, otherwise use anyhow
            match &e {
                 sagitta_search::error::SagittaError::ConfigurationError(ref msg) => {
                     eprintln!("Configuration Error: {}", msg);
                 }
                 sagitta_search::error::SagittaError::EmbeddingError(ref msg) => {
                     eprintln!("Embedding Error: {}", msg);
                 }
                 sagitta_search::error::SagittaError::IOError(ref io_err) => {
                     eprintln!("Error during search (IO): {}", io_err);
                 }
                 sagitta_search::error::SagittaError::QdrantError(ref msg) => {
                    eprintln!("Qdrant client error during search: {}", msg);
                 }
                 // Add other specific SagittaError arms here if needed
                 _ => eprintln!("An error occurred during search: {}", e),
             }
             return Err(anyhow!(e).context("Search operation failed"));
        }
    }

    Ok(())
}

// --- Simple Clear Handler (Refactored) ---

async fn handle_simple_clear(
    _args: &SimpleClearArgs,
    config: &AppConfig, // Keep config in case it's needed by other logic later, or for consistency
    client: Arc<Qdrant>,
) -> Result<()> {
    let collection_name = LEGACY_INDEX_COLLECTION;
    log::info!("Starting simple clear (delete collection) process for: '{}'", collection_name);

    if !client.collection_exists(collection_name).await.unwrap_or(false) {
        println!("Default collection '{}' does not exist. Nothing to clear.", collection_name);
        return Ok(());
    }

    println!("Deleting default collection '{}'...", collection_name);
    log::info!("Calling core delete_collection_by_name for collection '{}'...", collection_name);

    match delete_collection_by_name(client.clone(), collection_name).await {
        Ok(_) => {
            // The delete_collection_by_name function in core already logs success/warnings based on response.result
            println!("Successfully deleted default collection '{}'.", collection_name);
            log::info!("Successfully initiated deletion of collection '{}'.", collection_name);
        }
        Err(e) => {
            if e.to_string().contains("Not found") || e.to_string().contains("doesn\'t exist") {
                println!(
                    "{}",
                    format!("Collection '{}' did not exist.", collection_name).yellow()
                );
                log::warn!("Collection '{}' not found during delete attempt.", collection_name);
            } else {
                // For other errors, report them
                eprintln!(
                    "{}",
                    format!("Failed to delete collection '{}': {}", collection_name, e).red()
                );
                return Err(e).context(format!("Failed to delete collection '{}'", collection_name));
            }
        }
    }
    Ok(())
} 