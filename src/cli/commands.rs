// use crate::vectordb::embedding::EmbeddingModelType;
// use crate::vectordb::search::Search; // Removed
// use crate::vectordb::cache::CacheCheckResult; // Removed
use anyhow::{bail, Result};
// Removed: use clap::Parser;
// Removed: use log::{debug, warn};
// Removed: use num_cpus;
// Removed: use rayon;
// Removed: use std::path::PathBuf;
// Removed: use std::time::Instant;
// use crate::vectordb::search::result::SearchResult; // Removed
// use crate::vectordb::search::{chunking, snippet}; // Removed
// use std::collections::HashMap; // Removed
// use std::collections::HashSet; // Removed
// use crate::vectordb::utils::cosine_distance; // Removed
// use walkdir::WalkDir; // Removed
// use chrono::{DateTime, Utc, TimeZone, Local}; // Removed DateTime, TimeZone, Local

// Import Qdrant client types
// use qdrant_client::client::QdrantClient; // Old import
use qdrant_client::Qdrant; // Import the Qdrant struct
use std::sync::Arc;
use clap::{Parser, Subcommand};
use qdrant_client::qdrant::{
        // Removed unused: CollectionStatus,
        FieldType, PointStruct, TextIndexParams, KeywordIndexParams, TokenizerType, UpdateStatus, 
        UpsertPointsBuilder,
        UpsertPoints,
        CreateFieldIndexCollectionBuilder,
        GetCollectionInfoResponse, // Add this import
        payload_index_params::IndexParams, // Keep only used imports
    };
use indicatif::ProgressBar;
use log;

// Import config
use crate::config::AppConfig;

// Moved from index.rs
pub(crate) const BATCH_SIZE: usize = 128;

// CliArgs struct definition moved here from main binary
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,

    /// Path to ONNX model file (overrides config & env var)
    #[arg(short = 'm', long = "onnx-model", global = true, env = "VECTORDB_ONNX_MODEL")]
    pub onnx_model_path_arg: Option<String>,

    /// Path to ONNX tokenizer config directory (overrides config & env var)
    #[arg(short = 't', long = "onnx-tokenizer-dir", global = true, env = "VECTORDB_ONNX_TOKENIZER_DIR")]
    pub onnx_tokenizer_dir_arg: Option<String>,
}

// Global flag for handling interrupts
// pub static mut INTERRUPT_RECEIVED: bool = false; // Commented out as unused for now

// --- Module Definitions ---
// Modules are declared in src/cli/mod.rs.
// pub mod index;
// pub mod query;
// pub mod stats;
// pub mod list;

// No need for `pub use super::...` if modules are declared here and handlers are called directly.

// --- Constants ---
// pub(crate) const CODE_SEARCH_COLLECTION: &str = "vectordb-code-search"; // REMOVED
pub const FIELD_FILE_PATH: &str = "file_path";
// pub(crate) const FIELD_DIR_PATH: &str = "dir_path"; // REMOVED
pub const FIELD_START_LINE: &str = "start_line";
pub const FIELD_END_LINE: &str = "end_line";
pub const FIELD_FILE_EXTENSION: &str = "file_extension";
pub const FIELD_LANGUAGE: &str = "language";
pub const FIELD_ELEMENT_TYPE: &str = "element_type";
pub const FIELD_CHUNK_CONTENT: &str = "chunk_content";

// Moved to simple/mod.rs or repo_commands.rs potentially
// pub(crate) const DEFAULT_VECTOR_DIMENSION: u64 = 384; 
pub const LEGACY_INDEX_COLLECTION: &str = "vectordb-code-search"; // Keep for simple index

// Fields specific to repository indexing
pub const FIELD_BRANCH: &str = "branch";
pub const FIELD_COMMIT_HASH: &str = "commit_hash";

// Collection name for the simple/legacy index command
pub const SIMPLE_INDEX_COLLECTION: &str = "vectordb-code-search";

// --- Main Command Enum ---
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Query the active or specified repository's vector database
    #[command(subcommand_negates_reqs = true)]
    Query(super::query::QueryArgs), // Use super:: path
    /// Show statistics about the vector database collections
    #[command(subcommand_negates_reqs = true)]
    Stats(super::stats::StatsArgs), // Use super:: path
    /// List indexed files in the active repository
    #[command(subcommand_negates_reqs = true)]
    List(super::list::ListArgs), // Use super:: path
    /// Clear data for a specific repository
    #[command(subcommand_negates_reqs = true)]
    Clear(super::clear::ClearArgs), // Add Clear command
    /// Manage repositories (add, list, use, remove, sync)
    #[command(subcommand_negates_reqs = true)]
    Repo(super::repo_commands::RepoArgs),
    /// Manage the simple, non-repository index (index, query, clear)
    #[command(subcommand_negates_reqs = true)]
    Simple(super::simple::SimpleArgs), // Add Simple command group
}

