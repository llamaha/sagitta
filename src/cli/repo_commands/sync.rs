use anyhow::{anyhow, Context, Result};
use clap::Args;
use colored::*;
use git2::Repository;
use std::{path::PathBuf, sync::Arc};
use std::time::Instant;

use crate::config::{self, AppConfig};
use crate::cli::commands::CliArgs;
use crate::git;
use crate::vectordb::qdrant_client_trait::QdrantClientTrait;
use std::fmt::Debug;

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

    let _repo = Repository::open(&repo_config_clone.local_path)
        .with_context(|| format!("Failed to open repository at {}", repo_config_clone.local_path.display()))?;

    let options = git::SyncOptions {
        force: args.force,
        extensions: args.extensions.clone(),
    };

    match git::sync_repository(Arc::clone(&client), repo_config_clone, options, cli_args, config).await {
        Ok(sync_result) => {
            if sync_result.success {
                println!("Sync successful: {}", sync_result.message.green());
                if !sync_result.indexed_languages.is_empty() {
                    println!("Detected/updated languages: {}", sync_result.indexed_languages.join(", ").blue());
                    config.repositories[repo_config_index].indexed_languages = Some(sync_result.indexed_languages);
                }
                config::save_config(config, override_path)
                    .context("Failed to save config after successful sync")?;
            } else {
                println!("Sync completed with message: {}", sync_result.message.yellow());
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