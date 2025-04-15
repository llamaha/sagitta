// src/cli/repo_commands.rs
mod list;
mod r#use;
mod clear;
mod query;
pub mod helpers; // Make public

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use colored::*;
use git2::{Repository, DiffOptions, DiffFindOptions, Delta, DiffDelta};
use qdrant_client::Qdrant;
use std::{fs, path::PathBuf, sync::Arc, collections::HashMap};
use log;

use crate::config::{self, AppConfig};
use crate::cli::commands::{CliArgs};

const COLLECTION_NAME_PREFIX: &str = "repo_";
pub(crate) const FIELD_BRANCH: &str = "branch";
pub(crate) const FIELD_COMMIT_HASH: &str = "commit_hash";

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct RepoArgs {
    #[command(subcommand)]
    command: RepoCommand,
}

#[derive(Subcommand, Debug)]
#[derive(Clone)]
enum RepoCommand {
    /// Add a new repository to manage.
    Add(AddRepoArgs),
    /// List managed repositories.
    List,
    /// Set the active repository for commands.
    Use(r#use::UseRepoArgs),
    /// Remove a managed repository (config and index).
    Remove(RemoveRepoArgs),
    /// Clear the index for a repository.
    Clear(clear::ClearRepoArgs),
    /// Checkout a branch and set it as active for the current repository.
    UseBranch(UseBranchArgs),
    /// Query the index for a specific repository.
    Query(query::RepoQueryArgs),
    /// Fetch updates and sync the index for the current/specified repository.
    Sync(SyncRepoArgs),
}

#[derive(Args, Debug)]
#[derive(Clone)]
struct AddRepoArgs {
    #[arg(short = 'p', long, value_parser)]
    /// Local path where the repository should be cloned or already exists.
    local_path: Option<PathBuf>,

    #[clap(long, value_parser)]
    /// URL of the repository to clone (e.g., https://gitlab.com/user/repo.git)
    url: String,

    /// Optional custom name for the repository (defaults to deriving from URL).
    #[arg(short, long)]
    name: Option<String>,

    /// Optional specific branch to track initially (defaults to the repo's default).
    #[arg(short, long)]
    branch: Option<String>,

    /// Optional name for the Git remote (defaults to "origin").
    #[arg(short = 'r', long)]
    remote: Option<String>,

    /// Optional path to the SSH private key for authentication.
    #[arg(long)]
    ssh_key: Option<PathBuf>,

    /// Optional passphrase for the SSH private key.
    #[arg(long, requires = "ssh_key")] // Passphrase only makes sense if key is provided
    ssh_passphrase: Option<String>,
}

#[derive(Args, Debug)]
#[derive(Clone)]
struct RemoveRepoArgs {
    /// Name of the repository to remove.
    name: String,

    /// Skip confirmation prompt.
    #[arg(short, long, default_value_t = false)]
    yes: bool,
}

#[derive(Args, Debug)]
#[derive(Clone)]
struct UseBranchArgs {
    /// Name of the branch to checkout and set active.
    name: String,
}

#[derive(Args, Debug)]
#[derive(Clone)]
struct SyncRepoArgs {
    /// Optional name of the repository to sync (defaults to active repository).
    name: Option<String>,

    /// Force re-indexing even if the remote commit hasn't changed.
    #[arg(long, default_value_t = false)]
    force: bool,