// --- Main Command Handler Function ---
/// Parses the command-line arguments and dispatches to the appropriate command handler.
///
/// # Arguments
/// * `args` - The parsed top-level command line arguments ([`CliArgs`]).
/// * `config` - The loaded application configuration ([`AppConfig`]).
/// * `client` - An Arc-wrapped Qdrant client instance.
pub async fn handle_command(
    args: CliArgs,
    config: &mut AppConfig,
    client: Arc<Qdrant>,
) -> Result<()> {
    match args.command {
        // Pass args, config, and client to handlers that need them
        Commands::Query(ref cmd_args) => super::query::handle_query(cmd_args, &args, config.clone(), client).await,
        Commands::Stats(cmd_args) => super::stats::handle_stats(cmd_args, config.clone(), client).await,
        Commands::List(cmd_args) => super::list::handle_list(cmd_args, config.clone(), client).await,
        Commands::Clear(ref cmd_args) => super::clear::handle_clear(cmd_args, config.clone(), client).await,
        Commands::Repo(ref cmd_args) => super::repo_commands::handle_repo_command(cmd_args.clone(), &args, config, client).await,
        Commands::Simple(ref cmd_args) => super::simple::handle_simple_command(cmd_args.clone(), &args, config.clone(), client).await, // Add handler for Simple
    }
}

// --- Helper Functions ---

// Helper function to create payload indices, ignoring errors if index already exists
pub async fn ensure_payload_index(
    client: &Qdrant,
    collection_name: &str,
    field_name: &str,
    field_type: FieldType,
    keyword: bool, // Flag to create keyword index instead of text
    tokenizer: Option<TokenizerType>, // Optional tokenizer for text index
) -> Result<()> {
    let info: GetCollectionInfoResponse = client.collection_info(collection_name).await?;
    if let Some(config) = info.result {
        if config.payload_schema.get(field_name).is_some() {
            log::debug!("Payload index for '{}' on field '{}' already exists.", collection_name, field_name);
            return Ok(());
        }
    } else {
        bail!("Could not retrieve collection info for {}", collection_name);
    }

    log::info!("Creating payload index for '{}' on field '{}'...", collection_name, field_name);

    // Create the inner IndexParams enum variant
    let inner_index_params = if keyword {
        IndexParams::KeywordIndexParams(KeywordIndexParams {
            on_disk: None,
            is_tenant: Some(false),
        })
    } else {
        IndexParams::TextIndexParams(TextIndexParams {
            tokenizer: tokenizer.map(|t| t.into()).unwrap_or(TokenizerType::Word.into()),
            lowercase: Some(true),
            min_token_len: None,
            max_token_len: None,
            on_disk: None,
        })
    };

    let builder = CreateFieldIndexCollectionBuilder::new(collection_name, field_name, field_type)
        .field_index_params(inner_index_params);

    match client.create_field_index(builder).await {
        Ok(response) => {
             if let Some(result) = response.result {
                 match UpdateStatus::try_from(result.status) {
                     Ok(UpdateStatus::Completed) => {
                         log::info!("Payload index created successfully for field '{}'.", field_name);
                     }
                     Ok(status) => {
                         log::warn!("Payload index creation for field '{}' resulted in status: {:?}", field_name, status);
                     }
                     Err(_) => {
                         log::warn!("Payload index creation for field '{}' returned unknown status: {}", field_name, result.status);
                     }
                 }
             } else {
                 log::warn!("Payload index creation response for field '{}' did not contain a result.", field_name);
             }
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to create payload index for field '{}': {}. Ignoring error, assuming index might exist.", field_name, e);
            Ok(())
        }
    }
}

// Update upsert_batch signature and usage
pub(crate) async fn upsert_batch(
    client: &Qdrant,
    collection_name: &str,
    points: Vec<PointStruct>,
    batch_num: usize,
    total_batches: usize,
    progress_bar: &ProgressBar,
) -> Result<()> {
    if points.is_empty() {
        return Ok(());
    }
    let num_points = points.len();
    progress_bar.set_message(format!(
        "Upserting batch {}/{} ({} points) to collection '{}'...",
        batch_num,
        total_batches,
        num_points,
        collection_name
    ));
    
    let request: UpsertPoints = UpsertPointsBuilder::new(collection_name, points)
        .wait(false) 
        .build();

    match client.upsert_points(request).await {
         Ok(response) => {
             if let Some(result) = response.result {
                 match UpdateStatus::try_from(result.status) {
                     Ok(UpdateStatus::Completed) => {
                         progress_bar.inc(num_points as u64);
                         log::debug!("Upsert batch successful.");
                         Ok(())
                     },
                     Ok(status) => {
                         let msg = format!("Qdrant upsert batch completed with status: {:?}", status);
                         progress_bar.println(format!("Warning: {}", msg));
                         log::warn!("{}", msg);
                         Ok(()) 
                     },
                     Err(_) => {
                         let msg = format!("Qdrant upsert batch completed with unknown status code: {}", result.status);
                         progress_bar.println(format!("Error: {}", msg));
                         log::error!("{}", msg);
                         Err(anyhow::anyhow!(msg))
                     }
                 }
             } else {
                 let msg = "Qdrant upsert response missing result status";
                 progress_bar.println(format!("Error: {}", msg));
                 log::error!("{}", msg);
                 Err(anyhow::anyhow!(msg))
             }
         },
         Err(e) => {
             let msg = format!("Failed to upsert batch to {}: {}", collection_name, e);
             progress_bar.println(format!("Error: {}", msg));
             log::error!("{}", msg);
             Err(anyhow::anyhow!(msg).context(e))
         }
    }
}
