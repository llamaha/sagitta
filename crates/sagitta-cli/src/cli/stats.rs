//!
//! Top-level commands for the CLI application.

use anyhow::{anyhow, Context, Result};
use clap::Args;
use colored::*;
use std::sync::Arc;
use sagitta_search::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use std::path::PathBuf;
use qdrant_client::qdrant::CountPointsBuilder;
use serde::Serialize;
use serde_json;
use sagitta_search::repo_helpers::get_branch_aware_collection_name;
use git_manager::GitRepository;

// use super::commands::CODE_SEARCH_COLLECTION; // REMOVED

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct StatsArgs {
    /// Optional: Path to the configuration file.
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    pub config_file: Option<PathBuf>,
    // Removed unused optional repo name
    // /// Optional: Name of the repository to show stats for (defaults to active repository).
    // #[arg(long)]
    // pub repo_name: Option<String>,

    /// Output statistics in JSON format.
    #[arg(long)]
    pub json: bool,
}

// Define a simplified serializable struct for collection info
#[derive(Serialize)]
struct CollectionInfoStats {
    status: String,
    vectors_count: u64,
    segments_count: u64,
}

// Define a serializable struct for JSON output
#[derive(Serialize)]
struct CollectionStats {
    collection_name: String,
    repository_name: String,
    info: Option<CollectionInfoStats>,
    exact_point_count: Option<u64>,
    error: Option<String>,
}

// Accept qdrant_url as parameter
/// Handles the `stats` command, fetching and displaying collection statistics from Qdrant.
pub async fn handle_stats<C>(
    args: StatsArgs,
    config: AppConfig,
    client: Arc<C>,
    _cli_args: &crate::cli::CliArgs,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    // Stats command processing

    let active_repo_name_opt = config.active_repository.clone(); 
    let target_repo_name = match active_repo_name_opt {
        Some(name) => name,
        None => {
            if config.repositories.len() == 1 {
                config.repositories[0].name.clone()
            } else if config.repositories.is_empty() {
                anyhow::bail!("No repositories found. Add one using 'repo add'.");
            } else {
                anyhow::bail!("No active repository set and multiple repositories exist. Please specify a repository with 'repo use <name>'.");
            }
        }
    };

    let repo_config = config.repositories.iter()
        .find(|r| r.name == target_repo_name)
        .ok_or_else(|| anyhow!("Repository '{}' not found.", target_repo_name))?;

    // Get the current branch from filesystem, just like sync does
    let branch_name = if let Some(target_ref) = repo_config.target_ref.as_deref() {
        // If target_ref is specified, use that
        target_ref.to_string()
    } else {
        // Otherwise, get the current branch from the filesystem
        match GitRepository::open(&repo_config.local_path) {
            Ok(git_repo) => {
                match git_repo.current_branch() {
                    Ok(current_branch) => current_branch,
                    Err(e) => {
                        // If we can't get the current branch, try common defaults
                        if !args.json {
                            eprintln!("Warning: Could not determine current branch: {}. Using 'main' as default.", e);
                        }
                        "main".to_string()
                    }
                }
            }
            Err(e) => {
                // If we can't open the repository, try common defaults
                if !args.json {
                    eprintln!("Warning: Could not open repository: {}. Using 'main' as default.", e);
                }
                "main".to_string()
            }
        }
    };

    // Use branch-aware collection naming to match the new sync behavior
    let collection_name = get_branch_aware_collection_name(&repo_config.name, &branch_name, &config);

    if !args.json {
        println!("Fetching stats for repository: {}", repo_config.name.cyan());
        println!("Fetching statistics for collection: {}", collection_name.cyan());
    }

    let collection_info_result = client
        .get_collection_info(collection_name.clone())
        .await
        .context(format!("Failed to retrieve collection info for '{collection_name}'"));

    if args.json {
        let info = collection_info_result.as_ref().ok().map(|info| CollectionInfoStats {
            status: info.status.to_string(),
            vectors_count: info.vectors_count.unwrap_or(0),
            segments_count: info.segments_count,
        });
        
        let count_result_opt = if collection_info_result.is_ok() {
            let count_request = CountPointsBuilder::new(&collection_name).exact(true).build();
            client.count(count_request).await.ok()
        } else {
            None
        };
        
        let exact_count = count_result_opt.as_ref().and_then(|r| r.result.as_ref().map(|c| c.count));
        let error_msg = collection_info_result.as_ref().err().map(|e| e.to_string());

        let stats = CollectionStats {
            collection_name: collection_name.clone(),
            repository_name: repo_config.name.clone(),
            info,
            exact_point_count: exact_count,
            error: error_msg,
        };
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(());
    } else {
        match collection_info_result {
            Ok(info) => {
                let count_result_opt = {
                    let count_request = CountPointsBuilder::new(&collection_name).exact(true).build();
                    client.count(count_request).await.ok()
                };

                let exact_count = match count_result_opt {
                    Some(count_result) => count_result.result.map(|r| r.count).unwrap_or(0),
                    None => 0,
                };

                let info_status = info.status.to_string();
                let vectors_count = info.vectors_count.unwrap_or(0); 
                let segments_count = info.segments_count;

                println!("  Status: {}", info_status.green());
                println!("  Point count: {}", exact_count.to_string().yellow());
                println!("  Vector count: {}", vectors_count.to_string().yellow());
                println!("  Segments: {}", segments_count.to_string().yellow());
            }
            Err(e) => {
                eprintln!("{}", "  Error: Could not retrieve collection info (collection might not exist yet). Run 'repo sync'?".red());
                return Err(e.context(format!("Failed to get collection info for '{collection_name}'")));
            }
        }
    }

    Ok(())
} 