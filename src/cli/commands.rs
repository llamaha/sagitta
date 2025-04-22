// use crate::vectordb::embedding::EmbeddingModelType;
// use crate::vectordb::search::Search; // Removed
// use crate::vectordb::cache::CacheCheckResult; // Removed
use anyhow::{bail, Result};
use clap::Parser;
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
use clap::{Subcommand};
use qdrant_client::qdrant::{
        // Removed unused: CollectionStatus,
        payload_index_params::IndexParams, // Keep this one
        FieldType, IntegerIndexParams, KeywordIndexParams, TextIndexParams, TokenizerType, UpdateStatus, 
        UpsertPointsBuilder,
        UpsertPoints,
        CreateFieldIndexCollectionBuilder,
        GetCollectionInfoResponse, 
        PointStruct, 
        // REMOVED incorrect imports: field_type, field_index_params
    };
use indicatif::ProgressBar;
use log;

// Import config
// use crate::cli::server::ServerArgs; // Removed - Module/Struct likely moved or removed

// Use config types from vectordb_core
use vectordb_core::AppConfig;

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

// Implement Default for CliArgs for use in server code
impl Default for CliArgs {
    fn default() -> Self {
        Self {
            command: Commands::Simple(super::simple::SimpleArgs {
                command: super::simple::SimpleCommand::Query(super::simple::SimpleQueryArgs {
                    query: String::new(),
                    limit: 10,
                    lang: None,
                    element_type: None,
                    json: false,
                }),
            }),
            onnx_model_path_arg: None,
            onnx_tokenizer_dir_arg: None,
        }
    }
}

// Implementation for Commands enum needed for Default implementation
impl Default for Commands {
    fn default() -> Self {
        Commands::Simple(super::simple::SimpleArgs::default())
    }
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
    /// Manage repositories (add, list, use, remove, sync)
    #[command(subcommand_negates_reqs = true)]
    Repo(super::repo_commands::RepoArgs),
    /// Manage the simple, non-repository index (index, query, clear)
    #[command(subcommand_negates_reqs = true)]
    Simple(super::simple::SimpleArgs), // Add Simple command group
    /// Edit code using semantic understanding
    #[command(subcommand_negates_reqs = true)]
    Edit(crate::edit::cli::EditArgs),
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
        Commands::Repo(ref cmd_args) => super::repo_commands::handle_repo_command(cmd_args.clone(), &args, config, client, None).await,
        Commands::Simple(ref cmd_args) => super::simple::handle_simple_command(cmd_args.clone(), &args, config.clone(), client).await,
        Commands::Edit(ref cmd_args) => crate::edit::cli::handle_edit_command(cmd_args.clone(), &args, config.clone(), client).await,
    }
}

// --- Helper Functions ---

// Helper function to create payload indices, ignoring errors if index already exists
pub async fn ensure_payload_index(
    client: &Qdrant,
    collection_name: &str,
    field_name: &str,
    field_type: FieldType,
    _is_keyword: bool, // Renamed for clarity and marked as unused
    tokenizer: Option<TokenizerType>,
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

    // Explicitly define index parameters based on type
    let index_params = match field_type {
        FieldType::Keyword => Some(IndexParams::KeywordIndexParams(KeywordIndexParams {
            on_disk: None,
            // Note: is_tenant might also not be valid here depending on qdrant version,
            // keeping for now unless it causes errors. Adjust if needed.
            is_tenant: Some(false), 
        })),
        FieldType::Integer => Some(IndexParams::IntegerIndexParams(IntegerIndexParams {
            // Provide default values wrapped in Some()
            lookup: Some(false), 
            range: Some(false),
            is_principal: Some(false),
            on_disk: None, // Keep as None if optional, or set default bool if required
        })),
        FieldType::Text => Some(IndexParams::TextIndexParams(TextIndexParams {
            tokenizer: tokenizer.map(|t| t.into()).unwrap_or(TokenizerType::Word.into()),
            lowercase: Some(true),
            min_token_len: None,
            max_token_len: None,
            on_disk: None,
        })),
        // Add other types as needed, potentially with None if no params required
        _ => None,
    };

    let mut builder = CreateFieldIndexCollectionBuilder::new(collection_name, field_name, field_type);

    // Only add field_index_params if we defined some
    if let Some(params) = index_params {
        builder = builder.field_index_params(params);
    }

    match client.create_field_index(builder).await {
        Ok(response) => {
             if let Some(result) = response.result {
                 match UpdateStatus::try_from(result.status) {
                     Ok(UpdateStatus::Completed) => {
                         log::info!("Payload index created successfully for field '{}'.", field_name);
                         Ok(())
                     }
                     Ok(status) => {
                         log::warn!("Payload index creation for field '{}' resulted in status: {:?}", field_name, status);
                         Ok(())
                     }
                     Err(_) => {
                         log::warn!("Payload index creation for field '{}' returned unknown status: {}", field_name, result.status);
                         Ok(())
                     }
                 }
             } else {
                 log::warn!("Payload index creation response for field '{}' did not contain a result.", field_name);
                 Ok(())
             }
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
    _batch_num: usize,
    _total_batches: usize,
    progress_bar: &ProgressBar,
) -> Result<()> {
    if points.is_empty() {
        return Ok(());
    }
    let num_points = points.len();
    
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
                         log::debug!("{}", msg);
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