    /// Optional: Comma-separated list of file extensions to index (e.g., "rs,toml").
    #[arg(short = 'e', long, value_delimiter = ',')]
    extensions: Option<Vec<String>>,
}

pub async fn handle_repo_command(
    args: RepoArgs,
    cli_args: &CliArgs,
    config: &mut AppConfig,
    client: Arc<Qdrant>,
    override_path: Option<&PathBuf>,
) -> Result<()> {
    match args.command {
        RepoCommand::Add(add_args) => add_repository(add_args, config, client, override_path).await,
        RepoCommand::List => list::list_repositories(config),
        RepoCommand::Use(use_args) => r#use::use_repository(use_args, config, override_path),
        RepoCommand::Remove(remove_args) => remove_repository(remove_args, config, client, override_path).await,
        RepoCommand::Clear(clear_args) => clear::handle_repo_clear(clear_args, config, client, override_path).await,
        RepoCommand::UseBranch(branch_args) => use_branch(branch_args, config, override_path).await,
        RepoCommand::Query(query_args) => query::handle_repo_query(query_args, config, client, cli_args).await,
        RepoCommand::Sync(sync_args) => sync_repository(sync_args, cli_args, config, client, override_path).await,
    }
}

async fn add_repository(
    args: AddRepoArgs,
    config: &mut AppConfig,
    client: Arc<Qdrant>,
    override_path: Option<&PathBuf>,
) -> Result<()> {
    let repo_name = match args.name {
        Some(name) => name,
        None => PathBuf::from(&args.url)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim_end_matches(".git").to_string())
            .ok_or_else(|| anyhow!("Could not derive repository name from URL"))?,
    };

    if config.repositories.iter().any(|r| r.name == repo_name) {
        bail!("Repository '{}' already exists.", repo_name);
    }

    let repo_base_path = config::get_repo_base_path()?;
    fs::create_dir_all(&repo_base_path)
        .with_context(|| format!("Failed to create repository base directory at {}", repo_base_path.display()))?;
    let local_path = args.local_path.unwrap_or(repo_base_path.join(&repo_name));

    let repo = if local_path.exists() {
         println!(
            "{}",
            format!(
                "Local directory '{}' already exists. Assuming it's the intended clone. Skipping clone.",
                local_path.display()
            ).yellow()
        );
        Repository::open(&local_path)
            .with_context(|| format!("Failed to open existing repository at {}", local_path.display()))?
    } else {
        println!("Cloning repository '{}' from {}...", repo_name.cyan(), args.url.cyan());
        let repo_configs_clone = config.repositories.clone();
        let fetch_opts = helpers::create_fetch_options(repo_configs_clone, &args.url)?;

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);

        let repo = builder.clone(&args.url, &local_path)
            .with_context(|| format!("Failed to clone repository from {}", args.url))?;
        println!("Repository cloned successfully to {}", local_path.display());
        repo
    };

    let initial_branch_name = match args.branch {
        Some(branch_name) => branch_name,
        None => {
            let head_ref = repo.find_reference("HEAD")?;
            let head_ref_resolved = head_ref.resolve()?;
            head_ref_resolved.shorthand()
                .ok_or_else(|| anyhow!("Could not determine default branch name from HEAD"))?
                .to_string()
        }
    };
    println!("Default/Initial branch detected: {}", initial_branch_name.cyan());


    let collection_name = helpers::get_collection_name(&repo_name);
    println!("Ensuring Qdrant collection '{}' exists...", collection_name.cyan());
    helpers::ensure_repository_collection_exists(&client, &collection_name).await?;
    println!("Qdrant collection ensured.");

    let new_repo_config = config::RepositoryConfig {
        name: repo_name.clone(),
        url: args.url.clone(),
        local_path: local_path.clone(),
        default_branch: initial_branch_name.clone(),
        tracked_branches: vec![initial_branch_name.clone()],
        active_branch: Some(initial_branch_name.clone()),
        remote_name: Some(args.remote.unwrap_or_else(|| "origin".to_string())),
        ssh_key_path: args.ssh_key.clone(),
        ssh_key_passphrase: args.ssh_passphrase.clone(),
        last_synced_commits: HashMap::new(),
        indexed_languages: None,
    };

    config.repositories.push(new_repo_config);
    config.active_repository = Some(repo_name.clone());
    config::save_config(&config, override_path)?;

    println!("Set '{}' as the active repository.", repo_name.cyan());
    println!("{}", "Successfully added repository configuration.".green());
    println!("Run '{}' to fetch and index the '{}' branch.",
             format!("vectordb-cli repo sync {}", repo_name).cyan(),
             initial_branch_name.cyan());


    Ok(())
}

