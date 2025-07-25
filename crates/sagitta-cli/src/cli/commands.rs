// use crate::sagitta::embedding::EmbeddingModelType;
// use crate::sagitta::search::Search; // Removed
// use crate::sagitta::cache::CacheCheckResult; // Removed
use anyhow::Result;
use clap::Parser;
// Removed: use log::{debug, warn};
// Removed: use num_cpus;
// Removed: use rayon;
// Removed: use std::path::PathBuf;
// Removed: use std::time::Instant;
// use crate::sagitta::search::result::SearchResult; // Removed
// use crate::sagitta::search::{chunking, snippet}; // Removed
// use std::collections::HashMap; // Removed
// use std::collections::HashSet; // Removed
// use walkdir::WalkDir; // Removed
// use chrono::{DateTime, Utc, TimeZone, Local}; // Removed DateTime, TimeZone, Local

// Import Qdrant client types
// use qdrant_client::client::QdrantClient; // Old import
use qdrant_client::Qdrant; // Import the Qdrant struct
use std::sync::Arc;
use clap::{Subcommand};

// Import config
// use crate::cli::server::ServerArgs; // Removed - Module/Struct likely moved or removed

// Use config types from sagitta_search
use sagitta_search::AppConfig;
// Import constants from core
pub use sagitta_search::constants::{
    FIELD_FILE_PATH, FIELD_START_LINE, FIELD_END_LINE, FIELD_FILE_EXTENSION,
    FIELD_LANGUAGE, FIELD_ELEMENT_TYPE, FIELD_CHUNK_CONTENT, FIELD_BRANCH, FIELD_COMMIT_HASH,
    BATCH_SIZE // Using BATCH_SIZE from core now
};

// Moved from index.rs
// pub(crate) const BATCH_SIZE: usize = 128; // REMOVED - Use core constant

// CliArgs struct definition moved here from main binary
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,

    /// Path to ONNX model file (overrides config & env var)
    #[arg(short = 'm', long = "onnx-model", global = true, env = "SAGITTA_ONNX_MODEL")]
    pub onnx_model_path_arg: Option<String>,

    /// Path to ONNX tokenizer config directory (overrides config & env var)
    #[arg(short = 't', long = "onnx-tokenizer-dir", global = true, env = "SAGITTA_ONNX_TOKENIZER_DIR")]
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
// pub(crate) const CODE_SEARCH_COLLECTION: &str = "sagitta-code-search"; // REMOVED
// pub const FIELD_FILE_PATH: &str = "file_path"; // REMOVED - Use core constant
// pub(crate) const FIELD_DIR_PATH: &str = "dir_path"; // REMOVED
// pub const FIELD_START_LINE: &str = "start_line"; // REMOVED - Use core constant
// pub const FIELD_END_LINE: &str = "end_line"; // REMOVED - Use core constant
// pub const FIELD_FILE_EXTENSION: &str = "file_extension"; // REMOVED - Use core constant
// pub const FIELD_LANGUAGE: &str = "language"; // REMOVED - Use core constant
// pub const FIELD_ELEMENT_TYPE: &str = "element_type"; // REMOVED - Use core constant
// pub const FIELD_CHUNK_CONTENT: &str = "chunk_content"; // REMOVED - Use core constant

// Moved to simple/mod.rs or repo_commands.rs potentially
// pub(crate) const DEFAULT_VECTOR_DIMENSION: u64 = 384; 
// pub const LEGACY_INDEX_COLLECTION: &str = "sagitta-code-search"; // REMOVED - SIMPLE_INDEX_COLLECTION covers this

// Fields specific to repository indexing - REMOVED, use core constants
// pub const FIELD_BRANCH: &str = "branch";
// pub const FIELD_COMMIT_HASH: &str = "commit_hash";

// Collection name for the simple/legacy index command
// pub const SIMPLE_INDEX_COLLECTION: &str = "sagitta-code-search"; // REMOVED - Not used here
pub const LEGACY_INDEX_COLLECTION: &str = "sagitta-code-search"; // Keep for simple index command

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
    /// Initialize a new configuration file (with tenant_id, etc.)
    Init,
}

// --- New Function for Init Command ---
pub async fn execute_init_command(config: &mut AppConfig) -> Result<()> {
    let config_path = sagitta_search::config::get_config_path_or_default(None)?;
    let config_path_buf = config_path.clone(); // Keep a PathBuf version for saving
    let config_path_display = config_path_buf.display();

    if !config_path.exists() {
        println!("Configuration file not found at {config_path_display}, creating a new one.");
        // Ensure directory exists if creating for the first time
        if let Some(parent_dir) = config_path.parent() {
            std::fs::create_dir_all(parent_dir)?;
        }
    } else {
        println!("Existing configuration found at {config_path_display}. Checking tenant_id.");
    }

    // Configuration loaded successfully

    // Save the potentially modified config
    sagitta_search::config::save_config(config, Some(&config_path_buf))?;

    println!("Configuration saved to {config_path_display}.\nEnsure ONNX model/tokenizer paths and Qdrant URL are correctly set.");
    Ok(())
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
        Commands::Init => {
            // This arm should ideally not be reached if `main.rs` handles `Init` and exits early.
            // Adding it to make the match exhaustive.
            unreachable!("Init command should be handled in main.rs and not reach handle_command");
        }
    }
}

// --- Helper Functions ---

// REMOVED ensure_payload_index (moved to sagitta-search/src/qdrant_ops.rs)


// REMOVED upsert_batch (now unused in CLI, core version is used by indexing functions)
// pub(crate) async fn upsert_batch(
//     client: &Qdrant,
//     collection_name: &str,
//     points: Vec<PointStruct>,
//     _batch_num: usize,
//     _total_batches: usize,
//     progress_bar: &ProgressBar,
// ) -> Result<()> { ... }
