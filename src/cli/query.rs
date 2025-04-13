use anyhow::{Context, Result};
use clap::Args;
use qdrant_client::{
    qdrant::{
        r#match::MatchValue, Condition, Filter, PayloadIncludeSelector,
        ReadConsistencyType, SearchPoints, SearchPointsBuilder,
        read_consistency::Value as ReadConsistencyValue,
    },
    Qdrant,
};
use std::{
    sync::Arc,
    path::PathBuf,
};

use crate::{
    cli::commands::{
        CODE_SEARCH_COLLECTION,
        FIELD_CHUNK_CONTENT,
        FIELD_ELEMENT_TYPE,
        FIELD_FILE_PATH,
        FIELD_LANGUAGE,
        FIELD_START_LINE,
        // FIELD_END_LINE is not strictly needed for display here but good to have if needed later
    },
    config::AppConfig,
    vectordb::{embedding, embedding_logic::EmbeddingHandler},
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

/// Handles the `query` command, generating embeddings and searching Qdrant.
pub async fn handle_query(
    cmd_args: &QueryArgs,
    cli_args: &CliArgs,
    config: &AppConfig,
) -> Result<()> {
    log::info!("Starting query process...");
    log::debug!("QueryArgs: {:?}, CliArgs: {:?}, Config: {:?}", cmd_args, cli_args, config);
    log::info!("Using Qdrant URL: {}", config.qdrant_url);

    // --- Resolve ONNX Paths (Keep existing logic) ---
    let model_path_str = cli_args.onnx_model_path_arg.as_ref().or(config.onnx_model_path.as_ref());
    let tokenizer_path_str = cli_args.onnx_tokenizer_dir_arg.as_ref().or(config.onnx_tokenizer_path.as_ref());
    let model_path_buf: Option<PathBuf> = model_path_str.map(PathBuf::from);
    let tokenizer_path_buf: Option<PathBuf> = tokenizer_path_str.map(PathBuf::from);
    log::debug!("Resolved model path for handler: {:?}", model_path_buf);
    log::debug!("Resolved tokenizer path for handler: {:?}", tokenizer_path_buf);

    // --- Initialize Embedding Handler ---
    log::info!("Initializing embedding handler...");
    let embedding_handler = Arc::new(
        EmbeddingHandler::new(
            embedding::EmbeddingModelType::Onnx,
            model_path_buf,
            tokenizer_path_buf,
        )
        .context("Failed to initialize embedding handler")?,
    );

    // --- Initialize Qdrant Client ---
    log::info!("Connecting to Qdrant...");
    let client = Qdrant::from_url(&config.qdrant_url).build()?;
    log::info!("Qdrant client connected.");

    // --- Generate Query Embedding ---
    let model = embedding_handler
        .create_embedding_model()
        .context("Failed to create embedding model for query")?;
    log::info!("Generating embedding for query: \"{}\"", cmd_args.query);
    let query_embedding = model
        .embed(&cmd_args.query)
        .context("Failed to generate query embedding")?;
    log::debug!("Generated query embedding dimension: {}", query_embedding.len());

    // --- Build Search Request ---
    log::info!("Building search request...");
    let mut search_builder = SearchPointsBuilder::new(
        CODE_SEARCH_COLLECTION,
        query_embedding,
        cmd_args.limit,
    );

    // Select necessary payload fields
    search_builder = search_builder.with_payload(PayloadIncludeSelector {
        fields: vec![
            FIELD_FILE_PATH.to_string(),
            FIELD_START_LINE.to_string(),
            // FIELD_END_LINE.to_string(), // Optional for display
            FIELD_LANGUAGE.to_string(),
            FIELD_ELEMENT_TYPE.to_string(),
            FIELD_CHUNK_CONTENT.to_string(), // Crucial for displaying snippet
        ],
    });

    search_builder = search_builder.with_vectors(false); // Don't need vectors in response

    // Set read consistency (optional, adjust as needed)
    let consistency_value = ReadConsistencyValue::Type(ReadConsistencyType::Majority as i32);
    search_builder = search_builder.read_consistency(consistency_value);

    // --- Build Filter based on CLI args ---
    let mut filters = Vec::new();

    if let Some(lang) = &cmd_args.lang {
        log::info!("Adding language filter: {}", lang);
        filters.push(Condition::matches(
            FIELD_LANGUAGE,
            MatchValue::from(lang.to_lowercase()),
        ));
    }

    if let Some(element_type) = &cmd_args.element_type {
        log::info!("Adding element type filter: {}", element_type);
        filters.push(Condition::matches(
            FIELD_ELEMENT_TYPE,
            MatchValue::from(element_type.to_lowercase()),
        ));
    }

    // Combine filters if multiple are present
    if !filters.is_empty() {
        let filter = Filter::must(filters); // Use MUST for multiple filters
        search_builder = search_builder.filter(filter);
        log::info!("Applied filter(s).");
    }

    let search_request: SearchPoints = search_builder.build();
    log::debug!("Final Search Request: {:?}", search_request); // Log the built request

    // --- Execute Search ---
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

    // --- Display Results ---
    for (idx, point) in search_result.result.into_iter().enumerate() {
        let payload = point.payload;

        let file_path = payload
            .get(FIELD_FILE_PATH)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "<unknown_file>".to_string());
        let start_line = payload
            .get(FIELD_START_LINE)
            .and_then(|v| v.as_integer())
            .map(|l| l as usize)
            .unwrap_or(0);
        let language = payload
            .get(FIELD_LANGUAGE)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        let element_type = payload
            .get(FIELD_ELEMENT_TYPE)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        let snippet = payload
            .get(FIELD_CHUNK_CONTENT)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "[Error: Snippet content missing from payload]".to_string());

        println!(
            "Result {}: Score: {:.4} | File: {} | Line: {} | Lang: {} | Type: {}",
            idx + 1,
            point.score,
            file_path,
            start_line, // Display the actual start line from the chunk
            language,
            element_type
        );

        // Print the full chunk content as the snippet
        println!("\n{}", snippet);

        println!("---------------");
    }

    log::info!("Query process finished successfully.");
    Ok(())
}