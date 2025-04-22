use anyhow::{anyhow, Result};
use clap::Args;
use colored::*;
use std::{path::PathBuf, sync::Arc};
use std::time::Instant;
use log::{info, warn, error};

use vectordb_core::{AppConfig, save_config};
use crate::cli::commands::CliArgs;
use vectordb_core::qdrant_client_trait::QdrantClientTrait;
use std::fmt::Debug;
use futures::StreamExt;
use crate::git::SyncOptions;

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct SyncRepoArgs {
    /// Optional name of the repository to sync (defaults to active repository).
    pub name: Option<String>,

    /// Force re-indexing even if the remote commit hasn't changed.
    #[arg(long, default_value_t = false)]
    pub force: bool,

    /// Optional: Comma-separated list of file extensions to index (e.g., "rs,toml").
    #[arg(short = 'e', long, value_delimiter = ',')]
    pub extensions: Option<Vec<String>>,
}

pub async fn handle_repo_sync<C>(
    args: SyncRepoArgs, 
    cli_args: &CliArgs,
    config: &mut AppConfig,
    client: Arc<C>,
    override_path: Option<&PathBuf>,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let start_time = Instant::now();
    let repo_name = args.name.as_ref().or(config.active_repository.as_ref())
        .ok_or_else(|| anyhow::anyhow!("No active repository set and no repository name provided with --name."))?;

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == *repo_name)
        .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found in configuration.", repo_name))?;
    
    let repo_config_clone = config.repositories[repo_config_index].clone();

    let active_branch = repo_config_clone.active_branch
        .as_ref()
        .ok_or_else(|| anyhow!("Repository '{}' has no active branch set. Use 'use-branch' command.", repo_name))?;

    println!(
        "Syncing repository '{}' (Branch: {})...", 
        repo_name.cyan(), 
        active_branch.cyan()
    );

    let options = SyncOptions {
        force: args.force,
        extensions: args.extensions.clone(),
    };

    match crate::git::sync_repository(
        Arc::clone(&client), 
        repo_config_clone, 
        options, 
        cli_args,
        config,
    ).await {
        Ok(sync_result) => {
            if sync_result.success {
                info!(
                    "{}",
                    format!("Successfully synced repository '{}'", repo_name).green()
                );
                if !sync_result.indexed_languages.is_empty() {
                    println!("Detected/updated languages: {}", sync_result.indexed_languages.join(", ").blue());
                    config.repositories[repo_config_index].indexed_languages = Some(sync_result.indexed_languages);
                }
                if let Err(e) = save_config(config, override_path) {
                    error!("Failed to save config after sync: {}", e);
                }
                println!("{}", "Configuration saved.".dimmed());
            } else {
                warn!(
                    "{}",
                    match (sync_result.success, sync_result.indexed_languages.is_empty()) {
                        (true, true) => format!(
                            "Sync completed for repository '{}'. New commits were indexed. Message: {}",
                            repo_name,
                            sync_result.message
                        )
                        .green(),
                        (false, true) => format!(
                            "Sync completed for repository '{}'. No new commits were found, but existing data was updated. Message: {}",
                            repo_name,
                            sync_result.message
                        )
                        .yellow(),
                        (true, false) => format!(
                            "Sync completed for repository '{}'. New commits were indexed, but some data failed to update. Message: {}",
                            repo_name,
                            sync_result.message
                        )
                        .yellow(),
                        (false, false) => format!(
                            "Sync completed for repository '{}', but no new changes were detected or indexed. Message: {}",
                            repo_name,
                            sync_result.message
                        )
                        .yellow(),
                    }
                );
                if let Err(e) = save_config(config, override_path) {
                    error!("Failed to save config after sync message: {}", e);
                }
            }
        }
        Err(e) => {
            return Err(anyhow!("Failed to sync repository '{}': {}", repo_name, e));
        }
    }

    let duration = start_time.elapsed();
    println!("Sync operation finished in {:.2?}s.", duration.as_secs_f32());

    Ok(())
} 