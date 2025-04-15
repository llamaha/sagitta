// use crate::vectordb::embedding::EmbeddingModelType;
// use crate::vectordb::search::Search; // Removed
// use crate::vectordb::cache::CacheCheckResult; // Removed
use anyhow::Result;
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
use qdrant_client::{
    // Removed: client::QdrantClient,
    qdrant::{
        // Removed unused: CollectionStatus,
        FieldType, PointStruct, TextIndexParams, KeywordIndexParams, IntegerIndexParams, 
        FloatIndexParams, GeoIndexParams, BoolIndexParams, DatetimeIndexParams, 
        UuidIndexParams, TokenizerType, UpdateStatus, 
        UpsertPointsBuilder,
        UpsertPoints,
        CreateFieldIndexCollectionBuilder,
    },
    // Payload, // Removed unused Payload
};
use indicatif;

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

// Fields specific to repository indexing
pub const FIELD_BRANCH: &str = "branch";
pub const FIELD_COMMIT_HASH: &str = "commit_hash";

// --- Main Command Enum ---
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Index files into the vector database
    #[command(subcommand_negates_reqs = true)]
    Index(super::index::IndexArgs), // Use super:: path
    /// Query the vector database
    #[command(subcommand_negates_reqs = true)]
    Query(super::query::QueryArgs), // Use super:: path
    /// Show statistics about the vector database
    #[command(subcommand_negates_reqs = true)]
    Stats(super::stats::StatsArgs), // Use super:: path
    /// List indexed directories
    #[command(subcommand_negates_reqs = true)]
    List(super::list::ListArgs), // Use super:: path
    /// Clear all data or data for a specific directory
    #[command(subcommand_negates_reqs = true)]
    Clear(super::clear::ClearArgs), // Add Clear command
    /// Manage repositories (add, list, use, remove)
    #[command(subcommand_negates_reqs = true)]
    Repo(super::repo_commands::RepoArgs),
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
        Commands::Index(ref cmd_args) => super::index::handle_index(cmd_args, &args, config.clone(), client).await,
        Commands::Query(ref cmd_args) => super::query::handle_query(cmd_args, &args, config.clone(), client).await,
        Commands::Stats(cmd_args) => super::stats::handle_stats(cmd_args, config.clone(), client).await,
        Commands::List(cmd_args) => super::list::handle_list(cmd_args, config.clone(), client).await,
        Commands::Clear(cmd_args) => super::clear::handle_clear(cmd_args, config.clone(), client).await,
        Commands::Repo(ref cmd_args) => super::repo_commands::handle_repo_command(cmd_args.clone(), &args, config, client).await,
    }
}

// --- Helper Functions ---

// Helper function to create payload indices, ignoring errors if index already exists
pub async fn ensure_payload_index(
    client: &Qdrant,
    collection_name: &str,
    field_name: &str,
    field_type: FieldType,
) -> Result<()> {
    let request_builder = CreateFieldIndexCollectionBuilder::new(collection_name, field_name, field_type);

    let final_request = match field_type {
        FieldType::Keyword => request_builder.field_index_params(KeywordIndexParams::default()).build(),
        FieldType::Integer => request_builder.field_index_params(IntegerIndexParams::default()).build(),
        FieldType::Float => request_builder.field_index_params(FloatIndexParams::default()).build(),
        FieldType::Geo => request_builder.field_index_params(GeoIndexParams::default()).build(),
        FieldType::Text => request_builder.field_index_params(TextIndexParams {
            tokenizer: TokenizerType::Word.into(),
            lowercase: Some(true),
            min_token_len: None,
            max_token_len: None,
            on_disk: None,
        }).build(),
        FieldType::Bool => request_builder.field_index_params(BoolIndexParams::default()).build(),
        FieldType::Datetime => request_builder.field_index_params(DatetimeIndexParams::default()).build(),
        FieldType::Uuid => request_builder.field_index_params(UuidIndexParams::default()).build(),
    };

    match client.create_field_index(final_request).await {
        Ok(_) => {
            log::info!("Successfully created or confirmed index for field '{}'", field_name);
            Ok(())
        },
        Err(e) => {
            let error_string = e.to_string();
            if error_string.contains("already exists") || error_string.contains("exists already") {
                log::warn!("Index for field '{}' already exists.", field_name);
                Ok(())
            } else if error_string.contains("Collection") && error_string.contains("not found") {
                log::error!("Cannot create index because collection '{}' does not exist.", collection_name);
                Err(anyhow::anyhow!("Collection '{}' not found when creating index for '{}'.", collection_name, field_name).context(e))
            } else {
                Err(anyhow::anyhow!("Failed to create index for field '{}'", field_name).context(e))
            }
        }
    }
}

// --- Moved from index.rs ---

// Helper function to upsert points in batches with progress updates.
pub(crate) async fn upsert_batch(
    client: &Qdrant,
    collection_name: &str,
    points: Vec<PointStruct>,
    pb: &indicatif::ProgressBar,
) -> Result<()> {
    if points.is_empty() {
        return Ok(());
    }
    let count = points.len();
    log::debug!("Upserting batch of {} points to {}", count, collection_name);
    pb.set_message(format!("Upserting {} points...", count));
    
    // Use UpsertPointsBuilder as required by the API
    let request: UpsertPoints = UpsertPointsBuilder::new(collection_name, points)
        .wait(true) // Wait for the operation to complete
        .build();

    match client.upsert_points(request).await {
        Ok(response) => {
            if let Some(result) = response.result {
                match UpdateStatus::try_from(result.status) {
                    Ok(UpdateStatus::Completed) => {
                        // pb.inc handled in calling loop
                        log::debug!("Upsert batch successful.");
                        Ok(())
                    },
                    Ok(status) => {
                        let msg = format!("Qdrant upsert batch completed with status: {:?}", status);
                        pb.println(format!("Warning: {}", msg));
                        log::warn!("{}", msg);
                        Ok(()) // Still Ok, but log warning
                    },
                    Err(_) => {
                        let msg = format!("Qdrant upsert batch completed with unknown status code: {}", result.status);
                        pb.println(format!("Error: {}", msg));
                        log::error!("{}", msg);
                        Err(anyhow::anyhow!(msg))
                    }
                }
            } else {
                let msg = "Qdrant upsert response missing result status";
                pb.println(format!("Error: {}", msg));
                log::error!("{}", msg);
                Err(anyhow::anyhow!(msg))
            }
        }
        Err(e) => {
            let msg = format!("Failed to upsert batch to {}: {}", collection_name, e);
            pb.println(format!("Error: {}", msg));
            log::error!("{}", msg);
            // Use anyhow::Context for better error reporting
            Err(anyhow::anyhow!(msg).context(e))
        }
    }
}