async fn remove_repository(
    args: RemoveRepoArgs, 
    config: &mut AppConfig,
    client: Arc<Qdrant>,
    override_path: Option<&PathBuf>,
) -> Result<()> {
    let repo_name = &args.name;

    println!(
        "{}",
        format!(
            "Preparing to remove repository '{}'. This will delete configuration, Qdrant data, and the local clone.",
            repo_name.cyan()
        ).yellow()
    );
    println!("{}", "This action CANNOT be undone.".red().bold());

    // Add confirmation prompt if -y is not passed
    if !args.yes {
        println!("Are you sure you want to continue? (yes/No)");
        let mut confirmation = String::new();
        std::io::stdin().read_line(&mut confirmation)
            .context("Failed to read confirmation input")?;
        if confirmation.trim().to_lowercase() != "yes" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    // Find the repository index
    let repo_config_index = match config.repositories.iter().position(|r| r.name == *repo_name) {
        Some(index) => index,
        None => bail!("Repository '{}' not found.", repo_name),
    };

    // Get details before removing from config
    let repo_config = config.repositories[repo_config_index].clone(); 

    // Attempt to delete the Qdrant collection
    let collection_name = helpers::get_collection_name(repo_name);
    println!("Attempting to delete Qdrant collection '{}'...", collection_name.cyan());
    match client.delete_collection(&collection_name).await {
        Ok(response) => {
            // Check the result field within the response
            if response.result {
                println!("Successfully deleted Qdrant collection '{}'.", collection_name.green());
            } else {
                // This case might indicate the collection didn't exist or another non-error condition
                 println!("Qdrant reported non-success deleting collection '{}'. It might not have existed.", collection_name.yellow());
            }
        }
        Err(e) => {
            // Log error but continue with removing config/local dir
            log::error!(
                "Failed to delete Qdrant collection '{}': {}. Please check Qdrant manually.", 
                collection_name, e
            );
            println!(
                "{}", 
                format!("Warning: Failed to delete Qdrant collection '{}'. Please check Qdrant manually.", collection_name).yellow()
            );
        }
    }

    // Attempt to remove the local repository directory
    println!("Attempting to remove local clone at {}...", repo_config.local_path.display());
    if repo_config.local_path.exists() {
        match fs::remove_dir_all(&repo_config.local_path) {
            Ok(_) => println!("Successfully removed local directory '{}'.", repo_config.local_path.display().to_string().green()),
            Err(e) => {
                // Log error but continue with removing config
                 log::error!("Failed to remove local directory '{}': {}. Please remove it manually.", repo_config.local_path.display(), e);
                 println!(
                    "{}", 
                    format!("Warning: Failed to remove local directory '{}'. Please remove it manually.", repo_config.local_path.display()).yellow()
                );
            }
        }
    } else {
        println!("Local directory '{}' does not exist. Skipping removal.", repo_config.local_path.display().to_string().yellow());
    }

    // Remove the repository from the config
    println!("Removing repository configuration for '{}'.", repo_name.cyan());
    config.repositories.remove(repo_config_index);

    // If the removed repository was the active one, unset it
    if config.active_repository.as_deref() == Some(repo_name) {
        config.active_repository = None;
        println!("Removed repository was active, active repository is now unset.");
        // Optionally, set active to the first remaining repo?
        if let Some(first_repo) = config.repositories.first() {
             config.active_repository = Some(first_repo.name.clone());
             println!("Set active repository to '{}'.", first_repo.name.cyan());
         }
    }

    // Save the updated configuration
    config::save_config(config, override_path)
        .context("Failed to save configuration after removing repository")?;

    println!("{}", format!("Successfully removed repository '{}'.", repo_name.cyan()).green());

    Ok(())
}

async fn use_branch(args: UseBranchArgs, config: &mut AppConfig, override_path: Option<&PathBuf>) -> Result<()> {
    let repo_name = match config.active_repository.clone() {
        Some(name) => name,
        None => bail!("No active repository set. Use 'repo use <name>' first."),
    };

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name)
        .ok_or_else(|| anyhow::anyhow!("Active repository '{}' configuration not found.", repo_name))?;

    let repo_config = &config.repositories[repo_config_index];

    let repo = Repository::open(&repo_config.local_path)
        .with_context(|| format!("Failed to open repository at {}", repo_config.local_path.display()))?;

    let target_branch_name = &args.name;
    let remote_name = repo_config.remote_name.as_deref().unwrap_or("origin");
    let repo_url = repo_config.url.clone();

    if repo.find_branch(target_branch_name, git2::BranchType::Local).is_err() {
        println!(
            "Local branch '{}' not found. Checking remote '{}'...",
            target_branch_name, remote_name
        );
        
        println!("Fetching from remote '{}' to update refs...", remote_name);
        let mut remote = repo.find_remote(remote_name)?;
        let repo_configs_clone = config.repositories.clone();
        let mut fetch_opts = helpers::create_fetch_options(repo_configs_clone, &repo_url)?;
        remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)
            .with_context(|| format!("Failed initial fetch from remote '{}' before branch check", remote_name))?;
        println!("Fetch for refs update complete.");

        let remote_branch_ref = format!("{}/{}", remote_name, target_branch_name);
        match repo.find_branch(&remote_branch_ref, git2::BranchType::Remote) {
            Ok(remote_branch) => {
                println!(
                    "Branch '{}' found on remote '{}'. Creating local tracking branch...",
                    target_branch_name, remote_name
                );
                let commit = remote_branch.get().peel_to_commit()
                    .with_context(|| format!("Failed to get commit for remote branch {}", remote_branch_ref))?;
                repo.branch(target_branch_name, &commit, false)
                    .with_context(|| format!("Failed to create local branch '{}'", target_branch_name))?;
                let mut local_branch = repo.find_branch(target_branch_name, git2::BranchType::Local)?;
                local_branch.set_upstream(Some(&remote_branch_ref))
                    .with_context(|| format!("Failed to set upstream for branch '{}' to '{}'", target_branch_name, remote_branch_ref))?;
            }
            Err(_) => {
                bail!(
                    "Branch '{}' not found locally or on remote '{}'.",
                    target_branch_name,
                    remote_name
                );
            }
        }
    }

    println!("Checking out branch '{}'...", target_branch_name);
    let ref_name = format!("refs/heads/{}", target_branch_name);
    repo.set_head(&ref_name)
        .with_context(|| format!("Failed to checkout branch '{}'", target_branch_name))?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .with_context(|| format!("Failed to force checkout head for branch '{}'", target_branch_name))?;

    let repo_config_mut = &mut config.repositories[repo_config_index];
    repo_config_mut.active_branch = Some(target_branch_name.to_string());
    if !repo_config_mut.tracked_branches.contains(target_branch_name) {
        repo_config_mut.tracked_branches.push(target_branch_name.to_string());
    }

    config::save_config(&config, override_path)?;

    println!(
        "{}",
        format!(
            "Switched to branch '{}' for repository '{}'.",
            target_branch_name,
            repo_name.cyan()
        ).green()
    );

    Ok(())
}

