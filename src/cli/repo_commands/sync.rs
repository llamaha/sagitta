use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::*;
use git2::{Repository, DiffOptions, DiffFindOptions, Delta, DiffDelta};
use qdrant_client::Qdrant;
use std::{path::PathBuf, sync::Arc};
use log;

use crate::config::{self, AppConfig};
use crate::cli::commands::CliArgs;
use crate::cli::repo_commands::helpers;

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

pub async fn handle_repo_sync(
    args: SyncRepoArgs, 
    cli_args: &CliArgs,
    config: &mut AppConfig,
    client: Arc<Qdrant>,
    override_path: Option<&PathBuf>,
) -> Result<()> {
    let repo_name_ref = args.name.as_ref().or(config.active_repository.as_ref())
        .ok_or_else(|| anyhow!("No active repository set and no repository name provided with --name."))?;
    let repo_name = repo_name_ref.clone();
    
    let repo_config_index = config.repositories.iter()
        .position(|r| r.name == repo_name)
        .ok_or_else(|| anyhow!("Configuration for repository '{}' not found.", repo_name))?;
    
    let repo_config = config.repositories[repo_config_index].clone();

    let active_branch = repo_config.active_branch
        .as_ref()
        .ok_or_else(|| anyhow!("Repository '{}' has no active branch set. Use 'use-branch' command.", repo_name))?;

    println!(
        "Syncing repository '{}' (Branch: {})...", 
        repo_name.cyan(), 
        active_branch.cyan()
    );

    let repo = Repository::open(&repo_config.local_path)
        .with_context(|| format!("Failed to open repository at {}", repo_config.local_path.display()))?;

    // --- Fetch updates --- 
    println!("Fetching updates from remote '{}'...", repo_config.remote_name.as_deref().unwrap_or("origin").cyan());
    let remote = repo.find_remote(repo_config.remote_name.as_deref().unwrap_or("origin"))
        .with_context(|| format!("Failed to find remote '{}'", repo_config.remote_name.as_deref().unwrap_or("origin"))) ;
    
    let mut remote = match remote {
        Ok(r) => r,
        Err(e) => {
            bail!("Error finding remote: {}. Ensure the remote name ('{}') is correct in the config or the repository.", e, repo_config.remote_name.as_deref().unwrap_or("origin"));
        }
    };

    // Clone repositories from config BEFORE calling create_fetch_options
    let repo_configs_clone = config.repositories.clone();
    let mut fetch_opts = helpers::create_fetch_options(repo_configs_clone, &repo_config.url)?;
    // Pass fetch_opts mutably without cloning
    remote.fetch(&[active_branch], Some(&mut fetch_opts), None)
         .context("Failed to fetch updates from remote repository")?;
    println!("Fetch complete.");

    // --- Get Local and Remote Commit OIDs --- 
    let local_branch_ref_name = format!("refs/heads/{}", active_branch);
    let local_ref = repo.find_reference(&local_branch_ref_name)
        .with_context(|| format!("Failed to find local branch reference '{}'", local_branch_ref_name))?;
    let local_commit_oid = local_ref.target()
        .ok_or_else(|| anyhow!("Failed to get OID for local branch '{}'", active_branch))?;
    let local_commit_oid_str = local_commit_oid.to_string();
    println!("Local commit:  {}", local_commit_oid_str.yellow());

    let remote_branch_ref_name = format!("refs/remotes/{}/{}", repo_config.remote_name.as_deref().unwrap_or("origin"), active_branch);
    let remote_ref = repo.find_reference(&remote_branch_ref_name)
        .with_context(|| format!("Failed to find remote branch reference '{}'", remote_branch_ref_name))?;
    let remote_commit_oid = remote_ref.target()
        .ok_or_else(|| anyhow!("Failed to get OID for remote branch '{}'", remote_branch_ref_name))?;
    let remote_commit_oid_str = remote_commit_oid.to_string();
    println!("Remote commit: {}", remote_commit_oid_str.yellow());

    let remote_commit = repo.find_commit(remote_commit_oid)?;

    // --- Check if Sync Needed --- 
    let last_synced_commit = repo_config.last_synced_commits.get(active_branch);

    // Skip check if --force is used
    if !args.force && last_synced_commit.as_deref() == Some(&remote_commit_oid_str) {
        println!("Repository branch is already up-to-date and synced.");
        helpers::merge_local_branch(&repo, active_branch, &remote_commit)?;
        return Ok(());
    }
    
    if args.force {
        println!("{}", "--force specified, proceeding with sync regardless of commit hash.".yellow());
    }

    // --- Perform Merge (Fast-forward if possible) --- 
    helpers::merge_local_branch(&repo, active_branch, &remote_commit)?;
    repo.set_head(&local_branch_ref_name)
         .with_context(|| format!("Failed to set HEAD to '{}' after potential merge", local_branch_ref_name))?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
         .context("Failed checkout head after setting HEAD")?;
    log::debug!("HEAD reset to {} after merge attempt.", local_branch_ref_name);

    // --- Calculate Diff --- 
    let old_tree = match last_synced_commit {
        Some(oid_str) => {
            let oid = git2::Oid::from_str(oid_str)?;
            match repo.find_commit(oid) {
                 Ok(commit) => Some(commit.tree()?), 
                 Err(e) => {
                      log::warn!("Could not find last synced commit '{}' locally: {}. Performing full index.", oid_str, e);
                      None
                 }
            }
        }
        None => {
            log::info!("No previous sync found for branch '{}'. Performing initial full index.", active_branch);
            None
        }
    };
    let new_tree = remote_commit.tree()?;

    let mut diff_opts = DiffOptions::new();
    diff_opts.include_untracked(false);
    diff_opts.ignore_submodules(true);

    println!("Calculating differences...");
    let mut diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut diff_opts))?;

    let mut files_to_add = Vec::new();
    let mut files_to_delete = Vec::new();
    let mut files_to_update = Vec::new();

    let mut diff_find_opts = DiffFindOptions::new();
    diff.find_similar(Some(&mut diff_find_opts))?;

    diff.foreach(
        &mut |delta: DiffDelta<'_>, _progress: f32| {
            let old_path = delta.old_file().path().map(PathBuf::from);
            let new_path = delta.new_file().path().map(PathBuf::from);

            match delta.status() {
                Delta::Added => {
                    if let Some(p) = new_path { files_to_add.push(p); }
                }
                Delta::Deleted => {
                    if let Some(p) = old_path { files_to_delete.push(p); }
                }
                Delta::Modified => {
                    if let Some(p) = new_path { files_to_update.push(p); }
                }
                Delta::Renamed => {
                     if let Some(op) = old_path { files_to_delete.push(op); }
                     if let Some(np) = new_path { files_to_add.push(np); } 
                 }
                 Delta::Copied => {
                     if let Some(p) = new_path { files_to_add.push(p); }
                 }
                _ => {}
            }
            true
        },
        None,
        None,
        None,
    )?;

    println!(
        "Diff analysis: {} added, {} deleted, {} modified.", 
        files_to_add.len(), 
        files_to_delete.len(), 
        files_to_update.len()
    );

    let collection_name = helpers::get_collection_name(&repo_name);

    // --- Delete Obsolete Points --- 
    if !files_to_delete.is_empty() {
        println!("Deleting points for {} removed/renamed files...", files_to_delete.len());
        helpers::delete_points_for_files(&client, &collection_name, active_branch, &files_to_delete).await?;
    } else {
        log::debug!("No files marked for deletion in diff.");
    }

    // --- Index New/Updated Points --- 
    let files_to_index: Vec<PathBuf> = files_to_add.into_iter()
        .chain(files_to_update.into_iter())
        .collect();

    // --- Filter files by extension if provided ---
    let filtered_files_to_index = match &args.extensions {
        Some(allowed_extensions) => {
            let allowed_extensions_lower: Vec<String> = allowed_extensions
                .iter()
                .map(|ext| ext.trim().to_lowercase())
                .filter(|ext| !ext.is_empty()) // Remove empty strings after split
                .collect();

            if allowed_extensions_lower.is_empty() {
                 log::warn!("-e/--extensions flag was provided but contained no valid extensions after trimming.");
                 files_to_index // Proceed without filtering if input was invalid
            } else {
                log::debug!("Filtering sync for extensions: {:?}", allowed_extensions_lower);
                files_to_index
                    .into_iter()
                    .filter(|path| {
                        path.extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext_str| allowed_extensions_lower.contains(&ext_str.to_lowercase()))
                            .unwrap_or(false) // Exclude files without extensions or non-UTF8 extensions
                    })
                    .collect()
            }
        }
        None => files_to_index, // No filter provided, index all changed files
    };

    if !filtered_files_to_index.is_empty() {
        println!("Indexing {} added/modified files...", filtered_files_to_index.len());
        helpers::index_files(
            &client,
            cli_args,
            config,
            &repo_config.local_path,
            &filtered_files_to_index,
            &collection_name,
            active_branch,
            &remote_commit_oid_str,
        ).await?;
    } else {
         log::debug!("No files marked for indexing in diff.");
    }

    // --- Update Config --- 
    println!("Updating sync status in configuration...");
    helpers::update_sync_status_and_languages(
        config,
        repo_config_index, 
        active_branch, 
        &remote_commit_oid_str, 
        &client, 
        &collection_name
    ).await?;

    config::save_config(config, override_path)
        .context("Failed to save updated configuration after sync")?;

    println!("Sync completed successfully for repository '{}', branch '{}'.", repo_name.cyan(), active_branch.cyan());

    Ok(())
} 