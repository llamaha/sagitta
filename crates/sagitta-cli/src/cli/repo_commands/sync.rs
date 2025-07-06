use anyhow::{anyhow, Result};
use clap::Args;
use colored::*;
use std::{path::PathBuf, sync::Arc};
use std::time::Instant;
use log::error;

use git_manager::{GitManager, SyncType};
use sagitta_search::{AppConfig, save_config};
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use std::fmt::Debug;
use sagitta_search::sync::{sync_repository, SyncOptions};
use crate::progress::IndicatifProgressReporter;

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
    _cli_args: &crate::cli::CliArgs,
    override_path: Option<&PathBuf>,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let start_time = Instant::now();

    let repo_name_str = args.name.as_ref().or(config.active_repository.as_ref())
        .ok_or_else(|| anyhow!("No active repository set and no repository name provided with --name."))?
        .clone();

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name_str)
        .ok_or_else(|| anyhow!("Repository '{}' not found in configuration.", repo_name_str))?;
    
    let repo_config = &config.repositories[repo_config_index];
    let repo_path = PathBuf::from(&repo_config.local_path);
    
    let app_config_clone = config.clone();

    let active_branch_str = repo_config.active_branch
        .as_ref()
        .ok_or_else(|| anyhow!("Repository '{}' has no active branch set. Use 'use-branch' command.", repo_name_str))?;
    let target_ref_str_opt = repo_config.target_ref.clone();
    let current_sync_identifier = target_ref_str_opt.as_deref().unwrap_or(active_branch_str).to_string();

    println!(
        "Syncing repository '{}' (Branch/Ref: {})...", 
        repo_name_str.cyan(), 
        current_sync_identifier.cyan()
    );

    // Initialize git manager for enhanced sync detection
    let mut git_manager = GitManager::new();
    git_manager.initialize_repository(&repo_path).await?;

    // Calculate sync requirements
    let sync_req = git_manager.calculate_sync_requirements(&repo_path, &current_sync_identifier).await?;
    
    match sync_req.sync_type {
        SyncType::None => {
            println!("✅ Repository '{}' is already up to date.", repo_name_str.green());
            return Ok(());
        },
        SyncType::Incremental => {
            println!("🔄 Incremental sync required: {} files to update", 
                sync_req.files_to_update.len() + sync_req.files_to_add.len());
        },
        SyncType::Full => {
            println!("🔄 Full resync required");
        }
    }

    // Create progress reporter
    let progress_reporter = Arc::new(IndicatifProgressReporter::new());

    // Create sync options
    let options = SyncOptions {
        force: args.force,
        extensions: args.extensions,
    };

    match sync_repository(
        client,
        repo_config,
        options,
        &app_config_clone, // Pass reference instead of Arc<RwLock<>>
        Some(progress_reporter),
    ).await {
        Ok(sync_result) => {
            if sync_result.success {
                println!("✅ {}", sync_result.message);
                
                // Update the config with the new commit hash
                if let Some(commit) = sync_result.last_synced_commit {
                    let current_sync_identifier = repo_config.active_branch.as_deref()
                        .or(repo_config.target_ref.as_deref())
                        .unwrap_or(&repo_config.default_branch)
                        .to_string();
                    
                    let repo_name_for_update = repo_config.name.clone();
                    if let Some(repo_mut) = config.repositories.iter_mut().find(|r| r.name == repo_name_for_update) {
                        // Update last_synced_commits map
                        repo_mut.last_synced_commits.insert(current_sync_identifier, commit);
                        repo_mut.indexed_languages = Some(sync_result.indexed_languages);
                    }
                }

                if let Err(e) = save_config(config, override_path) {
                    error!("Failed to save config after sync: {e}");
                    println!("{}", "Warning: Failed to save configuration after successful sync.".red());
                } else {
                     println!("{}", "Configuration saved.".dimmed());
                }
            } else {
                error!("Sync failed: {}", sync_result.message);
                println!("{}", format!("❌ Sync failed: {}", sync_result.message).red());
                return Err(anyhow!("Sync failed: {}", sync_result.message));
            }
        }
        Err(e) => {
            error!("Sync error: {e}");
            println!("{}", format!("❌ Sync error: {e}").red());
            return Err(e.into());
        }
    }

    let duration = start_time.elapsed();
    println!("{}", format!("Sync completed in {:.2}s", duration.as_secs_f64()).dimmed());

    Ok(())
} 