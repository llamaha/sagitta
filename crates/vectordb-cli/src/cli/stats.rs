//!
//! Top-level commands for the CLI application.

use anyhow::{anyhow, Context, Result};
use clap::Args;
use colored::*;
use std::sync::Arc;
use vectordb_core::AppConfig;
use vectordb_core::repo_helpers::get_collection_name;
use vectordb_core::qdrant_client_trait::QdrantClientTrait;
use std::path::PathBuf;
use qdrant_client::qdrant::CountPointsBuilder;
use serde::Serialize;
use serde_json;

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
    cli_args: &crate::cli::CliArgs,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let cli_tenant_id = match cli_args.tenant_id.as_deref() {
        Some(id) => id,
        None => {
            anyhow::bail!("--tenant-id is required to get repository stats.");
        }
    };

    let active_repo_name_opt = config.active_repository.clone(); 
    let target_repo_name = match active_repo_name_opt {
        Some(name) => name,
        None => {
            let tenant_repos: Vec<&vectordb_core::config::RepositoryConfig> = config.repositories.iter()
                .filter(|r| r.tenant_id.as_deref() == Some(cli_tenant_id))
                .collect();
            if tenant_repos.len() == 1 {
                tenant_repos[0].name.clone()
            } else if tenant_repos.is_empty() {
                anyhow::bail!("No repositories found for tenant '{}'. Add one using 'repo add --tenant-id {}'.", cli_tenant_id, cli_tenant_id);
            } else {
                anyhow::bail!("No active repository set for tenant '{}' and multiple repositories exist. Please specify a repository with 'repo use <name> --tenant-id {}' or use the implicit single repository for the tenant.", cli_tenant_id, cli_tenant_id);
            }
        }
    };

    let repo_config = config.repositories.iter()
        .find(|r| r.name == target_repo_name && r.tenant_id.as_deref() == Some(cli_tenant_id))
        .ok_or_else(|| anyhow!("Repository '{}' not found for tenant '{}', or active repository does not belong to this tenant.", target_repo_name, cli_tenant_id))?;

    let collection_name = get_collection_name(cli_tenant_id, &repo_config.name, &config);

    if !args.json {
        println!("Fetching stats for repository: {} (Tenant: {})", repo_config.name.cyan(), cli_tenant_id.cyan());
        println!("Fetching statistics for collection: {}", collection_name.cyan());
    }

    let collection_info_result = client
        .get_collection_info(collection_name.clone())
        .await
        .context(format!("Failed to retrieve collection info for '{}'", collection_name));

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
                return Err(e.context(format!("Failed to get collection info for '{}'", collection_name)));
            }
        }
    }

    Ok(())
} 