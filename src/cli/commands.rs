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
// Removed: use std::sync::Arc;
use clap::{Parser, Subcommand};
use qdrant_client::{
    // Removed: client::QdrantClient,
    qdrant::{
        FieldType, PointStruct, TextIndexParams, KeywordIndexParams, IntegerIndexParams, 
        FloatIndexParams, GeoIndexParams, BoolIndexParams, DatetimeIndexParams, 
        UuidIndexParams, TokenizerType, UpdateStatus, 
        CreateFieldIndexCollectionBuilder,
        UpsertPointsBuilder,
        // Removed unused: CreateFieldIndexCollection,
        UpsertPoints,
    },
};
use indicatif;

// Import config
use crate::config::AppConfig;

// CliArgs struct definition moved here from main binary
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,

    /// Path to ONNX model file (overrides config & env var)
    #[arg(long = "onnx-model", global = true, env = "VECTORDB_ONNX_MODEL")]
    pub onnx_model_path_arg: Option<String>,

    /// Path to ONNX tokenizer config directory (overrides config & env var)
    #[arg(long = "onnx-tokenizer-dir", global = true, env = "VECTORDB_ONNX_TOKENIZER_DIR")]
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
pub(crate) const CODE_SEARCH_COLLECTION: &str = "vectordb-code-search";
pub(crate) const FIELD_FILE_PATH: &str = "file_path";
pub(crate) const FIELD_DIR_PATH: &str = "dir_path"; // Root directory provided to index command
pub(crate) const FIELD_START_LINE: &str = "start_line";
pub(crate) const FIELD_END_LINE: &str = "end_line";
pub(crate) const FIELD_FILE_EXTENSION: &str = "file_extension";
pub(crate) const FIELD_LANGUAGE: &str = "language";
pub(crate) const FIELD_ELEMENT_TYPE: &str = "element_type";
pub(crate) const FIELD_CHUNK_CONTENT: &str = "chunk_content";

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
}

// --- Main Command Handler Function ---
/// Parses the command-line arguments and dispatches to the appropriate command handler.
///
/// # Arguments
/// * `args` - The parsed top-level command line arguments ([`CliArgs`]).
/// * `config` - The loaded application configuration ([`AppConfig`]).
pub async fn handle_command(
    args: CliArgs, 
    config: AppConfig, 
) -> Result<()> { 
    match args.command {
        // Pass args struct and config to handlers
        Commands::Index(ref cmd_args) => super::index::handle_index(cmd_args, &args, &config).await,
        Commands::Query(ref cmd_args) => super::query::handle_query(cmd_args, &args, &config).await,
        // These commands don't need args or config beyond their specific Args and the qdrant_url
        Commands::Stats(cmd_args) => super::stats::handle_stats(cmd_args, &config.qdrant_url).await, 
        Commands::List(cmd_args) => super::list::handle_list(cmd_args, &config.qdrant_url).await, 
        Commands::Clear(cmd_args) => super::clear::handle_clear(cmd_args, &config.qdrant_url).await, 
    }
}

// --- Helper Functions ---

// Helper function to create payload indices, ignoring errors if index already exists
pub(crate) async fn ensure_payload_index(
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

// Helper to upsert a batch of points
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
    let request: UpsertPoints = UpsertPointsBuilder::new(collection_name, points)
        .wait(true)
        .build();

    match client.upsert_points(request).await {
        Ok(response) => {
            if let Some(result) = response.result {
                match UpdateStatus::try_from(result.status) {
                    Ok(UpdateStatus::Completed) => {
                        pb.inc(count as u64);
                        Ok(())
                    },
                    Ok(status) => {
                        let msg = format!("Qdrant upsert batch completed with status: {:?}", status);
                        pb.println(&msg);
                        log::warn!("{}", msg);
                        Ok(())
                    },
                    Err(_) => {
                        let msg = format!("Qdrant upsert batch completed with unknown status code: {}", result.status);
                        pb.println(&msg);
                        log::error!("{}", msg);
                        Err(anyhow::anyhow!(msg))
                    }
                }
            } else {
                let msg = "Qdrant upsert response missing result status";
                pb.println(msg);
                log::error!("{}", msg);
                Err(anyhow::anyhow!(msg))
            }
        }
        Err(e) => {
            let msg = format!("Failed to upsert batch: {}", e);
            pb.println(&msg);
            log::error!("{}", msg);
            Err(anyhow::anyhow!(msg).context(e))
        }
    }
}
