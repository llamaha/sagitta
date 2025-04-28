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

use vectordb_core::config::AppConfig;
use crate::cli::CliArgs;
use vectordb_core::embedding::EmbeddingHandler;
use crate::cli::commands::{ // Only import necessary items from commands
    LEGACY_INDEX_COLLECTION, 
    FIELD_CHUNK_CONTENT, FIELD_ELEMENT_TYPE, FIELD_END_LINE,
    FIELD_FILE_PATH, FIELD_LANGUAGE, FIELD_START_LINE,
};
use vectordb_core::search_collection;
use vectordb_core::qdrant_ops::delete_all_points; // Import core helpers
use vectordb_core::error::VectorDBError; // Import VectorDBError

// Local module imports
// pub mod index; // Remove index module
// use index::handle_file_processing; // Remove usage

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

    // --- Clear existing collection before indexing ---
    println!("Clearing default collection '{}' before indexing...", collection_name);
    log::info!("Calling core delete_all_points for collection '{}'...", collection_name);
    match delete_all_points(client.clone(), collection_name).await {
        Ok(_) => {
            log::info!("Successfully cleared collection '{}'.", collection_name);
            println!("Collection cleared.");
        }
        Err(e) => {
            // Log the error but proceed with indexing anyway
            log::error!("Failed to clear collection '{}' before indexing: {}. Proceeding anyway.", collection_name, e);
            eprintln!("Warning: Failed to clear collection '{}' before indexing: {}. Proceeding anyway.", collection_name, e);
            // Consider if we should bail here instead?
            // bail!("Failed to clear collection before indexing: {}", e);
        }
    }

    // Validate input paths exist (basic check)
    for path in &cmd_args.paths {
        if !path.exists() {
            bail!("Input path does not exist: {}", path.display());
        }
    }
    log::debug!("Input paths: {:?}", cmd_args.paths);

    // --- Validate Config for ONNX paths for simple index ---
    // For simple index, paths *must* come only from the config file.
    if cli_args.onnx_model_path_arg.is_some() || std::env::var("VECTORDB_ONNX_MODEL").is_ok() {
        return Err(anyhow!("For 'simple index', ONNX model path must be provided solely via the configuration file, not CLI arguments or environment variables."));
    }
    if cli_args.onnx_tokenizer_dir_arg.is_some() || std::env::var("VECTORDB_ONNX_TOKENIZER_DIR").is_ok() {
         return Err(anyhow!("For 'simple index', ONNX tokenizer path must be provided solely via the configuration file, not CLI arguments or environment variables."));
    }
    if config.onnx_model_path.is_none() || config.onnx_tokenizer_path.is_none() {
         return Err(anyhow!("ONNX model and tokenizer paths must be set in the configuration file when using 'simple index'"));
    }
    // Path existence/validity is checked later by EmbeddingHandler::new
    log::info!("Using ONNX paths from configuration file for simple index.");

    // --- Initialize Embedding Handler (uses paths from config) ---
    let embedding_handler = EmbeddingHandler::new(config)
        .context("Failed to initialize embedding handler for simple index")?;
    log::info!("Embedding dimension: {}", embedding_handler.dimension()?);

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
    let index_result = vectordb_core::indexing::index_paths(
        &cmd_args.paths, // Pass the raw paths from args
        file_extensions_set,
        collection_name,
        client.clone(),
        &embedding_handler,
        Some(&pb),
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
            // Print a user-friendly error based on VectorDBError type
            match e {
                vectordb_core::error::VectorDBError::ConfigurationError(ref msg) => {
                    eprintln!("Configuration Error: {}", msg);
                }
                vectordb_core::error::VectorDBError::EmbeddingError(ref msg) => {
                    eprintln!("Embedding Error: {}", msg);
                }
                vectordb_core::error::VectorDBError::QdrantError(ref msg) => {
                    eprintln!("Database Error: {}", msg);
                }
                 vectordb_core::error::VectorDBError::IOError(ref io_err) => {
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
    log::debug!("Handling simple query...");

    // Config validation and embedding handler initialization
    if config.onnx_model_path.is_none() || config.onnx_tokenizer_path.is_none() {
        bail!("ONNX model and tokenizer paths must be set in the config for simple query");
    }
    let embedding_handler = EmbeddingHandler::new(config)
        .context("Failed to initialize embedding handler for simple query")?;

    let collection_name = LEGACY_INDEX_COLLECTION;
    
    // Build filter directly here
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

    // Call the core search function with explicit type annotation
    let search_response_result: Result<QueryResponse, VectorDBError> = search_collection(
        client.clone(),
        collection_name,
        &embedding_handler,
        &args.query,
        args.limit,
        filter,
    ).await;

    match search_response_result {
        Ok(response) => {
            if args.json {
                // Output JSON
                let output_results: Vec<_> = response.result.into_iter()
                    .map(|point| {
                        // Convert payload Map<String, Value> to serde_json::Value
                        let payload_json = serde_json::to_value(point.payload)
                            .unwrap_or(serde_json::Value::Null);
                        serde_json::json!({
                            "id": point.id.map(|id| format!("{:?}", id)).unwrap_or_default(),
                            "score": point.score,
                            "payload": payload_json
                        })
                    }).collect();
                println!("{}", serde_json::to_string_pretty(&output_results)?);
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
            // Handle specific VectorDBError variants if needed, otherwise use anyhow
            match &e {
                 vectordb_core::error::VectorDBError::ConfigurationError(ref msg) => {
                     eprintln!("Configuration Error: {}", msg);
                 }
                 vectordb_core::error::VectorDBError::EmbeddingError(ref msg) => {
                     eprintln!("Embedding Error: {}", msg);
                 }
                 vectordb_core::error::VectorDBError::IOError(ref io_err) => {
                     eprintln!("Error during search (IO): {}", io_err);
                 }
                 vectordb_core::error::VectorDBError::QdrantError(ref msg) => {
                    eprintln!("Qdrant client error during search: {}", msg);
                 }
                 // Add other specific VectorDBError arms here if needed
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
    _config: &AppConfig, // config might not be needed now
    client: Arc<Qdrant>,
) -> Result<()> {
    let collection_name = LEGACY_INDEX_COLLECTION;
    log::info!("Starting simple clear process for collection: '{}'", collection_name);

    // --- Check Collection Existence (CLI Logic) ---
    if !client.collection_exists(collection_name).await? {
        println!("Default collection '{}' does not exist. Nothing to clear.", collection_name);
        return Ok(());
    }

    // --- Confirmation and Call Core Delete (CLI Logic) ---
    println!("Clearing all data from default collection '{}'...", collection_name);
    log::info!("Calling core delete_all_points for collection '{}'...", collection_name);

    let delete_result = delete_all_points(client.clone(), collection_name).await;

    // --- Handle Result Status (CLI Logic) ---
    match delete_result {
        Ok(response) => {
             // Access the UpdateResult within the response
             if let Some(update_result) = response.result {
                 // Now try_from on the status within UpdateResult
                 match UpdateStatus::try_from(update_result.status) {
                     Ok(UpdateStatus::Completed) => {
                         println!("Successfully cleared all data from collection '{}'.", collection_name);
                         log::info!("Cleared all points from collection '{}'.", collection_name);
                     }
                     Ok(status) => {
                         println!("Clear operation finished with status: {:?}", status);
                         log::warn!("Clear operation for '{}' finished with status: {:?}", collection_name, status);
                     }
                     Err(_) => {
                         // Use update_result.status for the unknown code
                         println!("Clear operation finished with unknown status code: {}", update_result.status);
                         log::warn!("Clear operation for '{}' finished with unknown status code: {}", collection_name, update_result.status);
                     }
                 }
             } else {
                 // Handle case where the response has no UpdateResult
                 println!("Clear operation response did not contain result details.");
                 log::warn!("Clear operation response for '{}' did not contain result details.", collection_name);
             }
        }
        Err(e) => {
            log::error!("Failed to clear collection '{}': {}", collection_name, e);
            eprintln!("Error during clear operation: {}", e);
            return Err(anyhow!(e)); // Convert to anyhow::Error
        }
    }

    Ok(())
} 