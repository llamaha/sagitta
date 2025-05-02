use anyhow::{anyhow, Result};
use clap::Args;
use colored::*;
use std::{path::PathBuf, sync::Arc};
use std::time::Instant;
use log::{info, warn, error};

use vectordb_core::{AppConfig, save_config};
use vectordb_core::qdrant_client_trait::QdrantClientTrait;
use std::fmt::Debug;
use vectordb_core::sync::{sync_repository, SyncOptions, SyncResult};

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
    config: &mut AppConfig,
    client: Arc<C>,
    override_path: Option<&PathBuf>,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let start_time = Instant::now();
    let repo_name = args.name.as_ref().or(config.active_repository.as_ref())
        .ok_or_else(|| anyhow!("No active repository set and no repository name provided with --name."))?
        .clone();

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == *repo_name)
        .ok_or_else(|| anyhow!("Repository '{}' not found in configuration.", repo_name))?;
    
    let repo_config = &config.repositories[repo_config_index];
    
    let app_config_clone = config.clone();

    let active_branch_str = repo_config.active_branch
        .as_ref()
        .ok_or_else(|| anyhow!("Repository '{}' has no active branch set. Use 'use-branch' command.", repo_name))?;
    let target_ref_str_opt = repo_config.target_ref.clone();
    let current_sync_identifier = target_ref_str_opt.as_deref().unwrap_or(active_branch_str).to_string();

    println!(
        "Syncing repository '{}' (Branch/Ref: {})...", 
        repo_name.cyan(), 
        current_sync_identifier.cyan()
    );

    let options = SyncOptions {
        force: args.force,
        extensions: args.extensions.clone(),
    };

    match sync_repository(
        Arc::clone(&client), 
        repo_config,
        options, 
        &app_config_clone,
    ).await {
        Ok(sync_result) => {
            if sync_result.success {
                info!(
                    "{}",
                    format!("Successfully synced repository '{}'", repo_name).green()
                );
                println!("{}", sync_result.message.green());
                 println!(
                    "Files Indexed: {}, Files Deleted: {}", 
                    sync_result.files_indexed.to_string().yellow(), 
                    sync_result.files_deleted.to_string().yellow()
                );
                if !sync_result.indexed_languages.is_empty() {
                    println!("Detected/updated languages: {}", sync_result.indexed_languages.join(", ").blue());
                }
                
                if let Some(commit) = sync_result.last_synced_commit {
                     if let Some(repo_mut) = config.repositories.get_mut(repo_config_index) {
                        repo_mut.last_synced_commits.insert(current_sync_identifier.clone(), commit);
                        repo_mut.indexed_languages = Some(sync_result.indexed_languages);
                     } else {
                         error!("Failed to get mutable repository config to update sync status.");
                     }
                } else {
                     warn!("Sync successful but no commit hash returned to update config.");
                }

                if let Err(e) = save_config(config, override_path) {
                    error!("Failed to save config after sync: {}", e);
                    println!("{}", "Warning: Failed to save configuration after successful sync.".red());
                } else {
                     println!("{}", "Configuration saved.".dimmed());
                }
            } else {
                warn!("Sync report for '{}': {}", repo_name, sync_result.message);
                println!("{}", sync_result.message.yellow());
                 println!(
                    "Files Indexed: {}, Files Deleted: {}", 
                    sync_result.files_indexed.to_string().yellow(), 
                    sync_result.files_deleted.to_string().yellow()
                );
            }
        }
        Err(e) => {
             error!("Sync failed for repository '{}': {:?}", repo_name, e);
             println!("{}", format!("Error during sync for repository '{}'.", repo_name).red());
             println!("{}", format!("  Details: {}", e).red());
            return Err(anyhow!(e).context(format!("Failed to sync repository '{}'", repo_name)));
        }
    }

    let duration = start_time.elapsed();
    println!("Sync operation finished in {:.2?}s.", duration.as_secs_f32());

    Ok(())
} 