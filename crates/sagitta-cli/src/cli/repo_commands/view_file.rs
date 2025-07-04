use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;
use serde_json::json;
use log;

use sagitta_search::config::AppConfig;
use crate::cli::utils::get_active_repo_config;
use sagitta_search::fs_utils::read_file_range;

#[derive(Args, Debug, Clone)]
pub struct ViewFileArgs {
    /// Relative path to the file within the repository.
    #[arg(required = true)]
    pub file_path: PathBuf,

    /// Start line number (1-based, inclusive).
    #[arg(long)]
    pub start_line: Option<usize>,

    /// End line number (1-based, inclusive).
    #[arg(long)]
    pub end_line: Option<usize>,

    /// Output result in JSON format (includes file content and metadata).
    #[arg(long)]
    pub json: bool,

    /// Optional: Specify the repository name (overrides active repo).
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(serde::Serialize)] // Needed for JSON output
struct FileViewResult {
    repository: String,
    relative_path: String,
    absolute_path: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
    content: String,
}

pub async fn handle_repo_view_file(
    args: ViewFileArgs,
    config: &AppConfig,
) -> Result<()> {
    log::debug!("Handling repo view-file with args: {:?}", args);

    let repo_config = get_active_repo_config(config, args.name.as_deref())?;
    let base_path = &repo_config.local_path;
    
    // Assume file_path is relative to the repository root.
    // Security check: Ensure the resolved path is still within the base_path.
    // std::fs::canonicalize can help resolve `..` etc., then check if it starts_with base_path.
    // For simplicity now, just join.
    let absolute_path = base_path.join(&args.file_path);
    log::debug!("Attempting to view absolute path: {}", absolute_path.display());
    
    // Add canonicalization and safety check
    let canonical_base = base_path.canonicalize()
        .with_context(|| format!("Failed to canonicalize base path: {}", base_path.display()))?;
    let canonical_target = absolute_path.canonicalize()
        .with_context(|| format!("Failed to canonicalize target path: {}", absolute_path.display()))?;

    if !canonical_target.starts_with(&canonical_base) {
         return Err(anyhow::anyhow!("Attempted path traversal detected. Target path is outside the repository root."));
    }

    // Use the canonicalized target path for reading
    let content = read_file_range(&canonical_target, args.start_line, args.end_line)
        .with_context(|| format!("Failed to read file content from {}", canonical_target.display()))?;

    if args.json {
        let result = FileViewResult {
            repository: repo_config.name.clone(),
            relative_path: args.file_path.to_string_lossy().to_string(),
            absolute_path: canonical_target.to_string_lossy().to_string(),
            start_line: args.start_line,
            end_line: args.end_line,
            content,
        };
        // Consider using serde_json::to_string_pretty for better readability
        println!("{}", json!(result));
    } else {
        // Simple print for non-JSON output
        println!("{}", content);
    }
    
    Ok(())
} 