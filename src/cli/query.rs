use anyhow::{Context, Result};
use clap::Args;
use qdrant_client::{
    Qdrant,
    qdrant::{
        Condition, Filter,
        SearchPoints,
        ReadConsistencyType,
        read_consistency::Value as ReadConsistencyValue,
        SearchPointsBuilder, PayloadIncludeSelector,
    },
};
use qdrant_client::qdrant::r#match::MatchValue;
use std::{
    sync::Arc,
    path::PathBuf,
};

use crate::{
    cli::commands::{CODE_SEARCH_COLLECTION, FIELD_FILE_EXTENSION, FIELD_FILE_PATH, FIELD_START_LINE, FIELD_END_LINE},
    // Commented out missing highlight import:
    // highlight::highlight_text,
    vectordb::{embedding, embedding_logic::EmbeddingHandler},
    config::AppConfig,
};
use super::commands::CliArgs;

const MAX_RESULTS: u64 = 10;

#[derive(Args, Debug)]
pub struct QueryArgs {
    /// The search query string
    #[arg(required = true)]
    pub query: String,

    /// Maximum number of results to return
    #[arg(short, long, default_value_t = 10)]
    pub limit: u64,

    /// Optional file extensions to filter by (e.g., ".rs", ".py")
    #[arg(short = 't', long = "type")]
    pub file_types: Option<Vec<String>>,

    /// Context lines before and after the matched line in the snippet
    #[arg(long, default_value_t = 2)]
    pub context: usize,
}

/// Handles the `query` command, generating embeddings and searching Qdrant.
pub async fn handle_query(
    cmd_args: &QueryArgs, 
    cli_args: &CliArgs,
    config: &AppConfig
) -> Result<()> {
    log::info!("Starting query process...");
    log::debug!("QueryArgs: {:?}", cmd_args);
    log::debug!("CliArgs: {:?}", cli_args);
    log::debug!("Config: {:?}", config);
    log::info!("Using Qdrant URL: {}", config.qdrant_url);

    log::info!("Initializing embedding handler...");
    
    // --- Resolve ONNX Paths (CLI > Env (implicitly handled by clap) > Config) ---
    // Use cli_args first, then fall back to config
    let model_path_str = cli_args.onnx_model_path_arg.as_ref().or(config.onnx_model_path.as_ref());
    let tokenizer_path_str = cli_args.onnx_tokenizer_dir_arg.as_ref().or(config.onnx_tokenizer_path.as_ref());

    let model_path_buf: Option<PathBuf> = model_path_str.map(PathBuf::from);
    let tokenizer_path_buf: Option<PathBuf> = tokenizer_path_str.map(PathBuf::from);

    log::debug!("Resolved model path for handler: {:?}", model_path_buf);
    log::debug!("Resolved tokenizer path for handler: {:?}", tokenizer_path_buf);

    let embedding_handler = Arc::new(
        EmbeddingHandler::new(
            embedding::EmbeddingModelType::Onnx,
            model_path_buf, // Use resolved path
            tokenizer_path_buf, // Use resolved path
        )
        .context("Failed to initialize embedding handler")?,
    );

    log::info!("Connecting to Qdrant...");
    let client = Qdrant::from_url(&config.qdrant_url).build()?;
    log::info!("Qdrant client connected.");

    // Create the embedding model once
    let model = embedding_handler.create_embedding_model()
        .context("Failed to create embedding model for query")?;
    
    // Generate the actual query embedding
    log::info!("Generating embedding for query: \"{}\"", cmd_args.query);
    let query_embedding = model.embed(&cmd_args.query)
        .context("Failed to generate query embedding")?;
    log::debug!("Generated query embedding dimension: {}", query_embedding.len());

    log::info!("Building search request...");
    let mut search_builder = SearchPointsBuilder::new(
        CODE_SEARCH_COLLECTION,
        query_embedding,
        cmd_args.limit
    );
    
    search_builder = search_builder.with_payload(PayloadIncludeSelector{
        fields: vec![
             FIELD_FILE_PATH.to_string(),
             FIELD_START_LINE.to_string(),
             FIELD_END_LINE.to_string(),
         ],
    });
        
    search_builder = search_builder.with_vectors(false);

    // Optional: Set read consistency (adjust as needed)
    // Construct the `read_consistency::Value` enum variant directly
    let consistency_value = ReadConsistencyValue::Type(
        ReadConsistencyType::Majority as i32
    );
    search_builder = search_builder.read_consistency(consistency_value);

    // Add filter conditions based on file types if provided
    if let Some(file_types) = &cmd_args.file_types {
        if !file_types.is_empty() {
            log::info!("Adding file type filter for: {:?}", file_types);
            let conditions: Vec<Condition> = file_types
                .iter()
                .map(|ft| {
                    let extension = ft.trim_start_matches('.').to_lowercase();
                    Condition::matches(FIELD_FILE_EXTENSION, MatchValue::from(extension))
                })
                .collect();

            let filter = if conditions.len() == 1 {
                Filter::must(conditions)
            } else {
                 Filter::should(conditions)
            };
            search_builder = search_builder.filter(filter);
            log::info!("Applied filter.");
        }
    }

    let search_request: SearchPoints = search_builder.build();
    log::info!("Search request built.");

    log::info!("Executing search against Qdrant...");
    let search_result = client
        .search_points(search_request)
        .await
        .context("Failed to execute search query")?;
    log::info!("Search returned {} results.", search_result.result.len());

    if search_result.result.is_empty() {
        println!("No results found.");
        return Ok(());
    }

    // Display results
    for (idx, point) in search_result.result.into_iter().enumerate() {
        let payload = point.payload;
        let file_path_val = payload.get(FIELD_FILE_PATH);
        let start_line_val = payload.get(FIELD_START_LINE);

        let file_path = match file_path_val.and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                log::warn!("Result {} missing file_path", idx + 1);
                "<unknown_file>"
            }
        };
        let start_line = match start_line_val.and_then(|v| v.as_integer()) {
            Some(l) => l as usize,
            None => {
                log::warn!("Result {} missing start_line for file {}", idx + 1, file_path);
                0
            }
        };

        println!(
            "Result {}: Score: {:.4} | File: {} | Line: {} ",
            idx + 1,
            point.score,
            file_path,
            start_line + 1
        );

        // Extract snippet using the function from the module
        match crate::vectordb::snippet_extractor::extract_snippet(file_path, start_line, cmd_args.context) {
            Ok(snippet) => {
                // Apply highlighting (commented out)
                // let highlighted_snippet = highlight_text(&snippet);
                // println!("\n{}", highlighted_snippet);
                println!("\n{}", snippet); // Print raw snippet for now
            }
            Err(e) => {
                log::error!("Error extracting snippet for {}:{}: {}", file_path, start_line + 1, e);
                println!("  [Error extracting snippet: {}]", e);
            }
        }
        println!("---------------");
    }

    log::info!("Query process finished successfully.");
    Ok(())
} 