async fn sync_repository(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, RepositoryConfig, load_config, save_config}; 
    use crate::cli::commands::Commands;
    use crate::cli::repo_commands::{RepoArgs, RepoCommand};
    use qdrant_client::{Qdrant};
    use std::sync::Arc;
    use tokio::runtime::Runtime;
    use std::collections::HashMap;
    use std::path::{PathBuf};
    use std::fs;
    use tempfile::{tempdir};

    // Helper function to create a default AppConfig for tests
    fn create_test_config_data() -> AppConfig {
        AppConfig {
            repositories: vec![
                RepositoryConfig { name: "repo1".to_string(), url: "url1".to_string(), local_path: PathBuf::from("/tmp/vectordb_test_repo1"), default_branch: "main".to_string(), tracked_branches: vec!["main".to_string()], active_branch: Some("main".to_string()), remote_name: Some("origin".to_string()), ssh_key_path: None, ssh_key_passphrase: None, last_synced_commits: HashMap::new(), indexed_languages: None },
                RepositoryConfig { name: "repo2".to_string(), url: "url2".to_string(), local_path: PathBuf::from("/tmp/vectordb_test_repo2"), default_branch: "dev".to_string(), tracked_branches: vec!["dev".to_string()], active_branch: Some("dev".to_string()), remote_name: Some("origin".to_string()), ssh_key_path: None, ssh_key_passphrase: None, last_synced_commits: HashMap::new(), indexed_languages: None },
            ],
            active_repository: None,
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
        }
    }

    // Helper function to create dummy CliArgs
     fn create_dummy_cli_args(repo_command: RepoCommand) -> CliArgs {
        // Add default dummy paths for ONNX, tests needing real paths should override
        let dummy_model_path = Some(PathBuf::from("/tmp/dummy_model.onnx"));
        let dummy_tokenizer_dir = Some(PathBuf::from("/tmp/dummy_tokenizer/"));

        CliArgs {
             command: Commands::Repo(RepoArgs { command: repo_command }),
             // Convert PathBuf to String
             onnx_model_path_arg: dummy_model_path.map(|p| p.to_string_lossy().into_owned()),
             onnx_tokenizer_dir_arg: dummy_tokenizer_dir.map(|p| p.to_string_lossy().into_owned()),
         }
      }

    // --- Updated Tests --- 
    // Note: repo clear tests might still need Qdrant connection or mocking
    // They don't save config, so isolation isn't strictly needed for that
    #[test]
    fn test_handle_repo_clear_specific_repo() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
            // Use create_test_config_data directly, no need for temp file here
            let mut config = create_test_config_data(); 
            let test_repo_name = "my-test-repo-clear-specific"; 
             config.repositories.push(RepositoryConfig { name: test_repo_name.to_string(), /* .. other fields .. */ url: "url_clear".to_string(), local_path: PathBuf::from("/tmp/clear_spec"), default_branch: "main".to_string(), tracked_branches: vec![], active_branch: None, remote_name: None, ssh_key_path: None, ssh_key_passphrase: None, last_synced_commits: HashMap::new(), indexed_languages: None});
             config.active_repository = Some("repo1".to_string()); 
            
            let args = clear::ClearRepoArgs { name: Some(test_repo_name.to_string()), yes: true };
            let dummy_cli_args = create_dummy_cli_args(RepoCommand::Clear(args.clone()));
            let _ = client.delete_collection(&helpers::get_collection_name(test_repo_name)).await; 

            let result = handle_repo_command(RepoArgs{ command: RepoCommand::Clear(args)}, &dummy_cli_args, &mut config, client, None).await;
            assert!(result.is_ok());
        });
    }
    #[test]
    fn test_handle_repo_clear_active_repo() {
         let rt = Runtime::new().unwrap();
         rt.block_on(async {
             let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
             // Use create_test_config_data directly
             let mut config = create_test_config_data(); 
             let active_repo_name = "my-test-repo-clear-active"; 
             config.repositories.push(RepositoryConfig { name: active_repo_name.to_string(), /* .. other fields .. */ url: "url_clear_active".to_string(), local_path: PathBuf::from("/tmp/clear_active"), default_branch: "main".to_string(), tracked_branches: vec![], active_branch: None, remote_name: None, ssh_key_path: None, ssh_key_passphrase: None, last_synced_commits: HashMap::new(), indexed_languages: None});
             config.active_repository = Some(active_repo_name.to_string());

             let args = clear::ClearRepoArgs { name: None, yes: true }; // Clear active
             let dummy_cli_args = create_dummy_cli_args(RepoCommand::Clear(args.clone()));
             let _ = client.delete_collection(&helpers::get_collection_name(active_repo_name)).await;

             let result = handle_repo_command(RepoArgs{ command: RepoCommand::Clear(args)}, &dummy_cli_args, &mut config, client, None).await;
             assert!(result.is_ok());

             // Add assertion for config state change if desired (e.g., sync status cleared)
             // let updated_repo = config.repositories.iter().find(|r| r.name == active_repo_name);
             // assert!(updated_repo.is_some());
             // assert!(updated_repo.unwrap().last_synced_commits.is_empty());
         });
    }
    #[test]
    fn test_handle_repo_clear_no_active_or_specified_fails() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
            // Use create_test_config_data directly
            let mut config = create_test_config_data();
            config.repositories.clear();
            config.active_repository = None;

            let args = clear::ClearRepoArgs { name: None, yes: true }; 
            let dummy_cli_args = create_dummy_cli_args(RepoCommand::Clear(args.clone()));

            let result = handle_repo_command(RepoArgs{ command: RepoCommand::Clear(args)}, &dummy_cli_args, &mut config, client, None).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("No active repository set"));
        });
    }

     #[test]
     fn test_handle_repo_use_existing() {
         let temp_dir = tempdir().unwrap(); // Use tempdir
         let temp_path = temp_dir.path().join("test_config.toml"); // Define path within tempdir

         let mut config = create_test_config_data();
         config.active_repository = Some("repo1".to_string());
         save_config(&config, Some(&temp_path)).unwrap(); // Save initial state to temp path

         let use_args = r#use::UseRepoArgs { name: "repo2".to_string() };
         let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); 
         let dummy_cli_args = create_dummy_cli_args(RepoCommand::Use(use_args.clone()));

         // Pass Some(&temp_path) as override_path
         let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
              handle_repo_command(RepoArgs{ command: RepoCommand::Use(use_args)}, &dummy_cli_args, &mut config, client, Some(&temp_path)).await
         });
         assert!(result.is_ok());
         
         // Verify by loading from the temporary file
         let saved_config = load_config(Some(&temp_path)).unwrap();
         assert_eq!(saved_config.active_repository, Some("repo2".to_string()));

         // Keep temp_dir alive until end of test scope automatically
     }

     #[test]
     fn test_handle_repo_use_nonexistent() {
        let temp_dir = tempdir().unwrap(); // Use tempdir
        let temp_path = temp_dir.path().join("test_config.toml"); // Define path within tempdir

        let mut config = create_test_config_data();
        save_config(&config, Some(&temp_path)).unwrap();
        let initial_config_state = config.clone(); // Save for comparison
        
        let use_args = r#use::UseRepoArgs { name: "repo3".to_string() }; 
        let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); 
        let dummy_cli_args = create_dummy_cli_args(RepoCommand::Use(use_args.clone()));

        let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
              handle_repo_command(RepoArgs{ command: RepoCommand::Use(use_args)}, &dummy_cli_args, &mut config, client, Some(&temp_path)).await
         });
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Repository 'repo3' not found"));

        // Verify config file was NOT changed because the command errored before saving
        let saved_config = load_config(Some(&temp_path)).unwrap();
        assert_eq!(saved_config.repositories, initial_config_state.repositories);
        assert_eq!(saved_config.active_repository, initial_config_state.active_repository);

        // Keep temp_dir alive until end of test scope automatically
     }

     #[test]
     fn test_handle_repo_remove_config_only_non_active() {
        let rt = Runtime::new().unwrap();
         rt.block_on(async {
             let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
             let temp_dir = tempdir().unwrap(); // Use tempdir
             let temp_path = temp_dir.path().join("test_config.toml"); // Define path within tempdir

             let mut config = create_test_config_data();
             config.active_repository = Some("repo1".to_string());
             save_config(&config, Some(&temp_path)).unwrap();
             let initial_repo_count = config.repositories.len();
             
             let remove_args = RemoveRepoArgs { name: "repo2".to_string(), yes: true }; 
             let dummy_cli_args = create_dummy_cli_args(RepoCommand::Remove(remove_args.clone()));
             let _ = fs::remove_dir_all("/tmp/vectordb_test_repo2"); // Keep dummy dir removal

             let result = handle_repo_command(RepoArgs{ command: RepoCommand::Remove(remove_args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
             assert!(result.is_ok());
             
             // Verify by loading from the temporary file
             let saved_config = load_config(Some(&temp_path)).unwrap();
             assert_eq!(saved_config.repositories.len(), initial_repo_count - 1);
             assert!(!saved_config.repositories.iter().any(|r| r.name == "repo2"));
             assert_eq!(saved_config.active_repository, Some("repo1".to_string()));

             // Keep temp_dir alive until end of test scope automatically
         });
     }

      #[test]
      fn test_handle_repo_remove_config_only_active() {
         let rt = Runtime::new().unwrap();
          rt.block_on(async {
              let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
              let temp_dir = tempdir().unwrap(); // Use tempdir
              let temp_path = temp_dir.path().join("test_config.toml"); // Define path within tempdir

              let mut config = create_test_config_data();
              config.active_repository = Some("repo2".to_string());
              config.repositories.push(RepositoryConfig { name: "repo3".to_string(), url: "url3".to_string(), local_path: PathBuf::from("/tmp/vectordb_test_repo3"), default_branch: "main".to_string(), tracked_branches: vec!["main".to_string()], active_branch: Some("main".to_string()), remote_name: Some("origin".to_string()), ssh_key_path: None, ssh_key_passphrase: None, last_synced_commits: HashMap::new(), indexed_languages: None });
              save_config(&config, Some(&temp_path)).unwrap();
              let initial_repo_count = config.repositories.len();

              let remove_args = RemoveRepoArgs { name: "repo2".to_string(), yes: true };
              let dummy_cli_args = create_dummy_cli_args(RepoCommand::Remove(remove_args.clone()));
              let _ = fs::remove_dir_all("/tmp/vectordb_test_repo2");

              let result = handle_repo_command(RepoArgs{ command: RepoCommand::Remove(remove_args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
              assert!(result.is_ok());
              
              // Verify by loading from the temporary file
              let saved_config = load_config(Some(&temp_path)).unwrap();
              assert_eq!(saved_config.repositories.len(), initial_repo_count - 1);
              assert!(!saved_config.repositories.iter().any(|r| r.name == "repo2"));
              assert_eq!(saved_config.active_repository, Some("repo1".to_string())); // Should switch to repo1

              // Keep temp_dir alive until end of test scope automatically
          });
      }

       #[test]
       fn test_handle_repo_remove_nonexistent() {
          let rt = Runtime::new().unwrap();
           rt.block_on(async {
               let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
               let temp_dir = tempdir().unwrap(); // Use tempdir
               let temp_path = temp_dir.path().join("test_config.toml"); // Define path within tempdir

               let mut config = create_test_config_data();
               save_config(&config, Some(&temp_path)).unwrap();
               let initial_config_state = config.clone();
               let initial_repo_count = config.repositories.len();

               let remove_args = RemoveRepoArgs { name: "repo3".to_string(), yes: true }; 
               let dummy_cli_args = create_dummy_cli_args(RepoCommand::Remove(remove_args.clone()));

               let result = handle_repo_command(RepoArgs{ command: RepoCommand::Remove(remove_args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
               assert!(result.is_err());
               assert!(result.unwrap_err().to_string().contains("Repository 'repo3' not found"));

               // Verify config file was NOT changed
               let saved_config = load_config(Some(&temp_path)).unwrap();
               assert_eq!(saved_config.repositories, initial_config_state.repositories);
               assert_eq!(saved_config.repositories.len(), initial_repo_count);

               // Keep temp_dir alive until end of test scope automatically
           });
       }

    // Keep repo list test as is, it doesn't save config
    #[test]
    fn test_handle_repo_list() {
        // Setup config
        let mut config = create_test_config_data();
        config.active_repository = Some("repo1".to_string());

        // Call list_repositories directly or via handle_repo_command
        // Since list doesn't modify/save, override_path isn't strictly needed, but let's pass None for consistency
        let list_args = RepoArgs { command: RepoCommand::List };
        let dummy_cli_args = create_dummy_cli_args(RepoCommand::List);
        let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); // Dummy client needed for handle_repo_command signature

        let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
             handle_repo_command(list_args, &dummy_cli_args, &mut config, client, None).await // Pass None
        });

        // List command prints to stdout, so we'd typically capture stdout to assert output
        // For now, just assert it runs without error
        assert!(result.is_ok());
    }

    // TODO: Add tests for sync_repository, especially for the extension filter.
    // #[tokio::test]
    // async fn test_sync_with_extension_filter() { ... }

    // #[tokio::test]
    // async fn test_sync_without_extension_filter() { ... }

    // #[tokio::test]
    // async fn test_sync_with_invalid_extension_filter() { ... }
}

